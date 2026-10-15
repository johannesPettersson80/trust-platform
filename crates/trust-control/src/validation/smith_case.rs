//! The Smith predictor's part of Validate (specification 35, C9): the model values a candidate writes into a run, the predictor's observed
//! channels when the program uses `Control.SmithPredictorFOPDT`, and the verdict on those channels. A run fails when the predictor is not
//! active at a sample, with the reason that its history is incomplete or that it is not enabled, and when its correction reaches the
//! program's `MaximumCorrection` while that bound is positive (C9; Specification 37 §2.2 and §2.5). A program that keeps its own predictor
//! has none of these channels, so only its run's requirements are judged.

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::connected_record::{ValidationRun, ValidationSample};
use crate::identification::independent_validation::RequirementStatus;
use crate::loop_definition::LoopDefinition;
use crate::tuning::candidate::LawCandidate;
use crate::tuning::project_candidate::served_smith;
use crate::tuning::project_seed::SmithLibraryBlock;
use crate::validation::isolated_run_protocol::{IsolatedParameterOverride, IsolatedRunObservation};
use crate::validation::requirement_record::RunRequirementResult;
use crate::validation::run_requests::override_of;
use crate::validation::run_validation::RecordedRunEvaluation;
use crate::validation::texts;

/// The identifier of the predictor's requirement in a run's verdicts.
const PREDICTOR_ID: &str = "smith-predictor";
/// The kind word of the predictor's requirement in a run's verdicts.
const PREDICTOR_KIND: &str = "predictor-active";
/// The unit of the predictor's flags, which are BOOL values read as 0 or 1.
const FLAG_UNIT: &str = "flag";
/// The unit of a time in a verdict's sentence and value.
const TIME_UNIT: &str = "s";
/// A flag read as a number is TRUE when it is above this value.
const FLAG_THRESHOLD: f64 = 0.5;
/// The refusal of a candidate that holds no value for one of the predictor's model variables (C9).
const MODEL_VALUE_MISSING: &str =
    "A Smith predictor candidate holds no value for one of its model variables.";
/// The verdict of a run whose samples are averages: a flag cannot be judged from an average of its scans.
const AVERAGED_SAMPLES: &str =
    "The predictor's flags cannot be judged on averaged samples, so the run is inconclusive.";

/// The model values a Smith predictor's candidate writes into a run (C9, the run's parameter overrides): the model's gain, time constant
/// and delay, and the operating point when the seed names the library block (D-18). A candidate of another law has none.
///
/// # Errors
/// The candidate holds no value for one of those variables, or the loop has no parameter binding.
pub fn overrides(
    loop_definition: &LoopDefinition,
    candidate: &LawCandidate,
) -> Result<Vec<IsolatedParameterOverride>, Failure> {
    let (LawCandidate::Project(project), Some(seed)) = (candidate, served_smith(loop_definition))
    else {
        return Ok(Vec::new());
    };
    let Some(binding) = &loop_definition.parameter_binding else {
        return Err(Failure::new(
            &texts::parameter_or_reference_binding_missing(),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ));
    };
    let mut names = vec![
        seed.gain_symbol.as_str(),
        seed.time_constant_symbol.as_str(),
        seed.delay_symbol.as_str(),
    ];
    if let Some(block) = &seed.library_block {
        names.push(block.operating_input_symbol.as_str());
        names.push(block.operating_output_symbol.as_str());
    }
    names
        .into_iter()
        .map(|symbol| -> Result<IsolatedParameterOverride, Failure> {
            let value = project.values.get(symbol).copied().ok_or_else(|| {
                Failure::new(MODEL_VALUE_MISSING, Cause::Implementation, NextAction::None)
            })?;
            override_of(binding, symbol, value)
        })
        .collect()
}

/// The predictor's observed channels of a run (C9): the block's `Active` and `HistoryComplete` flags, its `Correction`, and the
/// program's `MaximumCorrection`. Empty when the program keeps its own predictor.
#[must_use]
pub fn observations(loop_definition: &LoopDefinition) -> Vec<IsolatedRunObservation> {
    let Some(block) = served_smith(loop_definition).and_then(|seed| seed.library_block.as_ref())
    else {
        return Vec::new();
    };
    let value_unit = loop_definition.signals.process_variable.unit.clone();
    [
        (
            format!("{}.Active", block.output_symbol),
            FLAG_UNIT.to_owned(),
        ),
        (
            format!("{}.HistoryComplete", block.output_symbol),
            FLAG_UNIT.to_owned(),
        ),
        (
            format!("{}.Correction", block.output_symbol),
            value_unit.clone(),
        ),
        (block.maximum_correction_symbol.clone(), value_unit),
    ]
    .into_iter()
    .map(|(symbol, unit)| IsolatedRunObservation { symbol, unit })
    .collect()
}

