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
