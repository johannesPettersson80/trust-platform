//! The isolated test runs of Validate (runtime protocol annex E8.4; specification 35, B5.3, A2.6 and A9): the request of one run,
//! for a campaign case under one set of settings, and the conversion of a completed run into the record that the evaluator
//! reads. The engine sends the request and checks the answer (`IsolatedRunRequest::check` and `IsolatedRunStatus::check`);
//! this module decides what the request holds (`TypeScript` `engineerVerificationPlan` and `configuredPlantRequest`).
//!
//! The current settings run with no parameter overrides, so the program's own compiled record is the one under test. The
//! candidate runs with its record written as parameter overrides, one per record field that the controller maps
//! (`mapped_parameters`). The plant is the caller's choice: no inline plant runs the configured simulated plant, and a
//! `SimulationOverride` runs the selected model (B5.3).

use std::collections::BTreeMap;

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::connected_record::{
    RunConditions, ValidationRun, ValidationSample, ValidationValue,
};
use crate::limits::ISOLATED_SIMULATION_WALL_TIME_LIMIT_MS;
use crate::loop_definition::{LoopDefinition, RealType};
use crate::model::{GeneralModel, SimulationOverride};
use crate::recording::channels::ChannelQuality;
use crate::signals::Quality;
use crate::tuning::candidate::ControlPidCandidate;
use crate::tuning::settings_pins::{mapped_parameters, GainKey, MappedParameter};
use crate::validation::engineer_scenario_effects::{requirements_failure, scenario_effect_tables};
use crate::validation::engineer_verification_campaign::CampaignCase;
use crate::validation::engineer_verification_plan::VerificationPlan;
use crate::validation::isolated_run_protocol::{
    IsolatedExecution, IsolatedParameterOverride, IsolatedParameterType, IsolatedRunEvent,
    IsolatedRunObservation, IsolatedRunRecord, IsolatedRunRequest, IsolatedRunSample,
};
use crate::validation::texts;

/// The largest integer that JSON carries exactly, 2⁵³ − 1 (E0.3). It is exact as a float, because it is below 2⁵³.
const MAXIMUM_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
/// The refusal of a seed that the validation record cannot hold: its seed is 32 bits.
const SEED_TOO_LARGE: &str =
    "The run's seed does not fit the validation record, which holds 32-bit seeds.";
/// The refusal of an event time that is not a whole number of milliseconds of the run.
const EVENT_OFF_MILLISECOND: &str =
    "A scenario event does not fall on a whole millisecond of its run.";
/// The refusal of a candidate that sets no measurement filter time for a controller that declares one (C0).
const FILTER_MISSING: &str =
    "The candidate holds no measurement filter time, but the controller declares one.";

/// The settings an isolated run takes (B5.3): the program's own compiled record, or the candidate's record.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RunSettings<'a> {
    /// The controller's settings as they are now (B5.3): the record's values read from the running controller, by symbol,
    /// written to the run as parameter overrides exactly as the candidate's are (E8.3, E8.4). The compiled values are not the
    /// current settings once Apply has written a record.
    Current(&'a BTreeMap<String, f64>),
    /// The candidate's record, written to the run as parameter overrides (E8.4).
    Candidate(&'a ControlPidCandidate),
}

/// What one isolated run request is built from (B5.3, E8.4).
#[derive(Clone, Copy)]
pub struct RunRequestInput<'a> {
    /// The loop the run tests: its signals, its bound parameters and its experiment seed.
    pub loop_definition: &'a LoopDefinition,
    /// The selected model; the channels of a scenario effect are counted on it (`scenario_effect_tables`).
    pub model: &'a GeneralModel,
    /// The verification plan of the campaign: the sample grid, the initial values and the observed symbols.
    pub plan: &'a VerificationPlan,
    /// The campaign case the run executes: its events, its initial overrides and its effects.
    pub case: &'a CampaignCase,
    /// The settings the run tests.
    pub settings: RunSettings<'a>,
    /// The inline plant of the run. `None` runs the configured simulated plant; the selected model, or a scenario's own
    /// model, is given as its `SimulationOverride`. A case with effects needs one (B5.3).
    pub simulation: Option<&'a SimulationOverride>,
    /// The request's identifier, 1 to 128 ASCII letters, digits or hyphens (E0.2, E8.4).
    pub request_id: &'a str,
    /// The project revision the run is reviewed on (E8.2).
    pub expected_project_revision: &'a str,
}

