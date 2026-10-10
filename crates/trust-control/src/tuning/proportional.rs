//! Proportional designs (`simpleControllerDesign.ts`, `integratingProportionalDesign.ts`, `manualProportionalDesign.ts`;
//! numerical annex N5.8 and N5.9). A proportional gain is designed or evaluated for a delay-free first-order or integrating
//! plant, and its error and settling bounds are predictions, never a verification (the actual ST decides, B5.3). The
//! bounds of the gain apply to its magnitude (N6.4). The arithmetic is `trust_control_math::proportional`.

#![allow(clippy::module_name_repetitions)] // `ProportionalDesign` names the module's subject.

use trust_control_math::proportional::{
    compatible_integrating_gain, first_order_prediction, integrating_prediction,
    FirstOrderPrediction, IntegratingPrediction, IntegratingTask, PredictionInputs,
};
use trust_control_math::settling_threshold::minimum_loop_gain;
use trust_control_math::tuning::{FirstOrderPlusDelay, IntegratingModel};

use crate::identification::refusal::Refusal;
use crate::tuning::structure::is_exactly_zero;
use crate::tuning::texts;

/// The relative tolerance of the design's bound comparisons (`tolerance` of `simpleControllerDesign.ts`).
const TOLERANCE_ULPS: f64 = 64.0;
/// The most tolerance steps that move a gain to meet its prediction (`simpleControllerDesign.ts`).
const MAXIMUM_PREDICTION_STEPS: usize = 8;

/// The gain bounds of a parameter, as the parameter binding declares them (`proportionalGainBounds`). Bounds apply to the
/// magnitude of a signed gain (N6.4): the magnitudes the interval admits are [`GainBounds::magnitude_range`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GainBounds {
    /// The smallest declared value.
    pub minimum: f64,
    /// The largest declared value.
    pub maximum: f64,
}

impl GainBounds {
    /// The magnitudes that the bounds admit, `(lowest, highest)`: zero is admitted when the interval holds zero, and
    /// otherwise the smaller end magnitude; the highest is the larger end magnitude.
    #[must_use]
    pub fn magnitude_range(self) -> (f64, f64) {
        let highest = self.minimum.abs().max(self.maximum.abs());
        let lowest = if self.minimum <= 0.0 && self.maximum >= 0.0 {
            0.0
        } else {
            self.minimum.abs().min(self.maximum.abs())
        };
        (lowest, highest)
    }
}

/// The requirements that a proportional design meets (`SimpleProportionalRequirements`): the setpoint change, the constant
/// input disturbance, the steady-error allowance (the error limit less the settling band), the band and the settling
/// time, and the gain's bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProportionalRequirements {
    /// The setpoint change `ΔR`.
    pub reference_change: f64,
    /// The constant input disturbance `d` of the load trial (N5.12(b)); zero when none is declared.
    pub constant_input_disturbance: f64,
    /// The steady-error allowance in output units.
    pub maximum_steady_error: f64,
    /// The settling band `b` in output units.
    pub settling_band: f64,
    /// The settling time `Tset` in seconds.
    pub settling_time_seconds: f64,
    /// The gain's bounds.
    pub gain_bounds: GainBounds,
}

/// The prediction of a proportional design: the closed-loop pole, the error and settling bounds when the pole is
/// negative, and the reason there is no bound when it is not (N5.8, N5.9).
#[derive(Debug, Clone, PartialEq)]
pub struct ProportionalPrediction {
    /// The closed-loop pole.
    pub closed_loop_pole: f64,
    /// The steady-error bound, when the loop is stable.
    pub steady_error_bound: Option<f64>,
    /// The settling residual bound at the settling time, when the loop is stable.
    pub settling_residual_bound: Option<f64>,
    /// Why there is no asymptotic bound, when the pole is not negative.
    pub reason: Option<&'static str>,
}

/// A proportional gain with its prediction, unapplied (`SimpleProportionalDesign`, `ManualProportionalDesign`).
#[derive(Debug, Clone, PartialEq)]
pub struct ProportionalDesign {
    /// The signed gain `Kp`.
    pub proportional_gain: f64,
    /// Its prediction.
    pub prediction: ProportionalPrediction,
}

