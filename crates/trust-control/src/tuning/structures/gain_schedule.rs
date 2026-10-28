//! The value rules of gain scheduling (specification 35, C10; the rules of the TypeScript `gainScheduleGenerator.ts` that decide values,
//! and the rules C10 adds for the scheduling variable). A schedule holds two to four operating points, each with its own name and its own
//! two gain variables, and the values of the scheduling variable strictly increase in the order the points are given. Each scheduled gain is
//! bounded by ±10⁶, a range symmetric about zero, so that a loop whose output moves against its measurement (N6.3) has negative gains within it.
//! The choice between one fixed design and one design per point (D-19), the table of gains that the choice gives, the candidate that holds
//! that table, the span of each gain over the points (D-20), and the Identify precondition of a point (D-21) are here too. The generator
//! that writes one loop per point is `setup::schedule_loop`.

use std::collections::{BTreeMap, HashSet};

use super::texts;
use crate::identification::refusal::Refusal;
use crate::loop_definition::SchedulePoint;
use crate::tuning::candidate::CandidateSource;
use crate::tuning::project_candidate::ProjectCandidate;
use crate::tuning::proportional::GainBounds;
use crate::tuning::structure::{ParallelGains, Structure};

/// The fewest operating points of a schedule (C10).
pub const MINIMUM_POINTS: usize = 2;

/// The most operating points of a schedule (C10).
pub const MAXIMUM_POINTS: usize = 4;

/// The magnitude bound of each scheduled gain (N10).
pub const SCHEDULED_GAIN_MAGNITUDE: f64 = 1e6;

/// An operating point of a schedule: its name, and the program's proportional and integral gain variables for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatingPointGains {
    /// The point's name.
    pub name: String,
    /// The program's proportional gain variable for the point.
    pub proportional_gain: String,
    /// The program's integral gain variable for the point.
    pub integral_gain: String,
}

/// The choice between one fixed design and one design per point (C10 Tune, D-19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleChoice {
    /// The PI that Automatic designs on the first declared point's model, kept with the same gains at every point.
    Fixed,
    /// Each point's own PI, designed on that point's model.
    Scheduled,
}

/// The span of each gain over the schedule's points (C10 Validate, D-20): the largest minus the smallest value of that gain in the table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GainSpans {
    /// The span of the proportional gains.
    pub proportional: f64,
    /// The span of the integral gains.
    pub integral: f64,
}

/// Checks the operating points of a schedule (C10): two to four of them, each named once, with its own two gain variables.
///
/// The names are compared trimmed and in lower case, and the gain variables in upper case, so that a name or a variable that
/// differs only in case or in surrounding spaces is the same one.
///
/// # Errors
/// Returns a refusal when the number of points is outside two to four, or when a name is empty or repeated, or a gain variable is
/// repeated.
pub fn check_operating_points(points: &[OperatingPointGains]) -> Result<(), Refusal> {
    if !(MINIMUM_POINTS..=MAXIMUM_POINTS).contains(&points.len()) {
        return Err(Refusal::new(texts::SCHEDULE_POINTS));
    }
    let names: Vec<String> = points
        .iter()
        .map(|point| point.name.trim().to_lowercase())
        .collect();
    let symbols: Vec<String> = points
        .iter()
        .flat_map(|point| {
            [
                point.proportional_gain.to_uppercase(),
                point.integral_gain.to_uppercase(),
            ]
        })
        .collect();
    if names.iter().any(String::is_empty) || has_repeat(&names) || has_repeat(&symbols) {
        return Err(Refusal::new(texts::SCHEDULE_NAMES));
    }
    Ok(())
}

