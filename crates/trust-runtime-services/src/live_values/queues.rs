//! The Live Values queues: the pending writes, the active forces and their rules
//! (specification 11, "Live Values"; specification 34 §4.3.1).
//!
//! The structure is plain: no lock, no clock and no operating-system dependency, so a chip
//! runtime can hold it too. The composing debugger owns the lock and reports a refusal; these
//! queues own the rules. A write or force to a target that is already queued replaces it in
//! place and is never refused; a new target past [`REQUEST_QUEUE_CAPACITY`] is refused with
//! [`RequestQueueFull`] and changes nothing.

use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::memory::{FrameId, Generation, InstanceId};
use trust_runtime_core::value::Value;

/// Entries each bounded queue holds before it refuses a new request: pending one-shot I/O
/// writes, pending variable writes, pending lvalue writes, active I/O forces and active variable
/// forces (specification 34 §4.3.1, specification 11 "request queue full").
pub const REQUEST_QUEUE_CAPACITY: usize = 1_024;

/// A control request refused because its bounded queue is full. Its text is the stable control
/// error `request queue full`, and the refused request changes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestQueueFull;

impl core::fmt::Display for RequestQueueFull {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("request queue full")
    }
}

/// The destination of a pending variable write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingVarTarget {
    /// A global variable by name.
    Global(SmolStr),
    /// A retained global variable by name.
    Retain(SmolStr),
    /// A field of the instance with this identity in this installation generation.
    Instance(Generation, InstanceId, SmolStr),
    /// A local of the frame with this identity and this name.
    Local(FrameId, SmolStr),
}

/// A pending variable write: the target and the value to apply at the next cycle boundary.
#[derive(Debug, Clone)]
pub struct PendingVarWrite {
    /// The write's target.
    pub target: PendingVarTarget,
    /// The value to write.
    pub value: Value,
}

/// The destination of an active variable force.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForcedVarTarget {
    /// A global variable by name.
    Global(SmolStr),
    /// A retained variable by name.
    Retain(SmolStr),
    /// A field of the instance with this identity in this installation generation; the scan
    /// boundary rejects the force once another generation is installed (spec 34 §4.3.1).
    Instance(Generation, InstanceId, SmolStr),
}

/// An active variable force: the target and the value it is held at.
#[derive(Debug, Clone)]
pub struct ForcedVar {
    /// The forced target.
    pub target: ForcedVarTarget,
    /// The value the target is held at.
    pub value: Value,
}

/// An owned copy of the active forces at one moment.
#[derive(Debug, Clone)]
pub struct ForcedSnapshot {
    /// Active variable forces, in the order they were first forced.
    pub vars: Vec<ForcedVar>,
    /// Active I/O forces, in the order they were first forced.
    pub io: Vec<(IoAddress, Value)>,
}

/// The pending writes and active forces of one control surface.
///
/// A write or force to a target that is already queued replaces it in place; a `Vec` keeps the
/// position of the first request, so the drain order of the other targets is unchanged. A new
/// target past [`REQUEST_QUEUE_CAPACITY`] is refused with [`RequestQueueFull`] and queues
/// nothing. The one-shot I/O and lvalue queues keep repeated entries in arrival order, because
/// each request is a separate write (specification 11, "Live Values"; specification 34 §4.3.1).
///
/// It carries no lock; the composing debugger holds it inside its locked state.
#[derive(Debug, Clone, Default)]
pub struct LiveValueQueues {
    /// One-shot I/O writes, in arrival order; a repeated address is kept as a separate write.
    pub io_writes: Vec<(IoAddress, Value)>,
    /// Pending variable writes; a write to a target already here replaces it in place.
    pub pending_var_writes: Vec<PendingVarWrite>,
    /// Active variable forces; a force to a target already here replaces it in place.
    pub forced_vars: Vec<ForcedVar>,
    /// Active I/O forces; a force to an address already here replaces it in place.
    pub forced_io: Vec<(IoAddress, Value)>,
}

