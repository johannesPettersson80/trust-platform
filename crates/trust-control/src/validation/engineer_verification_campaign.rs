//! The mandatory verification campaign of a loop (specification 35, B5.3, C0 and C8; numerical annex N8.1 and N8.5): the
//! `target-response` case of the nominal plan, the envelope cases of the reviewed bounds, and the loop's declared scenarios. The whole
//! campaign is checked before any native job starts (`engineerVerificationCampaign.ts`).
//! A PID group adds one case for each controlled output, which excites that output's setpoint alone (`independentReferenceScenarios`,
//! `group_case`). Left out, because they belong to parked families: `measuredDisturbanceScenarios` and `engineerControllerScenarios`
//! (V7), and the ON/OFF filter of the target case (V7.3).

use std::collections::{BTreeMap, BTreeSet};

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::connected_record::{
    RunConditions, RunEvidence, ValidationChannel, ValidationRun,
};
use crate::identification::independent_validation::{RequirementStatus, RunEvaluation};
use crate::json::decode_value;
use crate::loop_definition::LoopDefinition;
use crate::model::{decode_general_model, GeneralModel};
use crate::study::inputs::held_scenarios;
use crate::validation::engineer_envelope_scenarios::engineer_envelope_scenarios;
use crate::validation::engineer_scenario_effects::{
    check_scenario_effects, milliseconds_as_float, requirements_failure, EngineerScenarioEffect,
};
use crate::validation::engineer_verification_plan::{
    engineer_verification_plan, EngineerRequirements, VerificationPlan,
};
use crate::validation::gain_schedule_case;
use crate::validation::group_case;
use crate::validation::isolated_run_sampling::recorded_sample_times;
use crate::validation::model_route::{model_route, ModelRoute};
use crate::validation::on_off_case;
use crate::validation::run_validation::{check_requirement, RunCheck, RunRequirement};
use crate::validation::scenario_contract::{
    latest_scenario_check, valid_engineer_scenarios, EngineerScenario, ScenarioEvent,
    ScenarioResult,
};
use crate::validation::state_case;
use crate::validation::texts;

/// The most requirements one run accepts, a declared scenario's together with the plan's constraints.
const MAXIMUM_REQUIREMENTS: usize = 64;

/// One case of the campaign (`CampaignCase`): a plan with its own overrides, events, effects and requirements.
#[derive(Debug)]
pub struct CampaignCase {
    /// The case's identity: `target-response`, an envelope case's identity, or a declared scenario's id.
    pub id: String,
    /// The plan the case runs on: the nominal plan, or the plan of the scenario's own model (C8). Its `requirements` are the plan's
    /// own, before a declared scenario adds its requirements; `requirements` below is the list to judge.
    pub plan: VerificationPlan,
    /// The scenario's initial values, applied over the plan's initial values.
    pub initial_overrides: BTreeMap<String, f64>,
    /// The scenario's events, each on a task sample inside the run.
    pub events: Vec<ScenarioEvent>,
    /// The plant effects of the case.
    pub effects: Vec<EngineerScenarioEffect>,
    /// The requirements the case is judged on. A declared scenario keeps the plan's constraints, without its settling
    /// and error checks, and adds its own requirements.
    pub requirements: Vec<RunRequirement>,
}

/// A declared scenario's own model and the plant it runs on (C8 Validate, B5.3): the model, and its route, which holds the model's inline
/// plant and the controller's initial values at the model's own operating point.
#[derive(Debug, Clone, PartialEq)]
pub struct SuppliedPlant {
    /// The model the scenario supplies.
    pub model: GeneralModel,
    /// The model's route: its inline plant, and the controller's initial values at its operating point (`model_route`).
    pub route: ModelRoute,
}

