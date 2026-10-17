//! STM32F4 platform services, one Cargo feature per chip: `stm32f401` (the NUCLEO-F401RE
//! reference composition, the default) and `stm32f407` (the chip, first used on the
//! STM32F407G-DISC1); no scheduler, compiler or VM implementation.
//!
//! Hardware is available only on bare-metal ARM. Host-visible arithmetic and profiles describe
//! the same timer, input polarity, reset-cookie, pin-name and clock contracts. What differs
//! between the chips is data ([`chip`]); the board's wiring is a [`Wiring`] the firmware passes in.
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
mod time;

pub use chip::{Chip, CHIP};
pub use console_budget::ConsoleError;
pub use pins::{Pin, PinError, Port, Pull, Wiring};
pub use profile::{InputSnapshot, ResetCookie, ResetReason, SCAN_PERIOD_US, SYSCLK_HZ};
pub use time::CounterExtension;

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "stm32f401",
    feature = "stm32f407"
))]
compile_error!("select one chip: the stm32f401 feature or the stm32f407 feature, not both");

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(any(feature = "stm32f401", feature = "stm32f407"))
))]
compile_error!("select a chip: the stm32f401 feature or the stm32f407 feature");

// The peripheral access crate of the selected chip, shared by the files both chips use.
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "stm32f401",
    not(feature = "stm32f407")
))]
use stm32f4::stm32f401 as pac;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "stm32f407",
    not(feature = "stm32f401")
))]
use stm32f4::stm32f407 as pac;

// The F401 composition: its hardware files are the ones the NUCLEO-F401RE evidence was taken with.
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "stm32f401",
    not(feature = "stm32f407")
))]
mod hardware;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "stm32f401",
    not(feature = "stm32f407")
))]
pub use hardware::{
    emergency_halt, systick_interrupt, Board, BoardError, Console, DigitalIo, Identity, Monotonic,
    ResetControl, Watchdog,
};

// The F407: pins by name, the crystal measured against the HSI, no pin of any board built in.
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "stm32f407",
    not(feature = "stm32f401")
))]
#[path = "hardware_f407/mod.rs"]
mod hardware;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "stm32f407",
    not(feature = "stm32f401")
))]
pub use hardware::{
    emergency_halt, systick_interrupt, Board, BoardError, Console, DigitalIo, Identity, Input,
    Monotonic, Output, Process, ResetControl, Watchdog,
};
