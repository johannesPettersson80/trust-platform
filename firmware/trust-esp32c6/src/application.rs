//! The installed application: the boot selection over the two program slots, which reads the header of
//! each slot, takes the committed container with the highest install sequence, and admits it against
//! the C6's descriptor before any state exists (design §1.1 and §1.2, specification 34 §5.1). Only a
//! `TRSTA002` container is committed: a slot that holds a `TRSTA001` container is not, and the refusal
//! of the boot names it.
use alloc::string::{String, ToString};
use trust_platform_esp32::profile::DEVICE_DESCRIPTOR;
use trust_runtime_core::ports::ProgramStore;
use trust_runtime_services::loading::select::{self, Booted};

/// Why the boot has no application, and which container it refused.
pub struct Refused {
    /// The reason, which `restart` names with `no installed program: <reason>` (design §1.1, R2).
    pub reason: String,
    /// The install sequence of the newest committed container, which the boot refused or whose program
    /// it could not start; none when the store held no committed container. A `restart` loads a
    /// container only when the store holds a newer one (specification 34 STATE-08).
    pub sequence: Option<u64>,
}

/// Select the installed application and admit it. The boot reads the headers of both slots from
/// flash, and then the chosen container once, into the heap. A container longer than the room the heap
/// has is refused with its numbers before it is read (design §1.1, R1). The container is not kept: the
/// program's bytes and the decoded record are handed over, and the rest is released, so the heap holds
/// no copy of the container for the rest of the boot.
///
/// # Errors
///
/// The reason the boot is refused: neither slot holds a committed container, the slots cannot be read
/// or ordered, the newest committed container is damaged or refused by admission (the other slot is
/// never started in its place, DEPLOY-03), or it is longer than the heap's room.
pub fn boot(store: &mut ProgramStore) -> Result<Booted, Refused> {
    select::boot(store, &DEVICE_DESCRIPTOR, crate::memory::HEAP.remaining()).map_err(|error| {
        // The sequence of the newest committed container, when the slots can be ordered again.
        let sequence = select::order(store)
            .ok()
            .and_then(|order| order.newest)
            .map(|newest| newest.header.sequence);
        Refused {
            reason: error.reason().to_string(),
            sequence,
        }
    })
}
