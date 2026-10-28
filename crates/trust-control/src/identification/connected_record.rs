//! The connected identification record (`connectedIdentificationRecord.ts`; specification 35, A4.4, A9, A11.1, A11.2 and
//! B3.4; the steady baseline window of N1.3 in `control-studio-numerics.md`; runtime protocol annex E4.4 and E4.5).
//!
//! Identify fits a connected loop from the runtime's own recording. The record keeps the whole recording, the acquisition
//! evidence that describes it and the reviewed plan. Its times come from the runtime's scan times and its values are the
//! ones the runtime recorded: nothing is replayed or resampled. The baseline start and end tell the fit where the steady
//! window lies (B3.4, N1.3), so the wait before the window never sets the operating point.
//!
//! The evaluation of the run is the caller's: the production one is the limits check of `recorded_limits` (B3.4 step 6),
//! and the requirement evaluation of N8 is stage V3. The builder and the decoder take it as a parameter, so the record is
//! built and checked with whichever evaluator the caller supplies.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::experiment::plan::{ControlExperimentPlan, PlanSource};
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::campaign::plant_input_range;
use crate::loop_definition::LoopDefinition;
use crate::recording::channels::{
    applied_mv, binding_forced, binding_value, capture_additional_channels, process_value,
    sample_quality, ChannelQuality, SETPOINT_BINDING,
};
use crate::recording::evidence::{recording_evidence, RecordingEvidence};
use crate::recording::samples::{invalid_samples, seconds_between};
use crate::recording::{Completeness, RecordedRun, RecordedSample, ScanRange};

/// The acquisition method of a recording taken from the runtime's scan buffer (annex E4.5).
const RUNTIME_SCAN_BUFFER: &str = "runtime_scan_buffer";

/// The execution of a connected record: the connected runtime ran the experiment (`execution`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ConnectedExecution {
    /// The connected runtime ran the experiment.
    ConnectedRuntime,
}

/// The acquisition of a connected record: the runtime's acquisition evidence (annex E4.5) with the identity of its recording
/// and its program revision. The record stores them as one flat object, as the `TypeScript` evidence (`LoopRecordingEvidence`)
/// writes them. The acquisition fields repeat those of `Acquisition`, because serde does not support `deny_unknown_fields`
/// together with `flatten`, and the stored object refuses unknown fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectedAcquisition {
    /// The identifier of the recording, which is also the run's identifier.
    pub recording_id: String,
    /// The program revision the recording ran on.
    pub program_revision: String,
    /// Always `runtime_scan_buffer` for a complete connected record (annex E4.5).
    pub method: String,
    /// Complete or incomplete: the verdict of the recording (A4.4, annex E4.5).
    pub completeness: Completeness,
    /// The executions observed.
    pub observed_sample_count: u64,
    /// The sequence range covered.
    pub scan_range: Option<ScanRange>,
    /// The gaps.
    pub missing_scan_ranges: Vec<ScanRange>,
    /// Always 0.
    pub dropped_observed_samples: u64,
    /// Whether the actual cycle timing is known.
    pub actual_cycle_timing_available: bool,
    /// Always false.
    pub freshness_available: bool,
    /// Why the recording is incomplete, or empty.
    pub reason: String,
}

/// The request a connected record was taken with (`request`): its execution, the loop revision, the acquisition, the reviewed
/// plan and the baseline window. The baseline fields are written only when they apply (B3.4, N1.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConnectedRequest {
    /// Always the connected runtime.
    pub execution: ConnectedExecution,
    /// The loop revision the recording was taken under.
    pub loop_revision: String,
    /// The acquisition the run was taken from.
    pub acquisition: ConnectedAcquisition,
    /// The reviewed plan of the experiment.
    pub experiment: ControlExperimentPlan,
    /// The reviewed plan of each plant input, in campaign order; written only when a campaign stepped several inputs (C7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experiments: Option<Vec<ControlExperimentPlan>>,
    /// The start of the steady baseline window, in s; written only when it is after the recording's start (B3.4).
    #[serde(
        rename = "baselineStartSeconds",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub baseline_start_seconds: Option<f64>,
    /// The end of the steady baseline window, in s; written only when it is after its start (N1.3).
    #[serde(
        rename = "baselineEndSeconds",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub baseline_end_seconds: Option<f64>,
}

