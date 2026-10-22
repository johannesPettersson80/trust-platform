//! The requirements of one controlled output of a loop with several (specification 35, B4.3 and C7). The requirement record keeps one
//! record for each controlled output under `channelRequirements`, keyed by the output's model channel identifier; the engine assembles
//! them from the rows that the requirements page sends for each output (`study::inputs_change`). Each record is checked by the rules of
//! the requirement record ([`super::requirements::TuningRequirements`]), and a refused value names its field with the output's channel,
//! as the page names that output's row (`settlingBand:P1.Zone2C`).

#![allow(clippy::module_name_repetitions)] // `OutputRequirements` names the module's subject.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::loop_definition::ChannelRequirements;
use crate::tuning::requirements::{
    RequirementRefusal, TuningRequirements, MAXIMUM_OVERSHOOT_PERCENT,
};
use crate::tuning::texts;

/// The sentence of a setpoint change that is zero (the requirements page's `referenceChange`).
const REFERENCE_CHANGE: &str = "Enter a nonzero setpoint change.";
/// The sentence of a settling band that is not positive (`settlingBand`).
const SETTLING_BAND: &str = "Enter a positive settling band.";
/// The sentence of an error limit that is not above the settling band (`maximumError`).
const ERROR_LIMIT: &str = "The error limit must be greater than the settling band.";
/// The sentence of a settling time that is not positive (`settlingTimeSeconds`).
const SETTLING_TIME: &str = "Enter a positive settling time.";
/// The sentence of a settling time typed as text that no duration reads (`settlingTimeSeconds`, spec 35 B4.3).
const UNREADABLE_SETTLING_TIME: &str =
    "The settling time must be a time such as 90 s, 15 min or 1 h 30 min.";
/// The sentence of an overshoot limit that is not above zero (`maximumOvershootPercent`); a blank limit declares none.
const OVERSHOOT_LIMIT: &str = "Enter an overshoot limit above 0 %, or leave it blank for no limit.";

/// The requirements that one controlled output holds to (B4.3, C7): the setpoint change of its own test, the error limit, the settling
/// band, the time by which it settles, and its overshoot limit when it declares one. The record's `channelRequirements` holds one of these
/// for each controlled output of a loop with several.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OutputRequirements {
    /// The setpoint change of the output's own test, in its unit. Never zero.
    pub reference_change: f64,
    /// The largest error allowed after the settling time, in the output's unit.
    pub maximum_error: f64,
    /// The settling band around the final value, in the output's unit. Smaller than `maximum_error`.
    pub settling_band: f64,
    /// The settling-time deadline of the output in seconds, positive. The page sends the time the engineer typed or a number of seconds.
    #[serde(deserialize_with = "crate::identification::acquisition_settings::duration_of_setting")]
    #[schemars(schema_with = "crate::json::number_or_text_schema")]
    pub settling_time_seconds: f64,
    /// The overshoot limit in percent of the setpoint change; absent declares no limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_overshoot_percent: Option<f64>,
}

impl OutputRequirements {
    /// The output's values of a record: the record's own setpoint change, error limit, band, settling time and overshoot limit. The
    /// record's values are the first output's copy in a loop with several outputs, so the same checks hold for both.
    #[must_use]
    pub fn of_record(record: &TuningRequirements) -> Self {
        Self {
            reference_change: record.reference_change,
            maximum_error: record.maximum_error,
            settling_band: record.settling_band,
            settling_time_seconds: record.settling_time_seconds,
            maximum_overshoot_percent: record.maximum_overshoot_percent,
        }
    }

    /// The requirements of a loop's own output requirements. A channel's own requirements carry no overshoot limit.
    #[must_use]
    pub fn of_channel(own: ChannelRequirements) -> Self {
        Self {
            reference_change: own.reference_change,
            maximum_error: own.maximum_error,
            settling_band: own.settling_band,
            settling_time_seconds: own.settling_time_seconds,
            maximum_overshoot_percent: None,
        }
    }

    /// The checks of one output's values, in the order of the requirements page (B4.3): the setpoint change, the settling band, the error
    /// limit, the settling time, and the overshoot limit, which is above zero and at most [`MAXIMUM_OVERSHOOT_PERCENT`]. A refused value
    /// names its field, and the output's channel when `channel` is given (the page's row of that output, such as `settlingBand:P1.Zone2C`).
    ///
    /// # Errors
    /// Returns a [`RequirementRefusal`] with the sentence of the first value that does not hold and the field it names.
    pub fn check(&self, channel: Option<&str>) -> Result<(), RequirementRefusal> {
        // A settling time typed as text that no duration reads is NaN (`duration_of_setting`): it is refused under its own field.
        if !self.settling_time_seconds.is_finite() {
            return Err(refusal(
                "settlingTimeSeconds",
                UNREADABLE_SETTLING_TIME,
                channel,
            ));
        }
        let finite = [
            self.reference_change,
            self.maximum_error,
            self.settling_band,
            self.settling_time_seconds,
        ]
        .iter()
        .all(|value| value.is_finite());
        if !finite {
            return Err(RequirementRefusal {
                field: None,
                sentence: texts::TASK_NOT_FINITE.to_owned(),
                channel: channel.map(str::to_owned),
            });
        }
        if self.reference_change == 0.0 {
            return Err(refusal("referenceChange", REFERENCE_CHANGE, channel));
        }
        if self.settling_band <= 0.0 {
            return Err(refusal("settlingBand", SETTLING_BAND, channel));
        }
        if self.maximum_error <= self.settling_band {
            return Err(refusal("maximumError", ERROR_LIMIT, channel));
        }
        if self.settling_time_seconds <= 0.0 {
            return Err(refusal("settlingTimeSeconds", SETTLING_TIME, channel));
        }
        if let Some(overshoot) = self.maximum_overshoot_percent {
            if overshoot <= 0.0 {
                return Err(refusal("maximumOvershootPercent", OVERSHOOT_LIMIT, channel));
            }
            if !overshoot.is_finite() || overshoot > MAXIMUM_OVERSHOOT_PERCENT {
                let sentence = texts::overshoot_out_of_range("Maximum overshoot");
                return Err(refusal("maximumOvershootPercent", &sentence, channel));
            }
        }
        Ok(())
    }
}

/// A refused value of one output: its field, its sentence, and the output's channel when it has one.
fn refusal(field: &'static str, sentence: &str, channel: Option<&str>) -> RequirementRefusal {
    RequirementRefusal {
        field: Some(field),
        sentence: sentence.to_owned(),
        channel: channel.map(str::to_owned),
    }
}

#[cfg(test)]
#[path = "output_requirements_tests.rs"]
mod tests;