/// The requirements are finite, the setpoint change is nonzero, the limits and the settling time are positive, and the
/// gain bounds are ordered (`validateRequirements`).
///
/// # Errors
/// Returns a [`Refusal`] with the sentence of the simple design's requirements.
pub fn validate_requirements(requirements: &ProportionalRequirements) -> Result<(), Refusal> {
    let values = [
        requirements.reference_change,
        requirements.constant_input_disturbance,
        requirements.maximum_steady_error,
        requirements.settling_band,
        requirements.settling_time_seconds,
        requirements.gain_bounds.minimum,
        requirements.gain_bounds.maximum,
    ];
    let usable = values.iter().all(|value| value.is_finite())
        && !is_exactly_zero(requirements.reference_change)
        && requirements.maximum_steady_error > 0.0
        && requirements.settling_band > 0.0
        && requirements.settling_time_seconds > 0.0
        && requirements.gain_bounds.minimum <= requirements.gain_bounds.maximum;
    if usable {
        Ok(())
    } else {
        Err(Refusal::new(texts::SIMPLE_P_REQUIREMENTS))
    }
}

/// Checks a gain against its bounds by magnitude (N6.4): a finite gain whose magnitude the bounds admit.
///
/// # Errors
/// Returns [`texts::MANUAL_P_NOT_FINITE`] for a gain that is not finite, and [`texts::MANUAL_P_OUTSIDE_BOUNDS`] for a
/// magnitude outside the admitted range.
pub fn check_gain_within(gain: f64, bounds: GainBounds) -> Result<(), Refusal> {
    if !gain.is_finite() {
        return Err(Refusal::new(texts::MANUAL_P_NOT_FINITE));
    }
    let (lowest, highest) = bounds.magnitude_range();
    let magnitude = gain.abs();
    if magnitude < lowest || magnitude > highest {
        return Err(Refusal::new(texts::MANUAL_P_OUTSIDE_BOUNDS));
    }
    Ok(())
}

/// The smallest gain that a delay-free integrating plant needs (N5.8, `designIntegratingProportionalController`): the
/// compatible gain of `trust_control_math` within the bounds, with the sign of the rate gain, and the bounds of the
/// integrating prediction. A P candidate on a delay-free integrator is always stable (N5.8).
///
/// # Errors
/// Returns a [`Refusal`] for the requirements, a zero or non-finite rate gain, bounds that cannot meet the requirements,
/// or a prediction that is not finite.
pub fn integrating_design(
    model: IntegratingModel,
    operating_input: f64,
    requirements: &ProportionalRequirements,
) -> Result<ProportionalDesign, Refusal> {
    validate_requirements(requirements)?;
    let rate = model.rate_gain;
    if !rate.is_finite() || is_exactly_zero(rate) {
        return Err(Refusal::new(texts::INTEGRATING_P_RATE));
    }
    let (lowest, highest) = requirements.gain_bounds.magnitude_range();
    let task = IntegratingTask {
        rate_gain: rate,
        operating_input,
        input_disturbance: requirements.constant_input_disturbance,
        reference_change: requirements.reference_change,
        maximum_steady_error: requirements.maximum_steady_error,
        settling_band: requirements.settling_band,
        settling_time_seconds: requirements.settling_time_seconds,
        minimum_gain: lowest,
        maximum_gain: highest,
    };
    let magnitude =
        compatible_integrating_gain(task).map_err(|_| Refusal::new(texts::INTEGRATING_P_BOUNDS))?;
    let proportional_gain = rate.signum() * magnitude;
    let prediction = integrating_prediction(prediction_inputs(
        rate,
        proportional_gain,
        operating_input,
        requirements,
    ))
    .map_err(|_| Refusal::new(texts::INTEGRATING_P_UNIDENTIFIABLE))?;
    Ok(ProportionalDesign {
        proportional_gain,
        prediction: from_integrating(&prediction),
    })
}

