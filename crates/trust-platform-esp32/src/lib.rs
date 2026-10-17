//! ESP32-C6-DevKitC-1 platform services; no scheduler, compiler or VM implementation.
//!
//! Hardware is available only on bare-metal RISC-V. Host-visible arithmetic and
//! profiles describe the same clock, watchdog, input polarity and reset-cookie contract.
#![no_std]
#![warn(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(all(target_arch = "riscv32", target_os = "none"))]
extern crate alloc;

mod console_budget;
pub use console_budget::ConsoleError;
mod cookie;
pub use cookie::{ResetCookie, RUNNING_PHASE, WATCHDOG_PHASE};
mod image;
pub use image::{decode_image_header, ImageHeader};
#[path = "hardware/process_io.rs"]
pub mod process_io;
pub mod profile;
mod record;
pub use record::{fail_record, FAIL_RECORD_MAX};
pub mod watchdog_rule;

#[cfg(all(target_arch = "riscv32", target_os = "none"))]
mod hardware;
#[cfg(all(target_arch = "riscv32", target_os = "none"))]
pub use hardware::{
    emergency_halt, Board, BoardError, Console, DigitalIo, Identity, Monotonic, ResetControl, Wait,
    Watchdog,
};
