//! The `Control.PID` adapter of the parameter sets (protocol annex E7.7): a `control.pid.*` write is
//! a parameter set over the fields of the controller's `ParametersValue`, committed at the start of
//! the task that runs the controller's program and confirmed after that task (E7.4), with the checks
//! of E7.3 to E7.5 and E10, the mode the controller observed in its last `Update`, and the operating
//! domain of its record (specification 45 §2.4).
use super::*;
use crate::memory::InstanceId;
use trust_runtime_core::native::image::{NONE, SYMBOL_INSTANCE, SYMBOL_PROGRAM};

/// The `iec_type` of a set's enumeration member, which only this adapter builds: a client's
/// binding names REAL and LREAL members only (E7.8).
pub(super) const ENUMERATION: &str = "ENUM";

/// The mode `Control.PID`'s last `Update` ran in, and whether an `Update` has run since the start
/// (specification 45, truST observation extension).
const MODE_VALUE: &str = "ModeValue";
const MODE_OBSERVED: &str = "ModeObserved";

/// A controller write that a pending set carries: the controller, the record the write sets, and
/// the value each field of it takes.
#[derive(Clone, Debug)]
pub(super) struct Set {
    instance: String,
    record: Value,
    writes: BTreeMap<String, Value>,
}

/// How a controller write ended (E7.4, E7.7, A6.7).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ControlPidOutcome {
    /// Its task ran with the record and left every field as written.
    Applied,
    /// It was refused at its task's boundary, stopped by a reload or restart, or the program changed
    /// a field it wrote; the text says which.
    Rejected(String),
}

/// The outcome of each controller's last write, by the instance path that named it, with the
/// write's identity; and the identities given so far.
#[derive(Debug, Default)]
pub(super) struct Writes {
    outcomes: HashMap<String, (String, ControlPidOutcome)>,
    given: u64,
}

/// A `control.pid.apply` or `control.pid.rollback` that the controller service checked (E7.7): the
/// controller's instance path, its record as the write found it, the record it writes, whether it
/// rolls back, and the runtime's target fact with the engineer's approval (E10).
pub(crate) struct ControlPidWrite<'a> {
    pub(crate) instance: &'a str,
    pub(crate) current: &'a Value,
    pub(crate) candidate: &'a Value,
    pub(crate) rollback: bool,
    pub(crate) physical_target: bool,
    pub(crate) physical_approved: bool,
}

/// A controller as the last scan left it, with the outcome of its last write and the runtime epoch,
/// read together (E7.2, E7.7).
pub(crate) struct ControlPidView {
    /// The controller instance's symbol in the program image.
    pub(crate) instance_id: InstanceId,
    /// Its `ParametersValue`.
    pub(crate) record: Value,
    /// The identity and the outcome of its last write that ended.
    pub(crate) outcome: Option<(String, ControlPidOutcome)>,
    /// The runtime epoch the program runs in; a reload or a restart ends it (A2.2).
    pub(crate) epoch: u64,
}

impl Writes {
    /// The identity of a new controller write.
    fn next_identity(&mut self) -> String {
        self.given += 1;
        format!("control.pid-{}", self.given)
    }

    /// Records how the write `transaction` of the set's controller ended.
    pub(super) fn record(&mut self, set: &Set, transaction: String, outcome: ControlPidOutcome) {
        self.outcomes
            .insert(set.instance.clone(), (transaction, outcome));
    }
}

impl Set {
    /// The controller's instance path, which a refusal names (E9.1).
    pub(super) fn instance(&self) -> &str {
        &self.instance
    }

    /// The checks at the task's boundary (E7.4): the controller's program is still run by the task
    /// the write waits for, and each field is still a writable, unaddressed member.
    pub(super) fn check_binding(&self, binding: &Binding, image: &Image<'_>) -> Result<(), String> {
        let task = owning_task(image, &self.instance)?;
        if !task.eq_ignore_ascii_case(&binding.task_name) {
            return Err(format!(
                "the controller {} is run by {task}, not by {}",
                self.instance, binding.task_name
            ));
        }
        writable(image, binding)
    }

