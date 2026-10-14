//! The assessment of an applied commissioning response (`applyObservation.ts`; specification 35, B6.3, B6.7, A8.5, A8.6,
//! A10.4; numerics N8.4 and N8.5).
//!
//! The rules decide when a measured output has left its permitted range, what each refusal says and offers, whether the
//! recorded response and its observed requirements pass, and which decisions the observation takes while the runtime records
//! it. Left out, as the transport of the observation: the follow loop (`followAppliedResponse`), the target change
//! (`observeTargetChange`), the resumed observation (`resumeApplyObservation`), the retained recovery (`stopRecovered`), the
//! recording follower, and the checkpoint writer of `ObservationRecord`. The transfers journalled with an observation wait for a
//! Rust transfer status type.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::sampling::median_spacing;

use super::confirmation::confirmation_window_ms;
use super::observation::{
    exact_f64, observation_run, observed_values, round_nanoseconds, CommissioningObservation,
    ResponseSample,
};
use super::texts;
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::connected_record::{RunEvidence, ValidationSample};
use crate::identification::independent_validation::RequirementStatus;
use crate::limits::{CONNECTION_LOSS_TIMEOUT_MS, SAMPLES_PER_SETTLING_TIME};
use crate::loop_definition::{AuxiliaryRole, Signals};
use crate::recording::Completeness;
use crate::validation::commissioning_assessment::{
    assess_commissioning_recording, CommissioningCriteria, CommissioningRecording,
};
use crate::validation::run_validation::{check_requirement, evaluate_observed_requirement};

/// The runtime answer that it no longer holds a recording (`recording_unavailable`, A8.6).
const RECORDING_UNAVAILABLE: &str = "recording_unavailable";

/// A measured output's permitted range (B6.3): the output's declared range, and the recorded channel that carries its value.
/// The process variable is recorded as `pv` of each scan; any other measured output by its auxiliary identifier (B1.4).
#[derive(Debug, Clone, PartialEq)]
pub struct PermittedRange {
    /// The output's symbol, which a departure names.
    pub symbol: String,
    /// The output's unit.
    pub unit: String,
    /// The smallest permitted value.
    pub minimum: f64,
    /// The largest permitted value.
    pub maximum: f64,
    /// The auxiliary identifier that records the output; None for the process variable.
    pub auxiliary_id: Option<String>,
}

/// Every measured output of a loop with its own permitted range, the process variable first (`observedRangesOf`, B6.3): the
/// process variable, and the additional signals of role PV and state (B1.4). No other additional signal is a measured output.
#[must_use]
pub fn observed_ranges_of(signals: &Signals) -> Vec<PermittedRange> {
    let process = PermittedRange {
        symbol: signals.process_variable.symbol.clone(),
        unit: signals.process_variable.unit.clone(),
        minimum: signals.process_variable.range.minimum,
        maximum: signals.process_variable.range.maximum,
        auxiliary_id: None,
    };
    let auxiliary = signals
        .additional
        .iter()
        .flatten()
        .filter(|signal| {
            matches!(
                signal.role,
                AuxiliaryRole::ProcessVariable | AuxiliaryRole::State
            )
        })
        .map(|signal| PermittedRange {
            symbol: signal.symbol.clone(),
            unit: signal.unit.clone(),
            minimum: signal.range.minimum,
            maximum: signal.range.maximum,
            auxiliary_id: Some(signal.id.clone()),
        });
    std::iter::once(process).chain(auxiliary).collect()
}

/// The typed refusals of an Apply observation (`PermittedRangeLeft`, `ObservationRequirementFailed`, `ObservationDetached` and
/// `ObservationNotHeld`). Each carries its failure: the text, the cause and the next action (A11.1, A11.2).
#[derive(Debug, Clone, PartialEq)]
pub enum ObservationRefusal {
    /// A measured output left its permitted range (B6.3): the previous settings are restored at once, and no action is offered
    /// (B6.7).
    PermittedRangeLeft(Failure),
    /// A recorded response failed a requirement (B6.3): the applied settings stay in place, and Restore previous settings is offered.
    RequirementFailed(Failure),
    /// The panel closed while the observation was reattached: the runtime keeps recording it, and no action is offered (A8.5, A10.4).
    Detached(Failure),
    /// The runtime no longer holds the recording (A8.6): the observation is incomplete, and Restore previous settings is offered.
    NotHeld(Failure),
}

