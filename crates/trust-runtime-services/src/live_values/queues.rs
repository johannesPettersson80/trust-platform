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
use trust_runtime_core::trace::ChangeSource;
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
    /// Where the write came from, which a cycle trace records (specification 48 Appendix C, C.6).
    pub source: ChangeSource,
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
    /// Where each write of `io_writes` came from, at the same index, which a cycle trace records
    /// (specification 48 Appendix C, C.6); a write without one is the control link's.
    pub io_write_sources: Vec<ChangeSource>,
    /// Pending variable writes; a write to a target already here replaces it in place.
    pub pending_var_writes: Vec<PendingVarWrite>,
    /// Active variable forces; a force to a target already here replaces it in place.
    pub forced_vars: Vec<ForcedVar>,
    /// Active I/O forces; a force to an address already here replaces it in place.
    pub forced_io: Vec<(IoAddress, Value)>,
    /// The forces set or released since the last cycle start, in the order they were, which the
    /// cycle start reports once (specification 48 RU-59); at most [`FORCE_CHANGES_CAPACITY`].
    pub force_changes: Vec<ForceChange>,
    /// Whether force changes past [`FORCE_CHANGES_CAPACITY`] were not kept since the last cycle
    /// start.
    pub force_changes_lost: bool,
}

/// The most force changes kept between two cycle starts: as many as forces can be active.
pub const FORCE_CHANGES_CAPACITY: usize = 2 * REQUEST_QUEUE_CAPACITY;

/// A force set or released, which a cycle trace records once, before the cycle it takes effect in
/// (specification 48 RU-59).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForceChange {
    /// A variable force set, or set to another value.
    VariableSet(ForcedVarTarget),
    /// A variable force released.
    VariableReleased(ForcedVarTarget),
    /// An address force set, or set to another value.
    AddressSet(IoAddress),
    /// An address force released.
    AddressReleased(IoAddress),
}

impl ForceChange {
    /// Whether `self` and `other` concern the same variable or the same address.
    #[must_use]
    pub fn same_target(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::VariableSet(left) | Self::VariableReleased(left),
                Self::VariableSet(right) | Self::VariableReleased(right),
            ) => left == right,
            (
                Self::AddressSet(left) | Self::AddressReleased(left),
                Self::AddressSet(right) | Self::AddressReleased(right),
            ) => left == right,
            _ => false,
        }
    }
}