/// A connected record (`RuntimeVerificationResult`): the project revision, the request it was taken with, the run and its
/// evaluation. `E` is the evaluation the caller's evaluator makes (stage V3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeVerificationResult<E> {
    /// The program revision the run was recorded on.
    pub project_revision: String,
    /// The request the record was taken with.
    pub request: ConnectedRequest,
    /// The run the evaluation is made on.
    pub run: ValidationRun,
    /// The evaluation of the run against its checks.
    pub evaluation: E,
}

// From runValidation.ts; moves with V3.
/// Where a run's values come from (`RunEvidence`): a model, the ST program or a physical target (A9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum RunEvidence {
    /// Predicted by a model.
    #[serde(rename = "model-predicted")]
    ModelPredicted,
    /// Executed by the ST program.
    #[serde(rename = "ST-executed")]
    StExecuted,
    /// Recorded from a physical target.
    #[serde(rename = "physical-recorded")]
    PhysicalRecorded,
}

// From runValidation.ts; moves with V3.
/// A run as the evaluator reads it (`RecordedRun`): its samples keyed by signal symbol, and the conditions it was taken under.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidationRun {
    /// The run's identifier, the recording's.
    pub id: String,
    /// Where its values come from.
    pub evidence: RunEvidence,
    /// Its numeric parameters; none in a connected record.
    pub parameters: BTreeMap<String, f64>,
    /// The conditions it was taken under.
    pub conditions: RunConditions,
    /// Its channels, one per signal in the record's order, named by symbol.
    pub channels: Vec<ValidationChannel>,
    /// Its samples, in order.
    pub samples: Vec<ValidationSample>,
}

// From runValidation.ts; moves with V3.
/// The conditions a run was taken under (`RunConditions`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunConditions {
    /// The values the signals had at the first sample, where they were readable, by symbol.
    pub initial_state: BTreeMap<String, f64>,
    /// The events during the run; none in a recording.
    pub events: Vec<RunEvent>,
    /// The duration of the run, in s: the time of its last sample.
    pub duration_seconds: f64,
    /// The time of each sample, in s.
    pub sample_times_seconds: Vec<f64>,
    /// The seed of the experiment; 1 when the plan has none.
    pub seed: u32,
    /// The model the run is compared with: `connected-plant:` and the program revision.
    pub model_revision: String,
    /// The program revision of the run.
    pub program_revision: String,
    /// The loop revision the parameter schema belongs to.
    pub parameter_schema_revision: String,
}

// From runValidation.ts; moves with V3.
/// An event of a run: a channel is set to a value at a time (`RunConditions.events`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunEvent {
    /// The time of the event, in s.
    pub at_seconds: f64,
    /// The channel the event sets, by symbol.
    pub channel: String,
    /// The value it sets.
    pub value: f64,
}

// From runValidation.ts; moves with V3.
/// A channel of a run: a signal's symbol and unit (`RecordedRun.channels`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidationChannel {
    /// The signal's symbol.
    pub id: String,
    /// The signal's unit.
    pub unit: String,
}

// From runValidation.ts; moves with V3.
/// One sample of a run: its time, its sequence value and each channel's value and quality, keyed by symbol.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidationSample {
    /// The sample's time, in s from the first runtime scan.
    pub time_seconds: f64,
    /// The last scan sequence value the sample covers (annex E4.4).
    pub sequence: u64,
    /// Each channel's value and quality at the sample, keyed by the signal's symbol.
    pub values: BTreeMap<String, ValidationValue>,
}

// From runValidation.ts; moves with V3.
/// A channel's value and quality at one sample. The value is `None` when it is missing (`null`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidationValue {
    /// The value, or `None` when it is missing.
    pub value: Option<f64>,
    /// The value's quality (A4.2).
    pub quality: ChannelQuality,
}

// From runValidation.ts; moves with V3.
/// A bounds requirement of a connected record: the channel's values in the window stay between the minimum and the maximum
/// (`kind: "bounds"`, identified as `bounds:<symbol>`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoundsCheck {
    /// The requirement's identifier: `bounds:` and the channel's symbol.
    pub id: String,
    /// Always `bounds`.
    pub kind: BoundsKind,
    /// The channel the bounds apply to, by symbol.
    pub channel: String,
    /// The window the bounds apply in, in s of the run.
    pub window: BoundsWindow,
    /// The smallest allowed value.
    pub minimum: f64,
    /// The largest allowed value.
    pub maximum: f64,
}

// From runValidation.ts; moves with V3.
/// The kind of a requirement a connected record checks: only `bounds`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum BoundsKind {
    /// The values stay between the minimum and the maximum.
    Bounds,
}

