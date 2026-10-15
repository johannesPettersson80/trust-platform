//! The Validate rules of a gain schedule (specification 35, C10 Validate; numerical annex N10). A run of one point is on that point's model,
//! starts the scheduling variable at the point's value, and writes the gains of every point of the table, so that the program interpolates
//! the candidate's schedule. The run records the active gains that the program interpolates, which must change continuously: the change
//! between two consecutive recorded samples is at most the continuity fraction of the gain's span over the schedule's points. The rules are
//! pure; the runs and their records are the caller's (`validation::run_requests`, `study::validate`), which add a completed candidate run's
//! continuity requirement with `add_continuity_result`.

use crate::commissioning::gain_schedule_adapter::gains_of_table;
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::connected_record::ValidationRun;
use crate::identification::independent_validation::RequirementStatus;
use crate::loop_definition::{
    GainSchedule, LoopDefinition, Parameter, ParameterBinding, RealType, SchedulePoint,
    SchedulingVariable,
};
use crate::recording::channels::ChannelQuality;
use crate::tuning::project_candidate::ProjectCandidate;
use crate::tuning::structure::ParallelGains;
use crate::tuning::structures::gain_schedule::{gain_spans, GainSpans};
use crate::validation::isolated_run_protocol::{IsolatedParameterOverride, IsolatedParameterType};
use crate::validation::isolated_run_sampling::isolated_record_every;
use crate::validation::requirement_record::RunRequirementResult;
use crate::validation::run_validation::RecordedRunEvaluation;

/// The continuity fraction (C10 Validate, N10): the change of an active gain between two consecutive recorded samples is at most this
/// fraction of the gain's span over the schedule's points, the largest minus the smallest value of that gain in the table (D-20).
pub const CONTINUITY_FRACTION: f64 = 0.10;

/// The refusal of a run whose gains do not cover the points of the table (C10 Validate).
const GAINS_PER_POINT: &str = "A schedule run needs one pair of gains for each operating point.";

/// The refusal of a loop whose parameter binding does not hold its point's gains (C10 Setup).
const NO_POINT_GAIN: &str =
    "The schedule's loop has no parameter for its operating point's gains, so a run cannot write the schedule.";

/// The refusal of a schedule's candidate that holds no gain table, so that the continuity of its active gains cannot be judged (C10).
const NO_GAIN_TABLE: &str =
    "A schedule's candidate holds no gain table, so the continuity of its active gains cannot be judged.";

/// The identifier and the wire kind of the continuity requirement in a run's verdicts (C10 Validate).
const CONTINUITY_ID: &str = "active-gains-change-continuously";
const CONTINUITY_KIND: &str = "gain-continuity";

/// The active gains in the words of the verdicts' sentences.
const PROPORTIONAL_NAME: &str = "proportional gain";
const INTEGRAL_NAME: &str = "integral gain";

/// The sentence of a run whose active gains all change continuously across the schedule (C10 Validate).
const CONTINUOUS: &str =
    "The active gains change continuously across the schedule: every change between two recorded samples is within its continuity limit.";

/// The sentence of a candidate run that averages its samples: its recording cannot judge the continuity of the active gains (C10 Validate).
const AVERAGED: &str =
    "The run averages its samples, so the continuity of the active gains cannot be judged from the recording.";

/// The largest change allowed between two consecutive samples of an active gain whose span over the schedule's points is `span` (C10
/// Validate, D-20). A gain with span zero may not change.
#[must_use]
pub fn continuity_limit(span: f64) -> f64 {
    CONTINUITY_FRACTION * span
}

/// The index of the first recorded sample that breaks the continuity rule (C10 Validate, D-20): a sample that is not a finite number, or a
/// change from the sample before it that is above `continuity_limit(span)`. `None` when no sample breaks the rule.
fn first_break(samples: &[f64], span: f64) -> Option<usize> {
    let limit = continuity_limit(span);
    samples.iter().enumerate().find_map(|(index, &value)| {
        if !value.is_finite() {
            return Some(index);
        }
        let before = *samples[..index].last()?;
        ((value - before).abs() > limit).then_some(index)
    })
}

