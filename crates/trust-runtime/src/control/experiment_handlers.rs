mod abort_binding;
use abort_binding::{AbortBinding, AbortBindingParams};
mod executor;
mod mode;
use mode::{ModeBinding, ModeBindingParams};
mod target;
use target::ExperimentTarget;
mod values;
use values::{
    check_trajectory_spacing, decode_params, decode_relay_feedback, decode_trajectory,
    experiment_type_id, experiment_value, numeric_value,
};

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value as JsonValue};

use super::{ControlResponse, ControlState};
use crate::debug::DebugControl;
use crate::io::IoAddress;
use crate::value::Value;
use trust_hir::TypeId;

/// The task name of the background programs, which execute in every scan.
const BACKGROUND_TASK: &str = "$background";
/// The shortest experiment, 1 s of process time (Specification 35, A7).
const MIN_DURATION_MS: u64 = 1_000;
/// The longest experiment, 24 h of process time (Specification 35, A7).
const MAX_DURATION_MS: u64 = 86_400_000;

#[derive(Clone, Debug, Default)]
pub struct ControlExperimentService {
    active: Arc<Mutex<Option<ExperimentRecord>>>,
    /// Whether the latest published scan's target is simulated (Specification 35, A2.4); `None` before the first scan.
    target_fact: Arc<Mutex<Option<bool>>>,
}

impl ControlExperimentService {
    /// The target fact of the latest published scan, or `None` before the first one (Specification 35, A2.4).
    pub(crate) fn simulated_target(&self) -> Option<bool> {
        self.target_fact.lock().ok().and_then(|fact| *fact)
    }

    /// Records the target fact that a published scan carries; the scan observer calls it once per cycle.
    pub(crate) fn record_simulated_target(&self, simulated: bool) {
        if let Ok(mut fact) = self.target_fact.lock() {
            *fact = Some(simulated);
        }
    }
}

#[derive(Clone, Debug)]
struct ExperimentRecord {
    id: String,
    source: String,
    kind: String,
    /// The task whose executions drive the experiment, or `BACKGROUND_TASK`.
    task: String,
    /// The period between executions: the task's interval, or the resource cycle for the background programs.
    interval: Duration,
    target: ExperimentTarget,
    restoration_value: Value,
    restoration_observed_value: Value,
    reviewed_scan: u64,
    started_scan: u64,
    /// The process time of the task's first execution after the start request; the trajectory is measured from it.
    origin: Option<Duration>,
    /// The process time the experiment has run, as of its last execution of the task.
    elapsed: Duration,
    duration: Duration,
    status: ExperimentStatus,
    stop_reason: Option<&'static str>,
    restoration: &'static str,
    restoration_reason: Option<&'static str>,
    /// Executions of the task since the restoring write; the restoration is confirmed within three (Specification 35, A7).
    restoration_checks: u32,
    minimum: f64,
    maximum: f64,
    trajectory: Vec<(Duration, Value)>,
    /// The first trajectory point not yet due.
    next_point: usize,
    /// The value last queued to the target; `None` until the first write.
    queued: Option<Value>,
    /// The trajectory point that became due last, until its value is read back.
    queued_point: Option<usize>,
    last_applied: Option<AppliedPoint>,
    /// Whether the request carried the engineer's approval for a physical target.
    physically_approved: bool,
    mode: Option<ModeBinding>,
    relay_feedback: Option<RelayFeedbackRecord>,
    write_enabled: bool,
    abort_binding: Option<AbortBinding>,
    restoration_after_scan: Option<u64>,
    /// The runtime epoch the experiment's references were resolved in; the epoch's end stops the experiment.
    epoch: u64,
}

impl ExperimentRecord {
    /// A running experiment, or one whose output is still being restored: live writes wait for it.
    fn is_busy(&self) -> bool {
        self.status == ExperimentStatus::Running || self.restoration == "pending"
    }
}

/// A trajectory point whose value the target read back after the execution that followed its write.
#[derive(Clone, Debug)]
struct AppliedPoint {
    index: usize,
    at: Duration,
    value: Value,
    scan: Option<u64>,
    process_time: Duration,
}

