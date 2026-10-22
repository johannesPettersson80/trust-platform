//! The design of a structure to the requirements (specification 35, B4.4 and C1; numerical annex N5.11 and N5.12): Automatic
//! takes the response nearest the declared one whose predicted response meets the requirements, places the poles when no
//! formula response does, and says what the structure misses when nothing does. The proportional gain of a delayed
//! integrator is designed the same way (N5.8).

use trust_control_math::identify::gain::ControllerAction;
use trust_control_math::tuning::{integrating_proportional_gain, IntegratingModel};

use crate::identification::refusal::Refusal;
use crate::identification::texts::gain_sign_contradiction;
use crate::tuning::formula::{action_sign, tune_pi};
use crate::tuning::objective::{TuningMethod, TuningObjective, AUTOMATIC_OBJECTIVE};
use crate::tuning::placement::{
    envelopes, first_envelope, in_magnitude_frame, within_sample_rate, PlacementRefusal,
    PolePlacement,
};
use crate::tuning::plant::TuningPlant;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::shortfall::{predicted_outcome, DesignContext, Outcome};
use crate::tuning::structure::Structure;
use crate::tuning::texts;
use crate::tuning::typed_pid::{typed_derivative, typed_proportional, TypedDesign};

/// The responses to try, from the declared one outward: of two equally near responses the more robust one first (N5.12(a),
/// `NEAREST` of `automaticPidResponse.ts`).
#[must_use]
pub fn nearest_responses(declared: TuningObjective) -> [TuningObjective; 3] {
    match declared {
        TuningObjective::Robust => [
            TuningObjective::Robust,
            TuningObjective::Balanced,
            TuningObjective::Fast,
        ],
        TuningObjective::Balanced => [
            TuningObjective::Balanced,
            TuningObjective::Robust,
            TuningObjective::Fast,
        ],
        TuningObjective::Fast => [
            TuningObjective::Fast,
            TuningObjective::Balanced,
            TuningObjective::Robust,
        ],
    }
}

/// The design of a structure to the requirements (`designPidToRequirements`): a declared formula method keeps its formula
/// unchanged, and Automatic tries the responses nearest the declared one and then the pole placement (N5.12).
///
/// # Errors
/// Returns a [`Refusal`] naming the structure, the reason and the fastest miss when no response or placement meets the
/// requirements, or the refusal of the formula itself.
pub fn design_to_requirements(
    plant: &TuningPlant,
    structure: Structure,
    requirements: &TuningRequirements,
    context: &DesignContext,
    placement: &dyn PolePlacement,
    unit: &str,
) -> Result<TypedDesign, Refusal> {
    if requirements.tuning_method.is_some() {
        return design_for_task(
            plant,
            structure,
            requirements,
            requirements.formula_objective(),
            context,
            placement,
            unit,
        );
    }
    let declared = requirements.tuning_objective.unwrap_or(AUTOMATIC_OBJECTIVE);
    let mut shortfalls: Vec<(TuningObjective, String)> = Vec::new();
    for (index, response) in nearest_responses(declared).into_iter().enumerate() {
        let candidate = design_for_task(
            plant,
            structure,
            requirements,
            response,
            context,
            placement,
            unit,
        )?;
        match predicted_outcome(&candidate, requirements, context) {
            Outcome::Meets => return Ok(candidate),
            Outcome::Unpredicted if index == 0 => return Ok(candidate),
            Outcome::Unpredicted => {}
            Outcome::Misses(sentence) => shortfalls.push((response, sentence)),
        }
    }
    let placed = place_poles(structure, requirements, context, placement);
    if let Some(design) = placed.design {
        return Ok(design);
    }
    let fastest = placed
        .shortfall
        .map(|sentence| format!("even its fastest response {sentence}"))
        .or_else(|| {
            shortfall_of(&shortfalls, TuningObjective::Fast)
                .map(|sentence| format!("even its fastest response {sentence}"))
        })
        .unwrap_or_else(|| {
            let other = shortfall_of(&shortfalls, TuningObjective::Balanced)
                .or_else(|| shortfall_of(&shortfalls, TuningObjective::Robust))
                .unwrap_or_default();
            format!("its predicted response {other}")
        });
    Err(Refusal::new(texts::family_cannot_meet(
        structure_word(structure),
        &fastest,
    )))
}

