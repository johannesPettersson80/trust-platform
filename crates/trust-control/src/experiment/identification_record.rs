//! The Test records Identify consumes (`experiment.ts`: the ledger of terminal Test evidence; specification 35, B3
//! and A4.4; annex E5.6). A terminal Test is immutable once it is recorded. Its samples come from the runtime's own
//! acquisition, never from the editor's monitor, which does not establish cycle completeness.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::plan::{ControlExperimentKind, ControlExperimentPlan};
use crate::loop_definition::{AuxiliaryRole, LoopDefinition};
use crate::recording::channels::{
    applied_mv, binding_forced, binding_value, capture_additional_channels, process_value,
    sample_quality, ChannelQuality, RecordedChannel, SETPOINT_BINDING,
};
use crate::recording::evidence::recording_evidence;
use crate::recording::samples::seconds_between;
use crate::recording::{Acquisition, Completeness, RecordedRun, RecordedSample};

/// The quality of an identification sample: the library's type, which has the same wire words (`"good"`, `"uncertain"`,
/// `"bad"`) for every consumer.
pub use trust_control_math::identify::SampleQuality;

/// Whether a Test is the estimation or the validation of the model (`ExperimentIdentificationRecord.role`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum IdentificationRole {
    /// The Test that the model is fitted from.
    Estimation,
    /// The Test that the fitted model is checked against.
    Validation,
}

/// Whether a Test record is completed or failed (`outcome`). A failed record is kept, and never fits a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum IdentificationOutcome {
    /// Completed, with complete runtime evidence.
    Completed,
    /// Failed, or without complete runtime evidence.
    Failed,
}

/// Whether a Test record qualifies for Identify (`qualification`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IdentificationQualification {
    /// The record qualifies.
    Qualified,
    /// The record does not qualify.
    NotQualified,
}

/// The status of a Test that has ended (annex E5.6): `completed`, `aborted`, `failed` or `failed_to_restore`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentTerminalStatus {
    /// The experiment ran to its end.
    Completed,
    /// The experiment was aborted.
    Aborted,
    /// The experiment failed.
    Failed,
    /// The experiment ended and its restoration was not confirmed.
    FailedToRestore,
}

/// The restoration of a Test that has ended (annex E5.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TerminalRestoration {
    /// The restoring write was confirmed.
    Restored,
    /// No write was queued.
    NotRequired,
    /// The restoration failed.
    Failed,
}

/// The final answer of the runtime for a Test (`ExperimentIdentificationTerminal`). Its keys are the runtime's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExperimentIdentificationTerminal {
    /// The experiment's identifier.
    pub experiment_id: String,
    /// How the experiment ended.
    pub status: ExperimentTerminalStatus,
    /// Why it stopped, when it did.
    pub stop_reason: Option<String>,
    /// How its restoration ended.
    pub restoration: TerminalRestoration,
    /// Why its restoration ended as it did, when it says so.
    pub restoration_reason: Option<String>,
}

/// One sample Identify fits: the time from the first runtime execution, the plant input and the measured output
/// (`ExperimentIdentificationSample`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentIdentificationSample {
    /// The process time from the first sample, in s.
    pub time_seconds: f64,
    /// The plant input; NaN when missing.
    pub input: f64,
    /// The measured output; NaN when missing.
    pub output: f64,
    /// The sample's quality.
    pub quality: SampleQuality,
    /// The input's unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_unit: Option<String>,
    /// The output's unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_unit: Option<String>,
    /// Whether the input is clipped by its limits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clipped: Option<bool>,
    /// Whether the manipulated output sits on its limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saturated: Option<bool>,
}

/// One Test record of the ledger (`ExperimentIdentificationRecord`): immutable once recorded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentIdentificationRecord {
    /// The experiment's identifier.
    pub experiment_id: String,
    /// Whether the record estimates or validates.
    pub role: IdentificationRole,
    /// Whether the record is completed or failed.
    pub outcome: IdentificationOutcome,
    /// Whether the record qualifies for Identify.
    pub qualification: IdentificationQualification,
    /// The reviewed plan the record belongs to.
    pub plan: ControlExperimentPlan,
    /// The runtime's final answer for the experiment.
    pub terminal: ExperimentIdentificationTerminal,
    /// The samples, as the runtime acquired them.
    pub samples: Vec<ExperimentIdentificationSample>,
    /// The acquisition the samples came from, when the runtime owned it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acquisition: Option<Acquisition>,
}