impl LiveValueQueues {
    /// Empty queues.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether no write is queued and no force is active, so applying the queues changes nothing
    /// (checklist P1-6).
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.io_writes.is_empty()
            && self.pending_var_writes.is_empty()
            && self.forced_vars.is_empty()
            && self.forced_io.is_empty()
    }

    /// Queue a one-shot I/O write, in arrival order.
    ///
    /// # Errors
    ///
    /// [`RequestQueueFull`] when the queue already holds [`REQUEST_QUEUE_CAPACITY`] writes; the
    /// refused write is not queued.
    pub fn try_enqueue_io_write(
        &mut self,
        address: IoAddress,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        if self.io_writes.len() >= REQUEST_QUEUE_CAPACITY {
            return Err(RequestQueueFull);
        }
        self.io_writes.push((address, value));
        Ok(())
    }

    /// Withdraw the most recently queued one-shot I/O write of `address` with `value`, when one
    /// is queued. A caller uses it when its own checked write failed after the request was
    /// queued; a write of the same address with another value is left alone.
    pub fn withdraw_io_write(&mut self, address: &IoAddress, value: &Value) {
        if let Some(index) = self
            .io_writes
            .iter()
            .rposition(|(queued, queued_value)| queued == address && queued_value == value)
        {
            self.io_writes.remove(index);
        }
    }

    /// Take the queued one-shot I/O writes, in arrival order, leaving the queue empty.
    #[must_use]
    pub fn drain_io_writes(&mut self) -> Vec<(IoAddress, Value)> {
        core::mem::take(&mut self.io_writes)
    }

    /// Queue a pending variable write, replacing the earlier write to the same target in place.
    ///
    /// # Errors
    ///
    /// [`RequestQueueFull`] when `target` is not queued yet and the queue already holds
    /// [`REQUEST_QUEUE_CAPACITY`] writes; replacing a queued target is never refused.
    pub fn try_upsert_var_write(
        &mut self,
        target: PendingVarTarget,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        let is_new = self
            .pending_var_writes
            .iter()
            .all(|entry| entry.target != target);
        if is_new && self.pending_var_writes.len() >= REQUEST_QUEUE_CAPACITY {
            return Err(RequestQueueFull);
        }
        self.upsert_var_write(target, value);
        Ok(())
    }

    /// Take the pending variable writes, leaving the queue empty.
    #[must_use]
    pub fn drain_var_writes(&mut self) -> Vec<PendingVarWrite> {
        core::mem::take(&mut self.pending_var_writes)
    }

    /// Clear every queued write and active force.
    pub fn clear(&mut self) {
        self.io_writes.clear();
        self.pending_var_writes.clear();
        self.forced_vars.clear();
        self.forced_io.clear();
    }

    /// The value of the active variable force on `target`, when the target is forced.
    #[must_use]
    pub fn forced_var_value(&self, target: &ForcedVarTarget) -> Option<Value> {
        self.forced_vars
            .iter()
            .find(|entry| entry.target == *target)
            .map(|entry| entry.value.clone())
    }

    /// Put back the variable force that a failed checked write had replaced: `previous` is the
    /// value forced before the failed request, or `None` when the target was not forced.
    pub fn restore_var_force(&mut self, target: &ForcedVarTarget, previous: Option<Value>) {
        match previous {
            Some(value) => self.upsert_forced_var(target.clone(), value),
            None => self.forced_vars.retain(|entry| entry.target != *target),
        }
    }

    /// The value of the active I/O force on `address`, when the address is forced.
    #[must_use]
    pub fn forced_io_value(&self, address: &IoAddress) -> Option<Value> {
        self.forced_io
            .iter()
            .find(|(current, _)| current == address)
            .map(|(_, value)| value.clone())
    }

    /// Put back the I/O force that a failed checked write had replaced: `previous` is the value
    /// forced before the failed request, or `None` when the address was not forced.
    pub fn restore_io_force(&mut self, address: &IoAddress, previous: Option<Value>) {
        match previous {
            Some(value) => self.upsert_forced_io(address.clone(), value),
            None => self.forced_io.retain(|(current, _)| current != address),
        }
    }

    /// Release the active I/O force on `address`, when one is set.
    pub fn release_io(&mut self, address: &IoAddress) {
        self.forced_io.retain(|(current, _)| current != address);
    }

    /// An owned copy of every active force, in the order the targets were first forced.
    #[must_use]
    pub fn forced_snapshot(&self) -> ForcedSnapshot {
        ForcedSnapshot {
            vars: self.forced_vars.clone(),
            io: self.forced_io.clone(),
        }
    }

    /// Remove the I/O force that still holds the rejected `value`, so the rejected value is not
    /// applied again. The entry goes only while it still holds that value: a newer force on the
    /// same address survives.
    pub fn discard_rejected_io_force(&mut self, address: &IoAddress, value: &Value) {
        self.forced_io
            .retain(|(current, current_value)| !(current == address && current_value == value));
    }

    /// Remove the variable force that still holds the rejected `value`. As for I/O forces, the
    /// entry goes only while it still holds that value.
    pub fn discard_rejected_var_force(&mut self, target: &ForcedVarTarget, value: &Value) {
        self.forced_vars
            .retain(|entry| !(entry.target == *target && entry.value == *value));
    }

    /// Set the active variable force on `target`, replacing an earlier force in place.
    ///
    /// # Errors
    ///
    /// [`RequestQueueFull`] when `target` is not forced yet and the queue already holds
    /// [`REQUEST_QUEUE_CAPACITY`] forces; re-forcing an existing target is never refused.
    pub fn try_upsert_forced_var(
        &mut self,
        target: ForcedVarTarget,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        let is_new = self.forced_vars.iter().all(|entry| entry.target != target);
        if is_new && self.forced_vars.len() >= REQUEST_QUEUE_CAPACITY {
            return Err(RequestQueueFull);
        }
        self.upsert_forced_var(target, value);
        Ok(())
    }

    /// Set the active I/O force on `address`, replacing an earlier force in place.
    ///
    /// # Errors
    ///
    /// [`RequestQueueFull`] when `address` is not forced yet and the queue already holds
    /// [`REQUEST_QUEUE_CAPACITY`] forces; re-forcing an existing address is never refused.
    pub fn try_upsert_forced_io(
        &mut self,
        address: IoAddress,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        let is_new = self
            .forced_io
            .iter()
            .all(|(current, _)| *current != address);
        if is_new && self.forced_io.len() >= REQUEST_QUEUE_CAPACITY {
            return Err(RequestQueueFull);
        }
        self.upsert_forced_io(address, value);
        Ok(())
    }

    /// Release every active variable force whose target matches `predicate`.
    pub fn clear_forced_vars(&mut self, predicate: impl Fn(&ForcedVarTarget) -> bool) {
        self.forced_vars.retain(|entry| !predicate(&entry.target));
    }

    /// Append the pending write to `target`, or replace the earlier write to it in place.
    fn upsert_var_write(&mut self, target: PendingVarTarget, value: Value) {
        if let Some(entry) = self
            .pending_var_writes
            .iter_mut()
            .find(|entry| entry.target == target)
        {
            entry.value = value;
        } else {
            self.pending_var_writes
                .push(PendingVarWrite { target, value });
        }
    }

    /// Add the active force on `target`, or replace the earlier value in place.
    fn upsert_forced_var(&mut self, target: ForcedVarTarget, value: Value) {
        if let Some(entry) = self
            .forced_vars
            .iter_mut()
            .find(|entry| entry.target == target)
        {
            entry.value = value;
        } else {
            self.forced_vars.push(ForcedVar { target, value });
        }
    }

    /// Add the active I/O force on `address`, or replace the earlier value in place.
    fn upsert_forced_io(&mut self, address: IoAddress, value: Value) {
        if let Some(entry) = self
            .forced_io
            .iter_mut()
            .find(|(current, _)| *current == address)
        {
            entry.1 = value;
        } else {
            self.forced_io.push((address, value));
        }
    }
}

#[cfg(test)]
mod tests;