/// Keeps `change` in `changes`, or notes it lost past [`FORCE_CHANGES_CAPACITY`].
fn note_force_change(changes: &mut Vec<ForceChange>, lost: &mut bool, change: ForceChange) {
    if changes.len() < FORCE_CHANGES_CAPACITY {
        changes.push(change);
    } else {
        *lost = true;
    }
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
            && self.force_changes.is_empty()
            && !self.force_changes_lost
    }

    /// Keeps the force change `change` until the next cycle start reports it.
    fn note_force(&mut self, change: ForceChange) {
        note_force_change(
            &mut self.force_changes,
            &mut self.force_changes_lost,
            change,
        );
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
        self.try_enqueue_io_write_from(address, value, ChangeSource::ControlLink)
    }

    /// Queue a one-shot I/O write that comes from `source` rather than the control link, such as
    /// Control Studio's, in arrival order: a cycle trace records it with its source (specification
    /// 48 Appendix C, C.6).
    ///
    /// # Errors
    ///
    /// [`RequestQueueFull`] when the queue already holds [`REQUEST_QUEUE_CAPACITY`] writes; the
    /// refused write is not queued.
    pub fn try_enqueue_io_write_from(
        &mut self,
        address: IoAddress,
        value: Value,
        source: ChangeSource,
    ) -> Result<(), RequestQueueFull> {
        if self.io_writes.len() >= REQUEST_QUEUE_CAPACITY {
            return Err(RequestQueueFull);
        }
        // The writes queued without a source, directly into `io_writes`, are the control link's.
        self.io_write_sources
            .resize(self.io_writes.len(), ChangeSource::ControlLink);
        self.io_writes.push((address, value));
        self.io_write_sources.push(source);
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
            if index < self.io_write_sources.len() {
                self.io_write_sources.remove(index);
            }
        }
    }

    /// Take the queued one-shot I/O writes, in arrival order, leaving the queue empty.
    #[must_use]
    pub fn drain_io_writes(&mut self) -> Vec<(IoAddress, Value)> {
        self.io_write_sources.clear();
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
        self.try_upsert_var_write_from(target, value, ChangeSource::ControlLink)
    }

    /// [`Self::try_upsert_var_write`] for a write that comes from `source` rather than the control
    /// link: a protocol server's, a screen's or Control Studio's.
    ///
    /// # Errors
    ///
    /// As [`Self::try_upsert_var_write`].
    pub fn try_upsert_var_write_from(
        &mut self,
        target: PendingVarTarget,
        value: Value,
        source: ChangeSource,
    ) -> Result<(), RequestQueueFull> {
        let is_new = self
            .pending_var_writes
            .iter()
            .all(|entry| entry.target != target);
        if is_new && self.pending_var_writes.len() >= REQUEST_QUEUE_CAPACITY {
            return Err(RequestQueueFull);
        }
        self.upsert_var_write(target, value, source);
        Ok(())
    }

    /// Queue the pending variable writes `writes` from `source` together, each replacing the earlier
    /// write to its target in place, or none of them (specification 11 §6.8.1, "Writing variables").
    ///
    /// # Errors
    ///
    /// [`RequestQueueFull`] when the targets that are not queued yet would take the queue past
    /// [`REQUEST_QUEUE_CAPACITY`] writes; nothing is queued then.
    pub fn try_upsert_var_writes(
        &mut self,
        writes: Vec<(PendingVarTarget, Value)>,
        source: ChangeSource,
    ) -> Result<(), RequestQueueFull> {
        let mut new: Vec<&PendingVarTarget> = Vec::new();
        for (target, _) in &writes {
            let queued = self
                .pending_var_writes
                .iter()
                .any(|entry| entry.target == *target);
            if !queued && !new.contains(&target) {
                new.push(target);
            }
        }
        if self.pending_var_writes.len() + new.len() > REQUEST_QUEUE_CAPACITY {
            return Err(RequestQueueFull);
        }
        for (target, value) in writes {
            self.upsert_var_write(target, value, source);
        }
        Ok(())
    }

    /// Take the pending variable writes, leaving the queue empty.
    #[must_use]
    pub fn drain_var_writes(&mut self) -> Vec<PendingVarWrite> {
        core::mem::take(&mut self.pending_var_writes)
    }

    /// Clear every queued write and active force, and the force changes not yet reported: a clear
    /// comes at a boundary where a recording has ended.
    pub fn clear(&mut self) {
        self.io_writes.clear();
        self.io_write_sources.clear();
        self.pending_var_writes.clear();
        self.forced_vars.clear();
        self.forced_io.clear();
        self.force_changes.clear();
        self.force_changes_lost = false;
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
        if let Some(value) = previous {
            self.upsert_forced_var(target.clone(), value);
            self.note_force(ForceChange::VariableSet(target.clone()));
        } else {
            self.forced_vars.retain(|entry| entry.target != *target);
            self.note_force(ForceChange::VariableReleased(target.clone()));
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
        if let Some(value) = previous {
            self.upsert_forced_io(address.clone(), value);
            self.note_force(ForceChange::AddressSet(address.clone()));
        } else {
            self.forced_io.retain(|(current, _)| current != address);
            self.note_force(ForceChange::AddressReleased(address.clone()));
        }
    }

    /// Release the active I/O force on `address`, when one is set.
    pub fn release_io(&mut self, address: &IoAddress) {
        let before = self.forced_io.len();
        self.forced_io.retain(|(current, _)| current != address);
        if self.forced_io.len() != before {
            self.note_force(ForceChange::AddressReleased(address.clone()));
        }
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
        let before = self.forced_io.len();
        self.forced_io
            .retain(|(current, current_value)| !(current == address && current_value == value));
        if self.forced_io.len() != before {
            self.note_force(ForceChange::AddressReleased(address.clone()));
        }
    }

    /// Remove the variable force that still holds the rejected `value`. As for I/O forces, the
    /// entry goes only while it still holds that value.
    pub fn discard_rejected_var_force(&mut self, target: &ForcedVarTarget, value: &Value) {
        let before = self.forced_vars.len();
        self.forced_vars
            .retain(|entry| !(entry.target == *target && entry.value == *value));
        if self.forced_vars.len() != before {
            self.note_force(ForceChange::VariableReleased(target.clone()));
        }
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
        self.note_force(ForceChange::VariableSet(target.clone()));
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
        self.note_force(ForceChange::AddressSet(address.clone()));
        self.upsert_forced_io(address, value);
        Ok(())
    }

    /// Release every active variable force whose target matches `predicate`.
    pub fn clear_forced_vars(&mut self, predicate: impl Fn(&ForcedVarTarget) -> bool) {
        let (changes, lost) = (&mut self.force_changes, &mut self.force_changes_lost);
        self.forced_vars.retain(|entry| {
            let released = predicate(&entry.target);
            if released {
                note_force_change(
                    changes,
                    lost,
                    ForceChange::VariableReleased(entry.target.clone()),
                );
            }
            !released
        });
    }

    /// Append the pending write to `target`, or replace the earlier write to it in place.
    fn upsert_var_write(&mut self, target: PendingVarTarget, value: Value, source: ChangeSource) {
        if let Some(entry) = self
            .pending_var_writes
            .iter_mut()
            .find(|entry| entry.target == target)
        {
            entry.value = value;
            entry.source = source;
        } else {
            self.pending_var_writes.push(PendingVarWrite {
                target,
                value,
                source,
            });
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
