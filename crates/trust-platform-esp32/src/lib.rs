//! ESP32-C6-DevKitC-1 platform services; no scheduler, compiler or VM implementation.
//!
//! Hardware is available only on bare-metal RISC-V. Host-visible arithmetic and
//! profiles describe the same clock, watchdog, input polarity and reset-cookie contract.
#![no_std]
#![warn(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

pub mod checkpoint;
mod console_budget;
pub use console_budget::ConsoleError;
mod cookie;
pub use cookie::{ResetCookie, RUNNING_PHASE, WATCHDOG_PHASE};
// The fixed-size reset record the C6 keeps in RTC fast memory, shared with the F407 so both
// chips report the same cause after a reset.
pub use trust_platform_memory::reset::{ResetCause, ResetRecord};
mod image;
pub use image::{decode_image_header, ImageHeader};
mod platform;
pub use platform::Platform;
#[path = "hardware/process_io.rs"]
pub mod process_io;
pub mod profile;
mod record;
pub use record::{fail_record, FAIL_RECORD_MAX};
pub mod usb_service;
pub mod watchdog_rule;

#[cfg(all(target_arch = "riscv32", target_os = "none"))]
mod hardware;
#[cfg(all(target_arch = "riscv32", target_os = "none"))]
pub use hardware::{
    emergency_halt, Board, BoardError, Console, Identity, Monotonic, ResetControl, UsbLink, Wait,
    Watchdog,
};