// From runValidation.ts; moves with V3.
/// The time window of a requirement, in s of the run.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoundsWindow {
    /// The start of the window.
    pub start: f64,
    /// The end of the window.
    pub end: f64,
}

/// Builds the connected record of a recording (`connectedIdentificationRecord`): the run of its samples as the evaluator reads
/// it, the request it was taken with, and the evaluation `evaluate` makes of the run.
///
/// The recording must be complete runtime evidence (A4.4, annex E4.5). `start_failure` is the reason the start was not
/// confirmed, when it was not: the verdict then stays incomplete and the stopped data stays evidence. Times start at the first
/// runtime scan. The plant input is checked against the output limits and the measured output against its range (B3.4 step
/// 6); the other recorded signals are kept but not checked, as the specification names only these two limits.
/// `baseline_start_seconds` is written when it is after 0 s, and
/// `baseline_end_seconds` when it is after the start (B3.4, N1.3).
///
/// # Errors
/// The recording is not complete runtime evidence, which refuses identification (A11.2); the refusal keeps the verdict's reason
/// (A11.1). A sample has no runtime time. An output limit is missing or not ordered, so the plant input cannot be checked
/// (B3.9). `evaluate` refuses the run.
pub fn connected_identification_record<E>(
    loop_definition: &LoopDefinition,
    run: &RecordedRun,
    start_failure: Option<&str>,
    plan: &ControlExperimentPlan,
    baseline_start_seconds: f64,
    baseline_end_seconds: Option<f64>,
    evaluate: impl FnOnce(&ValidationRun, &[BoundsCheck]) -> Result<E, Failure>,
) -> Result<RuntimeVerificationResult<E>, Failure> {
    let verdict = recording_evidence(run, start_failure);
    let Some(program_revision) = complete_program_revision(run, &verdict) else {
        return Err(incomplete_recording(&verdict.reason));
    };
    let origin = run
        .samples
        .first()
        .and_then(|sample| sample.runtime_time_ns)
        .ok_or_else(invalid_samples)?;
    let samples = run
        .samples
        .iter()
        .map(|sample| validation_sample(run, loop_definition, origin, sample))
        .collect::<Result<Vec<_>, _>>()?;
    let (Some(first), Some(last)) = (samples.first(), samples.last()) else {
        return Err(incomplete_recording(&verdict.reason));
    };
    let initial_state = initial_state_of(first);
    let duration_seconds = last.time_seconds;
    let sample_times_seconds: Vec<f64> = samples.iter().map(|sample| sample.time_seconds).collect();
    let run_record = ValidationRun {
        id: run.recording_id.clone(),
        evidence: run_evidence(plan.source),
        parameters: BTreeMap::new(),
        conditions: RunConditions {
            initial_state,
            events: Vec::new(),
            duration_seconds,
            sample_times_seconds,
            seed: plan.seed.unwrap_or(1),
            model_revision: format!("connected-plant:{program_revision}"),
            program_revision: program_revision.to_string(),
            parameter_schema_revision: loop_definition.revision.clone(),
        },
        channels: validation_channels(loop_definition),
        samples,
    };
    let checks = bounds_checks(loop_definition, duration_seconds)?;
    let evaluation = evaluate(&run_record, checks.as_slice())?;
    let baseline_start = (baseline_start_seconds > 0.0).then_some(baseline_start_seconds);
    let baseline_end = baseline_end_seconds.filter(|end| *end > baseline_start_seconds);
    Ok(RuntimeVerificationResult {
        project_revision: program_revision.to_string(),
        request: ConnectedRequest {
            execution: ConnectedExecution::ConnectedRuntime,
            loop_revision: loop_definition.revision.clone(),
            acquisition: ConnectedAcquisition {
                recording_id: run.recording_id.clone(),
                program_revision: program_revision.to_string(),
                method: run.acquisition.method.clone(),
                completeness: verdict.completeness,
                observed_sample_count: run.acquisition.observed_sample_count,
                scan_range: run.acquisition.scan_range,
                missing_scan_ranges: run.acquisition.missing_scan_ranges.clone(),
                dropped_observed_samples: run.acquisition.dropped_observed_samples,
                actual_cycle_timing_available: run.acquisition.actual_cycle_timing_available,
                freshness_available: run.acquisition.freshness_available,
                reason: verdict.reason,
            },
            experiment: plan.clone(),
            experiments: None,
            baseline_start_seconds: baseline_start,
            baseline_end_seconds: baseline_end,
        },
        run: run_record,
        evaluation,
    })
}

