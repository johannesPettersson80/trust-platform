//! The chip description files of truST, `chips/<chip>.toml` (specification 34, TARGET-03 and
//! Appendix H). Each supported chip is described once: its identity, processor, clock limits,
//! memory and the truST firmware's layout in it, its interfaces and pins, the runtime components
//! its firmware holds, and where each value comes from.
//!
//! [`ChipFile::parse`] reads one file and checks it. A key the format does not define is refused,
//! and so is a file whose values disagree with one another, for example flash sectors that do not
//! add up to the flash or layout regions that overlap. [`Catalog::embedded`] returns the
//! supported chips, whose files this crate embeds.
//!
//! The crate depends on no firmware and no platform adapter, so that the tools can use it: the
//! `trust-runtime` command line, the USB search and installation of TARGET-04, and `xtask`. The
//! adapters use it only in their host tests.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

mod catalog;
mod check;
mod error;
mod model;
mod sources;

pub use catalog::Catalog;
pub use error::{ChipFileError, Problem};
pub use model::{
    ChipFile, Clock, Component, Events, Family, Firmware, Flash, Fpu, Gpio, Identity, Interfaces,
    Layout, Memory, PinReason, PortPins, Processor, RamRegion, Region, ReservedPin, SectorRun,
    Sources, SpiFlash, UsbId,
};
