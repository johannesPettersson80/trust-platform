//! The value rules of gain scheduling (specification 35, C10; the rules of the TypeScript `gainScheduleGenerator.ts` that decide values).
//! A schedule holds two to four operating points, each with its own name and its own two gain variables. Each scheduled gain is bounded
//! by ±10⁶, a range symmetric about zero, so that a loop whose output moves against its measurement (N6.3) has negative gains within it.
//! The generator that writes one loop per point is not part of this module.

use std::collections::HashSet;

use super::texts;
use crate::identification::refusal::Refusal;
use crate::tuning::proportional::GainBounds;

/// The fewest operating points of a schedule (C10).
pub const MINIMUM_POINTS: usize = 2;

/// The most operating points of a schedule (C10).
pub const MAXIMUM_POINTS: usize = 4;

/// The magnitude bound of each scheduled gain.
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

/// The bounds of each scheduled gain: `±SCHEDULED_GAIN_MAGNITUDE`, symmetric about zero (C10, N6.3).
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

#[cfg(test)]
#[path = "gain_schedule_tests.rs"]
mod tests;