/// The prediction of an engineer-entered gain on a delay-free integrating plant (N5.8,
/// `evaluateManualIntegratingProportionalController`). The gain is kept whatever its value, and a zero or nonnegative pole
/// has no asymptotic bound.
///
/// # Errors
/// Returns a [`Refusal`] for the requirements, a gain outside its bounds, or a prediction with numerical uncertainty.
pub fn integrating_manual(
    model: IntegratingModel,
    proportional_gain: f64,
    operating_input: f64,
    requirements: &ProportionalRequirements,
) -> Result<ProportionalDesign, Refusal> {
    validate_requirements(requirements)?;
    check_gain_within(proportional_gain, requirements.gain_bounds)?;
    let prediction = integrating_prediction(prediction_inputs(
        model.rate_gain,
        proportional_gain,
        operating_input,
        requirements,
    ))
    .map_err(|_| Refusal::new(texts::MANUAL_INTEGRATING_UNCERTAIN))?;
    Ok(ProportionalDesign {
        proportional_gain,
        prediction: from_integrating(&prediction),
    })
}

/// The gain of a delay-free first-order plant that meets the steady-error and settling allowances (N5.9,
/// `designSimpleProportionalController`). The product `q = K·Kp` is the least value that meets the steady-error bound and
/// the settling bound, raised to the gain's lower bound; the prediction then confirms it within a few tolerance steps.
///
/// The settling threshold is N5.9a, `trust_control_math::settling_threshold::minimum_loop_gain`; the steady-error threshold
/// is N5.9. `trust_control_math::proportional::first_order_prediction` gives the reported bounds and the checks.
///
/// # Errors
/// Returns [`texts::SIMPLE_P_NO_DELAY`] for a dead time, a [`Refusal`] for the model or the requirements, and the bound
/// sentences when no gain within the bounds meets the requirements.
pub fn first_order_design(
    model: FirstOrderPlusDelay,
    operating_input: f64,
    requirements: &ProportionalRequirements,
) -> Result<ProportionalDesign, Refusal> {
    validate_requirements(requirements)?;
    let gain = model.gain;
    let time_constant = model.time_constant_seconds;
    if !is_exactly_zero(model.delay_seconds) {
        return Err(Refusal::new(texts::SIMPLE_P_NO_DELAY));
    }
    if !gain.is_finite()
        || is_exactly_zero(gain)
        || !time_constant.is_finite()
        || time_constant <= 0.0
    {
        return Err(Refusal::new(texts::SIMPLE_P_MODEL));
    }
    let worst_final_error = (requirements.reference_change + gain * operating_input)
        .abs()
        .max((gain * requirements.constant_input_disturbance).abs());
    let steady_threshold = worst_final_error / requirements.maximum_steady_error - 1.0;
    let settling_threshold = minimum_loop_gain(
        time_constant,
        requirements.reference_change,
        gain * operating_input,
        requirements.settling_band,
        requirements.settling_time_seconds,
    )
    .map_err(|_| Refusal::new(texts::SIMPLE_P_BOUNDS))?;
    let required_q = 0.0_f64.max(steady_threshold).max(settling_threshold);
    let (lowest, highest) = requirements.gain_bounds.magnitude_range();
    let lower_q = gain.abs() * lowest;
    let upper_q = gain.abs() * highest;
    let mut q = required_q.max(lower_q);
    if q > upper_q + tolerance(q, upper_q) {
        return Err(Refusal::new(texts::SIMPLE_P_BOUNDS));
    }
    let mut steps = 0;
    while steps < MAXIMUM_PREDICTION_STEPS
        && !meets(gain, time_constant, q, operating_input, requirements)
    {
        q += tolerance(q, required_q);
        steps += 1;
    }
    if q > upper_q + tolerance(q, upper_q)
        || !meets(gain, time_constant, q, operating_input, requirements)
    {
        return Err(Refusal::new(texts::SIMPLE_P_PREDICTION));
    }
    let proportional_gain = q / gain;
    let magnitude = proportional_gain.abs();
    if !proportional_gain.is_finite()
        || magnitude < lowest - tolerance(magnitude, lowest)
        || magnitude > highest + tolerance(magnitude, highest)
    {
        return Err(Refusal::new(texts::SIMPLE_P_OUTSIDE_BOUNDS));
    }
    let prediction = first_order_prediction(
        prediction_inputs(gain, proportional_gain, operating_input, requirements),
        time_constant,
    )
    .map_err(|_| Refusal::new(texts::SIMPLE_P_PREDICTION))?;
    Ok(ProportionalDesign {
        proportional_gain,
        prediction: from_first_order(&prediction),
    })
}