/// Checks that the operating points' values of the scheduling variable are finite and strictly increase in the order the points are
/// given (C10 Setup, the library's `Configure` rule). The refusal names the first point that is not above the point before it.
///
/// # Errors
/// A value is not finite, or a point's value is not above the value of the point before it.
pub fn check_point_order(points: &[(&str, f64)]) -> Result<(), Refusal> {
    if let Some((name, _)) = points.iter().find(|(_, value)| !value.is_finite()) {
        return Err(Refusal::new(non_finite_sentence(name)));
    }
    match points.windows(2).find(|pair| pair[1].1 <= pair[0].1) {
        Some(pair) => Err(Refusal::new(order_sentence(pair[1], pair[0]))),
        None => Ok(()),
    }
}

/// Checks an operating point's tolerance on the scheduling variable (C10 Setup, D-21): a finite number above zero. Setup gives none.
///
/// # Errors
/// The tolerance is not finite or is not above zero.
pub fn check_tolerance(tolerance: f64) -> Result<(), Refusal> {
    if tolerance.is_finite() && tolerance > 0.0 {
        Ok(())
    } else {
        Err(Refusal::new(TOLERANCE_NEEDED))
    }
}

/// The Identify precondition of a point (C10 Identify, D-21): the scheduling variable, read when Identify starts, must differ from the
/// point's value by no more than the point's tolerance. `unit` is the scheduling variable's unit, for the sentence.
///
/// # Errors
/// The recorded value is not finite, or it is outside the point's tolerance.
pub fn check_scheduling_value(
    recorded: f64,
    point: &SchedulePoint,
    unit: &str,
) -> Result<(), Refusal> {
    let allowed = point.tolerance * (1.0 + ROUNDING_ALLOWANCE);
    if recorded.is_finite() && (recorded - point.value).abs() <= allowed {
        return Ok(());
    }
    Err(Refusal::new(format!(
        "The scheduling variable reads {}, outside the tolerance of {} around operating point {} ({}). Bring the process to that value in Manual, then identify.",
        with_unit(recorded, unit),
        with_unit(point.tolerance, unit),
        point.name,
        with_unit(point.value, unit),
    )))
}

/// The choice between the fixed design and one design per point (C10 Tune, D-19). `fixed_meets_requirements` holds, for each declared point
/// in order, whether the fixed design's predicted response on that point's model meets the point's requirements. The schedule is
/// recommended only when the fixed design fails a requirement at one of the points; otherwise the fixed design is.
#[must_use]
pub fn choose_schedule(fixed_meets_requirements: &[bool]) -> ScheduleChoice {
    if fixed_meets_requirements.iter().all(|meets| *meets) {
        ScheduleChoice::Fixed
    } else {
        ScheduleChoice::Scheduled
    }
}

/// The gains of each point under a choice (C10 Tune, D-19): under the fixed design, the first point's PI at every point; under the
/// scheduled design, each point's own PI. `designs` holds each point's own PI in the order of the points.
#[must_use]
pub fn gains_per_point(choice: ScheduleChoice, designs: &[ParallelGains]) -> Vec<ParallelGains> {
    match (choice, designs.first()) {
        (ScheduleChoice::Fixed, Some(first)) => vec![*first; designs.len()],
        _ => designs.to_vec(),
    }
}

/// The values that one schedule transaction writes (C10 Apply, Validate and Tune): the proportional and integral gain of every point, from
/// the gains that the table gives that point. `gains` holds each point's gains in the order of the table.
///
/// # Errors
/// The gains do not give one pair for each point, or a variable is named by two of the gains.
pub fn table_values(
    points: &[SchedulePoint],
    gains: &[ParallelGains],
) -> Result<BTreeMap<String, f64>, Refusal> {
    if points.len() != gains.len() {
        return Err(Refusal::new(GAINS_PER_TABLE));
    }
    let mut values = BTreeMap::new();
    for (point, gain) in points.iter().zip(gains) {
        for (symbol, value) in [
            (&point.proportional_symbol, gain.proportional_gain),
            (&point.integral_symbol, gain.integral_gain),
        ] {
            if values.insert(symbol.clone(), value).is_some() {
                return Err(Refusal::new(DUPLICATE_VARIABLE));
            }
        }
    }
    Ok(values)
}

