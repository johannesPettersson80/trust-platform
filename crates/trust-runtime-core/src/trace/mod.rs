//! Cycle traces, the `.trusttrace` file (specification 48 RU-59 and Appendix C).
//!
//! A trace holds the program's state at its first cycle and, for each cycle, everything that came
//! into the program from outside and what it put out, so that a replay can run the cycles again
//! (RU-60). [`encode`](crate::trace::encode) writes a [`Trace`](crate::trace::Trace) as the file
//! and [`decode`](crate::trace::decode) reads one back, checking the header's and each section's
//! CRC-32. A cycle's record has a layout of its own ([`encode_cycle`](crate::trace::encode_cycle)),
//! which a recorder fills between the phases of a scan.

mod names;
mod read;
pub mod readings;
pub mod replay;
mod write;

#[cfg(test)]
mod tests;

use alloc::string::String;
use alloc::vec::Vec;

pub use names::{named_changes, named_cycle};
pub use read::{decode, Decoded};
pub use write::{
    change_len, cycle_record_len, encode, encode_change, encode_cycle, encode_cycle_parts,
    encode_records, front_sections, header, padded, section_entry, ChangeRef, CycleParts,
    FrontSections, StartParts,
};

/// The file's magic (Appendix C, C.1).
pub const MAGIC: [u8; 8] = *b"TRSTT001";
/// The format version this code writes and reads (Appendix C, C.1).
pub const FORMAT_VERSION: u16 = 1;
/// The bytes of the header (Appendix C, C.1).
pub const HEADER_BYTES: usize = 64;
/// The bytes of one entry of the section table (Appendix C, C.2).
pub const SECTION_ENTRY_BYTES: usize = 32;

/// The ids of a trace's sections (Appendix C, C.2).
pub mod section {
    /// The trace's identity.
    pub const META: u32 = 1;
    /// The string pool of `META` and `SYMS`.
    pub const STRS: u32 = 2;
    /// The located variables.
    pub const SYMS: u32 = 3;
    /// The start state.
    pub const STAT: u32 = 4;
    /// One cycle.
    pub const CYCL: u32 = 5;
    /// The recorded image's area table (the image's section 4 plus 100).
    pub const IMAGE_AREAS: u32 = 104;
    /// The recorded image's task table (6 plus 100).
    pub const IMAGE_TASKS: u32 = 106;
    /// The recorded image's I/O bindings (7 plus 100).
    pub const IMAGE_IOBIND: u32 = 107;
    /// The recorded image's symbol table (8 plus 100).
    pub const IMAGE_SYMBOLS: u32 = 108;
    /// The recorded image's type table (9 plus 100).
    pub const IMAGE_TYPES: u32 = 109;
    /// The recorded image's string pool (10 plus 100).
    pub const IMAGE_STRINGS: u32 = 110;
}

/// A cycle trace (specification 48 RU-59).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    /// Its identity.
    pub meta: Meta,
    /// The recorded image's located variables, in the order of its I/O binding table.
    pub located: Vec<Located>,
    /// The program's state at the start of the first cycle.
    pub start: StartState,
    /// The recorded image's tables that name its memory, byte for byte.
    pub image: ImageTables,
    /// The cycles, in order.
    pub cycles: Vec<Cycle>,
}

/// A trace's identity (Appendix C, C.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meta {
    /// The running image's SHA-256.
    pub image_sha256: [u8; 32],
    /// The image's sources' hash (specification 44 §5.2, offset 32).
    pub sources_hash: [u8; 32],
    /// The image's target id (specification 44 §5.2, offset 20).
    pub target: u32,
    /// Why the trace ended.
    pub end: TraceEnd,
    /// The resource's cycle interval, in nanoseconds.
    pub cycle_interval_ns: u64,
    /// The runtime's count of cycles at the trace's first.
    pub first_cycle: u64,
    /// The number of cycles the recording took. A damaged trace may hold fewer ([`Decoded`]).
    pub cycle_count: u64,
    /// The UTC time the recording began, in milliseconds (informational).
    pub started_utc_ms: i64,
    /// The runtime's version.
    pub runtime_version: String,
    /// The device's identity (`runtime_id`).
    pub runtime_id: String,
    /// The platform.
    pub platform: String,
    /// The resource's name.
    pub resource: String,
    /// The task whose runs the recording counted.
    pub task: Option<String>,
    /// Where the trace comes from.
    pub origin: TraceOrigin,
}

