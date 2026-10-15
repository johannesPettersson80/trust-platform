//! The `stbc` application bundle. The partition is copied once per boot, while STOPPED and before the
//! fixture borrows the board, into a static RAM buffer. The shared fixture validates and decodes it.
use core::sync::atomic::{AtomicBool, Ordering};
use trust_board_fixture::application::{decode, Applications};
use trust_platform_esp32::{profile::STBC_BYTES, Board};

/// RAM copy of the whole `stbc` partition, kept for the rest of the boot.
static mut BUNDLE: [u8; STBC_BYTES] = [0; STBC_BYTES];
/// Set by the one call to [`read`]. A second call would alias `BUNDLE`, so it is refused.
static READ: AtomicBool = AtomicBool::new(false);

/// Copy the installed bundle into RAM. The outcome is returned rather than propagated, so that the
/// runner's BOOT records come before any read failure, as on F401.
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