#[derive(Clone, Debug)]
struct RelayFeedbackRecord {
    address: IoAddress,
    address_text: String,
    value_type: TypeId,
    center: f64,
    hysteresis: f64,
    low_value: Value,
    high_value: Value,
    low_numeric: f64,
    high_numeric: f64,
    high_active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExperimentStatus {
    Running,
    Completed,
    Aborted,
    Failed,
    FailedToRestore,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StartParams {
    experiment_id: String,
    source: String,
    kind: String,
    /// The loop's task, by name, or "$background" for the background programs (Specification 35, A3.5).
    task: String,
    /// The process-image address the experiment writes, or with `symbol` a program variable
    /// (Specification 35, experiments by variable name); exactly one of the two.
    #[serde(default)]
    address: Option<String>,
    #[serde(default)]
    symbol: Option<String>,
    value: JsonValue,
    restoration_value: JsonValue,
    #[serde(default)]
    value_type: Option<String>,
    minimum: f64,
    maximum: f64,
    duration_ms: u64,
    expected_scan: u64,
    /// The loop's mode signal and the value that means Manual (Specification 35, A6.5).
    #[serde(default)]
    mode_binding: Option<ModeBindingParams>,
    #[serde(default)]
    physical_approval: Option<PhysicalApproval>,
    #[serde(default)]
    trajectory: Vec<TrajectoryPoint>,
    #[serde(default)]
    relay_feedback: Option<RelayFeedbackParams>,
    #[serde(default)]
    abort_binding: Option<AbortBindingParams>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TrajectoryPoint {
    at_ms: u64,
    value: JsonValue,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RelayFeedbackParams {
    address: String,
    value_type: String,
    center: f64,
    hysteresis: f64,
    low_value: JsonValue,
    high_value: JsonValue,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PhysicalApproval {
    user_approved: bool,
    topology_reviewed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExperimentIdParams {
    experiment_id: String,
}

/// The refusal text for a live write that a running or restoring experiment blocks: its identifier and whether it still
/// runs or restores (Specification 35, A6.6).
fn experiment_busy_message(record: &ExperimentRecord) -> String {
    if record.status == ExperimentStatus::Running {
        format!("experiment {} is running", record.id)
    } else {
        format!("experiment {} is restoring its output", record.id)
    }
}

/// The refusal for a request whose `source` label contradicts the runtime's target. The target's class is the runtime's
/// fact, never the label (Specification 35, A2.4).
pub(super) fn contradicted_source(source: &str, simulated_target: bool) -> Option<String> {
    let labelled_simulated = source == "simulation";
    (labelled_simulated != simulated_target).then(|| {
        let class = if simulated_target {
            "simulated"
        } else {
            "physical"
        };
        format!("The request says {source}, but the runtime's target is {class}.")
    })
}

/// The runtime checks a start makes under the experiment lock: no transfer or parameter transaction is pending, and the
/// runtime is still in the epoch the request resolved its target in. Starting claims that epoch for the experiment.
/// Returns the refusal, if any.
fn claim_start_admission(
    state: &ControlState,
    id: u64,
    expected_epoch: u64,
) -> Option<ControlResponse> {
    // A pending transfer or parameter transaction owns the controller's values until it is confirmed; its kind and
    // identifier name it (Specification 35, A6.6).
    if let Some(label) = state.debug.pending_transaction_label() {
        return Some(error(
            id,
            format!("{label} is pending; start the experiment after it is confirmed"),
            "transaction_pending",
        ));
    }
    if !state.debug.begin_experiment(expected_epoch) {
        return Some(error(
            id,
            "the runtime was reloaded or restarted while the experiment was starting; read its status again",
            "stale_scan",
        ));
    }
    None
}

/// Answers a request that writes live values. It is refused with `experiment_active` while an experiment is running or
/// restoring. The experiment lock is held across the write, so no experiment can start in the middle of it.
pub(super) fn answer_live_write(
    id: u64,
    state: &ControlState,
    write: impl FnOnce() -> Result<JsonValue, String>,
    error_code: &'static str,
) -> ControlResponse {
    let active = match state.experiments.active.lock() {
        Ok(active) => active,
        Err(_) => {
            return error(
                id,
                "experiment state unavailable",
                "experiment_state_unavailable",
            )
        }
    };
    if let Some(record) = active.as_ref().filter(|record| record.is_busy()) {
        return error(id, experiment_busy_message(record), "experiment_active");
    }
    let result = write();
    drop(active);
    match result {
        Ok(value) => ControlResponse::ok(id, value),
        Err(message) => ControlResponse::error_with_code(id, message, error_code),
    }
}

/// Records a start request. The request is validated and its target and mode resolved here; the first write is queued
/// by the executor at the next scan of the experiment's task (Specification 35, A3.5), not by this handler.
pub(super) fn handle_start(
    id: u64,
    params: Option<JsonValue>,
    state: &ControlState,
) -> ControlResponse {
    let params: StartParams = match decode_params(params) {
        Ok(value) => value,
        Err(error) => return ControlResponse::error_with_code(id, error, "invalid_experiment"),
    };
    if params.experiment_id.trim().is_empty()
        || !(MIN_DURATION_MS..=MAX_DURATION_MS).contains(&params.duration_ms)
        || !params.minimum.is_finite()
        || !params.maximum.is_finite()
        || params.minimum >= params.maximum
    {
        return error(
            id,
            "invalid experiment duration or limits; the duration is 1 s to 24 h of process time",
            "invalid_experiment",
        );
    }
    if params.source != "simulation" && params.source != "physical" {
        return error(id, "unsupported experiment source", "invalid_experiment");
    }
    if ![
        "observation",
        "setpoint_step",
        "output_step",
        "load_step",
        "mv_step",
        "mv_pulse",
        "relay",
        "prbs",
        "multisine",
        "project_sequence",
    ]
    .contains(&params.kind.as_str())
    {
        return error(id, "unsupported experiment kind", "invalid_experiment");
    }
    // The target's class is the runtime's fact from its latest scan; a label that contradicts it is refused (A2.4).
    let Some(simulated_target) = state.experiments.simulated_target() else {
        return error(
            id,
            "no completed runtime scan yet; the runtime's target is not known",
            "io_unavailable",
        );
    };
    if let Some(message) = contradicted_source(&params.source, simulated_target) {
        return error(id, message, "invalid_target");
    }
    let physically_approved = params
        .physical_approval
        .as_ref()
        .is_some_and(|approval| approval.user_approved && approval.topology_reviewed);
    if !simulated_target && !physically_approved {
        return error(
            id,
            "physical experiment requires immediate approval and a reviewed topology",
            "physical_approval_required",
        );
    }
    let (task, interval) = match experiment_task(&params.task, state) {
        Ok(task) => task,
        Err((message, code)) => return error(id, message, code),
    };

    // Read before the target is resolved: a reload that lands in between makes the start refuse rather than
    // resolve a variable in a program the experiment would not be bound to.
    let expected_epoch = state.debug.runtime_epoch();
    let target = {
        let metadata = state.metadata.lock().ok();
        match ExperimentTarget::decode(
            params.address.as_deref(),
            params.symbol.as_deref(),
            &state.debug,
            metadata.as_deref(),
        ) {
            Ok(target) => target,
            Err((message, code)) => return error(id, message, code),
        }
    };
    let declared_type = match params
        .value_type
        .as_deref()
        .map(experiment_type_id)
        .transpose()
    {
        Ok(value_type) => value_type,
        Err(message) => return error(id, message, "invalid_value"),
    };
    let value = match experiment_value(&params.value, params.value_type.as_deref()) {
        Ok(value) => value,
        Err(message) => return error(id, message, "invalid_value"),
    };
    let restoration_value =
        match experiment_value(&params.restoration_value, params.value_type.as_deref()) {
            Ok(value) => value,
            Err(message) => return error(id, message, "invalid_value"),
        };
    let Some(requested) = numeric_value(&value).filter(|value| value.is_finite()) else {
        return error(
            id,
            "experiment value must be finite and numeric",
            "invalid_value",
        );
    };
    if requested < params.minimum || requested > params.maximum {
        return error(
            id,
            "requested value is outside declared limits",
            "outside_limits",
        );
    }

    let snapshot = match state.io_snapshot.lock() {
        Ok(snapshot) => match snapshot.clone() {
            Some(snapshot) => snapshot,
            None => return error(id, "no current I/O snapshot", "io_unavailable"),
        },
        Err(_) => return error(id, "I/O snapshot unavailable", "io_unavailable"),
    };
    let Some(started_scan) = snapshot.scan else {
        return error(id, "no completed runtime scan yet", "io_unavailable");
    };
    if target.forced(&state.debug, &snapshot) {
        return error(id, "experiment signal is forced", "forced_signal");
    }
    if declared_type.is_some() && target.runtime_type(&snapshot) != declared_type {
        return error(
            id,
            "declared experiment type does not match the runtime binding",
            "incompatible_binding",
        );
    }
    let write_value = match target.coerce(value, declared_type) {
        Ok(value) => value,
        Err(message) => return error(id, message, "incompatible_binding"),
    };
    let restoration_write_value = match target.coerce(restoration_value.clone(), declared_type) {
        Ok(value) => value,
        Err(message) => return error(id, message, "incompatible_binding"),
    };
    // An address reads back as its binding decodes it; a variable in its own type.
    let restoration_observed_value = match &target {
        ExperimentTarget::Address { .. } => restoration_value,
        ExperimentTarget::Symbol(_) => restoration_write_value.clone(),
    };
    // An observation never writes this signal, so a changing PV between review
    // and start is normal plant behavior rather than a restoration conflict.
    if params.kind != "observation"
        && target.current(&state.debug, &snapshot).as_ref() != Some(&restoration_observed_value)
    {
        return error(
            id,
            "restoration value does not match the current runtime scan",
            "restoration_mismatch",
        );
    }
    let write_enabled = params.kind != "observation";
    if (params.kind == "relay") != params.relay_feedback.is_some() {
        return error(
            id,
            "relay experiments require relay feedback and other experiments forbid it",
            "invalid_experiment",
        );
    }
    if params.kind == "relay" && !params.trajectory.is_empty() {
        return error(
            id,
            "feedback relay experiments do not accept a timed trajectory",
            "invalid_experiment",
        );
    }
    let coerce = |raw: Value| target.coerce(raw, declared_type);
    let relay_feedback = match params.relay_feedback.as_ref() {
        Some(relay) => match decode_relay_feedback(
            relay,
            &snapshot,
            params.minimum,
            params.maximum,
            params.value_type.as_deref(),
            &coerce,
            requested,
        ) {
            Ok(relay) => Some(relay),
            Err((message, code)) => return error(id, message, code),
        },
        None => None,
    };
    let abort_binding = match (params.abort_binding, target.address()) {
        (Some(binding), Some(address)) => {
            match AbortBinding::decode(binding, &params.kind, address, declared_type, &snapshot) {
                Ok(binding) => Some(binding),
                Err((message, code)) => return error(id, message, code),
            }
        }
        (Some(_), None) => {
            return error(
                id,
                "a project abort binding needs an addressed activation signal",
                "invalid_target",
            )
        }
        (None, _) => None,
    };
    let trajectory = if let Some(relay) = relay_feedback.as_ref() {
        vec![(Duration::ZERO, relay.high_value.clone())]
    } else {
        match decode_trajectory(
            params.trajectory,
            params.duration_ms,
            params.minimum,
            params.maximum,
            params.value_type.as_deref(),
            &coerce,
        ) {
            Ok(values) if values.is_empty() && write_enabled => {
                vec![(Duration::ZERO, write_value.clone())]
            }
            Ok(values) => values,
            Err(message) => return error(id, message, "invalid_experiment"),
        }
    };
    if let Err(message) = check_trajectory_spacing(&trajectory, interval) {
        return error(id, message, "invalid_experiment");
    }
    if params.kind == "mv_pulse"
        && (trajectory.len() != 2
            || trajectory[0] != (Duration::ZERO, write_value.clone())
            || trajectory[1]
                != (
                    Duration::from_millis(params.duration_ms / 2),
                    restoration_write_value.clone(),
                ))
    {
        return error(
            id,
            "MV pulses must rise at 0 ms and return to baseline halfway through duration",
            "invalid_experiment",
        );
    }
    let mode =
        match (mode_requirement(&params.kind), params.mode_binding) {
            (Some(manual_required), Some(binding)) => {
                match ModeBinding::start(binding, manual_required, &state.debug) {
                    Ok(mode) => Some(mode),
                    Err((message, code)) => return error(id, message, code),
                }
            }
            (Some(_), None) => return error(
                id,
                "this experiment needs the loop's mode_binding (its mode signal and Manual value)",
                "invalid_experiment",
            ),
            (None, _) => None,
        };

    let (target_key, target_value) = target.echo();
    let record = ExperimentRecord {
        id: params.experiment_id.clone(),
        // The source a status reports is the target's class, whatever the request labelled it (A2.4).
        source: (if simulated_target {
            "simulation"
        } else {
            "physical"
        })
        .to_string(),
        kind: params.kind,
        task,
        interval,
        target: target.clone(),
        restoration_value: restoration_write_value,
        restoration_observed_value,
        reviewed_scan: params.expected_scan,
        started_scan,
        origin: None,
        elapsed: Duration::ZERO,
        duration: Duration::from_millis(params.duration_ms),
        status: ExperimentStatus::Running,
        stop_reason: None,
        restoration: if write_enabled {
            "pending"
        } else {
            "not_required"
        },
        restoration_reason: None,
        restoration_checks: 0,
        minimum: params.minimum,
        maximum: params.maximum,
        trajectory,
        next_point: 0,
        queued: None,
        queued_point: None,
        last_applied: None,
        physically_approved,
        mode,
        relay_feedback,
        write_enabled,
        abort_binding,
        restoration_after_scan: None,
        epoch: expected_epoch,
    };
    {
        let mut active = match state.experiments.active.lock() {
            Ok(active) => active,
            Err(_) => {
                return error(
                    id,
                    "experiment state unavailable",
                    "experiment_state_unavailable",
                )
            }
        };
        // A restoring experiment keeps its output until it is restored, so it is never replaced (A6.6).
        if let Some(current) = active.as_ref().filter(|current| current.is_busy()) {
            return error(id, experiment_busy_message(current), "experiment_active");
        }
        if let Some(refusal) = claim_start_admission(state, id, expected_epoch) {
            return refusal;
        }
        *active = Some(record);
    }
    ControlResponse::ok(
        id,
        json!({
            "experiment_id": params.experiment_id,
            "status": "running",
            "restoration": if write_enabled { "pending" } else { "not_required" },
            target_key: target_value,
        }),
    )
}

/// The task an experiment follows and its period: the configured task's interval, or the resource cycle for the
/// background programs (Specification 35, A3.5).
fn experiment_task(
    task: &str,
    state: &ControlState,
) -> Result<(String, Duration), (String, &'static str)> {
    if task.trim().is_empty() {
        return Err((
            "give the loop's task name, or $background".into(),
            "invalid_experiment",
        ));
    }
    if task == BACKGROUND_TASK {
        let cycle = state
            .settings
            .lock()
            .map_err(|_| {
                (
                    "runtime settings unavailable".to_string(),
                    "experiment_state_unavailable",
                )
            })?
            .cycle_interval;
        return Ok((BACKGROUND_TASK.to_string(), std_duration(cycle)));
    }
    let metadata = state.metadata.lock().map_err(|_| {
        (
            "runtime metadata unavailable".to_string(),
            "experiment_state_unavailable",
        )
    })?;
    metadata
        .tasks()
        .iter()
        .find(|configured| configured.name.eq_ignore_ascii_case(task))
        .map(|configured| {
            (
                configured.name.to_string(),
                std_duration(configured.interval),
            )
        })
        .ok_or_else(|| {
            (
                format!("experiment task {task} is not configured"),
                "invalid_experiment",
            )
        })
}

/// A process-time duration as the executor counts it.
fn std_duration(duration: crate::value::Duration) -> Duration {
    Duration::from_nanos(u64::try_from(duration.as_nanos()).unwrap_or(0))
}

/// Whether the experiment runs only while the loop is in Manual (`Some(true)`) or only in Auto (`Some(false)`), or in
/// either (`None`). Output experiments write the plant input or the manual output; setpoint and load experiments need
/// the loop to follow its setpoint (Specification 35, A6.5).
fn mode_requirement(kind: &str) -> Option<bool> {
    match kind {
        "output_step" | "mv_step" | "mv_pulse" | "relay" | "prbs" | "multisine" => Some(true),
        "setpoint_step" | "load_step" => Some(false),
        _ => None,
    }
}

/// The scan the control surface last published, which stamps a stop made between scans.
fn latest_scan(state: &ControlState) -> Option<u64> {
    state
        .io_snapshot
        .lock()
        .ok()
        .and_then(|snapshot| snapshot.as_ref().and_then(|value| value.scan))
}

pub(super) fn handle_status(
    id: u64,
    params: Option<JsonValue>,
    state: &ControlState,
) -> ControlResponse {
    let params: ExperimentIdParams = match decode_params(params) {
        Ok(value) => value,
        Err(message) => return error(id, message, "invalid_experiment"),
    };
    // A status request only reads. A boundary that ended the experiment is settled by the next scan (A6.7), never here.
    let active = match state.experiments.active.lock() {
        Ok(active) => active,
        Err(_) => {
            return error(
                id,
                "experiment state unavailable",
                "experiment_state_unavailable",
            )
        }
    };
    let Some(record) = active
        .as_ref()
        .filter(|record| record.id == params.experiment_id)
    else {
        return error(id, "experiment was not found", "experiment_not_found");
    };
    ControlResponse::ok(id, record_json(record))
}

pub(super) fn handle_abort(
    id: u64,
    params: Option<JsonValue>,
    state: &ControlState,
) -> ControlResponse {
    let params: ExperimentIdParams = match decode_params(params) {
        Ok(value) => value,
        Err(message) => return error(id, message, "invalid_experiment"),
    };
    match stop(
        state,
        &params.experiment_id,
        ExperimentStatus::Aborted,
        "user_abort",
    ) {
        Ok(result) => ControlResponse::ok(id, result),
        Err((message, code)) => error(id, message, code),
    }
}

/// Ends a running experiment as a normal completion, without asserting the project abort binding: the restoration is
/// written at the next scan and confirmed as for any other completion (Specification 35, A3.5). An experiment that is
/// not running returns its current status.
pub(super) fn handle_finish(
    id: u64,
    params: Option<JsonValue>,
    state: &ControlState,
) -> ControlResponse {
    let params: ExperimentIdParams = match decode_params(params) {
        Ok(value) => value,
        Err(message) => return error(id, message, "invalid_experiment"),
    };
    match stop(
        state,
        &params.experiment_id,
        ExperimentStatus::Completed,
        "finished_early",
    ) {
        Ok(result) => ControlResponse::ok(id, result),
        Err((message, code)) => error(id, message, code),
    }
}

/// Stops the named experiment with `status` and `stop_reason`, queuing its restoration when it has written anything.
fn stop(
    state: &ControlState,
    experiment_id: &str,
    status: ExperimentStatus,
    stop_reason: &'static str,
) -> Result<JsonValue, (String, &'static str)> {
    let scan = latest_scan(state);
    let mut active = state.experiments.active.lock().map_err(|_| {
        (
            "experiment state unavailable".to_string(),
            "experiment_state_unavailable",
        )
    })?;
    let record = active
        .as_mut()
        .filter(|record| record.id == experiment_id)
        .ok_or_else(|| {
            (
                "experiment was not found".to_string(),
                "experiment_not_found",
            )
        })?;
    settle_at_boundary(record, &state.debug, scan);
    finish_record(record, &state.debug, scan, status, stop_reason);
    Ok(record_json(record))
}

/// Stops a running experiment with `status` and `stop_reason`, and queues its restoration. A runtime boundary that ended
/// the experiment's epoch takes the place of the request: the experiment fails with the boundary's reason, and a variable
/// is not written, because its reference names the program that ended. Nothing is restored when nothing was written.
fn finish_record(
    record: &mut ExperimentRecord,
    debug: &DebugControl,
    scan: Option<u64>,
    status: ExperimentStatus,
    stop_reason: &'static str,
) {
    if record.status != ExperimentStatus::Running {
        return;
    }
    let boundary = debug.experiment_boundary(record.epoch);
    let (status, stop_reason) = match boundary {
        Some(reason) => (ExperimentStatus::Failed, reason),
        None => (status, stop_reason),
    };
    let ended_variable = boundary.is_some() && record.target.is_symbol();
    let restores = record.write_enabled && record.queued.is_some();
    // Abort assertion precedes deactivation; the declared ST program owns the
    // actuator policy. Queue both while excluding late excitation writes.
    if restores {
        if let Some(abort) = record.abort_binding.as_mut() {
            record.restoration_after_scan = Some(scan.unwrap_or(record.started_scan));
            if status != ExperimentStatus::Completed {
                abort.request(debug);
            }
        }
        if !ended_variable {
            record
                .target
                .write(debug, record.restoration_value.clone(), record.epoch);
        }
    }
    record.status = status;
    record.stop_reason = Some(stop_reason);
    record.restoration_checks = 0;
    if !restores {
        record.restoration = "not_required";
    } else if ended_variable {
        record.restoration = "failed";
        record.restoration_reason = boundary;
    } else {
        record.restoration = "pending";
    }
}

/// Settles an experiment whose runtime epoch has ended, whichever request or scan reaches it first: a running experiment
/// stops with the boundary's reason, and a variable restoration, which can no longer be written, fails at once.
fn settle_at_boundary(record: &mut ExperimentRecord, debug: &DebugControl, scan: Option<u64>) {
    let Some(reason) = debug.experiment_boundary(record.epoch) else {
        return;
    };
    if record.status == ExperimentStatus::Running {
        finish_record(record, debug, scan, ExperimentStatus::Failed, reason);
    } else if record.restoration == "pending" && record.target.is_symbol() {
        record.restoration = "failed";
        record.restoration_reason = Some(reason);
    }
}

fn applied_json(point: &AppliedPoint) -> JsonValue {
    json!({
        "index": point.index,
        "value": numeric_value(&point.value),
        "time_ms": point.at.as_millis() as u64,
        "scan": point.scan,
        "process_time_ms": point.process_time.as_millis() as u64,
    })
}

fn record_json(record: &ExperimentRecord) -> JsonValue {
    let relay_feedback = record.relay_feedback.as_ref().map(|relay| {
        json!({
        "address": relay.address_text,
            "center": relay.center,
            "hysteresis": relay.hysteresis,
            "lower_threshold": relay.center - relay.hysteresis,
            "upper_threshold": relay.center + relay.hysteresis,
        "low_value": relay.low_numeric,
        "high_value": relay.high_numeric,
            "high_active": relay.high_active,
        })
    });
    let (target_key, target_value) = record.target.echo();
    json!({
        "experiment_id": record.id,
        "source": record.source,
        "kind": record.kind,
        "task": record.task,
        target_key: target_value,
        "status": match record.status {
            ExperimentStatus::Running => "running",
            ExperimentStatus::Completed => "completed",
            ExperimentStatus::Aborted => "aborted",
            ExperimentStatus::Failed => "failed",
            ExperimentStatus::FailedToRestore => "failed_to_restore",
        },
        "elapsed_ms": record.elapsed.as_millis() as u64,
        "duration_ms": record.duration.as_millis() as u64,
        "reviewed_scan": record.reviewed_scan,
        "started_scan": record.started_scan,
        "stop_reason": record.stop_reason,
        "restoration": record.restoration,
        "restoration_reason": record.restoration_reason,
        "last_applied": record.last_applied.as_ref().map(applied_json),
        "relay_feedback": relay_feedback,
        "abort_binding": record.abort_binding.as_ref().map(AbortBinding::json),
    })
}

fn error(id: u64, message: impl Into<String>, code: &'static str) -> ControlResponse {
    ControlResponse::error_with_code(id, message.into(), code)
}
