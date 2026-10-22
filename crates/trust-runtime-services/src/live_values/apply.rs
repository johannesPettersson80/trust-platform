//! Apply the queued Live Values writes and the active forces at the cooperative cycle boundaries
//! (specification 11, "Live Values"; specification 34 §4.3.1 and §8).
//!
//! A control surface queues writes and forces in [`super::queues`]; this module owns the order in
//! which a scan applies them and how a refusal is handled, so a chip and trust-runtime run the
//! same code. Applying admits every request without allocating: the write queues are drained in
//! place (their buffers keep their capacity) and a force is applied from the queue it lives in,
//! with no snapshot copy. A target's text is built only when a request is refused, and the
//! composing platform turns that refusal into its own report; a refused force is withdrawn so its
//! value is not applied again.
//!
//! The queued lvalue writes need the platform's HIR expression evaluation, which the core does not
//! carry, so [`cycle_start`] applies them through a caller-supplied writer. Everything else,
//! including retained-name resolution, is shared.

use alloc::format;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::{Generation, InstanceId};
use trust_runtime_core::value::{Duration, Value, ValueRef};
use trust_runtime_core::vm::CycleAccess;

use super::queues::{ForcedVarTarget, LiveValueQueues, PendingVarTarget, PendingVarWrite};
use super::retained_names::{self, RetainedNames};

/// Reports a Live Values request that a scan boundary refused. The platform turns it into its own
/// report; trust-runtime pushes its `WriteRejected` event with this target text and time.
pub trait WriteSink {
    /// The boundary refused `target` with `error` at logical `time`.
    fn refused(&mut self, target: &str, error: &RuntimeError, time: Duration);
}

/// Whether an error consuming the enclosing cycle's bounded resources must fault the cycle. An
/// operation's own destination error stays request-local and is only reported (specification 11,
/// "Live Values").
#[must_use]
pub fn is_cycle_operational_error(error: &RuntimeError) -> bool {
    matches!(
        error,
        RuntimeError::WatchdogTimeout
            | RuntimeError::ExecutionTimeout
            | RuntimeError::ExecutionLimit { .. }
    )
}

/// The destination text of a pending variable write, as the runtime reports it in its event.
#[must_use]
pub fn pending_var_label(target: &PendingVarTarget) -> SmolStr {
    match target {
        PendingVarTarget::Global(name) | PendingVarTarget::Retain(name) => name.clone(),
        PendingVarTarget::Instance(_, id, name) => SmolStr::new(format!("{}.{name}", id.0)),
        PendingVarTarget::Local(id, name) => SmolStr::new(format!("{}.{name}", id.0)),
    }
}

/// The destination text of a variable force, as the runtime reports it in its event.
#[must_use]
pub fn forced_var_label(target: &ForcedVarTarget) -> SmolStr {
    match target {
        ForcedVarTarget::Global(name) | ForcedVarTarget::Retain(name) => name.clone(),
        ForcedVarTarget::Instance(_, id, name) => SmolStr::new(format!("{}.{name}", id.0)),
    }
}

/// Apply the queued Live Values writes at the cycle start, before the scheduler runs a program, in
/// arrival order: the variable and reference writes, then the lvalue writes through `write_lvalue`.
///
/// A refused request is passed to `sink` and the drain continues; a deadline or resource error
/// faults the enclosing cycle and stops the drain.
///
/// # Errors
///
/// The deadline or resource error of a request that must fault the enclosing cycle.
pub fn cycle_start<T, S>(
    queues: &mut LiveValueQueues<T>,
    names: &RetainedNames,
    access: &mut CycleAccess<'_, '_>,
    sink: &mut S,
    mut write_lvalue: impl FnMut(&mut CycleAccess<'_, '_>, &T, Value) -> Result<(), RuntimeError>,
) -> Result<(), RuntimeError>
where
    T: core::fmt::Debug,
    S: WriteSink,
{
    for write in queues.pending_var_writes.drain(..) {
        let PendingVarWrite { target, value } = write;
        if let Err(error) = apply_pending_var(access, names, &target, value) {
            let operational = is_cycle_operational_error(&error);
            sink.refused(&pending_var_label(&target), &error, access.logical_time());
            if operational {
                return Err(error);
            }
        }
    }
    for write in queues.pending_lvalue_writes.drain(..) {
        let result = if write.frame_id.is_some() {
            Err(RuntimeError::ReferenceLifetime)
        } else {
            write_lvalue(access, &write.target, write.value)
        };
        if let Err(error) = result {
            let operational = is_cycle_operational_error(&error);
            sink.refused(
                &format!("{:?}", write.target),
                &error,
                access.logical_time(),
            );
            if operational {
                return Err(error);
            }
        }
    }
    Ok(())
}

/// Apply the queued one-shot I/O writes and then every active force before the input sample, as
/// the boundary does today.
///
/// # Errors
///
/// The deadline or resource error of a request that must fault the enclosing cycle.
pub fn io_writes_and_forces<T, S>(
    queues: &mut LiveValueQueues<T>,
    names: &RetainedNames,
    access: &mut CycleAccess<'_, '_>,
    sink: &mut S,
) -> Result<(), RuntimeError>
where
    S: WriteSink,
{
    for (address, value) in queues.io_writes.drain(..) {
        if let Err(error) = access.write_admitted_image(&address, value) {
            let operational = is_cycle_operational_error(&error);
            sink.refused(&format!("{address:?}"), &error, access.logical_time());
            if operational {
                return Err(error);
            }
        }
    }
    forces(queues, names, access, sink)
}

