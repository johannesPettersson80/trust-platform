//! The SIMC and AMIGO PI results of the numerical annex (N5.2 to N5.6, N6.4) as candidates of a parallel `Control.PID`
//! record. The formulas and their refusals are `trust_control_math::tuning`; this module chooses the response (N5.4),
//! applies the sign of the controller action (N6.4), keeps the numbers the record stores, and states what was refused.

use trust_control_math::identify::gain::{required_gain_sign, ControllerAction, GainSign};
use trust_control_math::tuning::{
    amigo_pi, closed_loop_time_constant, half_rule, self_regulating_faster_scale,
    simc_pi_integrating, simc_pi_self_regulating, PiGains, TuningRefusal,
};

use crate::experiment::number_text::js_to_precision;
use crate::identification::plant::IdentifiedModel;
use crate::identification::refusal::Refusal;
use crate::tuning::objective::{TuningMethod, TuningObjective};
use crate::tuning::plant::{check_normalized_error, check_plant, from_identified, TuningPlant};
use crate::tuning::structure::ParallelGains;
use crate::tuning::texts;

/// The significant digits that a stored PI result keeps (`tuning.ts`, `rounded`): eight.
const STORED_SIGNIFICANT_DIGITS: usize = 8;

/// A PI result of a formula method, unapplied (N5.6, N8.6): the signed proportional gain, the integral time, and the
/// closed-loop time constant `λ` for SIMC (N5.4). The numbers are the stored ones: rounded to eight significant digits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PiTuning {
    /// The method that produced the result.
    pub method: TuningMethod,
    /// The declared response that chose `λ`. AMIGO has no response-speed parameter (N5.5): its result does not change
    /// with it, and the result says so.
    pub objective: TuningObjective,
    /// The proportional gain `Kp = s_a·|Kp|` (N6.4), in the record's sign.
    pub proportional_gain: f64,
    /// The integral time `Ti` in seconds, positive.
    pub integral_time_seconds: f64,
    /// The closed-loop time constant `λ` in seconds (N5.4). Absent for AMIGO, which has none.
    pub closed_loop_time_seconds: Option<f64>,
}

impl PiTuning {
    /// The parallel gains of the result (N5.6): `Kp`, `Ki = Kp / Ti` with the sign of `Kp`, and `Kd = 0`.
    #[must_use]
    pub fn gains(&self) -> ParallelGains {
        ParallelGains {
            proportional_gain: self.proportional_gain,
            integral_gain: self.proportional_gain / self.integral_time_seconds,
            derivative_gain: 0.0,
        }
    }
}

/// The result of a SIMC or AMIGO PI for a plant (N5.2 to N5.5). The formulas run on gain magnitudes; the sign of the
/// controller action is then applied to `Kp` (N6.4). `unit` is the gain's unit, for the sign sentence.
///
/// # Errors
/// Returns a [`Refusal`] with the sentence of the first refusal that holds: the plant's checks (N5.7, N6.3), or the
/// formula's own domain.
pub fn tune_pi(
    plant: &TuningPlant,
    method: TuningMethod,
    objective: TuningObjective,
    action: ControllerAction,
    unit: &str,
) -> Result<PiTuning, Refusal> {
    check_plant(plant, method, action, unit)?;
    let choice = objective.response_choice();
    let (gains, faster_seconds) = match (plant, method) {
        (TuningPlant::FirstOrder(model), TuningMethod::Simc) => {
            let faster =
                self_regulating_faster_scale(model.time_constant_seconds, model.delay_seconds);
            (simc_pi_self_regulating(*model, choice), Some(faster))
        }
        (TuningPlant::SecondOrder(model), TuningMethod::Simc) => {
            // SIMC's half rule (N5.2) reduces the model to a first-order one, which is then tuned as above.
            let reduced = half_rule(*model).map_err(|refusal| refusal_of(refusal, method))?;
            let faster =
                self_regulating_faster_scale(reduced.time_constant_seconds, reduced.delay_seconds);
            (simc_pi_self_regulating(reduced, choice), Some(faster))
        }
        (TuningPlant::Integrating(model), TuningMethod::Simc) => (
            simc_pi_integrating(*model, choice),
            Some(model.delay_seconds),
        ),
        (TuningPlant::FirstOrder(model), TuningMethod::Amigo) => (amigo_pi(*model), None),
        (TuningPlant::Integrating(_), TuningMethod::Amigo) => {
            return Err(Refusal::new(texts::AMIGO_INTEGRATING));
        }
        (TuningPlant::SecondOrder(_), TuningMethod::Amigo) => {
            return Err(Refusal::new(texts::needs_first_order(method)));
        }
    };
    let gains = gains.map_err(|refusal| refusal_of(refusal, method))?;
    let closed_loop = faster_seconds.map(|faster| closed_loop_time_constant(faster, choice));
    Ok(stored_result(gains, method, objective, action, closed_loop))
}

