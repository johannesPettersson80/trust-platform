//! Fail-closed runtime boundary helpers.

#![allow(missing_docs)]

mod error;
mod protocol_envelope;

pub use error::BoundaryError;
pub use protocol_envelope::{BoundaryEntry, BoundaryEntryStatus};