/// Why a trace ended (Appendix C, C.3, offset 68).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum TraceEnd {
    Complete = 1,
    Stopped = 2,
    ResourceStopped = 3,
    Restarted = 4,
    Faulted = 5,
    ProgramChanged = 6,
    Paused = 7,
    MemoryFull = 8,
}

impl TraceEnd {
    /// The end's name, as `trace.read` gives it (specification 11 §6.8.1).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Stopped => "stopped",
            Self::ResourceStopped => "resource_stopped",
            Self::Restarted => "restarted",
            Self::Faulted => "faulted",
            Self::ProgramChanged => "program_changed",
            Self::Paused => "paused",
            Self::MemoryFull => "memory_full",
        }
    }

    const fn from_code(code: u32) -> Option<Self> {
        Some(match code {
            1 => Self::Complete,
            2 => Self::Stopped,
            3 => Self::ResourceStopped,
            4 => Self::Restarted,
            5 => Self::Faulted,
            6 => Self::ProgramChanged,
            7 => Self::Paused,
            8 => Self::MemoryFull,
            _ => return None,
        })
    }
}

/// Where a trace comes from (Appendix C, C.3, offset 124).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceOrigin {
    /// A recording on demand (`trace.record`).
    OnDemand = 1,
    /// The flight recorder, at a fault.
    FlightRecorder = 2,
}

impl TraceOrigin {
    const fn from_code(code: u32) -> Option<Self> {
        match code {
            1 => Some(Self::OnDemand),
            2 => Some(Self::FlightRecorder),
            _ => None,
        }
    }
}

/// A process image's area (Appendix C, C.4 and C.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoArea {
    /// `%I`.
    Input = 1,
    /// `%Q`.
    Output = 2,
    /// `%M`.
    Memory = 3,
}

impl IoArea {
    const fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::Input),
            2 => Some(Self::Output),
            3 => Some(Self::Memory),
            _ => None,
        }
    }
}

/// A located variable of the recorded image (Appendix C, C.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    /// Its path, as the debugger names it.
    pub path: String,
    /// Its type's name.
    pub type_name: String,
    /// Its area.
    pub area: IoArea,
    /// Its bit, for a `BOOL` of one bit.
    pub bit: Option<u8>,
    /// Its byte offset in the area.
    pub offset: u32,
    /// Its size in bytes.
    pub size: u32,
}

/// The program's state at the start of a trace's first cycle (Appendix C, C.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartState {
    /// Each area of specification 44 §7.1 but the stack: its id and its bytes.
    pub areas: Vec<(u32, Vec<u8>)>,
    /// The forces then in effect, each a [`ChangeKind::VariableForce`] or
    /// [`ChangeKind::AddressForce`].
    pub forces: Vec<Change>,
}

/// The recorded image's sections that name its memory, byte for byte (Appendix C, C.2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImageTables {
    /// The area table (specification 44 §5.5).
    pub areas: Vec<u8>,
    /// The task table (§5.6).
    pub tasks: Vec<u8>,
    /// The I/O bindings (§5.6).
    pub iobind: Vec<u8>,
    /// The symbol table (§5.7).
    pub symbols: Vec<u8>,
    /// The type table (§5.8).
    pub types: Vec<u8>,
    /// The string pool.
    pub strings: Vec<u8>,
}

/// One cycle's record (Appendix C, C.6).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cycle {
    /// The cycle's logical time, in nanoseconds.
    pub time_ns: i64,
    /// Each task activation, in the order the tasks ran.
    pub activations: Vec<Activation>,
    /// Each reading of the wall clock (`CURRENT_DT`), in order: UTC milliseconds.
    pub wall_clock_ms: Vec<i64>,
    /// Every change from outside before the cycle, in the order applied.
    pub changes: Vec<Change>,
    /// The input image as the scan sampled it.
    pub inputs: Vec<u8>,
    /// The output image the cycle wrote.
    pub outputs: Vec<u8>,
    /// The fault that ended the cycle.
    pub fault: Option<CycleFault>,
}

/// A task activation of a cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Activation {
    /// The task's index in the recorded image's task table.
    pub task: u32,
    /// The task's logical time (`now_ns`).
    pub now_ns: i64,
}