/// The prediction of an engineer-entered gain on a delay-free first-order plant (N5.9,
/// `designManualProportionalCandidate`). A nonnegative pole has no asymptotic bound, and the result says so.
///
/// # Errors
/// Returns a [`Refusal`] for the requirements, a gain outside its bounds, a dead time, or a prediction with numerical
/// uncertainty.
pub fn first_order_manual(
    model: FirstOrderPlusDelay,
    proportional_gain: f64,
    operating_input: f64,
    requirements: &ProportionalRequirements,
) -> Result<ProportionalDesign, Refusal> {
    validate_requirements(requirements)?;
    check_gain_within(proportional_gain, requirements.gain_bounds)?;
    if !is_exactly_zero(model.delay_seconds) {
        return Err(Refusal::new(texts::SIMPLE_P_NO_DELAY));
    }
    let prediction = first_order_prediction(
        prediction_inputs(model.gain, proportional_gain, operating_input, requirements),
        model.time_constant_seconds,
    )
    .map_err(|_| Refusal::new(texts::MANUAL_P_UNCERTAIN))?;
    Ok(ProportionalDesign {
        proportional_gain,
        prediction: from_first_order(&prediction),
    })
}

/// Whether a gain with `q = K·Kp` meets both allowances on the first-order prediction (`meetsPredictions`): a negative
/// pole, a steady error within its allowance, and a settling residual within the band.
fn meets(
    plant_gain: f64,
    time_constant: f64,
    q: f64,
    operating_input: f64,
    requirements: &ProportionalRequirements,
) -> bool {
    let proportional_gain = q / plant_gain;
    let Ok(prediction) = first_order_prediction(
        prediction_inputs(plant_gain, proportional_gain, operating_input, requirements),
        time_constant,
    ) else {
        return false;
    };
    let steady_ok = prediction.steady_error_bound.is_some_and(|bound| {
        bound
            <= requirements.maximum_steady_error
                + tolerance(bound, requirements.maximum_steady_error)
    });
    let settling_ok = prediction.settling_residual_bound.is_some_and(|bound| {
        bound <= requirements.settling_band + tolerance(bound, requirements.settling_band)
    });
    prediction.closed_loop_pole < 0.0 && steady_ok && settling_ok
}

/// The tolerance of a bound comparison: 64 ε times the larger of one and the magnitudes compared.
fn tolerance(first: f64, second: f64) -> f64 {
    TOLERANCE_ULPS * f64::EPSILON * 1.0_f64.max(first.abs()).max(second.abs())
}

/// The prediction inputs of a proportional gain on a plant with the given gain: the rate gain `k` of an integrator (N5.8)
/// or the static gain `K` of a first-order plant (N5.9).
fn prediction_inputs(
    plant_gain: f64,
    proportional_gain: f64,
    operating_input: f64,
    requirements: &ProportionalRequirements,
) -> PredictionInputs {
    PredictionInputs {
        plant_gain,
        proportional_gain,
        operating_input,
        input_disturbance: requirements.constant_input_disturbance,
        reference_change: requirements.reference_change,
        settling_time_seconds: requirements.settling_time_seconds,
    }
}

/// The integrating prediction as the design reports it.
fn from_integrating(prediction: &IntegratingPrediction) -> ProportionalPrediction {
    ProportionalPrediction {
        closed_loop_pole: prediction.closed_loop_pole,
        steady_error_bound: prediction.steady_error_bound,
        settling_residual_bound: prediction.settling_residual_bound,
        reason: prediction
            .steady_error_bound
            .is_none()
            .then_some(texts::MANUAL_INTEGRATING_NO_BOUND),
    }
}

