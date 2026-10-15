//! The Validate of a switching controller on the actual ST (specification 35, C3 and B5; annex N8): the checks of its two output levels
//! and its dwell times, the checks of an on-off actuator's own levels and minimum times, the switching-cycle scenario that crosses both
//! thresholds, the target case without its dwell checks, the record period of every execution, and the overrides that write the three
//! settings by symbol. The rules are the TypeScript's (`engineerVerificationPlan.ts`, `engineerControllerScenarios.ts` and
//! `engineerVerificationCampaign.ts`).

use std::collections::BTreeMap;

use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{LoopDefinition, OnOffController};
use crate::model::simulation::ActuatorCapabilities;
use crate::model::GeneralModel;
use crate::validation::engineer_verification_campaign::CampaignCase;
use crate::validation::engineer_verification_plan::{
    engineer_verification_plan, EngineerRequirements,
};
use crate::validation::isolated_run_protocol::IsolatedParameterOverride;
use crate::validation::isolated_run_sampling::isolated_record_every;
use crate::validation::run_requests::override_of;
use crate::validation::run_validation::{DwellState, RunCheck, RunRequirement, Window};
use crate::validation::scenario_contract::ScenarioEvent;

/// The identifier of the switching-cycle scenario (`switching-cycle`, `engineerControllerScenarios.ts`).
pub(crate) const SWITCHING_CYCLE: &str = "switching-cycle";
/// The refusal of a reference range that cannot take the output across both thresholds (`engineerControllerScenarios.ts`).
const RANGE_TOO_NARROW: &str =
    "The reference range cannot exercise both switching thresholds; review scenario requirements.";
/// The refusal of a switching controller whose block or setpoint the run needs and the loop does not declare.
const NO_SWITCHING_BLOCK: &str =
    "The switching controller's block and its setpoint are needed to validate it.";

/// The switching requirements of a run (`engineerVerificationPlan.ts`, the switching block): the output stays on its two levels, and each
/// state's completed dwell is at least its minimum, measured at the midpoint threshold of the two levels (C3).
///
/// # Errors
/// The loop declares no switching block, or the settings lack a value that a requirement needs.
pub(crate) fn switching_requirements(
    loop_definition: &LoopDefinition,
    values: &BTreeMap<String, f64>,
    window: Window,
) -> Result<Vec<RunRequirement>, Failure> {
    let Some(block) = loop_definition.controller.on_off.as_ref() else {
        return Ok(Vec::new());
    };
    let channel = loop_definition.signals.manipulated_variable.symbol.as_str();
    let threshold = f64::midpoint(block.low_output, block.high_output);
    let on_minimum = value_of(values, &block.minimum_on_time_symbol)?;
    let off_minimum = value_of(values, &block.minimum_off_time_symbol)?;
    Ok(vec![
        requirement(
            "switching-output-levels",
            channel,
            window,
            RunCheck::Levels {
                allowed: vec![block.low_output, block.high_output],
                tolerance: 0.0,
            },
        ),
        requirement(
            "switching-on-dwell",
            channel,
            window,
            RunCheck::Dwell {
                threshold,
                state: DwellState::High,
                minimum_seconds: on_minimum,
            },
        ),
        requirement(
            "switching-off-dwell",
            channel,
            window,
            RunCheck::Dwell {
                threshold,
                state: DwellState::Low,
                minimum_seconds: off_minimum,
            },
        ),
    ])
}

/// The checks of an on-off actuator (`engineerVerificationPlan.ts`, the input loop): its two levels, and each minimum time it declares
/// as a completed dwell at the midpoint of its levels. A declared time of zero adds no check, and an undeclared one adds none either.
#[must_use]
pub(crate) fn actuator_requirements(
    actuator: &ActuatorCapabilities,
    channel: &str,
    prefix: &str,
    window: Window,
) -> Vec<RunRequirement> {
    let threshold = f64::midpoint(actuator.minimum, actuator.maximum);
    let mut requirements = vec![requirement(
        &format!("{prefix}actuator-levels"),
        channel,
        window,
        RunCheck::Levels {
            allowed: vec![actuator.minimum, actuator.maximum],
            tolerance: 0.0,
        },
    )];
    let dwells = [
        (
            "actuator-high-dwell",
            DwellState::High,
            actuator.minimum_on_seconds,
        ),
        (
            "actuator-low-dwell",
            DwellState::Low,
            actuator.minimum_off_seconds,
        ),
    ];
    for (id, state, seconds) in dwells {
        if let Some(minimum_seconds) = seconds.filter(|seconds| *seconds > 0.0) {
            requirements.push(requirement(
                &format!("{prefix}{id}"),
                channel,
                window,
                RunCheck::Dwell {
                    threshold,
                    state,
                    minimum_seconds,
                },
            ));
        }
    }
    requirements
}

/// The requirements of the target case of a switching controller: every check but its dwell checks (`engineerVerificationCampaign.ts`).
/// The switching-cycle scenario keeps them, so the dwell is judged where the output crosses both thresholds.
#[must_use]
pub(crate) fn without_dwell(requirements: &[RunRequirement]) -> Vec<RunRequirement> {
    requirements
        .iter()
        .filter(|requirement| !matches!(requirement.check, RunCheck::Dwell { .. }))
        .cloned()
        .collect()
}

