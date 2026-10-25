//! The portable kernel of the truST runtime (specification 44): the program image and the kernel's
//! side of the module interface, the values and their codecs, retain, the deployment record, the
//! process image, tasks and scheduling, faults and the watchdog, which every device builds without
//! the standard library. Source lowering, product transports and hardware drivers stay outside.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]
// `RuntimeError` owns heap payloads, so `ok_or(Variant)` builds an error and runs its drop glue
// on every success; the paths that run each scan build errors lazily.
#![allow(clippy::unnecessary_lazy_evaluations)]

extern crate alloc;

mod sort;

#[cfg(all(test, not(feature = "std")))]
extern crate std;

/// Portable insertion-ordered collection aliases.
pub mod collections;
/// Shared CRC-32/ISO-HDLC wire checksum, with a compact no_std implementation.
pub mod crc32;
/// Portable cycle scheduling helpers.
pub mod cycle;
/// Portable date/time calculation helpers.
pub mod datetime;
/// A statement's place in its source (specification 44 §13.1).
pub mod debug;
/// Deployment records and their admission (specification 34 §5.1.1).
pub mod deployment;
/// Portable runtime errors.
pub mod error;
/// Stable machine-readable runtime error identifiers.
pub mod error_code;
/// Fixed-size fault records for the core control path.
pub mod fault;
/// Portable direct I/O address syntax.
pub mod io_address;
/// Shared process-image address and value codecs.
pub mod io_image;
/// Portable runtime memory identity types.
pub mod memory;
/// Compiled programs: the program image and the kernel's side of the module interface
/// (specification 44).
pub mod native;
/// Portable numeric conversion helpers.
pub mod numeric;
/// Small platform contracts: clocks, the resource loop's wait and wake, and the control link.
pub mod ports;
/// Portable runtime program model helpers.
pub mod program_model;
/// Portable retain and restart policy records.
pub mod retain;
/// Portable scheduler model records.
pub mod scheduler;
/// Portable task configuration records.
pub mod task;
/// Portable runtime value model pieces.
pub mod value;
/// Portable watchdog, retain-mode, and fault-policy model records.
pub mod watchdog;

/// Portable standard library execution.
pub mod stdlib;
