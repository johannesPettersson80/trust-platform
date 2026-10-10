//! The status of an experiment as the runtime reports it, and the stored recovery a reopened panel reattaches to
//! (`experiment.ts`, `classifyControlExperimentStatus` and `decodeControlExperimentRecovery`; specification 35,
//! A8.5 and A8.6; annex E5.6).
//!
//! The runtime's answers and the stored recovery are untyped JSON, so both are checked over the JSON the way
//! the extension checks them. A stored plan is decoded into [`ControlExperimentPlan`] only after every check
//! holds.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::LoopDefinition;

use super::plan::{ControlExperimentPlan, PlanSource};

/// The largest integer JSON carries exactly: 2⁵³ − 1 (E0.3).
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
/// The shortest and longest experiment, in ms (A7).
const MINIMUM_DURATION_MS: f64 = 1_000.0;
const MAXIMUM_DURATION_MS: f64 = 86_400_000.0;
/// The most trajectory points and the most stop conditions a stored plan keeps.
const MAXIMUM_TRAJECTORY_POINTS: usize = 72_000;
const MAXIMUM_STOP_CONDITIONS: usize = 16;
const MAXIMUM_FREQUENCIES: usize = 64;
const MAXIMUM_BINDINGS: usize = 32;

/// The kinds a stored plan may carry: the kinds of the plan and the legacy alias (`decodeRecoveryPlan`).
const PLAN_KINDS: [&str; 10] = [
    "observation",
    "setpoint_step",
    "load_step",
    "mv_step",
    "mv_pulse",
    "relay",
    "prbs",
    "multisine",
    "output_step",
    "project_sequence",
];

/// The keys a stored plan must carry, and the keys it may carry as well (`decodeRecoveryPlan`). `stopOnStaleMs`
/// is accepted and ignored, as plans journaled before the runtime owned staleness carry it.
const PLAN_REQUIRED_KEYS: [&str; 16] = [
    "id",
    "source",
    "kind",
    "affectedSignal",
    "valueType",
    "from",
    "to",
    "unit",
    "minimum",
    "maximum",
    "durationMs",
    "expectedScan",
    "stopConditions",
    "restorationValue",
    "requiresPhysicalApproval",
    "trajectory",
];
const PLAN_OPTIONAL_KEYS: [&str; 9] = [
    "address",
    "abortBinding",
    "relayFeedback",
    "intervalMs",
    "seed",
    "frequenciesHz",
    "task",
    "modeBinding",
    "stopOnStaleMs",
];

/// The sorted keys of a version-1 recovery and of a version-2 recovery.
const RECOVERY_V1_KEYS: [&str; 5] = [
    "experimentId",
    "loopId",
    "loopRevision",
    "projectId",
    "schemaVersion",
];
const RECOVERY_V2_KEYS: [&str; 7] = [
    "experimentId",
    "loopId",
    "loopRevision",
    "plan",
    "projectId",
    "recordingOwner",
    "schemaVersion",
];

/// Where a running experiment stands, as the editor reads the runtime's status (`ControlExperimentRuntimePhase`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ControlExperimentRuntimePhase {
    /// The experiment runs and its restoration is not yet due.
    Running,
    /// The experiment has ended and its restoration is pending: keep polling.
    AwaitingRestoration,
    /// The experiment has ended and its restoration is settled.
    Terminal,
    /// The answer does not describe a status of the runtime's vocabulary.
    Invalid,
}

/// The restoration of an experiment (E5.6): its restoring write is queued and not yet confirmed, none was queued, it is
/// confirmed, or it failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestorationState {
    /// The restoring write is queued and not yet confirmed.
    Pending,
    /// No restoring write was queued.
    NotRequired,
    /// The restoration is confirmed.
    Restored,
    /// The restoration failed.
    Failed,
}

