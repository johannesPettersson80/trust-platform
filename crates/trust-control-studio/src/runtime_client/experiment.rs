//! The experiment operations of the runtime protocol annex (E5.3 to E5.6) as methods on the runtime client: the start
//! from a reviewed plan, the status record, abort and finish (`ControlExperimentClient` of `experiment.ts`).
//!
//! The start request is built from trust-control's plan, and each answer is decoded and checked before it is used.
//! A refusal keeps the runtime's text and code (E0.4). Which requests may be sent, and when, is decided by the rules
//! of `experiment_recording` and `test_experiment_guards`.

use serde::Deserialize;
use serde_json::{json, Value};
use trust_control::experiment::{
    checked_experiment_start, classify_control_experiment_status, ControlExperimentKind,
    ControlExperimentPlan, ControlExperimentRuntimePhase, ExperimentValue, PlanSource,
};
pub use trust_control::experiment::{ExperimentStart, RestorationState};
use trust_control::failure::Failure;
use trust_control::identification::connected_run::StartRead;
use trust_control::runtime::RuntimeConnection;

use super::{decode, open_client, RuntimeClient};
use trust_runtime_client::ControlError;

/// The largest integer JSON carries exactly (E0.3).
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// The state of an experiment (E5.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentState {
    /// The experiment runs.
    Running,
    /// The experiment completed its duration or was finished early.
    Completed,
    /// The experiment was aborted.
    Aborted,
    /// The experiment failed.
    Failed,
    /// The experiment stopped and its restoration was not confirmed.
    FailedToRestore,
}

/// The status record of an experiment (E5.6). Abort and finish answer with the same record (E5.5).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ExperimentStatus {
    /// The experiment's identifier.
    pub experiment_id: String,
    /// Whether the experiment runs on a simulated or a physical target.
    pub source: PlanSource,
    /// The experiment's kind.
    pub kind: ControlExperimentKind,
    /// The task that runs the experiment, or `$background`.
    pub task: String,
    /// The affected signal's address, when the experiment writes through one.
    #[serde(default)]
    pub address: Option<String>,
    /// The affected signal's symbol, when the experiment writes by name.
    #[serde(default)]
    pub symbol: Option<String>,
    /// The state.
    pub status: ExperimentState,
    /// Why the experiment stopped; null while it runs.
    #[serde(default)]
    pub stop_reason: Option<String>,
    /// The restoration.
    pub restoration: RestorationState,
    /// Why the restoration failed or was reset; null when it did not.
    #[serde(default)]
    pub restoration_reason: Option<String>,
    /// The process time elapsed, in ms.
    pub elapsed_ms: u64,
    /// The planned duration, in ms.
    pub duration_ms: u64,
    /// The sequence value the review was made against.
    pub reviewed_scan: u64,
    /// The sequence value of the first execution, once the experiment started.
    #[serde(default)]
    pub started_scan: Option<u64>,
    /// The trajectory point last applied, or null.
    #[serde(default)]
    pub last_applied: Option<Value>,
    /// The relay of a relay experiment, or null.
    #[serde(default)]
    pub relay_feedback: Option<Value>,
    /// The abort binding of a project-owned experiment, or null.
    #[serde(default)]
    pub abort_binding: Option<Value>,
}

impl RuntimeClient {
    /// Starts the experiment of a reviewed plan (E5.3). `physical_approved` is the engineer's approval of a physical
    /// plan (E10.2); the runtime checks it again.
    ///
    /// # Errors
    /// The request cannot be built, the runtime refuses it, or its answer does not describe a started experiment.
    pub fn start_experiment(
        &mut self,
        plan: &ControlExperimentPlan,
        physical_approved: bool,
    ) -> Result<ExperimentStart, Failure> {
        let params = start_request(plan, physical_approved)?;
        let answer = self.call("control.experiment.start", Some(params))?;
        // The answer must start the experiment that was sent, running, with its restoration pending or not required (E5.3).
        checked_experiment_start(&answer, &plan.id)
    }

    /// Sends the start of a reviewed plan once, over a connection of its own, and says what came back (E5.3, E0.4): the
    /// runtime's answer as it sent it, its refusal with the code and text it carries, or no answer that can be read. The
    /// request is the one [`start_request`] builds without a physical approval, as a connected identification sends it.
    ///
    /// The answer is not checked here, because the rules in `trust-control` decide what it means (`start_outcome`). The
    /// refusal keeps its code, because the code decides whether the start is reconciled by its identity (E11.4); `call`
    /// cannot give this, since it turns a refusal into a [`Failure`] without its code. A request that cannot be built, or a
    /// connection that cannot be opened, gives no answer, so its start is reconciled as a lost one is.
    #[must_use]
    pub fn start_experiment_read(
        connection: &RuntimeConnection,
        plan: &ControlExperimentPlan,
    ) -> StartRead {
        let params = match start_request(plan, false) {
            Ok(params) => params,
            Err(failure) => return StartRead::Unreadable(failure.to_string()),
        };
        let mut client = match open_client(connection) {
            Ok(client) => client,
            Err(reason) => return StartRead::Unreadable(reason),
        };
        match client.request("control.experiment.start", Some(params)) {
            Ok(answer) => StartRead::Answer(answer),
            Err(ControlError::Refused { error, code }) => StartRead::Refused { code, text: error },
            Err(error) => StartRead::Unreadable(error.to_string()),
        }
    }

