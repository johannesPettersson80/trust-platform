//! Pending deferred calls of the stack dispatcher (spec 34 §6.2.2).
use super::session::PumpContinuation;
use super::ExecutionContext;
use crate::error::RuntimeError;
use crate::vm::VmTrap;
use alloc::vec::Vec;
use core::ops::{Deref, DerefMut};

/// The callers waiting for their deferred callees, innermost last. Construction reserves the
/// storage for the admitted call depth; growth is charged against `charged_capacity`, the
/// capacity the stack has when it grows only on demand, so a reservation changes no charge or
/// refusal (spec 34 TIME-08).
#[derive(Debug, Default)]
pub(super) struct ContinuationStack {
    entries: Vec<PumpContinuation>,
    charged_capacity: usize,
}

impl Deref for ContinuationStack {
    type Target = [PumpContinuation];

    fn deref(&self) -> &[PumpContinuation] {
        &self.entries
    }
}

impl DerefMut for ContinuationStack {
    fn deref_mut(&mut self) -> &mut [PumpContinuation] {
        &mut self.entries
    }
}

impl ContinuationStack {
    /// Reserve storage for `capacity` pending calls at construction (spec 34 MEM-03).
    pub(super) fn reserve_storage(&mut self, capacity: usize) -> Result<(), VmTrap> {
        self.entries
            .try_reserve_exact(capacity.saturating_sub(self.entries.len()))
            .map_err(|_| VmTrap::BudgetExceeded)
    }

    /// Admit room for one more pending call before any state is published: the doubled
    /// capacity, bounded by the admitted call depth, is charged as call storage when the
    /// charged capacity is full.
    pub(super) fn reserve(&mut self, runtime: &impl ExecutionContext) -> Result<(), RuntimeError> {
        let len = self.entries.len();
        if len < self.charged_capacity {
            return Ok(());
        }
        let capacity = self
            .charged_capacity
            .checked_mul(2)
            .ok_or(RuntimeError::Overflow)?
            .max(1)
            .min(runtime.call_depth_limit());
        if capacity <= len {
            return Err(VmTrap::CallStackOverflow.into_runtime_error());
        }
        let bytes = capacity
            .checked_mul(core::mem::size_of::<PumpContinuation>())
            .ok_or(RuntimeError::Overflow)?;
        runtime.charge_call_storage(bytes, len)?;
        // Storage reserved at construction already holds the entry; without a reservation
        // the stack grows here, exactly to the charged capacity.
        if len == self.entries.capacity() {
            self.entries
                .try_reserve_exact(capacity - len)
                .map_err(|_| RuntimeError::Overflow)?;
        }
        self.charged_capacity = capacity;
        Ok(())
    }

    /// An empty stack charged as this one, with storage only for its charged capacity, as a
    /// stack that grew on demand holds.
    #[cfg(test)]
    pub(super) fn grown_on_demand(&self) -> Self {
        Self {
            entries: Vec::with_capacity(self.charged_capacity),
            charged_capacity: self.charged_capacity,
        }
    }

    pub(super) fn push(&mut self, continuation: PumpContinuation) {
        self.entries.push(continuation);
    }

    pub(super) fn pop(&mut self) -> Option<PumpContinuation> {
        self.entries.pop()
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }
}
