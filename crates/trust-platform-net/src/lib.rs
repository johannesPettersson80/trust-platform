//! The network of a chip without an operating system (specification 34 ARCH-12; checklist L3): an
//! IPv4 interface over the chip's Ethernet or Wi-Fi frames, its address from DHCP or from the device
//! settings, and the control connection over TCP with the framing every control link shares
//! (PORT-05). Every chip with a network runs it (ARCH-11): the chip's adapter supplies the frames as
//! a [`smoltcp`] device, the link state, its MAC address and its clock in microseconds, and joins the
//! network itself.
#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

mod control;
pub mod retry;
mod settings;
mod stack;

pub use control::BUSY_ERROR;
pub use settings::{Addressing, NetworkSettings};
pub use smoltcp;
pub use stack::{Network, FRAMES_PER_POLL};