/// The isolated run request of one campaign case under one set of settings (E8.4, B5.3): the plan's grid, the case's events
/// and initial values, the observed channels with their units, the settings as parameter overrides and the inline plant.
/// The request is checked against the limits of E8.4 before it is returned.
///
/// # Errors
/// A case with effects but no inline plant; an event that is not on a whole millisecond; an observed signal with no declared
/// unit; a candidate record without a parameter binding, or whose record lacks a value the binding maps; a request that
/// breaks a limit of E8.4 (`IsolatedRunRequest::check`).
pub fn isolated_run_request(input: &RunRequestInput<'_>) -> Result<IsolatedRunRequest, Failure> {
    let plan = input.plan;
    let case = input.case;
    let request = IsolatedRunRequest {
        request_id: input.request_id.to_owned(),
        expected_project_revision: input.expected_project_revision.to_owned(),
        execution: IsolatedExecution::IsolatedSimulation,
        step_ms: plan.step_ms,
        duration_ms: plan.duration_ms,
        seed: run_seed(input.loop_definition),
        max_total_cycles: plan.max_total_cycles,
        wall_budget_ms: ISOLATED_SIMULATION_WALL_TIME_LIMIT_MS,
        record_every: plan.record_every,
        initial_values: initial_values(plan, case),
        events: events(case)?,
        observations: observations(input.loop_definition, plan)?,
        parameter_overrides: parameter_overrides(input.loop_definition, input.settings)?,
        simulation_override: simulation_override(input)?,
        feedback_relay: None,
    };
    request.check()?;
    Ok(request)
}

/// The seed of the loop's experiment, or 1 when the loop declares none (`loop.experiment?.seed ?? 1`).
#[must_use]
pub fn run_seed(loop_definition: &LoopDefinition) -> u64 {
    loop_definition
        .experiment
        .as_ref()
        .map_or(1, |experiment| experiment.seed)
}

/// The unit that the loop declares for a symbol: one of its signals, its auxiliary signals or its bound parameters. `None`
/// when the loop declares no such symbol.
#[must_use]
pub fn observation_unit(loop_definition: &LoopDefinition, symbol: &str) -> Option<String> {
    let signals = &loop_definition.signals;
    let signal_unit = [
        Some(&signals.process_variable),
        Some(&signals.manipulated_variable),
        signals.setpoint.as_ref(),
        signals.actuator.as_ref(),
    ]
    .into_iter()
    .flatten()
    .find(|signal| signal.symbol == symbol)
    .map(|signal| signal.unit.clone());
    let auxiliary_unit = || {
        signals
            .additional
            .iter()
            .flatten()
            .find(|signal| signal.symbol == symbol)
            .map(|signal| signal.unit.clone())
    };
    let parameter_unit = || {
        loop_definition
            .parameter_binding
            .as_ref()
            .and_then(|binding| {
                binding
                    .parameters
                    .iter()
                    .find(|parameter| parameter.symbol == symbol)
            })
            .map(|parameter| parameter.unit.clone())
    };
    signal_unit.or_else(auxiliary_unit).or_else(parameter_unit)
}

/// The inline plant of a run (B5.3, A9): the selected model's, with the tables of the case's effects appended to its TOML, as
/// the engineer's scenario effects are written. A case without effects takes the plant as it is.
fn simulation_override(input: &RunRequestInput<'_>) -> Result<Option<SimulationOverride>, Failure> {
    match (input.simulation, input.case.effects.is_empty()) {
        (None, true) => Ok(None),
        (None, false) => Err(requirements_failure(&texts::scenario_effects_need_plant())),
        (Some(selected), true) => Ok(Some(selected.clone())),
        (Some(selected), false) => {
            let tables =
                scenario_effect_tables(&input.case.effects, input.model, input.plan.duration_ms)?;
            Ok(Some(SimulationOverride {
                simulation_toml: format!("{}\n{tables}", selected.simulation_toml),
                ..selected.clone()
            }))
        }
    }
}

