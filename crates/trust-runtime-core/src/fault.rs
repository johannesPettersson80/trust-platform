//! Fixed-size fault records for the core control path (specification 34 MEM-05, STATE-04).
//!
//! A [`FaultRecord`](crate::fault::FaultRecord) is a `Copy` value of at most 56 bytes on a
//! 32-bit target: the stable code, bounded [`FaultDetail`](crate::fault::FaultDetail) context,
//! the first failing instruction's [`FaultSite`](crate::fault::FaultSite), the task and the
//! logical time. Every scan-path fault is built from these values without allocating; rich
//! names and messages are rendered outside the core control path.
//!
//! A fault whose reply text needs a hosted name or message keeps it as a
//! [`FaultMessage`](crate::fault::FaultMessage), shared from the error without copying, and
//! [`FaultRecord::render`](crate::fault::FaultRecord::render) builds today's text when a reply
//! asks for it.
//!
//! The record's logical time is the scan's whole seconds beside the millisecond within the
//! second, the millisecond precision the host's fault log already reports
//! ([`FaultRecord::latch`](crate::fault::FaultRecord::latch)).

use alloc::format;
use alloc::string::{String, ToString};
use core::fmt;

use smol_str::SmolStr;

use crate::error::ExecutionLimitKind;
use crate::error::RuntimeError;
use crate::error::StableErrorCode;
use crate::error::VmFaultDetail;
use crate::value::Duration;

/// Milliseconds in one whole second of the record's logical time.
const MILLIS_PER_SECOND: i64 = 1_000;

/// Bounded context of a fault, held without allocation (specification 34 MEM-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultDetail {
    /// No additional context.
    None,
    /// A fixed diagnostic text.
    Text(&'static str),
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
        }
    }
}

/// The first failing instruction of a fault, recorded while unwinding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaultSite {
    /// POU identity of the failing instruction.
    pub pou: u32,
    /// Offset of the failing instruction in the prepared code section, the offset the
    /// disassembler prints. The offset inside the POU body is this value less the POU's
    /// `code_offset`.
    pub pc: u32,
}

impl FaultSite {
    /// The site of a fault that no dispatched instruction raised, such as an adapter,
    /// deadline, retain or output failure (specification 34 MEM-05). No instruction sits at
    /// this offset: a code section that reached it would carry `u32::MAX` bytes of
    /// instructions.
    pub const NONE: Self = Self {
        pou: u32::MAX,
        pc: u32::MAX,
    };
}

/// A fixed-size fault record (specification 34 MEM-05, STATE-04).
///
/// It is `Copy` and at most 56 bytes on a 32-bit target, so an engine latches it without
/// cloning and without allocating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaultRecord {
    /// Stable machine-readable fault identity.
    pub code: StableErrorCode,
    /// Bounded fault context.
    pub detail: FaultDetail,
    /// The first failing instruction.
    pub site: FaultSite,
    /// Task that was running when the fault was raised, [`NO_TASK`] when none was.
    pub task: u16,
    /// Whole seconds of the scan's logical time when the fault was raised, saturating at
    /// `u32::MAX` (about 136 years of uptime).
    pub logical_seconds: u32,
    /// Milliseconds within that second, 0 through 999.
    pub logical_millis: u16,
}

/// The task a record names when no admitted task was running: a background program, work
/// outside a task, or a task index beyond the field (specification 34 MEM-05).
pub const NO_TASK: u16 = u16::MAX;

