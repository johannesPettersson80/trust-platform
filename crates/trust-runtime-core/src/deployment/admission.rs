//! Admit a deployment against a device before any state exists (specification 34 §5.1.1,
//! DEPLOY-04, LOAD-07, LOAD-09).

use super::{
    identity, AdmissionRefusal, Capabilities, DeploymentRecord, ImportKind, LimitKind,
    NumericContract, RequestedLimits, RequiredImport, SchedulingProfile, TimingEvidence,
};
use crate::bytecode::{BytecodeVersion, ValidationLimits, BYTECODE_MAX_CONTAINER_BYTES};
use crate::error::{ImportRefusal, RuntimeError};
use crate::stdlib::contracts;
use crate::vm::{PreparationLimits, VM_MAX_CALL_DEPTH};

/// What a device runs and how much it admits (specification 34 COMPAT-02, §5.1.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceDescriptor {
    /// The STBC versions the device prepares.
    pub bytecode_versions: &'static [BytecodeVersion],
    /// The scheduling/I/O profiles the device runs.
    pub scheduling_profiles: &'static [SchedulingProfile],
    /// The numeric contracts the device implements.
    pub numeric_contracts: &'static [NumericContract],
    /// The strongest timing evidence the device has for an application.
    pub timing_evidence: TimingEvidence,
    /// The capabilities the platform supplies.
    pub capabilities: Capabilities,
    /// The largest request the device admits for each preparation limit.
    pub maxima: RequestedLimits,
    /// The newest events the device's event store keeps when a project sets none (0 in the
    /// record's section 7).
    pub event_store_default: u32,
    /// The most a project may set for the device's event store; a record above it is refused.
    pub event_store_max: u32,
    /// The bytes of the slot that holds the program and its record, on a device that has one.
    pub application_slot_bytes: Option<u64>,
    /// The bytes of memory the device has for a program: its prepared program and its engine, the
    /// program's memory figure (specification 34 MEM-12). A chip measures it at boot, before it
    /// loads a program; a computer has no maximum.
    pub program_memory_bytes: Option<u64>,
    /// What the device offers compiled programs, on a device whose versions name
    /// [`BytecodeVersion::NATIVE_IMAGE`] (specification 44 §10.2).
    pub native: Option<NativeTarget>,
}

/// What a device offers compiled programs (specification 44 §10.2, steps 2 and 6, and NX-10): the
/// CPU it runs a module on, the bytes of the task stack a module's entry points run on, and the most
/// statements one task activation may execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeTarget {
    /// The CPU.
    pub target: crate::native::image::Target,
    /// The bytes of the task stack that a program's stack bound must fit (specification 44 §8.5).
    pub task_stack_bytes: u64,
    /// The statement budget of one activation (specification 44 NX-10).
    pub max_statements: u32,
}

impl NativeTarget {
    /// The device the kernel admits an image against (specification 44 §10.2), with
    /// `program_memory_bytes`, the memory the device has for programs; a device without a maximum, a
    /// computer, admits every memory figure.
    #[must_use]
    pub fn device(self, program_memory_bytes: Option<u64>) -> crate::native::program::Device {
        crate::native::program::Device {
            target: self.target,
            program_memory: program_memory_bytes.unwrap_or(u64::MAX),
            task_stack: self.task_stack_bytes,
            max_statements: self.max_statements,
        }
    }
}

/// The STBC versions a source-free runtime prepares.
const SOURCE_FREE_VERSIONS: &[BytecodeVersion] = &[
    BytecodeVersion::SOURCE_FREE_V2,
    BytecodeVersion::SOURCE_FREE,
];

impl DeviceDescriptor {
    /// A computer: Linux, macOS or Windows. It allows at most the engine's 64 MiB artifact and
    /// 1,024 call levels, has no maximum for the other limits, keeps the newest 1,000 events by
    /// default and admits at most 1,000,000, and has no application slot (specification 34
    /// HOST-02).
    #[must_use]
    pub fn hosted() -> Self {
        let unbounded = super::wide(usize::MAX);
        Self {
            bytecode_versions: SOURCE_FREE_VERSIONS,
            scheduling_profiles: &[SchedulingProfile::COOPERATIVE_RESOURCE_V1],
            numeric_contracts: &[NumericContract::V1],
            timing_evidence: TimingEvidence::UNQUALIFIED,
            capabilities: Capabilities::WALL_CLOCK,
            maxima: RequestedLimits {
                max_artifact_bytes: super::wide(BYTECODE_MAX_CONTAINER_BYTES),
                max_preparation_bytes: unbounded,
                max_preparation_work: unbounded,
                max_validation_scratch_bytes: unbounded,
                max_validation_work: unbounded,
                max_construction_values: unbounded,
                max_construction_bytes: unbounded,
                max_process_image_bytes: unbounded,
                max_work: unbounded,
                max_construction_work: unbounded,
                max_call_depth: super::wide(VM_MAX_CALL_DEPTH),
            },
            event_store_default: 1_000,
            event_store_max: 1_000_000,
            application_slot_bytes: None,
            program_memory_bytes: None,
            native: None,
        }
    }
}

