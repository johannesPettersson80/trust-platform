//! The recorded-data assessment of a commissioning response (`commissioningAssessment.ts`; specification 35, A4.2, A6.4,
//! B6.2 and B6.3). The same check runs when a commissioning completes and when its evidence is reopened. The recording
//! must be complete and hold good samples only. Its response starts at the reviewed Auto target, covers the reviewed
//! duration, stays in Auto at that target within the process and output ranges, and settles within the error limit. A
//! declared switching check must pass as well.
//!
//! Left out (stage V4): the observed-requirement step of the `TypeScript` (`criteria.observed`, through
//! `assessCommissioningObservation`). The assessment therefore checks the recorded response of a commissioning, and not
//! the observed requirements that an Apply observation may declare. The switching checks go through `run_validation`, as
//! the `TypeScript` does.
//!
//! Two differences from the `TypeScript` follow the specification. A measurement or output that is not a number fails the
//! range check: the `TypeScript`'s `<` and `>` let `NaN` through, and B5.4 says missing evidence never passes. The applied
//! output's unit is kept once per recording, where the `TypeScript` reads it from the first sample of the response; a
//! recording has one output unit.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::sampling::{largest, median_spacing};

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::connected_record::{
    RunConditions, RunEvidence, ValidationChannel, ValidationRun, ValidationSample, ValidationValue,
};
use crate::identification::independent_validation::RequirementStatus;
use crate::loop_definition::Range;
use crate::recording::channels::ChannelQuality;
use crate::recording::Completeness;
use crate::validation::run_validation::{
    check_requirement, evaluate_observed_requirement, DwellState, RunCheck, RunRequirement,
    RunRequirementResult, Window,
};
use crate::validation::texts;

/// The run's identifier in the switching check (`observed` of the `TypeScript`).
const SWITCHING_RUN_ID: &str = "commissioning";
/// The channel that holds the applied output in the switching check.
const SWITCHING_CHANNEL: &str = "MV";

/// The switching limits of a commissioning: the two output levels and the shortest dwell at each (`switching` of the
/// `TypeScript`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SwitchingCriteria {
    /// The lower output level, in the output's unit.
    pub low: f64,
    /// The upper output level, in the output's unit.
    pub high: f64,
    /// The shortest dwell at the upper level, in s.
    pub minimum_on_seconds: f64,
    /// The shortest dwell at the lower level, in s.
    pub minimum_off_seconds: f64,
}

/// What a commissioning response must meet (`CommissioningCriteria`): the reviewed Auto target and duration, the settling
/// time and the error limit, the task period, the ranges of the process and of the output, and the switching limits when
/// the output switches. The criteria are stored with the workflow result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommissioningCriteria {
    /// The reviewed Auto target: the reference the response holds.
    pub target: f64,
    /// The reviewed duration of the response, in s.
    pub duration_seconds: f64,
    /// The time after the start at which the error is judged, in s.
    pub settling_time_seconds: f64,
    /// The largest error the settled samples may have, in the output's unit.
    pub maximum_error: f64,
    /// The task period, in ms.
    pub task_period_ms: f64,
    /// The range of the measurement.
    pub process_range: Range,
    /// The range of the applied output.
    pub output_limits: Range,
    /// The switching limits, when the output switches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switching: Option<SwitchingCriteria>,
}

/// One scan of a commissioning recording, with the fields the assessment reads (the fields of `WorkflowResultSample` it
/// uses).
#[derive(Debug, Clone, PartialEq)]
pub struct CommissioningSample {
    /// The time of the scan in s from the start of the recording (`elapsedSeconds`).
    pub elapsed_seconds: f64,
    /// The scan's sequence value.
    pub scan: u64,
    /// Whether the loop is in Auto at the scan (`controllerMode` is `Auto`).
    pub in_auto: bool,
    /// The reference at the scan (`sp`), when it has one.
    pub setpoint: Option<f64>,
    /// The measurement at the scan (`pv`), when it has one.
    pub process_value: Option<f64>,
    /// The applied output at the scan (`appliedMv`), when it has one.
    pub applied_output: Option<f64>,
    /// The scan's quality (`quality`).
    pub quality: ChannelQuality,
}

/// A stopped commissioning recording as the assessment reads it: its evidence, its scans and the output's unit.
#[derive(Debug, Clone, PartialEq)]
pub struct CommissioningRecording {
    /// Whether the recording's evidence is complete (`evidence.completeness`).
    pub completeness: Completeness,
    /// The scans, in time order.
    pub samples: Vec<CommissioningSample>,
    /// The unit of the applied output.
    pub output_unit: String,
    /// Where the recorded values come from (A9). It labels the run of the switching check.
    pub evidence: RunEvidence,
}