impl FaultRecord {
    /// Latch a failed scan: the error's stable code and bounded detail, the site of the
    /// failing instruction, the task that ran and the scan's logical time (specification 34
    /// MEM-05).
    ///
    /// The error is read by reference and never cloned, so latching a fault allocates nothing.
    /// `now` is the scan's logical time: its whole seconds and the millisecond within the
    /// second.
    #[must_use]
    pub fn latch(error: &RuntimeError, site: FaultSite, task: u16, now: Duration) -> Self {
        let (code, detail) = Self::code_and_detail(error);
        // A logical time never runs backwards within an installation, so a negative value
        // would be a composition defect rather than a time to publish.
        let millis = now.as_millis().max(0);
        let logical_seconds = u32::try_from(millis / MILLIS_PER_SECOND).unwrap_or(u32::MAX);
        let logical_millis =
            u16::try_from(millis.rem_euclid(MILLIS_PER_SECOND)).unwrap_or(u16::MAX);
        Self {
            code,
            detail,
            site,
            task,
            logical_seconds,
            logical_millis,
        }
    }

    /// The text `faults`, `events.tail` and the host's fault log report for this record and the
    /// message the error carried (specification 34 MEM-05).
    ///
    /// Without a message the record's own text is today's text. With one, the message selects
    /// the wording and supplies the name or text the fixed record does not hold, so the reply is
    /// byte for byte what trust-runtime answers today. The text is built here, at reply time; a
    /// fault is captured without it.
    #[must_use]
    pub fn render(&self, message: Option<&FaultMessage>) -> String {
        match message {
            None => self.to_string(),
            Some(FaultMessage::SimulationFault(text)) => format!("simulation fault '{text}'"),
            Some(FaultMessage::ResourcePanic(text)) => format!("resource panic '{text}'"),
            Some(FaultMessage::IoDriver(text)) => format!("i/o driver error '{text}'"),
            Some(FaultMessage::IoTransport(text)) => format!("i/o transport error '{text}'"),
            Some(FaultMessage::IoAddress(text)) => format!("i/o address error '{text}'"),
            Some(FaultMessage::IoFreshness(text)) => format!("i/o freshness error '{text}'"),
            Some(FaultMessage::RetainStore(text)) => format!("retain store error '{text}'"),
            Some(FaultMessage::RetainCorruption(text)) => format!("retain corruption '{text}'"),
            Some(FaultMessage::RetainMigration(text)) => {
                format!("retain migration error '{text}'")
            }
            Some(FaultMessage::InvalidConfig(text)) => format!("invalid config '{text}'"),
            Some(FaultMessage::ReadOnlyAlias(name)) => {
                format!("VAR_ACCESS '{name}' is read-only")
            }
            // The retained-value range is the record's bounded subrange detail, whose text is
            // exactly the range today's error prints between the retained name and the quote.
            Some(FaultMessage::RetainSubrange(name)) => format!(
                "retain migration error 'invalid retained value for '{name}': {}'",
                self.detail
            ),
        }
    }

    /// The stable code and bounded detail that render exactly as today's error does
    /// (specification 34 MEM-05).
    fn code_and_detail(error: &RuntimeError) -> (StableErrorCode, FaultDetail) {
        match error {
            RuntimeError::VmFault(fault) => (fault.code, fault.detail),
            RuntimeError::IndexOutOfBounds {
                index,
                lower,
                upper,
            } => (
                StableErrorCode::RuntimeIndexOutOfBounds,
                FaultDetail::Index {
                    index: *index,
                    lower: *lower,
                    upper: *upper,
                },
            ),
            RuntimeError::SubrangeViolation {
                value,
                lower,
                upper,
            }
            | RuntimeError::RetainSubrangeViolation {
                value,
                lower,
                upper,
                ..
            } => (
                StableErrorCode::RuntimeSubrangeViolation,
                subrange_detail(*value, *lower, *upper),
            ),
            RuntimeError::ExecutionLimit {
                kind,
                used,
                requested,
                limit,
            } => (
                StableErrorCode::RuntimeExecutionTimeout,
                FaultDetail::Limit {
                    kind: *kind,
                    used: u64::try_from(*used).unwrap_or(u64::MAX),
                    requested: u64::try_from(*requested).unwrap_or(u64::MAX),
                    limit: u64::try_from(*limit).unwrap_or(u64::MAX),
                },
            ),
            other => (other.stable_code(), FaultDetail::None),
        }
    }
}

