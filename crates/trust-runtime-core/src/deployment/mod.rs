//! The record of a deployment and its admission (specification 34 §5.1.1, specification 44 §10.1).
//!
//! A deployment record describes one program image: its identities, the contracts and
//! capabilities it needs, the process image it requests, its tasks and the configuration a device
//! without an operating system runs from. It travels as the versioned binary sidecar
//! `deployment-record.bin` next to `program.tpi`. Before any state exists, a runtime admits the
//! record against its device descriptor. The same code runs on every target, with and without
//! `std`.

mod admission;
mod codec;
mod configuration;
mod identity;
mod refusal;

pub use admission::{admit, DeviceDescriptor, NativeTarget, HOSTED_MAX_STATEMENTS};
pub use configuration::{
    Configuration, ControlMode, GpioInput, GpioOutput, HangAction, IoDriver, LogLevel,
    SafeStateEntry, TaskOverride,
};
pub use identity::{configuration_identity, sha256, sha256_parts, Sha256Stream};
pub use refusal::{AdmissionRefusal, DeploymentRecordError, LimitKind};

use crate::retain::checkpoint::CheckpointIdentity;
use alloc::vec::Vec;
use core::fmt;
use smol_str::SmolStr;

/// The record version this runtime writes, and the only version it reads.
pub const RECORD_VERSION: u16 = 1;
/// The largest record a runtime reads or writes, in bytes.
pub const RECORD_MAX_BYTES: usize = 65_536;

/// A deployment record (specification 34 §5.1.1).
#[derive(Debug, Clone, PartialEq)]
pub struct DeploymentRecord {
    /// Section 1: the program and the configuration it was built with.
    pub application: ApplicationIdentity,
    /// Section 2: the contracts and capabilities the program needs.
    pub contracts: Contracts,
    /// Section 3: the preparation limits the program requests.
    pub limits: RequestedLimits,
    /// Section 4: the program's process image.
    pub process_image: ProcessImageSummary,
    /// Section 5: the program's tasks, in declaration order.
    pub tasks: Vec<TaskSummary>,
    /// Section 6: the built-in imports the program calls, each once, ordered by kind and then
    /// by name.
    pub imports: Vec<RequiredImport>,
    /// Section 7: the settings a device without an operating system needs from `runtime.toml`
    /// and `io.toml`.
    pub configuration: Configuration,
}

impl DeploymentRecord {
    /// The identities a checkpoint of this program carries (specification 34 §10.1): the first
    /// 16 bytes of the program's SHA-256 and of its retain-schema identity.
    #[must_use]
    pub fn checkpoint_identity(&self) -> CheckpointIdentity {
        let mut identity = CheckpointIdentity::default();
        identity
            .application
            .copy_from_slice(&self.application.program_sha256[..16]);
        identity
            .schema
            .copy_from_slice(&self.application.retain_schema_sha256[..16]);
        identity
    }
}

/// The version of the program format a record names (specification 34 §5.1.1, section 1): 44.1
/// for a compiled program image, the only program a runtime runs (specification 44 §10.1). The
/// versions below 44 named the STBC programs of the bytecode engine, which a runtime refuses with
/// "rebuild the project" (specification 44 §15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramVersion {
    /// Major version.
    pub major: u16,
    /// Minor version.
    pub minor: u16,
}

impl ProgramVersion {
    /// The version pair `major.minor`.
    #[must_use]
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// A compiled program image `TRSTI001` (specification 44 §10.1).
    pub const NATIVE_IMAGE: Self = Self::new(44, 1);
}

/// The program and the configuration it was built with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplicationIdentity {
    /// SHA-256 of the exact bytes of `program.tpi`.
    pub program_sha256: [u8; 32],
    /// The length of `program.tpi` in bytes.
    pub program_bytes: u32,
    /// The version of the program's format.
    pub program_version: ProgramVersion,
    /// The program's format flags; 0 for an image.
    pub program_flags: u32,
    /// SHA-256 over the project's `runtime.toml` and `io.toml` (`configuration_identity`).
    pub configuration_sha256: [u8; 32],
    /// SHA-256 over the retained declarations and the types (`retain_schema_identity`).
    pub retain_schema_sha256: [u8; 32],
}

/// The contracts and capabilities a program needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Contracts {
    /// The scheduling/I/O profile (specification 34 §4.4).
    pub scheduling_profile: SchedulingProfile,
    /// The numeric contract (specification 34 §12.1).
    pub numeric_contract: NumericContract,
    /// The timing evidence the program requires (specification 34 §4.4, LOAD-08).
    pub timing_evidence: TimingEvidence,
    /// The platform capabilities the program needs.
    pub capabilities: Capabilities,
}

/// A scheduling/I/O profile (specification 34 §4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SchedulingProfile(pub u16);

impl SchedulingProfile {
    /// `cooperative-resource-v1`: the nonpreemptive single-owner resource scan.
    pub const COOPERATIVE_RESOURCE_V1: Self = Self(1);
}

impl fmt::Display for SchedulingProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::COOPERATIVE_RESOURCE_V1 => formatter.write_str("cooperative-resource-v1"),
            Self(other) => write!(formatter, "{other}"),
        }
    }
}

/// A numeric contract (specification 34 §12.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NumericContract(pub u16);

impl NumericContract {
    /// Version 1 of the numeric contract of specification 34 §12.1.
    pub const V1: Self = Self(1);
}