    /// The status record of an experiment (E5.6). A status request never changes the record.
    ///
    /// # Errors
    /// The runtime refuses, cannot be reached, or answers with another record.
    pub fn experiment_status(&mut self, experiment_id: &str) -> Result<ExperimentStatus, Failure> {
        let answer = self.call(
            "control.experiment.status",
            Some(json!({ "experiment_id": experiment_id })),
        )?;
        checked_status("control.experiment.status", experiment_id, answer)
    }

    /// Aborts a running experiment (E5.5). The record of an experiment that is not running is returned unchanged.
    ///
    /// # Errors
    /// The runtime refuses, cannot be reached, or answers with another record.
    pub fn abort_experiment(&mut self, experiment_id: &str) -> Result<ExperimentStatus, Failure> {
        let answer = self.call(
            "control.experiment.abort",
            Some(json!({ "experiment_id": experiment_id })),
        )?;
        checked_status("control.experiment.abort", experiment_id, answer)
    }

    /// Finishes a running experiment early and normally: its abort binding is not asserted (E5.5).
    ///
    /// # Errors
    /// The runtime refuses, cannot be reached, or answers with another record.
    pub fn finish_experiment(&mut self, experiment_id: &str) -> Result<ExperimentStatus, Failure> {
        let answer = self.call(
            "control.experiment.finish",
            Some(json!({ "experiment_id": experiment_id })),
        )?;
        checked_status("control.experiment.finish", experiment_id, answer)
    }
}

/// The request of `control.experiment.start` for a reviewed plan (E5.3). The target is the plan's address when it has
/// one, else the variable by name (E2.4). `physical_approved` is sent with the plan's topology review, for a physical
/// plan only (E10.2).
///
/// # Errors
/// A time or count of the plan is not a whole number of its unit, so the request cannot be sent.
pub fn start_request(
    plan: &ControlExperimentPlan,
    physical_approved: bool,
) -> Result<Value, Failure> {
    let mut request = json!({
        "experiment_id": plan.id,
        "source": source_name(plan.source),
        "kind": plan.kind.wire_name(),
        "value": experiment_value(plan.to),
        "restoration_value": experiment_value(plan.restoration_value),
        "value_type": plan.value_type,
        "minimum": plan.minimum,
        "maximum": plan.maximum,
        "duration_ms": whole_number(plan.duration_ms)?,
        "expected_scan": plan.expected_scan,
        "task": plan.task.as_deref().unwrap_or("$background"),
        "trajectory": trajectory_request(plan)?,
    });
    match &plan.address {
        Some(address) => request["address"] = json!(address),
        None => request["symbol"] = json!(plan.affected_signal),
    }
    // A project's abort binding accompanies only an addressed experiment (the extension's `experiment.ts`, §17.3).
    if let (Some(abort), Some(_)) = (&plan.abort_binding, &plan.address) {
        request["abort_binding"] = json!({
            "address": abort.address,
            "value_type": abort.value_type,
            "asserted_value": abort.asserted_value,
        });
    }
    if let Some(mode) = &plan.mode_binding {
        request["mode_binding"] = json!({
            "symbol": mode.symbol,
            "manual_value": mode.manual_value,
        });
    }
    if let Some(relay) = &plan.relay_feedback {
        request["relay_feedback"] = json!({
            "address": relay.address,
            "value_type": relay.value_type,
            "center": relay.center,
            "hysteresis": relay.hysteresis,
            "low_value": relay.low_output,
            "high_value": relay.high_output,
        });
    }
    if plan.source == PlanSource::Physical {
        request["physical_approval"] = json!({
            "user_approved": physical_approved,
            "topology_reviewed": true,
        });
    }
    Ok(request)
}

/// The whole number of milliseconds `value` holds, when it is finite, not negative and exact in JSON (E0.3).
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // Checked: whole, from 0 to 2^53 − 1.
pub(super) fn whole_milliseconds(value: f64) -> Option<u64> {
    let whole =
        value.is_finite() && value >= 0.0 && value.fract() == 0.0 && value <= MAX_SAFE_INTEGER;
    whole.then_some(value as u64)
}

fn whole_number(value: f64) -> Result<Value, Failure> {
    whole_milliseconds(value)
        .map(|number| json!(number))
        .ok_or_else(|| {
            Failure::invalid_request(&format!("{value} is not a whole number of milliseconds."))
        })
}

fn experiment_value(value: ExperimentValue) -> Value {
    match value {
        ExperimentValue::Number(number) => json!(number),
        ExperimentValue::Boolean(boolean) => json!(boolean),
    }
}

