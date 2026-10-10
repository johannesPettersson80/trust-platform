//! Portable runtime values, scheduling helpers and bytecode boundary.
//!
//! The core owns shared value/numeric operations and the STBC representation,
//! decoder, encoder, budgeted validator and disassembler. Source/HIR lowering,
//! product transports and hardware drivers remain outside this portable boundary.
//! The shared dispatcher executes legacy hosted STBC through a host context and
//! source-free STBC 2.0 through artifact-backed preparation and single-owner state.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

extern crate alloc;

#[cfg(all(test, not(feature = "std")))]
extern crate std;

/// Portable bytecode metadata records.
pub mod bytecode;
/// Portable insertion-ordered collection aliases.
pub mod collections;
/// Portable cycle scheduling helpers.
pub mod cycle;
/// Portable date/time calculation helpers.
pub mod datetime;
/// Portable runtime errors.
pub mod error;
/// Stable machine-readable runtime error identifiers.
pub mod error_code;
/// Portable direct I/O address syntax.
pub mod io_address;
/// Shared process-image address and value codecs.
pub mod io_image;
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

/// Portable standard library execution.
pub mod stdlib;
