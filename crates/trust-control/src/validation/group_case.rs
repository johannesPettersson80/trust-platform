//! The isolated runs of a PID group's candidate (specification 35, C7 and B5.3): every member's gains and each entry of the static
//! decoupler are written as parameter overrides, each under its own symbol with the type, unit, convention and bounds that its binding
//! declares (`run_requests::override_of`). The group's values are the candidate's, so the run tests the group that Apply would write.
//! The campaign of a group adds one case for each controlled output, which excites that output's setpoint alone
//! (`independent_reference_cases`).

use std::collections::BTreeMap;

use super::engineer_verification_campaign::CampaignCase;
use super::engineer_verification_plan::{
    engineer_verification_plan, output_requirements_missing, output_task_of, EngineerRequirements,
};
use super::isolated_run_protocol::IsolatedParameterOverride;
use super::run_requests::override_of;
use super::texts;
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::LoopDefinition;
use crate::model::GeneralModel;
use crate::tuning::group_candidate::GroupCandidate;
use crate::tuning::output_requirements::OutputRequirements;

/// The identity prefix of a case that excites one controlled output alone (`independentReferenceScenarios`); the output's model channel
/// identifier follows it.
const INDEPENDENT_REFERENCE_PREFIX: &str = "independent-reference:";

/// The overrides of a PID group's candidate: one for each value of the candidate, in symbol order.
///
/// # Errors
/// The loop has no parameter binding, or the binding has no parameter of one of the candidate's symbols.
pub(crate) fn overrides(
    loop_definition: &LoopDefinition,
    group: &GroupCandidate,
) -> Result<Vec<IsolatedParameterOverride>, Failure> {
    let Some(binding) = &loop_definition.parameter_binding else {
        return Err(Failure::new(
            &texts::parameter_or_reference_binding_missing(),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ));
    };
    group
        .values
        .iter()
        .map(|(symbol, value)| override_of(binding, symbol, *value))
        .collect()
}

/// The cases that excite each controlled output of a PID group alone (specification 35, C7; `independentReferenceScenarios`): one case
/// for each output with a setpoint, identified `independent-reference:<model channel id>`, when the group has two or more of them. In
/// that case the output's own setpoint steps by its reference change (its own record, its loop's requirement, or the task for the process
/// variable).
/// Every other controlled output keeps its requirements with a reference change of zero, so its setpoint holds at its operating value
/// and its settling and error checks still apply. A loop without a group has no such case.
///
/// # Errors
/// A held output has no requirements to hold it to (see `output_requirements_missing`), or the plan of a case refuses (see
/// `engineer_verification_plan`).
pub(crate) fn independent_reference_cases(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
    values: &BTreeMap<String, f64>,
    controller_initial: &BTreeMap<String, f64>,
) -> Result<Vec<CampaignCase>, Failure> {
    let controlled: Vec<&str> = loop_definition
        .channels
        .iter()
        .flat_map(|channels| channels.outputs.iter())
        .filter(|output| output.setpoint_signal_id.is_some())
        .map(|output| output.model_channel_id.as_str())
        .collect();
    if loop_definition.controller.group.is_none() || controlled.len() < 2 {
        return Ok(Vec::new());
    }
    controlled
        .into_iter()
        .map(|active| {
            excited_case(
                loop_definition,
                model,
                task,
                values,
                controller_initial,
                active,
            )
        })
        .collect()
}

/// The case that excites the output `active` alone: the plan of the loop, held to the task that excites that output alone.
///
/// # Errors
/// As [`independent_reference_cases`].
fn excited_case(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
    values: &BTreeMap<String, f64>,
    controller_initial: &BTreeMap<String, f64>,
    active: &str,
) -> Result<CampaignCase, Failure> {
    let held = excite_only(loop_definition, task, active)?;
    let plan =
        engineer_verification_plan(loop_definition, model, &held, values, controller_initial)?;
    Ok(CampaignCase {
        id: format!("{INDEPENDENT_REFERENCE_PREFIX}{active}"),
        requirements: plan.requirements.clone(),
        plan,
        initial_overrides: BTreeMap::new(),
        events: Vec::new(),
        effects: Vec::new(),
    })
}

/// The task of the case that excites `active` alone (`independentReferenceScenarios`): every other controlled output keeps the
/// requirements it holds to (its own record, else its loop's, else the task for the process variable), with a reference change of zero,
/// so its setpoint holds at its operating value. The active output keeps its own requirements.
///
/// # Errors
/// A held output has no requirements to hold it to (see `output_requirements_missing`).
fn excite_only(
    loop_definition: &LoopDefinition,
    task: &EngineerRequirements,
    active: &str,
) -> Result<EngineerRequirements, Failure> {
    let mut held_channels = task.channel_requirements.clone().unwrap_or_default();
    let outputs = loop_definition
        .channels
        .iter()
        .flat_map(|channels| channels.outputs.iter());
    for output in outputs {
        if output.setpoint_signal_id.is_none() || output.model_channel_id == active {
            continue;
        }
        let process_variable = output.signal_id == "processVariable";
        let own = output_task_of(
            task,
            &output.model_channel_id,
            output.requirements,
            process_variable,
        )
        .ok_or_else(|| output_requirements_missing(&output.model_channel_id))?;
        held_channels.insert(
            output.model_channel_id.clone(),
            OutputRequirements {
                reference_change: 0.0,
                ..OutputRequirements::of_record(&own)
            },
        );
    }
    Ok(EngineerRequirements {
        channel_requirements: Some(held_channels),
        ..task.clone()
    })
}

#[cfg(test)]
#[path = "group_case_tests.rs"]
mod tests;