/// The first-order prediction as the design reports it.
fn from_first_order(prediction: &FirstOrderPrediction) -> ProportionalPrediction {
    ProportionalPrediction {
        closed_loop_pole: prediction.closed_loop_pole,
        steady_error_bound: prediction.steady_error_bound,
        settling_residual_bound: prediction.settling_residual_bound,
        reason: prediction
            .steady_error_bound
            .is_none()
            .then_some(texts::MANUAL_P_NO_BOUND),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)] // The bounds of the closed forms are compared exactly where the rule gives exact values.

    use super::*;

    fn requirements(bounds: GainBounds) -> ProportionalRequirements {
        ProportionalRequirements {
            reference_change: 1.0,
            constant_input_disturbance: 0.0,
            maximum_steady_error: 0.1,
            settling_band: 0.02,
            settling_time_seconds: 20.0,
            gain_bounds: bounds,
        }
    }

    fn wide() -> GainBounds {
        GainBounds {
            minimum: 0.0,
            maximum: 100.0,
        }
    }

    #[test]
    fn the_magnitude_range_of_signed_bounds_follows_the_interval() {
        assert_eq!(
            GainBounds {
                minimum: 0.0,
                maximum: 100.0
            }
            .magnitude_range(),
            (0.0, 100.0)
        );
        assert_eq!(
            GainBounds {
                minimum: -100.0,
                maximum: 100.0
            }
            .magnitude_range(),
            (0.0, 100.0)
        );
        assert_eq!(
            GainBounds {
                minimum: -100.0,
                maximum: -10.0
            }
            .magnitude_range(),
            (10.0, 100.0)
        );
        assert_eq!(
            GainBounds {
                minimum: 2.0,
                maximum: 5.0
            }
            .magnitude_range(),
            (2.0, 5.0)
        );
    }

    #[test]
    fn a_manual_gain_is_checked_by_magnitude() {
        let reverse = GainBounds {
            minimum: -100.0,
            maximum: -10.0,
        };
        assert!(check_gain_within(-20.0, reverse).is_ok());
        assert_eq!(
            check_gain_within(5.0, reverse)
                .expect_err("below the magnitude range")
                .0,
            "Manual proportional gain is outside the declared bounds."
        );
        assert_eq!(
            check_gain_within(f64::NAN, wide())
                .expect_err("not finite")
                .0,
            "Manual proportional gain must be finite."
        );
    }

    #[test]
    fn an_integrating_plant_gets_the_compatible_gain_with_the_rate_sign() {
        let model = IntegratingModel {
            rate_gain: 0.5,
            delay_seconds: 0.0,
        };
        let design = integrating_design(model, 0.0, &requirements(wide()))
            .expect("a compatible gain exists");
        assert!(
            design.proportional_gain > 0.0,
            "the rate gain is positive, so Kp is"
        );
        let prediction = &design.prediction;
        assert!(prediction.closed_loop_pole < 0.0);
        let settling = prediction
            .settling_residual_bound
            .expect("stable loop has a bound");
        assert!(
            settling <= 0.02 + 1e-9,
            "the settling residual {settling} meets the band"
        );
        let steady = prediction
            .steady_error_bound
            .expect("stable loop has a bound");
        assert!(
            steady <= 0.1 + 1e-12,
            "the steady error {steady} meets its allowance"
        );
    }

    #[test]
    fn an_integrating_plant_with_a_reverse_rate_gets_a_negative_gain() {
        let model = IntegratingModel {
            rate_gain: -0.5,
            delay_seconds: 0.0,
        };
        let reverse = GainBounds {
            minimum: -100.0,
            maximum: -0.0,
        };
        let design = integrating_design(model, 0.0, &requirements(reverse)).expect("compatible");
        assert!(design.proportional_gain < 0.0);
    }

    #[test]
    fn a_nonpositive_pole_has_no_bound_in_a_manual_integrating_evaluation() {
        let model = IntegratingModel {
            rate_gain: -0.5,
            delay_seconds: 0.0,
        };
        let design = integrating_manual(model, 2.0, 0.0, &requirements(wide())).expect("gain kept");
        assert_eq!(design.prediction.steady_error_bound, None);
        assert_eq!(
            design.prediction.reason,
            Some(texts::MANUAL_INTEGRATING_NO_BOUND)
        );
    }

    #[test]
    fn a_first_order_plant_with_delay_has_no_simple_proportional_design() {
        let model = FirstOrderPlusDelay {
            gain: 1.5,
            time_constant_seconds: 4.0,
            delay_seconds: 0.5,
        };
        assert_eq!(
            first_order_design(model, 0.0, &requirements(wide()))
                .expect_err("delay")
                .0,
            "Simple P design has no transport-delay adapter."
        );
    }

    #[test]
    fn a_first_order_design_meets_its_bounds_and_reports_them() {
        let model = FirstOrderPlusDelay {
            gain: 1.5,
            time_constant_seconds: 4.0,
            delay_seconds: 0.0,
        };
        let design =
            first_order_design(model, 0.0, &requirements(wide())).expect("a design exists");
        assert!(design.proportional_gain > 0.0);
        let steady = design.prediction.steady_error_bound.expect("stable");
        let settling = design.prediction.settling_residual_bound.expect("stable");
        assert!(steady <= 0.1 + 1e-9);
        assert!(settling <= 0.02 + 1e-9);
        assert!(design.prediction.closed_loop_pole < 0.0);
    }

    #[test]
    fn a_first_order_manual_gain_with_a_nonnegative_pole_is_kept_without_a_bound() {
        let model = FirstOrderPlusDelay {
            gain: -1.5,
            time_constant_seconds: 4.0,
            delay_seconds: 0.0,
        };
        let design = first_order_manual(
            model,
            1.0,
            0.0,
            &requirements(GainBounds {
                minimum: -100.0,
                maximum: 100.0,
            }),
        )
        .expect("the gain is kept");
        assert_eq!(design.proportional_gain, 1.0);
        assert_eq!(design.prediction.steady_error_bound, None);
        assert_eq!(design.prediction.reason, Some(texts::MANUAL_P_NO_BOUND));
    }

    #[test]
    fn a_zero_rate_gain_is_refused_for_the_integrating_design() {
        let model = IntegratingModel {
            rate_gain: 0.0,
            delay_seconds: 0.0,
        };
        assert_eq!(
            integrating_design(model, 0.0, &requirements(wide()))
                .expect_err("zero rate")
                .0,
            "Integrating P design requires a finite nonzero rate gain."
        );
    }

    /// The requirements of the manual proportional test (`control-studio-manual-proportional-design.test.ts`): a unit
    /// change, a 0.1 error allowance, a 0.02 band, a 2 s settling time, and gain bounds of ±2.
    fn manual_requirements() -> ProportionalRequirements {
        ProportionalRequirements {
            reference_change: 1.0,
            constant_input_disturbance: 0.0,
            maximum_steady_error: 0.1,
            settling_band: 0.02,
            settling_time_seconds: 2.0,
            gain_bounds: GainBounds {
                minimum: -2.0,
                maximum: 2.0,
            },
        }
    }

    #[test]
    fn a_pinned_integrating_gain_keeps_its_bounds_and_its_stable_prediction() {
        // G = 2/s with an operating input 0.4: pole −4, steady error 0.4/2 = 0.2, settling 1.2·e^(−8).
        let plant = IntegratingModel {
            rate_gain: 2.0,
            delay_seconds: 0.0,
        };
        let stable = integrating_manual(plant, 2.0, 0.4, &manual_requirements()).expect("in range");
        assert_eq!(stable.proportional_gain, 2.0);
        assert_eq!(stable.prediction.closed_loop_pole, -4.0);
        assert_eq!(stable.prediction.steady_error_bound, Some(0.2));
        let settling = stable.prediction.settling_residual_bound.expect("stable");
        assert!((settling - 1.2 * (-8.0_f64).exp()).abs() < 1e-12);
        for gain in [0.0, -1.0] {
            let kept = integrating_manual(plant, gain, 0.4, &manual_requirements()).expect("kept");
            assert_eq!(kept.proportional_gain, gain);
            assert_eq!(
                kept.prediction.closed_loop_pole,
                if gain == 0.0 { 0.0 } else { 2.0 }
            );
            assert_eq!(kept.prediction.steady_error_bound, None);
            assert_eq!(kept.prediction.settling_residual_bound, None);
        }
        assert!(
            integrating_manual(plant, 3.0, 0.4, &manual_requirements()).is_err(),
            "3 is outside ±2"
        );
    }

    #[test]
    fn an_in_range_manual_first_order_gain_keeps_its_bounds_when_the_pole_is_negative() {
        // K = 1, τ = 1, Kp = 0.5: q = 0.5, pole −1.5, steady error 1/1.5, settling e^(−3).
        let model = FirstOrderPlusDelay {
            gain: 1.0,
            time_constant_seconds: 1.0,
            delay_seconds: 0.0,
        };
        let design = first_order_manual(model, 0.5, 0.0, &manual_requirements()).expect("kept");
        assert_eq!(design.proportional_gain, 0.5);
        assert_eq!(design.prediction.closed_loop_pole, -1.5);
        assert_eq!(design.prediction.steady_error_bound, Some(2.0 / 3.0));
        assert_eq!(
            design.prediction.settling_residual_bound,
            Some((-3.0_f64).exp())
        );
        assert_eq!(design.prediction.reason, None);
    }

    #[test]
    fn an_unstable_manual_first_order_gain_is_kept_without_an_invented_bound() {
        let model = FirstOrderPlusDelay {
            gain: 1.0,
            time_constant_seconds: 1.0,
            delay_seconds: 0.0,
        };
        let design = first_order_manual(model, -1.0, 0.0, &manual_requirements()).expect("kept");
        assert_eq!(design.prediction.closed_loop_pole, 0.0);
        assert_eq!(design.prediction.steady_error_bound, None);
        assert_eq!(design.prediction.settling_residual_bound, None);
        assert_eq!(design.prediction.reason, Some(texts::MANUAL_P_NO_BOUND));
    }

    #[test]
    fn a_negative_q_uses_the_amplified_reference_transient_bound() {
        // K = 1, τ = 1, Kp = −0.75: q = −0.75, pole −0.25, steady error 1/0.25 = 4, transient 3·e^(−0.5).
        let model = FirstOrderPlusDelay {
            gain: 1.0,
            time_constant_seconds: 1.0,
            delay_seconds: 0.0,
        };
        let design = first_order_manual(model, -0.75, 0.0, &manual_requirements()).expect("kept");
        assert_eq!(design.prediction.closed_loop_pole, -0.25);
        assert_eq!(design.prediction.steady_error_bound, Some(4.0));
        assert_eq!(
            design.prediction.settling_residual_bound,
            Some(3.0 * (-0.5_f64).exp())
        );
    }

    #[test]
    fn a_manual_gain_is_refused_when_out_of_range_or_not_finite() {
        let model = FirstOrderPlusDelay {
            gain: 1.0,
            time_constant_seconds: 1.0,
            delay_seconds: 0.0,
        };
        assert_eq!(
            first_order_manual(model, 2.1, 0.0, &manual_requirements())
                .expect_err("out of range")
                .0,
            "Manual proportional gain is outside the declared bounds."
        );
        assert_eq!(
            first_order_manual(model, f64::NAN, 0.0, &manual_requirements())
                .expect_err("not finite")
                .0,
            "Manual proportional gain must be finite."
        );
    }

    #[test]
    fn numerical_overflow_and_underflow_are_uncertainty_not_instability() {
        let overflowing = FirstOrderPlusDelay {
            gain: f64::MAX,
            time_constant_seconds: 1.0,
            delay_seconds: 0.0,
        };
        assert_eq!(
            first_order_manual(overflowing, 2.0, 0.0, &manual_requirements())
                .expect_err("overflow")
                .0,
            "Manual proportional prediction has numerical uncertainty."
        );
        let underflowing = FirstOrderPlusDelay {
            gain: 1.0,
            time_constant_seconds: f64::MAX,
            delay_seconds: 0.0,
        };
        assert_eq!(
            first_order_manual(
                underflowing,
                -0.999_999_999_999_999_8,
                0.0,
                &manual_requirements()
            )
            .expect_err("underflow")
            .0,
            "Manual proportional prediction has numerical uncertainty."
        );
    }

    #[test]
    fn the_requirements_must_be_ordered_and_finite() {
        let mut bad = requirements(wide());
        bad.reference_change = 0.0;
        assert_eq!(
            validate_requirements(&bad).expect_err("zero change").0,
            "Simple P design requires finite nonzero reference, positive error/settling limits, and ordered gain bounds."
        );
    }
}

#[cfg(test)]
#[path = "proportional_ported_tests.rs"]
mod ported_tests;
