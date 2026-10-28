//! The installed `TRSTA002` application: the container in the application region of bank 2, read in
//! place and admitted against the chip's descriptor before any state exists (specification 34, §5.1;
//! design §4.4). The flash is memory-mapped, so nothing is copied into the heap: the compiled program's
//! module runs where its image lies, and only its areas take the heap (specification 44 §10.3). The
//! region holds one container
//! until the U5A9's program slots exist; `cargo xtask portability pack-project` writes it as a first
//! flash (spec 34 §5.1, "A first flash").
use alloc::string::{String, ToString};
use trust_runtime_services::installation::{admit_sequenced, Admitted, MAGIC, SEQUENCED_MAGIC};

use crate::profile::{APPLICATION_BASE, APPLICATION_BYTES, DEVICE_DESCRIPTOR};

/// Admits the installed application, or returns the refusal's text: the region holds no container
/// (an erased or partly written region reads as no program, DEPLOY-03), or admission refuses it.
///
/// # Errors
///
/// The refusal's text, which the link reports while the device is Stopped.
pub fn read_and_admit() -> Result<Admitted<'static>, String> {
    // SAFETY: the application region is bank 2 from `APPLICATION_BASE` for `APPLICATION_BYTES`, which
    // the linker keeps out of the firmware (profile.x) and the checkpoint slots follow. The firmware
    // never writes or erases it, so the bytes are constant for the whole boot.
    let region: &'static [u8] =
        unsafe { core::slice::from_raw_parts(APPLICATION_BASE as *const u8, APPLICATION_BYTES) };
    // The magic is checked first, so a partial write reads as no installed program (DEPLOY-03). A
    // version 1 container is not committed, as on the C6 (spec 34 §5.1, the program store).
    if region.starts_with(&MAGIC) {
        return Err(String::from(
            "no installed application: the region holds a TRSTA001 container, which is not committed (only TRSTA002 is read)",
        ));
    }
    if !region.starts_with(&SEQUENCED_MAGIC) {
        return Err(String::from(
            "no installed application: no TRSTA002 container in the region",
        ));
    }
    admit_sequenced(region, &DEVICE_DESCRIPTOR)
        .map(|sequenced| sequenced.admitted)
        .map_err(|error| error.to_string())
}
