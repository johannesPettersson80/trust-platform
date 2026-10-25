//! Why a deployment record is unreadable or a deployment is refused (specification 34 §5.1.1).

use super::{Capabilities, NumericContract, SchedulingProfile, TimingEvidence, RECORD_MAX_BYTES};
use core::fmt;

/// Why a byte sequence is not a readable deployment record. The stable code is
/// `runtime_deployment_record_invalid`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentRecordError {
    /// The bytes end before a header, a section or a declared length.
    Truncated,
    /// The record does not start with the magic `TRDR`.
    Magic,
    /// The record has a version this runtime does not read.
    Version(u16),
    /// The record's length field differs from the length of its bytes.
    Length,
    /// The record is longer than `RECORD_MAX_BYTES`.
    TooLarge,
    /// The CRC-32 does not match the record's bytes.
    Checksum,
    /// The record version defines no section with this identifier.
    UnknownSection(u16),
    /// A section is repeated or out of ascending order.
    SectionOrder(u16),
    /// A section the record version requires is missing.
    MissingSection(u16),
    /// A section's payload is longer or shorter than its content.
    SectionLength(u16),
    /// A reserved field is not 0.
    Reserved,
    /// A field holds a value outside its range.
    Field(&'static str),
    /// The device's memory cannot hold the decoded record.
    Memory,
}

impl fmt::Display for DeploymentRecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => formatter.write_str("the record ends before its declared lengths"),
            Self::Magic => formatter.write_str("the record does not start with TRDR"),
            Self::Version(version) => write!(
                formatter,
                "record version {version} is unknown; this runtime reads versions 1 and {}",
                super::RECORD_VERSION
            ),
            Self::Length => formatter.write_str("the record length differs from its bytes"),
            Self::TooLarge => write!(
                formatter,
                "the record is longer than {RECORD_MAX_BYTES} bytes"
            ),
            Self::Checksum => formatter.write_str("the CRC-32 does not match"),
            Self::UnknownSection(id) => write!(
                formatter,
                "section {id} is not defined in this record version"
            ),
            Self::SectionOrder(id) => {
                write!(formatter, "section {id} is repeated or out of order")
            }
            Self::MissingSection(id) => write!(formatter, "section {id} is missing"),
            Self::SectionLength(id) => write!(
                formatter,
                "section {id} has a payload of another length than its content"
            ),
            Self::Reserved => formatter.write_str("a reserved field is not 0"),
            Self::Field(field) => write!(formatter, "{field} is outside its range"),
            Self::Memory => formatter.write_str("the device's memory cannot hold the record"),
        }
    }
}

impl core::error::Error for DeploymentRecordError {}

/// One limit of a deployment record, named as the bytecode engine's preparation named it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LimitKind {
    /// `max_artifact_bytes`.
    ArtifactBytes,
    /// `max_preparation_bytes`.
    PreparationBytes,
    /// `max_preparation_work`.
    PreparationWork,
    /// `validation.max_scratch_bytes`.
    ValidationScratchBytes,
    /// `validation.max_work`.
    ValidationWork,
    /// `max_construction_values`.
    ConstructionValues,
    /// `max_construction_bytes`.
    ConstructionBytes,
    /// `max_process_image_bytes`.
    ProcessImageBytes,
    /// `max_work`.
    Work,
    /// `max_construction_work`.
    ConstructionWork,
    /// `max_call_depth`.
    CallDepth,
}

impl LimitKind {
    /// Every limit, in the order of the record's limits section.
    pub const ALL: [Self; 11] = [
        Self::ArtifactBytes,
        Self::PreparationBytes,
        Self::PreparationWork,
        Self::ValidationScratchBytes,
        Self::ValidationWork,
        Self::ConstructionValues,
        Self::ConstructionBytes,
        Self::ProcessImageBytes,
        Self::Work,
        Self::ConstructionWork,
        Self::CallDepth,
    ];

