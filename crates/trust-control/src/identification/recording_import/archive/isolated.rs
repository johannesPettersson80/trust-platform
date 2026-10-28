//! The isolated-simulation runs of a saved result's engineer's study (specification 35, B10.4 and E8.4), read as
//! `decodeRuntimeVerificationResult` and `matchesFrozenRequest` read them in `engineerStudyPersistence.ts`. An isolated run is a
//! completed job of the runtime's simulation with its request frozen beside it. The decoder checks that the run answers that
//! request (the grid, the seed, the initial values, the events, the channels, the selected model when it took one), that its
//! evidence is the Structured Text's, and that its verdict is the verdict of its requirements evaluated again. A run that fails
//! these checks refuses the file as a whole (B10.4).
//!
//! The engine compares the verdict, the requirement results and the scope of the stored evaluation. It does not compare the copy of
//! the run that the stored evaluation holds, because the run itself is checked above.

#![allow(clippy::module_name_repetitions)] // The run names its concept: an isolated run.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::validation_value;
use crate::identification::connected_record::{
    RunConditions, RunEvent, RunEvidence, ValidationChannel, ValidationRun, ValidationSample,
    ValidationValue,
};
use crate::identification::independent_validation::RequirementStatus;
use crate::limits::{ISOLATED_SIMULATION_SCAN_LIMIT, MAXIMUM_EXPERIMENT_DURATION_MS};
use crate::loop_definition::LoopDefinition;
use crate::recording::opened_channels::binding_channel;
use crate::recording::{Completeness, RecordingFile};
use crate::validation::isolated_run_sampling::recorded_sample_times;
use crate::validation::run_validation::{
    evaluate_recorded_run, RunRequirement, RunRequirementResult,
};

/// The execution an isolated run's request names (E8.4).
pub const EXECUTION: &str = "isolated-simulation";
/// The keys of an isolated run record (`decodeRuntimeVerificationResult`).
const RUN_KEYS: [&str; 4] = ["projectRevision", "request", "run", "evaluation"];
/// The largest scan interval of an isolated run, in milliseconds (E8.4).
const MAXIMUM_STEP_MS: u64 = 1_000;
/// The most requirements a stored evaluation may hold (`decodeRuntimeVerificationResult`).
const MAXIMUM_REQUIREMENTS: usize = 64;

/// An isolated-simulation run of a saved result, checked against its frozen request and with its verdict evaluated again (B10.4).
#[derive(Debug, Clone)]
pub struct IsolatedRun {
    /// The program revision the run was taken on (`projectRevision`).
    pub project_revision: String,
    /// The run as the evaluation reads it.
    pub run: ValidationRun,
    /// The verdict of the run's requirements (N8.5).
    pub status: RequirementStatus,
    /// Whether the run took a selected model's plant (`simulation_override`). The run is read and checked, but the import does not
    /// archive it (`openedRecordingIdentification.ts`).
    pub selected_model: bool,
}

/// The run of a stored isolated record, as the file writes it. Its conditions hold the optional parts of a run (A4.2, E8.4).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredRun {
    id: String,
    evidence: RunEvidence,
    #[serde(default)]
    parameters: BTreeMap<String, f64>,
    conditions: StoredConditions,
    channels: Vec<ValidationChannel>,
    samples: Vec<ValidationSample>,
}

/// The conditions of a stored isolated run. The seed is the request's, which may be larger than a connected record's.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredConditions {
    initial_state: BTreeMap<String, f64>,
    events: Vec<RunEvent>,
    duration_seconds: f64,
    sample_times_seconds: Vec<f64>,
    seed: u64,
    model_revision: String,
    program_revision: String,
    parameter_schema_revision: String,
    #[serde(default)]
    record_every: Option<u64>,
    #[serde(default)]
    model_provenance: Option<String>,
    #[serde(default)]
    simulation_config_revision: Option<String>,
    #[serde(default)]
    feedback_relay: Option<Value>,
}

/// The stored verdict of an isolated run: its evaluation, of which the engine reads the verdict, the requirements, the scope and the
/// checks it was made with. The copy of the run it holds is not read.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredEvaluation {
    status: RequirementStatus,
    specification: Vec<RunRequirement>,
    requirements: Vec<RunRequirementResult>,
    scope: String,
}

