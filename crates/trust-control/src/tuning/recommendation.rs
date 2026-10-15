//! The Automatic recommendation (specification 35, B4.4 and C0; numerical annex N5.11): the simplest structure that the
//! declared model, the controller's law and the requirements establish as a candidate. P is tried first, then PI, PD and
//! PID, each designed to the requirements and each recorded as an attempt with its reason. Every choice still needs the
//! actual ST to verify it (B5.3); nothing here applies anything.

#![allow(clippy::module_name_repetitions)] // `Recommendation` names the module's subject.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::identify::gain::ControllerAction;

use crate::identification::refusal::Refusal;
use crate::tuning::design_to_requirements::{
    design_to_requirements, integrating_proportional_to_requirements, pole_matched,
};
use crate::tuning::placement::PolePlacement;
use crate::tuning::plant::TuningPlant;
use crate::tuning::proportional::{first_order_design, integrating_design, GainBounds};
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::shortfall::DesignContext;
use crate::tuning::structure::{ParallelGains, Structure};
use crate::tuning::texts;
use crate::tuning::typed_pid::{typed_proportional, TypedDesign};

/// The reason the P candidate carries when the loop has only P (`P is the simplest model-screened candidate`).
pub const P_SIMPLEST: &str =
    "P is the simplest structure the model allows. Validate it before applying.";
/// The reason the selected P carries when the comparison kept every attempt.
pub const P_AFTER_COMPARISON: &str = "P is the simplest structure that meets the requirements on the model; PI, PD and PID were compared too. Validate it before applying.";
/// The reason a feasible P attempt keeps.
pub const P_FEASIBLE: &str = "Its predicted response on the model meets the requirements.";
/// The reason a compatible PI, PD or PID attempt keeps.
pub const TYPED_RETAINED: &str = "A compatible candidate on the model.";
/// The reason PI is selected when the P design was rejected.
pub const PI_SELECTED: &str = "PI is the simplest compatible structure: P was rejected for the model, its bounds or the requirements. Validate it before applying.";
/// The reason PD is selected after P and PI were rejected.
pub const PD_SELECTED: &str = "PD is the simplest compatible structure after P and PI were rejected. Validate it before applying: Validate shows whether it meets the requirements.";
/// The reason PID is selected after P and PI were rejected.
pub const PID_SELECTED: &str = "PID is the simplest compatible structure after P and PI were rejected. Validate it before applying.";

/// Whether an attempt made a candidate or was rejected (`AutomaticControllerAttempt`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AttemptStatus {
    /// The structure has a compatible candidate on the model.
    Feasible,
    /// The structure has none, and the reason says why.
    Rejected,
}

/// One attempt of the comparison: the structure, its status and the reason in the engineer's words. The result and the study
/// keep every attempt, so the rejected candidates stay visible (Part C, "Candidates and results").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Attempt {
    /// The structure attempted.
    pub structure: Structure,
    /// Whether it made a candidate.
    pub status: AttemptStatus,
    /// The reason in plain words.
    pub reason: String,
}

/// The recommended candidate with every attempt, and the reason it was chosen (`AutomaticControllerRecommendation`).
#[derive(Debug, Clone, PartialEq)]
pub struct Recommendation {
    /// The chosen candidate, unapplied.
    pub design: TypedDesign,
    /// Every attempt, in the order they were made.
    pub attempts: Vec<Attempt>,
    /// The reason the candidate was chosen.
    pub reason: String,
}

/// Why no recommendation exists: the attempts of a comparison (`AutomaticControllerSelectionError`), or the refusal of a
/// loop that offers only P (`MainWorkflowError`).
#[derive(Debug, Clone, PartialEq)]
pub enum RecommendationError {
    /// The comparison made no candidate: every attempt with its reason.
    NoCandidate(Vec<Attempt>),
    /// A loop without a PID law refuses with its P attempt's reason.
    Refused(Refusal),
}

/// What the recommendation reads from the loop and the model besides the plant (B4.4). The caller computes these from the
/// loop definition and the model's structure.
pub struct RecommendationInputs<'a> {
    /// The refusal of the controller's topology when another adapter owns the loop (ON/OFF or multivariable), for every
    /// structure (`controllerTopology`).
    pub topology: Option<&'static str>,
    /// The refusal of a model structure that the scalar comparison does not design (`unsupportedPlantBranch`).
    pub unsupported_branch: Option<&'static str>,
    /// Whether the law has an integral term, so that PI is offered, and PID with a derivative term too (C0).
    pub has_integral_term: bool,
    /// Whether the law has a derivative term, so that PD is offered, and PID with an integral term too (C0). A law with neither
    /// term offers P only.
    pub has_derivative_term: bool,
    /// Whether the initial output is measured from the operating input (the loop declares an initialization adapter).
    pub operating_bias_applies: bool,
    /// The bounds of the proportional gain's parameter, or `None` when the loop binds none.
    pub proportional_bounds: Option<GainBounds>,
    /// The initialization check of an Auto entry (`checkInitialization`), `Ok` when none is needed.
    pub initialization: Result<(), Refusal>,
    /// The controller action (N6.1).
    pub action: ControllerAction,
    /// The design's shared context: the operating point, the limits and the predictor.
    pub context: &'a DesignContext<'a>,
    /// The requirements the candidates must meet.
    pub requirements: &'a TuningRequirements,
    /// The pole placement of the family's coefficient matching (N5.12(c)).
    pub placement: &'a dyn PolePlacement,
    /// The gain's unit, for the sign sentence (N6.3).
    pub gain_unit: &'a str,
}