/// Whether the recorded samples of an active gain change continuously (C10 Validate, D-20): every sample is finite, and between two
/// consecutive samples the change is at most `continuity_limit(span)`, where `span` is the gain's span over the schedule's points. A hidden or
/// discontinuous schedule fails this check.
#[must_use]
pub fn continuity_holds(samples: &[f64], span: f64) -> bool {
    first_break(samples, span).is_none()
}

/// The variables that hold the active gains the program interpolates: the record's `ProportionalGain` and `IntegralGain` (C10 Setup,
/// `parameterRecord`).
#[must_use]
pub fn active_gain_symbols(parameter_record: &str) -> (String, String) {
    (
        format!("{parameter_record}.ProportionalGain"),
        format!("{parameter_record}.IntegralGain"),
    )
}

/// The start of a run at an operating point (C10 Validate): the scheduling variable starts at the point's value, so the run starts
/// from that operating point.
#[must_use]
pub fn run_start(scheduling: &SchedulingVariable, point: &SchedulePoint) -> (String, f64) {
    (scheduling.symbol.clone(), point.value)
}

/// The parameter overrides of a run of the schedule's candidate (C10 Validate): the proportional and integral gain of every point, each
/// with the type, unit and bounds of the loop's own parameter for that role. `gains` holds each point's gains in the order of the table.
///
/// # Errors
/// The gains do not give one pair for each point, or the loop's parameter binding holds no gain for its own point.
pub fn run_overrides(
    loop_definition: &LoopDefinition,
    schedule: &GainSchedule,
    gains: &[ParallelGains],
) -> Result<Vec<IsolatedParameterOverride>, Failure> {
    if gains.len() != schedule.points.len() {
        return Err(defect(GAINS_PER_POINT));
    }
    let binding = loop_definition
        .parameter_binding
        .as_ref()
        .ok_or_else(|| defect(NO_POINT_GAIN))?;
    let own = schedule.own_point().ok_or_else(|| defect(NO_POINT_GAIN))?;
    let proportional = binding_parameter(binding, &own.proportional_symbol)?;
    let integral = binding_parameter(binding, &own.integral_symbol)?;
    let mut overrides = Vec::with_capacity(2 * schedule.points.len());
    for (point, gain) in schedule.points.iter().zip(gains) {
        overrides.push(override_of(
            proportional,
            &point.proportional_symbol,
            &format!("Scheduled proportional gain at {}", point.name),
            gain.proportional_gain,
        ));
        overrides.push(override_of(
            integral,
            &point.integral_symbol,
            &format!("Scheduled integral gain at {}", point.name),
            gain.integral_gain,
        ));
    }
    Ok(overrides)
}

/// The parameter of the binding that holds a symbol.
///
/// # Errors
/// The binding holds no parameter with the symbol.
fn binding_parameter<'a>(
    binding: &'a ParameterBinding,
    symbol: &str,
) -> Result<&'a Parameter, Failure> {
    binding
        .parameters
        .iter()
        .find(|parameter| parameter.symbol == symbol)
        .ok_or_else(|| defect(NO_POINT_GAIN))
}

/// One gain as a run's override: the symbol, the type, unit and bounds of the role's parameter, its convention and its value.
fn override_of(
    parameter: &Parameter,
    symbol: &str,
    convention: &str,
    value: f64,
) -> IsolatedParameterOverride {
    IsolatedParameterOverride {
        symbol: symbol.to_owned(),
        iec_type: match parameter.iec_type {
            RealType::Real => IsolatedParameterType::Real,
            RealType::Lreal => IsolatedParameterType::Lreal,
        },
        unit: parameter.unit.clone(),
        convention: convention.to_owned(),
        minimum: parameter.minimum,
        maximum: parameter.maximum,
        value,
    }
}

