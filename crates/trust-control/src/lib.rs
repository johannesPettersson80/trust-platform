//! Control Studio's calculations and rules (specification 35, §0.1a), in pure Rust without I/O.
//!
//! The language server's Control Studio commands run them against the runtime and store the
//! study; the VS Code extension only shows the result. Times are passed in, never read from a
//! clock, so every function here is deterministic and tested with `cargo test`.

#![forbid(unsafe_code)]

pub mod experiment;
pub mod failure;
pub mod identification;
pub mod json;
pub mod limits;
pub mod loop_definition;
pub mod messages;
pub mod model;
pub mod recording;
pub mod runtime;
pub mod signals;
pub mod stages;
pub mod study;
pub mod tuning;
pub mod validation;
