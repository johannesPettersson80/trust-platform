//! The parts of the Validate answer that the page draws (specification 35, B5.4 and B9.6): the measured response of the
//! target-response case for the plot, the measured range and the declared bounds of an operating-range or state-range
//! requirement, whether Automatic generated a limit, the independent model check's row, and the words the table names its
//! rows with. Each is read from the runs and checks that the campaign already judged; none is a rule of its own.

use trust_control_math::sampling::plot_extrema::extrema_indices;

use crate::identification::connected_record::{ValidationRun, ValidationSample};
use crate::loop_definition::LoopDefinition;
use crate::messages::{
    LimitSide, ModelCheck, PlotSeries, RequirementChannel, RequirementOutcome, ValidateLabels,
    ValidationPlot, ValueRange,
};
use crate::recording::channels::ChannelQuality;
use crate::study::tune::prediction::{PLOT_GROUPS, PLOT_POINT_LIMIT};
use crate::tuning::automatic_criteria::AutomaticCriteria;
use crate::validation::engineer_verification_plan::VerificationPlan;
use crate::validation::run_validation::{RunCheck, RunRequirement};
use crate::validation::texts;

#[cfg(test)]
#[path = "answer_tests.rs"]
mod tests;

/// The words the table names its rows with (B9.9).
#[must_use]
pub(super) fn labels() -> ValidateLabels {
    ValidateLabels {
        current_settings: texts::CURRENT_SETTINGS.to_owned(),
        candidate: texts::CANDIDATE.to_owned(),
        candidate_overall: texts::CANDIDATE_OVERALL.to_owned(),
        automatic: texts::AUTOMATIC.to_owned(),
        guards: texts::GUARDS.to_owned(),
        guards_limit: texts::GUARDS_LIMIT.to_owned(),
    }
}

/// Whether a requirement is a guard (B9.9): a bounds or rate check that the plan generated from the loop's declaration, such as an
/// operating range, a state range, an output's rate limit or a fixed parameter's value (B5.3). A requirement the engineer sets, and
/// a settling, error or overshoot check, is never a guard, so the rule holds for every case of the campaign.
#[must_use]
pub(super) fn is_guard(requirement: &RunRequirement, plan: &VerificationPlan) -> bool {
    matches!(
        requirement.check,
        RunCheck::Bounds { .. } | RunCheck::StateBounds { .. } | RunCheck::Rate { .. }
    ) && plan
        .requirements
        .iter()
        .any(|generated| generated.id == requirement.id)
}

/// The Validate plot of the target-response case (B5.4, B9.6): the measured output of the candidate's run and of the current
/// settings' run, each reduced as B4.5 reduces the Tune plot. `None` when the candidate's run has no sample.
#[must_use]
pub(super) fn plot(
    loop_definition: &LoopDefinition,
    current: Option<&ValidationRun>,
    candidate: Option<&ValidationRun>,
) -> Option<ValidationPlot> {
    let candidate = candidate.filter(|run| !run.samples.is_empty())?;
    let output = &loop_definition.signals.process_variable;
    Some(ValidationPlot {
        title: texts::MEASURED_RESPONSE.to_owned(),
        role: texts::MEASUREMENT.to_owned(),
        unit: output.unit.clone(),
        tuned: series(candidate, &output.symbol, texts::TUNED),
        current: current
            .filter(|run| !run.samples.is_empty())
            .map(|run| series(run, &output.symbol, texts::CURRENT)),
    })
}

/// One run's trace of the output, reduced as B4.5 reduces the Tune plot: a run of more than 6,000 points is split into 1,000
/// consecutive groups, and each group keeps the points where the trace is lowest and highest, with the first and the last point.
/// A sample that is not good is a gap (B9.6): it is never kept as an extreme and its value is absent.
fn series(run: &ValidationRun, symbol: &str, name: &str) -> PlotSeries {
    let values: Vec<f64> = run
        .samples
        .iter()
        .map(|sample| good_value(sample, symbol).unwrap_or(f64::NAN))
        .collect();
    let kept = extrema_indices(
        values.len(),
        &[values.as_slice()],
        PLOT_GROUPS,
        PLOT_POINT_LIMIT,
    );
    PlotSeries {
        name: name.to_owned(),
        time_seconds: kept
            .iter()
            .map(|&index| run.samples[index].time_seconds)
            .collect(),
        values: kept
            .iter()
            .map(|&index| Some(values[index]).filter(|value| value.is_finite()))
            .collect(),
    }
}

/// The value of a channel at a sample when that value is good (A4.2): `None` for a missing or a forced value, and for a sample
/// that does not carry the channel.
fn good_value(sample: &ValidationSample, symbol: &str) -> Option<f64> {
    sample
        .values
        .get(symbol)
        .filter(|value| value.quality == ChannelQuality::Good)
        .and_then(|value| value.value)
}

/// The measured range and the declared bounds of an operating-range or state-range requirement (B5.4): the lowest and the
/// highest good value of its channel in its window, and the bounds the check holds. Any other check has neither.
#[must_use]
pub(super) fn bounds_of(
    requirement: &RunRequirement,
    run: &ValidationRun,
) -> (Option<ValueRange>, Option<ValueRange>) {
    match &requirement.check {
        RunCheck::Bounds { minimum, maximum } | RunCheck::StateBounds { minimum, maximum } => (
            measured_range(requirement, run),
            Some(ValueRange {
                minimum: *minimum,
                maximum: *maximum,
            }),
        ),
        _ => (None, None),
    }
}

/// The lowest and the highest good value of the requirement's channel over its window; `None` when no good value lies in it.
fn measured_range(requirement: &RunRequirement, run: &ValidationRun) -> Option<ValueRange> {
    run.samples
        .iter()
        .filter(|sample| {
            sample.time_seconds >= requirement.window.start
                && sample.time_seconds <= requirement.window.end
        })
        .filter_map(|sample| good_value(sample, &requirement.channel))
        .filter(|value| value.is_finite())
        .fold(None, |range, value| {
            Some(match range {
                None => ValueRange {
                    minimum: value,
                    maximum: value,
                },
                Some(range) => ValueRange {
                    minimum: range.minimum.min(value),
                    maximum: range.maximum.max(value),
                },
            })
        })
}

/// Whether Automatic generated the limit of a check (B4.3, B5.4): the settling time and the maximum error of the criteria record.
/// The other checks are the declared requirements, which Automatic never fills.
#[must_use]
pub(super) fn generated_limit(check: &RunCheck, criteria: Option<&AutomaticCriteria>) -> bool {
    let key = match check {
        RunCheck::Settling { .. } => "settlingTimeSeconds",
        RunCheck::Error { .. } => "maximumError",
        _ => return false,
    };
    criteria.is_some_and(|criteria| criteria.fields.iter().any(|field| field == key))
}

/// The independent model check as the table's first row (B5.2, N3.4): its normalized error and its limit, in percent of the
/// measured output's span. The row names no channel, so its name carries no unit.
#[must_use]
pub(super) fn model_agreement(check: &ModelCheck) -> RequirementOutcome {
    RequirementOutcome {
        id: "model-check".to_owned(),
        label: texts::MODEL_AGREEMENT.to_owned(),
        channel: RequirementChannel {
            role: String::new(),
            unit: String::new(),
            symbol: String::new(),
        },
        limit_side: LimitSide::AtMost,
        status: check.status,
        value: check.normalized_error.map(|error| error * 100.0),
        limit: Some(check.limit * 100.0),
        unit: "%".to_owned(),
        source: String::new(),
        reason: check.reason.clone(),
        automatic: false,
        guard: false,
        measured_range: None,
        limit_range: None,
    }
}