/// The predictor's verdict on a completed run (C9), as one requirement of the run: Pass when the predictor is active at every sample and
/// its correction stays below its maximum, Fail at the first sample that breaks either rule, and Inconclusive when a flag or the correction
/// is missing or the samples are averages. `None` when the program keeps its own predictor.
#[must_use]
pub fn predictor_result(
    loop_definition: &LoopDefinition,
    run: &ValidationRun,
    averaged: bool,
) -> Option<RunRequirementResult> {
    let block = served_smith(loop_definition)?.library_block.as_ref()?;
    let unit = loop_definition.signals.process_variable.unit.clone();
    let (status, time, reason) = if averaged {
        (
            RequirementStatus::Inconclusive,
            None,
            AVERAGED_SAMPLES.to_owned(),
        )
    } else {
        judge(&run.samples, block, &unit)
    };
    Some(RunRequirementResult {
        id: PREDICTOR_ID.to_owned(),
        kind: PREDICTOR_KIND.to_owned(),
        status,
        value: time,
        limit: None,
        unit: TIME_UNIT.to_owned(),
        reason,
    })
}

/// Adds the predictor's verdict to a completed run's evaluation: its requirement, and the aggregate status with it (C9). A program that
/// keeps its own predictor leaves the evaluation as it is.
pub fn add_predictor_result(
    loop_definition: &LoopDefinition,
    run: &ValidationRun,
    averaged: bool,
    evaluation: &mut RecordedRunEvaluation,
) {
    let Some(result) = predictor_result(loop_definition, run, averaged) else {
        return;
    };
    evaluation.status = combined(evaluation.status, result.status);
    evaluation.requirements.push(result);
}

/// The verdict of the samples of a run: the first sample at which the predictor is not active or its correction reaches its maximum,
/// with the time and the sentence that says why, or Inconclusive at the first sample whose flags or correction are missing.
fn judge(
    samples: &[ValidationSample],
    block: &SmithLibraryBlock,
    unit: &str,
) -> (RequirementStatus, Option<f64>, String) {
    let mut missing: Option<f64> = None;
    for sample in samples {
        let time = sample.time_seconds;
        let history_symbol = format!("{}.HistoryComplete", block.output_symbol);
        let active_symbol = format!("{}.Active", block.output_symbol);
        let correction_symbol = format!("{}.Correction", block.output_symbol);
        let history = value_of(sample, &history_symbol);
        let active = value_of(sample, &active_symbol);
        let correction = value_of(sample, &correction_symbol);
        let maximum = value_of(sample, &block.maximum_correction_symbol);
        let (Some(history), Some(active), Some(correction), Some(maximum)) =
            (history, active, correction, maximum)
        else {
            if missing.is_none() {
                missing = Some(time);
            }
            continue;
        };
        if history <= FLAG_THRESHOLD {
            return (
                RequirementStatus::Fail,
                Some(time),
                history_incomplete(time),
            );
        }
        if active <= FLAG_THRESHOLD {
            return (RequirementStatus::Fail, Some(time), not_enabled(time));
        }
        if maximum > 0.0 && correction.abs() >= maximum {
            return (
                RequirementStatus::Fail,
                Some(time),
                correction_at_maximum(maximum, unit, time),
            );
        }
    }
    match missing {
        Some(time) => (
            RequirementStatus::Inconclusive,
            Some(time),
            value_missing(time),
        ),
        None => (RequirementStatus::Pass, None, active_throughout()),
    }
}

/// The value of a channel at a sample, or `None` when the sample holds no value for it.
fn value_of(sample: &ValidationSample, symbol: &str) -> Option<f64> {
    sample.values.get(symbol).and_then(|value| value.value)
}

/// The aggregate of two verdicts: Fail when either fails, Inconclusive when either is inconclusive, and Pass otherwise.
fn combined(first: RequirementStatus, second: RequirementStatus) -> RequirementStatus {
    if first == RequirementStatus::Fail || second == RequirementStatus::Fail {
        RequirementStatus::Fail
    } else if first == RequirementStatus::Inconclusive || second == RequirementStatus::Inconclusive
    {
        RequirementStatus::Inconclusive
    } else {
        RequirementStatus::Pass
    }
}

/// The sentence of a run whose predictor's history is incomplete at a time: the predictor is not active (C9).
fn history_incomplete(time: f64) -> String {
    format!("The predictor's history is incomplete at {time:.3} {TIME_UNIT}, so the predictor is not active.")
}

/// The sentence of a run whose predictor is not enabled at a time, though its history is complete (C9).
fn not_enabled(time: f64) -> String {
    format!(
        "The predictor is not enabled at {time:.3} {TIME_UNIT}, although its history is complete."
    )
}

/// The sentence of a run whose correction reaches the program's maximum at a time (C9).
fn correction_at_maximum(maximum: f64, unit: &str, time: f64) -> String {
    format!("The predictor's correction reaches its maximum of {maximum} {unit} at {time:.3} {TIME_UNIT}.")
}

/// The sentence of a run whose predictor's flags or correction are missing at a time (B5.4).
fn value_missing(time: f64) -> String {
    format!("The predictor's flags or correction are missing at {time:.3} {TIME_UNIT}.")
}

/// The sentence of a run whose predictor is active at every sample, with its correction below the maximum (C9).
fn active_throughout() -> String {
    "The predictor is active at every sample and its correction stays below its maximum.".to_owned()
}

#[cfg(test)]
#[path = "smith_case_tests.rs"]
mod tests;