/// Reads a stored connected record and accepts it only when it is the record the builder would make
/// (`decodeConnectedIdentificationRecord`): its plan has an identifier, its acquisition is the complete runtime evidence of its
/// run, and `evaluate`, given the checks the stored evaluation holds, gives back the stored evaluation.
///
/// Equality is exact, as the `TypeScript` deep equality is: `serde_json` reads each stored float back to the value written,
/// because trust-control enables its `float_roundtrip` feature (A8.3).
#[must_use]
pub fn decode_connected_identification_record<E>(
    value: &serde_json::Value,
    evaluate: impl FnOnce(&ValidationRun, &[BoundsCheck]) -> Result<E, Failure>,
) -> Option<RuntimeVerificationResult<E>>
where
    E: DeserializeOwned + PartialEq,
{
    let record: RuntimeVerificationResult<E> = crate::json::decode_value(value.clone()).ok()?;
    let stored: StoredSpecification =
        crate::json::decode_value(value.get("evaluation")?.clone()).ok()?;
    if !is_consistent(&record) {
        return None;
    }
    let fresh = evaluate(&record.run, stored.specification.as_slice()).ok()?;
    (fresh == record.evaluation).then_some(record)
}

/// The checks a stored evaluation was made with: its `specification`, the only part of it the decoder reads.
#[derive(Deserialize)]
struct StoredSpecification {
    specification: Vec<BoundsCheck>,
}

/// The program revision of a recording that is complete runtime evidence: its acquisition is the runtime's scan buffer, its
/// verdict is complete, and it has a recording identifier and a program revision.
fn complete_program_revision<'a>(
    run: &'a RecordedRun,
    verdict: &RecordingEvidence,
) -> Option<&'a str> {
    let revision = run
        .program_revision
        .as_deref()
        .filter(|revision| !revision.is_empty())?;
    let complete = run.acquisition.method == RUNTIME_SCAN_BUFFER
        && verdict.completeness == Completeness::Complete
        && !run.recording_id.is_empty();
    complete.then_some(revision)
}

/// The refusal of a recording that is not complete runtime evidence. It keeps the verdict's reason, names the next action
/// (Identify, A11.2) and says what the next attempt changes (A11.1).
fn incomplete_recording(reason: &str) -> Failure {
    Failure::new(
        &format!(
            "Identification requires the complete recording from the connected runtime. {reason} \
             The incomplete recording is kept as evidence, and Identify plant records a new one."
        ),
        Cause::ModelOrData,
        NextAction::Identify,
    )
}

/// One sample of the run: its time from the first runtime scan, its sequence value and its values by symbol.
fn validation_sample(
    run: &RecordedRun,
    loop_definition: &LoopDefinition,
    origin: i64,
    sample: &RecordedSample,
) -> Result<ValidationSample, Failure> {
    let time = sample.runtime_time_ns.ok_or_else(invalid_samples)?;
    Ok(ValidationSample {
        time_seconds: seconds_between(origin, time),
        sequence: sample.scan,
        values: sample_values(run, loop_definition, sample),
    })
}

/// The values of one sample by signal symbol. The process value, the applied input and the setpoint carry the sample's
/// quality; each additional signal carries its channel's value and quality (`connectedIdentificationRecord.ts`).
fn sample_values(
    run: &RecordedRun,
    loop_definition: &LoopDefinition,
    sample: &RecordedSample,
) -> BTreeMap<String, ValidationValue> {
    let quality = sample_quality(run, loop_definition, sample);
    let channels = capture_additional_channels(
        loop_definition,
        |id| binding_value(run, sample, id),
        |id| binding_forced(run, sample, id),
    );
    let signals = &loop_definition.signals;
    let mut values = BTreeMap::new();
    values.insert(
        signals.process_variable.symbol.clone(),
        ValidationValue {
            value: process_value(run, sample),
            quality,
        },
    );
    values.insert(
        signals.manipulated_variable.symbol.clone(),
        ValidationValue {
            value: applied_mv(run, loop_definition, sample),
            quality,
        },
    );
    if let Some(setpoint) = &signals.setpoint {
        values.insert(
            setpoint.symbol.clone(),
            ValidationValue {
                value: binding_value(run, sample, SETPOINT_BINDING),
                quality,
            },
        );
    }
    for additional in signals.additional.iter().flatten() {
        let measured = channels.iter().find(|channel| channel.id == additional.id);
        values.insert(
            additional.symbol.clone(),
            ValidationValue {
                value: measured.and_then(|channel| channel.value),
                quality: measured.map_or(quality, |channel| channel.quality),
            },
        );
    }
    values
}

