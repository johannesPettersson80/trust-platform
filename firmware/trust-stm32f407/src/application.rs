//! Independent, stopped-install application region. No artifact is linked into firmware.
use trust_board_fixture::application::{decode, Applications, CAPACITY};

/// Flash sectors 10 and 11 (256 KiB), the separately installed application bundle (spec 34, D.4);
/// the installation step writes it at this address.
const ADDRESS: usize = 0x080C_0000;
// The bundle is the fixture's capacity (16 KiB), never the whole 256 KiB application region
// (spec 34 §5.1).
const _: () = assert!(CAPACITY < 256 * 1024);

pub fn load() -> Result<Applications<'static>, &'static str> {
    // SAFETY: F407 flash sectors 10 and 11 are readable, independently installed before
    // reset, never programmed by this firmware, and excluded by the linker.
    let sector = unsafe { core::slice::from_raw_parts(ADDRESS as *const u8, CAPACITY) };
    decode(sector)
}
