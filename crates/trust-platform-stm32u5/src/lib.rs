//! The STM32U5A9 chip adapter for the shared truST engine (specification 34, ARCH-10 and Appendix
//! D.5): the chip table, the pin names, the clock tree, the runner's clock, wait and wake ([`wait`])
//! and the USB control link, and the flash checkpoint slots ([`flash`]) and the reset record
//! ([`reset`]). The watchdog and the GPIO follow in S1b-8u-3b, and the firmware and the bench in S1b-9u.
//!
//! The chip's values are data ([`chip`]) and must equal `chips/stm32u5a9.toml`; a host test compares
//! them. The clock arithmetic is host-tested ([`clock`]); its register setup is compiled for the chip
//! only. The USB identity and class are host-tested ([`usb`], [`usb_service`]); the interrupt and the
//! static link are compiled for the chip only (`usb_link`). No HAL crate is used: the adapter is
//! register-level against the `stm32u5` PAC.
#![no_std]
#![warn(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(test)]
extern crate std;

pub mod chip;
pub mod clock;
pub mod flash;
pub mod pins;
pub mod reset;
pub mod usb;
pub mod usb_service;
pub mod wait;

/// The control link over the OTG_HS, compiled for the chip only (its interrupt and registers).
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub mod usb_link;