/// The hosted name or message a fault error carried, kept beside the fixed record so `faults`
/// and `events.tail` render today's text (specification 34 MEM-05).
///
/// It is built from the error by reference: every payload is the `SmolStr` the error already
/// holds, shared by an inline copy or a reference count, so capturing a fault formats no string.
/// The record's [`FaultRecord::render`](crate::fault::FaultRecord::render) turns it into today's
/// wording when a reply asks for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaultMessage {
    /// `simulation fault '{0}'`.
    SimulationFault(SmolStr),
    /// `resource panic '{0}'`.
    ResourcePanic(SmolStr),
    /// `i/o driver error '{0}'`.
    IoDriver(SmolStr),
    /// `i/o transport error '{0}'`.
    IoTransport(SmolStr),
    /// `i/o address error '{0}'`.
    IoAddress(SmolStr),
    /// `i/o freshness error '{0}'`.
    IoFreshness(SmolStr),
    /// `retain store error '{0}'`.
    RetainStore(SmolStr),
    /// `retain corruption '{0}'`.
    RetainCorruption(SmolStr),
    /// `retain migration error '{0}'`.
    RetainMigration(SmolStr),
    /// `invalid config '{0}'`.
    InvalidConfig(SmolStr),
    /// `VAR_ACCESS '{0}' is read-only`.
    ReadOnlyAlias(SmolStr),
    /// The retained variable of `retain migration error 'invalid retained value for '{0}': …'`;
    /// the record's bounded subrange detail supplies the range.
    RetainSubrange(SmolStr),
}

impl FaultMessage {
    /// The message `error` carries, sharing its payload without copying, or `None` for a fault
    /// whose text the fixed record renders alone (specification 34 MEM-05).
    #[must_use]
    pub fn from_error(error: &RuntimeError) -> Option<Self> {
        match error {
            RuntimeError::SimulationFault(text) => Some(Self::SimulationFault(text.clone())),
            RuntimeError::ResourcePanic(text) => Some(Self::ResourcePanic(text.clone())),
            RuntimeError::IoDriver(text) => Some(Self::IoDriver(text.clone())),
            RuntimeError::IoTransport(text) => Some(Self::IoTransport(text.clone())),
            RuntimeError::IoAddress(text) => Some(Self::IoAddress(text.clone())),
            RuntimeError::IoFreshness(text) => Some(Self::IoFreshness(text.clone())),
            RuntimeError::RetainStore(text) => Some(Self::RetainStore(text.clone())),
            RuntimeError::RetainCorruption(text) => Some(Self::RetainCorruption(text.clone())),
            RuntimeError::RetainMigration(text) => Some(Self::RetainMigration(text.clone())),
            RuntimeError::InvalidConfig(text) => Some(Self::InvalidConfig(text.clone())),
            RuntimeError::ReadOnlyAlias(name) => Some(Self::ReadOnlyAlias(name.clone())),
            RuntimeError::RetainSubrangeViolation { name, .. } => {
                Some(Self::RetainSubrange(name.clone()))
            }
            _ => None,
        }
    }
}

