//! The `stbc` application bundle. The installed bundle is copied once per boot, while STOPPED and
//! before the fixture borrows the board, into a static RAM buffer the size of the fixture's
//! capacity. The shared fixture validates and decodes it.
use core::sync::atomic::{AtomicBool, Ordering};
use trust_board_fixture::application::{decode, Applications, CAPACITY};
use trust_platform_esp32::{profile::STBC_BYTES, Board};

/// RAM copy of the installed bundle, kept for the rest of the boot. The buffer is the bench
/// bundle's capacity (16 KiB), not the 6112 KiB `stbc` partition (spec 34 §5.1): a static buffer
/// of the partition's size cannot exist in the chip's 512 KiB of RAM.
static mut BUNDLE: [u8; CAPACITY] = [0; CAPACITY];
// The RAM buffer is the bench bundle's capacity, never the whole partition.
const _: () = assert!(CAPACITY < STBC_BYTES);
/// Set by the one call to [`read`]. A second call would alias `BUNDLE`, so it is refused.
static READ: AtomicBool = AtomicBool::new(false);

/// Copy the installed bundle into RAM. The outcome is returned rather than propagated, so that the
/// runner's BOOT records come before any read failure, as on the STM32F407.
pub fn read(board: &mut Board) -> Result<&'static [u8], &'static str> {
    if READ.swap(true, Ordering::AcqRel) {
        return Err("bundle-reread");
    }
    // SAFETY: the swap above succeeds once per boot, so this is the only reference to `BUNDLE` that
    // the program ever creates.
    let buffer: &'static mut [u8] = unsafe { &mut *core::ptr::addr_of_mut!(BUNDLE) };
    board.read_application(buffer).map_err(|_| "flash-read")?;
    let bundle: &'static [u8] = buffer;
    Ok(bundle)
}

/// The fixture's `load` step: validate the bundle that [`read`] copied before any payload is exposed.
pub fn load(
    bundle: Result<&'static [u8], &'static str>,
) -> Result<Applications<'static>, &'static str> {
    decode(bundle?)
}