impl RecommendationInputs<'_> {
    /// Whether the law has a PID term, so that a structure beyond P is offered.
    fn has_pid_term(&self) -> bool {
        self.has_integral_term || self.has_derivative_term
    }

    /// Whether the law offers `structure`, from its terms (C0).
    fn offers(&self, structure: Structure) -> bool {
        structure.offered_by_terms(self.has_integral_term, self.has_derivative_term)
    }
}

/// The recommendation of the least complex candidate (`recommendAutomaticController`).
///
/// # Errors
/// Returns [`RecommendationError::NoCandidate`] with every attempt when no structure is compatible, and
/// [`RecommendationError::Refused`] for a loop without a PID law whose P is rejected.
pub fn recommend(
    plant: &TuningPlant,
    inputs: &RecommendationInputs,
) -> Result<Recommendation, RecommendationError> {
    if let Some(topology) = inputs.topology {
        let attempts = [Structure::P, Structure::PI, Structure::PD, Structure::PID]
            .into_iter()
            .map(|structure| Attempt {
                structure,
                status: AttemptStatus::Rejected,
                reason: topology.to_string(),
            })
            .collect();
        return Err(RecommendationError::NoCandidate(attempts));
    }
    let mut attempts = Vec::new();
    let proportional = proportional_attempt(plant, inputs);
    let proportional_choice = match proportional {
        Ok(design) => {
            attempts.push(Attempt {
                structure: Structure::P,
                status: AttemptStatus::Feasible,
                reason: P_FEASIBLE.to_string(),
            });
            Some(design)
        }
        Err(refusal) => {
            attempts.push(Attempt {
                structure: Structure::P,
                status: AttemptStatus::Rejected,
                reason: refusal.0,
            });
            None
        }
    };
    if !inputs.has_pid_term() {
        return match proportional_choice {
            Some(design) => Ok(Recommendation {
                design,
                attempts,
                reason: P_SIMPLEST.to_string(),
            }),
            None => Err(RecommendationError::Refused(Refusal::new(
                attempts[0].reason.clone(),
            ))),
        };
    }
    let mut typed: Vec<(Structure, Result<TypedDesign, Refusal>)> = Vec::new();
    for structure in [Structure::PI, Structure::PD, Structure::PID]
        .into_iter()
        .filter(|structure| inputs.offers(*structure))
    {
        let outcome = typed_attempt(plant, structure, inputs);
        attempts.push(attempt_of(structure, &outcome));
        typed.push((structure, outcome));
    }
    if let Some(design) = proportional_choice {
        return Ok(Recommendation {
            design,
            attempts,
            reason: P_AFTER_COMPARISON.to_string(),
        });
    }
    for (structure, reason) in [
        (Structure::PI, PI_SELECTED),
        (Structure::PD, PD_SELECTED),
        (Structure::PID, PID_SELECTED),
    ] {
        if let Some((_, Ok(design))) = typed.iter().find(|(wanted, _)| *wanted == structure) {
            return Ok(Recommendation {
                design: *design,
                attempts,
                reason: reason.to_string(),
            });
        }
    }
    let attempts = match inputs.unsupported_branch {
        Some(branch) => attempts
            .into_iter()
            .map(|attempt| Attempt {
                reason: format!("{} {branch}", attempt.reason),
                ..attempt
            })
            .collect(),
        None => attempts,
    };
    Err(RecommendationError::NoCandidate(attempts))
}