/// The values the run starts from: the plan's, with the case's own initial overrides written over them (N8.1).
fn initial_values(plan: &VerificationPlan, case: &CampaignCase) -> BTreeMap<String, f64> {
    let mut values = plan.initial_values.clone();
    for (symbol, value) in &case.initial_overrides {
        values.insert(symbol.clone(), *value);
    }
    values
}

/// The case's events, each at a whole millisecond of the run (E8.3).
fn events(case: &CampaignCase) -> Result<Vec<IsolatedRunEvent>, Failure> {
    case.events
        .iter()
        .map(|event| -> Result<IsolatedRunEvent, Failure> {
            Ok(IsolatedRunEvent {
                at_ms: whole_milliseconds(event.at_ms)?,
                symbol: event.symbol.clone(),
                value: event.value,
            })
        })
        .collect()
}

/// The observed channels of the plan, each with the unit the loop declares for it (E8.3).
fn observations(
    loop_definition: &LoopDefinition,
    plan: &VerificationPlan,
) -> Result<Vec<IsolatedRunObservation>, Failure> {
    plan.observed_symbols
        .iter()
        .map(|symbol| -> Result<IsolatedRunObservation, Failure> {
            let unit = observation_unit(loop_definition, symbol).ok_or_else(|| {
                Failure::new(
                    &format!("The signal {symbol} has no declared unit."),
                    Cause::ModelOrData,
                    NextAction::OpenLoopSetup,
                )
            })?;
            Ok(IsolatedRunObservation {
                symbol: symbol.clone(),
                unit,
            })
        })
        .collect()
}

/// The parameter overrides of a run (B5.3): the current values of the record for the current settings, and the candidate's
/// record for the candidate, each record field that the controller maps.
fn parameter_overrides(
    loop_definition: &LoopDefinition,
    settings: RunSettings<'_>,
) -> Result<Vec<IsolatedParameterOverride>, Failure> {
    match settings {
        RunSettings::Current(observed) => record_overrides(loop_definition, |mapped| {
            observed.get(&mapped.symbol).copied().ok_or_else(|| {
                Failure::new(
                    &texts::current_setting_unread(&mapped.symbol),
                    Cause::Runtime,
                    NextAction::ConnectRuntime,
                )
            })
        }),
        RunSettings::Candidate(candidate) => record_overrides(loop_definition, |mapped| {
            candidate_value(candidate, mapped.key)
        }),
    }
}

/// A record as parameter overrides: each record field that the controller maps, with the type, unit, convention and bounds that
/// its binding declares, and the value `value_of` gives it (E8.4).
fn record_overrides(
    loop_definition: &LoopDefinition,
    value_of: impl Fn(&MappedParameter) -> Result<f64, Failure>,
) -> Result<Vec<IsolatedParameterOverride>, Failure> {
    let Some(binding) = &loop_definition.parameter_binding else {
        return Err(Failure::new(
            &texts::parameter_or_reference_binding_missing(),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ));
    };
    mapped_parameters(loop_definition)
        .iter()
        .map(|mapped| -> Result<IsolatedParameterOverride, Failure> {
            let parameter = binding
                .parameters
                .iter()
                .find(|parameter| parameter.symbol == mapped.symbol)
                .ok_or_else(|| {
                    Failure::new(
                        &texts::parameter_or_reference_binding_missing(),
                        Cause::Settings,
                        NextAction::OpenLoopSetup,
                    )
                })?;
            Ok(IsolatedParameterOverride {
                symbol: parameter.symbol.clone(),
                iec_type: match &parameter.iec_type {
                    RealType::Real => IsolatedParameterType::Real,
                    RealType::Lreal => IsolatedParameterType::Lreal,
                },
                unit: parameter.unit.clone(),
                convention: parameter.convention.clone(),
                minimum: parameter.minimum,
                maximum: parameter.maximum,
                value: value_of(mapped)?,
            })
        })
        .collect()
}