    /// Manual is observed now (E7.3 step 7), from the controller itself, `task` being its task and
    /// `task_ran` whether that task ran in the current program generation.
    pub(super) fn check_mode(
        &self,
        program: &dyn ProgramRead,
        task: &str,
        task_ran: bool,
    ) -> Result<(), String> {
        controller_manual(program, &self.instance, task, task_ran)
    }

    /// The record lies in the controller's operating domain (specification 45 §2.4).
    pub(super) fn validate(&self) -> Result<(), String> {
        validate_record(&self.record)
    }

    /// The value the field `symbol` takes, in its own type.
    pub(super) fn value(&self, symbol: &str) -> Result<Value, String> {
        self.writes
            .get(symbol)
            .cloned()
            .ok_or_else(|| format!("{symbol} is not a field of the controller's record"))
    }
}

/// The task whose boundary commits a write to the controller at `instance` (E2.3, E7.1): the task
/// that runs the program instance the path starts with, or `$background` for a program outside
/// every task. A controller outside every program instance, a global one, has no owning task.
fn owning_task(image: &Image<'_>, instance: &str) -> Result<String, String> {
    let root = instance.split('.').next().unwrap_or_default();
    if !program_facts::is_program_instance(image, root) {
        return Err(format!(
            "the controller {instance} is not inside a program instance, so no task owns its record"
        ));
    }
    let owners: Vec<_> = program_facts::program_tasks(image)
        .into_iter()
        .filter(|task| {
            task.programs
                .iter()
                .any(|program| program.eq_ignore_ascii_case(root))
        })
        .collect();
    match owners.as_slice() {
        [] => Ok("$background".into()),
        [task] => Ok(task.name.to_string()),
        _ => Err(format!(
            "the program {root} of the controller {instance} runs in more than one task"
        )),
    }
}

/// Each member of `binding` is a writable, unaddressed, non-aliased member (E2.3, E7.1).
fn writable(image: &Image<'_>, binding: &Binding) -> Result<(), String> {
    match binding
        .parameters
        .iter()
        .find(|parameter| !program_facts::parameter_writable(image, &parameter.symbol))
    {
        Some(parameter) => Err(format!(
            "{} must be a writable, unaddressed, non-aliased member",
            parameter.symbol
        )),
        None => Ok(()),
    }
}

/// The fields of the controller's record as the members of a set, `<instance>.ParametersValue.<field>`,
/// each with the number it compares as and its value: an LREAL field as it is, an enumeration field
/// as its variant's number. Another field type is refused (E7.8).
fn members(instance: &str, record: &Value) -> Result<Vec<(Parameter, f64, Value)>, String> {
    let Value::Struct(record) = record else {
        return Err("Control.PID ParametersValue is not a structure".into());
    };
    record
        .fields()
        .iter()
        .map(|(name, value)| {
            let (iec_type, number) = match value {
                Value::LReal(number) => ("LREAL", *number),
                Value::Enum(variant) => (ENUMERATION, variant.numeric_value() as f64),
                _ => return Err(format!("unsupported Control.PID parameter '{name}'")),
            };
            let parameter = Parameter {
                symbol: format!("{instance}.ParametersValue.{name}"),
                iec_type: iec_type.into(),
                unit: String::new(),
                convention: "Control.PID".into(),
                minimum: f64::MIN,
                maximum: f64::MAX,
            };
            Ok((parameter, number, value.clone()))
        })
        .collect()
}