/// The P candidate of the comparison (the P part of `recommendAutomaticController`): the Auto entry's initialization is
/// checked first; then the design of a delayed integrator with SIMC's gain for a PID loop, or the simple design of the model
/// or its pole placement; and the first output from zero model deviation is within the output limits.
fn proportional_attempt(
    plant: &TuningPlant,
    inputs: &RecommendationInputs,
) -> Result<TypedDesign, Refusal> {
    inputs.initialization.clone()?;
    let context = inputs.context;
    let design = match plant {
        TuningPlant::Integrating(model) if inputs.has_pid_term() && model.delay_seconds > 0.0 => {
            integrating_proportional_to_requirements(
                *model,
                inputs.requirements,
                context,
                inputs.gain_unit,
            )?
        }
        _ => match simple_proportional(plant, inputs) {
            Ok(design) => design,
            Err(_) => pole_matched(inputs.placement, Structure::P, inputs.requirements)?,
        },
    };
    check_initial_output(&design, inputs)?;
    if inputs.has_pid_term() {
        typed_proportional(design.gains.proportional_gain)
    } else {
        Ok(design)
    }
}

/// The simple proportional design of the model (`designSimpleProportionalController`): a delay-free integrator or a
/// delay-free first-order plant, with the gain's bounds.
fn simple_proportional(
    plant: &TuningPlant,
    inputs: &RecommendationInputs,
) -> Result<TypedDesign, Refusal> {
    let bounds = inputs
        .proportional_bounds
        .ok_or_else(|| Refusal::new(texts::NO_PROPORTIONAL_BINDING))?;
    let requirements = inputs.requirements.proportional(bounds);
    let operating_input = inputs.context.operating_input;
    let designed = match plant {
        TuningPlant::FirstOrder(model) => {
            first_order_design(*model, operating_input, &requirements)?
        }
        TuningPlant::Integrating(model) if model.delay_seconds == 0.0 => {
            integrating_design(*model, operating_input, &requirements)?
        }
        TuningPlant::Integrating(_) => return Err(Refusal::new(texts::SIMPLE_P_NO_DELAY)),
        TuningPlant::SecondOrder(_) | TuningPlant::IntegratingWithLag(_) => {
            return Err(Refusal::new(texts::SIMPLE_P_MODEL))
        }
    };
    Ok(TypedDesign {
        structure: Structure::P,
        gains: ParallelGains {
            proportional_gain: designed.proportional_gain,
            integral_gain: 0.0,
            derivative_gain: 0.0,
        },
    })
}

/// The attempt of a PI, PD or PID structure: its design to the requirements, and for a PD its steady error on the model
/// (`typedCandidate`). The initialization check comes first, and the initial output is checked as for P.
fn typed_attempt(
    plant: &TuningPlant,
    structure: Structure,
    inputs: &RecommendationInputs,
) -> Result<TypedDesign, Refusal> {
    inputs.initialization.clone()?;
    let design = design_to_requirements(
        plant,
        structure,
        inputs.requirements,
        inputs.context,
        inputs.placement,
        inputs.gain_unit,
    )?;
    if structure == Structure::PD {
        check_derivative_steady_error(plant, &design, inputs)?;
    }
    check_initial_output(&design, inputs)?;
    Ok(design)
}

/// The first output from zero model deviation, `bias + Kp·Δr`, must lie within the output limits (`assertInitialOutputWithinLimits`).
/// The bias is the operating input when the loop declares an initialization adapter, and zero otherwise.
fn check_initial_output(
    design: &TypedDesign,
    inputs: &RecommendationInputs,
) -> Result<(), Refusal> {
    let bias = if inputs.operating_bias_applies {
        inputs.context.operating_input
    } else {
        0.0
    };
    let output = bias + design.gains.proportional_gain * inputs.requirements.reference_change;
    let limits = inputs.context.output_limits;
    if output.is_finite() && output >= limits.minimum && output <= limits.maximum {
        Ok(())
    } else {
        Err(Refusal::new(texts::initial_output_outside(
            output,
            limits.minimum,
            limits.maximum,
        )))
    }
}

/// The steady error of a PD on a first-order plant (N5.9, `typedCandidate` of the TypeScript): the larger of the setpoint
/// term and the load term over the closed-loop DC denominator `1 + K·Kp`. The allowance is the error limit less the band.
/// A nonpositive denominator, a steady error that is not finite, or one above the allowance is refused.
fn check_derivative_steady_error(
    plant: &TuningPlant,
    design: &TypedDesign,
    inputs: &RecommendationInputs,
) -> Result<(), Refusal> {
    let TuningPlant::FirstOrder(model) = plant else {
        return Ok(());
    };
    let gain = model.gain;
    let operating_input = inputs.context.operating_input;
    let disturbance = inputs
        .requirements
        .constant_input_disturbance
        .unwrap_or(0.0);
    let denominator = 1.0 + gain * design.gains.proportional_gain;
    let steady_error = (inputs.requirements.reference_change + gain * operating_input)
        .abs()
        .max((gain * (operating_input - disturbance)).abs())
        / denominator;
    let allowance = inputs.requirements.maximum_error - inputs.requirements.settling_band;
    if denominator <= 0.0 || !steady_error.is_finite() || steady_error > allowance {
        return Err(Refusal::new(texts::pd_steady_error_exceeds(steady_error)));
    }
    Ok(())
}