/// Why a Test's runtime samples cannot be handed to Identify (the refusals of `identificationSamplesFromRuntimeRecording`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IdentificationSamplesError {
    /// The recording is not complete: its acquisition, its sample count, its record period or its start is not
    /// confirmed. The sentence is the recording's evidence reason (`recording_evidence`).
    #[error("{0}")]
    Incomplete(String),
    /// A sample has no input or no output.
    #[error("A complete runtime-owned recording contains a missing input or output sample.")]
    MissingSample,
    /// A load step has no single declared disturbance to read.
    #[error("A load-step Test recording requires exactly one declared disturbance input.")]
    DisturbanceCount,
    /// A sample has no runtime time, so its time cannot be placed.
    #[error("The runtime recording contains invalid samples.")]
    InvalidSamples,
    /// A plant input limit is missing, so whether a manipulated sample is saturated cannot be told (B3.4 step 1).
    #[error("The plant input limits are missing: limits.outputMinimum and limits.outputMaximum must be set, with the minimum below the maximum.")]
    MissingPlantInputLimits,
}

/// Why the ledger refuses a Test record or a retry (`ExperimentIdentificationLedger`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExperimentLedgerError {
    /// The terminal answer names another experiment than the reviewed plan.
    #[error("Terminal Test evidence does not match the reviewed plan.")]
    EvidenceMismatch,
    /// A plan has one immutable terminal record already.
    #[error("A Test plan can only create one immutable terminal record.")]
    DuplicateRecord,
    /// Only a failed record may be retried.
    #[error("Only a retained failed Test can be retried.")]
    NotRetryable,
    /// A retry needs a plan that has not been recorded.
    #[error("Retry requires a fresh reviewed plan.")]
    NotFresh,
}

/// The ledger of Test records (`ExperimentIdentificationLedger`). It keeps the Test outcomes as immutable inputs
/// to Identify, and fits nothing itself.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExperimentIdentificationLedger {
    entries: Vec<ExperimentIdentificationRecord>,
}

impl ExperimentIdentificationLedger {
    /// Every record, in the order it was taken.
    #[must_use]
    pub fn records(&self) -> &[ExperimentIdentificationRecord] {
        &self.entries
    }

    /// The completed records (`identificationRecords`).
    #[must_use]
    pub fn identification_records(&self) -> Vec<&ExperimentIdentificationRecord> {
        self.entries
            .iter()
            .filter(|entry| entry.outcome == IdentificationOutcome::Completed)
            .collect()
    }

    /// The role of the next record: the first completed Test estimates, the later ones validate.
    #[must_use]
    pub fn next_role(&self) -> IdentificationRole {
        if self.identification_records().is_empty() {
            IdentificationRole::Estimation
        } else {
            IdentificationRole::Validation
        }
    }

    /// Records the Test of `plan` with its terminal answer, its samples and its acquisition, once. It qualifies
    /// when it completed, every sample has a finite input and output, and the acquisition is complete runtime
    /// evidence (or there is none to check).
    ///
    /// # Errors
    /// The terminal answer names another experiment, or the plan already has a record.
    pub fn record(
        &mut self,
        plan: &ControlExperimentPlan,
        terminal: &ExperimentIdentificationTerminal,
        samples: &[ExperimentIdentificationSample],
        acquisition: Option<&Acquisition>,
    ) -> Result<ExperimentIdentificationRecord, ExperimentLedgerError> {
        if terminal.experiment_id != plan.id {
            return Err(ExperimentLedgerError::EvidenceMismatch);
        }
        if self
            .entries
            .iter()
            .any(|entry| entry.experiment_id == plan.id)
        {
            return Err(ExperimentLedgerError::DuplicateRecord);
        }
        let qualified = terminal.status == ExperimentTerminalStatus::Completed
            && samples
                .iter()
                .all(|sample| sample.input.is_finite() && sample.output.is_finite())
            && acquisition.is_none_or(is_complete_runtime_evidence);
        let entry = ExperimentIdentificationRecord {
            experiment_id: plan.id.clone(),
            role: self.next_role(),
            outcome: if qualified {
                IdentificationOutcome::Completed
            } else {
                IdentificationOutcome::Failed
            },
            qualification: if qualified {
                IdentificationQualification::Qualified
            } else {
                IdentificationQualification::NotQualified
            },
            plan: plan.clone(),
            terminal: terminal.clone(),
            samples: samples.to_vec(),
            acquisition: acquisition.cloned(),
        };
        self.entries.push(entry.clone());
        Ok(entry)
    }