/// The operating domain of a `Control.PID` record (specification 45 §2.4), each number finite
/// (E7.7): `OutputMinimum <= OutputMaximum`, `RateMinimum <= 0 <= RateMaximum`, and
/// `TrackingTime > 0` when `IntegralGain <> 0`.
fn validate_record(record: &Value) -> Result<(), String> {
    let Value::Struct(record) = record else {
        return Err("Control.PID ParametersValue is not a structure".into());
    };
    if let Some((name, _)) = record
        .fields()
        .iter()
        .find(|(_, value)| matches!(value, Value::LReal(number) if !number.is_finite()))
    {
        return Err(format!("parameter '{name}' must be finite"));
    }
    let number = |name: &str| match record.field(name) {
        Some(Value::LReal(number)) => Ok(*number),
        _ => Err(format!("the Control.PID record has no LREAL field {name}")),
    };
    let (minimum, maximum) = (number("OutputMinimum")?, number("OutputMaximum")?);
    if minimum > maximum {
        return Err(format!(
            "OutputMinimum {minimum} exceeds OutputMaximum {maximum}; specification 45 §2.4 requires OutputMinimum <= OutputMaximum"
        ));
    }
    let (falling, rising) = (number("RateMinimum")?, number("RateMaximum")?);
    if falling > 0.0 || rising < 0.0 {
        return Err(format!(
            "RateMinimum {falling} and RateMaximum {rising} leave the domain; specification 45 §2.4 requires RateMinimum <= 0 <= RateMaximum"
        ));
    }
    let (integral, tracking) = (number("IntegralGain")?, number("TrackingTime")?);
    if integral != 0.0 && tracking <= 0.0 {
        return Err(format!(
            "TrackingTime {tracking} with IntegralGain {integral}; specification 45 §2.4 requires TrackingTime > 0 when IntegralGain <> 0"
        ));
    }
    Ok(())
}

/// Manual is observed now (E7.3 step 7, E7.7): the controller's task `task` ran in the current
/// program generation (`task_ran`), so the controller's record of its mode is this program's, since
/// an online change keeps every member (specification 44 §12.2); the controller ran an `Update`
/// since the start (`ModeObserved`), and its last one ran in Manual (`ModeValue`).
fn controller_manual(
    program: &dyn ProgramRead,
    instance: &str,
    task: &str,
    task_ran: bool,
) -> Result<(), String> {
    if !task_ran {
        return Err(format!(
            "confirmed Manual mode required: the controller {instance} has not run in Manual mode; its task {task} has not run since the program was loaded or restarted"
        ));
    }
    let observed = program.value(&format!("{instance}.{MODE_OBSERVED}"));
    let mode = program.value(&format!("{instance}.{MODE_VALUE}"));
    match (observed, mode) {
        (Some(Value::Bool(true)), Some(Value::Enum(mode)))
            if mode.variant_name().eq_ignore_ascii_case("Manual") =>
        {
            Ok(())
        }
        (Some(Value::Bool(true)), Some(Value::Enum(mode))) => Err(format!(
            "confirmed Manual mode required: the controller {instance} has not run in Manual mode; its last Update ran in {}",
            mode.variant_name()
        )),
        (Some(Value::Bool(false)), Some(Value::Enum(_))) => Err(format!(
            "confirmed Manual mode required: the controller {instance} has not run in Manual mode; it has not run an Update since the start"
        )),
        _ => Err(format!(
            "confirmed Manual mode required: the controller {instance} records no mode ({MODE_VALUE} and {MODE_OBSERVED})"
        )),
    }
}

/// The symbol of the controller instance `path` names (`Main.Controller`) in the program of
/// `image`: a program instance or a global instance, then an instance member at each step
/// (specification 44 §13.3).
fn controller_instance(image: &Image<'_>, path: &str) -> Result<InstanceId, String> {
    let mut parts = path.split('.');
    let root = parts
        .next()
        .filter(|value| !value.is_empty())
        .ok_or("instance path is empty")?;
    let mut canonical = root.to_ascii_uppercase();
    let (mut instance, symbol) = image
        .find_symbol(&canonical)
        .filter(|(_, symbol)| symbol.parent == NONE)
        .ok_or("program instance was not found")?;
    if symbol.flags & (SYMBOL_PROGRAM | SYMBOL_INSTANCE) == 0 {
        return Err("program binding is not an instance".into());
    }
    for part in parts {
        canonical.push('.');
        canonical.push_str(&part.to_ascii_uppercase());
        let (next, symbol) = image
            .find_symbol(&canonical)
            .ok_or("controller instance was not found")?;
        if symbol.flags & SYMBOL_INSTANCE == 0 {
            return Err("controller path does not resolve to an instance".into());
        }
        instance = next;
    }
    Ok(InstanceId(instance))
}

