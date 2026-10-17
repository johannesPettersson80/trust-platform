//! Fixed-size fault records for the core control path (specification 34 MEM-05, STATE-04).
//!
//! A [`FaultRecord`](crate::fault::FaultRecord) is a `Copy` value of at most 48 bytes on a
//! 32-bit target: the stable code, bounded [`FaultDetail`](crate::fault::FaultDetail) context,
//! the first failing instruction's [`FaultSite`](crate::fault::FaultSite), the task and the
//! logical time. Every scan-path fault is built from these values without allocating; rich
//! names and messages are rendered outside the core control path.

use core::fmt;

use crate::error::ExecutionLimitKind;
use crate::error::StableErrorCode;

/// The prepared table a bounded [`FaultDetail::TableIndex`] names, which selects its text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultTableIndex {
    /// A reference-table index that does not exist.
    Ref,
    /// A constant-pool index that does not exist.
    Const,
    /// A string-table index used where the table does not reach.
    String,
    /// A POU identity that is absent.
    MissingPou,
    /// A native call kind that is not supported.
    NativeCallKind,
    /// A native symbol descriptor index that is invalid.
    NativeSymbol,
}

/// The fixed wording a bounded [`FaultDetail::StaticName`] prefixes to its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultStaticName {
    /// A known instruction that has no execution implementation.
    UnsupportedOpcode,
    /// A reference location that the execution composition does not provide.
    UnsupportedRefLocation,
}

/// Bounded context of a fault, held without allocation (specification 34 MEM-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultDetail {
    /// No additional context.
    None,
    /// A fixed diagnostic text.
    Text(&'static str),
    /// A fixed diagnostic text followed by a static name.
    StaticName {
        /// The fixed wording that the name follows.
        kind: FaultStaticName,
        /// The static name that completes the text.
        name: &'static str,
    },
    /// A rejected opcode byte.
    Opcode(u8),
    /// A rejected jump target.
    JumpTarget(i64),
    /// A rejected index into a prepared table.
    TableIndex {
        /// The prepared table whose index was rejected.
        kind: FaultTableIndex,
        /// The rejected index.
        index: u32,
    },
    /// A reference outside the active frame's local range.
    LocalRef {
        /// Requested reference-table index.
        index: u32,
        /// First reference index owned by the frame.
        start: u32,
        /// Number of references owned by the frame.
        count: u32,
    },
    /// A value outside an inclusive `[lower, upper]` range.
    Index {
        /// Rejected value.
        index: i64,
        /// Inclusive lower bound.
        lower: i64,
        /// Inclusive upper bound.
        upper: i64,
    },
    /// A value outside a declared subrange, as 64-bit magnitudes with sign bits that cover
    /// the 65-bit signed domain.
    Subrange {
        /// Rejected value magnitude.
        value: u64,
        /// Inclusive lower bound magnitude.
        lower: u64,
        /// Inclusive upper bound magnitude.
        upper: u64,
        /// Sign bits, least significant first: value, lower, upper; set means negative.
        negative: u8,
    },
    /// A limit refusal: the exhausted allowance, accepted demand, requested demand and the
    /// admitted limit.
    Limit {
        /// The independently admitted allowance that refused the operation.
        kind: ExecutionLimitKind,
        /// Accepted demand before the rejected operation.
        used: u64,
        /// Complete demand of the rejected operation.
        requested: u64,
        /// Admitted maximum for this operation entry.
        limit: u64,
    },
    /// A prepared string-table index naming a POU, program or function block.
    PreparedName(u32),
}

impl FaultDetail {
    /// Write a signed 65-bit endpoint as a decimal magnitude.
    fn write_signed(
        formatter: &mut fmt::Formatter<'_>,
        negative: bool,
        magnitude: u64,
    ) -> fmt::Result {
        if negative {
            formatter.write_str("-")?;
        }
        write!(formatter, "{magnitude}")
    }
}

impl fmt::Display for FaultDetail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => formatter.write_str(""),
            Self::Text(text) => formatter.write_str(text),
            Self::StaticName { kind, name } => match kind {
                FaultStaticName::UnsupportedOpcode => {
                    write!(formatter, "vm unsupported opcode {name}")
                }
                FaultStaticName::UnsupportedRefLocation => {
                    write!(formatter, "vm unsupported ref location {name}")
                }
            },
            Self::Opcode(opcode) => write!(formatter, "vm invalid opcode 0x{opcode:02X}"),
            Self::JumpTarget(target) => write!(formatter, "vm invalid jump target {target}"),
            Self::TableIndex { kind, index } => match kind {
                FaultTableIndex::Ref => write!(formatter, "vm invalid ref index {index}"),
                FaultTableIndex::Const => write!(formatter, "vm invalid const index {index}"),
                FaultTableIndex::String => write!(formatter, "invalid index {index} for string"),
                FaultTableIndex::MissingPou => write!(formatter, "vm missing pou id {index}"),
                FaultTableIndex::NativeCallKind => {
                    write!(formatter, "vm invalid CALL_NATIVE kind {index}")
                }
                FaultTableIndex::NativeSymbol => {
                    write!(formatter, "vm invalid index {index} for native symbol")
                }
            },
            Self::LocalRef {
                index,
                start,
                count,
            } => write!(
                formatter,
                "vm invalid local ref {index} (frame local range {start}..{})",
                start.saturating_add(*count)
            ),
            Self::Index {
                index,
                lower,
                upper,
            } => write!(
                formatter,
                "array index {index} out of bounds [{lower}..{upper}]"
            ),
            Self::Subrange {
                value,
                lower,
                upper,
                negative,
            } => {
                formatter.write_str("value ")?;
                Self::write_signed(formatter, negative & 1 != 0, *value)?;
                formatter.write_str(" outside declared subrange ")?;
                Self::write_signed(formatter, negative & 2 != 0, *lower)?;
                formatter.write_str("..")?;
                Self::write_signed(formatter, negative & 4 != 0, *upper)
            }
            Self::Limit {
                kind,
                used,
                requested,
                limit,
            } => write!(
                formatter,
                "execution timed out: {kind} limit {limit}, used {used}, requested {requested}"
            ),
            Self::PreparedName(index) => write!(formatter, "prepared name #{index}"),
        }
    }
}

/// The first failing instruction of a fault, recorded while unwinding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaultSite {
    /// POU identity of the failing instruction.
    pub pou: u32,
    /// Instruction offset inside the POU body.
    pub pc: u32,
}

/// A fixed-size fault record (specification 34 MEM-05, STATE-04).
///
/// It is `Copy` and at most 48 bytes on a 32-bit target, so an engine latches it without
/// cloning and without allocating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaultRecord {
    /// Stable machine-readable fault identity.
    pub code: StableErrorCode,
    /// Bounded fault context.
    pub detail: FaultDetail,
    /// The first failing instruction.
    pub site: FaultSite,
    /// Task that was running when the fault was raised.
    pub task: u16,
    /// Logical scan time in milliseconds when the fault was raised.
    pub logical_time: u32,
}

impl FaultRecord {
    /// Build the record that holds a VM fault with its site, task and logical time.
    #[must_use]
    pub const fn from_vm_fault(
        fault: crate::error::VmFaultDetail,
        site: FaultSite,
        task: u16,
        logical_time: u32,
    ) -> Self {
        Self {
            code: fault.code,
            detail: fault.detail,
            site,
            task,
            logical_time,
        }
    }
}

// The bound is a product contract (specification 34 MEM-05): the record stays fixed-size on
// every target, hosted and MCU.
const _: () = assert!(core::mem::size_of::<FaultRecord>() <= 48);
