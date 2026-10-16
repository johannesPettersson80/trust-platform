//! Independent, stopped-install application sector. No artifact is linked into firmware.
use trust_board_fixture::application::{decode, Applications, CAPACITY};

/// F401RE flash sector 1: the separately installed application bundle.
const ADDRESS: usize = 0x0800_4000;

pub fn load() -> Result<Applications<'static>, &'static str> {
    // SAFETY: F401RE flash sector 1 is readable, independently installed before
    // reset, never programmed by this firmware, and excluded by the linker.
    let sector = unsafe { core::slice::from_raw_parts(ADDRESS as *const u8, CAPACITY) };
    decode(sector)
}
