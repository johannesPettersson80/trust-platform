//! Bounded diagnostics for an exhausted execution allowance.

use core::fmt;

/// The independently admitted allowance that refused an execution operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionLimitKind {
    /// Instructions and charged helper work in the current entry.
    Work,
    /// Values materialized by initialization in the current entry.
    ConstructedValues,
    /// Cumulative backing-storage/copy demand in the current entry.
    AllocationBytes,
}

impl fmt::Display for ExecutionLimitKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Work => "work",
            Self::ConstructedValues => "constructed values",
            Self::AllocationBytes => "allocation bytes",
        })
    }
}