/// The schedule that a loop declares (C10), or `None` for a loop that is not a `gain-scheduled` law.
#[must_use]
pub fn schedule_of_loop(loop_definition: &LoopDefinition) -> Option<&GainSchedule> {
    loop_definition
        .controller
        .project
        .as_ref()?
        .schedule
        .as_ref()
}

/// The start of a run at the loop's own operating point (C10 Validate): the scheduling variable and its value. `None` for a loop that is not
/// a schedule.
#[must_use]
pub fn scheduled_start(loop_definition: &LoopDefinition) -> Option<(String, f64)> {
    let schedule = schedule_of_loop(loop_definition)?;
    Some(run_start(&schedule.scheduling, schedule.own_point()?))
}

/// Whether the runs of a schedule record every scan (C10 Validate, A7): the continuity of the active gains is judged on the recorded samples,
/// so a schedule's run takes the record period of 1 when its grid points and their cells fit the isolated limits. Its case plan is shared by
/// its current settings' run and its candidate's, so both record every scan. `false` for a loop that is not a schedule, and for a run that
/// does not fit: its record period then follows the process (A4.2).
#[must_use]
pub fn records_every_sample(
    loop_definition: &LoopDefinition,
    step_ms: u64,
    duration_ms: u64,
    observed: usize,
) -> bool {
    let channels = u64::try_from(observed).unwrap_or(u64::MAX);
    schedule_of_loop(loop_definition).is_some()
        && isolated_record_every(step_ms, duration_ms, channels) == 1
}

/// The active gains that a run of a schedule's loop observes (C10 Validate), the proportional one and then the integral one. A loop that is
/// not a schedule observes none.
#[must_use]
pub fn observed_active_gains(loop_definition: &LoopDefinition) -> Vec<String> {
    schedule_of_loop(loop_definition).map_or_else(Vec::new, |schedule| {
        let (proportional, integral) = active_gain_symbols(&schedule.parameter_record);
        vec![proportional, integral]
    })
}

/// The unit that a schedule's loop declares for one of its active gains (C10 Validate): the unit of the own point's gain variable of the same
/// role, in the loop's binding. `None` for any other symbol.
#[must_use]
pub fn active_gain_unit(loop_definition: &LoopDefinition, symbol: &str) -> Option<String> {
    let schedule = schedule_of_loop(loop_definition)?;
    let own = schedule.own_point()?;
    let (proportional, integral) = active_gain_symbols(&schedule.parameter_record);
    let variable = if symbol == proportional.as_str() {
        own.proportional_symbol.as_str()
    } else if symbol == integral.as_str() {
        own.integral_symbol.as_str()
    } else {
        return None;
    };
    loop_definition
        .parameter_binding
        .as_ref()?
        .parameters
        .iter()
        .find(|parameter| parameter.symbol == variable)
        .map(|parameter| parameter.unit.clone())
}

/// What the continuity requirement of a schedule's candidate reads (C10 Validate, D-20): the two active gains with their units, the span of
/// each gain over the table that the candidate writes, and whether the candidate's run averages its samples.
#[derive(Debug, Clone, PartialEq)]
pub struct ContinuityRule {
    proportional_symbol: String,
    integral_symbol: String,
    proportional_unit: String,
    integral_unit: String,
    spans: GainSpans,
    /// Whether the candidate's run averages its samples, that is its record period is above 1 (A4.2): its recording cannot judge continuity.
    averaged: bool,
}

