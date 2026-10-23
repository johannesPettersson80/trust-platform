//! The requirement record that Tune designs to (specification 35, B4.3; the Tune page specification "Control
//! requirements"): the settling time, the error and its band, the optional overshoot, the optional input disturbance, the
//! declared method and response, and the test design (`duration`, the Auto-entry tolerance, the noise) with the origin of
//! the generated values. The record is checked here, as `engineerRequirements` checks it. Each refused value names its
//! field, so that the requirements page shows the sentence under that field.

#![allow(clippy::module_name_repetitions)] // `TuningRequirements` names the module's subject.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::refusal::Refusal;
use crate::limits::MAXIMUM_EXPERIMENT_DURATION_MS;
use crate::tuning::automatic_criteria::AutomaticCriteria;
use crate::tuning::objective::{TuningMethod, TuningObjective};
use crate::tuning::output_requirements::OutputRequirements;
use crate::tuning::project_components::ProjectTuningComponent;
use crate::tuning::proportional::{GainBounds, ProportionalRequirements};
use crate::tuning::texts;

/// The largest declared overshoot limit, in percent of the setpoint change (B4.3, `MAXIMUM_OVERSHOOT_PERCENT`).
pub const MAXIMUM_OVERSHOOT_PERCENT: f64 = 1000.0;

/// The sentence of a validation duration that is not longer than the settling time (`durationSeconds`).
const DURATION_LONGER: &str = "Allow longer than every settling time.";
/// The sentence of a validation duration typed as text that no duration reads (`durationSeconds`, spec 35 B4.3).
const UNREADABLE_DURATION: &str =
    "The validation duration must be a time such as 90 s, 15 min or 1 h 30 min.";

/// The requirements that Tune designs to and validates against (B4.3). The values the engineer declared and the values
/// Automatic generated are kept apart by [`TuningRequirements::automatic_criteria`]: the generated ones are marked, frozen
/// before validation and never relaxed by its results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TuningRequirements {
    /// The setpoint change of the test, in output units. Never zero.
    pub reference_change: f64,
    /// The largest allowed error from the target after the deadline, in output units (the steady-state error limit).
    pub maximum_error: f64,
    /// The settling band around the final value, in output units. Smaller than `maximum_error`.
    pub settling_band: f64,
    /// The settling-time deadline in seconds, positive. The page sends the time the engineer typed (`90 s`, `1 h 30 min`) or a number of
    /// seconds; a time no duration reads is NaN and is refused under its field.
    #[serde(deserialize_with = "crate::identification::acquisition_settings::duration_of_setting")]
    #[schemars(schema_with = "crate::json::number_or_text_schema")]
    pub settling_time_seconds: f64,
    /// The validation duration in seconds: longer than the settling time, and within the runtime limit (A7). It is read as the settling
    /// time is.
    #[serde(deserialize_with = "crate::identification::acquisition_settings::duration_of_setting")]
    #[schemars(schema_with = "crate::json::number_or_text_schema")]
    pub duration_seconds: f64,
    /// The constant input disturbance of the load trial, in the output's unit; absent when none is declared (N5.12(b)).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constant_input_disturbance: Option<f64>,
    /// The measurement noise band of a tuned filter, in output units, zero or more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measurement_noise_band: Option<f64>,
    /// The fraction of gain uncertainty of the scenarios, between zero and one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gain_uncertainty_fraction: Option<f64>,
    /// The Auto-entry tolerance in output units, zero or more (A6.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transfer_tolerance: Option<f64>,
    /// The overshoot limit in percent of the setpoint change; absent declares no limit, and Tune never generates one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_overshoot_percent: Option<f64>,
    /// The formula method the engineer chose; absent for Automatic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tuning_method: Option<TuningMethod>,
    /// The declared response (N5.4); absent when the engineer declares none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tuning_objective: Option<TuningObjective>,
    /// The test criteria that Automatic generated, with the model they were generated from (B4.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub automatic_criteria: Option<AutomaticCriteria>,
    /// The requirements of each controlled output of a loop with several, by the output's model channel identifier (B4.3, C7). Each
    /// is checked by the rules of this record, and its refusals name the output's channel. The record's own values are the first
    /// output's copy, which the engine writes when it assembles the record from the rows typed for each output (`study::inputs_change`);
    /// absent for a loop with one controlled output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_requirements: Option<BTreeMap<String, OutputRequirements>>,
}