/// Admit a prepared program by its memory figure (specification 34 MEM-12): after preparation and
/// before anything is constructed, `device` refuses a program whose figure, with the capture of the
/// retained values when the program keeps them (`retains`), is more than the memory the device has
/// for programs. A device without a maximum (a computer) admits every figure.
///
/// # Errors
///
/// `RuntimeError::DeploymentRefused` with [`AdmissionRefusal::Memory`] and both numbers.
pub fn admit_memory(
    figure: &crate::vm::MemoryFigure,
    retains: bool,
    device: &DeviceDescriptor,
) -> Result<(), RuntimeError> {
    let needed = super::wide(figure.total(retains));
    match device.program_memory_bytes {
        Some(available) if needed > available => {
            Err(RuntimeError::DeploymentRefused(AdmissionRefusal::Memory {
                needed,
                available,
            }))
        }
        _ => Ok(()),
    }
}

/// Admit `program` with its `record` on `device`, before any state exists. The checks run in
/// the order of specification 34 §5.1.1, and the first that fails refuses the deployment.
/// Nothing is constructed either way: prepare the program with the returned limits.
///
/// # Errors
///
/// `RuntimeError::DeploymentRefused` with the reason and its numbers, or
/// `RuntimeError::ImportUnsupported` for an import the device has no contract of that version
/// for.
pub fn admit(
    record: &DeploymentRecord,
    program: &[u8],
    device: &DeviceDescriptor,
) -> Result<PreparationLimits, RuntimeError> {
    let refuse = |reason| Err(RuntimeError::DeploymentRefused(reason));
    let application = &record.application;
    if usize::try_from(application.program_bytes).ok() != Some(program.len())
        || identity::sha256(program) != application.program_sha256
    {
        return refuse(AdmissionRefusal::ProgramMismatch);
    }
    let version = application.bytecode_version;
    if !device.bytecode_versions.contains(&version) {
        return refuse(AdmissionRefusal::BytecodeVersion {
            major: version.major,
            minor: version.minor,
        });
    }
    let needs = &record.contracts;
    if !device
        .scheduling_profiles
        .contains(&needs.scheduling_profile)
    {
        return refuse(AdmissionRefusal::SchedulingProfile(
            needs.scheduling_profile,
        ));
    }
    if !device.numeric_contracts.contains(&needs.numeric_contract) {
        return refuse(AdmissionRefusal::NumericContract(needs.numeric_contract));
    }
    if needs.timing_evidence > device.timing_evidence {
        return refuse(AdmissionRefusal::TimingEvidence {
            required: needs.timing_evidence,
            available: device.timing_evidence,
        });
    }
    let missing = needs.capabilities.missing_from(device.capabilities);
    if !missing.is_empty() {
        return refuse(AdmissionRefusal::Capability(missing));
    }
    for import in &record.imports {
        check_import(import)?;
    }
    let mut values = [0usize; LimitKind::ALL.len()];
    for ((limit, requested), value) in record.limits.values().into_iter().zip(&mut values) {
        // A request above this target's address range cannot be held, whatever its maximum.
        let maximum = device.maxima.get(limit).min(super::wide(usize::MAX));
        match usize::try_from(requested) {
            Ok(admitted) if requested <= maximum => *value = admitted,
            _ => {
                return refuse(AdmissionRefusal::Limit {
                    limit,
                    requested,
                    maximum,
                })
            }
        }
    }
    // Step 7 also bounds the record's section-7 event-store capacity: 0 keeps the device's
    // default, and a non-zero request may not be above the device's maximum (specification 34
    // §5.1.1, section 7).
    let requested_capacity = record.configuration.event_capacity;
    if requested_capacity != 0 && requested_capacity > device.event_store_max {
        return refuse(AdmissionRefusal::EventStore {
            requested: requested_capacity,
            maximum: device.event_store_max,
        });
    }
    if let Some(available) = device.application_slot_bytes {
        let needed = super::wide(program.len()).saturating_add(super::wide(record.encoded_len()));
        if needed > available {
            return refuse(AdmissionRefusal::ApplicationSlot { needed, available });
        }
    }
    let [max_artifact_bytes, max_preparation_bytes, max_preparation_work, max_scratch_bytes, max_validation_work, max_construction_values, max_construction_bytes, max_process_image_bytes, max_work, max_construction_work, max_call_depth] =
        values;
    Ok(PreparationLimits {
        max_artifact_bytes,
        max_preparation_bytes,
        max_preparation_work,
        validation: ValidationLimits {
            max_scratch_bytes,
            max_work: max_validation_work,
        },
        max_construction_values,
        max_construction_bytes,
        max_process_image_bytes,
        max_work,
        max_construction_work,
        max_call_depth,
    })
}

/// An import the device has a contract of the same version for (specification 34 §5.2.1).
fn check_import(import: &RequiredImport) -> Result<(), RuntimeError> {
    let contract = match import.kind {
        ImportKind::Function => contracts::function(&import.name).map(|found| found.contract),
        ImportKind::FunctionBlock => {
            contracts::function_block(&import.name).map(|found| found.contract)
        }
    };
    let reason = match contract {
        None => ImportRefusal::NoContract,
        Some(contract) if contract.version != import.contract_version => {
            ImportRefusal::ContractVersion {
                record: import.contract_version,
                runtime: contract.version,
            }
        }
        Some(_) => return Ok(()),
    };
    Err(RuntimeError::ImportUnsupported {
        import: import.name.clone(),
        reason,
    })
}