/// The attempt that a structure's outcome becomes.
fn attempt_of(structure: Structure, outcome: &Result<TypedDesign, Refusal>) -> Attempt {
    match outcome {
        Ok(_) => Attempt {
            structure,
            status: AttemptStatus::Feasible,
            reason: TYPED_RETAINED.to_string(),
        },
        Err(refusal) => Attempt {
            structure,
            status: AttemptStatus::Rejected,
            reason: refusal.0.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuning::placement::PlacementRefusal;
    use crate::tuning::shortfall::{GainLimits, OutputLimits, ResponsePredictor, TimePoint, Trial};
    use crate::tuning::structure::ParallelGains;
    use trust_control_math::tuning::FirstOrderPlusDelay;

    /// A predictor that predicts nothing, so no candidate is judged on a response.
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

    /// A placement that gives no gains, so that only the formula and the simple designs are tried.
    struct NoForm;

    impl PolePlacement for NoForm {
        fn gains(
            &self,
            _family: Structure,
            _envelope: f64,
            _settling: f64,
        ) -> Result<ParallelGains, PlacementRefusal> {
            Err(PlacementRefusal::NoPlacementForm)
        }
    }

    fn requirements() -> TuningRequirements {
        TuningRequirements {
            reference_change: 1.0,
            maximum_error: 0.1,
            settling_band: 0.02,
            settling_time_seconds: 20.0,
            duration_seconds: 40.0,
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

    fn inputs<'a>(
        context: &'a DesignContext<'a>,
        requirements: &'a TuningRequirements,
        placement: &'a dyn PolePlacement,
        has_pid_law: bool,
        topology: Option<&'static str>,
    ) -> RecommendationInputs<'a> {
        RecommendationInputs {
            topology,
            unsupported_branch: None,
            has_integral_term: has_pid_law,
            has_derivative_term: has_pid_law,
            operating_bias_applies: false,
            proportional_bounds: Some(GainBounds {
                minimum: 0.0,
                maximum: 100.0,
            }),
            initialization: Ok(()),
            action: ControllerAction::Direct,
            context,
            requirements,
            placement,
            gain_unit: "°C/%",
        }
    }

    #[test]
    fn a_loop_with_only_p_takes_the_simple_proportional_design() {
        let predictor = NoPrediction;
        let context = DesignContext {
            action: ControllerAction::Direct,
            task_period_ms: 100.0,
            operating_input: 0.0,
            operating_output: 0.0,
            output_limits: OutputLimits {
                minimum: -100.0,
                maximum: 100.0,
            },
            gain_limits: GainLimits::default(),
            process_unit: "°C",
            manipulated_unit: "%",
            predictor: &predictor,
        };
        let requirements = requirements();
        let placement = NoForm;
        let plant = TuningPlant::FirstOrder(FirstOrderPlusDelay {
            gain: 1.5,
            time_constant_seconds: 4.0,
            delay_seconds: 0.0,
        });
        let recommendation = recommend(
            &plant,
            &inputs(&context, &requirements, &placement, false, None),
        )
        .expect("P is feasible");
        assert_eq!(recommendation.design.structure, Structure::P);
        assert!(recommendation.design.gains.proportional_gain > 0.0);
        assert_eq!(recommendation.reason, P_SIMPLEST);
        assert_eq!(recommendation.attempts.len(), 1);
        assert_eq!(recommendation.attempts[0].status, AttemptStatus::Feasible);
    }

    #[test]
    fn a_topology_owned_by_another_adapter_rejects_every_structure_with_its_reason() {
        let predictor = NoPrediction;
        let context = DesignContext {
            action: ControllerAction::Direct,
            task_period_ms: 100.0,
            operating_input: 0.0,
            operating_output: 0.0,
            output_limits: OutputLimits {
                minimum: -100.0,
                maximum: 100.0,
            },
            gain_limits: GainLimits::default(),
            process_unit: "°C",
            manipulated_unit: "%",
            predictor: &predictor,
        };
        let requirements = requirements();
        let placement = NoForm;
        let plant = TuningPlant::FirstOrder(FirstOrderPlusDelay {
            gain: 1.5,
            time_constant_seconds: 4.0,
            delay_seconds: 0.0,
        });
        let reason = texts::MULTIVARIABLE_LOOP;
        let refused = recommend(
            &plant,
            &inputs(&context, &requirements, &placement, true, Some(reason)),
        );
        let Err(RecommendationError::NoCandidate(attempts)) = refused else {
            panic!("the topology refuses every structure");
        };
        assert_eq!(attempts.len(), 4);
        assert!(attempts
            .iter()
            .all(|attempt| attempt.status == AttemptStatus::Rejected && attempt.reason == reason));
    }
}
