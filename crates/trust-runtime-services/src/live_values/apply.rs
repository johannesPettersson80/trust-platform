//! Apply the queued Live Values writes and the active forces at the cooperative cycle boundaries
//! (specification 11, "Live Values"; specification 34 §4.3.1 and §8; specification 44 §13.3).
//!
//! A control surface queues writes and forces in [`super::queues`]; this module owns the order in
//! which a scan applies them and how a refusal is handled, so a chip and trust-runtime run the
//! same code. Applying admits every request without allocating: the write queues are drained in
//! place (their buffers keep their capacity) and a force is applied from the queue it lives in,
//! with no snapshot copy. A target's text is built only when a request is refused, and the
//! composing platform turns that refusal into its own report; a refused force is withdrawn so its
//! value is not applied again.

use alloc::format;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::memory::IoArea;
use trust_runtime_core::memory::{Generation, InstanceId};
use trust_runtime_core::native::variables::CompiledVariables;
use trust_runtime_core::value::{Duration, Value};

use super::queues::{
    ForceChange, ForcedVarTarget, LiveValueQueues, PendingVarTarget, PendingVarWrite,
};

/// Reports a Live Values request that a scan boundary refused. The platform turns it into its own
/// report; trust-runtime pushes its `WriteRejected` event with this target text and time.
pub trait WriteSink {
    /// The boundary refused `target` with `error` at logical `time`.
    fn refused(&mut self, target: &str, error: &RuntimeError, time: Duration);

    /// The boundary applied the queued write of `target` to `variables`: a cycle trace records it
    /// (specification 48 RU-59). Forces, which the boundary applies again every cycle, are not
    /// reported here. The default records nothing, and nothing is worked out for it: the target
    /// itself is passed, so that a sink that records nothing allocates nothing.
    fn written(&mut self, target: &PendingVarTarget, variables: &dyn CompiledVariables) {
        let _ = (target, variables);
    }

    /// Whether the sink records what the boundary applies, so that a value it reports is worked
    /// out: a written address's value is copied only for a sink that records. The default records
    /// nothing.
    fn records(&self) -> bool {
        false
    }

    /// The boundary applied the queued one-shot write of `value` at `address` (`io.write`): a cycle
    /// trace records it (specification 48 RU-59). Reported only to a sink that [`Self::records`].
    fn address_written(
        &mut self,
        address: &IoAddress,
        value: &Value,
        variables: &dyn CompiledVariables,
    ) {
        let _ = (address, value, variables);
    }

    /// A force set or released since the last cycle start, reported once, as the cycle start
    /// leaves it in effect (specification 48 RU-59). The default records nothing.
    fn forced(&mut self, effect: ForceEffect<'_>, variables: &dyn CompiledVariables) {
        let _ = (effect, variables);
    }

    /// Force changes were lost since the last cycle start: more came than were kept. The default
    /// records nothing.
    fn force_changes_lost(&mut self) {}
}

