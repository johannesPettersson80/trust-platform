//! Fail-closed runtime boundary helpers.

#![allow(missing_docs)]

mod error;
mod protocol_envelope;
mod resolver;

pub use error::BoundaryError;
pub use protocol_envelope::{BoundaryEntry, BoundaryEntryStatus};

pub(crate) use resolver::{resolve_bind, resolve_read, resolve_write};