/// The design of the structure from a formula for one response (`designPidForTask`): the SIMC or the declared method's PI,
/// with its constrained derivative for a PD or PID. The plant's sign is checked before any fallback (N6.3). Without a
/// declared method, a plant without delay falls back to the pole-matched design of the default envelope.
///
/// # Errors
/// Returns the formula's refusal, with the choice-of-method sentence when the engineer declared a method.
pub fn design_for_task(
    plant: &TuningPlant,
    structure: Structure,
    requirements: &TuningRequirements,
    objective: TuningObjective,
    context: &DesignContext,
    placement: &dyn PolePlacement,
    unit: &str,
) -> Result<TypedDesign, Refusal> {
    if let Some(sentence) = gain_sign_contradiction(plant.signed_gain(), unit, context.action) {
        return Err(Refusal::new(sentence));
    }
    let method = requirements.tuning_method.unwrap_or(TuningMethod::Simc);
    match formula_design(plant, structure, method, objective, context.action, unit) {
        Ok(design) => Ok(design),
        Err(error) => {
            if requirements.tuning_method.is_some() {
                return Err(Refusal::new(format!(
                    "{} Choose Automatic or another tuning rule.",
                    error.0
                )));
            }
            if plant.delay_seconds() > 0.0 {
                return Err(error);
            }
            pole_matched(placement, structure, requirements).map_err(|_| error)
        }
    }
}

/// The formula's design of a structure (`designTypedPiCandidate` and `designTypedDerivativeCandidate`). A P is the formula
/// PI with its integral action removed (N5.10, and Part C's P with the SIMC proportional gain).
fn formula_design(
    plant: &TuningPlant,
    structure: Structure,
    method: TuningMethod,
    objective: TuningObjective,
    action: ControllerAction,
    unit: &str,
) -> Result<TypedDesign, Refusal> {
    let tuning = tune_pi(plant, method, objective, action, unit)?;
    match structure {
        Structure::P => typed_proportional(tuning.proportional_gain),
        Structure::PI => Ok(TypedDesign {
            structure: Structure::PI,
            gains: tuning.gains(),
        }),
        Structure::PD | Structure::PID => typed_derivative(plant, &tuning, structure),
    }
}

/// The pole-matched design of a structure at the first envelope of the requirements, `max(4, ln(|Δr|/b))` (N5.12(c), B4.4),
/// through the placement.
///
/// # Errors
/// Returns a [`Refusal`] that the placement needs a model when it has no form for the structure, or the refusal the
/// placement gives for the requirements.
pub fn pole_matched(
    placement: &dyn PolePlacement,
    structure: Structure,
    requirements: &TuningRequirements,
) -> Result<TypedDesign, Refusal> {
    let envelope = first_envelope(requirements.reference_change, requirements.settling_band);
    match placement.gains(structure, envelope, requirements.settling_time_seconds) {
        Ok(gains) => Ok(TypedDesign { structure, gains }),
        Err(PlacementRefusal::NoPlacementForm) => Err(Refusal::new(texts::PLACEMENT_NEEDS_MODEL)),
        Err(PlacementRefusal::Refused(refusal)) => Err(refusal),
    }
}

/// The outcome of the pole placement: the first placement that meets the requirements, or the fastest miss.
struct Placed {
    design: Option<TypedDesign>,
    shortfall: Option<String>,
}

/// The pole placement sequence (N5.12(c)): the first envelope, then each 25 % faster while the sample-rate limit holds, the
/// gains non-negative in the magnitude frame, and the first placement whose prediction meets the requirements is used.
fn place_poles(
    structure: Structure,
    requirements: &TuningRequirements,
    context: &DesignContext,
    placement: &dyn PolePlacement,
) -> Placed {
    let mut shortfall = None;
    let action_factor = action_sign(context.action);
    for envelope in envelopes(first_envelope(
        requirements.reference_change,
        requirements.settling_band,
    )) {
        if !within_sample_rate(
            envelope,
            requirements.settling_time_seconds,
            context.task_period_ms,
        ) {
            break;
        }
        let gains = match placement.gains(structure, envelope, requirements.settling_time_seconds) {
            Ok(gains) => gains,
            Err(PlacementRefusal::NoPlacementForm) => break,
            Err(PlacementRefusal::Refused(_)) => continue,
        };
        if !in_magnitude_frame(gains, action_factor) {
            continue;
        }
        let candidate = TypedDesign { structure, gains };
        match predicted_outcome(&candidate, requirements, context) {
            Outcome::Meets => {
                return Placed {
                    design: Some(candidate),
                    shortfall: None,
                }
            }
            Outcome::Misses(sentence) => shortfall = Some(sentence),
            Outcome::Unpredicted => {}
        }
    }
    Placed {
        design: None,
        shortfall,
    }
}

