//! The installed application: the boot selection over the two program slots, which reads the header of
//! each slot, takes the committed container with the highest install sequence, and admits it against
//! the C6's descriptor before any state exists (design §1.1 and §1.2, specification 34 §5.1). Only a
//! `TRSTA002` container is committed: a slot that holds a `TRSTA001` container is not, and the refusal
//! of the boot names it.
use alloc::string::{String, ToString};
use trust_platform_esp32::profile::DEVICE_DESCRIPTOR;
use trust_runtime_core::ports::ProgramStore;
use trust_runtime_services::loading::select::{self, Booted};

/// Select the installed application and admit it. The boot reads the headers of both slots from
/// flash, and then the chosen container once, into the heap. A container longer than the room the heap
/// has is refused with its numbers before it is read (design §1.1, R1). The container is not kept: the
/// program's bytes and the decoded record are handed over, and the rest is released, so the heap holds
/// no copy of the container for the rest of the boot.
///
/// # Errors
///
/// The reason the boot is refused, which `restart` names with `no installed program: <reason>` (design
/// §1.1, R2): neither slot holds a committed container, the slots cannot be read or ordered, the
/// newest committed container is damaged or refused by admission (the other slot is never started in
/// its place, DEPLOY-03), or it is longer than the heap's room.
pub fn boot(store: &mut ProgramStore) -> Result<Booted, String> {
    select::boot(store, &DEVICE_DESCRIPTOR, crate::memory::HEAP.remaining())
        .map_err(|error| error.reason().to_string())
}
