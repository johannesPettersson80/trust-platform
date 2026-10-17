//! Independent, stopped-install application sector. No artifact is linked into firmware.
use trust_board_fixture::application::{decode, Applications, CAPACITY};

/// Flash sector 1 (16 KiB), the separately installed application bundle (spec 34, D.4); the
/// installation step writes it at this address.
const ADDRESS: usize = 0x0800_4000;

pub fn load() -> Result<Applications<'static>, &'static str> {
    // SAFETY: F407 flash sector 1 is readable, independently installed before
    // reset, never programmed by this firmware, and excluded by the linker.
    let sector = unsafe { core::slice::from_raw_parts(ADDRESS as *const u8, CAPACITY) };
    decode(sector)
}
