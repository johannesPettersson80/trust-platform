//! The plants that the formula methods accept (numerical annex N5.1 to N5.7 and N6.3; specification 35, C1): a stable,
//! monotone first-order model with or without a dead time, an accepted integrating model, and a stable second-order model
//! with real poles, which SIMC reduces by its half rule (N5.2). Every other structure is refused, with the sentence the
//! method gives for it. The checks are those of `validateTuningModel` in `tuning.ts`.

#![allow(clippy::module_name_repetitions)] // `TuningPlant` names the module's subject.

use trust_control_math::identify::gain::ControllerAction;
use trust_control_math::identify::LowOrderModelKind;
use trust_control_math::tuning::{
    check_model_error, FirstOrderPlusDelay, IntegratingModel, SecondOrderPlusDelay,
};

use crate::identification::plant::IdentifiedModel;
use crate::identification::refusal::Refusal;
use crate::identification::texts::gain_sign_contradiction;
use crate::tuning::objective::TuningMethod;
use crate::tuning::structure::is_exactly_zero;
use crate::tuning::texts;

/// The normalized error above which a model is not used for tuning (N5.7, N3.2).
pub const MAXIMUM_TUNING_MODEL_ERROR: f64 = 0.25;

/// A plant in the form a formula method accepts (N5.2, N5.3). The signed gain is the static gain `K` or the rate gain
/// `K′`; the formulas run on its magnitude and the sign of N6.4 is applied to the result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TuningPlant {
    /// `K·exp(−L·s)/(T·s + 1)`: a stable, monotone first-order model, with or without a dead time (N5.2).
    FirstOrder(FirstOrderPlusDelay),
    /// `K′·exp(−L·s)/s`: an accepted integrating model (N5.3). `K′` is never used as a static gain.
    Integrating(IntegratingModel),
    /// A stable second-order model with real poles (N5.2). Only SIMC's PI reduces it, by the half rule; every other
    /// structure refuses it, as it refuses any form that is not first order.
    SecondOrder(SecondOrderPlusDelay),
}

impl TuningPlant {
    /// The signed gain that the sign rule of N6.2 checks: `K` for a first-order or second-order form, `K′` for an
    /// integrating one.
    #[must_use]
    pub fn signed_gain(&self) -> f64 {
        match self {
            Self::FirstOrder(model) => model.gain,
            Self::Integrating(model) => model.rate_gain,
            Self::SecondOrder(model) => model.gain,
        }
    }

    /// The dead time `L` in seconds (N0.4).
    #[must_use]
    pub fn delay_seconds(&self) -> f64 {
        match self {
            Self::FirstOrder(model) => model.delay_seconds,
            Self::Integrating(model) => model.delay_seconds,
            Self::SecondOrder(model) => model.delay_seconds,
        }
    }

    /// Whether the plant is an integrating one.
    #[must_use]
    pub fn is_integrating(&self) -> bool {
        matches!(self, Self::Integrating(_))
    }
}

/// The plant of an identified model for a method (`tuneController`, N5.7). The stable flag, the time constants and the
/// form are checked here, with the sentences of the TypeScript; a form the method does not take is refused with the
/// method's first-order sentence. SIMC takes a second-order form (N5.2); AMIGO does not (N5.5).
///
/// # Errors
/// Returns a [`Refusal`] for an unstable model whose form needs stability, a zero, infinite or NaN gain, a time constant
/// that is not finite and positive, or a form other than a stable first-order, second-order (SIMC) or integrating one.
pub fn from_identified(
    model: &IdentifiedModel,
    method: TuningMethod,
) -> Result<TuningPlant, Refusal> {
    let integrating = model.kind == LowOrderModelKind::Integrating;
    if !model.stable && !integrating {
        return Err(Refusal::new(texts::STABLE_MODEL_REQUIRED));
    }
    check_gain_value(model.gain)?;
    for time_constant in [
        model.time_constant_seconds,
        model.second_time_constant_seconds,
    ]
    .into_iter()
    .flatten()
    {
        if !time_constant.is_finite() || time_constant <= 0.0 {
            return Err(Refusal::new(texts::TIME_CONSTANT_NOT_POSITIVE));
        }
    }
    let delay = model.dead_time_seconds.unwrap_or(0.0);
    match model.kind {
        LowOrderModelKind::Integrating => Ok(TuningPlant::Integrating(IntegratingModel {
            rate_gain: model.gain,
            delay_seconds: delay,
        })),
        LowOrderModelKind::FirstOrder | LowOrderModelKind::FirstOrderDeadTime => {
            Ok(TuningPlant::FirstOrder(FirstOrderPlusDelay {
                gain: model.gain,
                // A first-order model without its time constant has no PI settings: NaN gives the formula's refusal.
                time_constant_seconds: model.time_constant_seconds.unwrap_or(f64::NAN),
                delay_seconds: delay,
            }))
        }
        LowOrderModelKind::SecondOrder | LowOrderModelKind::SecondOrderDeadTime => match method {
            TuningMethod::Simc => second_order_plant(model, delay),
            TuningMethod::Amigo => Err(Refusal::new(texts::needs_first_order(method))),
        },
        LowOrderModelKind::StaticGain => Err(Refusal::new(texts::needs_first_order(method))),
    }
}

