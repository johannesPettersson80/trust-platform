//! The schedule block of a `gain-scheduled` project law (specification 35, C10): the record the PID is configured from, the scheduling
//! variable, the operating points in the order of their values, and the point this loop tunes. Every loop of one schedule holds the whole
//! table, so each loop names its siblings and every gain that Apply and Validate write for the schedule. The setup writes the block, and
//! the decoder reads it under the same rules (`tuning::structures::gain_schedule`).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{ProjectSeedBlock, RealType, Structure};
use crate::json::decode_value;
use crate::tuning::project_seed::ParallelPidSeed;
use crate::tuning::structures::gain_schedule::{
    check_operating_points, check_point_order, check_tolerance, OperatingPointGains,
};

/// The family a schedule's project block names (C10), one of the project families.
pub const FAMILY: &str = "gain-scheduled";

/// The key of the block in `controller.project`.
const KEY: &str = "schedule";

/// The sentence of a schedule whose family or seed does not match it (C10).
const SEED_SENTENCE: &str =
    "A gain schedule's seed is the PI law of its own operating point's proportional and integral gain variables.";

/// The variable the schedule is scheduled by, its unit and its IEC type (C10 Setup). The loop keeps the type that setup read from the
/// compiled program, because the runtime reads a variable only as its exact type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchedulingVariable {
    /// The variable's path.
    pub symbol: String,
    /// The variable's unit.
    pub unit: String,
    /// The variable's IEC type, `REAL` or `LREAL`.
    pub iec_type: RealType,
}

/// One operating point of a schedule (C10 Setup): its name, the loop that tunes it, its value of the scheduling variable with the
/// tolerance on that value, and the program's proportional and integral gain variables for it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchedulePoint {
    /// The point's name, trimmed.
    pub name: String,
    /// The loop that tunes and applies this point.
    pub loop_id: String,
    /// The point's value of the scheduling variable, in its unit.
    pub value: f64,
    /// How far the scheduling variable may differ from `value` when Identify starts (D-21), in its unit.
    pub tolerance: f64,
    /// The program's proportional gain variable for the point.
    pub proportional_symbol: String,
    /// The program's integral gain variable for the point.
    pub integral_symbol: String,
}

/// A schedule as one loop declares it (C10): the record the PID is configured from, the scheduling variable, the operating points in the
/// order given, and the name of this loop's point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GainSchedule {
    /// The record the PID is configured from. Its `ProportionalGain` and `IntegralGain` hold the gains the program interpolates.
    pub parameter_record: String,
    /// The variable the schedule is scheduled by.
    pub scheduling: SchedulingVariable,
    /// The name of the point this loop tunes.
    pub point: String,
    /// The operating points, in the order the setup gave them: two to four, with strictly increasing values.
    pub points: Vec<SchedulePoint>,
}

impl GainSchedule {
    /// Reads a schedule block and checks it (C10 Setup): the parameter record and the scheduling variable are named, the points follow the
    /// rules of `check_operating_points`, their values strictly increase in the order given, each tolerance is above zero, and the point
    /// this loop tunes is one of them.
    ///
    /// # Errors
    /// The block is not a schedule of the form of C10, or breaks one of those rules. The text is the sentence the setup shows.
    pub fn from_value(value: &Value) -> Result<Self, String> {
        let schedule: Self = decode_value(value.clone())
            .map_err(|error| format!("controller.project.schedule is not a schedule: {error}"))?;
        schedule.check()?;
        Ok(schedule)
    }

    /// The rules that the fields alone do not give (see [`Self::from_value`]).
    fn check(&self) -> Result<(), String> {
        if self.parameter_record.trim().is_empty() || self.scheduling.symbol.trim().is_empty() {
            return Err(
                "controller.project.schedule needs its parameter record and its scheduling variable."
                    .to_owned(),
            );
        }
        let gains: Vec<OperatingPointGains> = self
            .points
            .iter()
            .map(|point| OperatingPointGains {
                name: point.name.clone(),
                proportional_gain: point.proportional_symbol.clone(),
                integral_gain: point.integral_symbol.clone(),
            })
            .collect();
        check_operating_points(&gains).map_err(|refusal| refusal.0)?;
        let values: Vec<(&str, f64)> = self
            .points
            .iter()
            .map(|point| (point.name.as_str(), point.value))
            .collect();
        check_point_order(&values).map_err(|refusal| refusal.0)?;
        for point in &self.points {
            check_tolerance(point.tolerance).map_err(|refusal| refusal.0)?;
        }
        if self.own_point().is_none() {
            return Err(format!(
                "controller.project.schedule.point {} is not one of its operating points.",
                self.point
            ));
        }
        Ok(())
    }

    /// The operating point this loop tunes, when the table holds it.
    #[must_use]
    pub fn own_point(&self) -> Option<&SchedulePoint> {
        self.points.iter().find(|point| point.name == self.point)
    }

    /// The block as the loop file writes it.
    #[must_use]
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

/// The schedule block of a `controller.project` object, when the object declares one.
///
/// # Errors
/// The `schedule` key is present and is not a schedule (see [`GainSchedule::from_value`]).
pub fn schedule_of(object: &Map<String, Value>) -> Result<Option<GainSchedule>, String> {
    match object.get(KEY) {
        None | Some(Value::Null) => Ok(None),
        Some(block) => GainSchedule::from_value(block).map(Some),
    }
}

/// The family, the schedule and the seed of a block agree (C10). The `gain-scheduled` family needs a schedule, and a schedule belongs to
/// that family. A schedule's seed is the PI law of its own point's two gain variables.
///
/// # Errors
/// The family and the schedule disagree, or the seed is not the schedule's own PI law.
pub fn check_block(
    family: &str,
    schedule: Option<&GainSchedule>,
    seed: Option<&ProjectSeedBlock>,
) -> Result<(), String> {
    match (family == FAMILY, schedule) {
        (false, None) => Ok(()),
        (true, None) => Err(format!(
            "controller.project.family {FAMILY} needs its controller.project.schedule."
        )),
        (false, Some(_)) => Err(format!(
            "controller.project.schedule belongs to the {FAMILY} family."
        )),
        (true, Some(schedule)) => match seed {
            Some(ProjectSeedBlock::ParallelPid(pid))
                if pid.family == Structure::PI && names_own_point(schedule, pid) =>
            {
                Ok(())
            }
            _ => Err(SEED_SENTENCE.to_owned()),
        },
    }
}

/// Whether a seed names the schedule's own point's two gain variables.
fn names_own_point(schedule: &GainSchedule, seed: &ParallelPidSeed) -> bool {
    schedule.own_point().is_some_and(|point| {
        seed.proportional_symbol == point.proportional_symbol
            && seed.integral_symbol.as_deref() == Some(point.integral_symbol.as_str())
    })
}

#[cfg(test)]
#[path = "gain_schedule_tests.rs"]
mod tests;
