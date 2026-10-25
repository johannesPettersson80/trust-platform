//! The network of a chip without an operating system (specification 34 ARCH-12; checklist L3, S6
//! and T6): an IPv4 interface over the chip's Ethernet or Wi-Fi frames, its address from DHCP or
//! from the device settings, the control connection over TCP with the framing every control link
//! shares (PORT-05), the TCP connections of the I/O drivers to their servers, the mDNS announcement by which every truST runtime finds the chip, the time
//! server that sets its clock, and the pre-shared key of a WPA2 passphrase. Every chip with a network runs it (ARCH-11): the chip's adapter supplies the frames as
//! a [`smoltcp`] device, the link state, its MAC address and its clock in microseconds, and joins the
//! network itself.
#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

mod client;
mod control;
mod discovery;
pub mod mdns;
pub mod retry;
mod settings;
pub mod sntp;
mod stack;
mod time;
pub mod wpa;

pub use client::{ClientLink, LinkFull, LinkStatus, LINK_BYTES};
pub use control::{BUSY_CODE, BUSY_ERROR};
pub use discovery::Identity;
pub use settings::{Addressing, NetworkSettings};
pub use smoltcp;
pub use stack::{Network, FRAMES_PER_POLL};
pub use time::TimeResult;