/// A refused requirement value (B4.3): the sentence the engineer reads and the key of the requirements field it names. The
/// requirements page shows the sentence under that field (`data-studio-requirement`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementRefusal {
    /// The key of the refused field, such as `settlingBand`; absent when the refusal names no single field.
    pub field: Option<&'static str>,
    /// The sentence that says why the value is refused.
    pub sentence: String,
    /// The channel of the output whose own requirements hold the refused value. Its field then reads `<field>:<channel>`, the page's
    /// name for that output's row; absent for the record's own values.
    pub channel: Option<String>,
}

impl RequirementRefusal {
    /// A refusal of the field `field` with `sentence`.
    fn of(field: &'static str, sentence: &str) -> Self {
        Self {
            field: Some(field),
            sentence: sentence.to_owned(),
            channel: None,
        }
    }

    /// A refusal that names no single field.
    fn general(sentence: &str) -> Self {
        Self {
            field: None,
            sentence: sentence.to_owned(),
            channel: None,
        }
    }

    /// The failure of a refused requirement: the sentence, the settings cause, the requirements to open, and the field it names.
    #[must_use]
    pub fn into_failure(self) -> Failure {
        let failure = Failure::new(
            &self.sentence,
            Cause::Settings,
            NextAction::OpenRequirements,
        );
        match (self.field, self.channel) {
            (Some(field), Some(channel)) => failure.with_field(&format!("{field}:{channel}")),
            (Some(field), None) => failure.with_field(field),
            (None, _) => failure,
        }
    }
}

impl TuningRequirements {
    /// The checks of `engineerRequirements` (see [`TuningRequirements::check`]), as the sentence of the first requirement that
    /// does not hold.
    ///
    /// # Errors
    /// Returns a [`Refusal`] with the sentence of the first requirement that does not hold.
    pub fn validate(&self) -> Result<(), Refusal> {
        self.check()
            .map_err(|refusal| Refusal::new(refusal.sentence))
    }

    /// The checks of `engineerRequirements`: finite values; then each controlled output's own record, under its channel; then the
    /// record's own values, in the requirements page's order (the setpoint change, the settling band, the error limit, the settling time,
    /// the overshoot limit and the duration), each refused value with its own sentence and field; an Auto-entry tolerance that is
    /// nonnegative; and the disturbance, noise and uncertainty bounds in range.
    ///
    /// # Errors
    /// Returns a [`RequirementRefusal`] with the sentence of the first requirement that does not hold and the field it names.
    pub fn check(&self) -> Result<(), RequirementRefusal> {
        // The two durations are checked under their own fields (`check_task_values`): a text no duration reads is NaN.
        let finite = [
            self.reference_change,
            self.maximum_error,
            self.settling_band,
        ]
        .iter()
        .all(|value| value.is_finite());
        if !finite {
            return Err(RequirementRefusal::general(texts::TASK_NOT_FINITE));
        }
        self.check_channels()?;
        self.check_task_values()?;
        if let Some(tolerance) = self.transfer_tolerance
            && (!tolerance.is_finite() || tolerance < 0.0)
        {
            return Err(RequirementRefusal::of(
                "transferTolerance",
                texts::TRANSFER_TOLERANCE,
            ));
        }
        self.check_bounds()
    }

    /// Each controlled output's own record, in channel order, checked under its channel (B4.3, C7).
    fn check_channels(&self) -> Result<(), RequirementRefusal> {
        self.channel_requirements
            .iter()
            .flatten()
            .try_for_each(|(channel, own)| own.check(Some(channel.as_str())))
    }

    /// The longest settling time the record declares: its own, or a controlled output's when that is longer (B4.3). The duration must
    /// outlast it, and the run must reach every response deadline before it ends.
    #[must_use]
    pub fn slowest_settling_time_seconds(&self) -> f64 {
        self.channel_requirements
            .iter()
            .flat_map(BTreeMap::values)
            .fold(self.settling_time_seconds, |slowest, own| {
                slowest.max(own.settling_time_seconds)
            })
    }

    /// The record that one controlled output holds to: the output's own values in place of the record's, with the shared values (the
    /// duration, the disturbance, the noise, the uncertainty and the tolerance) kept as they are, and no entries of its own.
    #[must_use]
    pub fn for_output(&self, own: &OutputRequirements) -> Self {
        Self {
            reference_change: own.reference_change,
            maximum_error: own.maximum_error,
            settling_band: own.settling_band,
            settling_time_seconds: own.settling_time_seconds,
            maximum_overshoot_percent: own.maximum_overshoot_percent,
            channel_requirements: None,
            ..self.clone()
        }
    }