impl ObservationRefusal {
    /// The failure this refusal reports.
    #[must_use]
    pub fn failure(&self) -> &Failure {
        match self {
            Self::PermittedRangeLeft(failure)
            | Self::RequirementFailed(failure)
            | Self::Detached(failure)
            | Self::NotHeld(failure) => failure,
        }
    }
}

/// The refusal of an observation whose panel closed while it was reattached (`ObservationDetached`, A8.5, A10.4).
#[must_use]
pub fn detached_refusal() -> ObservationRefusal {
    ObservationRefusal::Detached(Failure::new(
        &texts::apply_observation_detached(),
        Cause::Unclassified,
        NextAction::None,
    ))
}

/// The refusal of an observation whose recording the runtime no longer holds (`ObservationNotHeld`, A8.6): the observation is
/// incomplete, and Restore previous settings is offered. `runtime_reason` is the runtime's own sentence.
#[must_use]
pub fn not_held_refusal(runtime_reason: &str) -> ObservationRefusal {
    ObservationRefusal::NotHeld(Failure::new(
        &texts::apply_observation_not_held(runtime_reason),
        Cause::Runtime,
        NextAction::Restore,
    ))
}

/// Whether a runtime error code says that the runtime no longer holds the recording (`recordingNotHeld`, A8.6). Any other code
/// is a failure of the recovery itself.
#[must_use]
pub fn recording_not_held(runtime_code: &str) -> bool {
    runtime_code == RECORDING_UNAVAILABLE
}

/// Checks every measured output in every scan, in declared order, and names the first one that leaves its range
/// (`checkPermittedRanges`, B6.3, B6.7). A scan without a value of an output is no departure.
///
/// # Errors
/// Returns [`ObservationRefusal::PermittedRangeLeft`] for the first value below the minimum or above the maximum of its range.
pub fn check_permitted_ranges(
    samples: &[ResponseSample],
    ranges: &[PermittedRange],
) -> Result<(), ObservationRefusal> {
    for sample in samples {
        for range in ranges {
            let Some(value) = recorded_value_of(sample, range) else {
                continue;
            };
            if outside_range(value, range) {
                return Err(ObservationRefusal::PermittedRangeLeft(Failure::new(
                    &texts::permitted_range_statement(
                        &range.symbol,
                        &range.unit,
                        value,
                        range.minimum,
                        range.maximum,
                    ),
                    Cause::ModelOrData,
                    NextAction::None,
                )));
            }
        }
    }
    Ok(())
}

/// The value of an output in a scan: `pv` for the process variable, otherwise its auxiliary channel by identifier. None when the
/// scan does not record it.
#[must_use]
fn recorded_value_of(sample: &ResponseSample, range: &PermittedRange) -> Option<f64> {
    match &range.auxiliary_id {
        None => sample.process_value,
        Some(identifier) => sample
            .additional
            .iter()
            .find(|channel| &channel.id == identifier)
            .and_then(|channel| channel.value),
    }
}

/// Whether a value lies below the minimum or above the maximum of its range. A value that is not a number is not outside: the
/// recorded-data assessment refuses it, as the TypeScript comparisons do.
#[allow(clippy::manual_range_contains)]
#[must_use]
fn outside_range(value: f64, range: &PermittedRange) -> bool {
    value < range.minimum || value > range.maximum
}

/// Whether the response covers the reviewed duration from its own start (B6.3): the newest scan lies at least `duration_ms` after
/// the first scan of the response, so that the target change never shortens the observation. No response start covers nothing.
#[must_use]
pub fn response_covers_duration(
    response_start_seconds: Option<f64>,
    newest_sample_seconds: f64,
    duration_ms: f64,
) -> bool {
    response_start_seconds
        .is_some_and(|start| (newest_sample_seconds - start) * 1_000.0 >= duration_ms)
}

/// Whether the runtime has stopped sending values (A3.4, A7, B6.3): no newer scan has arrived for longer than the connection-loss
/// timeout of a followed recording. That timeout is 5 s, or three intervals between the samples followed when that is longer, so
/// `record_period_ms` is the record period of the recording (0 when it is not known, which keeps 5 s). This follows the
/// specification; the TypeScript keeps 5 s alone. Both times are in ms of wall time.
#[must_use]
pub fn connection_lost(newest_progress_ms: f64, now_ms: f64, record_period_ms: f64) -> bool {
    let timeout_ms = exact_f64(CONNECTION_LOSS_TIMEOUT_MS).max(3.0 * record_period_ms);
    now_ms - newest_progress_ms > timeout_ms
}

