//! Portable runtime execution core scaffold.
//!
//! `trust-runtime-core` is reserved for behavior-preserving runtime execution
//! pieces that can move out of the Linux/product host. The crate must not own
//! host transports, web/HMI/control/cloud adapters, Linux realtime setup,
//! product CLI wiring, test harness compilation, or external I/O drivers.
//!
//! The first scaffold intentionally contains only ownership markers and a
//! minimal test. Runtime behavior stays in `trust-runtime` until the behavior
//! locks and full-map doctor gates for the moved slice are green.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

extern crate alloc;

/// Portable bytecode metadata records.
pub mod bytecode;
/// Portable cycle scheduling helpers.
pub mod cycle;
/// Portable date/time calculation helpers.
pub mod datetime;
/// Portable runtime errors.
pub mod error;
/// Portable runtime memory identity types.
pub mod memory;
/// Portable numeric conversion helpers.
pub mod numeric;
/// Portable runtime program model helpers.
pub mod program_model;
/// Portable retain and restart policy records.
pub mod retain;
/// Scaffold ownership markers for the pre-move core crate.
pub mod scaffold;
/// Portable scheduler model records.
pub mod scheduler;
/// Portable task configuration records.
pub mod task;
/// Portable runtime value model pieces.
pub mod value;
/// Portable VM execution helpers.
pub mod vm;
/// Portable watchdog, retain-mode, and fault-policy model records.
pub mod watchdog;
