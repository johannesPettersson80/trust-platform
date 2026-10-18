//! STM32F4 platform services for the STM32F407 (Cargo feature `stm32f407`, the default), first
//! used on the STM32F407G-DISC1; no scheduler, compiler or VM implementation.
//!
//! Hardware is available only on bare-metal ARM. Host-visible arithmetic and profiles describe
//! the timer, reset-cookie, pin-name and clock contracts. The chip's values are data ([`chip`]);
//! the board's wiring is a [`Wiring`] the firmware passes in.
#![no_std]
#![warn(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(test)]
extern crate std;

pub mod baud;
pub mod chip;
mod console_budget;
pub mod crystal;
pub mod pins;
mod profile;

pub use chip::{Chip, CHIP};
pub use console_budget::ConsoleError;
pub use pins::{Pin, PinError, Port, Pull, Wiring};
pub use profile::{SCAN_PERIOD_US, SYSCLK_HZ};
pub use trust_platform_stm32::time::CounterExtension;
pub use trust_platform_stm32::{ResetCookie, ResetReason};

#[cfg(all(target_arch = "arm", target_os = "none", not(feature = "stm32f407")))]
compile_error!("select the chip: the stm32f407 feature");

// The peripheral access crate of the chip, shared by the hardware files.
#[cfg(all(target_arch = "arm", target_os = "none", feature = "stm32f407"))]
use stm32f4::stm32f407 as pac;

// The F407: pins by name, the crystal measured against the HSI, no pin of any board built in.
#[cfg(all(target_arch = "arm", target_os = "none", feature = "stm32f407"))]
#[path = "hardware_f407/mod.rs"]
mod hardware;
#[cfg(all(target_arch = "arm", target_os = "none", feature = "stm32f407"))]
pub use hardware::{
    emergency_halt, systick_interrupt, Board, BoardError, Console, DigitalIo, Identity, Input,
    Monotonic, Output, Process, ResetControl, Watchdog,
};
