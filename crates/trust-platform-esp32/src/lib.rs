//! ESP32-C6-DevKitC-1 platform services: the loader of compiled program images and the chip's
//! adapters; no scheduler or compiler.
//!
//! Hardware is available only on bare-metal RISC-V. Host-visible arithmetic and
//! profiles describe the same clock, watchdog, input polarity and reset-cookie contract.
#![no_std]
#![warn(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

pub mod boot_counter;
pub mod calendar;
pub mod checkpoint;
mod console_budget;
pub use console_budget::ConsoleError;
pub mod compiled_loader;
mod cookie;
pub use cookie::{ResetCookie, RUNNING_PHASE, WATCHDOG_PHASE};
#[cfg(any(test, all(target_arch = "riscv32", target_os = "none")))]
mod cycle_clock;
pub mod device_flash;
pub mod driver_plans;
pub mod firmware_flash;
pub mod firmware_slots;
// The fixed-size reset record the C6 keeps in RTC fast memory, shared with the U5A9 so both chips
// report the same cause after a reset.
pub use trust_platform_memory::reset::{ResetCause, ResetRecord};
mod image;
pub use image::{decode_image_header, ImageHeader};
pub mod loader;
mod platform;
pub use platform::Platform;
#[path = "hardware/process_io.rs"]
pub mod process_io;
pub mod profile;
pub mod program_flash;
mod record;
pub use record::{fail_record, FAIL_RECORD_MAX};
pub mod usb_service;
pub mod watchdog_rule;

#[cfg(all(target_arch = "riscv32", target_os = "none"))]
mod hardware;
#[cfg(all(target_arch = "riscv32", target_os = "none"))]
pub use hardware::{
    bring_up, emergency_halt, end_program_load, mark_program_load, pairing_random, start_scheduler,
    Board, BoardError, BoardHardware, BoardParts, C6Net, Console, Identity, Monotonic, NetLink,
    RadioFailure, RadioParts, ResetControl, UsbLink, Wait, Watchdog,
};

#[cfg(all(target_arch = "riscv32", target_os = "none"))]
pub use hardware::{program_region, C6Code, Line, ProgramCode};