/// The continuity rule of a loop's candidate (C10 Validate, D-20), for the candidate's run at the record period `record_every` that the run
/// request was built with (A4.2). `None` for a loop that is not a schedule.
///
/// # Errors
/// The loop is a schedule and the candidate is not a program's law with a gain table, the table does not give each point its gains, or the
/// loop's binding holds no gain variable of its own point.
pub fn continuity_rule(
    loop_definition: &LoopDefinition,
    candidate: Option<&ProjectCandidate>,
    record_every: u64,
) -> Result<Option<ContinuityRule>, Failure> {
    let Some(schedule) = schedule_of_loop(loop_definition) else {
        return Ok(None);
    };
    let candidate = candidate.ok_or_else(|| defect(NO_GAIN_TABLE))?;
    let spans = gain_spans(&gains_of_table(&schedule.points, &candidate.values)?);
    let (proportional_symbol, integral_symbol) = active_gain_symbols(&schedule.parameter_record);
    let proportional_unit = active_gain_unit(loop_definition, &proportional_symbol)
        .ok_or_else(|| defect(NO_POINT_GAIN))?;
    let integral_unit =
        active_gain_unit(loop_definition, &integral_symbol).ok_or_else(|| defect(NO_POINT_GAIN))?;
    Ok(Some(ContinuityRule {
        proportional_symbol,
        integral_symbol,
        proportional_unit,
        integral_unit,
        spans,
        averaged: record_every > 1,
    }))
}

/// One active gain's recorded series, as the continuity verdict reads it (C10 Validate, D-20): the gain in words and its unit, its span over
/// the schedule's points, and its recorded samples as (time in s, value), in order. `samples` is `None` when a sample holds no valid value.
#[derive(Debug, Clone, Copy)]
pub struct ActiveGain<'a> {
    /// The gain in words, such as `proportional gain`.
    pub name: &'a str,
    /// The gain's unit.
    pub unit: &'a str,
    /// The gain's span over the schedule's points (D-20).
    pub span: f64,
    /// The recorded samples, in order, as the time in s and the value.
    pub samples: Option<&'a [(f64, f64)]>,
}

