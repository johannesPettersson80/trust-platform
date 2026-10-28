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

use crate::commissioning::gain_schedule_adapter::gains_of_table;
use crate::commissioning::search_adapter::search_law;
use crate::commissioning::state_adapter::{
    coefficient_symbols as state_coefficient_symbols, decode_state_law,
};
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::connected_record::{
    RunConditions, ValidationRun, ValidationSample, ValidationValue,
};
use crate::limits::ISOLATED_SIMULATION_WALL_TIME_LIMIT_MS;
use crate::loop_definition::{LoopDefinition, ParameterBinding, RealType};
use crate::model::{GeneralModel, SimulationOverride};
use crate::recording::channels::ChannelQuality;
use crate::signals::Quality;
use crate::tuning::candidate::{ControlPidCandidate, LawCandidate};
use crate::tuning::project_candidate::{served_feedforward, served_trajectory};
use crate::tuning::settings_pins::{mapped_parameters, GainKey, MappedParameter};
use crate::validation::engineer_scenario_effects::{requirements_failure, scenario_effect_tables};
use crate::validation::engineer_verification_campaign::CampaignCase;
use crate::validation::engineer_verification_plan::VerificationPlan;
use crate::validation::gain_schedule_case;
use crate::validation::group_case;
use crate::validation::isolated_run_protocol::{
    IsolatedExecution, IsolatedParameterOverride, IsolatedParameterType, IsolatedRunEvent,
    IsolatedRunObservation, IsolatedRunRecord, IsolatedRunRequest, IsolatedRunSample,
};
use crate::validation::on_off_case;
use crate::validation::search_case::candidate_coefficients;
use crate::validation::smith_case;
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
    /// The candidate's values, written to the run as parameter overrides (E8.4): a `Control.PID` record's fields, or the seed
    /// variables of a program's own law.
    Candidate(&'a LawCandidate),
}

/// What one isolated run request is built from (B5.3, E8.4).
#[derive(Clone, Copy)]
pub struct RunRequestInput<'a> {
    /// The loop the run tests: its signals, its bound parameters and its experiment seed.
    pub loop_definition: &'a LoopDefinition,
    /// The model the case runs on: the selected model, or a declared scenario's own model (C8). The channels of a scenario effect are
    /// counted on it (`scenario_effect_tables`).
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
        initial_values: initial_values(input.loop_definition, plan, case),
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

/// The unit that the loop declares for a symbol: one of its signals, its auxiliary signals, its bound parameters or a schedule's active
/// gains (C10). `None` when the loop declares no such symbol.
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
    // A switching controller's declared states, its output latch among them, are observed in their own units (C3).
    let state_unit = || {
        loop_definition
            .controller
            .on_off
            .as_ref()
            .and_then(|block| {
                block
                    .initialization
                    .states()
                    .iter()
                    .find(|state| state.symbol == symbol)
            })
            .map(|state| state.unit.clone())
    };
    signal_unit
        .or_else(auxiliary_unit)
        .or_else(parameter_unit)
        .or_else(state_unit)
        .or_else(|| gain_schedule_case::active_gain_unit(loop_definition, symbol))
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