    /// The record's own values, in the requirements page's order (B4.3): the output's values (see [`OutputRequirements::check`]), then
    /// the duration, which outlasts every settling time of the record and fits the runtime limit.
    fn check_task_values(&self) -> Result<(), RequirementRefusal> {
        OutputRequirements::of_record(self).check(None)?;
        if !self.duration_seconds.is_finite() {
            return Err(RequirementRefusal::of(
                "durationSeconds",
                UNREADABLE_DURATION,
            ));
        }
        if self.duration_seconds <= self.slowest_settling_time_seconds() {
            return Err(RequirementRefusal::of("durationSeconds", DURATION_LONGER));
        }
        let limit = maximum_duration_seconds();
        if self.duration_seconds > limit {
            return Err(RequirementRefusal::of(
                "durationSeconds",
                &format!("Use {limit} s or less."),
            ));
        }
        Ok(())
    }

    /// The disturbance, noise and uncertainty bounds: finite, the noise and the uncertainty not negative, and the uncertainty at
    /// most one. Each bound names its own field.
    fn check_bounds(&self) -> Result<(), RequirementRefusal> {
        if self
            .constant_input_disturbance
            .is_some_and(|value| !value.is_finite())
        {
            return Err(RequirementRefusal::of(
                "constantInputDisturbance",
                texts::BOUNDS_IN_RANGE,
            ));
        }
        if self
            .measurement_noise_band
            .is_some_and(|band| !band.is_finite() || band < 0.0)
        {
            return Err(RequirementRefusal::of(
                "measurementNoiseBand",
                texts::BOUNDS_IN_RANGE,
            ));
        }
        let uncertain = |fraction: f64| !fraction.is_finite() || !(0.0..=1.0).contains(&fraction);
        if self.gain_uncertainty_fraction.is_some_and(uncertain) {
            return Err(RequirementRefusal::of(
                "gainUncertaintyFraction",
                texts::BOUNDS_IN_RANGE,
            ));
        }
        Ok(())
    }

    /// The upper bound of the Auto-entry tolerance (B4.3, A6.4): the tolerance may not exceed the loop's output span, which the
    /// requirements page names as `Use … or less.`. A loop that declares no output span has no upper bound. Only a finite tolerance
    /// is compared; a value that is not finite is refused by [`TuningRequirements::check`].
    ///
    /// # Errors
    /// Returns a [`RequirementRefusal`] under `transferTolerance` when the tolerance is above the span.
    pub fn check_output_span(&self, output_span: Option<f64>) -> Result<(), RequirementRefusal> {
        match (self.transfer_tolerance, output_span) {
            (Some(tolerance), Some(span)) if tolerance.is_finite() && tolerance > span => Err(
                RequirementRefusal::of("transferTolerance", &format!("Use {span} or less.")),
            ),
            _ => Ok(()),
        }
    }

    /// The response of a formula method when the engineer declared none: balanced (`tuneController`'s default).
    #[must_use]
    pub fn formula_objective(&self) -> TuningObjective {
        self.tuning_objective.unwrap_or(TuningObjective::Balanced)
    }

    /// The requirements as the design of [`super::proportional`] reads them: the steady-error allowance is the error limit
    /// less the settling band, and an absent disturbance is zero.
    #[must_use]
    pub fn proportional(&self, gain_bounds: GainBounds) -> ProportionalRequirements {
        ProportionalRequirements {
            reference_change: self.reference_change,
            constant_input_disturbance: self.constant_input_disturbance.unwrap_or(0.0),
            maximum_steady_error: self.maximum_error - self.settling_band,
            settling_band: self.settling_band,
            settling_time_seconds: self.settling_time_seconds,
            gain_bounds,
        }
    }
}

/// The longest validation duration the runtime supports, in seconds (A7, `MAXIMUM_EXPERIMENT_DURATION_MS`).
#[must_use]
pub fn maximum_duration_seconds() -> f64 {
    f64::from(u32::try_from(MAXIMUM_EXPERIMENT_DURATION_MS / 1000).unwrap_or(u32::MAX))
}

/// The response deadline: the settling time rounded up to a whole task period (`responseDeadlineSeconds`).
#[must_use]
pub fn response_deadline_seconds(settling_time_seconds: f64, task_period_ms: f64) -> f64 {
    (settling_time_seconds * 1000.0 / task_period_ms).ceil() * task_period_ms / 1000.0
}

/// Whether a loop's declared components need the measurement noise band (B4.3): a low-pass or a notch filter on the measurement
/// is checked against the noise, so the requirements need the band. The requirements page shows the band's field when this holds.
#[must_use]
pub fn noise_band_required(components: &[ProjectTuningComponent]) -> bool {
    components.iter().any(|component| {
        matches!(
            component,
            ProjectTuningComponent::LowPass { .. } | ProjectTuningComponent::Notch { .. }
        )
    })
}

#[cfg(test)]
#[path = "requirements_tests.rs"]
mod tests;