    /// The plan that retries a failed Test: it must be a newly reviewed plan, with an identity no record has.
    ///
    /// # Errors
    /// The experiment is not a recorded failure, or the plan is not fresh.
    pub fn retry(
        &self,
        failed_experiment_id: &str,
        fresh_plan: &ControlExperimentPlan,
    ) -> Result<ControlExperimentPlan, ExperimentLedgerError> {
        let failed = self
            .entries
            .iter()
            .find(|entry| entry.experiment_id == failed_experiment_id);
        if !failed.is_some_and(|entry| entry.outcome == IdentificationOutcome::Failed) {
            return Err(ExperimentLedgerError::NotRetryable);
        }
        if fresh_plan.id == failed_experiment_id
            || self
                .entries
                .iter()
                .any(|entry| entry.experiment_id == fresh_plan.id)
        {
            return Err(ExperimentLedgerError::NotFresh);
        }
        Ok(fresh_plan.clone())
    }
}

/// The record of `role` that Identify fits from: the first completed record with that role. Failed records never
/// qualify (`identificationRecordForRole`).
#[must_use]
pub fn identification_record_for_role(
    records: &[ExperimentIdentificationRecord],
    role: IdentificationRole,
) -> Option<&ExperimentIdentificationRecord> {
    records
        .iter()
        .find(|record| record.outcome == IdentificationOutcome::Completed && record.role == role)
}

/// The samples Identify fits from a Test's runtime recording (`identificationSamplesFromRuntimeRecording`). The
/// recording must be complete by its evidence verdict (`recording_evidence`: complete claims, at least two samples at
/// one steady record period, and a confirmed start), and every sample must have an input and an output.
///
/// # Errors
/// The recording is incomplete (the verdict's reason is returned), a sample has no input or output, a load step has no
/// single disturbance, or a sample has no runtime time.
pub fn identification_samples_from_runtime_recording(
    kind: ControlExperimentKind,
    run: &RecordedRun,
    loop_definition: &LoopDefinition,
    start_failure: Option<&str>,
) -> Result<Vec<ExperimentIdentificationSample>, IdentificationSamplesError> {
    let verdict = recording_evidence(run, start_failure);
    if verdict.completeness != Completeness::Complete {
        return Err(IdentificationSamplesError::Incomplete(verdict.reason));
    }
    map_samples(kind, run, loop_definition, true)
}

/// The samples of a Test's runtime recording that failed or were aborted: what was received, kept as evidence,
/// with the missing values as NaN. They never qualify for Identify (`retainedIdentificationSamplesFromRuntimeRecording`).
///
/// # Errors
/// A load step has no single disturbance, or a sample has no runtime time.
pub fn retained_identification_samples_from_runtime_recording(
    kind: ControlExperimentKind,
    run: &RecordedRun,
    loop_definition: &LoopDefinition,
) -> Result<Vec<ExperimentIdentificationSample>, IdentificationSamplesError> {
    map_samples(kind, run, loop_definition, false)
}

/// The plant input, its unit and quality, and whether the input is the manipulated output (`identificationInput`).
struct Selected {
    value: Option<f64>,
    unit: String,
    quality: ChannelQuality,
    manipulated: bool,
}