/// The second-order form of an identified model for SIMC (N5.2): the denominator `τ1·τ2·s² + (τ1 + τ2)·s + 1`, with the
/// static gain `K` of the model. The time constants were checked as positive and finite by [`from_identified`].
///
/// # Errors
/// Returns [`texts::TIME_CONSTANT_NOT_POSITIVE`] when either time constant is missing.
fn second_order_plant(model: &IdentifiedModel, delay: f64) -> Result<TuningPlant, Refusal> {
    let (Some(first), Some(second)) = (
        model.time_constant_seconds,
        model.second_time_constant_seconds,
    ) else {
        return Err(Refusal::new(texts::TIME_CONSTANT_NOT_POSITIVE));
    };
    Ok(TuningPlant::SecondOrder(SecondOrderPlusDelay {
        gain: model.gain,
        leading: first * second,
        linear: first + second,
        constant: 1.0,
        delay_seconds: delay,
    }))
}

/// The normalized error check of N5.7 for an identified model (`validateTuningModel`): an error that is not finite or
/// is above 0.25 creates no candidate.
///
/// # Errors
/// Returns a [`Refusal`] naming the method when the normalized error is not finite or is above the limit.
pub fn check_normalized_error(
    model: &IdentifiedModel,
    method: TuningMethod,
) -> Result<(), Refusal> {
    check_model_error(model.normalized_error, MAXIMUM_TUNING_MODEL_ERROR)
        .map_err(|_| Refusal::new(texts::model_error_above_limit(method)))
}

/// The checks of a plant before a formula method runs (`validateTuningModel`, N5.7, N6.3): the gain is finite and not
/// zero, the time constant is finite and positive, the dead time is finite and not negative, the gain's sign agrees
/// with the controller action, and the method's own domain holds. `unit` is the gain's unit, for the sign sentence.
///
/// # Errors
/// Returns a [`Refusal`] with the sentence of the first refusal that holds.
pub fn check_plant(
    plant: &TuningPlant,
    method: TuningMethod,
    action: ControllerAction,
    unit: &str,
) -> Result<(), Refusal> {
    check_gain_value(plant.signed_gain())?;
    if let TuningPlant::FirstOrder(model) = plant {
        if !model.time_constant_seconds.is_finite() || model.time_constant_seconds <= 0.0 {
            return Err(Refusal::new(texts::TIME_CONSTANT_NOT_POSITIVE));
        }
    }
    let delay = plant.delay_seconds();
    if !delay.is_finite() || delay < 0.0 {
        return Err(Refusal::new(texts::DEAD_TIME_NEGATIVE));
    }
    if let Some(sentence) = gain_sign_contradiction(plant.signed_gain(), unit, action) {
        return Err(Refusal::new(sentence));
    }
    match (plant, method) {
        (TuningPlant::Integrating(_), TuningMethod::Amigo) => {
            Err(Refusal::new(texts::AMIGO_INTEGRATING))
        }
        (TuningPlant::Integrating(_), TuningMethod::Simc) if delay <= 0.0 => {
            Err(Refusal::new(texts::SIMC_INTEGRATING_DELAY))
        }
        (TuningPlant::FirstOrder(_), TuningMethod::Amigo) => check_amigo_domain(plant),
        (TuningPlant::SecondOrder(_), TuningMethod::Amigo) => {
            Err(Refusal::new(texts::needs_first_order(method)))
        }
        _ => Ok(()),
    }
}

/// AMIGO's domain (N5.5): a positive dead time, and `L/T ≤ 1`.
fn check_amigo_domain(plant: &TuningPlant) -> Result<(), Refusal> {
    let delay = plant.delay_seconds();
    if delay <= 0.0 {
        return Err(Refusal::new(texts::AMIGO_NEEDS_DELAY));
    }
    if let TuningPlant::FirstOrder(model) = plant {
        if delay / model.time_constant_seconds > 1.0 {
            return Err(Refusal::new(texts::AMIGO_DELAY_DOMAIN));
        }
    }
    Ok(())
}

