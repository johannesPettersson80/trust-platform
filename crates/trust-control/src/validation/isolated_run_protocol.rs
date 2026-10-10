//! Isolated test runs (runtime protocol annex, E8.4, with E0.2, E0.3, E8.1 to E8.3 and E11.2; specification 35, A2.6,
//! A4.6 and B5.3): the records of the start, the status and the cancel of an isolated run, and the checks that Studio
//! makes before it sends a request and after it receives an answer.
//!
//! The runtime runs the project's actual Structured Text in a separate process with simulated I/O only. Nothing here
//! opens a connection, reads a clock or writes a target: the engine sends these records and calls these checks.
//!
//! A run that ended early is evidence of what it recorded, never a completed validation (E8.4); see
//! [`IsolatedRunStatus::evidence`]. The records of a run's conditions reuse the run record of identification
//! (`RunEvent`, `RunEvidence`, `ValidationChannel`), and the inline plant is `SimulationOverride`. The limits and rules of
//! a request are in the child module `request_checks`.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::connected_record::{RunEvent, RunEvidence, ValidationChannel};
use crate::model::SimulationOverride;
use crate::recording::wire::expand_recorded_run_wire;
use crate::signals::Quality;

use super::isolated_run_sampling::{recorded_sample_times, scan_count};

mod request_checks;

/// The execution word of the wire (E8.2, E8.4).
const EXECUTION: &str = "isolated-simulation";
/// The source basis of an isolated run's answers (E8.2, E8.4).
const SOURCE_BASIS: &str = "loaded-runtime-sources";
/// The I/O provenance of an isolated run's answers (E8.4).
const IO_PROVENANCE: &str = "isolated-simulation-no-physical-drivers";
/// The status operation, named in a refusal of its answer.
const STATUS_OPERATION: &str = "control.simulation.run.status";
/// The readiness reason when the runtime gives none (E8.2).
const NOT_READY: &str = "The loaded ST and isolated simulation are not ready for verification.";

/// The execution of an isolated run, which is always `isolated-simulation` (E8.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum IsolatedExecution {
    /// The project's actual Structured Text runs against simulated I/O (E8.1).
    IsolatedSimulation,
}

/// The state of an isolated job (E8.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum IsolatedJobState {
    /// Accepted and waiting for its worker.
    Queued,
    /// Its worker is executing the grid.
    Running,
    /// Every scan of the grid ran.
    Completed,
    /// Cancelled by a request; the job ends when its child process has ended.
    Cancelled,
    /// Ended by an abnormal exit, a missed scan deadline or the wall budget.
    Failed,
}

impl IsolatedJobState {
    /// Whether the job has ended: `completed`, `cancelled` or `failed` (E8.4).
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed)
    }
}

/// The type of an overridden parameter: `REAL` or `LREAL`, or a fixed configuration `BOOL` (E8.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum IsolatedParameterType {
    /// `REAL`: its value and bounds must be exactly representable as single precision (E0.3).
    Real,
    /// `LREAL`: any finite double.
    Lreal,
    /// A fixed configuration `BOOL`: unit `1`, bounds 0 to 1 and value 0 or 1 (E8.4).
    Bool,
}

/// An observed channel of a run (E8.3): its symbol and its unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IsolatedRunObservation {
    /// The signal's symbol.
    pub symbol: String,
    /// The signal's unit.
    pub unit: String,
}

/// An event of a run (E8.3): a symbol is set to a value at a time that lies on a scan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IsolatedRunEvent {
    /// The time of the event in milliseconds of simulation time; a multiple of the step.
    pub at_ms: u64,
    /// The symbol the event sets.
    pub symbol: String,
    /// The value it sets.
    pub value: f64,
}

/// A parameter override of a run (E8.4): a writable scalar the run holds for its whole duration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IsolatedParameterOverride {
    /// The parameter's symbol.
    pub symbol: String,
    /// `REAL`, `LREAL` or a fixed configuration `BOOL`.
    pub iec_type: IsolatedParameterType,
    /// The parameter's unit; `1` for a `BOOL`.
    pub unit: String,
    /// What the parameter means, for example `proportional gain`.
    pub convention: String,
    /// The smallest value the run may hold.
    pub minimum: f64,
    /// The largest value the run may hold.
    pub maximum: f64,
    /// The value the run holds.
    pub value: f64,
}