/// The proportional design of a delayed integrator (`designIntegratingPToRequirements`, N5.8): SIMC's proportional gain
/// `Kp = s_a / (|K′|(λ + L))` with `λ` the response's multiple of the dead time, kept when its prediction meets the requirements.
///
/// # Errors
/// Returns a [`Refusal`] for a rate gain whose sign contradicts the action (N6.3), a formula refusal, or when no response
/// meets the requirements, with the fastest miss.
pub fn integrating_proportional_to_requirements(
    model: IntegratingModel,
    requirements: &TuningRequirements,
    context: &DesignContext,
    unit: &str,
) -> Result<TypedDesign, Refusal> {
    if let Some(sentence) = gain_sign_contradiction(model.rate_gain, unit, context.action) {
        return Err(Refusal::new(sentence));
    }
    let declared = requirements.tuning_objective.unwrap_or(AUTOMATIC_OBJECTIVE);
    let mut shortfalls: Vec<(TuningObjective, String)> = Vec::new();
    for (index, response) in nearest_responses(declared).into_iter().enumerate() {
        let magnitude = integrating_proportional_gain(model, response.response_choice())
            .map_err(|_| Refusal::new(texts::SIMC_INTEGRATING_DELAY))?;
        let candidate = typed_proportional(action_sign(context.action) * magnitude)?;
        match predicted_outcome(&candidate, requirements, context) {
            Outcome::Meets => return Ok(candidate),
            Outcome::Unpredicted if index == 0 => return Ok(candidate),
            Outcome::Unpredicted => {}
            Outcome::Misses(sentence) => shortfalls.push((response, sentence)),
        }
    }
    let reason = match shortfall_of(&shortfalls, TuningObjective::Fast) {
        Some(sentence) => format!("even its fastest response {sentence}"),
        None => format!(
            "its predicted response {}",
            shortfall_of(&shortfalls, TuningObjective::Balanced)
                .or_else(|| shortfall_of(&shortfalls, TuningObjective::Robust))
                .unwrap_or_default()
        ),
    };
    Err(Refusal::new(texts::family_cannot_meet("P", &reason)))
}

/// The miss recorded for a response, if any.
fn shortfall_of(
    shortfalls: &[(TuningObjective, String)],
    response: TuningObjective,
) -> Option<String> {
    shortfalls
        .iter()
        .find(|(recorded, _)| *recorded == response)
        .map(|(_, sentence)| sentence.clone())
}