/// The result of a SIMC or AMIGO PI for an identified model (`tuneController`): the model's own checks (its stability,
/// its gain and time constants, its normalized error N5.7) and then the formula.
///
/// # Errors
/// Returns a [`Refusal`] as [`from_identified`], [`check_normalized_error`] and [`tune_pi`] do.
pub fn tune_identified(
    model: &IdentifiedModel,
    method: TuningMethod,
    objective: TuningObjective,
    action: ControllerAction,
    unit: &str,
) -> Result<PiTuning, Refusal> {
    let plant = from_identified(model, method)?;
    check_normalized_error(model, method)?;
    tune_pi(&plant, method, objective, action, unit)
}

/// The sign of N6.4 as a factor: `+1` for a direct action, `−1` for a reverse one.
#[must_use]
pub fn action_sign(action: ControllerAction) -> f64 {
    match required_gain_sign(action) {
        GainSign::Positive => 1.0,
        GainSign::Negative => -1.0,
    }
}

/// Keeps the numbers the record stores: the signed `Kp`, the `Ti` and the `λ`, each rounded to eight significant digits
/// as the TypeScript stores them. The integral gain is then taken from the stored numbers (`Kp / Ti`).
fn stored_result(
    gains: PiGains,
    method: TuningMethod,
    objective: TuningObjective,
    action: ControllerAction,
    closed_loop_seconds: Option<f64>,
) -> PiTuning {
    PiTuning {
        method,
        objective,
        proportional_gain: stored(action_sign(action) * gains.proportional_gain),
        integral_time_seconds: stored(gains.integral_time_seconds),
        closed_loop_time_seconds: closed_loop_seconds.map(stored),
    }
}

/// A number as the record stores it: rounded to eight significant digits.
#[must_use]
pub fn stored(value: f64) -> f64 {
    js_to_precision(value, STORED_SIGNIFICANT_DIGITS)
}