/// The phase of an experiment from the runtime's status answer (`classifyControlExperimentStatus`, annex E5.6).
/// A running experiment restores nothing yet; an ended one keeps being polled until its restoration is confirmed.
#[must_use]
pub fn classify_control_experiment_status(value: &Value) -> ControlExperimentRuntimePhase {
    let Some(object) = value.as_object() else {
        return ControlExperimentRuntimePhase::Invalid;
    };
    let status = object.get("status").and_then(Value::as_str);
    let restoration = object.get("restoration").and_then(Value::as_str);
    if status == Some("running") && matches!(restoration, Some("pending" | "not_required")) {
        return ControlExperimentRuntimePhase::Running;
    }
    if matches!(status, Some("completed" | "aborted" | "failed")) {
        return match restoration {
            Some("pending") => ControlExperimentRuntimePhase::AwaitingRestoration,
            Some("restored" | "not_required") => ControlExperimentRuntimePhase::Terminal,
            _ => ControlExperimentRuntimePhase::Invalid,
        };
    }
    if status == Some("failed_to_restore") && restoration == Some("failed") {
        ControlExperimentRuntimePhase::Terminal
    } else {
        ControlExperimentRuntimePhase::Invalid
    }
}

/// The loop an experiment recovery must belong to (`ControlExperimentRecoveryScope`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlExperimentRecoveryScope {
    /// The project's identifier.
    pub project_id: String,
    /// The loop's identifier.
    pub loop_id: String,
    /// The loop's revision.
    pub loop_revision: String,
}

/// Whether a recording's values come from a simulated or a physical target (`ControlEvidenceSource` of the
/// monitor, which the recording owner copies). From loopMonitor.ts; moves with the runtime-recording port (V1.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ControlEvidenceSource {
    /// A simulated target.
    Simulation,
    /// A physical target.
    Physical,
}

/// One binding of a recording: the signal it reads, by identifier, address or symbol (`RecordingRecoveryOwner`'s
/// `bindings`). From runtimeRecording.ts; moves with the runtime-recording port (V1.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RecordingRecoveryBinding {
    /// The binding's identifier in the recording.
    pub id: String,
    /// The signal's address, when it reads by address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// The signal's symbol, when it reads by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// The signal's IEC type.
    pub iec_type: String,
}

/// The persistable identity of a recording a reopened panel stops, with no credentials and no data
/// (`RecordingRecoveryOwner`). From runtimeRecording.ts; moves with the runtime-recording port (V1.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecordingRecoveryOwner {
    /// Always 1.
    pub schema_version: u32,
    /// The recording's identifier on the runtime.
    pub recording_id: String,
    /// The project the recording belongs to.
    pub project_id: String,
    /// The runtime the recording runs on.
    pub runtime_id: String,
    /// The loop the recording belongs to.
    pub loop_id: String,
    /// The loop's revision.
    pub loop_revision: String,
    /// Whether the recording is simulated or physical.
    pub source: ControlEvidenceSource,
    /// The program revision the recording was started against, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program_revision: Option<String>,
    /// Set when the start reached the runtime but its acknowledgement was lost; only `true` is stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_acknowledgement_unconfirmed: Option<bool>,
    /// The signals the recording reads.
    pub bindings: Vec<RecordingRecoveryBinding>,
}

/// Checks that a recording owner belongs to the loop it is recovered for (`RuntimeRecordingSession.fromRecoveryOwner` of
/// `runtimeRecording.ts`): the same project, runtime, loop and revision, an identifier, and 1 to 32 bindings.
///
/// # Errors
/// The owner is outside the scope of the loop.
pub fn check_recording_owner_scope(
    owner: &RecordingRecoveryOwner,
    loop_definition: &LoopDefinition,
) -> Result<(), Failure> {
    let in_scope = owner.project_id == loop_definition.project_id
        && owner.runtime_id == loop_definition.runtime_id
        && owner.loop_id == loop_definition.id
        && owner.loop_revision == loop_definition.revision
        && !owner.recording_id.is_empty()
        && (1..=MAXIMUM_BINDINGS).contains(&owner.bindings.len());
    if in_scope {
        return Ok(());
    }
    Err(Failure::new(
        "Recording recovery owner does not match the active loop scope.",
        Cause::Implementation,
        NextAction::None,
    ))
}