/// The error of a commissioning response, once the response passes (`assessCommissioningRecording`). The value is the
/// largest error of the settled samples, in the output's unit.
///
/// The refusals and their next actions: an incomplete or not-good recording (runtime, none); no Auto sample at the target
/// (implementation, open loop setup); a response that does not cover the duration (runtime, none); a sample outside the
/// mode, reference or ranges (settings, restore, B6.3); no settled sample (runtime, none); an error above its limit, or a
/// failed switching check (task infeasible, restore, B6.3); an inconclusive switching check (model or data, none).
///
/// # Errors
/// The refusal of the first check that fails, in the order of the `TypeScript`.
pub fn assess_commissioning_recording(
    recording: &CommissioningRecording,
    criteria: &CommissioningCriteria,
) -> Result<f64, Failure> {
    if recording.completeness != Completeness::Complete
        || recording
            .samples
            .iter()
            .any(|sample| sample.quality != ChannelQuality::Good)
    {
        return Err(Failure::new(
            &texts::commissioning_recording_incomplete(),
            Cause::Runtime,
            NextAction::None,
        ));
    }
    let (first, start_sample) = recording
        .samples
        .iter()
        .enumerate()
        .find(|(_, sample)| sample.in_auto && sample.setpoint == Some(criteria.target))
        .ok_or_else(|| {
            Failure::new(
                &texts::commissioning_no_auto_target(),
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            )
        })?;
    let start = start_sample.elapsed_seconds;
    let response = &recording.samples[first..];
    check_coverage(response, start, criteria)?;
    check_bounds(response, criteria)?;
    let error = settled_error(response, start, criteria)?;
    if error > criteria.maximum_error {
        return Err(Failure::new(
            &texts::commissioning_error_exceeds(error, criteria.maximum_error),
            Cause::TaskInfeasible,
            NextAction::Restore,
        ));
    }
    if let Some(switching) = criteria.switching {
        check_switching(recording, response, start, switching)?;
    }
    Ok(error)
}

/// The response reaches the reviewed duration less one spacing of its samples (A4.2). A long observation is recorded at a
/// record period, so its samples lie one record period apart, not one task period. The spacing is the larger of the task
/// period and the median interval of the response, and a single sample has the task period.
fn check_coverage(
    response: &[CommissioningSample],
    start: f64,
    criteria: &CommissioningCriteria,
) -> Result<(), Failure> {
    let period = criteria.task_period_ms / 1000.0;
    let times: Vec<f64> = response
        .iter()
        .map(|sample| sample.elapsed_seconds)
        .collect();
    let spacing = if times.len() > 1 {
        period.max(median_spacing(&times))
    } else {
        period
    };
    let end = response
        .last()
        .map_or(start, |sample| sample.elapsed_seconds);
    if end - start < criteria.duration_seconds - spacing {
        return Err(Failure::new(
            &texts::commissioning_duration_not_covered(),
            Cause::Runtime,
            NextAction::None,
        ));
    }
    Ok(())
}

/// Every sample of the response stays in Auto at the target, with a measurement and an applied output inside their ranges
/// (A6.4, B6.3). A sample without a measurement or an output fails, and so does a value that is not a number.
fn check_bounds(
    response: &[CommissioningSample],
    criteria: &CommissioningCriteria,
) -> Result<(), Failure> {
    if response
        .iter()
        .all(|sample| within_bounds(sample, criteria))
    {
        Ok(())
    } else {
        Err(Failure::new(
            &texts::commissioning_bounds_violated(),
            Cause::Settings,
            NextAction::Restore,
        ))
    }
}

fn within_bounds(sample: &CommissioningSample, criteria: &CommissioningCriteria) -> bool {
    match (sample.process_value, sample.applied_output) {
        (Some(process_value), Some(applied_output)) => {
            sample.in_auto
                && sample.setpoint == Some(criteria.target)
                && (criteria.process_range.minimum..=criteria.process_range.maximum)
                    .contains(&process_value)
                && (criteria.output_limits.minimum..=criteria.output_limits.maximum)
                    .contains(&applied_output)
        }
        _ => false,
    }
}

