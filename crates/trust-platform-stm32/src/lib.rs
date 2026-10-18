//! The STM32 chip rules that the STM32 adapters share (specification 34, ARCH-10 and Appendix D.4 and
//! D.5): the pin-name grammar, the lookup of a pin in a chip's table, the reset record and the unique
//! ID words. Each rule is written once here; an adapter holds only its own chip's values.
#![no_std]
#![warn(missing_docs)]

#[cfg(test)]
extern crate std;

pub mod pins;
pub mod reset;
pub mod table;
pub mod uid;

pub use reset::{ResetCookie, ResetReason};
pub use table::{PortSpec, RamRegion, Reservation, SectorRun};
pub use uid::uid_words;
