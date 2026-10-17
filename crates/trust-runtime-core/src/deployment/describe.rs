//! Describe a program for deployment: the producer side of the record (specification 34
//! §5.1.1). The development computer runs it when it builds a project; the code is the same
//! shared core as on every target.

use super::{
    identity, ApplicationIdentity, Capabilities, Contracts, DeploymentRecord, ImportKind,
    NumericContract, ProcessImageSummary, RequestedLimits, RequiredImport, SchedulingProfile,
    TaskSummary, TimingEvidence,
};
use crate::bytecode::{
    BytecodeModule, InitializerEntry, PouEntry, PouKind, SectionData, SectionId,
    NATIVE_CALL_KIND_STDLIB,
};
use crate::error::{ImportRefusal, RuntimeError};
use crate::stdlib::contracts::{self, ImportCapability};
use crate::vm::{PreparationLimits, PreparedModule};
use alloc::vec::Vec;
use smol_str::SmolStr;

/// The `CALL_NATIVE` opcode (specification 12): call kind, descriptor string and argument count.
const CALL_NATIVE: u8 = 0x09;

impl DeploymentRecord {
    /// The record of `program`, requesting `limits`, built with the configuration whose identity
    /// is `configuration_sha256` (`configuration_identity`).
    ///
    /// The program is first prepared under `limits`, exactly as a runtime prepares it, so a
    /// record exists only for a program that a runtime admitting these limits can run. The
    /// record requires `cooperative-resource-v1`, numeric contract 1 and no timing evidence.
    ///
    /// # Errors
    ///
    /// Preparation refuses the program, for example a legacy STBC 1.1 artifact, more than one
    /// resource, or a limit the program exceeds.
    pub fn for_program(
        program: &[u8],
        limits: PreparationLimits,
        configuration_sha256: [u8; 32],
    ) -> Result<Self, RuntimeError> {
        PreparedModule::from_bytes(program, limits)?;
        let module = BytecodeModule::decode(program).map_err(RuntimeError::from)?;
        let (imports, capabilities) = imports(&module)?;
        let resource = match module.section(SectionId::ResourceMeta) {
            Some(SectionData::ResourceMeta(meta)) => meta.resources.first(),
            _ => None,
        }
        .ok_or_else(|| {
            RuntimeError::InvalidBytecodeMetadata(SmolStr::new_static("missing resource"))
        })?;
        let bindings = match module.section(SectionId::IoMap) {
            Some(SectionData::IoMap(map)) => map.bindings.len(),
            _ => 0,
        };
        Ok(Self {
            application: ApplicationIdentity {
                program_sha256: identity::sha256(program),
                program_bytes: u32::try_from(program.len())
                    .map_err(|_| RuntimeError::PreparationLimit)?,
                bytecode_version: module.version,
                bytecode_flags: module.flags,
                configuration_sha256,
                retain_schema_sha256: identity::retain_schema_identity(&module)?,
            },
            contracts: Contracts {
                scheduling_profile: SchedulingProfile::COOPERATIVE_RESOURCE_V1,
                numeric_contract: NumericContract::V1,
                timing_evidence: TimingEvidence::UNQUALIFIED,
                capabilities,
            },
            limits: RequestedLimits::from_preparation(&limits),
            process_image: ProcessImageSummary {
                input_bytes: resource.inputs_size,
                output_bytes: resource.outputs_size,
                marker_bytes: resource.memory_size,
                bindings: u32::try_from(bindings).map_err(|_| RuntimeError::Overflow)?,
            },
            tasks: resource
                .tasks
                .iter()
                .map(|task| TaskSummary {
                    interval_nanos: task.interval_nanos,
                    priority: task.priority,
                    event: task.single_name_idx.is_some(),
                })
                .collect(),
            imports,
        })
    }
}

/// Every built-in import of a prepared program, ordered by kind and name, each once, and the
/// capabilities they need: each standard-library call of a POU body or an initializer, and
/// each function-block template that carries a standard function block's name.
fn imports(module: &BytecodeModule) -> Result<(Vec<RequiredImport>, Capabilities), RuntimeError> {
    let invalid = |what| RuntimeError::InvalidBytecodeMetadata(SmolStr::new_static(what));
    let strings: &[SmolStr] = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(table)) => table.entries.as_slice(),
        _ => &[],
    };
    let bodies: &[u8] = match module.section(SectionId::PouBodies) {
        Some(SectionData::PouBodies(code)) => code.as_slice(),
        _ => &[],
    };
    let pous: &[PouEntry] = match module.section(SectionId::PouIndex) {
        Some(SectionData::PouIndex(index)) => index.entries.as_slice(),
        _ => &[],
    };
    let initializers: &[InitializerEntry] = match module.section(SectionId::Initializers) {
        Some(SectionData::Initializers(index)) => index.entries.as_slice(),
        _ => &[],
    };
    let mut found = Vec::new();
    let mut capabilities = Capabilities::NONE;
    let ranges = pous
        .iter()
        .map(|pou| (pou.code_offset, pou.code_length))
        .chain(
            initializers
                .iter()
                .map(|entry| (entry.code_offset, entry.code_length)),
        );
    for (offset, length) in ranges {
        let start = offset as usize;
        let code = start
            .checked_add(length as usize)
            .and_then(|end| bodies.get(start..end))
            .ok_or_else(|| invalid("executable range"))?;
        let mut pc = 0;
        while pc < code.len() {
            let opcode = code[pc];
            let next = crate::vm::opcode_operand_len(opcode)
                .map(|width| pc + 1 + width)
                .filter(|next| *next <= code.len())
                .ok_or_else(|| invalid("executable instruction"))?;
            if opcode == CALL_NATIVE {
                let operand = |at: usize| {
                    u32::from_le_bytes([code[at], code[at + 1], code[at + 2], code[at + 3]])
                };
                if operand(pc + 1) == NATIVE_CALL_KIND_STDLIB {
                    let descriptor = strings
                        .get(operand(pc + 5) as usize)
                        .ok_or_else(|| invalid("native call descriptor"))?;
                    // The descriptor is the target name, then `|`-separated arguments.
                    let target = descriptor.split('|').next().unwrap_or_default();
                    let name = SmolStr::new(target.to_ascii_uppercase());
                    let import = contracts::function(&name).ok_or_else(|| {
                        RuntimeError::ImportUnsupported {
                            import: name.clone(),
                            reason: ImportRefusal::NoContract,
                        }
                    })?;
                    if import.contract.capability == Some(ImportCapability::WallClock) {
                        capabilities = capabilities.with(Capabilities::WALL_CLOCK);
                    }
                    found.push(RequiredImport {
                        kind: ImportKind::Function,
                        name,
                        contract_version: import.contract.version,
                    });
                }
            }
            pc = next;
        }
    }
    for pou in pous.iter().filter(|pou| pou.kind == PouKind::FunctionBlock) {
        let name = strings
            .get(pou.name_idx as usize)
            .ok_or_else(|| invalid("POU name"))?;
        let name = SmolStr::new(name.to_ascii_uppercase());
        if let Some(block) = contracts::function_block(&name) {
            found.push(RequiredImport {
                kind: ImportKind::FunctionBlock,
                name,
                contract_version: block.contract.version,
            });
        }
    }
    found.sort_by(|left, right| {
        (left.kind, left.name.as_bytes()).cmp(&(right.kind, right.name.as_bytes()))
    });
    found.dedup();
    Ok((found, capabilities))
}