/// The candidate of one point of a schedule (C10 Tune): the point's own gains, and the values of the whole table, so that the candidate of
/// any point writes every point's gains at Apply and Validate. `own` is the point's index in the table.
///
/// # Errors
/// The table's values refuse (see [`table_values`]), or `own` is not a point of the table.
pub fn schedule_candidate(
    points: &[SchedulePoint],
    gains: &[ParallelGains],
    own: usize,
    source: CandidateSource,
) -> Result<ProjectCandidate, Refusal> {
    let values = table_values(points, gains)?;
    let own_gains = gains
        .get(own)
        .copied()
        .ok_or_else(|| Refusal::new(GAINS_PER_TABLE))?;
    Ok(ProjectCandidate {
        applied: false,
        source,
        structure: Structure::PI,
        gains: own_gains,
        values,
    })
}

/// The span of each gain over the schedule's points (C10 Validate, D-20). A gain that has one value at every point has a span of zero.
#[must_use]
pub fn gain_spans(gains: &[ParallelGains]) -> GainSpans {
    let proportional: Vec<f64> = gains.iter().map(|gain| gain.proportional_gain).collect();
    let integral: Vec<f64> = gains.iter().map(|gain| gain.integral_gain).collect();
    GainSpans {
        proportional: span_of(&proportional),
        integral: span_of(&integral),
    }
}

/// The span of one gain over the points: the largest minus the smallest of its values, zero when there is none or one.
#[must_use]
pub fn span_of(values: &[f64]) -> f64 {
    let mut iter = values.iter().copied();
    match iter.next() {
        None => 0.0,
        Some(first) => {
            let (minimum, maximum) = iter.fold((first, first), |(low, high), value| {
                (low.min(value), high.max(value))
            });
            maximum - minimum
        }
    }
}

/// The bounds of each scheduled gain: `±SCHEDULED_GAIN_MAGNITUDE`, symmetric about zero (C10, N10).
#[must_use]
pub fn scheduled_gain_bounds() -> GainBounds {
    GainBounds {
        minimum: -SCHEDULED_GAIN_MAGNITUDE,
        maximum: SCHEDULED_GAIN_MAGNITUDE,
    }
}

/// Whether a value appears more than once.
fn has_repeat(values: &[String]) -> bool {
    let unique: HashSet<&String> = values.iter().collect();
    unique.len() != values.len()
}

/// A value with its unit, or the bare value when the unit is blank.
fn with_unit(value: f64, unit: &str) -> String {
    if unit.is_empty() {
        format!("{value}")
    } else {
        format!("{value} {unit}")
    }
}

/// The allowance on a tolerance for the rounding of decimal values: `0.10 − 0.09` is not exactly `0.01` in binary, so a reading at the
/// tolerance is accepted. The allowance is one part in 10⁹ of the tolerance.
const ROUNDING_ALLOWANCE: f64 = 1e-9;

/// The refusal of a tolerance that is not above zero.
const TOLERANCE_NEEDED: &str =
    "Give each operating point a tolerance above zero, in the scheduling variable's unit.";

/// The refusal of a table whose gains do not give one pair for each point.
const GAINS_PER_TABLE: &str = "A schedule needs one pair of gains for each operating point.";

/// The refusal of a table that names one variable for two of its gains: one transaction cannot write it twice.
const DUPLICATE_VARIABLE: &str =
    "Two gains of the schedule name the same variable, so one transaction cannot write both.";

/// The sentence of a value that is not finite.
fn non_finite_sentence(name: &str) -> String {
    format!("The operating point {name} needs a finite value of the scheduling variable.")
}

/// The sentence of a point that is not above the point before it. `current` and `previous` are the points' names and values.
fn order_sentence(current: (&str, f64), previous: (&str, f64)) -> String {
    format!(
        "The operating points must increase in the order they are given: {} at {} is not above {} at {}.",
        current.0, current.1, previous.0, previous.1
    )
}

#[cfg(test)]
#[path = "gain_schedule_tests.rs"]
mod tests;
