//! The requirement record that Tune designs to (specification 35, B4.3; the Tune page specification "Control
//! requirements"): the settling time, the error and its band, the optional overshoot, the optional input disturbance, the
//! declared method and response, and the test design (`duration`, the Auto-entry tolerance, the noise) with the origin of
//! the generated values. The record is checked here, as `engineerRequirements` checks it.

#![allow(clippy::module_name_repetitions)] // `TuningRequirements` names the module's subject.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::identification::refusal::Refusal;
use crate::limits::MAXIMUM_EXPERIMENT_DURATION_MS;
use crate::tuning::automatic_criteria::AutomaticCriteria;
use crate::tuning::objective::{TuningMethod, TuningObjective};
use crate::tuning::proportional::{GainBounds, ProportionalRequirements};
use crate::tuning::texts;

/// The largest declared overshoot limit, in percent of the setpoint change (B4.3, `MAXIMUM_OVERSHOOT_PERCENT`).
pub const MAXIMUM_OVERSHOOT_PERCENT: f64 = 1000.0;

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
    /// The settling-time deadline in seconds, positive.
    pub settling_time_seconds: f64,
    /// The validation duration in seconds: longer than the settling time, and within the runtime limit (A7).
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
}

impl TuningRequirements {
    /// The checks of `engineerRequirements`: finite values, a nonzero change, the settling band below the error limit,
    /// a positive settling time, a duration longer than the settling time and within the runtime limit, an Auto-entry
    /// tolerance that is nonnegative, disturbance and noise bounds in range, and an overshoot limit above zero and at most
    /// [`MAXIMUM_OVERSHOOT_PERCENT`].
    ///
    /// # Errors
    /// Returns a [`Refusal`] with the sentence of the first requirement that does not hold.
    pub fn validate(&self) -> Result<(), Refusal> {
        let finite = [
            self.reference_change,
            self.maximum_error,
            self.settling_band,
            self.settling_time_seconds,
            self.duration_seconds,
        ]
        .iter()
        .all(|value| value.is_finite());
        if !finite {
            return Err(Refusal::new(texts::TASK_NOT_FINITE));
        }
        let duration_limit = maximum_duration_seconds();
        let ordered = self.reference_change != 0.0
            && self.settling_band > 0.0
            && self.maximum_error > self.settling_band
            && self.settling_time_seconds > 0.0
            && self.duration_seconds > self.settling_time_seconds
            && self.duration_seconds <= duration_limit;
        if !ordered {
            return Err(Refusal::new(texts::TASK_ORDER));
        }
        if let Some(tolerance) = self.transfer_tolerance {
            if !tolerance.is_finite() || tolerance < 0.0 {
                return Err(Refusal::new(texts::TRANSFER_TOLERANCE));
            }
        }
        self.validate_bounds()?;
        if let Some(overshoot) = self.maximum_overshoot_percent {
            if !overshoot.is_finite() || overshoot <= 0.0 || overshoot > MAXIMUM_OVERSHOOT_PERCENT {
                return Err(Refusal::new(texts::overshoot_out_of_range(
                    "Maximum overshoot",
                )));
            }
        }
        Ok(())
    }

    /// The disturbance, noise and uncertainty bounds: finite, the noise and the uncertainty not negative, and the
    /// uncertainty at most one.
    fn validate_bounds(&self) -> Result<(), Refusal> {
        let disturbance_ok = self.constant_input_disturbance.is_none_or(f64::is_finite);
        let noise_ok = self
            .measurement_noise_band
            .is_none_or(|band| band.is_finite() && band >= 0.0);
        let uncertainty_ok = self
            .gain_uncertainty_fraction
            .is_none_or(|fraction| fraction.is_finite() && (0.0..=1.0).contains(&fraction));
        if disturbance_ok && noise_ok && uncertainty_ok {
            Ok(())
        } else {
            Err(Refusal::new(texts::BOUNDS_IN_RANGE))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> TuningRequirements {
        TuningRequirements {
            reference_change: 2.0,
            maximum_error: 0.4,
            settling_band: 0.1,
            settling_time_seconds: 45.0,
            duration_seconds: 90.0,
            constant_input_disturbance: None,
            measurement_noise_band: None,
            gain_uncertainty_fraction: None,
            transfer_tolerance: None,
            maximum_overshoot_percent: None,
            tuning_method: None,
            tuning_objective: None,
            automatic_criteria: None,
        }
    }

    #[test]
    fn a_complete_record_is_accepted() {
        assert!(valid().validate().is_ok());
    }

    #[test]
    fn the_settling_band_must_be_below_the_error_limit() {
        let mut bad = valid();
        bad.settling_band = 0.4;
        assert_eq!(
            bad.validate().expect_err("band not below error").0,
            "Use a nonzero target change, positive response time, a smaller settling allowance than the error limit, and a longer test duration."
        );
    }

    #[test]
    fn the_duration_must_outlast_the_settling_time_and_fit_the_runtime() {
        let mut short = valid();
        short.duration_seconds = 45.0;
        assert!(short.validate().is_err());
        let mut long = valid();
        long.duration_seconds = maximum_duration_seconds() + 1.0;
        assert!(long.validate().is_err());
    }

    #[test]
    fn an_overshoot_limit_must_be_positive_and_at_most_a_thousand_percent() {
        let mut ok = valid();
        ok.maximum_overshoot_percent = Some(1000.0);
        assert!(ok.validate().is_ok());
        ok.maximum_overshoot_percent = Some(1000.5);
        assert!(ok.validate().is_err());
        ok.maximum_overshoot_percent = Some(0.0);
        assert!(ok.validate().is_err());
    }

    #[test]
    fn the_uncertainty_fraction_is_between_zero_and_one() {
        let mut bad = valid();
        bad.gain_uncertainty_fraction = Some(1.5);
        assert_eq!(
            bad.validate().expect_err("out of range").0,
            "Enter finite disturbance/noise bounds and gain uncertainty between zero and one."
        );
    }

    #[test]
    fn the_steady_error_allowance_is_the_error_limit_less_the_band() {
        let bounds = GainBounds {
            minimum: 0.0,
            maximum: 100.0,
        };
        let proportional = valid().proportional(bounds);
        assert!((proportional.maximum_steady_error - 0.3).abs() < 1e-12);
        assert_eq!(proportional.constant_input_disturbance, 0.0);
    }

    #[test]
    fn the_response_deadline_rounds_up_to_a_task_period() {
        // 45 s at a 100 ms task is already whole periods; 45.05 s rounds up to 45.1 s.
        assert_eq!(response_deadline_seconds(45.0, 100.0), 45.0);
        assert!((response_deadline_seconds(45.05, 100.0) - 45.1).abs() < 1e-9);
    }
}