/// The verdict on the active gains of a completed candidate run, as one requirement (C10 Validate, D-20): Pass when every gain changes
/// continuously, Fail at the first gain that breaks, and Inconclusive when a gain has no recorded series. A break is judged before a missing
/// series, so that a broken schedule fails whatever else the run holds.
#[must_use]
pub fn continuity_verdict(gains: &[ActiveGain<'_>]) -> RunRequirementResult {
    let mut missing: Option<&str> = None;
    for gain in gains {
        let Some(samples) = gain.samples.filter(|samples| !samples.is_empty()) else {
            missing = missing.or(Some(gain.name));
            continue;
        };
        if let Some(breach) = breach_of(gain, samples) {
            return row(
                RequirementStatus::Fail,
                gain.unit,
                breach.value,
                breach.limit,
                breach.reason,
            );
        }
    }
    match missing {
        Some(name) => row(
            RequirementStatus::Inconclusive,
            "",
            None,
            None,
            no_series(name),
        ),
        None => row(
            RequirementStatus::Pass,
            "",
            None,
            None,
            CONTINUOUS.to_owned(),
        ),
    }
}

/// A break of a gain's recorded series (C10 Validate, D-20): the measured change and its limit when the break is a change, and the sentence
/// that says where it is.
struct Breach {
    value: Option<f64>,
    limit: Option<f64>,
    reason: String,
}

/// The first break of a gain's recorded series, with the sentence that says where it is, or `None` when the series holds the rule.
fn breach_of(gain: &ActiveGain<'_>, samples: &[(f64, f64)]) -> Option<Breach> {
    let values: Vec<f64> = samples.iter().map(|&(_, value)| value).collect();
    let index = first_break(&values, gain.span)?;
    let &(time, value) = samples.get(index)?;
    let earlier = index.checked_sub(1).and_then(|before| samples.get(before));
    if let Some(&(from, before)) = earlier.filter(|_| value.is_finite()) {
        let change = (value - before).abs();
        let limit = continuity_limit(gain.span);
        return Some(Breach {
            value: Some(change),
            limit: Some(limit),
            reason: step_break(gain.name, gain.unit, from, time, change, limit),
        });
    }
    Some(Breach {
        value: None,
        limit: None,
        reason: not_finite_break(gain.name, time),
    })
}

/// The sentence of a gain whose change between two recorded samples is above its continuity limit (C10 Validate, D-20).
fn step_break(name: &str, unit: &str, from: f64, to: f64, change: f64, limit: f64) -> String {
    format!(
        "The active gains do not change continuously across the schedule: the {name} changes by {change:.6} {unit} between the samples at {from:.3} s and {to:.3} s, above its continuity limit of {limit:.6} {unit}."
    )
}

/// The sentence of a gain that is not a finite number at a recorded sample (C10 Validate).
fn not_finite_break(name: &str, time: f64) -> String {
    format!(
        "The active gains do not change continuously across the schedule: the {name} is not a finite number at {time:.3} s."
    )
}

/// The sentence of a gain whose recorded series is missing: a sample holds no valid value of it (A4.2, B5.4).
fn no_series(name: &str) -> String {
    format!(
        "The active gains cannot be checked for continuity: the run holds no valid recorded value of the {name} at every sample."
    )
}

/// The requirement row of the continuity check (B5.4): its identifier and wire kind, the status, the measured value and its limit when there
/// are some, their unit, and the sentence.
fn row(
    status: RequirementStatus,
    unit: &str,
    value: Option<f64>,
    limit: Option<f64>,
    reason: String,
) -> RunRequirementResult {
    RunRequirementResult {
        id: CONTINUITY_ID.to_owned(),
        kind: CONTINUITY_KIND.to_owned(),
        status,
        value,
        limit,
        unit: unit.to_owned(),
        reason,
    }
}

/// Adds the continuity requirement to a completed candidate run's evaluation (C10 Validate, D-20): its verdict, and the aggregate status with
/// it. A run that averages its samples is Inconclusive whatever its samples hold. The caller judges only a candidate's run, because only the
/// candidate writes the schedule's table.
pub fn add_continuity_result(
    rule: &ContinuityRule,
    run: &ValidationRun,
    evaluation: &mut RecordedRunEvaluation,
) {
    let result = if rule.averaged {
        row(
            RequirementStatus::Inconclusive,
            "",
            None,
            None,
            AVERAGED.to_owned(),
        )
    } else {
        let proportional = recorded_series(run, &rule.proportional_symbol);
        let integral = recorded_series(run, &rule.integral_symbol);
        continuity_verdict(&[
            ActiveGain {
                name: PROPORTIONAL_NAME,
                unit: rule.proportional_unit.as_str(),
                span: rule.spans.proportional,
                samples: proportional.as_deref(),
            },
            ActiveGain {
                name: INTEGRAL_NAME,
                unit: rule.integral_unit.as_str(),
                span: rule.spans.integral,
                samples: integral.as_deref(),
            },
        ])
    };
    evaluation.status = combined(evaluation.status, result.status);
    evaluation.requirements.push(result);
}

/// The recorded values of a symbol over a run, as (time in s, value), in order. `None` when a sample holds no valid value of the symbol: a
/// valid value is good or forced, and a bad or missing one is not (A4.2).
fn recorded_series(run: &ValidationRun, symbol: &str) -> Option<Vec<(f64, f64)>> {
    run.samples
        .iter()
        .map(|sample| {
            let channel = sample.values.get(symbol)?;
            let valid = matches!(
                channel.quality,
                ChannelQuality::Good | ChannelQuality::Forced
            );
            channel
                .value
                .filter(|_| valid)
                .map(|value| (sample.time_seconds, value))
        })
        .collect()
}

/// The aggregate of a run's verdict and the continuity verdict: Fail when either fails, Inconclusive when either is, and Pass otherwise.
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

/// A defect of the schedule's loop or its gains: the checks before a run refuse them, so reaching one is an implementation fault.
fn defect(text: &str) -> Failure {
    Failure::new(text, Cause::Implementation, NextAction::None)
}

#[cfg(test)]
#[path = "gain_schedule_case_tests.rs"]
mod tests;