/// The frozen request of an isolated run (`FrozenVerificationRequest`). The runtime's own identifier and limits are not checked
/// here, as `decodeRuntimeVerificationResult` does not check them.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
struct FrozenRequest {
    execution: String,
    expected_project_revision: String,
    step_ms: u64,
    duration_ms: u64,
    seed: u64,
    #[serde(default)]
    record_every: Option<u64>,
    initial_values: BTreeMap<String, f64>,
    events: Vec<FrozenEvent>,
    observations: Vec<FrozenObservation>,
    #[serde(default)]
    parameter_overrides: Vec<FrozenParameter>,
    #[serde(default)]
    simulation_override: Option<FrozenSelectedModel>,
    #[serde(default)]
    feedback_relay: Option<Value>,
}

/// A frozen event: at `at_ms` milliseconds the channel `symbol` is set to `value`.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
struct FrozenEvent {
    at_ms: f64,
    symbol: String,
    value: f64,
}

/// A frozen observed channel of the run.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
struct FrozenObservation {
    symbol: String,
    unit: String,
}

/// A frozen parameter override of the run.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
struct FrozenParameter {
    symbol: String,
    value: f64,
}

/// The selected model a run took as its plant (`simulation_override`).
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
struct FrozenSelectedModel {
    model_revision: String,
    model_provenance: String,
    simulation_toml: String,
}

/// Reads an isolated-simulation run record of a saved result, or `None` when it does not decode or does not answer its frozen
/// request (`decodeRuntimeVerificationResult`, B10.4).
#[must_use]
pub fn decode_isolated_run(value: &Value) -> Option<IsolatedRun> {
    let object = value.as_object()?;
    if object.keys().any(|key| !RUN_KEYS.contains(&key.as_str())) {
        return None;
    }
    let project_revision = object
        .get("projectRevision")?
        .as_str()
        .filter(|text| !text.is_empty())?;
    // Every typed decode goes through `crate::json`: the requirements are internally tagged and flattened, and the workspace's
    // serde_json reads fractional numbers through those types only in that way.
    let request: FrozenRequest = crate::json::decode_value(object.get("request")?.clone()).ok()?;
    let stored: StoredRun = crate::json::decode_value(object.get("run")?.clone()).ok()?;
    let verdict: StoredEvaluation =
        crate::json::decode_value(object.get("evaluation")?.clone()).ok()?;
    if stored.evidence != RunEvidence::StExecuted
        || !answers_request(&request, project_revision, &stored)
        || !(1..=MAXIMUM_REQUIREMENTS).contains(&verdict.specification.len())
    {
        return None;
    }
    // A run's seed is a connected record's `u32` (`RunConditions`); a larger seed is refused rather than truncated.
    let seed = u32::try_from(stored.conditions.seed).ok()?;
    let every = request.record_every.unwrap_or(1);
    let run = ValidationRun {
        id: stored.id,
        evidence: RunEvidence::StExecuted,
        parameters: stored.parameters,
        conditions: RunConditions {
            initial_state: stored.conditions.initial_state,
            events: stored.conditions.events,
            duration_seconds: stored.conditions.duration_seconds,
            sample_times_seconds: stored.conditions.sample_times_seconds,
            seed,
            model_revision: stored.conditions.model_revision,
            program_revision: stored.conditions.program_revision,
            parameter_schema_revision: stored.conditions.parameter_schema_revision,
        },
        channels: stored.channels,
        samples: stored.samples,
    };
    let fresh = evaluate_recorded_run(&run, &verdict.specification, every > 1).ok()?;
    if fresh.status != verdict.status
        || fresh.requirements != verdict.requirements
        || fresh.scope != verdict.scope
    {
        return None;
    }
    Some(IsolatedRun {
        project_revision: project_revision.to_string(),
        run,
        status: verdict.status,
        selected_model: request.simulation_override.is_some(),
    })
}