/// The name of a structure as the sentences print it.
#[must_use]
pub fn structure_word(structure: Structure) -> &'static str {
    match structure {
        Structure::P => "P",
        Structure::PI => "PI",
        Structure::PD => "PD",
        Structure::PID => "PID",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuning::shortfall::{GainLimits, OutputLimits, ResponsePredictor, TimePoint, Trial};
    use crate::tuning::structure::ParallelGains;

    #[test]
    fn the_responses_are_tried_from_the_declared_one_outward() {
        assert_eq!(
            nearest_responses(TuningObjective::Balanced),
            [
                TuningObjective::Balanced,
                TuningObjective::Robust,
                TuningObjective::Fast
            ]
        );
        assert_eq!(
            nearest_responses(TuningObjective::Fast),
            [
                TuningObjective::Fast,
                TuningObjective::Balanced,
                TuningObjective::Robust
            ]
        );
        assert_eq!(
            nearest_responses(AUTOMATIC_OBJECTIVE)[0],
            TuningObjective::Robust
        );
    }

    #[test]
    fn the_structure_words_are_the_engineers_words() {
        assert_eq!(structure_word(Structure::PID), "PID");
        assert_eq!(structure_word(Structure::P), "P");
    }

    /// A placement that gives the same gains for every envelope, for the fallback of the default design.
    struct FixedPlacement(ParallelGains);

    impl PolePlacement for FixedPlacement {
        fn gains(
            &self,
            _family: Structure,
            _envelope: f64,
            _settling: f64,
        ) -> Result<ParallelGains, PlacementRefusal> {
            Ok(self.0)
        }
    }

    /// A predictor that predicts nothing, so every candidate is unpredicted.
    struct NoPrediction;

    impl ResponsePredictor for NoPrediction {
        fn predict(
            &self,
            _design: &TypedDesign,
            _trial: Trial,
            _requirements: &TuningRequirements,
        ) -> Option<Vec<TimePoint>> {
            None
        }
    }

    fn requirements(method: Option<TuningMethod>) -> TuningRequirements {
        TuningRequirements {
            channel_requirements: None,
            reference_change: 1.0,
            maximum_error: 0.1,
            settling_band: 0.02,
            settling_time_seconds: 2.0,
            duration_seconds: 4.0,
            constant_input_disturbance: None,
            measurement_noise_band: None,
            gain_uncertainty_fraction: None,
            transfer_tolerance: None,
            maximum_overshoot_percent: None,
            tuning_method: method,
            tuning_objective: None,
            automatic_criteria: None,
        }
    }

    /// The integrator of the explicit-method test (`control-studio-explicit-tuning-method.test.ts`): `G = 1/s` with no delay.
    fn integrator() -> TuningPlant {
        TuningPlant::Integrating(IntegratingModel {
            rate_gain: 0.05,
            delay_seconds: 0.0,
        })
    }

    #[test]
    fn an_explicit_formula_keeps_its_refusal_and_is_not_replaced_by_a_placement() {
        let predictor = NoPrediction;
        let context = DesignContext {
            action: ControllerAction::Direct,
            task_period_ms: 100.0,
            operating_input: 0.0,
            operating_output: 0.0,
            output_limits: OutputLimits {
                minimum: -20.0,
                maximum: 20.0,
            },
            gain_limits: GainLimits::default(),
            process_unit: "level",
            manipulated_unit: "valve",
            predictor: &predictor,
        };
        let placement = FixedPlacement(ParallelGains {
            proportional_gain: 1.0,
            integral_gain: 0.1,
            derivative_gain: 0.0,
        });
        let simc = design_to_requirements(
            &integrator(),
            Structure::PI,
            &requirements(Some(TuningMethod::Simc)),
            &context,
            &placement,
            "level/valve/s",
        );
        let refusal = simc.expect_err("SIMC needs a positive effective delay");
        assert!(
            refusal.0.contains("positive effective delay"),
            "{}",
            refusal.0
        );
        assert!(
            refusal
                .0
                .ends_with("Choose Automatic or another tuning rule."),
            "{}",
            refusal.0
        );
        let amigo = design_to_requirements(
            &integrator(),
            Structure::PI,
            &requirements(Some(TuningMethod::Amigo)),
            &context,
            &placement,
            "level/valve/s",
        );
        assert!(amigo
            .expect_err("AMIGO has no integrating form")
            .0
            .contains("AMIGO"));
    }

    #[test]
    fn an_undeclared_method_falls_back_to_the_placement_for_a_plant_without_delay() {
        let predictor = NoPrediction;
        let context = DesignContext {
            action: ControllerAction::Direct,
            task_period_ms: 100.0,
            operating_input: 0.0,
            operating_output: 0.0,
            output_limits: OutputLimits {
                minimum: -20.0,
                maximum: 20.0,
            },
            gain_limits: GainLimits::default(),
            process_unit: "level",
            manipulated_unit: "valve",
            predictor: &predictor,
        };
        let gains = ParallelGains {
            proportional_gain: 1.0,
            integral_gain: 0.1,
            derivative_gain: 0.0,
        };
        let design = design_to_requirements(
            &integrator(),
            Structure::PI,
            &requirements(None),
            &context,
            &FixedPlacement(gains),
            "level/valve/s",
        )
        .expect("the placement designs a plant without delay");
        assert_eq!(design.structure, Structure::PI);
        assert_eq!(design.gains, gains);
    }

    /// A P asked of a declared formula is the formula PI without its integral action (N5.10), never the PI itself.
    #[test]
    fn a_formula_p_is_the_formula_pi_without_its_integral_action() {
        let predictor = NoPrediction;
        let context = DesignContext {
            action: ControllerAction::Direct,
            task_period_ms: 100.0,
            operating_input: 0.0,
            operating_output: 0.0,
            output_limits: OutputLimits {
                minimum: -20.0,
                maximum: 20.0,
            },
            gain_limits: GainLimits::default(),
            process_unit: "°C",
            manipulated_unit: "%",
            predictor: &predictor,
        };
        let plant = TuningPlant::FirstOrder(trust_control_math::tuning::FirstOrderPlusDelay {
            gain: 2.0,
            time_constant_seconds: 10.0,
            delay_seconds: 1.0,
        });
        let requirements = requirements(Some(TuningMethod::Simc));
        let placement = FixedPlacement(ParallelGains {
            proportional_gain: 9.0,
            integral_gain: 9.0,
            derivative_gain: 0.0,
        });
        let design = design_to_requirements(
            &plant,
            Structure::P,
            &requirements,
            &context,
            &placement,
            "°C/%",
        )
        .expect("SIMC designs a delayed first-order plant");
        let pi = tune_pi(
            &plant,
            TuningMethod::Simc,
            requirements.formula_objective(),
            ControllerAction::Direct,
            "°C/%",
        )
        .expect("the formula PI");
        assert_eq!(design.structure, Structure::P);
        assert_eq!(
            design.gains,
            ParallelGains {
                proportional_gain: pi.proportional_gain,
                integral_gain: 0.0,
                derivative_gain: 0.0,
            }
        );
    }

    #[test]
    fn a_missing_shortfall_is_none() {
        let shortfalls = vec![(TuningObjective::Fast, "is up to 1 °C".to_string())];
        assert_eq!(
            shortfall_of(&shortfalls, TuningObjective::Fast).as_deref(),
            Some("is up to 1 °C")
        );
        assert_eq!(shortfall_of(&shortfalls, TuningObjective::Robust), None);
    }
}

#[cfg(test)]
#[path = "design_to_requirements_ported_tests.rs"]
mod ported_tests;