/// The fault that ended a cycle: its fault record (specification 44 §8.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CycleFault {
    /// The fault's code.
    pub code: u32,
    /// The POU's id.
    pub pou: u32,
    /// The statement's id.
    pub statement: u32,
}

/// A change made to the program from outside (Appendix C, C.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// Where it came from.
    pub source: ChangeSource,
    /// What it changed.
    pub kind: ChangeKind,
}

/// Where a change came from (Appendix C, C.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeSource {
    /// The control link: the debugger, Live Values and `ctl` among its clients.
    ControlLink = 1,
    /// A screen.
    Screen = 2,
    /// Control Studio.
    ControlStudio = 3,
    /// A service (specification 48 RU-63).
    Service = 4,
    /// The mesh.
    Mesh = 5,
    /// A protocol server, such as OPC UA's.
    ProtocolServer = 6,
    /// A plant of the simulation.
    Plant = 7,
}

impl ChangeSource {
    const fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            1 => Self::ControlLink,
            2 => Self::Screen,
            3 => Self::ControlStudio,
            4 => Self::Service,
            5 => Self::Mesh,
            6 => Self::ProtocolServer,
            7 => Self::Plant,
            _ => return None,
        })
    }
}

/// What a change changed (Appendix C, C.6, kinds 1 to 7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
    /// A variable's write (kind 1): its value's bytes at the declared size.
    VariableWrite {
        /// The variable's path.
        path: String,
        /// Its type's name.
        type_name: String,
        /// The value's bytes.
        value: Vec<u8>,
    },
    /// A force set on a variable (kind 2).
    VariableForce {
        /// The variable's path.
        path: String,
        /// Its type's name.
        type_name: String,
        /// The forced value's bytes.
        value: Vec<u8>,
    },
    /// A force released on a variable (kind 3).
    VariableRelease {
        /// The variable's path.
        path: String,
    },
    /// An address's one-shot write (kind 4).
    AddressWrite {
        /// The area.
        area: IoArea,
        /// The bit, for one bit.
        bit: Option<u8>,
        /// The byte offset.
        offset: u32,
        /// The bytes written.
        bytes: Vec<u8>,
    },
    /// A force set on an address (kind 5).
    AddressForce {
        /// The area.
        area: IoArea,
        /// The bit, for one bit.
        bit: Option<u8>,
        /// The byte offset.
        offset: u32,
        /// The forced bytes.
        bytes: Vec<u8>,
    },
    /// A force released on an address (kind 6).
    AddressRelease {
        /// The area.
        area: IoArea,
        /// The bit, for one bit.
        bit: Option<u8>,
        /// The byte offset.
        offset: u32,
        /// The size in bytes.
        size: u32,
    },
    /// A tool's method call (kind 7).
    MethodCall {
        /// The instance's path.
        instance: String,
        /// The interface's qualified name.
        interface: String,
        /// The method's name.
        method: String,
        /// Each input's name and Structured Text literal.
        inputs: Vec<(String, String)>,
    },
}

/// Why a trace could not be read (Appendix C, C.2).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TraceError {
    /// The file is shorter than its header, its section table or a section says.
    #[error("the trace is cut short: {0}")]
    Truncated(&'static str),
    /// The file does not start with the magic `TRSTT001`.
    #[error("not a cycle trace: the file does not start with TRSTT001")]
    NotATrace,
    /// A format version this code does not read.
    #[error("the trace's format version {0} is not version 1, which this runtime reads")]
    UnsupportedVersion(u16),
    /// The header's checksum does not match the header and the section table.
    #[error("the trace's header or section table is damaged (its CRC-32 does not match)")]
    HeaderDamaged,
    /// A required section's checksum does not match its bytes.
    #[error("the trace's section {0} is damaged (its CRC-32 does not match)")]
    SectionDamaged(&'static str),
    /// A required section is missing or appears twice.
    #[error("the trace has {count} {name} sections, where it needs one")]
    SectionCount {
        /// The section's name.
        name: &'static str,
        /// How many the table lists.
        count: usize,
    },
    /// A section's content does not follow Appendix C.
    #[error("the trace's section {section} is malformed: {reason}")]
    Malformed {
        /// The section's name.
        section: &'static str,
        /// What is wrong.
        reason: &'static str,
    },
}