/// The bounded feedback relay of a run (E8.4): a two-level command that switches on the measurement's band, with
/// the bounds of the runtime's declaration. The run echoes it in its conditions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IsolatedFeedbackRelay {
    /// The measurement the relay switches on, by symbol.
    pub process_variable: String,
    /// The command the relay writes, by symbol.
    pub command: String,
    /// The centre of the switching band.
    pub center: f64,
    /// The half width of the switching band; above 0.
    pub hysteresis: f64,
    /// The command while the measurement is low; below `high`.
    pub low: f64,
    /// The command while the measurement is high.
    pub high: f64,
    /// The command after the relay restores; between `low` and `high`.
    pub restore: f64,
    /// The smallest measurement the declaration allows.
    pub process_minimum: f64,
    /// The largest measurement the declaration allows.
    pub process_maximum: f64,
    /// Whether a high measurement switches the command high.
    pub positive_gain: bool,
    /// When the relay starts, in milliseconds of simulation time; a multiple of the step.
    pub start_ms: u64,
    /// When the relay restores, in milliseconds of simulation time; after `start_ms`, within the run.
    pub restore_ms: u64,
    /// The shortest high dwell, in milliseconds; a multiple of the step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_high_ms: Option<u64>,
    /// The shortest low dwell, in milliseconds; a multiple of the step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_low_ms: Option<u64>,
}

/// The request of `control.simulation.run.start` (E8.4). Studio chooses `request_id` and journals it before it sends
/// the request (E0.2, E11.2), so the run can be read back by its identifier and never started again (E11.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IsolatedRunRequest {
    /// The run's identifier, 1 to 128 ASCII letters, digits or hyphens; the runtime's `job_id`.
    pub request_id: String,
    /// The project revision the run was reviewed on (E8.2); the runtime refuses a run on another one.
    pub expected_project_revision: String,
    /// Always `isolated-simulation`.
    pub execution: IsolatedExecution,
    /// The scan interval, in milliseconds: 1 to 1,000.
    pub step_ms: u64,
    /// The simulated duration, in milliseconds: more than 0, at most 86,400,000, a multiple of the step.
    pub duration_ms: u64,
    /// The seed: at most 9,007,199,254,740,991.
    pub seed: u64,
    /// The most scans the run may execute: at least its grid points, at most 10,000,000.
    pub max_total_cycles: u64,
    /// The wall time of the run, in milliseconds: 1 to 600,000.
    pub wall_budget_ms: u64,
    /// The executions one sample averages, 1 to 100,000; 1 keeps every grid point (A4.2).
    #[serde(default = "one")]
    pub record_every: u64,
    /// The values applied before the first scan, by symbol (E8.3).
    pub initial_values: BTreeMap<String, f64>,
    /// The events, in time order and unique by time and symbol (E8.3).
    pub events: Vec<IsolatedRunEvent>,
    /// The observed channels: 1 to 64 (E8.3).
    pub observations: Vec<IsolatedRunObservation>,
    /// The parameter overrides: at most 64 (E8.4).
    #[serde(default)]
    pub parameter_overrides: Vec<IsolatedParameterOverride>,
    /// The inline plant of a selected model, when the run takes one (E8.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub simulation_override: Option<SimulationOverride>,
    /// The bounded feedback relay, when the run takes one (E8.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_relay: Option<IsolatedFeedbackRelay>,
}

fn one() -> u64 {
    1
}

/// The answer to a start: the job, its state and the revision it runs on (E8.4). A repeated start with the same
/// identifier and inputs answers with the current state of that run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IsolatedRunStart {
    /// The job's identifier, which is the request's `request_id`.
    pub job_id: String,
    /// The job's state when the runtime answered.
    pub status: IsolatedJobState,
    /// The project revision the job runs on.
    pub project_revision: String,
}

