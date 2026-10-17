//! Reusable per-depth call-frame payloads (spec 34 MEM-03, MEM-09).
//!
//! Construction reserves one payload per call depth: the local slots and the
//! parameter-presence bits of the largest POU. An activation takes the payload of its
//! depth and its frame returns it when popped, so no scan allocates a locals or presence
//! vector (census A3, A4). A depth beyond the reservation falls back to fresh vectors,
//! exactly as an activation without the reservation allocates today.
//!
//! The payloads are owned by a `FrameStack`, which the engine construction file reserves.
//! The register tier keeps its own single payload as before.
use alloc::vec::Vec;

use crate::error::RuntimeError;
use crate::value::Value;

/// One call depth's reusable payload: its local slots and its parameter-presence bits.
#[derive(Debug, Default)]
struct FramePayload {
    /// Local slot values of the depth, reset to NULL when returned.
    locals: Vec<Value>,
    /// Parameter-presence bits of the depth, reset to false when returned.
    present: Vec<bool>,
}

/// One payload per reserved call depth. Taking a depth's payload leaves its slot empty
/// until [`FramePayloads::reclaim`] returns it; a frame that never took one leaves the
/// slot untouched, so its locals and presence are never confused with a payload.
#[derive(Debug, Default)]
pub struct FramePayloads {
    /// The reserved payloads, indexed by call depth; empty while that depth's frame holds
    /// the payload.
    depths: Vec<Option<FramePayload>>,
}

impl FramePayloads {
    /// Reserve one payload of `slots` local slots and `slots` presence bits for each of
    /// `levels` call depths (spec 34 MEM-09). Every take at a reserved depth then makes no
    /// allocation.
    pub fn reserve(&mut self, levels: usize, slots: usize) -> Result<(), RuntimeError> {
        let refused = |_| super::VmTrap::BudgetExceeded.into_runtime_error();
        self.depths.try_reserve_exact(levels).map_err(refused)?;
        for _ in 0..levels {
            let mut locals = Vec::new();
            locals.try_reserve_exact(slots).map_err(refused)?;
            let mut present = Vec::new();
            present.try_reserve_exact(slots).map_err(refused)?;
            self.depths.push(Some(FramePayload { locals, present }));
        }
        Ok(())
    }

    /// Take the payload reserved for `depth`, cleared to `count` NULL local slots and
    /// `count` absent parameter-presence bits. A depth beyond the reservation allocates a
    /// fresh pair, as a stack without the reservation holds.
    pub fn take(&mut self, depth: usize, count: usize) -> (Vec<Value>, Vec<bool>) {
        if let Some(payload) = self.depths.get_mut(depth).and_then(Option::take) {
            let mut locals = payload.locals;
            locals.clear();
            locals.resize(count, Value::Null);
            let mut present = payload.present;
            present.clear();
            present.resize(count, false);
            (locals, present)
        } else {
            (alloc::vec![Value::Null; count], alloc::vec![false; count])
        }
    }

    /// Return the payload taken at `depth`, reset so it holds no value a later scan could
    /// observe. A slot that still holds its reserved payload is left untouched.
    pub fn reclaim(&mut self, depth: usize, mut locals: Vec<Value>, mut present: Vec<bool>) {
        locals.fill(Value::Null);
        present.fill(false);
        if let Some(slot) = self.depths.get_mut(depth)
            && slot.is_none()
        {
            *slot = Some(FramePayload { locals, present });
        }
    }

    /// Whether the payload of `depth` is out on an active frame.
    #[must_use]
    pub fn is_out(&self, depth: usize) -> bool {
        self.depths.get(depth).is_some_and(Option::is_none)
    }

    /// Bytes that [`FramePayloads::reserve`] takes for `levels` depths of `slots` slots:
    /// the depth table, every local slot and every presence bit.
    #[must_use]
    pub fn reservation_bytes(levels: usize, slots: usize) -> Option<usize> {
        let table = levels.checked_mul(core::mem::size_of::<Option<FramePayload>>())?;
        let values = levels
            .checked_mul(slots)?
            .checked_mul(core::mem::size_of::<Value>())?;
        let presence = levels
            .checked_mul(slots)?
            .checked_mul(core::mem::size_of::<bool>())?;
        table.checked_add(values)?.checked_add(presence)
    }

    /// Payload bytes held by the reserved and out payloads, for the register tier's storage
    /// accounting in tests.
    #[cfg(all(feature = "register-ir", any(test, feature = "test-support")))]
    #[must_use]
    pub fn storage_bytes(&self) -> usize {
        let mut bytes = self
            .depths
            .capacity()
            .saturating_mul(core::mem::size_of::<Option<FramePayload>>());
        for payload in self.depths.iter().flatten() {
            bytes = bytes.saturating_add(
                payload
                    .locals
                    .capacity()
                    .saturating_mul(core::mem::size_of::<Value>()),
            );
            bytes = bytes.saturating_add(
                payload
                    .present
                    .capacity()
                    .saturating_mul(core::mem::size_of::<bool>()),
            );
        }
        bytes
    }
}

#[cfg(test)]
#[path = "payloads/tests.rs"]
mod tests;