/// A stopped applied recording as the observation assesses it: its completeness, its scans, the unit of the applied output and
/// its evidence (A9).
#[derive(Debug, Clone, PartialEq)]
pub struct ObservedRecording {
    /// Whether the recording's evidence is complete.
    pub completeness: Completeness,
    /// The scans, in time order.
    pub samples: Vec<ResponseSample>,
    /// The unit of the applied output.
    pub output_unit: String,
    /// Where the recorded values come from (A9).
    pub evidence: RunEvidence,
}

impl ObservedRecording {
    /// The recording as the recorded-data assessment reads it.
    #[must_use]
    pub fn commissioning_recording(&self) -> CommissioningRecording {
        CommissioningRecording {
            completeness: self.completeness,
            samples: self
                .samples
                .iter()
                .map(ResponseSample::commissioning_sample)
                .collect(),
            output_unit: self.output_unit.clone(),
            evidence: self.evidence,
        }
    }
}

/// The span of an applied response: its reviewed duration, and whether it is recorded at a record period above one execution,
/// so that its samples are averages (`recordEvery` above 1, A4.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObservedResponseSpan {
    /// The reviewed duration, in s.
    pub duration_seconds: f64,
    /// Whether the samples are recorded at a record period above one execution.
    pub averaged: bool,
}

/// Whether a response is recorded at a record period above one execution (`recordEvery` above 1, A4.2). Its samples are spaced by
/// the larger of the task period and the median interval of the response; a single sample has the task period. The record period
/// is the spacing over the task period, rounded as `Math.round` rounds it.
#[must_use]
pub fn response_is_averaged(response: &[ResponseSample], task_period_ms: f64) -> bool {
    let period = task_period_ms / 1_000.0;
    let times: Vec<f64> = response
        .iter()
        .map(|sample| sample.elapsed_seconds)
        .collect();
    let spacing = if times.len() > 1 {
        period.max(median_spacing(&times))
    } else {
        period
    };
    (spacing / period).round() > 1.0
}

/// The response: the scans from the first one in Auto at the reviewed target. It is empty when no scan holds the target, which the
/// recorded-data assessment has already refused.
#[must_use]
fn response_from(samples: &[ResponseSample], target: f64) -> &[ResponseSample] {
    samples
        .iter()
        .position(|sample| sample.controller_in_auto && sample.setpoint == Some(target))
        .map_or(&samples[..0], |first| &samples[first..])
}

/// Assesses the recorded response of an applied observation (`assessObservation`, B6.3): the recorded-data assessment first, then
/// the observed requirements of the observation, when it declares any, on the response from the reviewed Auto target. Every failure
/// is a failed requirement whose settings stay applied; its text is kept (A11.1). The result is the largest error of the response.
///
/// # Errors
/// Returns [`ObservationRefusal::RequirementFailed`] for the first check that fails.
pub fn assess_observation(
    recording: &ObservedRecording,
    criteria: &CommissioningCriteria,
    observed: Option<&CommissioningObservation>,
) -> Result<f64, ObservationRefusal> {
    let error = assess_commissioning_recording(&recording.commissioning_recording(), criteria)
        .map_err(|failure| ObservationRefusal::RequirementFailed(restore_failure(failure)))?;
    if let Some(specification) = observed {
        let response = response_from(&recording.samples, criteria.target);
        let span = ObservedResponseSpan {
            duration_seconds: criteria.duration_seconds,
            averaged: response_is_averaged(response, criteria.task_period_ms),
        };
        assess_commissioning_observation(response, specification, span, recording.evidence)
            .map_err(|failure| ObservationRefusal::RequirementFailed(restore_failure(failure)))?;
    }
    Ok(error)
}

