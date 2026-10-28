//! The typed PI, PD and PID candidates of a formula result (`pidPiDesign.ts`; numerical annex N5.10): the PI of SIMC or
//! AMIGO, the constrained derivative alternatives that are labelled as such, and the constrained comparison around the
//! qualified SIMC candidate. A derivative term is never presented as a SIMC or AMIGO formula.

use trust_control_math::tuning::FirstOrderPlusDelay;

use crate::experiment::number_text::js_round;
use crate::identification::refusal::Refusal;
use crate::tuning::formula::{tune_pi, PiTuning};
use crate::tuning::objective::{TuningMethod, TuningObjective};
use crate::tuning::plant::TuningPlant;
use crate::tuning::structure::{is_exactly_zero, ParallelGains, Structure};
use crate::tuning::texts;
use trust_control_math::identify::gain::ControllerAction;

/// The decimal places of the derivative gain's rounding (`pidPiDesign.ts`: `Math.round(value · 1e12) / 1e12`).
const DERIVATIVE_ROUNDING: f64 = 1e12;

/// The lower and upper fractions of the time constant that bound the constrained derivative time (N5.10): `0.05·T` and
/// `0.25·T`.
const DERIVATIVE_LOWER_FRACTION: f64 = 0.05;
/// See [`DERIVATIVE_LOWER_FRACTION`].
const DERIVATIVE_UPPER_FRACTION: f64 = 0.25;

/// A typed candidate of a structure, unapplied: the structure and the gains it writes (C1, C0 "Families and zero terms").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypedDesign {
    /// The structure of the candidate.
    pub structure: Structure,
    /// The parallel gains, with the terms the structure does not use written as zero.
    pub gains: ParallelGains,
}

/// The typed PI candidate of a plant (`designTypedPiCandidate`): the formula PI of the method, with its gains.
///
/// # Errors
/// Returns the [`Refusal`] of [`tune_pi`] for the plant, the method and the action.
pub fn typed_pi(
    plant: &TuningPlant,
    method: TuningMethod,
    objective: TuningObjective,
    action: ControllerAction,
    unit: &str,
) -> Result<TypedDesign, Refusal> {
    let tuning = tune_pi(plant, method, objective, action, unit)?;
    Ok(TypedDesign {
        structure: Structure::PI,
        gains: tuning.gains(),
    })
}

/// The constrained PD or PID candidate of a formula PI (`designTypedDerivativeCandidate`, N5.10): the derivative gain is
/// `Kd = Kp·min(max(L, 0.05·T), 0.25·T)`, with the signed `Kp` of the PI. A PID keeps the PI's integral gain; a PD has
/// none. It is refused for an integrating model, which has no time constant to bound the derivative.
///
/// # Errors
/// Returns a [`Refusal`] when the plant is integrating, or when the derivative gain is not finite and nonzero.
pub fn typed_derivative(
    plant: &TuningPlant,
    tuning: &PiTuning,
    structure: Structure,
) -> Result<TypedDesign, Refusal> {
    let TuningPlant::FirstOrder(model) = plant else {
        return Err(Refusal::new(texts::DERIVATIVE_NEEDS_STABLE_FIRST_ORDER));
    };
    let derivative_gain = derivative_gain(model, tuning.proportional_gain);
    if !derivative_gain.is_finite() || is_exactly_zero(derivative_gain) {
        return Err(Refusal::new(texts::DERIVATIVE_NOT_FINITE));
    }
    let gains = tuning.gains();
    Ok(TypedDesign {
        structure,
        gains: ParallelGains {
            proportional_gain: gains.proportional_gain,
            integral_gain: if structure == Structure::PID {
                gains.integral_gain
            } else {
                0.0
            },
            derivative_gain,
        },
    })
}

/// The typed P candidate for a given gain (`designTypedProportionalCandidate`).
///
/// # Errors
/// Returns a [`Refusal`] when the gain is not finite.
pub fn typed_proportional(proportional_gain: f64) -> Result<TypedDesign, Refusal> {
    if !proportional_gain.is_finite() {
        return Err(Refusal::new(texts::TYPED_P_NEEDS_FINITE_GAIN));
    }
    Ok(TypedDesign {
        structure: Structure::P,
        gains: ParallelGains {
            proportional_gain,
            integral_gain: 0.0,
            derivative_gain: 0.0,
        },
    })
}