/// The answer to a status or a cancel (E8.4). The run is present only once the job has ended, and only when the
/// request asked for results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IsolatedRunStatus {
    /// The job's identifier.
    pub job_id: String,
    /// The project revision the job runs on.
    pub project_revision: String,
    /// The job's state.
    pub status: IsolatedJobState,
    /// Whether a cancel has been requested.
    pub cancel_requested: bool,
    /// Why the job failed or was cancelled, when the runtime says so.
    #[serde(default)]
    pub reason: Option<String>,
    /// Where the job's source comes from: `loaded-runtime-sources`.
    pub source_basis: String,
    /// The job's I/O provenance: `isolated-simulation-no-physical-drivers`.
    pub io_provenance: String,
    /// The complete sample frames the runtime has received.
    pub captured_sample_count: u64,
    /// The run, once the job has ended and the request asked for results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<IsolatedRunRecord>,
}

/// The run of an isolated job (the TypeScript `RecordedRun`): its identity, its evidence, its conditions and its
/// samples, as the runtime sends them after expansion of the compact samples (A7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IsolatedRunRecord {
    /// The run's identifier, `run-0` for an isolated run.
    pub id: String,
    /// Where its values come from: `ST-executed` for a run that executed the project's Structured Text.
    pub evidence: RunEvidence,
    /// The parameter overrides, by symbol.
    pub parameters: BTreeMap<String, f64>,
    /// The conditions the run was taken under.
    pub conditions: IsolatedRunConditions,
    /// The observed channels, in the request's order.
    pub channels: Vec<ValidationChannel>,
    /// The samples, in order: each on its grid point.
    pub samples: Vec<IsolatedRunSample>,
}

/// The conditions of a run (E8.4): the request's frozen conditions, and the optional parts a request declares.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IsolatedRunConditions {
    /// The initial values, by symbol.
    pub initial_state: BTreeMap<String, f64>,
    /// The events, in seconds of the run.
    pub events: Vec<RunEvent>,
    /// The duration of the run in seconds.
    pub duration_seconds: f64,
    /// The time of each grid point in seconds.
    pub sample_times_seconds: Vec<f64>,
    /// The seed of the run.
    pub seed: u64,
    /// The model the run is compared with.
    pub model_revision: String,
    /// The program revision of the run.
    pub program_revision: String,
    /// The revision of the parameter declarations.
    pub parameter_schema_revision: String,
    /// The executions each sample averages, when more than one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_every: Option<u64>,
    /// The provenance of the selected model, when the run takes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_provenance: Option<String>,
    /// The digest of the selected model's simulation configuration, when the run takes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub simulation_config_revision: Option<String>,
    /// The feedback relay the run took, when it took one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_relay: Option<IsolatedFeedbackRelay>,
}

/// One sample of a run, expanded from its compact form (A7): its time, its sequence and each channel's value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IsolatedRunSample {
    /// The sample's time in seconds of the run.
    pub time_seconds: f64,
    /// The sequence value of the sample.
    pub sequence: u64,
    /// Each channel's value and quality, keyed by the channel's symbol.
    pub values: BTreeMap<String, IsolatedRunValue>,
}

/// A channel's value at one sample: `None` when there is no measurement.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IsolatedRunValue {
    /// The value, or `None` when it is missing.
    pub value: Option<f64>,
    /// The value's quality.
    pub quality: Quality,
}

/// What the answer to a status is evidence of (E8.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IsolatedRunEvidence<'a> {
    /// The job has not ended; there is no result yet.
    Pending,
    /// The job completed on its whole grid. Its evidence may validate (E8.4).
    Complete(&'a IsolatedRunRecord),
    /// The job was cancelled or failed after some complete samples. The prefix is evidence of what it recorded and
    /// never a completed validation (E8.4).
    Prefix(&'a IsolatedRunRecord),
    /// The job ended with complete samples, but this answer was asked without its run (a thin answer). Read the
    /// answer with its results to see the evidence.
    NotRequested,
    /// The job ended without a complete sample, so there is no run to read.
    NoRun,
}

impl IsolatedRunEvidence<'_> {
    /// Whether the evidence may validate: only a completed job's whole run may (E8.4).
    #[must_use]
    pub fn validates(self) -> bool {
        matches!(self, Self::Complete(_))
    }
}