/// The mandatory campaign of a loop (`engineerVerificationCampaign`): the `target-response` case, a PID group's per-output cases (C7),
/// the envelope cases, then the declared scenarios in their order. Every case is checked before any native job starts.
///
/// Every case runs on `model`, the study's model, except a declared scenario that supplies its own model: that case is planned on its
/// model, from the model's own operating point and initial values (C8 Validate, `supplied_plant`), as the `TypeScript` plans it.
///
/// A declared scenario's refusals are `Settings` failures whose next action opens the requirements, because the
/// scenarios are edited there. A supplied model that does not decode is a `ModelOrData` failure with the same next action.
///
/// # Errors
/// The refusals of the nominal plan and of the envelope; the declared scenarios that are not valid
/// (`scenarios_invalid`); a declared id that repeats an automatic case (`scenario_duplicates_automatic`); a declared
/// check that ends after the run, which names the duration it needs (`scenario_check_after_run`); a supplied model
/// that does not decode (`scenario_model_invalid`), is missing for an operating range (`scenario_operating_model_missing`), or
/// whose route refuses; a declared scenario that writes a frozen parameter (`scenario_overwrites_parameters`); an event off the task
/// grid or after the run (`scenario_events_off_grid`); repeated or more than 64 requirements (`scenario_requirements_invalid`); the
/// effects' own checks; and a requirement the run cannot hold (the check of `check_requirement`).
pub fn engineer_verification_campaign(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
    values: &BTreeMap<String, f64>,
    controller_initial: &BTreeMap<String, f64>,
) -> Result<Vec<CampaignCase>, Failure> {
    let nominal =
        engineer_verification_plan(loop_definition, model, task, values, controller_initial)?;
    let scenarios = declared_scenarios(loop_definition)?;
    let envelope =
        engineer_envelope_scenarios(loop_definition, model, task, values, controller_initial)?;
    // The cases that excite each controlled output of a PID group alone (C7); none for any other loop.
    let excited = group_case::independent_reference_cases(
        loop_definition,
        model,
        task,
        values,
        controller_initial,
    )?;
    // The state-recovery case of a state-feedback law (C4), which runs from the perturbed plant state (`state_case`).
    let recovery = state_case::recovery_case(loop_definition, model, &nominal)?;
    let mut reserved: BTreeSet<&str> = envelope.iter().map(|case| case.id.as_str()).collect();
    reserved.extend(excited.iter().map(|case| case.id.as_str()));
    reserved.insert("target-response");
    if recovery.is_some() {
        reserved.insert(state_case::STATE_RECOVERY_ID);
    }
    if scenarios
        .iter()
        .any(|scenario| reserved.contains(scenario.id.as_str()))
    {
        return Err(requirements_failure(&texts::scenario_duplicates_automatic()));
    }
    // A check after the end of the run cannot be evaluated: the refusal names the validation duration it needs.
    let run_seconds = milliseconds_as_float(nominal.duration_ms) / 1000.0;
    if let Some(latest) =
        latest_scenario_check(&scenarios).filter(|latest| latest.end_seconds > run_seconds + 1e-9)
    {
        return Err(requirements_failure(&texts::scenario_check_after_run(
            &latest.scenario,
            &latest.requirement,
            latest.end_seconds,
            run_seconds,
        )));
    }
    let mut declared = Vec::with_capacity(scenarios.len());
    for scenario in scenarios {
        declared.push(declared_case(
            loop_definition,
            model,
            task,
            values,
            &nominal,
            scenario,
        )?);
    }
    let target = CampaignCase {
        id: "target-response".to_owned(),
        requirements: if loop_definition.controller.on_off.is_some() {
            on_off_case::without_dwell(&nominal.requirements)
        } else {
            nominal.requirements.clone()
        },
        plan: nominal,
        initial_overrides: BTreeMap::new(),
        events: Vec::new(),
        effects: Vec::new(),
    };
    let mut cases = vec![target];
    cases.extend(excited);
    cases.extend(envelope);
    cases.extend(recovery);
    cases.extend(declared);
    if loop_definition.controller.on_off.is_some() {
        cases.push(on_off_case::switching_cycle_case(
            loop_definition,
            model,
            task,
            values,
            controller_initial,
        )?);
    }
    // A schedule's runs record every scan when they fit the isolated limits (C10 Validate): its continuity is judged on recorded samples.
    for case in &mut cases {
        if gain_schedule_case::records_every_sample(
            loop_definition,
            case.plan.step_ms,
            case.plan.duration_ms,
            case.plan.observed_symbols.len(),
        ) {
            case.plan.record_every = 1;
        }
    }
    Ok(cases)
}

/// The plants that the cases of a campaign run on when they keep their own: each declared scenario that supplies its own model (C8), and
/// the state-recovery case of a state-feedback law (C4), by case identity. A case without an entry runs on the selected model's inline
/// plant or the configured plant (B5.3).
///
/// # Errors
/// As [`supplied_plant`], and as `state_case::recovery_plant`.
pub fn supplied_plants(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    values: &BTreeMap<String, f64>,
) -> Result<BTreeMap<String, SuppliedPlant>, Failure> {
    let mut supplied = BTreeMap::new();
    for scenario in held_scenarios(loop_definition, None) {
        if let Some(plant) = supplied_plant(loop_definition, &scenario, values)? {
            supplied.insert(scenario.id.clone(), plant);
        }
    }
    if let Some(plant) = state_case::recovery_plant(loop_definition, model, values)? {
        supplied.insert(state_case::STATE_RECOVERY_ID.to_owned(), plant);
    }
    Ok(supplied)
}

