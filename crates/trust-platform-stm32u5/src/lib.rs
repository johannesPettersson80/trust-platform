//! The STM32U5A9 chip adapter for the shared truST engine (specification 34, ARCH-10 and Appendix
//! D.5): the chip table, the pin names and the clock tree. The USB device, the flash, the watchdog
//! and the reset record follow in S1b-8u-2 and S1b-8u-3, and the firmware and the bench in S1b-9u.
//!
//! The chip's values are data ([`chip`]) and must equal `chips/stm32u5a9.toml`; a host test compares
//! them. The clock arithmetic is host-tested ([`clock`]); its register setup is compiled for the chip
//! only. No HAL crate is used: the adapter is register-level against the `stm32u5` PAC.
#![no_std]
#![warn(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(test)]
extern crate std;

pub mod chip;
pub mod clock;
pub mod pins;
