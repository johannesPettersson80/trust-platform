//! Envelope cases (specification 35, B5.3 and C0; numerical annex N8.2): the mandatory cases that the reviewed bounds of a
//! loop create. Each one is a scenario effect on the plant: a constant load on the process input, run as a regulation
//! plan whose reference change is zero so that only the disturbance acts; uniform measurement noise on the measured
//! output; and a lower and an upper gain of the process input (the bounded model variation). This is the single-channel
//! path of `engineerEnvelopeScenarios.ts`. Left out, because they belong to parked families: the reference-weighting and
//! low-pass or notch checks of project components, and the constraint-recovery case of MPC and back-calculation (V7).

use std::collections::BTreeMap;

use crate::failure::Failure;
use crate::loop_definition::LoopDefinition;
use crate::model::GeneralModel;
use crate::validation::engineer_scenario_effects::{
    check_scenario_effects, EffectGenerator, EffectMode, EffectTarget, EngineerScenarioEffect,
};
use crate::validation::engineer_verification_campaign::CampaignCase;
use crate::validation::engineer_verification_plan::{
    engineer_verification_plan, EngineerRequirements, VerificationPlan,
};

/// The envelope cases of a loop, in the `TypeScript`'s order: `load-rejection`, `measurement-noise`, `lower-gain` and
/// `upper-gain`. A case is created only for a reviewed bound that is not zero.
///
/// The loop has one model input, the manipulated variable, and one model output, the process variable (no channel
/// bindings), so every case addresses channel 0. The nominal plan is computed first, and it refuses a model that does
/// not have exactly one input and one output.
///
/// # Errors
/// The refusals of the nominal and the regulation plans, and the effect checks of `check_scenario_effects`.
pub fn engineer_envelope_scenarios(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
    values: &BTreeMap<String, f64>,
    controller_initial: &BTreeMap<String, f64>,
) -> Result<Vec<CampaignCase>, Failure> {
    let nominal =
        engineer_verification_plan(loop_definition, model, task, values, controller_initial)?;
    let mut cases = Vec::new();
    if let Some(load) = task.constant_input_disturbance.filter(|load| *load != 0.0) {
        let regulation_task = EngineerRequirements {
            reference_change: 0.0,
            ..task.clone()
        };
        let regulation = engineer_verification_plan(
            loop_definition,
            model,
            &regulation_task,
            values,
            controller_initial,
        )?;
        let effect = EngineerScenarioEffect {
            value: Some(load),
            ..starting_effect(
                "reviewed-load",
                EffectTarget::ProcessInput,
                EffectMode::Add,
                EffectGenerator::Constant,
            )
        };
        cases.push(envelope_case(
            "load-rejection",
            regulation,
            vec![effect],
            model,
        )?);
    }
    if let Some(band) = task.measurement_noise_band.filter(|band| *band != 0.0) {
        let effect = EngineerScenarioEffect {
            minimum: Some(-band),
            maximum: Some(band),
            ..starting_effect(
                "reviewed-noise",
                EffectTarget::Measurement,
                EffectMode::Add,
                EffectGenerator::UniformWhite,
            )
        };
        cases.push(envelope_case(
            "measurement-noise",
            nominal.clone(),
            vec![effect],
            model,
        )?);
    }
    if let Some(fraction) = task
        .gain_uncertainty_fraction
        .filter(|fraction| *fraction != 0.0)
    {
        for (id, direction) in [("lower-gain", -1.0), ("upper-gain", 1.0)] {
            let effect = EngineerScenarioEffect {
                value: Some(1.0 + direction * fraction),
                ..starting_effect(
                    "gain-0",
                    EffectTarget::ProcessInput,
                    EffectMode::Multiply,
                    EffectGenerator::Constant,
                )
            };
            cases.push(envelope_case(id, nominal.clone(), vec![effect], model)?);
        }
    }
    Ok(cases)
}

/// An effect that starts with the run, on channel 0 of its target, with no end and no generator values yet. The caller
/// sets the values of its generator.
fn starting_effect(
    id: &str,
    target: EffectTarget,
    mode: EffectMode,
    generator: EffectGenerator,
) -> EngineerScenarioEffect {
    EngineerScenarioEffect {
        id: id.to_owned(),
        target,
        channel: 0,
        mode,
        start_seconds: 0.0,
        end_seconds: None,
        generator,
        value: None,
        minimum: None,
        maximum: None,
        duration_seconds: None,
    }
}

/// A case of the envelope on its plan. Its effects are checked against the plan's duration before the case is kept, as
/// the `TypeScript`'s `add` does; the case evaluates the plan's requirements and has no events or overrides of its own.
fn envelope_case(
    id: &str,
    plan: VerificationPlan,
    effects: Vec<EngineerScenarioEffect>,
    model: &GeneralModel,
) -> Result<CampaignCase, Failure> {
    check_scenario_effects(&effects, model, plan.duration_ms)?;
    Ok(CampaignCase {
        id: id.to_owned(),
        requirements: plan.requirements.clone(),
        plan,
        initial_overrides: BTreeMap::new(),
        events: Vec::new(),
        effects,
    })
}

#[cfg(test)]
#[path = "engineer_envelope_scenarios_tests.rs"]
mod tests;