/// The values the signals had at the first sample, where they were readable (`initialState`).
fn initial_state_of(first: &ValidationSample) -> BTreeMap<String, f64> {
    first
        .values
        .iter()
        .filter_map(|(symbol, value)| value.value.map(|measured| (symbol.clone(), measured)))
        .collect()
}

/// The channels of the run in the record's order: the process value, the manipulated variable, the setpoint when the loop has
/// one, then the additional signals. Each is named by its symbol and carries its unit.
fn validation_channels(loop_definition: &LoopDefinition) -> Vec<ValidationChannel> {
    let signals = &loop_definition.signals;
    let mut channels = vec![
        validation_channel(
            &signals.process_variable.symbol,
            &signals.process_variable.unit,
        ),
        validation_channel(
            &signals.manipulated_variable.symbol,
            &signals.manipulated_variable.unit,
        ),
    ];
    channels.extend(
        signals
            .setpoint
            .iter()
            .map(|setpoint| validation_channel(&setpoint.symbol, &setpoint.unit)),
    );
    channels.extend(
        signals
            .additional
            .iter()
            .flatten()
            .map(|additional| validation_channel(&additional.symbol, &additional.unit)),
    );
    channels
}

fn validation_channel(symbol: &str, unit: &str) -> ValidationChannel {
    ValidationChannel {
        id: symbol.to_string(),
        unit: unit.to_string(),
    }
}

/// The bounds the record checks, over the whole run (B3.4 step 6): the measured output against its range, then the plant input
/// against the output limits. The other signals are not bounded.
///
/// # Errors
/// An output limit is missing or not ordered; the refusal names it.
fn bounds_checks(
    loop_definition: &LoopDefinition,
    duration_seconds: f64,
) -> Result<Vec<BoundsCheck>, Failure> {
    let signals = &loop_definition.signals;
    let plant_input = plant_input_range(loop_definition)?;
    Ok(vec![
        bounds_check(
            &signals.process_variable.symbol,
            signals.process_variable.range.minimum,
            signals.process_variable.range.maximum,
            duration_seconds,
        ),
        bounds_check(
            &signals.manipulated_variable.symbol,
            plant_input.minimum,
            plant_input.maximum,
            duration_seconds,
        ),
    ])
}

fn bounds_check(symbol: &str, minimum: f64, maximum: f64, duration_seconds: f64) -> BoundsCheck {
    BoundsCheck {
        id: format!("bounds:{symbol}"),
        kind: BoundsKind::Bounds,
        channel: symbol.to_string(),
        window: BoundsWindow {
            start: 0.0,
            end: duration_seconds,
        },
        minimum,
        maximum,
    }
}

/// The evidence a plan's run has: a simulated run is executed by the ST program, a physical run is recorded (A9).
fn run_evidence(source: PlanSource) -> RunEvidence {
    match source {
        PlanSource::Simulation => RunEvidence::StExecuted,
        PlanSource::Physical => RunEvidence::PhysicalRecorded,
    }
}

/// The identity and acquisition checks of `decodeConnectedIdentificationRecord`: the plan has an identifier, the acquisition is
/// complete runtime evidence, and the run was recorded on the project revision, under its model and its evidence.
fn is_consistent<E>(record: &RuntimeVerificationResult<E>) -> bool {
    let project = record.project_revision.as_str();
    let plan = &record.request.experiment;
    let acquisition = &record.request.acquisition;
    let run = &record.run;
    let first = run.samples.first().map(|sample| sample.sequence);
    let last = run.samples.last().map(|sample| sample.sequence);
    !plan.id.is_empty()
        && acquisition.method == RUNTIME_SCAN_BUFFER
        && acquisition.completeness == Completeness::Complete
        && acquisition.recording_id == run.id
        && acquisition.program_revision == project
        && run.conditions.program_revision == project
        && run.conditions.model_revision == format!("connected-plant:{project}")
        && run.evidence == run_evidence(plan.source)
        && acquisition.dropped_observed_samples == 0
        && acquisition.missing_scan_ranges.is_empty()
        && u64::try_from(run.samples.len()).ok() == Some(acquisition.observed_sample_count)
        && acquisition.actual_cycle_timing_available
        && acquisition.scan_range.map(|range| range.first) == first
        && acquisition.scan_range.map(|range| range.last) == last
}

#[cfg(test)]
#[path = "connected_record_tests.rs"]
mod tests;