impl IsolatedRunRequest {
    /// Checks the request against the limits and rules of E8.4, with E0.2 and E0.3, before it is sent.
    ///
    /// # Errors
    /// A limit or rule is broken; the failure names the field and the values involved.
    pub fn check(&self) -> Result<(), Failure> {
        request_checks::check_request(self)
    }
}

impl IsolatedRunStart {
    /// Checks the answer to a start: it names the request's identifier and the request's revision (E8.4).
    ///
    /// # Errors
    /// The job or the revision is not the request's.
    pub fn check(&self, request: &IsolatedRunRequest) -> Result<(), Failure> {
        if self.job_id != request.request_id
            || self.project_revision != request.expected_project_revision
        {
            return Err(refuse_answer(
                "Runtime returned an invalid verification identity.",
            ));
        }
        Ok(())
    }
}

impl IsolatedRunStatus {
    /// Checks the answer to a status or a cancel for `request` (E8.4): the job and the revision are the request's,
    /// the provenance is isolated, a thin answer carries no run, a run appears only once the job has ended, and a
    /// run's samples and conditions are the request's (E8.3). `include_results` is the flag the request sent.
    ///
    /// # Errors
    /// The answer does not answer the request, breaks a rule of E8.3 or E8.4, or carries a run the request does not
    /// describe; the failure names the rule.
    pub fn check(
        &self,
        request: &IsolatedRunRequest,
        include_results: Option<bool>,
    ) -> Result<(), Failure> {
        request.check()?;
        if self.job_id != request.request_id
            || self.project_revision != request.expected_project_revision
        {
            return Err(refuse_answer(&format!(
                "Verification identity or revision changed during execution. The runtime answered for job {} at revision {}, not for job {} at revision {}.",
                self.job_id, self.project_revision, request.request_id, request.expected_project_revision
            )));
        }
        if self.source_basis != SOURCE_BASIS || self.io_provenance != IO_PROVENANCE {
            return Err(refuse_answer(&format!(
                "Runtime returned untrusted verification evidence: its source basis is {} and its I/O provenance is {}.",
                self.source_basis, self.io_provenance
            )));
        }
        if include_results == Some(false) && self.run.is_some() {
            return Err(refuse_answer(
                "The runtime returned a run to a status that did not ask for results.",
            ));
        }
        if !self.status.is_terminal() && self.run.is_some() {
            return Err(refuse_answer(
                "The runtime returned a run before the job ended; no raw data is shared before the job is terminal.",
            ));
        }
        let Some(run) = &self.run else {
            // A thin answer of a completed job carries no run by design; an answer that asked for results must.
            if self.status == IsolatedJobState::Completed && include_results != Some(false) {
                return Err(refuse_answer(
                    "Runtime returned incomplete or untrusted verification evidence.",
                ));
            }
            return Ok(());
        };
        check_run(run, request, self.status)?;
        let captured = u64::try_from(run.samples.len()).unwrap_or(u64::MAX);
        if self.captured_sample_count != captured {
            return Err(refuse_answer(&format!(
                "The runtime counts {} captured samples, but its run holds {}.",
                self.captured_sample_count,
                run.samples.len()
            )));
        }
        Ok(())
    }

    /// What the answer is evidence of (E8.4): nothing while the job runs, the whole run when it completed, and a
    /// prefix when it was cancelled or failed after some complete samples. A prefix never validates.
    #[must_use]
    pub fn evidence(&self) -> IsolatedRunEvidence<'_> {
        match (self.status, self.run.as_ref()) {
            (IsolatedJobState::Queued | IsolatedJobState::Running, _) => {
                IsolatedRunEvidence::Pending
            }
            (IsolatedJobState::Completed, Some(run)) => IsolatedRunEvidence::Complete(run),
            (IsolatedJobState::Cancelled | IsolatedJobState::Failed, Some(run)) => {
                IsolatedRunEvidence::Prefix(run)
            }
            (_, None) if self.captured_sample_count > 0 => IsolatedRunEvidence::NotRequested,
            (_, None) => IsolatedRunEvidence::NoRun,
        }
    }
}