/// A gain that is finite and not zero (N5.7, `validateModel`).
fn check_gain_value(gain: f64) -> Result<(), Refusal> {
    if !gain.is_finite() || is_exactly_zero(gain) {
        return Err(Refusal::new(texts::GAIN_NOT_FINITE_OR_ZERO));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identified(
        kind: LowOrderModelKind,
        gain: f64,
        tau: Option<f64>,
        delay: Option<f64>,
    ) -> IdentifiedModel {
        IdentifiedModel {
            kind,
            gain,
            time_constant_seconds: tau,
            second_time_constant_seconds: None,
            dead_time_seconds: delay,
            normalized_error: 0.0,
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
    fn a_first_order_model_with_delay_is_a_first_order_plant() {
        let model = identified(
            LowOrderModelKind::FirstOrderDeadTime,
            1.0,
            Some(1.0),
            Some(0.5),
        );
        let plant = from_identified(&model, TuningMethod::Simc)
            .expect("a stable first-order model is accepted");
        assert_eq!(
            plant,
            TuningPlant::FirstOrder(FirstOrderPlusDelay {
                gain: 1.0,
                time_constant_seconds: 1.0,
                delay_seconds: 0.5
            })
        );
    }

    #[test]
    fn a_second_order_model_is_refused_with_the_first_order_sentence() {
        let model = identified(LowOrderModelKind::SecondOrder, 1.0, Some(1.0), None);
        let refusal =
            from_identified(&model, TuningMethod::Amigo).expect_err("second order is outside N5.2");
        assert_eq!(
            refusal.0,
            "AMIGO PI tuning requires a stable first-order model."
        );
    }

    #[test]
    fn an_unstable_model_is_refused_unless_it_integrates() {
        let mut model = identified(LowOrderModelKind::FirstOrder, 1.0, Some(1.0), None);
        model.stable = false;
        assert_eq!(
            from_identified(&model, TuningMethod::Simc)
                .expect_err("unstable")
                .0,
            "A stable model is required for frequency results."
        );
        let integrating = IdentifiedModel {
            kind: LowOrderModelKind::Integrating,
            ..model
        };
        assert!(from_identified(&integrating, TuningMethod::Simc).is_ok());
    }

    #[test]
    fn the_model_error_limit_is_0_25_inclusive() {
        let mut model = identified(LowOrderModelKind::FirstOrder, 1.0, Some(1.0), None);
        model.normalized_error = 0.25;
        assert!(check_normalized_error(&model, TuningMethod::Simc).is_ok());
        model.normalized_error = 0.3;
        assert_eq!(
            check_normalized_error(&model, TuningMethod::Simc)
                .expect_err("above the limit")
                .0,
            "SIMC PI tuning requires accepted model error no greater than 0.25."
        );
    }

    #[test]
    fn amigo_needs_a_positive_dead_time_inside_its_ratio() {
        let no_delay = TuningPlant::FirstOrder(FirstOrderPlusDelay {
            gain: 1.0,
            time_constant_seconds: 1.0,
            delay_seconds: 0.0,
        });
        assert_eq!(
            check_plant(&no_delay, TuningMethod::Amigo, ControllerAction::Direct, "")
                .expect_err("no delay")
                .0,
            "AMIGO requires resolved positive dead time."
        );
        let ratio = TuningPlant::FirstOrder(FirstOrderPlusDelay {
            gain: 1.0,
            time_constant_seconds: 1.0,
            delay_seconds: 2.0,
        });
        assert!(
            check_plant(&ratio, TuningMethod::Amigo, ControllerAction::Direct, "")
                .expect_err("ratio above one")
                .0
                .contains("delay")
        );
    }

    #[test]
    fn a_gain_that_contradicts_the_action_is_refused_in_plain_words() {
        let falling = TuningPlant::FirstOrder(FirstOrderPlusDelay {
            gain: -1.0,
            time_constant_seconds: 1.0,
            delay_seconds: 0.0,
        });
        let refusal = check_plant(
            &falling,
            TuningMethod::Simc,
            ControllerAction::Direct,
            "°C/%",
        )
        .expect_err("sign");
        assert!(
            refusal
                .0
                .contains("measurement falls when the output rises"),
            "{}",
            refusal.0
        );
    }

    #[test]
    fn an_integrating_plant_needs_simc_and_a_positive_delay() {
        let no_delay = TuningPlant::Integrating(IntegratingModel {
            rate_gain: 0.05,
            delay_seconds: 0.0,
        });
        assert_eq!(
            check_plant(&no_delay, TuningMethod::Simc, ControllerAction::Direct, "")
                .expect_err("delay")
                .0,
            "SIMC integrating PI requires resolved positive effective delay."
        );
        let amigo = TuningPlant::Integrating(IntegratingModel {
            rate_gain: 0.05,
            delay_seconds: 1.0,
        });
        assert_eq!(
            check_plant(&amigo, TuningMethod::Amigo, ControllerAction::Direct, "")
                .expect_err("amigo")
                .0,
            "AMIGO is unavailable for integrating models."
        );
    }
}
