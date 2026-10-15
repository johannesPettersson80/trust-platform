//! The Manual-to-Auto entry case of an Auto commissioning (specification 35, B5.3, B5.4, A6.4 and C1 "Validate"): the actual-ST run
//! that starts the controller where its plant starts (`entry_start_model`), with the candidate's record and its declared
//! initialization, enters Auto at the transfer, and is judged on its first output, its mode, its gains and states, and its signal
//! bounds. The start plans are the commissioning's own, which Apply runs (`commissioning::adapters`). This module turns one of them
//! into a campaign case that the job builder of the other cases requests and the runner ends (E8.4).

use std::collections::BTreeMap;

use trust_control_math::model::OperatingPoint;

use crate::commissioning::adapters::{controller_adapter, AdapterKind, AdapterStartPlan};
use crate::commissioning::pid_initialization::pid_initialization_adapter;
use crate::commissioning::project_law::application_states;
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::LoopDefinition;
use crate::model::{GeneralModel, SimulationOverride};
use crate::tuning::candidate::LawCandidate;
use crate::validation::engineer_scenario_effects::milliseconds_as_float;
use crate::validation::engineer_verification_campaign::CampaignCase;
use crate::validation::engineer_verification_plan::{EngineerRequirements, VerificationPlan};
use crate::validation::initialization_texts;
use crate::validation::isolated_run_protocol::{
    IsolatedRunObservation, IsolatedRunRecord, IsolatedRunRequest,
};
use crate::validation::plant_choice::first_sample_value;
use crate::validation::run_requests::{
    isolated_run_request, observation_unit, RunRequestInput, RunSettings,
};
use crate::validation::scenario_contract::ScenarioEvent;

/// The identity of the entry case in the campaign (B5.3). The case's rows name their own checks.
pub const ENTRY_CASE_ID: &str = "manual-to-auto-entry";

/// The entry run records every scan, so that its first output is judged at the scan that produces it (A6.4).
const RECORD_EVERY: u64 = 1;

/// The Manual-to-Auto entry case, planned (B5.3): the campaign case, which is judged as the other cases are, and the channels that
/// the run observes with a unit that no declared signal or parameter names. The job builder resolves the units of those from the
/// signals and parameters only, so [`EntryCase::request`] adds them with the units the loop declares for them.
#[derive(Debug)]
pub struct EntryCase {
    /// The case: its plan (the run's grid, its starting values and its observed channels), its events and its requirements.
    pub case: CampaignCase,
    /// The mode, the Manual command and the controller states that the run observes, each with its unit.
    pub extra_observations: Vec<IsolatedRunObservation>,
}

/// The model that the Manual-to-Auto entry case plans on (B5.3): the case starts where the plant it runs on starts. On the model, that is
/// the model's own operating point, and `start` is `None`. On the configured plant, `start` is the target-response run of the current
/// settings on that plant: its first sample gives the measured output and the plant input, and the copy of the model that starts from
/// them is returned (`GeneralModel::with_operating_point`). The plan takes one input and one output, so the point holds those two. A
/// run without its first sample, or without one of the two values, keeps the model's own operating point.
#[must_use]
pub fn entry_start_model(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    start: Option<&IsolatedRunRecord>,
) -> GeneralModel {
    let Some(run) = start else {
        return model.clone();
    };
    let output = first_sample_value(run, &loop_definition.signals.process_variable.symbol);
    let input = first_sample_value(run, &loop_definition.signals.manipulated_variable.symbol);
    match (input, output) {
        (Some(input), Some(output)) => model.with_operating_point(OperatingPoint {
            inputs: vec![input],
            outputs: vec![output],
        }),
        _ => model.clone(),
    }
}

/// Plans the entry case of the loop's Auto commissioning (B5.3, A6.4): the start plan that the loop's adapter builds at the operating
/// point of `model`, which the caller passes as the case's start model (see [`entry_start_model`]), from the candidate's record `values`
/// and with the measured output noise `process_noise` (zero without a measured baseline, as Apply uses it). A loop whose adapter cannot
/// start an Auto entry is refused, so Auto stays blocked (C1).
///
/// # Errors
/// The adapter cannot start an Auto entry, or its start plan does not fit the loop (see `ControllerAdapter::initialization_plan`); or a
/// channel of the run has no unit that the loop declares for it.
pub fn plan_entry_case(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
    values: &BTreeMap<String, f64>,
    process_noise: f64,
) -> Result<EntryCase, Failure> {
    let adapter = controller_adapter(loop_definition)?;
    let start = match adapter.initialization_plan(model, task, values, Some(process_noise))? {
        AdapterStartPlan::Start(start) => start,
        AdapterStartPlan::ParallelPid(parallel) => parallel.plan,
    };
    let states = declared_states(loop_definition, adapter.kind())?;
    let (observed_symbols, extra_observations) =
        split_observations(loop_definition, &start.run.observed_symbols, &states)?;
    let events = start
        .run
        .events
        .iter()
        .map(|event| ScenarioEvent {
            at_ms: milliseconds_as_float(event.at_ms),
            symbol: event.symbol.clone(),
            value: event.value,
        })
        .collect();
    let requirements = start.requirements;
    Ok(EntryCase {
        case: CampaignCase {
            id: ENTRY_CASE_ID.to_owned(),
            plan: VerificationPlan {
                step_ms: start.run.step_ms,
                duration_ms: start.run.duration_ms,
                max_total_cycles: start.run.max_total_cycles,
                record_every: RECORD_EVERY,
                initial_values: start.run.initial_values,
                observed_symbols,
                requirements: requirements.clone(),
            },
            initial_overrides: BTreeMap::new(),
            events,
            effects: Vec::new(),
            requirements,
        },
        extra_observations,
    })
}