/// The largest error `|target − measurement|` of the samples at or after the settling time (B5.4). Every sample of the
/// response holds a measurement once the bounds check passed, so only the settled samples are read here.
fn settled_error(
    response: &[CommissioningSample],
    start: f64,
    criteria: &CommissioningCriteria,
) -> Result<f64, Failure> {
    let errors: Vec<f64> = response
        .iter()
        .filter(|sample| sample.elapsed_seconds - start >= criteria.settling_time_seconds)
        .filter_map(|sample| sample.process_value)
        .map(|process_value| (criteria.target - process_value).abs())
        .collect();
    if errors.is_empty() {
        return Err(Failure::new(
            &texts::commissioning_deadline_unrecorded(),
            Cause::Runtime,
            NextAction::None,
        ));
    }
    Ok(largest(&errors, f64::NEG_INFINITY))
}

/// The switching check of a commissioning recording (`switching` of the `TypeScript`). The applied output keeps to its two
/// levels, and its high and its low dwells each last at least the declared minimum. The requirements are checked in turn
/// through `run_validation`, and the first one that does not pass refuses the recording with its identity and reason.
fn check_switching(
    recording: &CommissioningRecording,
    response: &[CommissioningSample],
    start: f64,
    switching: SwitchingCriteria,
) -> Result<(), Failure> {
    let times: Vec<f64> = response
        .iter()
        .map(|sample| sample.elapsed_seconds - start)
        .collect();
    let end = response
        .last()
        .map_or(0.0, |sample| sample.elapsed_seconds - start);
    let samples: Vec<ValidationSample> = response
        .iter()
        .zip(&times)
        .map(|(sample, &time_seconds)| ValidationSample {
            time_seconds,
            sequence: sample.scan,
            values: BTreeMap::from([(
                SWITCHING_CHANNEL.to_owned(),
                ValidationValue {
                    value: sample.applied_output,
                    quality: sample.quality,
                },
            )]),
        })
        .collect();
    // A view of the response for the requirement checks, which read only its channel, its samples and its sample times.
    // The fields left empty are not read by them.
    let run = ValidationRun {
        id: SWITCHING_RUN_ID.to_owned(),
        evidence: recording.evidence,
        parameters: BTreeMap::new(),
        conditions: RunConditions {
            initial_state: BTreeMap::new(),
            events: Vec::new(),
            duration_seconds: end,
            sample_times_seconds: times,
            seed: 1,
            model_revision: String::new(),
            program_revision: String::new(),
            parameter_schema_revision: String::new(),
        },
        channels: vec![ValidationChannel {
            id: SWITCHING_CHANNEL.to_owned(),
            unit: recording.output_unit.clone(),
        }],
        samples,
    };
    let window = Window { start: 0.0, end };
    let threshold = f64::midpoint(switching.low, switching.high);
    let requirements = [
        RunRequirement {
            id: "switching-output-levels".to_owned(),
            channel: SWITCHING_CHANNEL.to_owned(),
            window,
            check: RunCheck::Levels {
                allowed: vec![switching.low, switching.high],
                tolerance: 0.0,
            },
        },
        RunRequirement {
            id: "switching-on-dwell".to_owned(),
            channel: SWITCHING_CHANNEL.to_owned(),
            window,
            check: RunCheck::Dwell {
                threshold,
                state: DwellState::High,
                minimum_seconds: switching.minimum_on_seconds,
            },
        },
        RunRequirement {
            id: "switching-off-dwell".to_owned(),
            channel: SWITCHING_CHANNEL.to_owned(),
            window,
            check: RunCheck::Dwell {
                threshold,
                state: DwellState::Low,
                minimum_seconds: switching.minimum_off_seconds,
            },
        },
    ];
    for requirement in &requirements {
        check_requirement(requirement, &run)?;
        let result = evaluate_observed_requirement(&run, requirement, false);
        if result.status != RequirementStatus::Pass {
            return Err(switching_failure(&result));
        }
    }
    Ok(())
}

/// The refusal of a switching requirement that does not pass. A failed requirement is the observation's failure, so the
/// settings are restored (B6.3). An inconclusive one names the missing evidence (B5.5).
fn switching_failure(result: &RunRequirementResult) -> Failure {
    let text = texts::commissioning_check_failed(&result.id, &result.reason);
    if result.status == RequirementStatus::Inconclusive {
        Failure::new(&text, Cause::ModelOrData, NextAction::None)
    } else {
        Failure::new(&text, Cause::TaskInfeasible, NextAction::Restore)
    }
}

#[cfg(test)]
#[path = "commissioning_assessment_tests.rs"]
mod tests;