impl DebugControl {
    /// The controller at `instance` in the program as the last scan left it: its identity, its
    /// record, how its last write ended, and the runtime epoch, read under one lock, so an outcome
    /// comes with the record of the scan that decided it, or of a later one (E7.2, E7.7).
    pub(crate) fn control_pid_view(&self, instance: &str) -> Result<ControlPidView, String> {
        let state = self.lock();
        let program = self
            .published_program()
            .ok_or("runtime snapshot unavailable")?;
        let instance_id = controller_instance(&program.program_image(), instance)?;
        let record = ProgramRead::value(&*program, &format!("{instance}.ParametersValue"))
            .ok_or("Control.PID ParametersValue is unavailable")?;
        Ok(ControlPidView {
            instance_id,
            record,
            outcome: state.parameters.control_pid.outcomes.get(instance).cloned(),
            epoch: state.parameters.epoch(),
        })
    }

    /// Queues a controller write as a parameter set for the boundary of the task that runs the
    /// controller (E7.7): the set's members are the record's fields, written together at the task's
    /// start and confirmed after the task, as E7.4 confirms a set. It is refused when the members are
    /// not writable, another live write is pending (E9.1), a variable or address is forced, the
    /// snapshot is stale, the controller has not run in Manual mode since its task ran in the
    /// current program, or the record leaves the controller's operating domain (specification 45
    /// §2.4). Returns the write's identity, which its outcome names.
    pub(crate) fn control_pid_apply(&self, write: &ControlPidWrite<'_>) -> Result<String, String> {
        let mut state = self.lock();
        let program = self
            .published_program()
            .ok_or("runtime snapshot unavailable")?;
        let image = program.program_image();
        let task_name = owning_task(&image, write.instance)?;
        let task_ran = state.parameters.task_ran(&task_name);
        let current = members(write.instance, write.current)?;
        let candidate = members(write.instance, write.candidate)?;
        if current.len() != candidate.len()
            || current
                .iter()
                .zip(&candidate)
                .any(|((before, ..), (after, ..))| before.symbol != after.symbol)
        {
            return Err("the candidate record does not have the controller's fields".into());
        }
        let binding = Binding {
            loop_id: write.instance.to_string(),
            task_name,
            parameters: candidate
                .iter()
                .map(|(parameter, ..)| parameter.clone())
                .collect(),
            mode: Mode {
                symbol: format!("{}.{MODE_VALUE}", write.instance),
                manual_value: true,
            },
        };
        writable(&image, &binding)?;
        if let Some(id) = state.transfers.pending_id() {
            return Err(format!("a controller transfer {id} is pending"));
        }
        if let Some(label) = state.parameters.pending_label() {
            return Err(format!("{label} is pending"));
        }
        if !state.live_values.forced_vars.is_empty() || !state.live_values.forced_io.is_empty() {
            return Err("release active forcing before parameter application".into());
        }
        if state.parameters.snapshot_is_stale() {
            return Err(STALE_SNAPSHOT.into());
        }
        controller_manual(&*program, write.instance, &binding.task_name, task_ran)?;
        validate_record(write.candidate)?;
        let expected_values: BTreeMap<String, f64> = current
            .into_iter()
            .map(|(parameter, number, _)| (parameter.symbol, number))
            .collect();
        let mut numbers = BTreeMap::new();
        let mut writes = BTreeMap::new();
        for (parameter, number, value) in candidate {
            numbers.insert(parameter.symbol.clone(), number);
            writes.insert(parameter.symbol, value);
        }
        let transaction_id = state.parameters.control_pid.next_identity();
        let request = ApplyRequest {
            transaction_id: transaction_id.clone(),
            binding,
            expected_program_revision: state.parameters.program_revision(&image),
            expected_binding_revision: String::new(),
            expected_active_revision: hash(&expected_values),
            expected_values,
            values: Some(numbers.clone()),
            previous_revision: None,
            source: if write.physical_target {
                "physical"
            } else {
                "simulation"
            }
            .into(),
            physical_approved: write.physical_approved,
        };
        state.parameters.pending = Some(Pending {
            request,
            candidate: numbers,
            rollback: write.rollback,
            committed: false,
            restore_points: Vec::new(),
            confirmation: None,
            kind: SetKind::ControlPid(Set {
                instance: write.instance.to_string(),
                record: write.candidate.clone(),
                writes,
            }),
        });
        Ok(transaction_id)
    }
}

#[cfg(test)]
#[path = "control_pid_tests.rs"]
mod tests;