fn map_samples(
    kind: ControlExperimentKind,
    run: &RecordedRun,
    loop_definition: &LoopDefinition,
    require_finite: bool,
) -> Result<Vec<ExperimentIdentificationSample>, IdentificationSamplesError> {
    let Some(first) = run.samples.first() else {
        return Ok(Vec::new());
    };
    let origin = first
        .runtime_time_ns
        .ok_or(IdentificationSamplesError::InvalidSamples)?;
    let output_unit = loop_definition.signals.process_variable.unit.clone();
    run.samples
        .iter()
        .map(
            |sample| -> Result<ExperimentIdentificationSample, IdentificationSamplesError> {
                let time = sample
                    .runtime_time_ns
                    .ok_or(IdentificationSamplesError::InvalidSamples)?;
                let selected = identification_input(kind, run, loop_definition, sample)?;
                let output = process_value(run, sample);
                let input_known = is_finite_value(selected.value);
                let output_known = is_finite_value(output);
                if require_finite && !(input_known && output_known) {
                    return Err(IdentificationSamplesError::MissingSample);
                }
                let good = sample_quality(run, loop_definition, sample) == ChannelQuality::Good
                    && selected.quality == ChannelQuality::Good
                    && input_known
                    && output_known;
                let saturated = if selected.manipulated {
                    let (Some(minimum), Some(maximum)) = (
                        loop_definition.limits.output_minimum,
                        loop_definition.limits.output_maximum,
                    ) else {
                        return Err(IdentificationSamplesError::MissingPlantInputLimits);
                    };
                    selected.value.is_some_and(|input| {
                        is_at_limit(input, minimum) || is_at_limit(input, maximum)
                    })
                } else {
                    false
                };
                Ok(ExperimentIdentificationSample {
                    time_seconds: seconds_between(origin, time),
                    input: finite_or_nan(selected.value),
                    output: finite_or_nan(output),
                    quality: if good {
                        SampleQuality::Good
                    } else {
                        SampleQuality::Bad
                    },
                    input_unit: Some(selected.unit),
                    output_unit: Some(output_unit.clone()),
                    clipped: None,
                    saturated: saturated.then_some(true),
                })
            },
        )
        .collect()
}

/// The plant input of a sample by experiment kind: the setpoint for a setpoint step, the single disturbance for a
/// load step, and the applied manipulated input otherwise.
fn identification_input(
    kind: ControlExperimentKind,
    run: &RecordedRun,
    loop_definition: &LoopDefinition,
    sample: &RecordedSample,
) -> Result<Selected, IdentificationSamplesError> {
    match kind {
        ControlExperimentKind::SetpointStep => Ok(Selected {
            value: binding_value(run, sample, SETPOINT_BINDING),
            unit: loop_definition
                .signals
                .setpoint
                .as_ref()
                .map_or_else(String::new, |setpoint| setpoint.unit.clone()),
            quality: sample_quality(run, loop_definition, sample),
            manipulated: false,
        }),
        ControlExperimentKind::LoadStep => {
            let channels = captured_channels(run, loop_definition, sample);
            let disturbances: Vec<&RecordedChannel> = channels
                .iter()
                .filter(|channel| channel.role == AuxiliaryRole::Disturbance)
                .collect();
            let [disturbance] = disturbances.as_slice() else {
                return Err(IdentificationSamplesError::DisturbanceCount);
            };
            Ok(Selected {
                value: disturbance.value,
                unit: disturbance.unit.clone(),
                quality: disturbance.quality,
                manipulated: false,
            })
        }
        _ => Ok(Selected {
            value: applied_mv(run, loop_definition, sample),
            unit: loop_definition.signals.manipulated_variable.unit.clone(),
            quality: sample_quality(run, loop_definition, sample),
            manipulated: true,
        }),
    }
}

fn captured_channels(
    run: &RecordedRun,
    loop_definition: &LoopDefinition,
    sample: &RecordedSample,
) -> Vec<RecordedChannel> {
    capture_additional_channels(
        loop_definition,
        |id| binding_value(run, sample, id),
        |id| binding_forced(run, sample, id),
    )
}

/// Whether an acquisition is complete runtime evidence: the runtime's scan buffer, complete, with cycle timing, no
/// missing scan and no dropped sample.
fn is_complete_runtime_evidence(acquisition: &Acquisition) -> bool {
    acquisition.method == "runtime_scan_buffer"
        && acquisition.completeness == Completeness::Complete
        && acquisition.actual_cycle_timing_available
        && acquisition.missing_scan_ranges.is_empty()
        && acquisition.dropped_observed_samples == 0
}

fn is_finite_value(value: Option<f64>) -> bool {
    value.is_some_and(f64::is_finite)
}

fn finite_or_nan(value: Option<f64>) -> f64 {
    match value {
        Some(value) if value.is_finite() => value,
        _ => f64::NAN,
    }
}

/// Whether a value sits exactly on an output limit: such an input is saturated (A4.2).
#[allow(clippy::float_cmp)] // Saturation is an exact match of the output limit.
fn is_at_limit(value: f64, limit: f64) -> bool {
    value == limit
}

#[cfg(test)]
#[path = "identification_record_tests.rs"]
mod tests;