/// The switching-cycle case (`engineerControllerScenarios.ts`): the reference starts at its high level and then steps to its low and
/// high levels in three events, each a quarter of the run, so that the output crosses both thresholds and completes its dwells. The run
/// is four quarters long, each at least twice the longer dwell. Its settling and error checks are dropped, as the scenario judges
/// the switching checks.
///
/// # Errors
/// The loop declares no switching block or no setpoint; a setting has no value; or the reference range is too narrow for both thresholds.
pub(crate) fn switching_cycle_case(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
    values: &BTreeMap<String, f64>,
    controller_initial: &BTreeMap<String, f64>,
) -> Result<CampaignCase, Failure> {
    let (Some(block), Some(reference)) = (
        loop_definition.controller.on_off.as_ref(),
        loop_definition.signals.setpoint.as_ref(),
    ) else {
        return Err(Failure::new(
            NO_SWITCHING_BLOCK,
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        ));
    };
    let step_ms = loop_definition.controller.task_period_ms;
    let dwell = value_of(values, &block.minimum_on_time_symbol)?
        .max(value_of(values, &block.minimum_off_time_symbol)?)
        .max(step_ms / 1000.0);
    let quarter =
        ((task.duration_seconds / 4.0).max(2.0 * dwell) * 1000.0 / step_ms).ceil() * step_ms;
    let duration_seconds = 4.0 * quarter / 1000.0;
    let mut scenario_task = task.clone();
    scenario_task.duration_seconds = duration_seconds;
    scenario_task.settling_time_seconds = task.settling_time_seconds.min(duration_seconds / 2.0);
    let plan = engineer_verification_plan(
        loop_definition,
        model,
        &scenario_task,
        values,
        controller_initial,
    )?;
    let process = &loop_definition.signals.process_variable.range;
    let low = reference.range.minimum.max(process.minimum);
    let high = reference.range.maximum.min(process.maximum);
    let width = block
        .band_convention
        .half_width(value_of(values, &block.hysteresis_symbol)?);
    if high - low <= 2.0 * width {
        return Err(Failure::new(
            RANGE_TOO_NARROW,
            Cause::Settings,
            NextAction::OpenRequirements,
        ));
    }
    let events = (1..=3_u32)
        .map(|index| ScenarioEvent {
            at_ms: quarter * f64::from(index),
            symbol: reference.symbol.clone(),
            value: if index % 2 == 1 { low } else { high },
        })
        .collect();
    let requirements = plan
        .requirements
        .iter()
        .filter(|requirement| {
            !matches!(
                requirement.check,
                RunCheck::Settling { .. } | RunCheck::Error { .. }
            )
        })
        .cloned()
        .collect();
    Ok(CampaignCase {
        id: SWITCHING_CYCLE.to_owned(),
        plan,
        initial_overrides: BTreeMap::from([(reference.symbol.clone(), high)]),
        events,
        effects: Vec::new(),
        requirements,
    })
}

/// The record period of a switching controller's run: every execution, so that its levels and dwell times are judged at every scan
/// (`engineerVerificationPlan.ts`). The sample and cell limits still apply, as they do to every run.
#[must_use]
pub(crate) fn record_every(step_ms: u64, duration_ms: u64, observed: usize) -> u64 {
    isolated_record_every(
        step_ms,
        duration_ms,
        u64::try_from(observed).unwrap_or(u64::MAX),
    )
}

/// The overrides that write the three settings of a switching controller, by symbol, each with the type, unit, convention and bounds
/// that its binding declares (B5.3, C3). The run of the running controller and the run of the candidate both use them.
///
/// # Errors
/// The loop declares no switching block or no binding, a setting has no value, or the binding has no parameter for it.
pub(crate) fn overrides(
    loop_definition: &LoopDefinition,
    values: &BTreeMap<String, f64>,
) -> Result<Vec<IsolatedParameterOverride>, Failure> {
    let (Some(block), Some(binding)) = (
        loop_definition.controller.on_off.as_ref(),
        loop_definition.parameter_binding.as_ref(),
    ) else {
        return Err(Failure::new(
            NO_SWITCHING_BLOCK,
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        ));
    };
    settings_of(block)
        .into_iter()
        .map(|symbol| {
            let value = value_of(values, symbol)?;
            override_of(binding, symbol, value)
        })
        .collect()
}

/// The three setting symbols of a switching controller, in the order the overrides write them.
fn settings_of(block: &OnOffController) -> [&str; 3] {
    [
        block.hysteresis_symbol.as_str(),
        block.minimum_on_time_symbol.as_str(),
        block.minimum_off_time_symbol.as_str(),
    ]
}

/// The value of a setting, which the settings of a run must hold.
fn value_of(values: &BTreeMap<String, f64>, symbol: &str) -> Result<f64, Failure> {
    values.get(symbol).copied().ok_or_else(|| {
        Failure::new(
            &format!("The switching setting {symbol} has no value."),
            Cause::Settings,
            NextAction::Tune,
        )
    })
}

/// One requirement of a run: its identifier, its channel, its window and its check.
fn requirement(id: &str, channel: &str, window: Window, check: RunCheck) -> RunRequirement {
    RunRequirement {
        id: id.to_owned(),
        channel: channel.to_owned(),
        window,
        check,
    }
}

#[cfg(test)]
#[path = "on_off_case_tests.rs"]
mod tests;
