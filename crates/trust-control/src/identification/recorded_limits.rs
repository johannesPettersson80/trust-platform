//! The limits check of a connected recording (specification 35, B3.4 step 6 and B3.9; A7): before the fit, every recorded
//! sample of the plant input and of the measured output is checked against the loop's limits. This is the evaluation a
//! connected record takes in production. The full requirement evaluation of N8 is stage V3 (Validate) and is not here.
//!
//! A sample outside its bounds makes its check Fail. A sample without a good, finite value makes its check Inconclusive,
//! which is not Pass. The first problem in time order decides a check, and the first check with a problem decides the
//! evaluation. Its finding names the signal, the time from the recording's origin, and the value and the bound with their
//! units (A11.1). The specification names only these two limits, so the other recorded signals are not checked here.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::experiment::number_text::{js_number, js_to_precision};
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::connected_record::{BoundsCheck, ValidationRun, ValidationValue};
use crate::identification::independent_validation::{RequirementStatus, RunEvaluation};
use crate::recording::channels::ChannelQuality;

/// The digits a finding writes its values and times with (`toPrecision(4)`).
const FINDING_DIGITS: usize = 4;

/// The limits check of a connected run (B3.4 step 6): its verdict, the checks it made, and what it found when a check does not
/// pass. The record stores the checks, and the decoder reads them back to make the same evaluation again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordedLimitsEvaluation {
    /// Pass when every sample of every check is good and within its bounds; otherwise the verdict of the first problem.
    pub status: RequirementStatus,
    /// The checks the evaluation made: one for the plant input and one for the measured output, over the whole recording.
    pub specification: Vec<BoundsCheck>,
    /// What the first problem found, in plain words; absent when every check passes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finding: Option<String>,
}

impl RunEvaluation for RecordedLimitsEvaluation {
    fn status(&self) -> RequirementStatus {
        self.status
    }

    fn finding(&self) -> Option<&str> {
        self.finding.as_deref()
    }
}

/// What a check found at its first problem.
struct Problem {
    /// Fail for a sample outside its bounds, Inconclusive for a sample without a good value.
    status: RequirementStatus,
    /// The sentence that names the signal, the time and the values involved.
    finding: String,
}

/// Evaluates a connected run against its checks, the production evaluation of the connected record (B3.4 step 6). The checks
/// are taken in order, and each over its window in time order; the first problem found decides the evaluation.
///
/// # Errors
/// A check names a signal that the run does not hold.
pub fn evaluate_recorded_limits(
    run: &ValidationRun,
    checks: &[BoundsCheck],
) -> Result<RecordedLimitsEvaluation, Failure> {
    let mut problem = None;
    for check in checks {
        problem = first_problem(run, check)?;
        if problem.is_some() {
            break;
        }
    }
    let (status, finding) = match problem {
        Some(problem) => (problem.status, Some(problem.finding)),
        None => (RequirementStatus::Pass, None),
    };
    Ok(RecordedLimitsEvaluation {
        status,
        specification: checks.to_vec(),
        finding,
    })
}

/// The first sample of a check, in time order, that is not a good value or lies outside the bounds; `None` when every sample in
/// the window is good and within the bounds.
fn first_problem(run: &ValidationRun, check: &BoundsCheck) -> Result<Option<Problem>, Failure> {
    let unit = run
        .channels
        .iter()
        .find(|channel| channel.id == check.channel)
        .map(|channel| channel.unit.as_str())
        .ok_or_else(|| {
            Failure::new(
                &format!(
                    "The limits check names {}, which the recording does not hold.",
                    check.channel
                ),
                Cause::Implementation,
                NextAction::None,
            )
        })?;
    let window = check.window.start..=check.window.end;
    let in_window = run
        .samples
        .iter()
        .filter(|sample| window.contains(&sample.time_seconds));
    for sample in in_window {
        let time = format!(
            "{} s",
            js_number(js_to_precision(sample.time_seconds, FINDING_DIGITS))
        );
        match sample.values.get(&check.channel) {
            Some(ValidationValue {
                value: Some(value),
                quality: ChannelQuality::Good,
            }) if value.is_finite() => {
                let value = *value;
                let outside = if value > check.maximum {
                    Some(("above", "maximum", check.maximum))
                } else if value < check.minimum {
                    Some(("below", "minimum", check.minimum))
                } else {
                    None
                };
                if let Some((side, limit_name, bound)) = outside {
                    return Ok(Some(Problem {
                        status: RequirementStatus::Fail,
                        finding: format!(
                            "{} is {} at {time}, {side} its {limit_name} of {}.",
                            check.channel,
                            quantity(value, unit),
                            quantity(bound, unit)
                        ),
                    }));
                }
            }
            reading => {
                return Ok(Some(Problem {
                    status: RequirementStatus::Inconclusive,
                    finding: format!(
                        "{} has no good value at {time}: {}.",
                        check.channel,
                        missing_reason(reading)
                    ),
                }));
            }
        }
    }
    Ok(None)
}

/// A number with its unit as a finding writes it, such as `104 degC`; the number alone when the unit is empty.
fn quantity(value: f64, unit: &str) -> String {
    let number = js_number(js_to_precision(value, FINDING_DIGITS));
    if unit.is_empty() {
        number
    } else {
        format!("{number} {unit}")
    }
}

/// Why a reading is not a good value: missing, bad, forced, or not a finite number.
fn missing_reason(reading: Option<&ValidationValue>) -> &'static str {
    match reading {
        None | Some(ValidationValue { value: None, .. }) => "missing",
        Some(ValidationValue {
            quality: ChannelQuality::Bad,
            ..
        }) => "bad",
        Some(ValidationValue {
            quality: ChannelQuality::Forced,
            ..
        }) => "forced",
        Some(ValidationValue {
            value: Some(_),
            quality: ChannelQuality::Good,
        }) => "not a finite number",
    }
}

#[cfg(test)]
#[path = "recorded_limits_tests.rs"]
mod tests;