/// Decodes the answer to `control.simulation.run.status` or `control.simulation.run.cancel`, expanding the compact
/// samples of its run (A7) into the per-channel form before the record is read.
///
/// # Errors
/// The answer does not have the form of E8.4, or its run's samples are not compact samples of its channels.
pub fn decode_status(mut answer: Value) -> Result<IsolatedRunStatus, Failure> {
    if let Some(run) = answer.get_mut("run") {
        let expanded = expand_recorded_run_wire(run)?;
        *run = expanded;
    }
    crate::json::decode_value(answer).map_err(|_| Failure::invalid_response(STATUS_OPERATION))
}

/// The answer to `control.simulation.run.status` with `{}` (E8.2): whether an isolated run may start.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Readiness {
    ready: bool,
    #[serde(default)]
    project_revision: Option<String>,
    #[serde(default)]
    source_basis: Option<String>,
    #[serde(default)]
    execution: Option<String>,
    #[serde(default)]
    supports_request_id: Option<bool>,
    #[serde(default)]
    reason: Option<String>,
}

/// The project revision an isolated run starts on, from the runtime's readiness answer (E8.2). The runtime must support
/// recoverable identities, because Studio journals each run's identifier before the start (E0.2, E11.2).
///
/// # Errors
/// The runtime is not ready (its reason is the failure's text), or it does not support request identifiers, or its
/// answer does not have the form of E8.2.
pub fn ready_project_revision(answer: Value) -> Result<String, Failure> {
    let readiness: Readiness = crate::json::decode_value(answer)
        .map_err(|_| Failure::invalid_response(STATUS_OPERATION))?;
    let reason = readiness.reason.filter(|text| !text.is_empty());
    let revision = match readiness.project_revision.filter(|text| !text.is_empty()) {
        Some(revision)
            if readiness.ready
                && readiness.source_basis.as_deref() == Some(SOURCE_BASIS)
                && readiness.execution.as_deref() == Some(EXECUTION) =>
        {
            revision
        }
        _ => return Err(refuse_answer(reason.as_deref().unwrap_or(NOT_READY))),
    };
    if readiness.supports_request_id != Some(true) {
        return Err(refuse_answer(
            "Reload the runtime with recoverable isolated-operation identity support before verification.",
        ));
    }
    Ok(revision)
}

/// A duration in milliseconds as seconds, as the runtime writes event and duration times (E8.4).
#[allow(clippy::cast_precision_loss)] // Durations stay below 2^53 ms (E0.3, A7), so the conversion is exact.
fn seconds_of_ms(milliseconds: u64) -> f64 {
    milliseconds as f64 / 1000.0
}

/// An answer the runtime gave that does not answer the request, or does not hold to the protocol (E11.8).
fn refuse_answer(text: &str) -> Failure {
    Failure::new(text, Cause::Runtime, NextAction::None)
}

/// The run of a status: its identity and evidence, its conditions as the request froze them, its grid, and its samples
/// on that grid. A completed job holds every grid point; a job that ended early holds a prefix of them (E8.4).
fn check_run(
    run: &IsolatedRunRecord,
    request: &IsolatedRunRequest,
    status: IsolatedJobState,
) -> Result<(), Failure> {
    if run.id.is_empty() {
        return Err(refuse_answer("Runtime recording has no run identity."));
    }
    if run.evidence != RunEvidence::StExecuted {
        return Err(refuse_answer(evidence_refusal(status)));
    }
    if run.samples.is_empty() {
        return Err(refuse_answer(
            "The run holds no complete sample; a run that ended before its first sample has no run (E8.4).",
        ));
    }
    check_frozen_conditions(run, request)?;
    let grid = recorded_sample_times(request.step_ms, request.duration_ms, request.record_every);
    if run.conditions.sample_times_seconds != grid {
        return Err(refuse_answer(
            "Recorded run sample times do not match the frozen verification request.",
        ));
    }
    let complete = run.samples.len() == grid.len();
    if run.samples.len() > grid.len() || (status == IsolatedJobState::Completed && !complete) {
        return Err(refuse_answer(&format!(
            "The run holds {} samples; its time grid has {} points.",
            run.samples.len(),
            grid.len()
        )));
    }
    check_samples(run, &grid)
}

