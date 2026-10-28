//! The structured, allocation-free VM fault carried by
//! [`RuntimeError::VmFault`](crate::error::RuntimeError::VmFault).
//!
//! A [`VmFaultDetail`](crate::error::VmFaultDetail) is the fixed-size, `Copy` value that a
//! [`VmTrap`](crate::vm::VmTrap) becomes during a scan (specification 34 MEM-05). Its text is
//! today's text and its stable code is unchanged.

use core::fmt;

use crate::error::StableErrorCode;
use crate::fault::FaultDetail;

/// A VM fault with its stable code and bounded context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmFaultDetail {
    /// Stable machine-readable fault identity.
    pub code: StableErrorCode,
    /// Bounded fault context.
    pub detail: FaultDetail,
}

impl fmt::Display for VmFaultDetail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.detail, formatter)
    }
}

impl VmFaultDetail {
    /// The fault as a runtime error reads: `invalid bytecode '<detail>'` for a fault of the
    /// executable, and `call depth exceeded` for a call beyond the call levels, which a valid
    /// program that recurses too deep reaches (specification 12 §7.4.1). The stable code is the
    /// same for both readings.
    #[must_use]
    pub fn runtime_text(&self) -> RuntimeText<'_> {
        RuntimeText(self)
    }
}

/// [`VmFaultDetail::runtime_text`].
pub struct RuntimeText<'a>(&'a VmFaultDetail);

impl fmt::Display for RuntimeText<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.code == StableErrorCode::VmCallStackOverflow {
            formatter.write_str("call depth exceeded")
        } else {
            write!(formatter, "invalid bytecode '{}'", self.0)
        }
    }
}