/// A force set or released, as a cycle start reports it with the value in effect (specification 48
/// RU-59).
#[derive(Debug, Clone, Copy)]
pub enum ForceEffect<'a> {
    /// A variable force in effect.
    VariableSet(&'a ForcedVarTarget),
    /// A variable force released.
    VariableReleased(&'a ForcedVarTarget),
    /// An address force in effect, with its value.
    AddressSet(&'a IoAddress, &'a Value),
    /// An address force released.
    AddressReleased(&'a IoAddress),
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

/// The queued variable writes and the active variable forces of a compiled program at its cycle
/// start, before the program runs (specification 44 §13.3): the writes in arrival order through the
/// program's symbol table, then every force as [`compiled_forces`] applies it. A target the program
/// does not hold, or an instance identity of another generation than `generation`, is refused.
///
/// # Errors
///
/// The deadline or resource error of a request that must fault the enclosing cycle.
pub fn compiled_cycle_start<S>(
    queues: &mut LiveValueQueues,
    generation: Generation,
    variables: &mut dyn CompiledVariables,
    time: Duration,
    sink: &mut S,
) -> Result<(), RuntimeError>
where
    S: WriteSink,
{
    for write in queues.pending_var_writes.drain(..) {
        let PendingVarWrite { target, value } = write;
        let result = match &target {
            PendingVarTarget::Global(name) => variables.write_named(name, &value),
            PendingVarTarget::Retain(label) => compiled_retained(variables, label, &value),
            PendingVarTarget::Instance(at, id, name) => {
                compiled_member(variables, generation, *at, *id, name, &value)
            }
            // A local of a paused frame is not reachable from the scan; the queued write is refused.
            PendingVarTarget::Local(_, _) => Err(RuntimeError::ReferenceLifetime),
        };
        match result {
            Ok(()) => sink.written(&target, &*variables),
            Err(error) => {
                let operational = is_cycle_operational_error(&error);
                sink.refused(&pending_var_label(&target), &error, time);
                if operational {
                    return Err(error);
                }
            }
        }
    }
    compiled_forces(queues, generation, variables, time, sink)?;
    report_force_changes(queues, &*variables, sink);
    Ok(())
}

/// Reports the forces set or released since the last cycle start once each, as the cycle start
/// leaves them (specification 48 RU-59): for each variable and address its last change, a set when
/// its force is in effect and a release when it is not, so that a force the program refused, or one
/// set and released between two cycles, is not reported as set. The changes are emptied whether or
/// not the sink records them.
fn report_force_changes<S>(
    queues: &mut LiveValueQueues,
    variables: &dyn CompiledVariables,
    sink: &mut S,
) where
    S: WriteSink,
{
    if core::mem::take(&mut queues.force_changes_lost) {
        sink.force_changes_lost();
    }
    let changes = &queues.force_changes;
    for (index, change) in changes.iter().enumerate() {
        if changes[index + 1..]
            .iter()
            .any(|later| later.same_target(change))
        {
            continue;
        }
        let variable_forced =
            |target: &ForcedVarTarget| queues.forced_vars.iter().any(|f| f.target == *target);
        let address_force = |address: &IoAddress| {
            queues
                .forced_io
                .iter()
                .find(|(forced, _)| forced == address)
        };
        let effect = match change {
            ForceChange::VariableSet(target) => {
                variable_forced(target).then_some(ForceEffect::VariableSet(target))
            }
            ForceChange::VariableReleased(target) => {
                (!variable_forced(target)).then_some(ForceEffect::VariableReleased(target))
            }
            ForceChange::AddressSet(address) => {
                address_force(address).map(|(forced, value)| ForceEffect::AddressSet(forced, value))
            }
            ForceChange::AddressReleased(address) => address_force(address)
                .is_none()
                .then_some(ForceEffect::AddressReleased(address)),
        };
        if let Some(effect) = effect {
            sink.forced(effect, variables);
        }
    }
    queues.retire_force_changes();
}

/// The active variable forces of a compiled program, in the order the targets were first forced,
/// through the program's symbol table: at the cycle start, after the queued writes, and again after
/// the scan's last task, before the outputs are sampled, so that a forced variable holds its value
/// between scans and a forced output variable reaches its output whatever the program wrote
/// (specification 10 §4.7, steps 2 and 6). A target the program does not hold, or an instance
/// identity of another generation than `generation`, is refused, and the refused force is withdrawn.
///
/// # Errors
///
/// The deadline or resource error of a request that must fault the enclosing cycle.
pub fn compiled_forces<S>(
    queues: &mut LiveValueQueues,
    generation: Generation,
    variables: &mut dyn CompiledVariables,
    time: Duration,
    sink: &mut S,
) -> Result<(), RuntimeError>
where
    S: WriteSink,
{
    // Each queue holds at most one entry per target, so the refused entry is the one at `index`.
    let mut index = 0;
    while index < queues.forced_vars.len() {
        let force = &queues.forced_vars[index];
        let result = match &force.target {
            ForcedVarTarget::Global(name) => variables.write_named(name, &force.value),
            ForcedVarTarget::Retain(label) => compiled_retained(variables, label, &force.value),
            ForcedVarTarget::Instance(at, id, name) => {
                compiled_member(variables, generation, *at, *id, name, &force.value)
            }
        };
        match result {
            Ok(()) => index += 1,
            Err(error) => {
                let label = forced_var_label(&queues.forced_vars[index].target);
                let operational = is_cycle_operational_error(&error);
                if !operational {
                    queues.forced_vars.remove(index);
                }
                sink.refused(&label, &error, time);
                if operational {
                    return Err(error);
                }
            }
        }
    }
    Ok(())
}

/// The queued one-shot process-image writes and the active forces of the images `areas` of a
/// compiled program (specification 44 §13.3): the writes, in arrival order, of an input or marker
/// address after the drivers read the inputs and before the program samples them, then every
/// variable force again, and the forces of `areas` last; the marker and output images' forces
/// again after the scan, with the variable forces, before the outputs reach the drivers. A forced
/// variable that the program binds to an input or a marker address is written at that address too
/// (`CompiledVariables` holds a write there), so that after the drivers' input read it holds over
/// the physical input and over a one-shot write of its address, as its own force holds a variable,
/// and a force of the address holds over it. A refused force is withdrawn.
///
/// # Errors
///
/// The deadline or resource error of a request that must fault the enclosing cycle.
pub fn compiled_images<S>(
    queues: &mut LiveValueQueues,
    generation: Generation,
    areas: &[IoArea],
    variables: &mut dyn CompiledVariables,
    time: Duration,
    sink: &mut S,
) -> Result<(), RuntimeError>
where
    S: WriteSink,
{
    if !areas.contains(&IoArea::Output) {
        for (address, value) in queues.io_writes.drain(..) {
            let reported = sink.records().then(|| value.clone());
            match variables.write_image(&address, value) {
                Ok(()) => {
                    if let Some(value) = &reported {
                        sink.address_written(&address, value, &*variables);
                    }
                }
                Err(error) => {
                    let operational = is_cycle_operational_error(&error);
                    sink.refused(&format!("{address:?}"), &error, time);
                    if operational {
                        return Err(error);
                    }
                }
            }
        }
    }
    if areas.contains(&IoArea::Input) {
        compiled_forces(queues, generation, variables, time, sink)?;
    }
    let mut index = 0;
    while index < queues.forced_io.len() {
        if !areas.contains(&queues.forced_io[index].0.area) {
            index += 1;
            continue;
        }
        let result = variables.write_image(
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
                sink.refused(&label, &error, time);
                if operational {
                    return Err(error);
                }
            }
        }
    }
    Ok(())
}

/// Writes `value` to the compiled program's retained variable `label`; a key the retain map does
/// not hold is refused as the engine refuses a retained label it does not resolve.
fn compiled_retained(
    variables: &mut dyn CompiledVariables,
    label: &str,
    value: &Value,
) -> Result<(), RuntimeError> {
    match variables.write_retained(label, value) {
        Err(RuntimeError::UndefinedVariable(_)) => Err(RuntimeError::ReferenceLifetime),
        result => result,
    }
}

/// Writes `value` to member `name` of the compiled program's instance `id`, an identity resolved
/// in generation `at`; another installed generation, or an instance or member the program does not
/// hold, is refused as the engine refuses it (specification 34 §4.3.1).
fn compiled_member(
    variables: &mut dyn CompiledVariables,
    generation: Generation,
    at: Generation,
    id: InstanceId,
    name: &str,
    value: &Value,
) -> Result<(), RuntimeError> {
    if at != generation {
        return Err(RuntimeError::ReferenceLifetime);
    }
    match variables.write_member(id.0, name, value) {
        Err(RuntimeError::UndefinedVariable(_) | RuntimeError::UndefinedField(_)) => {
            Err(RuntimeError::ReferenceLifetime)
        }
        result => result,
    }
}

#[cfg(test)]
mod compiled_tests;