/// The values the run starts from: the plan's, with the case's own initial overrides written over them (N8.1), and a schedule's
/// scheduling variable at the loop's own operating point (C10).
fn initial_values(
    loop_definition: &LoopDefinition,
    plan: &VerificationPlan,
    case: &CampaignCase,
) -> BTreeMap<String, f64> {
    let mut values = plan.initial_values.clone();
    for (symbol, value) in &case.initial_overrides {
        values.insert(symbol.clone(), *value);
    }
    // A schedule's run starts at its own operating point, written last so that no other value starts it elsewhere (C10 Validate).
    if let Some((symbol, value)) = gain_schedule_case::scheduled_start(loop_definition) {
        values.insert(symbol, value);
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
    let planned = plan
        .observed_symbols
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
        .collect::<Result<Vec<_>, _>>()?;
    // A Smith predictor's flags and correction are observed beside the plan's channels (C9).
    Ok(planned
        .into_iter()
        .chain(smith_case::observations(loop_definition))
        .collect())
}

/// The parameter overrides of a run (B5.3): the current values of the record for the current settings, and the candidate's
/// record for the candidate, each record field that the controller maps.
fn parameter_overrides(
    loop_definition: &LoopDefinition,
    settings: RunSettings<'_>,
) -> Result<Vec<IsolatedParameterOverride>, Failure> {
    match settings {
        // A switching controller's three settings are written by symbol, for the running values and the candidate's (C3).
        RunSettings::Current(observed) if loop_definition.controller.on_off.is_some() => {
            on_off_case::overrides(loop_definition, observed)
        }
        RunSettings::Candidate(LawCandidate::OnOff(settings)) => {
            on_off_case::overrides(loop_definition, &settings.values)
        }
        // A PID group writes each member's gains and the decoupler's entries by symbol (C7); it has no single record to map.
        RunSettings::Candidate(LawCandidate::Group(group)) => {
            group_case::overrides(loop_definition, group)
        }
        // A state or search law's current settings are its coefficients as the controller holds them (C4, C5).
        RunSettings::Current(observed) => match law_symbols(loop_definition)? {
            Some(symbols) => {
                let coefficients = symbols
                    .iter()
                    .map(|symbol| {
                        observed
                            .get(symbol)
                            .map(|value| (symbol.clone(), *value))
                            .ok_or_else(|| {
                                Failure::new(
                                    &texts::current_setting_unread(symbol),
                                    Cause::Runtime,
                                    NextAction::ConnectRuntime,
                                )
                            })
                    })
                    .collect::<Result<BTreeMap<String, f64>, Failure>>()?;
                coefficient_overrides(loop_definition, &coefficients)
            }
            None => record_overrides(loop_definition, |mapped| {
                observed.get(&mapped.symbol).copied().ok_or_else(|| {
                    Failure::new(
                        &texts::current_setting_unread(&mapped.symbol),
                        Cause::Runtime,
                        NextAction::ConnectRuntime,
                    )
                })
            }),
        },
        RunSettings::Candidate(candidate) => {
            let mut overrides = match law_coefficients(loop_definition, candidate)? {
                Some(coefficients) => coefficient_overrides(loop_definition, &coefficients)?,
                None => candidate_record_overrides(loop_definition, candidate)?,
            };
            overrides.extend(feedforward_overrides(loop_definition, candidate)?);
            overrides.extend(smith_case::overrides(loop_definition, candidate)?);
            Ok(overrides)
        }
    }
}

/// The parameter symbols that a state or search law writes in its current settings (C4, C5): the coefficients of the state law, and every
/// bound parameter of a search law, since its trials write them all. `None` for any other law, whose settings are its record fields.
fn law_symbols(loop_definition: &LoopDefinition) -> Result<Option<Vec<String>>, Failure> {
    if loop_definition.controller.state_feedback.is_some() {
        let law = decode_state_law(loop_definition)?;
        return Ok(Some(state_coefficient_symbols(&law)));
    }
    if search_law(loop_definition).is_none() {
        return Ok(None);
    }
    let binding = loop_definition.parameter_binding.as_ref().ok_or_else(|| {
        Failure::new(
            &texts::parameter_or_reference_binding_missing(),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        )
    })?;
    Ok(Some(
        binding
            .parameters
            .iter()
            .map(|parameter| parameter.symbol.clone())
            .collect(),
    ))
}

/// The coefficients that a candidate writes by symbol (C4, C5): a state law's design, and a search law's chosen trial, which must hold
/// every bound parameter. `None` for a candidate whose record fields are its own (C0). Apply writes the same values through this rule.
///
/// # Errors
/// A search law's chosen trial holds no value for a bound parameter, or the loop declares no parameter binding.
pub fn law_coefficients(
    loop_definition: &LoopDefinition,
    candidate: &LawCandidate,
) -> Result<Option<BTreeMap<String, f64>>, Failure> {
    match candidate {
        LawCandidate::State(design) => {
            let law = decode_state_law(loop_definition)?;
            if let Some(symbol) = state_coefficient_symbols(&law)
                .into_iter()
                .find(|symbol| !design.candidate.parameters.contains_key(symbol))
            {
                return Err(Failure::new(
                    &format!("The state design holds no value for {symbol}, a coefficient of the law (C4); Tune again."),
                    Cause::Implementation,
                    NextAction::Tune,
                ));
            }
            Ok(Some(design.candidate.parameters.clone()))
        }
        LawCandidate::Search(search) => Ok(Some(
            candidate_coefficients(loop_definition, search)?.clone(),
        )),
        // A PID group's write set is its values by symbol: every member's gains and each decoupler entry (C7). Apply and Validate write
        // them through the coefficient path, which checks each symbol against the loop's binding.
        LawCandidate::Group(group) => Ok(Some(group.values.clone())),
        LawCandidate::Pid(_) | LawCandidate::Project(_) | LawCandidate::OnOff(_) => Ok(None),
    }
}

/// The overrides of a set of coefficients, each typed, bounded and unit-checked as the binding declares it (E8.4, C4, C5).
fn coefficient_overrides(
    loop_definition: &LoopDefinition,
    coefficients: &BTreeMap<String, f64>,
) -> Result<Vec<IsolatedParameterOverride>, Failure> {
    let binding = loop_definition.parameter_binding.as_ref().ok_or_else(|| {
        Failure::new(
            &texts::parameter_or_reference_binding_missing(),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        )
    })?;
    coefficients
        .iter()
        .map(|(symbol, value)| override_of(binding, symbol, *value))
        .collect()
}

/// The refusal of a trajectory candidate that lacks one of its feedforward gains (C11).
const TRAJECTORY_VALUE_MISSING: &str =
    "A trajectory candidate holds no value for a feedforward gain.";
/// The refusal of a measured-disturbance candidate that lacks one of its feedforward values (C8).
const FEEDFORWARD_VALUE_MISSING: &str =
    "A feedforward candidate holds no value for one of its feedforward variables.";

/// The feedforward overrides of a feedforward-law candidate (C8, C11): a trajectory's rate and acceleration gains, or a measured-disturbance
/// feedforward's gain with its lead and lag when its seed names them. Each is written to the variable its seed names, with the type, unit,
/// convention and bounds that the binding declares. A candidate of another law has none.
///
/// # Errors
/// The candidate holds no value for one of those variables, or the binding has no parameter of that variable.
fn feedforward_overrides(
    loop_definition: &LoopDefinition,
    candidate: &LawCandidate,
) -> Result<Vec<IsolatedParameterOverride>, Failure> {
    let LawCandidate::Project(project) = candidate else {
        return Ok(Vec::new());
    };
    let symbols = feedforward_symbols(loop_definition);
    if symbols.is_empty() {
        return Ok(Vec::new());
    }
    let missing = if served_trajectory(loop_definition).is_some() {
        TRAJECTORY_VALUE_MISSING
    } else {
        FEEDFORWARD_VALUE_MISSING
    };
    let Some(binding) = &loop_definition.parameter_binding else {
        return Err(Failure::new(
            &texts::parameter_or_reference_binding_missing(),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ));
    };
    symbols
        .into_iter()
        .map(|symbol| -> Result<IsolatedParameterOverride, Failure> {
            let value =
                project.values.get(symbol).copied().ok_or_else(|| {
                    Failure::new(missing, Cause::Implementation, NextAction::None)
                })?;
            override_of(binding, symbol, value)
        })
        .collect()
}

/// The variables a feedforward-law candidate writes beyond its PI (C8, C11): a trajectory's rate and acceleration gains, or a
/// measured-disturbance feedforward's gain and, when its seed names them, its lead and lag. A loop with neither has none.
fn feedforward_symbols(loop_definition: &LoopDefinition) -> Vec<&str> {
    if let Some(seed) = served_trajectory(loop_definition) {
        return vec![seed.rate_symbol.as_str(), seed.acceleration_symbol.as_str()];
    }
    let Some(seed) = served_feedforward(loop_definition) else {
        return Vec::new();
    };
    let mut symbols = vec![seed.gain_symbol.as_str()];
    if let Some(lead_lag) = &seed.lead_lag {
        symbols.push(lead_lag.lead_symbol.as_str());
        symbols.push(lead_lag.lag_symbol.as_str());
    }
    symbols
}

/// The parameter overrides of a candidate that is not a set of coefficients (C0, C10). A schedule's candidate writes the gains of every point
/// of its table (C10 Validate), and any other candidate writes each record field that the controller maps.
fn candidate_record_overrides(
    loop_definition: &LoopDefinition,
    candidate: &LawCandidate,
) -> Result<Vec<IsolatedParameterOverride>, Failure> {
    match (
        candidate,
        gain_schedule_case::schedule_of_loop(loop_definition),
    ) {
        (LawCandidate::Project(project), Some(schedule)) => {
            let gains = gains_of_table(&schedule.points, &project.values)?;
            gain_schedule_case::run_overrides(loop_definition, schedule, &gains)
        }
        _ => record_overrides(loop_definition, |mapped| candidate_value(candidate, mapped)),
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
            override_of(binding, &mapped.symbol, value_of(mapped)?)
        })
        .collect()
}