fn source_name(source: PlanSource) -> &'static str {
    match source {
        PlanSource::Simulation => "simulation",
        PlanSource::Physical => "physical",
    }
}

/// The trajectory as the runtime reads it (E5.3). A project's Boolean authority carries its 0 and 1 as JSON booleans.
#[allow(clippy::float_cmp)] // The authority's points are exactly 0 or 1.
fn trajectory_request(plan: &ControlExperimentPlan) -> Result<Value, Failure> {
    let authority =
        plan.kind == ControlExperimentKind::ProjectSequence && plan.value_type == "BOOL";
    let points = plan
        .trajectory
        .iter()
        .map(|point| -> Result<Value, Failure> {
            let value = if authority && (point.value == 0.0 || point.value == 1.0) {
                let on = point.value == 1.0;
                json!(on)
            } else {
                json!(point.value)
            };
            Ok(json!({ "at_ms": whole_number(point.at_ms)?, "value": value }))
        })
        .collect::<Result<Vec<Value>, Failure>>()?;
    Ok(Value::Array(points))
}

/// Decodes a status record of `operation` and checks it: the runtime's vocabulary (E5.6), and the identifier the
/// request named. A record that fails either check is never evidence (E11.8).
fn checked_status(
    operation: &str,
    experiment_id: &str,
    answer: Value,
) -> Result<ExperimentStatus, Failure> {
    if classify_control_experiment_status(&answer) == ControlExperimentRuntimePhase::Invalid {
        return Err(Failure::invalid_response(operation));
    }
    let status: ExperimentStatus = decode(operation, answer)?;
    if status.experiment_id != experiment_id {
        return Err(Failure::invalid_response(operation));
    }
    Ok(status)
}

#[cfg(test)]
mod start_read_tests {
    //! The start that keeps the runtime's refusal code (`RuntimeClient::start_experiment_read`; annex E5.3, E0.4 and
    //! E11.4). The fake runtime answers with exactly the fields that E5.3 names, for the experiment that the request names.

    use serde_json::json;
    use trust_control::experiment::ControlExperimentPlan;
    use trust_control::identification::connected_run::StartRead;

    use super::{start_request, RuntimeClient};
    use crate::runtime_client::fake_runtime::{answer, refusal, FakeRuntime};

    /// A reviewed setpoint step of the tank, written by symbol.
    fn plan() -> ControlExperimentPlan {
        serde_json::from_value(json!({
            "id": "experiment-1", "source": "simulation", "kind": "setpoint_step", "affectedSignal": "LevelSP",
            "valueType": "LREAL", "from": 45, "to": 47, "unit": "%", "minimum": 0, "maximum": 100,
            "durationMs": 570_000, "expectedScan": 7, "task": "$background", "stopConditions": ["Abort"],
            "restorationValue": 45, "requiresPhysicalApproval": false, "trajectory": [{ "atMs": 0, "value": 47 }]
        }))
        .expect("the plan decodes")
    }

    #[test]
    fn the_start_answer_and_a_refusal_with_its_code_come_back_and_a_lost_answer_is_unreadable() {
        let started = FakeRuntime::start(|request| {
            let params = &request["params"];
            Some(answer(
                request,
                json!({
                    "experiment_id": params["experiment_id"], "status": "running", "restoration": "pending",
                    "symbol": params["symbol"]
                }),
            ))
        });
        assert_eq!(
            RuntimeClient::start_experiment_read(&started.connection(Some("secret")), &plan()),
            StartRead::Answer(json!({
                "experiment_id": "experiment-1", "status": "running", "restoration": "pending", "symbol": "LevelSP"
            }))
        );
        let requests = started.requests();
        assert_eq!(requests.len(), 1, "the start is sent once");
        assert_eq!(requests[0]["type"], "control.experiment.start");
        assert_eq!(
            requests[0]["params"],
            start_request(&plan(), false).expect("the request"),
            "the request a connected identification sends, without a physical approval"
        );
        assert_eq!(
            requests[0]["auth"], "secret",
            "the control token goes with the request"
        );

        let refused = FakeRuntime::start(|request| {
            Some(refusal(
                request,
                "outside_limits",
                "The value 47 lies outside the limits of LevelSP.",
            ))
        });
        assert_eq!(
            RuntimeClient::start_experiment_read(&refused.connection(None), &plan()),
            StartRead::Refused {
                code: Some("outside_limits".to_string()),
                text: "The value 47 lies outside the limits of LevelSP.".to_string()
            }
        );

        // A runtime that closes the connection without an answer: a lost acknowledgement (E11.4).
        let silent = FakeRuntime::start(|_| None);
        assert_eq!(
            RuntimeClient::start_experiment_read(&silent.connection(None), &plan()),
            StartRead::Unreadable("the runtime closed the connection".to_string())
        );
    }
}

#[cfg(test)]
#[path = "experiment_tests.rs"]
mod tests;