/// The value of one record field of the candidate, by the gain key that names it (C0): the gains, the setpoint weights, the
/// back-calculation gain `1/Tt` and the measurement filter time.
fn candidate_value(candidate: &ControlPidCandidate, key: GainKey) -> Result<f64, Failure> {
    let record = &candidate.parameters;
    match key {
        GainKey::ProportionalGain => Ok(record.proportional_gain),
        GainKey::IntegralGain => Ok(record.integral_gain),
        GainKey::DerivativeGain => Ok(record.derivative_gain),
        GainKey::ProportionalSetpointWeight => Ok(record.proportional_setpoint_weight),
        GainKey::DerivativeSetpointWeight => Ok(record.derivative_setpoint_weight),
        GainKey::BackCalculationGain => Ok(1.0 / record.tracking_time),
        GainKey::MeasurementFilterSeconds => {
            candidate.measurement_filter_seconds.ok_or_else(|| {
                Failure::new(FILTER_MISSING, Cause::Settings, NextAction::OpenLoopSetup)
            })
        }
    }
}

/// A time in milliseconds as a whole number from 0 to 2⁵³ − 1 (E0.3, E8.3).
#[allow(
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)] // The fraction test is exact: a whole number has no fraction. The cast follows the range check, so it is exact.
fn whole_milliseconds(value: f64) -> Result<u64, Failure> {
    let whole = value.fract() == 0.0;
    if whole && (0.0..=MAXIMUM_SAFE_INTEGER).contains(&value) {
        return Ok(value as u64);
    }
    Err(Failure::new(
        EVENT_OFF_MILLISECOND,
        Cause::Settings,
        NextAction::OpenRequirements,
    ))
}

/// The record the evaluator reads, from a completed isolated run (`validation_run_of`): the same identity, evidence,
/// conditions, channels and samples, with each quality in the record's form. A missing value is bad quality (A4.2).
///
/// # Errors
/// The run's seed does not fit the record's 32-bit seed.
pub fn validation_run_of(record: &IsolatedRunRecord) -> Result<ValidationRun, Failure> {
    let conditions = &record.conditions;
    let seed = u32::try_from(conditions.seed)
        .map_err(|_| Failure::new(SEED_TOO_LARGE, Cause::Runtime, NextAction::None))?;
    Ok(ValidationRun {
        id: record.id.clone(),
        evidence: record.evidence,
        parameters: record.parameters.clone(),
        conditions: RunConditions {
            initial_state: conditions.initial_state.clone(),
            events: conditions.events.clone(),
            duration_seconds: conditions.duration_seconds,
            sample_times_seconds: conditions.sample_times_seconds.clone(),
            seed,
            model_revision: conditions.model_revision.clone(),
            program_revision: conditions.program_revision.clone(),
            parameter_schema_revision: conditions.parameter_schema_revision.clone(),
        },
        channels: record.channels.clone(),
        samples: record.samples.iter().map(sample_of).collect(),
    })
}

/// One sample of an isolated run, in the evaluator's form.
fn sample_of(sample: &IsolatedRunSample) -> ValidationSample {
    ValidationSample {
        time_seconds: sample.time_seconds,
        sequence: sample.sequence,
        values: sample
            .values
            .iter()
            .map(|(symbol, value)| {
                (
                    symbol.clone(),
                    ValidationValue {
                        value: value.value,
                        quality: channel_quality(value.quality),
                    },
                )
            })
            .collect(),
    }
}

/// The evaluator's quality of a value: good and forced stay, and bad and missing are bad (A4.2).
fn channel_quality(quality: Quality) -> ChannelQuality {
    match quality {
        Quality::Good => ChannelQuality::Good,
        Quality::Forced => ChannelQuality::Forced,
        Quality::Bad | Quality::Missing => ChannelQuality::Bad,
    }
}

#[cfg(test)]
#[path = "run_requests_tests.rs"]
mod tests;
