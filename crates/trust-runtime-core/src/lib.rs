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

/// Scaffold ownership markers for the pre-move core crate.
pub mod scaffold;
/// Portable runtime value model pieces.
pub mod value;