impl EntryCase {
    /// The isolated request of the entry case under the candidate's record (E8.4, B5.3): the job builder of the other cases builds it
    /// from the plan, the case, the candidate's parameter overrides and the inline plant, and the extra channels follow with their units.
    ///
    /// # Errors
    /// The job builder refuses the request (see `isolated_run_request`), or the request with its extra channels breaks a limit of E8.4.
    pub fn request(
        &self,
        loop_definition: &LoopDefinition,
        model: &GeneralModel,
        candidate: &LawCandidate,
        simulation: Option<&SimulationOverride>,
        request_id: &str,
        project_revision: &str,
    ) -> Result<IsolatedRunRequest, Failure> {
        let mut request = isolated_run_request(&RunRequestInput {
            loop_definition,
            model,
            plan: &self.case.plan,
            case: &self.case,
            settings: RunSettings::Candidate(candidate),
            simulation,
            request_id,
            expected_project_revision: project_revision,
        })?;
        request
            .observations
            .extend(
                self.extra_observations
                    .iter()
                    .map(|observation| IsolatedRunObservation {
                        symbol: observation.symbol.clone(),
                        unit: observation.unit.clone(),
                    }),
            );
        request.check()?;
        Ok(request)
    }
}

/// The states the entry run initializes and observes, each with the unit its declaration gives (C0, C1). A controller without states
/// has none.
fn declared_states(
    loop_definition: &LoopDefinition,
    kind: &AdapterKind,
) -> Result<Vec<(String, String)>, Failure> {
    match kind {
        AdapterKind::OnOff(initialization) => Ok(initialization
            .states()
            .iter()
            .map(|state| (state.symbol.clone(), state.unit.clone()))
            .collect()),
        AdapterKind::ControlPid(initialization) => Ok(initialization
            .states()
            .iter()
            .map(|state| (state.symbol.clone(), state.unit.clone()))
            .collect()),
        AdapterKind::ProjectPid { project, .. }
        | AdapterKind::Cascade { project, .. }
        | AdapterKind::Group { project, .. }
        | AdapterKind::Search { project } => Ok(project
            .initialization
            .states()
            .iter()
            .map(|state| (state.symbol.clone(), state.unit.clone()))
            .collect()),
        AdapterKind::State(law) => Ok(law
            .initialization
            .states()
            .iter()
            .map(|state| (state.symbol.clone(), state.unit.clone()))
            .collect()),
        AdapterKind::ParallelPid => {
            let contract = pid_initialization_adapter(loop_definition)?;
            Ok([contract.integral_state, contract.derivative_state]
                .iter()
                .map(|state| (state.symbol.clone(), state.unit.clone()))
                .collect())
        }
        // A program's own P law starts its application states (C1), which the entry run observes; a Control.PID's P law has none.
        AdapterKind::ConnectedP => Ok(application_states(loop_definition)
            .into_iter()
            .map(|state| (state.symbol.clone(), state.unit.clone()))
            .collect()),
    }
}

/// Splits the channels of the run: a channel that a declared signal or parameter names keeps its unit through the job builder, and the
/// others (the mode, the Manual command and the states) take the unit that the loop declares for them (E8.3).
fn split_observations(
    loop_definition: &LoopDefinition,
    symbols: &[String],
    states: &[(String, String)],
) -> Result<(Vec<String>, Vec<IsolatedRunObservation>), Failure> {
    let mut observed = Vec::new();
    let mut extra = Vec::new();
    for symbol in symbols {
        if observation_unit(loop_definition, symbol).is_some() {
            observed.push(symbol.clone());
            continue;
        }
        let unit = undeclared_unit(loop_definition, states, symbol).ok_or_else(|| {
            Failure::new(
                &initialization_texts::unit_not_declared(symbol),
                Cause::ModelOrData,
                NextAction::OpenLoopSetup,
            )
        })?;
        extra.push(IsolatedRunObservation {
            symbol: symbol.clone(),
            unit,
        });
    }
    Ok((observed, extra))
}

/// The unit of a channel that no signal or parameter names: the mode is dimensionless, the Manual command has the unit of the output
/// (C1), and a state has the unit its declaration gives. `None` when the loop declares none of these for the symbol.
fn undeclared_unit(
    loop_definition: &LoopDefinition,
    states: &[(String, String)],
    symbol: &str,
) -> Option<String> {
    let mode = loop_definition
        .parameter_binding
        .as_ref()
        .map(|binding| binding.mode.symbol.as_str());
    if mode == Some(symbol) {
        return Some("1".to_owned());
    }
    if loop_definition.controller.manual_output_signal.as_deref() == Some(symbol) {
        return Some(loop_definition.signals.manipulated_variable.unit.clone());
    }
    states
        .iter()
        .find(|(name, _)| name == symbol)
        .map(|(_, unit)| unit.clone())
}

#[cfg(test)]
#[path = "initialization_case_tests.rs"]
mod tests;