/// One variable of a binding as a parameter override: its type, unit, convention and bounds as the binding declares them, with the value
/// written (E8.4).
///
/// # Errors
/// The binding holds no parameter of that symbol.
pub(super) fn override_of(
    binding: &ParameterBinding,
    symbol: &str,
    value: f64,
) -> Result<IsolatedParameterOverride, Failure> {
    let parameter = binding
        .parameters
        .iter()
        .find(|parameter| parameter.symbol == symbol)
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
        value,
    })
}

/// The value of one record field of the candidate, by the gain key that names it (C0): the gains, the setpoint weights, the
/// back-calculation gain `1/Tt` and the measurement filter time.
fn candidate_value(candidate: &LawCandidate, mapped: &MappedParameter) -> Result<f64, Failure> {
    match candidate {
        LawCandidate::Pid(candidate) => pid_value(candidate, mapped.key),
        // A switching controller writes its settings by symbol, never through a gain key (C3): its overrides are not built here.
        LawCandidate::OnOff(_) => Err(Failure::new(
            "A switching controller holds no gain of this kind.",
            Cause::Implementation,
            NextAction::None,
        )),
        // A PID group's values are written by symbol, never through one gain key (C7): its overrides are built by `group_case`.
        LawCandidate::Group(_) => Err(Failure::new(
            "A PID group holds no single gain of this kind.",
            Cause::Implementation,
            NextAction::None,
        )),
        // A program's own law writes each of its variables from its gains, with zero for a term the structure does not use (C2). A
        // variable that the candidate holds by its own symbol is written as it is: a cascade's outer PI is one of those (C6).
        LawCandidate::Project(candidate) => {
            match (candidate.values.get(&mapped.symbol), mapped.key) {
                (Some(value), _) => Ok(*value),
                (None, GainKey::ProportionalGain) => Ok(candidate.gains.proportional_gain),
                (None, GainKey::IntegralGain) => Ok(candidate.gains.integral_gain),
                (None, GainKey::DerivativeGain) => Ok(candidate.gains.derivative_gain),
                (None, _) => Err(Failure::new(
                    "A program's own law holds no gain of this kind.",
                    Cause::Implementation,
                    NextAction::None,
                )),
            }
        }
        // A state or search law writes its coefficients by symbol (`law_coefficients`), never through a gain key (C4, C5).
        _ => Err(Failure::new(
            "A state or search law holds no record field of this kind.",
            Cause::Implementation,
            NextAction::None,
        )),
    }
}

/// The value of one record field of a `Control.PID` candidate, by the gain key that names it (C0).
fn pid_value(candidate: &ControlPidCandidate, key: GainKey) -> Result<f64, Failure> {
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

#[cfg(test)]
#[path = "run_requests_feedforward_tests.rs"]
mod feedforward_tests;

#[cfg(test)]
#[path = "run_requests_feedforward_c8_tests.rs"]
mod feedforward_c8_tests;

#[cfg(test)]
#[path = "run_requests_law_tests.rs"]
mod law_tests;

#[cfg(test)]
#[path = "run_requests_schedule_tests.rs"]
mod schedule_tests;