/// The sentence of a formula refusal (N5.7). The layer-1 refusal says what failed; the sentence is the engineer's.
fn refusal_of(refusal: TuningRefusal, method: TuningMethod) -> Refusal {
    Refusal::new(match refusal {
        TuningRefusal::ZeroGain => texts::GAIN_NOT_FINITE_OR_ZERO.to_string(),
        TuningRefusal::NonfiniteParameter | TuningRefusal::NonfiniteResult => {
            texts::PI_NOT_FINITE.to_string()
        }
        TuningRefusal::NonpositiveTimeConstant => texts::TIME_CONSTANT_NOT_POSITIVE.to_string(),
        TuningRefusal::InvalidDelay => texts::DEAD_TIME_NEGATIVE.to_string(),
        TuningRefusal::ModelErrorAboveLimit => texts::model_error_above_limit(method),
        TuningRefusal::UnsupportedStructure => texts::needs_first_order(method),
        TuningRefusal::NoResponseTimeScale => texts::SIMC_INTEGRATING_DELAY.to_string(),
        TuningRefusal::AmigoNeedsPositiveDelay => texts::AMIGO_NEEDS_DELAY.to_string(),
        TuningRefusal::AmigoDelayRatioAboveOne => texts::AMIGO_DELAY_DOMAIN.to_string(),
        TuningRefusal::ComplexPoles => texts::HALF_RULE_COMPLEX_POLES.to_string(),
        TuningRefusal::NotStable => texts::HALF_RULE_NOT_STABLE.to_string(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)] // Several assertions compare stored numbers exactly, on purpose.

    use super::*;
    use trust_control_math::identify::LowOrderModelKind;
    use trust_control_math::tuning::{FirstOrderPlusDelay, IntegratingModel, SecondOrderPlusDelay};

    /// Relative agreement within `1e-7`, as the TypeScript oracle tests compare.
    fn assert_relative(actual: f64, expected: f64) {
        let error = (actual - expected).abs() / expected.abs().max(f64::EPSILON);
        assert!(error <= 1e-7, "expected {expected}, got {actual}");
    }

    fn first_order(gain: f64, time_constant: f64, delay: f64) -> TuningPlant {
        TuningPlant::FirstOrder(FirstOrderPlusDelay {
            gain,
            time_constant_seconds: time_constant,
            delay_seconds: delay,
        })
    }

    #[test]
    fn symmetric_fopdt_matches_the_published_oracles() {
        // K = 1, T = 1, L = 1, balanced: λ = 2·max(L, T/3) = 2, Kp = T/(K(λ+L)) = 1/3, Ti = min(T, 4(λ+L)) = 1.
        let plant = first_order(1.0, 1.0, 1.0);
        let simc = tune_pi(
            &plant,
            TuningMethod::Simc,
            TuningObjective::Balanced,
            ControllerAction::Direct,
            "",
        )
        .expect("SIMC");
        assert_relative(simc.proportional_gain, 1.0 / 3.0);
        assert_relative(simc.integral_time_seconds, 1.0);
        assert_relative(simc.gains().integral_gain, 1.0 / 3.0);
        // AMIGO: Kp = 0.15/K + (0.35 − LT/(L+T)²)·T/(K·L) = 0.25, Ti = 0.35L + 13LT²/(T²+12LT+7L²) = 1.
        let amigo = tune_pi(
            &plant,
            TuningMethod::Amigo,
            TuningObjective::Balanced,
            ControllerAction::Direct,
            "",
        )
        .expect("AMIGO");
        assert_relative(amigo.proportional_gain, 0.25);
        assert_relative(amigo.integral_time_seconds, 1.0);
        assert_relative(amigo.gains().integral_gain, 0.25);
        assert_eq!(amigo.closed_loop_time_seconds, None);
    }

    #[test]
    fn asymmetric_fopdt_amigo_matches_the_published_oracle() {
        let plant = first_order(2.0, 10.0, 1.0);
        let amigo = tune_pi(
            &plant,
            TuningMethod::Amigo,
            TuningObjective::Fast,
            ControllerAction::Direct,
            "",
        )
        .expect("AMIGO");
        assert_relative(amigo.proportional_gain, 1.411_776_859_5);
        assert_relative(amigo.integral_time_seconds, 6.076_872_246_7);
        assert_relative(
            amigo.gains().integral_gain,
            1.411_776_859_5 / 6.076_872_246_7,
        );
    }

    #[test]
    fn the_response_choice_moves_only_simc_and_orders_it() {
        let plant = first_order(1.0, 1.0, 0.5);
        let kp = |objective, method| {
            tune_pi(&plant, method, objective, ControllerAction::Direct, "")
                .expect("tuned")
                .proportional_gain
        };
        assert!(
            kp(TuningObjective::Fast, TuningMethod::Simc)
                > kp(TuningObjective::Balanced, TuningMethod::Simc)
        );
        assert!(
            kp(TuningObjective::Balanced, TuningMethod::Simc)
                > kp(TuningObjective::Robust, TuningMethod::Simc)
        );
        assert_eq!(
            kp(TuningObjective::Fast, TuningMethod::Amigo),
            kp(TuningObjective::Robust, TuningMethod::Amigo),
            "response labels must not rewrite the published AMIGO formula"
        );
    }

    #[test]
    fn the_closed_loop_time_constant_is_the_faster_value_times_the_choice() {
        let plant = first_order(1.0, 1.0, 0.5);
        let lambda = |objective| {
            tune_pi(
                &plant,
                TuningMethod::Simc,
                objective,
                ControllerAction::Direct,
                "",
            )
            .expect("tuned")
            .closed_loop_time_seconds
        };
        // max(L, T/3) = max(0.5, 0.333…) = 0.5 is the Faster value.
        assert_eq!(lambda(TuningObjective::Fast), Some(0.5));
        assert_eq!(lambda(TuningObjective::Balanced), Some(1.0));
        assert_eq!(lambda(TuningObjective::Robust), Some(2.0));
    }

    #[test]
    fn integrating_tank_matches_the_simc_values_and_keeps_the_rate_gain() {
        let tank = TuningPlant::Integrating(IntegratingModel {
            rate_gain: 0.05,
            delay_seconds: 1.0,
        });
        let tuned = |objective| {
            tune_pi(
                &tank,
                TuningMethod::Simc,
                objective,
                ControllerAction::Direct,
                "",
            )
            .expect("tuned")
        };
        let fast = tuned(TuningObjective::Fast);
        let balanced = tuned(TuningObjective::Balanced);
        let robust = tuned(TuningObjective::Robust);
        assert_relative(fast.proportional_gain, 10.0);
        assert_relative(fast.integral_time_seconds, 8.0);
        assert_relative(balanced.proportional_gain, 6.666_666_7);
        assert_relative(balanced.integral_time_seconds, 12.0);
        assert_relative(robust.proportional_gain, 4.0);
        assert_relative(robust.integral_time_seconds, 20.0);
        assert_eq!(
            [
                fast.closed_loop_time_seconds,
                balanced.closed_loop_time_seconds,
                robust.closed_loop_time_seconds
            ],
            [Some(1.0), Some(2.0), Some(4.0)]
        );
    }

    #[test]
    fn a_negative_gain_gets_the_sign_of_a_reverse_action() {
        let cooling = first_order(-1.0, 1.0, 0.5);
        let tuned = tune_pi(
            &cooling,
            TuningMethod::Simc,
            TuningObjective::Balanced,
            ControllerAction::Reverse,
            "",
        )
        .expect("tuned");
        assert!(
            tuned.proportional_gain < 0.0,
            "a negative plant gain needs a negative proportional gain"
        );
        assert!(tuned.integral_time_seconds > 0.0);
        assert!(tuned.gains().integral_gain < 0.0, "Ki takes the sign of Kp");
    }

    #[test]
    fn a_gain_that_contradicts_the_action_is_refused_in_plain_words() {
        let falling = first_order(-1.0, 1.0, 0.5);
        let refusal = tune_pi(
            &falling,
            TuningMethod::Simc,
            TuningObjective::Balanced,
            ControllerAction::Direct,
            "",
        )
        .expect_err("the sign contradicts the action");
        assert!(
            refusal
                .0
                .contains("measurement falls when the output rises"),
            "{}",
            refusal.0
        );
    }

    #[test]
    fn the_stored_numbers_keep_eight_significant_digits() {
        assert_eq!(stored(6.666_666_666_666_667), 6.666_666_7);
        assert_eq!(stored(0.0), 0.0);
    }

    fn second_order_identified(gain: f64, first: f64, second: f64, delay: f64) -> IdentifiedModel {
        IdentifiedModel {
            kind: LowOrderModelKind::SecondOrderDeadTime,
            gain,
            time_constant_seconds: Some(first),
            second_time_constant_seconds: Some(second),
            dead_time_seconds: Some(delay),
            normalized_error: Some(0.0),
            normalized_estimation_error: None,
            normalized_validation_error: None,
            estimation_residuals: None,
            validation_residuals: None,
            uncertainty: None,
            stable: true,
            on_search_boundary: false,
        }
    }

    #[test]
    fn simc_reduces_an_identified_second_order_model_by_the_half_rule() {
        // K = 2, τ1 = 10, τ2 = 4, θ = 1 reduces to T = 12 and L = 3 (N5.2). Balanced gives λ = 2·max(3, 4) = 8, so
        // Kp = T/(K(λ + L)) = 6/11 and Ti = min(T, 4(λ + L)) = 12.
        let model = second_order_identified(2.0, 10.0, 4.0, 1.0);
        let tuning = tune_identified(
            &model,
            TuningMethod::Simc,
            TuningObjective::Balanced,
            ControllerAction::Direct,
            "",
        )
        .expect("SIMC accepts a stable second-order model with real poles");
        assert_relative(tuning.proportional_gain, 6.0 / 11.0);
        assert_relative(tuning.integral_time_seconds, 12.0);
        assert_relative(tuning.closed_loop_time_seconds.expect("λ"), 8.0);
    }

    #[test]
    fn the_half_rule_matches_the_typescript_oracle_of_its_test_suite() {
        // control-studio-controller-law.test.ts: G(s) = 2·e^(−0.5s)/((5s + 1)(s + 1)) has τ = 5 + 1/2 = 5.5 s and
        // θ = 0.5 + 1/2 = 1 s. Balanced gives λ = 2·max(L, T/3) = 2·(5.5/3), Kp = T/(K(λ + L)) and Ti = T.
        let model = second_order_identified(2.0, 5.0, 1.0, 0.5);
        let tuning = tune_identified(
            &model,
            TuningMethod::Simc,
            TuningObjective::Balanced,
            ControllerAction::Direct,
            "",
        )
        .expect("SIMC");
        let closed_loop = 2.0 * (5.5 / 3.0);
        assert_relative(tuning.proportional_gain, 5.5 / (2.0 * (closed_loop + 1.0)));
        assert_relative(tuning.integral_time_seconds, 5.5);
    }

    #[test]
    fn amigo_still_refuses_a_second_order_model_with_the_first_order_sentence() {
        let model = second_order_identified(2.0, 10.0, 4.0, 1.0);
        let refusal = tune_identified(
            &model,
            TuningMethod::Amigo,
            TuningObjective::Balanced,
            ControllerAction::Direct,
            "",
        )
        .expect_err("AMIGO takes first-order models only (N5.5)");
        assert_eq!(
            refusal.0,
            "AMIGO PI tuning requires a stable first-order model."
        );
    }

    #[test]
    fn complex_poles_are_refused_for_simc_with_the_advice_to_choose_another_rule() {
        // s² + s + 1 has complex poles, so SIMC's half rule does not apply (N5.2, N5.7).
        let complex = TuningPlant::SecondOrder(SecondOrderPlusDelay {
            gain: 1.0,
            leading: 1.0,
            linear: 1.0,
            constant: 1.0,
            delay_seconds: 0.0,
        });
        let refusal = tune_pi(
            &complex,
            TuningMethod::Simc,
            TuningObjective::Balanced,
            ControllerAction::Direct,
            "",
        )
        .expect_err("complex poles are refused");
        assert_eq!(refusal.0, texts::HALF_RULE_COMPLEX_POLES);
        let sentence = refusal.0;
        assert!(sentence.contains("Choose Automatic or another tuning rule."));
    }
}

#[cfg(test)]
#[path = "formula_identify_tune_tests.rs"]
mod identify_tune_tests;