impl fmt::Display for NumericContract {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// A timing-evidence class (specification 34 §4.4); a larger value is a stronger claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimingEvidence(pub u8);

impl TimingEvidence {
    /// Functional or bring-up execution only, with no deadline claim.
    pub const UNQUALIFIED: Self = Self(0);
    /// Specified workloads met their deadlines under a recorded envelope.
    pub const MEASURED_QUALIFIED: Self = Self(1);
    /// A documented scheduling argument with justified bounds.
    pub const ANALYTICALLY_SUPPORTED: Self = Self(2);
}

impl fmt::Display for TimingEvidence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::UNQUALIFIED => formatter.write_str("unqualified"),
            Self::MEASURED_QUALIFIED => formatter.write_str("measured-qualified"),
            Self::ANALYTICALLY_SUPPORTED => formatter.write_str("analytically supported"),
            Self(other) => write!(formatter, "class {other}"),
        }
    }
}

/// Platform capabilities, as a set of bits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Capabilities(pub u16);

impl Capabilities {
    /// No capability.
    pub const NONE: Self = Self(0);
    /// The platform wall clock that `CURRENT_DT` reads (specification 34 §5.2).
    pub const WALL_CLOCK: Self = Self(1);

    /// Whether the set holds no capability.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The capabilities of `self` that `supplied` lacks.
    #[must_use]
    pub const fn missing_from(self, supplied: Self) -> Self {
        Self(self.0 & !supplied.0)
    }

    /// The union of both sets.
    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl fmt::Display for Capabilities {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut separator = "";
        for bit in 0..16 {
            if self.0 & (1 << bit) == 0 {
                continue;
            }
            formatter.write_str(separator)?;
            if bit == 0 {
                formatter.write_str("the platform wall clock")?;
            } else {
                write!(formatter, "capability bit {bit}")?;
            }
            separator = ", ";
        }
        Ok(())
    }
}

/// The limits a program requests, one 64-bit value each, so that a record means the same on 32- and
/// 64-bit targets. An image requests its process image's bytes and 0 for every other limit, which
/// named a limit of the bytecode engine's preparation (specification 44 §10.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestedLimits {
    /// `max_artifact_bytes`.
    pub max_artifact_bytes: u64,
    /// `max_preparation_bytes`.
    pub max_preparation_bytes: u64,
    /// `max_preparation_work`.
    pub max_preparation_work: u64,
    /// `validation.max_scratch_bytes`.
    pub max_validation_scratch_bytes: u64,
    /// `validation.max_work`.
    pub max_validation_work: u64,
    /// `max_construction_values`.
    pub max_construction_values: u64,
    /// `max_construction_bytes`.
    pub max_construction_bytes: u64,
    /// `max_process_image_bytes`.
    pub max_process_image_bytes: u64,
    /// `max_work`.
    pub max_work: u64,
    /// `max_construction_work`.
    pub max_construction_work: u64,
    /// `max_call_depth`.
    pub max_call_depth: u64,
}

impl RequestedLimits {
    /// Every limit with its value, in the order of the record's limits section.
    #[must_use]
    pub fn values(&self) -> [(LimitKind, u64); LimitKind::ALL.len()] {
        LimitKind::ALL.map(|kind| (kind, self.get(kind)))
    }

    /// The value of one limit.
    #[must_use]
    pub fn get(&self, kind: LimitKind) -> u64 {
        match kind {
            LimitKind::ArtifactBytes => self.max_artifact_bytes,
            LimitKind::PreparationBytes => self.max_preparation_bytes,
            LimitKind::PreparationWork => self.max_preparation_work,
            LimitKind::ValidationScratchBytes => self.max_validation_scratch_bytes,
            LimitKind::ValidationWork => self.max_validation_work,
            LimitKind::ConstructionValues => self.max_construction_values,
            LimitKind::ConstructionBytes => self.max_construction_bytes,
            LimitKind::ProcessImageBytes => self.max_process_image_bytes,
            LimitKind::Work => self.max_work,
            LimitKind::ConstructionWork => self.max_construction_work,
            LimitKind::CallDepth => self.max_call_depth,
        }
    }

    /// The request whose values are `values`, in the order of [`RequestedLimits::values`].
    pub(crate) fn from_values(values: [u64; LimitKind::ALL.len()]) -> Self {
        let [max_artifact_bytes, max_preparation_bytes, max_preparation_work, max_validation_scratch_bytes, max_validation_work, max_construction_values, max_construction_bytes, max_process_image_bytes, max_work, max_construction_work, max_call_depth] =
            values;
        Self {
            max_artifact_bytes,
            max_preparation_bytes,
            max_preparation_work,
            max_validation_scratch_bytes,
            max_validation_work,
            max_construction_values,
            max_construction_bytes,
            max_process_image_bytes,
            max_work,
            max_construction_work,
            max_call_depth,
        }
    }
}

/// The program's process image (specification 34 §5.1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProcessImageSummary {
    /// The `%I` image size in bytes.
    pub input_bytes: u32,
    /// The `%Q` image size in bytes.
    pub output_bytes: u32,
    /// The `%M` image size in bytes.
    pub marker_bytes: u32,
    /// The number of I/O bindings.
    pub bindings: u32,
}

/// One configured task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskSummary {
    /// The task interval in nanoseconds.
    pub interval_nanos: i64,
    /// The task priority; smaller values run first.
    pub priority: u32,
    /// Whether an event (`SINGLE`) starts the task.
    pub event: bool,
}

/// The kind of a built-in import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ImportKind {
    /// A built-in function (specification 34 §5.2.1).
    Function = 1,
    /// A standard function block.
    FunctionBlock = 2,
}

/// A built-in import the program calls, with the version of its contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiredImport {
    /// Function or function block.
    pub kind: ImportKind,
    /// The canonical uppercase name.
    pub name: SmolStr,
    /// The contract version the program was built against (specification 34 §5.2.1).
    pub contract_version: u16,
}

/// A target-sized value as the record's 64-bit field. No supported target has a wider `usize`.
fn wide(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}