/// A stored recovery of an experiment (`ControlExperimentRecovery`): the experiment's identity in its loop and,
/// from schema version 2, its reviewed plan and the recording it runs with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ControlExperimentRecovery {
    /// 1 for an experiment with no plan on record; 2 when the plan and its recording are kept.
    pub schema_version: u8,
    /// The experiment's identifier.
    pub experiment_id: String,
    /// The project the experiment belongs to.
    pub project_id: String,
    /// The loop the experiment belongs to.
    pub loop_id: String,
    /// The loop's revision the experiment was reviewed against.
    pub loop_revision: String,
    /// The reviewed plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<ControlExperimentPlan>,
    /// The recording the experiment runs with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_owner: Option<RecordingRecoveryOwner>,
}

/// What a stored recovery yields once it is accepted: the experiment's identifier, and for version 2 its plan and
/// recording (the `Pick` of `decodeControlExperimentRecovery`).
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedControlExperimentRecovery {
    /// The experiment's identifier.
    pub experiment_id: String,
    /// The reviewed plan, for version 2.
    pub plan: Option<ControlExperimentPlan>,
    /// The recording the experiment runs with, for version 2.
    pub recording_owner: Option<RecordingRecoveryOwner>,
}

/// Accepts only a stored recovery whose identity is bound to the exact loop revision being reopened
/// (`decodeControlExperimentRecovery`). Version 1 keeps the identifier only; version 2 keeps the plan and its
/// recording, which must name the same experiment and the source of the plan.
#[must_use]
pub fn decode_control_experiment_recovery(
    value: &Value,
    scope: &ControlExperimentRecoveryScope,
) -> Option<DecodedControlExperimentRecovery> {
    let object = value.as_object()?;
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    let version = object.get("schemaVersion").and_then(Value::as_u64);
    if version == Some(1) && keys == RECOVERY_V1_KEYS {
        if !scope_matches(object, scope) || !is_text(object.get("experimentId")) {
            return None;
        }
        return Some(DecodedControlExperimentRecovery {
            experiment_id: text_of(object, "experimentId"),
            plan: None,
            recording_owner: None,
        });
    }
    if version != Some(2) || keys != RECOVERY_V2_KEYS {
        return None;
    }
    if !scope_matches(object, scope) || !is_text(object.get("experimentId")) {
        return None;
    }
    let plan = decode_recovery_plan(object.get("plan")?)?;
    let recording_owner = decode_recovery_owner(object.get("recordingOwner")?, scope)?;
    let experiment_id = text_of(object, "experimentId");
    if plan.id != experiment_id || recording_owner.source != owner_source(plan.source) {
        return None;
    }
    Some(DecodedControlExperimentRecovery {
        experiment_id,
        plan: Some(plan),
        recording_owner: Some(recording_owner),
    })
}

/// The recording source a plan's target implies: a simulated plan records a simulated target.
fn owner_source(source: PlanSource) -> ControlEvidenceSource {
    match source {
        PlanSource::Simulation => ControlEvidenceSource::Simulation,
        PlanSource::Physical => ControlEvidenceSource::Physical,
    }
}

fn scope_matches(
    object: &serde_json::Map<String, Value>,
    scope: &ControlExperimentRecoveryScope,
) -> bool {
    text_is(object.get("projectId"), &scope.project_id)
        && text_is(object.get("loopId"), &scope.loop_id)
        && text_is(object.get("loopRevision"), &scope.loop_revision)
}