/// The derivative gain of the constrained alternative (N5.10): the signed `Kp` times `min(max(L, 0.05·T), 0.25·T)`,
/// rounded as the TypeScript rounds it.
#[must_use]
pub fn derivative_gain(model: &FirstOrderPlusDelay, proportional_gain: f64) -> f64 {
    let bound = (model
        .delay_seconds
        .max(DERIVATIVE_LOWER_FRACTION * model.time_constant_seconds))
    .min(DERIVATIVE_UPPER_FRACTION * model.time_constant_seconds);
    js_round(proportional_gain * bound * DERIVATIVE_ROUNDING) / DERIVATIVE_ROUNDING
}

/// The label a constrained-comparison candidate carries (N5.10): every one is `Constrained comparison`, not SIMC or
/// AMIGO, and the formula PI is the one that keeps the method's label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonLabel {
    /// The formula PI of N5.2 or N5.3, unchanged.
    FormulaPi,
    /// The less aggressive PI with `0.8·Kp` and `1.25·Ti`.
    LessAggressivePi,
    /// P: the formula PI with its integral action removed.
    P,
    /// PD: the formula PI without integral action, with the constrained derivative gain.
    Pd,
    /// PID: the formula PI with the constrained derivative gain.
    Pid,
}

/// One candidate of the constrained comparison (N5.10), unapplied, with its label.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComparisonCandidate {
    /// What the candidate is.
    pub label: ComparisonLabel,
    /// The candidate's structure.
    pub structure: Structure,
    /// The candidate's parallel gains.
    pub gains: ParallelGains,
}

