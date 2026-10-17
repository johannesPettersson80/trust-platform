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
    /// The bytes of the slot that holds the program and its record, on a device that has one.
    pub application_slot_bytes: Option<u64>,
}

/// The STBC versions a source-free runtime prepares.
const SOURCE_FREE_VERSIONS: &[BytecodeVersion] = &[
    BytecodeVersion::SOURCE_FREE_V2,
    BytecodeVersion::SOURCE_FREE,
];

impl DeviceDescriptor {
    /// A computer: Linux, macOS or Windows. It allows at most the engine's 64 MiB artifact and
    /// 1,024 call levels, has no maximum for the other limits and no application slot
    /// (specification 34 HOST-02).
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
            application_slot_bytes: None,
        }
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