/// Checks a stored plan against the rules of its stored form and decodes it (`decodeRecoveryPlan`): every
/// required key is present, no other key is unknown, each value is in range, and the trajectory lies within the
/// duration.
fn decode_recovery_plan(value: &Value) -> Option<ControlExperimentPlan> {
    let object = value.as_object()?;
    if !PLAN_REQUIRED_KEYS
        .iter()
        .all(|key| object.contains_key(*key))
        || object.keys().any(|key| {
            !PLAN_REQUIRED_KEYS.contains(&key.as_str())
                && !PLAN_OPTIONAL_KEYS.contains(&key.as_str())
        })
    {
        return None;
    }
    let get = |key: &str| object.get(key);
    let source_ok = matches!(
        get("source").and_then(Value::as_str),
        Some("simulation" | "physical")
    );
    let kind_ok = get("kind")
        .and_then(Value::as_str)
        .is_some_and(|kind| PLAN_KINDS.contains(&kind));
    if !(is_text(get("id"))
        && source_ok
        && kind_ok
        && is_text(get("affectedSignal"))
        && is_text(get("valueType"))
        && is_text(get("unit"))
        && optional_text(get("address"))
        && optional_text(get("task"))
        && optional_mode_binding(get("modeBinding"))
        && number_or_boolean(get("from"))
        && number_or_boolean(get("to")))
    {
        return None;
    }
    let minimum = finite(get("minimum"))?;
    let maximum = finite(get("maximum"))?;
    if minimum > maximum {
        return None;
    }
    let duration_ms = safe_integer(get("durationMs"))?;
    if !(MINIMUM_DURATION_MS..=MAXIMUM_DURATION_MS).contains(&duration_ms) {
        return None;
    }
    if safe_integer(get("expectedScan"))? < 0.0 {
        return None;
    }
    let stop_conditions = get("stopConditions")?.as_array()?;
    if stop_conditions.is_empty()
        || stop_conditions.len() > MAXIMUM_STOP_CONDITIONS
        || !stop_conditions.iter().all(|text| is_text(Some(text)))
    {
        return None;
    }
    if !number_or_boolean(get("restorationValue")) || !get("requiresPhysicalApproval")?.is_boolean()
    {
        return None;
    }
    let trajectory = get("trajectory")?.as_array()?;
    if trajectory.len() > MAXIMUM_TRAJECTORY_POINTS
        || !trajectory
            .iter()
            .all(|point| trajectory_point_ok(point, duration_ms))
    {
        return None;
    }
    if !optional_abort_binding(get("abortBinding"))
        || !optional_relay_feedback(get("relayFeedback"))
        || !optional_positive_integer(get("intervalMs"), f64::INFINITY)
        || !optional_positive_integer(get("seed"), 4_294_967_295.0)
        || !optional_frequencies(get("frequenciesHz"))
    {
        return None;
    }
    crate::json::decode_value::<ControlExperimentPlan>(value.clone()).ok()
}

/// A trajectory point: an integer time within the duration and a finite value.
fn trajectory_point_ok(point: &Value, duration_ms: f64) -> bool {
    let Some(object) = point.as_object() else {
        return false;
    };
    safe_integer(object.get("atMs")).is_some_and(|at_ms| at_ms >= 0.0 && at_ms <= duration_ms)
        && finite(object.get("value")).is_some()
}

/// The recording a stored plan runs with (`decodeRecoveryRecordingOwner`): version 1, the scope, text fields, a
/// source, bindings of one to 32 signals that each name an identifier and a type and one selector.
fn decode_recovery_owner(
    value: &Value,
    scope: &ControlExperimentRecoveryScope,
) -> Option<RecordingRecoveryOwner> {
    let object = value.as_object()?;
    let bindings = object.get("bindings")?.as_array()?;
    let source_ok = matches!(
        object.get("source").and_then(Value::as_str),
        Some("Simulation" | "Physical")
    );
    let unconfirmed_ok = match object.get("startAcknowledgementUnconfirmed") {
        None | Some(Value::Bool(true)) => true,
        Some(_) => false,
    };
    if object.get("schemaVersion").and_then(Value::as_u64) != Some(1)
        || !is_text(object.get("recordingId"))
        || !scope_matches(object, scope)
        || !is_text(object.get("runtimeId"))
        || !source_ok
        || !optional_text(object.get("programRevision"))
        || !unconfirmed_ok
        || bindings.is_empty()
        || bindings.len() > MAXIMUM_BINDINGS
        || !bindings.iter().all(recording_binding_ok)
    {
        return None;
    }
    crate::json::decode_value::<RecordingRecoveryOwner>(value.clone()).ok()
}

fn recording_binding_ok(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let address = object.get("address");
    let symbol = object.get("symbol");
    is_text(object.get("id"))
        && is_text(object.get("iec_type"))
        && optional_text(address)
        && optional_text(symbol)
        && (address.is_some() || symbol.is_some())
}