/// Apply every active force to its destination, in the order the targets were first forced. A
/// refused force is withdrawn from its queue so its value is not applied again.
///
/// # Errors
///
/// The deadline or resource error of a force that must fault the enclosing cycle.
pub fn forces<T, S>(
    queues: &mut LiveValueQueues<T>,
    names: &RetainedNames,
    access: &mut CycleAccess<'_, '_>,
    sink: &mut S,
) -> Result<(), RuntimeError>
where
    S: WriteSink,
{
    // Each queue holds at most one entry per target, so the refused entry is the one at `index`.
    let mut index = 0;
    while index < queues.forced_io.len() {
        let result = access.write_admitted_image(
            &queues.forced_io[index].0,
            queues.forced_io[index].1.clone(),
        );
        match result {
            Ok(()) => index += 1,
            Err(error) => {
                let label = format!("{:?}", queues.forced_io[index].0);
                let operational = is_cycle_operational_error(&error);
                if !operational {
                    queues.forced_io.remove(index);
                }
                sink.refused(&label, &error, access.logical_time());
                if operational {
                    return Err(error);
                }
            }
        }
    }
    let mut index = 0;
    while index < queues.forced_vars.len() {
        let result = apply_forced_var(
            access,
            names,
            &queues.forced_vars[index].target,
            queues.forced_vars[index].value.clone(),
        );
        match result {
            Ok(()) => index += 1,
            Err(error) => {
                let label = forced_var_label(&queues.forced_vars[index].target);
                let operational = is_cycle_operational_error(&error);
                if !operational {
                    queues.forced_vars.remove(index);
                }
                sink.refused(&label, &error, access.logical_time());
                if operational {
                    return Err(error);
                }
            }
        }
    }
    Ok(())
}

/// Refuse every queued write and every active force with `error` at logical `time`, each passed to
/// `sink`, and withdraw them: a program that takes none of them, such as a compiled program before
/// its symbol table serves them (specification 44 §13.3).
#[cfg(feature = "native")]
pub fn refuse_all<T, S>(
    queues: &mut LiveValueQueues<T>,
    error: &RuntimeError,
    time: Duration,
    sink: &mut S,
) where
    T: core::fmt::Debug,
    S: WriteSink,
{
    for write in queues.pending_var_writes.drain(..) {
        sink.refused(&pending_var_label(&write.target), error, time);
    }
    for write in queues.pending_lvalue_writes.drain(..) {
        sink.refused(&format!("{:?}", write.target), error, time);
    }
    for (address, _) in queues.io_writes.drain(..) {
        sink.refused(&format!("{address:?}"), error, time);
    }
    for (address, _) in queues.forced_io.drain(..) {
        sink.refused(&format!("{address:?}"), error, time);
    }
    for force in queues.forced_vars.drain(..) {
        sink.refused(&forced_var_label(&force.target), error, time);
    }
}

fn apply_pending_var(
    access: &mut CycleAccess<'_, '_>,
    names: &RetainedNames,
    target: &PendingVarTarget,
    value: Value,
) -> Result<(), RuntimeError> {
    match target {
        PendingVarTarget::Global(name) => access.write_admitted_global(name, value),
        PendingVarTarget::Retain(name) => write_retained(access, names, name, value),
        PendingVarTarget::Instance(generation, id, name) => {
            instance_reference(access, *generation, *id, name)
                .and_then(|reference| access.write_admitted_reference(reference.as_view(), value))
        }
        // A local of a paused frame is not reachable from the scan; the queued write is refused.
        PendingVarTarget::Local(_, _) => Err(RuntimeError::ReferenceLifetime),
    }
}

fn apply_forced_var(
    access: &mut CycleAccess<'_, '_>,
    names: &RetainedNames,
    target: &ForcedVarTarget,
    value: Value,
) -> Result<(), RuntimeError> {
    match target {
        ForcedVarTarget::Global(name) => access.write_admitted_global(name, value),
        ForcedVarTarget::Retain(name) => write_retained(access, names, name, value),
        ForcedVarTarget::Instance(generation, id, name) => {
            instance_reference(access, *generation, *id, name)
                .and_then(|reference| access.write_admitted_reference(reference.as_view(), value))
        }
    }
}

/// Resolve a declared retained label and write the literal-admitted value to it. The label lookup
/// is bookkeeping under TIME-08, so it is charged before it runs.
fn write_retained(
    access: &mut CycleAccess<'_, '_>,
    names: &RetainedNames,
    name: &str,
    value: Value,
) -> Result<(), RuntimeError> {
    access
        .charge_management_name(name)
        .and_then(|()| {
            retained_names::retained_reference(names, &*access, name)
                .ok_or(RuntimeError::ReferenceLifetime)
        })
        .and_then(|reference| access.write_admitted_reference(reference.as_view(), value))
}

/// Resolve a staged request that names an instance by identity. The identity belongs to the
/// generation the request was resolved in; another installed generation rejects it, so a restart
/// or replacement in between cannot redirect it to the instance that now has that identity
/// (specification 34 §4.3.1).
fn instance_reference(
    access: &CycleAccess<'_, '_>,
    generation: Generation,
    id: InstanceId,
    name: &str,
) -> Result<ValueRef, RuntimeError> {
    if generation != access.generation() {
        return Err(RuntimeError::ReferenceLifetime);
    }
    access
        .ref_for_instance_recursive(id, name)
        .ok_or(RuntimeError::ReferenceLifetime)
}