/// The model and plant of a declared scenario that supplies its own model (C8 Validate, B5.3): the model decodes, and its route is the
/// model's inline plant with the controller's initial values at the model's own operating point (`model_route`). A scenario without a
/// model runs on the study's model, so it has none.
///
/// # Errors
/// The supplied model does not decode (`scenario_model_invalid`, with the requirements as the next action), or its route refuses (see
/// `model_route`).
pub fn supplied_plant(
    loop_definition: &LoopDefinition,
    scenario: &EngineerScenario,
    values: &BTreeMap<String, f64>,
) -> Result<Option<SuppliedPlant>, Failure> {
    let Some(value) = &scenario.model else {
        return Ok(None);
    };
    let model = decode_general_model(value).ok_or_else(|| {
        Failure::new(
            &texts::scenario_model_invalid(&scenario.id),
            Cause::ModelOrData,
            NextAction::OpenRequirements,
        )
    })?;
    let route = model_route(loop_definition, &model, values)?;
    Ok(Some(SuppliedPlant { model, route }))
}

/// The loop's declared scenarios, decoded and checked: an absent list is empty (`verificationScenarios ?? []`), and a
/// list that does not decode or is not valid is refused (`validEngineerScenarios`). The study's inputs and its validations
/// read the same list (`study::inputs`, `study::validate`).
pub(crate) fn declared_scenarios(
    loop_definition: &LoopDefinition,
) -> Result<Vec<EngineerScenario>, Failure> {
    let scenarios = match &loop_definition.verification_scenarios {
        None => Vec::new(),
        Some(value) => decode_value::<Vec<EngineerScenario>>(value.clone())
            .map_err(|_| requirements_failure(&texts::scenarios_invalid()))?,
    };
    if !valid_engineer_scenarios(&scenarios) {
        return Err(requirements_failure(&texts::scenarios_invalid()));
    }
    Ok(scenarios)
}

/// One declared scenario's case. The checks run in the `TypeScript`'s order: the operating range, the supplied model and its plan, the
/// frozen parameters, the event grid, the effects, the requirement count and identities, and each requirement against the planned run.
/// A scenario that supplies its own model is planned on that model (C8); any other scenario is planned on the study's.
fn declared_case(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
    values: &BTreeMap<String, f64>,
    nominal: &VerificationPlan,
    scenario: EngineerScenario,
) -> Result<CampaignCase, Failure> {
    // A model is linear about its own operating point: another operating range needs the model validated there.
    if scenario.operating_outputs.is_some() && scenario.model.is_none() {
        return Err(Failure::new(
            &texts::scenario_operating_model_missing(&scenario.id),
            Cause::ModelOrData,
            NextAction::OpenRequirements,
        ));
    }
    let supplied = supplied_plant(loop_definition, &scenario, values)?;
    let supplied_plan = supplied
        .as_ref()
        .map(|plant| {
            engineer_verification_plan(
                loop_definition,
                &plant.model,
                task,
                values,
                &plant.route.initial_values,
            )
        })
        .transpose()?;
    let base = supplied_plan.as_ref().unwrap_or(nominal);
    let planned_model = supplied.as_ref().map_or(model, |plant| &plant.model);
    let frozen = frozen_symbols(loop_definition);
    if scenario
        .initial_values
        .keys()
        .any(|symbol| frozen.contains(symbol.as_str()))
        || scenario
            .events
            .iter()
            .any(|event| frozen.contains(event.symbol.as_str()))
    {
        return Err(requirements_failure(
            &texts::scenario_overwrites_parameters(&scenario.id),
        ));
    }
    let step = milliseconds_as_float(base.step_ms);
    let duration = milliseconds_as_float(base.duration_ms);
    if scenario
        .events
        .iter()
        .any(|event| event.at_ms % step != 0.0 || event.at_ms > duration)
    {
        return Err(requirements_failure(&texts::scenario_events_off_grid(
            &scenario.id,
        )));
    }
    if let Some(effects) = &scenario.effects {
        check_scenario_effects(effects, planned_model, base.duration_ms)?;
    }
    let requirements = case_requirements(&scenario.id, base, scenario.requirements)?;
    Ok(CampaignCase {
        id: scenario.id,
        plan: base.clone(),
        initial_overrides: scenario.initial_values,
        events: scenario.events,
        effects: scenario.effects.unwrap_or_default(),
        requirements,
    })
}