fn evidence_refusal(status: IsolatedJobState) -> &'static str {
    match status {
        IsolatedJobState::Completed => "Verification requires ST-executed runtime evidence.",
        IsolatedJobState::Cancelled => {
            "Terminal cancellation run must come from actual ST execution."
        }
        IsolatedJobState::Queued | IsolatedJobState::Running | IsolatedJobState::Failed => {
            "Failed-run evidence must come from actual ST execution."
        }
    }
}

/// The run's conditions are the request's frozen conditions (E8.3, E8.4): the same feedback relay, the same selected
/// model, the same duration, seed, initial values, events, channels, parameters and record period.
#[allow(clippy::float_cmp)] // The duration is compared as the runtime writes it: the same division of the same integer.
fn check_frozen_conditions(
    run: &IsolatedRunRecord,
    request: &IsolatedRunRequest,
) -> Result<(), Failure> {
    let conditions = &run.conditions;
    if conditions.feedback_relay != request.feedback_relay {
        return Err(refuse_answer(
            "Recorded feedback policy differs from the frozen acquisition request.",
        ));
    }
    if let Some(selected) = &request.simulation_override {
        let digest = format!(
            "sha256:{:x}",
            Sha256::digest(selected.simulation_toml.as_bytes())
        );
        if conditions.model_revision != selected.model_revision
            || conditions.model_provenance.as_deref() != Some(selected.model_provenance.as_str())
            || conditions.simulation_config_revision.as_deref() != Some(digest.as_str())
        {
            return Err(refuse_answer(
                "Recorded model or simulation configuration differs from the selected model.",
            ));
        }
    }
    let frozen = !conditions.program_revision.is_empty()
        && !conditions.model_revision.is_empty()
        && !conditions.parameter_schema_revision.is_empty()
        && conditions.duration_seconds == seconds_of_ms(request.duration_ms)
        && conditions.seed == request.seed
        && conditions.initial_state == request.initial_values
        && conditions.events == frozen_events(request)
        && run.channels == frozen_channels(request)
        && run.parameters == frozen_parameters(request);
    if !frozen {
        return Err(refuse_answer(
            "Recorded run does not match the frozen verification request.",
        ));
    }
    if conditions.record_every.unwrap_or(1) != request.record_every {
        return Err(refuse_answer(
            "Recorded run averages a different number of scans than the frozen verification request.",
        ));
    }
    Ok(())
}

fn frozen_events(request: &IsolatedRunRequest) -> Vec<RunEvent> {
    request
        .events
        .iter()
        .map(|event| RunEvent {
            at_seconds: seconds_of_ms(event.at_ms),
            channel: event.symbol.clone(),
            value: event.value,
        })
        .collect()
}

fn frozen_channels(request: &IsolatedRunRequest) -> Vec<ValidationChannel> {
    request
        .observations
        .iter()
        .map(|observation| ValidationChannel {
            id: observation.symbol.clone(),
            unit: observation.unit.clone(),
        })
        .collect()
}

fn frozen_parameters(request: &IsolatedRunRequest) -> BTreeMap<String, f64> {
    request
        .parameter_overrides
        .iter()
        .map(|parameter| (parameter.symbol.clone(), parameter.value))
        .collect()
}

/// Each sample lies on its grid point and holds one value for every observed channel (E8.3).
#[allow(clippy::float_cmp)] // A sample's time is compared exactly with its grid point (E8.3).
fn check_samples(run: &IsolatedRunRecord, grid: &[f64]) -> Result<(), Failure> {
    for (sample, time) in run.samples.iter().zip(grid) {
        if sample.time_seconds != *time {
            return Err(refuse_answer(
                "Recorded run sample times do not match the frozen verification request.",
            ));
        }
        let holds_every_channel = sample.values.len() == run.channels.len()
            && run
                .channels
                .iter()
                .all(|channel| sample.values.contains_key(&channel.id));
        if !holds_every_channel {
            return Err(refuse_answer(
                "A recorded sample does not hold one value for each observed channel.",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "isolated_run_protocol_tests.rs"]
mod tests;