/// The optional mode binding of a stored plan: absent, or an object with a symbol and a Boolean Manual value.
fn optional_mode_binding(value: Option<&Value>) -> bool {
    match value {
        None => true,
        Some(mode) => mode.as_object().is_some_and(|object| {
            is_text(object.get("symbol"))
                && object.get("manualValue").is_some_and(Value::is_boolean)
        }),
    }
}

/// The optional abort binding of a stored plan: absent, or an object with a symbol, an address, a type and a
/// Boolean asserted value.
fn optional_abort_binding(value: Option<&Value>) -> bool {
    match value {
        None => true,
        Some(abort) => abort.as_object().is_some_and(|object| {
            is_text(object.get("symbol"))
                && is_text(object.get("address"))
                && is_text(object.get("valueType"))
                && object.get("assertedValue").is_some_and(Value::is_boolean)
        }),
    }
}

/// The optional relay of a stored plan: absent, or the measurement's identity, finite thresholds with a positive
/// hysteresis, and a lower threshold below the upper one.
fn optional_relay_feedback(value: Option<&Value>) -> bool {
    match value {
        None => true,
        Some(relay) => relay.as_object().is_some_and(|object| {
            ["processVariable", "address", "valueType", "unit"]
                .iter()
                .all(|key| is_text(object.get(*key)))
                && [
                    "center",
                    "hysteresis",
                    "lowerThreshold",
                    "upperThreshold",
                    "lowOutput",
                    "highOutput",
                ]
                .iter()
                .all(|key| finite(object.get(*key)).is_some())
                && finite(object.get("hysteresis")).is_some_and(|hysteresis| hysteresis > 0.0)
                && finite(object.get("lowerThreshold")).is_some_and(|lower| {
                    finite(object.get("upperThreshold")).is_some_and(|upper| lower < upper)
                })
        }),
    }
}

/// An optional whole number above zero, at most `maximum` (the interval of a pseudo-random sequence, the seed).
fn optional_positive_integer(value: Option<&Value>, maximum: f64) -> bool {
    match value {
        None => true,
        Some(_) => safe_integer(value).is_some_and(|number| number > 0.0 && number <= maximum),
    }
}

/// The optional frequencies of a multisine: between one and 64 finite numbers.
fn optional_frequencies(value: Option<&Value>) -> bool {
    match value {
        None => true,
        Some(frequencies) => frequencies.as_array().is_some_and(|items| {
            !items.is_empty()
                && items.len() <= MAXIMUM_FREQUENCIES
                && items.iter().all(|item| finite(Some(item)).is_some())
        }),
    }
}

/// A non-empty text (`text` of the extension's checks).
fn is_text(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .is_some_and(|text| !text.is_empty())
}

/// A text, or absent: the value of a key that may be left out, but never `null`.
fn optional_text(value: Option<&Value>) -> bool {
    value.is_none() || is_text(value)
}

fn text_is(value: Option<&Value>, expected: &str) -> bool {
    value.and_then(Value::as_str) == Some(expected)
}

fn text_of(object: &serde_json::Map<String, Value>, key: &str) -> String {
    object
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// A finite number.
fn finite(value: Option<&Value>) -> Option<f64> {
    value?.as_f64().filter(|number| number.is_finite())
}

/// A whole number of safe magnitude, as JSON carries it exactly (`Number.isSafeInteger`).
fn safe_integer(value: Option<&Value>) -> Option<f64> {
    let number = finite(value)?;
    (is_whole(number) && number.abs() <= MAX_SAFE_INTEGER).then_some(number)
}

/// Whether a number has no fraction.
#[allow(clippy::float_cmp)] // A whole number is exactly its integer part.
fn is_whole(number: f64) -> bool {
    number.trunc() == number
}

/// A number, or a Boolean (`numberOrBoolean`).
fn number_or_boolean(value: Option<&Value>) -> bool {
    value.is_some_and(|value| value.is_boolean() || finite(Some(value)).is_some())
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