/// The requirements a declared case is judged on: the plan's constraints, without its settling and error checks (the
/// scenario replaces them), then the scenario's own. They must be at most 64 with distinct identities, and each must
/// hold for the planned run.
fn case_requirements(
    id: &str,
    base: &VerificationPlan,
    own: Vec<RunRequirement>,
) -> Result<Vec<RunRequirement>, Failure> {
    let constraints = base
        .requirements
        .iter()
        .filter(|requirement| {
            !matches!(
                requirement.check,
                RunCheck::Settling { .. } | RunCheck::Error { .. }
            )
        })
        .cloned();
    let requirements: Vec<RunRequirement> = constraints.chain(own).collect();
    let identities: BTreeSet<&str> = requirements
        .iter()
        .map(|requirement| requirement.id.as_str())
        .collect();
    if requirements.len() > MAXIMUM_REQUIREMENTS || identities.len() != requirements.len() {
        return Err(requirements_failure(&texts::scenario_requirements_invalid(
            id,
        )));
    }
    for requirement in &requirements {
        check_planned_requirement(requirement, base)?;
    }
    Ok(requirements)
}

/// The symbols of the frozen controller parameters, which no scenario may write.
fn frozen_symbols(loop_definition: &LoopDefinition) -> BTreeSet<&str> {
    loop_definition
        .parameter_binding
        .as_ref()
        .map(|binding| {
            binding
                .parameters
                .iter()
                .map(|parameter| parameter.symbol.as_str())
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default()
}

/// `decodeRunRequirement` against the planned run, which does not exist yet. Its channels are the plan's observed
/// symbols, its samples are the task grid, and the grid's last time is its duration. `check_requirement` reads the
/// channels, the window and the calculation only, so the evidence, the revisions, the seed and the channel units below
/// are placeholders it never reads. The plan carries no units, so the unit match of an error's channel reference is not
/// made here; the run's own evaluation makes it.
fn check_planned_requirement(
    requirement: &RunRequirement,
    plan: &VerificationPlan,
) -> Result<(), Failure> {
    let sample_times_seconds = recorded_sample_times(plan.step_ms, plan.duration_ms, 1);
    let run = ValidationRun {
        id: requirement.id.clone(),
        evidence: RunEvidence::ModelPredicted,
        parameters: BTreeMap::new(),
        conditions: RunConditions {
            initial_state: BTreeMap::new(),
            events: Vec::new(),
            duration_seconds: sample_times_seconds.last().copied().unwrap_or_default(),
            sample_times_seconds,
            seed: 1,
            model_revision: String::new(),
            program_revision: String::new(),
            parameter_schema_revision: String::new(),
        },
        channels: plan
            .observed_symbols
            .iter()
            .map(|symbol| ValidationChannel {
                id: symbol.clone(),
                unit: String::new(),
            })
            .collect(),
        samples: Vec::new(),
    };
    check_requirement(requirement, &run)
}

/// The verdict of the campaign from its results (`campaignStatus`): `Fail` when any result fails; otherwise
/// `Inconclusive` unless each expected case has exactly one result, that result passes, no other result exists, and all
/// results ran on one project revision; otherwise `Pass`.
#[must_use]
pub fn campaign_status<E: RunEvaluation>(
    expected: &[String],
    results: &[ScenarioResult<E>],
) -> RequirementStatus {
    let verdicts: Vec<Verdict<'_>> = results
        .iter()
        .map(|entry| Verdict {
            id: entry.id.as_str(),
            revision: entry.result.project_revision.as_str(),
            status: entry.result.evaluation.status(),
        })
        .collect();
    campaign_verdict(expected, &verdicts)
}

/// One result as the campaign verdict reads it: the case it ran, the project revision it ran on, and its status.
struct Verdict<'a> {
    id: &'a str,
    revision: &'a str,
    status: RequirementStatus,
}

/// The campaign verdict on plain verdicts, so that the rule can be stated without building whole runs.
fn campaign_verdict(expected: &[String], verdicts: &[Verdict<'_>]) -> RequirementStatus {
    if verdicts
        .iter()
        .any(|verdict| verdict.status == RequirementStatus::Fail)
    {
        return RequirementStatus::Fail;
    }
    let ids: BTreeSet<&str> = verdicts.iter().map(|verdict| verdict.id).collect();
    let revisions: BTreeSet<&str> = verdicts.iter().map(|verdict| verdict.revision).collect();
    let each_expected_passes = expected.iter().all(|id| {
        verdicts
            .iter()
            .any(|verdict| verdict.id == id.as_str() && verdict.status == RequirementStatus::Pass)
    });
    if expected.is_empty()
        || verdicts.len() != expected.len()
        || ids.len() != verdicts.len()
        || !each_expected_passes
        || revisions.len() != 1
    {
        return RequirementStatus::Inconclusive;
    }
    RequirementStatus::Pass
}

#[cfg(test)]
#[path = "engineer_verification_campaign_tests.rs"]
mod tests;
