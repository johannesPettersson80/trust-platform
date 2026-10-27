//! The memory of the program code region (specification 44 §10.3): the one static of the linker's
//! `.trust_code` section, which `firmware/trust-esp32c6/trust_code.x` places between the firmware's
//! static data and its main stack. `code_region.rs` writes and runs it.

use core::mem::MaybeUninit;

use crate::profile::PROGRAM_CODE_BYTES;

/// The region's bytes, aligned for any section of a module.
#[repr(C, align(64))]
pub(super) struct Region([MaybeUninit<u8>; PROGRAM_CODE_BYTES]);

/// The program region. The firmware gives it to its heap (`code_region::program_region`), whose
/// blocks there hold the programs' code.
// SAFETY: `.trust_code` is a NOLOAD section of RAM that only this static occupies
// (`trust_code.x`), so its placement aliases no other object.
#[unsafe(link_section = ".trust_code")]
pub(super) static mut REGION: Region = Region([MaybeUninit::uninit(); PROGRAM_CODE_BYTES]);
