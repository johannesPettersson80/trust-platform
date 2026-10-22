//! Control Studio's calculations and rules (specification 35, §0.1a), in pure Rust without I/O.
//!
//! The language server's Control Studio commands run them against the runtime and store the
//! study; the VS Code extension only shows the result. Times are passed in, never read from a
//! clock, so every function here is deterministic and tested with `cargo test`.

#![forbid(unsafe_code)]

pub mod analysis;
pub mod chart_export;
pub mod commissioning;
pub mod experiment;
pub mod export;
pub mod failure;
pub mod identification;
pub mod json;
pub mod limits;
pub mod loop_definition;
pub mod loop_monitor;
pub mod messages;
pub mod model;
pub mod plant_monitor;
pub mod recording;
pub mod runtime;
pub mod setup;
pub mod signals;
pub mod simulator_start;
pub mod stages;
pub mod study;
pub mod target_label;
pub mod tuning;
pub mod validation;