/// The fixed neighbourhood of the qualified SIMC candidate (N5.10): the formula PI, a less aggressive PI with `0.8·Kp`
/// and `1.25·Ti` (so `Ki` becomes `0.64·Ki`), the P of the formula, and the PD and PID with the constrained derivative
/// gain. For an integrating model PD and PID are unavailable, and P remains eligible (N5.8). Every candidate is unapplied.
#[must_use]
pub fn constrained_comparison(plant: &TuningPlant, tuning: &PiTuning) -> Vec<ComparisonCandidate> {
    let formula = tuning.gains();
    let mut candidates = vec![
        ComparisonCandidate {
            label: ComparisonLabel::FormulaPi,
            structure: Structure::PI,
            gains: formula,
        },
        ComparisonCandidate {
            label: ComparisonLabel::LessAggressivePi,
            structure: Structure::PI,
            gains: ParallelGains {
                proportional_gain: 0.8 * formula.proportional_gain,
                integral_gain: 0.64 * formula.integral_gain,
                derivative_gain: 0.0,
            },
        },
        ComparisonCandidate {
            label: ComparisonLabel::P,
            structure: Structure::P,
            gains: ParallelGains {
                proportional_gain: formula.proportional_gain,
                integral_gain: 0.0,
                derivative_gain: 0.0,
            },
        },
    ];
    if let TuningPlant::FirstOrder(model) = plant {
        let derivative = derivative_gain(model, formula.proportional_gain);
        candidates.push(ComparisonCandidate {
            label: ComparisonLabel::Pd,
            structure: Structure::PD,
            gains: ParallelGains {
                proportional_gain: formula.proportional_gain,
                integral_gain: 0.0,
                derivative_gain: derivative,
            },
        });
        candidates.push(ComparisonCandidate {
            label: ComparisonLabel::Pid,
            structure: Structure::PID,
            gains: ParallelGains {
                derivative_gain: derivative,
                ..formula
            },
        });
    }
    candidates
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)] // The derivative and the comparison are checked against exact closed-form values.

    use super::*;
    use trust_control_math::tuning::IntegratingModel;

    fn first_order(gain: f64, time_constant: f64, delay: f64) -> TuningPlant {
        TuningPlant::FirstOrder(FirstOrderPlusDelay {
            gain,
            time_constant_seconds: time_constant,
            delay_seconds: delay,
        })
    }

    #[test]
    fn the_typed_pi_is_the_formula_pi() {
        let plant = first_order(1.0, 1.0, 1.0);
        let design = typed_pi(
            &plant,
            TuningMethod::Simc,
            TuningObjective::Balanced,
            ControllerAction::Direct,
            "",
        )
        .expect("SIMC PI");
        assert_eq!(design.structure, Structure::PI);
        assert_eq!(design.gains.derivative_gain, 0.0);
        assert!((design.gains.proportional_gain - 1.0 / 3.0).abs() < 1e-7);
        assert!((design.gains.integral_gain - 1.0 / 3.0).abs() < 1e-7);
    }

    #[test]
    fn the_constrained_derivative_uses_the_clamped_delay_fraction() {
        // T = 10, L = 1: min(max(1, 0.5), 2.5) = 1, so Kd = Kp·1.
        let model = FirstOrderPlusDelay {
            gain: 2.0,
            time_constant_seconds: 10.0,
            delay_seconds: 1.0,
        };
        assert_eq!(derivative_gain(&model, 0.5), 0.5);
        // L = 0 clamps up to 0.05·T = 0.5: Kd = Kp·0.5.
        let no_delay = FirstOrderPlusDelay {
            gain: 2.0,
            time_constant_seconds: 10.0,
            delay_seconds: 0.0,
        };
        assert_eq!(derivative_gain(&no_delay, 0.5), 0.25);
        // L = 5 clamps down to 0.25·T = 2.5: Kd = Kp·2.5.
        let long = FirstOrderPlusDelay {
            gain: 2.0,
            time_constant_seconds: 10.0,
            delay_seconds: 5.0,
        };
        assert_eq!(derivative_gain(&long, 0.5), 1.25);
    }

    #[test]
    fn a_pd_has_no_integral_gain_and_a_pid_keeps_the_formula_one() {
        let plant = first_order(1.0, 1.0, 0.5);
        let tuning = tune_pi(
            &plant,
            TuningMethod::Simc,
            TuningObjective::Balanced,
            ControllerAction::Direct,
            "",
        )
        .expect("tuned");
        let pd = typed_derivative(&plant, &tuning, Structure::PD).expect("PD");
        assert_eq!(pd.gains.integral_gain, 0.0);
        assert!(pd.gains.derivative_gain > 0.0);
        let pid = typed_derivative(&plant, &tuning, Structure::PID).expect("PID");
        assert_eq!(pid.gains.integral_gain, tuning.gains().integral_gain);
        assert_eq!(pid.gains.derivative_gain, pd.gains.derivative_gain);
    }

    #[test]
    fn an_integrating_model_has_no_constrained_derivative() {
        let tank = TuningPlant::Integrating(IntegratingModel {
            rate_gain: 0.05,
            delay_seconds: 1.0,
        });
        let tuning = tune_pi(
            &tank,
            TuningMethod::Simc,
            TuningObjective::Balanced,
            ControllerAction::Direct,
            "",
        )
        .expect("tuned");
        assert_eq!(
            typed_derivative(&tank, &tuning, Structure::PD).expect_err("no time constant").0,
            "The constrained derivative design requires a stable first-order model with a finite time constant."
        );
        let labels: Vec<ComparisonLabel> = constrained_comparison(&tank, &tuning)
            .iter()
            .map(|c| c.label)
            .collect();
        assert_eq!(
            labels,
            vec![
                ComparisonLabel::FormulaPi,
                ComparisonLabel::LessAggressivePi,
                ComparisonLabel::P
            ]
        );
    }

    #[test]
    fn the_comparison_scales_the_integral_of_the_less_aggressive_pi() {
        let plant = first_order(1.0, 1.0, 0.5);
        let tuning = tune_pi(
            &plant,
            TuningMethod::Simc,
            TuningObjective::Balanced,
            ControllerAction::Direct,
            "",
        )
        .expect("tuned");
        let candidates = constrained_comparison(&plant, &tuning);
        let formula = tuning.gains();
        let less = candidates
            .iter()
            .find(|c| c.label == ComparisonLabel::LessAggressivePi)
            .expect("present");
        assert!((less.gains.proportional_gain - 0.8 * formula.proportional_gain).abs() < 1e-12);
        assert!((less.gains.integral_gain - 0.64 * formula.integral_gain).abs() < 1e-12);
        assert_eq!(candidates.len(), 5);
    }

    #[test]
    fn a_non_finite_typed_proportional_gain_is_refused() {
        assert_eq!(
            typed_proportional(f64::NAN).expect_err("not finite").0,
            "Typed proportional design requires a finite gain."
        );
        assert_eq!(
            typed_proportional(0.75)
                .expect("finite")
                .gains
                .proportional_gain,
            0.75
        );
    }
}
