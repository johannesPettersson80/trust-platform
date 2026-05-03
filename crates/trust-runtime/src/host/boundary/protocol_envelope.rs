use crate::value::Value;

use super::BoundaryError;

#[derive(Debug, Clone, PartialEq)]
pub struct BoundaryEntry {
    pub status: BoundaryEntryStatus,
    pub value: Option<Value>,
    pub error: Option<BoundaryError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryEntryStatus {
    Ok,
    Error,
}

impl BoundaryEntry {
    #[must_use]
    pub fn ok(value: Value) -> Self {
        Self {
            status: BoundaryEntryStatus::Ok,
            value: Some(value),
            error: None,
        }
    }

    #[must_use]
    pub fn error(error: BoundaryError) -> Self {
        Self {
            status: BoundaryEntryStatus::Error,
            value: None,
            error: Some(error),
        }
    }

    #[must_use]
    pub fn is_ok(&self) -> bool {
        matches!(self.status, BoundaryEntryStatus::Ok)
    }
}