/// The isolated run a recording kept from an isolated simulation reads back as (B3.6, B5.2): its channels with their units, its
/// samples on their times, the model revision the file keeps and the program revision the run was taken on. The file keeps no seed,
/// initial values, events or parameters, and none is read here. Its verdict is the file's evidence: a complete recording passes, and
/// an incomplete one is inconclusive. `None` when the file names no program revision, or a binding names no channel.
#[must_use]
pub fn kept_isolated_run(
    loop_definition: &LoopDefinition,
    file: &RecordingFile,
) -> Option<IsolatedRun> {
    let model_revision = file.model_revision.clone()?;
    let project_revision = file
        .run
        .program_revision
        .clone()
        .filter(|text| !text.is_empty())?;
    let channels = file
        .run
        .bindings
        .iter()
        .map(|binding| binding_channel(loop_definition, binding))
        .collect::<Option<Vec<(String, String)>>>()?;
    let samples = file
        .run
        .samples
        .iter()
        .map(|sample| {
            let nanos = u64::try_from(sample.runtime_time_ns?).ok()?;
            let values: BTreeMap<String, ValidationValue> = channels
                .iter()
                .zip(sample.values.iter().zip(&sample.quality))
                .map(|((symbol, _), (value, quality))| {
                    (symbol.clone(), validation_value(*value, *quality))
                })
                .collect();
            Some(ValidationSample {
                time_seconds: Duration::from_nanos(nanos).as_secs_f64(),
                sequence: sample.scan,
                values,
            })
        })
        .collect::<Option<Vec<ValidationSample>>>()?;
    let sample_times_seconds: Vec<f64> = samples.iter().map(|sample| sample.time_seconds).collect();
    let run = ValidationRun {
        id: file.run.recording_id.clone(),
        evidence: RunEvidence::StExecuted,
        parameters: BTreeMap::new(),
        conditions: RunConditions {
            initial_state: BTreeMap::new(),
            events: Vec::new(),
            duration_seconds: sample_times_seconds.last().copied().unwrap_or(0.0),
            sample_times_seconds,
            seed: 1,
            model_revision,
            program_revision: project_revision.clone(),
            parameter_schema_revision: loop_definition.revision.clone(),
        },
        channels: channels
            .into_iter()
            .map(|(id, unit)| ValidationChannel { id, unit })
            .collect(),
        samples,
    };
    Some(IsolatedRun {
        project_revision,
        run,
        status: match file.evidence.completeness {
            Completeness::Complete => RequirementStatus::Pass,
            Completeness::Incomplete => RequirementStatus::Inconclusive,
        },
        selected_model: false,
    })
}

/// Whether the stored run answers its frozen request (`matchesFrozenRequest`): the grid, the seed, the initial values, the events,
/// the channels, the parameter overrides, the feedback relay and the selected model. The times and the duration are compared
/// exactly, as the TypeScript deep equality compares them.
#[allow(clippy::float_cmp, clippy::cast_precision_loss)] // Exact equality of the stored times; the duration is far below 2^53.
fn answers_request(request: &FrozenRequest, project_revision: &str, run: &StoredRun) -> bool {
    let every = request.record_every.unwrap_or(1);
    let (step, duration) = (request.step_ms, request.duration_ms);
    let conditions = &run.conditions;
    request.execution == EXECUTION
        && request.expected_project_revision == project_revision
        && (1..=MAXIMUM_STEP_MS).contains(&step)
        && (1..=MAXIMUM_EXPERIMENT_DURATION_MS).contains(&duration)
        && duration % step == 0
        && duration / step < ISOLATED_SIMULATION_SCAN_LIMIT
        && every >= 1
        && conditions.record_every.unwrap_or(1) == every
        && conditions.seed == request.seed
        && conditions.duration_seconds == duration as f64 / 1000.0
        && conditions.sample_times_seconds == recorded_sample_times(step, duration, every)
        && conditions.initial_state == request.initial_values
        && conditions.events
            == request
                .events
                .iter()
                .map(|event| RunEvent {
                    at_seconds: event.at_ms / 1000.0,
                    channel: event.symbol.clone(),
                    value: event.value,
                })
                .collect::<Vec<_>>()
        && run.channels
            == request
                .observations
                .iter()
                .map(|observation| ValidationChannel {
                    id: observation.symbol.clone(),
                    unit: observation.unit.clone(),
                })
                .collect::<Vec<_>>()
        && run.parameters
            == request
                .parameter_overrides
                .iter()
                .map(|parameter| (parameter.symbol.clone(), parameter.value))
                .collect::<BTreeMap<_, _>>()
        && conditions.feedback_relay.clone().unwrap_or(Value::Null)
            == request.feedback_relay.clone().unwrap_or(Value::Null)
        && selected_model_answers(request.simulation_override.as_ref(), conditions)
}

/// Whether the run's model is the selected model its request names, by revision, provenance and the digest of its simulation
/// configuration (`matchesSimulationOverride`). A run without a selected model answers any.
fn selected_model_answers(
    model: Option<&FrozenSelectedModel>,
    conditions: &StoredConditions,
) -> bool {
    model.is_none_or(|model| {
        conditions.model_revision == model.model_revision
            && conditions.model_provenance.as_deref() == Some(model.model_provenance.as_str())
            && conditions.simulation_config_revision.as_deref()
                == Some(simulation_digest(&model.simulation_toml).as_str())
    })
}

/// The digest of a simulation configuration as the runtime names it: `sha256:` and the hexadecimal SHA-256 of its text.
fn simulation_digest(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    let mut value = String::from("sha256:");
    for byte in &digest {
        let _ = write!(value, "{byte:02x}");
    }
    value
}