/// Checks the observed requirements of an observation on a response (`assessCommissioningObservation`, N8.3, N8.4, B6.3). The
/// scans are placed on the sample times of the response, each from its first scan, and every requirement must decode on those
/// times and pass. A requirement that does not pass is named with its identifier and its reason.
///
/// # Errors
/// Returns a failure with the settings cause and the restore action for a response without scans, a requirement that does not
/// decode, and a requirement that does not pass.
pub fn assess_commissioning_observation(
    samples: &[ResponseSample],
    specification: &CommissioningObservation,
    span: ObservedResponseSpan,
    evidence: RunEvidence,
) -> Result<(), Failure> {
    let Some(first) = samples.first() else {
        return Err(Failure::new(
            &texts::observe_channels_missing(),
            Cause::Settings,
            NextAction::Restore,
        ));
    };
    let start = first.elapsed_seconds;
    let sample_times: Vec<f64> = samples
        .iter()
        .map(|sample| round_nanoseconds(sample.elapsed_seconds - start))
        .collect();
    let observed_samples: Vec<ValidationSample> = samples
        .iter()
        .zip(&sample_times)
        .map(|(sample, &time_seconds)| ValidationSample {
            time_seconds,
            sequence: sample.scan,
            values: observed_values(sample, &specification.channels),
        })
        .collect();
    let run = observation_run(
        &specification.channels,
        observed_samples,
        sample_times,
        span.duration_seconds,
        evidence,
    );
    for requirement in &specification.requirements {
        check_requirement(requirement, &run).map_err(restore_failure)?;
        let result = evaluate_observed_requirement(&run, requirement, span.averaged);
        if result.status != RequirementStatus::Pass {
            return Err(Failure::new(
                &texts::observe_requirement_not_passed(&result.id, &result.reason),
                Cause::Settings,
                NextAction::Restore,
            ));
        }
    }
    Ok(())
}

/// The failure of an applied response that did not pass: the settings stay applied, and Restore previous settings is offered
/// (B6.3, B6.7). The text and the detail are kept.
#[must_use]
fn restore_failure(failure: Failure) -> Failure {
    Failure::from_parts(
        failure.message,
        failure.detail,
        Cause::Settings,
        NextAction::Restore,
    )
}

/// What reopening needs about a running observation: where it started in process time and how long it is planned to last
/// (A8.5). The checkpoint journals it with the observation's recording identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObservationJournal {
    /// Process time at which the observation started, in ns.
    pub started_at_process_ns: u64,
    /// The reviewed duration of the observation, in ms.
    pub duration_ms: f64,
}

/// The stages of the commissioning journal (`CommissioningCheckpoint.stage`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum JournalStage {
    /// The reviewed settings are prepared.
    Prepared,
    /// The runtime records the observation.
    Recording,
    /// The parameter record is being written.
    Parameters,
    /// The controller transfer is being made.
    Transfer,
    /// The commissioning completed.
    Completed,
    /// The journal was recovered.
    Recovered,
    /// The journal was retired.
    Retired,
}

/// Whether a journalled checkpoint holds an observation the runtime may still record (A8.5): the checkpoint journals both the
/// observation and the recording's identity, and it has not completed, recovered or retired.
#[must_use]
pub fn observation_in_progress(stage: JournalStage, observed: bool, recorded: bool) -> bool {
    observed
        && recorded
        && !matches!(
            stage,
            JournalStage::Completed | JournalStage::Recovered | JournalStage::Retired
        )
}

/// The record periods the planned recording holds beyond the target change's allowance (A4.2). A sample holds the mean of its
/// executions, so the response starts at the first sample whose executions all lie at the target, up to one and a half record
/// periods after the change. The sample that covers the reviewed duration completes up to one and a half periods after its own
/// time. Three periods therefore hold the whole duration, whatever the phase of the samples.
const MARGIN_RECORD_PERIODS: f64 = 3.0;

/// The duration the runtime is asked to record for an Apply observation, in ms (`observeTargetChange`, B6.3, A4.2, A7): the reviewed
/// duration, the allowance of the target change (the confirmation window of the loop's task and two executions of that task), and
/// `MARGIN_RECORD_PERIODS` record periods. The recording starts before the target change, so it holds the change's confirmation as
/// well. A record period is at most a 200th of the planned length plus one execution (A4.2, `record_period_executions`), so the
/// planned length is the solution of `planned = duration + allowance + 3 · (planned / 200 + task)`.
#[must_use]
pub fn planned_recording_ms(duration_ms: f64, task_period_ms: f64) -> f64 {
    let allowance = confirmation_window_ms(task_period_ms) + 2.0 * task_period_ms;
    let fixed = duration_ms + allowance + MARGIN_RECORD_PERIODS * task_period_ms;
    fixed / (1.0 - MARGIN_RECORD_PERIODS / exact_f64(SAMPLES_PER_SETTLING_TIME))
}

#[cfg(test)]
#[path = "observation_assessment_tests.rs"]
mod tests;