impl fmt::Display for FaultRecord {
    /// The text the fault log reports for this record (specification 34 MEM-05).
    ///
    /// It is today's `RuntimeError` text for every fault the record holds: a bounded detail
    /// renders its parameterised form and a parameterless fault its fixed text. A fault whose
    /// text the record does not hold, because the error carried a name or message it never
    /// stored, renders its stable code: rich names and messages are rendered outside the core
    /// control path (MEM-05).
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.detail {
            FaultDetail::None => formatter.write_str(fixed_message(self.code)),
            FaultDetail::Index {
                index,
                lower,
                upper,
            } => write!(
                formatter,
                "array index {index} out of bounds [{lower}..{upper}]"
            ),
            FaultDetail::Subrange {
                value,
                lower,
                upper,
                negative,
            } => {
                formatter.write_str("value ")?;
                FaultDetail::write_signed(formatter, negative & 1 != 0, value)?;
                formatter.write_str(" outside declared subrange ")?;
                FaultDetail::write_signed(formatter, negative & 2 != 0, lower)?;
                formatter.write_str("..")?;
                FaultDetail::write_signed(formatter, negative & 4 != 0, upper)
            }
            FaultDetail::Limit {
                kind,
                used,
                requested,
                limit,
            } => write!(
                formatter,
                "execution timed out: {kind} limit {limit}, used {used}, requested {requested}"
            ),
            // Every other bounded detail belongs to a VM fault, which reads as its runtime error
            // does: the bytecode prefix around the detail, or `call depth exceeded` for a call
            // beyond the call levels (specification 34 MEM-05; specification 12 §7.4.1).
            detail => fmt::Display::fmt(
                &VmFaultDetail {
                    code: self.code,
                    detail,
                }
                .runtime_text(),
                formatter,
            ),
        }
    }
}

/// Today's fixed text of a fault whose record holds no detail, or its stable code when the
/// error carried a name or message the record does not hold (specification 34 MEM-05).
fn fixed_message(code: StableErrorCode) -> &'static str {
    match code {
        StableErrorCode::RuntimeReferenceLifetime => {
            "reference outlived its activation or instance"
        }
        StableErrorCode::RuntimeInvalidAlias => "invalid storage alias use",
        StableErrorCode::RuntimeInvalidExecutionState => {
            "admitted execution metadata is inconsistent"
        }
        StableErrorCode::RuntimeTypeMismatch => "type mismatch",
        StableErrorCode::RuntimeNonFiniteValue => "non-finite floating-point value",
        StableErrorCode::RuntimeDivisionByZero => "division by zero",
        StableErrorCode::RuntimeModuloByZero => "modulo by zero",
        StableErrorCode::RuntimeOverflow => "arithmetic overflow",
        StableErrorCode::RuntimeNullReference => "null reference dereference",
        StableErrorCode::RuntimeForStepZero => "FOR loop step cannot be zero",
        StableErrorCode::RuntimeDateTimeRange => "date/time out of range",
        StableErrorCode::RuntimeResourceFaulted => "resource faulted",
        StableErrorCode::RuntimeWatchdogTimeout => "watchdog timeout",
        StableErrorCode::RuntimeExecutionTimeout => "execution timed out",
        StableErrorCode::RuntimeStringCapacityExceeded => {
            "string result longer than the maximum of 65535"
        }
        other => other.as_str(),
    }
}

/// Map a 128-bit subrange violation into the record's bounded 65-bit form, the form the
/// declared 64-bit IEC types need. A value whose three endpoints do not fit that form keeps
/// its stable code and reports no bounded detail.
fn subrange_detail(value: i128, lower: i128, upper: i128) -> FaultDetail {
    let (
        Some((value, value_negative)),
        Some((lower, lower_negative)),
        Some((upper, upper_negative)),
    ) = (magnitude(value), magnitude(lower), magnitude(upper))
    else {
        return FaultDetail::None;
    };
    FaultDetail::Subrange {
        value,
        lower,
        upper,
        // Sign bits, least significant first: value, lower, upper (specification 34 MEM-05).
        negative: u8::from(value_negative)
            | (u8::from(lower_negative) << 1)
            | (u8::from(upper_negative) << 2),
    }
}

/// One endpoint of a subrange violation as an unsigned magnitude and its sign.
fn magnitude(value: i128) -> Option<(u64, bool)> {
    u64::try_from(value.unsigned_abs())
        .ok()
        .map(|magnitude| (magnitude, value.is_negative()))
}

// The bound is a product contract (specification 34 MEM-05): the record stays fixed-size on
// every target, hosted and MCU.
const _: () = assert!(core::mem::size_of::<FaultRecord>() <= 56);
