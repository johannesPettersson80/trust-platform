//! Portable debugger resolution that needs no ST source text.
//!
//! Specification 12 §6.13 stores each executable statement's code span and source
//! span in the module's `LINE_TABLE`, so every runtime reaches the same breakpoint
//! and step answer from the artifact alone (specification 12 §10; specification 13,
//! "Source Mapping"). [`crate::debug::line_resolve`] owns that portable
//! resolver.

pub mod line_resolve;

pub use line_resolve::{LineResolver, ResolvedStatement};