    /// The limit's name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ArtifactBytes => "max_artifact_bytes",
            Self::PreparationBytes => "max_preparation_bytes",
            Self::PreparationWork => "max_preparation_work",
            Self::ValidationScratchBytes => "validation.max_scratch_bytes",
            Self::ValidationWork => "validation.max_work",
            Self::ConstructionValues => "max_construction_values",
            Self::ConstructionBytes => "max_construction_bytes",
            Self::ProcessImageBytes => "max_process_image_bytes",
            Self::Work => "max_work",
            Self::ConstructionWork => "max_construction_work",
            Self::CallDepth => "max_call_depth",
        }
    }
}

impl fmt::Display for LimitKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

/// Why a device refuses a deployment before constructing any state (specification 34 §5.1.1,
/// DEPLOY-04). The stable code is `runtime_deployment_refused`; an import refusal is
/// `RuntimeError::ImportUnsupported` instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionRefusal {
    /// The supplied program's length or SHA-256 differs from the record's.
    ProgramMismatch,
    /// The device does not run the version of the program's format: an STBC program of the
    /// bytecode engine, or an image on a device that runs none.
    ProgramVersion {
        /// Major version.
        major: u16,
        /// Minor version.
        minor: u16,
    },
    /// The device does not run the scheduling/I/O profile.
    SchedulingProfile(SchedulingProfile),
    /// The device does not implement the numeric contract.
    NumericContract(NumericContract),
    /// The record requires stronger timing evidence than the device has (LOAD-08).
    TimingEvidence {
        /// The class the record requires.
        required: TimingEvidence,
        /// The device's class.
        available: TimingEvidence,
    },
    /// The device lacks these required capabilities.
    Capability(Capabilities),
    /// The record requests more than the device's maximum for a limit.
    Limit {
        /// The limit.
        limit: LimitKind,
        /// The record's request.
        requested: u64,
        /// The device's maximum.
        maximum: u64,
    },
    /// The record's section-7 event-store capacity is above the device's maximum.
    EventStore {
        /// The events the record requests.
        requested: u32,
        /// The events the device's store keeps at most.
        maximum: u32,
    },
    /// The program and its record do not fit in the device's application slot.
    ApplicationSlot {
        /// The bytes of the program and its record.
        needed: u64,
        /// The bytes of the slot.
        available: u64,
    },
    /// The program's memory figure is more than the memory the device has for programs
    /// (specification 34 MEM-12).
    Memory {
        /// The bytes of the program's memory figure.
        needed: u64,
        /// The bytes of memory the device has for programs.
        available: u64,
    },
}

impl fmt::Display for AdmissionRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProgramMismatch => formatter.write_str("the record describes another program"),
            Self::ProgramVersion { major, minor }
                if super::ProgramVersion::new(*major, *minor)
                    == super::ProgramVersion::NATIVE_IMAGE =>
            {
                formatter.write_str("the device does not run compiled program images")
            }
            Self::ProgramVersion { major, minor } => write!(
                formatter,
                "the program is STBC {major}.{minor}, which this runtime does not run: rebuild the \
                 project"
            ),
            Self::SchedulingProfile(profile) => write!(
                formatter,
                "the device does not run the scheduling profile {profile}"
            ),
            Self::NumericContract(contract) => write!(
                formatter,
                "the device does not implement numeric contract {contract}"
            ),
            Self::TimingEvidence {
                required,
                available,
            } => write!(
                formatter,
                "the record requires {required} timing evidence, the device has {available}"
            ),
            Self::Capability(missing) => write!(formatter, "the device lacks {missing}"),
            Self::Limit {
                limit,
                requested,
                maximum,
            } => write!(
                formatter,
                "the record requests {limit} {requested}, the device allows at most {maximum}"
            ),
            Self::EventStore { requested, maximum } => write!(
                formatter,
                "the record requests an event store of {requested} events, the device keeps at \
                 most {maximum}"
            ),
            Self::ApplicationSlot { needed, available } => write!(
                formatter,
                "the program and its record need {needed} bytes of the application slot, \
                 the device has {available}"
            ),
            Self::Memory { needed, available } => write!(
                formatter,
                "the program needs {needed} bytes of memory, the device has {available} for \
                 programs"
            ),
        }
    }
}

impl core::error::Error for AdmissionRefusal {}
