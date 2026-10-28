//! The loop definition of a P, PI or PID law that the program computes itself (specification 35, C2, B1.4 and §17.1): a project-owned
//! controller whose `parallel-pid` seed names the gain variables. Ported from `ownLawLoopGenerator.ts`. The Tune families and the seed
//! design are V7 and not here; the project block is written as the TypeScript writes it, for V7 to decode.

#![allow(clippy::module_name_repetitions)] // `OwnLawLoopDraft` names the module's subject, as the TypeScript does.

use std::collections::BTreeSet;

use serde::{Deserialize, Deserializer};
use serde_json::{json, Value};

use super::catalog::{CompiledCatalog, CompiledController, CompiledSymbol, CompiledTask};
use super::loop_bindings::{
    binding, compiled, cyclic_task, explicit_experiment, form_number, loop_id, manual_output_type,
    measured_signal, measurement_binding, real_type, required_choice, state_type, stated_signal,
    with_revision, FormSignal, MeasuredSignal,
};
use super::pid_bindings::{startup_flag_is_bool, startup_reason, startup_state};
use super::pid_loop::{required_mode, setpoint_signal, ModeChoice, SignalChoice};
use super::texts::generators as texts;
use super::texts::save;
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    Action, Controller, ControllerProject, Design, Direction, Execution, ExperimentKind,
    InitialSource, InitialValue, InitializedState, Limits, LoopDefinition, LoopScope, ModeBinding,
    ModeWhenTrue, Parameter, ParameterBinding, PostScan, SampleTiming, Signals, Topology,
    LOOP_SCHEMA_VERSION,
};

/// The law a program computes itself: proportional, proportional and integral, or all three parts (C2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum OwnLawFamily {
    /// Proportional only; the JSON name is `P`.
    P,
    /// Proportional and integral; the JSON name is `PI`.
    #[serde(rename = "PI")]
    Pi,
    /// Proportional, integral and derivative; the JSON name is `PID`.
    #[serde(rename = "PID")]
    Pid,
}

/// Where a PID law takes its derivative: from the error, or from the measurement (C2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DerivativeOn {
    /// The derivative of the error.
    Error,
    /// The derivative of the measurement, with the sign the law gives it (`D = −Kd·dy/dt`).
    Measurement,
}

/// A variable that holds one gain of the program's law, with the range Tune may choose from (`OwnLawGain`). The form sends each bound as
/// the text typed (`form_number`); a blank bound, or one that no number reads, is NaN: `checked_gain` refuses it under its field before
/// any bound is used.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnLawGain {
    /// The variable's path; blank when none is chosen.
    pub symbol: String,
    /// The smallest value Tune may choose.
    #[serde(default = "missing_bound", deserialize_with = "bound_or_missing")]
    pub minimum: f64,
    /// The largest value Tune may choose.
    #[serde(default = "missing_bound", deserialize_with = "bound_or_missing")]
    pub maximum: f64,
}

/// A bound the form did not send: NaN, which `checked_gain` refuses (see `OwnLawGain`).
fn missing_bound() -> f64 {
    f64::NAN
}

/// Reads a bound as the form's text gives it (`form_number`): its number, or NaN when it is blank or no number reads it (see
/// `OwnLawGain`).
fn bound_or_missing<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    Ok(form_number(deserializer)?.unwrap_or(f64::NAN))
}

/// A program's own law as the engineer confirmed it (`OwnLawLoopDraft`, C2). Setup asks for no experiment step or duration.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnLawLoopDraft {
    /// The loop's name, from which its id is made.
    pub name: String,
    /// Whether the output rises (`direct`) or falls (`reverse`) while the measurement is below the setpoint.
    pub action: Action,
    /// The law the program computes.
    pub law: OwnLawFamily,
    /// The compiled program or function block instance that computes the law.
    pub instance: String,
    /// The measurement.
    pub process_variable: FormSignal,
    /// The controller output.
    pub manipulated_variable: FormSignal,
    /// The setpoint.
    pub setpoint: SignalChoice,
    /// The Auto/Manual signal.
    pub mode: ModeChoice,
    /// The manual output.
    pub manual_output: String,
    /// The variable that holds the proportional gain, with its range.
    pub proportional_gain: OwnLawGain,
    /// The variable that holds the integral gain, for PI and PID.
    pub integral_gain: Option<OwnLawGain>,
    /// The variable that holds the integral term, in the output's unit, for PI and PID.
    pub integral_term: Option<String>,
    /// The variable that holds the derivative gain, for PID.
    pub derivative_gain: Option<OwnLawGain>,
    /// The variable that holds the previous error or measurement, for PID.
    pub derivative_state: Option<String>,
    /// Whether the derivative is taken from the error or from the measurement, for PID.
    pub derivative_on: Option<DerivativeOn>,
    /// The identification experiment's output step; Identify chooses it when absent.
    pub experiment_step: Option<f64>,
    /// How long identification runs, in s; Identify chooses it when absent.
    pub experiment_duration_seconds: Option<f64>,
    /// The BOOL the program sets when its startup code has run.
    pub startup_flag: Option<String>,
}

/// The loop definition of a program's own law (`ownLawLoopDefinition`): a project-owned controller whose seed names the gain
/// variables, with the integral term and the derivative's previous value started as Validate starts them (C2).
///
/// # Errors
/// The instance is not a compiled program or function block, or does not run in exactly one cyclic task; the law lacks its integral
/// gain or term, or its derivative gain or previous value; two gains or the integral term share a variable; a gain's range is not an
/// interval; a variable is not `REAL` or `LREAL`; a signal is not compiled or is a measurement the program computes; or the program
/// writes a seeded value and the draft names no startup-done flag.
pub fn own_law_loop_definition(
    catalog: &CompiledCatalog,
    scope: &LoopScope,
    draft: &OwnLawLoopDraft,
) -> Result<LoopDefinition, Failure> {
    let checked = check_choices(catalog, draft)?;
    let (program, task) = own_law_program(catalog, draft)?;
    let parts = checked.parts;
    check_variables(draft, parts)?;
    let process_variable = stated_signal(&checked.process_variable);
    let manipulated_variable = stated_signal(&checked.manipulated_variable);
    let setpoint = setpoint_signal(&draft.setpoint, &process_variable);
    let manual = compiled(catalog, &draft.manual_output)?;
    let mut states = integral_state(
        catalog,
        parts.integral,
        &draft.proportional_gain.symbol,
        &manipulated_variable,
    )?;
    states.extend(derivative_state(
        catalog,
        parts.derivative,
        &process_variable,
    )?);
    // The startup-done flag is an application state (C1): it accompanies the law's states, and a program's P law may declare it.
    let startup = draft
        .startup_flag
        .as_deref()
        .filter(|flag| !flag.is_empty());
    let application: Vec<InitializedState> = startup.map(startup_state).into_iter().collect();
    let controller = own_law_controller(
        draft,
        program,
        manual,
        &states,
        &application,
        &parallel_seed(draft, parts),
        task.interval_ms,
    )?;
    let signals = own_law_signals(catalog, &setpoint, &process_variable, &manipulated_variable)?;
    let experiment = explicit_experiment(
        &draft.name,
        Execution::ControlStudio,
        ExperimentKind::MvStep,
        draft.experiment_step,
        draft.experiment_duration_seconds,
    )?;
    check_own_law_startup(catalog, draft, parts)?;
    let parameters = gain_parameters(
        catalog,
        draft,
        parts,
        &process_variable,
        &manipulated_variable,
    )?;

    let id = loop_id(&draft.name);
    Ok(with_revision(LoopDefinition {
        schema_version: LOOP_SCHEMA_VERSION,
        id: id.clone(),
        name: draft.name.clone(),
        revision: "sha256:pending".to_string(),
        project_id: scope.project_id.clone(),
        runtime_id: scope.runtime_id.clone(),
        controller,
        signals,
        limits: Limits {
            output_minimum: Some(manipulated_variable.minimum),
            output_maximum: Some(manipulated_variable.maximum),
            rate_up_per_s: None,
            rate_down_per_s: None,
            expected_settling_seconds: None,
        },
        observed_sample_timing: SampleTiming {
            nominal_period_ms: task.interval_ms,
            maximum_jitter_ms: None,
        },
        parameter_binding: Some(ParameterBinding {
            loop_id: id,
            task_name: task.name.clone(),
            mode: ModeBinding {
                symbol: draft.mode.symbol.clone(),
                manual_value: draft.mode.when_true == ModeWhenTrue::Manual,
            },
            parameters,
        }),
        parameter_purpose: None,
        topology: Some(Topology::ClosedLoop),
        experiment,
        plant_id: None,
        operating_point: None,
        identification_policy: None,
        verification_scenarios: None,
        channels: None,
        analysis: None,
        analysis_parameter_symbols: None,
        cascade: None,
    }))
}

/// The compiled program that computes the law and its one cyclic task (`ownLawLoopDefinition`, the first checks).
///
/// # Errors
/// No compiled program or function block has the instance, or it does not run in exactly one cyclic task.
fn own_law_program<'a>(
    catalog: &'a CompiledCatalog,
    draft: &OwnLawLoopDraft,
) -> Result<(&'a CompiledController, &'a CompiledTask), Failure> {
    let program = catalog
        .controllers
        .iter()
        .find(|entry| entry.instance == draft.instance)
        .ok_or_else(|| {
            Failure::new(
                &texts::not_compiled_program(&draft.instance),
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            )
        })?;
    Ok((program, cyclic_task(program)?))
}

/// The gain variables a law of the draft has: the integral gain and the variable of its integral term; the derivative gain, the
/// variable of its previous value and where that is taken from (C2).
#[derive(Debug, Clone, Copy)]
struct Parts<'a> {
    integral: Option<(&'a OwnLawGain, &'a str)>,
    derivative: Option<(&'a OwnLawGain, &'a str, DerivativeOn)>,
}

/// The checked questions of an own-law draft: the measurement and the output as signals, and the gain variables the law has.
struct Checked<'a> {
    process_variable: MeasuredSignal,
    manipulated_variable: MeasuredSignal,
    parts: Parts<'a>,
}

/// The questions of an own-law setup, checked in the order the form asks them (`ownLawDraft`, `loopSetupForm.ts`): the name, the instance
/// that computes the law, the measurement, the setpoint, the output, the Auto/Manual signal, the manual output, and the gains and terms the
/// law uses (`law_parts`).
///
/// # Errors
/// A question is blank, a signal is invalid (`measured_signal`), or the law lacks a gain or term it needs.
fn check_choices<'a>(
    catalog: &CompiledCatalog,
    draft: &'a OwnLawLoopDraft,
) -> Result<Checked<'a>, Failure> {
    required_choice(&draft.name, save::name_needed(), "name")?;
    required_choice(
        &draft.instance,
        texts::law_instance_needed(),
        "own.instance",
    )?;
    let process_variable = measured_signal(
        &draft.process_variable,
        "measurement",
        Some("own.processVariable"),
    )?;
    required_choice(
        &draft.setpoint.symbol,
        save::setpoint_needed(),
        "own.setpoint",
    )?;
    let manipulated_variable = measured_signal(
        &draft.manipulated_variable,
        "output",
        Some("own.manipulatedVariable"),
    )?;
    required_mode(catalog, &draft.mode.symbol, "own.mode")?;
    required_choice(
        &draft.manual_output,
        save::manual_output_needed(),
        "own.manualOutput",
    )?;
    let parts = law_parts(draft)?;
    Ok(Checked {
        process_variable,
        manipulated_variable,
        parts,
    })
}

/// The parts the law needs, with their gains checked: the proportional gain, then the integral gain and term for PI and PID, then the
/// derivative gain, the variable of its previous value and where that is taken from for PID (`ownLawLoopDefinition`, the first checks).
///
/// # Errors
/// A gain's variable is blank, its range is not an interval, or the law lacks the integral or derivative part it needs.
fn law_parts(draft: &OwnLawLoopDraft) -> Result<Parts<'_>, Failure> {
    let law = family_name(draft.law);
    checked_gain(
        &draft.proportional_gain,
        "proportional gain",
        "own.proportionalGain",
    )?;
    let integral = if draft.law == OwnLawFamily::P {
        None
    } else {
        let gain = draft.integral_gain.as_ref().ok_or_else(|| {
            refuse_settings(&texts::integral_gain_needed(law)).with_field("own.integralGain")
        })?;
        checked_gain(gain, "integral gain", "own.integralGain")?;
        let term = named(draft.integral_term.as_deref()).ok_or_else(|| {
            refuse_settings(&texts::integral_term_needed(law)).with_field("own.integralTerm")
        })?;
        Some((gain, term))
    };
    let derivative = if draft.law == OwnLawFamily::Pid {
        let gain = draft.derivative_gain.as_ref().ok_or_else(|| {
            refuse_settings(texts::derivative_gain_needed()).with_field("own.derivativeGain")
        })?;
        checked_gain(gain, "derivative gain", "own.derivativeGain")?;
        let state = named(draft.derivative_state.as_deref()).ok_or_else(|| {
            refuse_settings(texts::derivative_state_needed()).with_field("own.derivativeState")
        })?;
        let on = draft.derivative_on.ok_or_else(|| {
            refuse_settings(texts::derivative_on_needed()).with_field("own.derivativeOn")
        })?;
        Some((gain, state, on))
    } else {
        None
    };
    Ok(Parts {
        integral,
        derivative,
    })
}

/// A gain variable the form asks for (`gainValue`, `loopSetupForm.ts`): a variable, and a range whose bounds are numbers with the minimum
/// below the maximum. The first missing item is the one refused, under its field (`field`, `field.minimum`, `field.maximum`).
///
/// # Errors
/// The variable is blank, a bound is not a number, or the minimum is not below the maximum.
pub(super) fn checked_gain(gain: &OwnLawGain, label: &str, field: &str) -> Result<(), Failure> {
    required_choice(&gain.symbol, &texts::gain_symbol_needed(label), field)?;
    if !gain.minimum.is_finite() {
        return Err(refuse_settings(&save::signal_range_needed(label))
            .with_field(&format!("{field}.minimum")));
    }
    if !gain.maximum.is_finite() {
        return Err(refuse_settings(&save::signal_range_needed(label))
            .with_field(&format!("{field}.maximum")));
    }
    if gain.maximum <= gain.minimum {
        return Err(refuse_settings(&texts::gain_range(&gain.symbol))
            .with_field(&format!("{field}.maximum")));
    }
    Ok(())
}

/// The gains, the integral term and the derivative's previous value are distinct variables (`ownLawLoopDefinition`). The gains' ranges are
/// checked by `checked_gain` before this runs.
fn check_variables(draft: &OwnLawLoopDraft, parts: Parts<'_>) -> Result<(), Failure> {
    let mut variables = vec![draft.proportional_gain.symbol.as_str()];
    if let Some((gain, term)) = parts.integral {
        variables.push(gain.symbol.as_str());
        variables.push(term);
    }
    if let Some((gain, state, _)) = parts.derivative {
        variables.push(gain.symbol.as_str());
        variables.push(state);
    }
    let distinct: BTreeSet<String> = variables
        .iter()
        .map(|symbol| symbol.to_uppercase())
        .collect();
    if distinct.len() != variables.len() {
        return Err(refuse_settings(texts::distinct_variables()));
    }
    Ok(())
}

/// The integral term starts at the output of the switch to Auto, with the setpoint at the measurement, and is bounded about the
/// output's range (C2 Validate; `integralState`).
fn integral_state(
    catalog: &CompiledCatalog,
    integral: Option<(&OwnLawGain, &str)>,
    proportional_symbol: &str,
    manipulated_variable: &MeasuredSignal,
) -> Result<Vec<InitializedState>, Failure> {
    let Some((_, term)) = integral else {
        return Ok(Vec::new());
    };
    let span = manipulated_variable.maximum - manipulated_variable.minimum;
    Ok(vec![InitializedState {
        symbol: term.to_string(),
        iec_type: state_type(real_type(compiled(catalog, term)?)?),
        unit: manipulated_variable.unit.clone(),
        minimum: manipulated_variable.minimum - 10.0 * span,
        maximum: manipulated_variable.maximum + 10.0 * span,
        initial: InitialValue::From(InitialSource::PidIntegral {
            index: 0,
            input_index: 0,
            gain_symbol: proportional_symbol.to_string(),
            weight_symbol: None,
            weight: None,
        }),
        post_scan: PostScan::Bounded,
    }])
}

/// The previous value the derivative is taken from. It starts at the error of the switch, which is zero with the setpoint at the
/// measurement, or at the measurement, so that the derivative starts at zero (C2 Validate; `derivativeState`).
fn derivative_state(
    catalog: &CompiledCatalog,
    derivative: Option<(&OwnLawGain, &str, DerivativeOn)>,
    process_variable: &MeasuredSignal,
) -> Result<Vec<InitializedState>, Failure> {
    let Some((_, state, on)) = derivative else {
        return Ok(Vec::new());
    };
    let span = process_variable.maximum - process_variable.minimum;
    let (minimum, maximum, initial) = match on {
        DerivativeOn::Error => (
            -span,
            span,
            InitialValue::From(InitialSource::WeightedError {
                index: 0,
                weight_symbol: None,
                weight: Some(1.0),
            }),
        ),
        DerivativeOn::Measurement => (
            process_variable.minimum,
            process_variable.maximum,
            InitialValue::From(InitialSource::ModelOutput { index: 0 }),
        ),
    };
    Ok(vec![InitializedState {
        symbol: state.to_string(),
        iec_type: state_type(real_type(compiled(catalog, state)?)?),
        unit: process_variable.unit.clone(),
        minimum,
        maximum,
        initial,
        post_scan: PostScan::Bounded,
    }])
}

/// The signals of a program's own law (B1.4): the setpoint and the output are the program's variables, the measurement is read.
///
/// # Errors
/// A signal is not compiled, is not `REAL` or `LREAL`, or is a measurement the program computes.
fn own_law_signals(
    catalog: &CompiledCatalog,
    setpoint: &MeasuredSignal,
    process_variable: &MeasuredSignal,
    manipulated_variable: &MeasuredSignal,
) -> Result<Signals, Failure> {
    Ok(Signals {
        setpoint: Some(binding(catalog, setpoint, Direction::Read)?),
        process_variable: measurement_binding(catalog, process_variable)?,
        manipulated_variable: binding(catalog, manipulated_variable, Direction::Write)?,
        actuator: None,
        additional: None,
    })
}

/// The seed that names the gain variables of the law, with the integral and derivative gains only when the law has them.
fn parallel_seed(draft: &OwnLawLoopDraft, parts: Parts<'_>) -> Value {
    let mut seed = json!({
        "kind": "parallel-pid",
        "family": family_name(draft.law),
        "proportionalSymbol": draft.proportional_gain.symbol,
    });
    if let Some(object) = seed.as_object_mut() {
        if let Some((gain, _)) = parts.integral {
            object.insert("integralSymbol".to_string(), json!(gain.symbol));
        }
        if let Some((gain, _, _)) = parts.derivative {
            object.insert("derivativeSymbol".to_string(), json!(gain.symbol));
        }
    }
    seed
}

/// The controller of a program's own law: a project-owned controller with its states and its seed (C2; B1.4).
///
/// # Errors
/// The manual output is not `REAL` or `LREAL`.
fn own_law_controller(
    draft: &OwnLawLoopDraft,
    program: &CompiledController,
    manual: &CompiledSymbol,
    states: &[InitializedState],
    application: &[InitializedState],
    seed: &Value,
    period_ms: f64,
) -> Result<Controller, Failure> {
    Ok(Controller {
        instance: draft.instance.clone(),
        type_name: program.type_name.clone(),
        revision: "sha256:unverified".to_string(),
        action: draft.action,
        task_period_ms: period_ms,
        design: Some(Design::Project),
        law: None,
        mode_signal: Some(draft.mode.symbol.clone()),
        mode_when_true: Some(draft.mode.when_true),
        tracking_signal: None,
        manual_output_signal: Some(manual.symbol.clone()),
        manual_output_address: manual.address.clone().filter(|address| !address.is_empty()),
        manual_output_type: Some(manual_output_type(real_type(manual)?)),
        manual_output_follows_output: None,
        pid_initialization: None,
        state_feedback: None,
        on_off: None,
        project: Some(
            ControllerProject::from_value(&project_block(
                family_name(draft.law),
                seed,
                states,
                application,
            ))
            .map_err(|message| refuse_settings(&message))?,
        ),
        group: None,
    })
}

/// The program's own law as `controller.project` holds it (C2): its family, its states, its seed, and the requirement that Tune ranks
/// its candidates by. V7 decodes this value.
fn project_block(
    family: &str,
    seed: &Value,
    states: &[InitializedState],
    application: &[InitializedState],
) -> Value {
    // These records always serialize; the default (null) could only be refused by the loop contract, never written silently.
    let states = serde_json::to_value(states).unwrap_or_default();
    let mut initialization = json!({ "kind": "declared-state-v1", "states": states });
    if !application.is_empty() {
        initialization["applicationStates"] = serde_json::to_value(application).unwrap_or_default();
    }
    json!({
        "family": family,
        "convention": "project-owned-native-controller",
        "initialization": initialization,
        "seed": seed,
        // One controlled output: its verification campaign names the error requirement without a channel prefix.
        "axes": [],
        "objective": { "requirementId": "target-error", "direction": "minimize" },
        "maximumCandidates": 4,
        "maximumCycles": 50_000,
        "wallBudgetMs": 60_000,
    })
}

/// The gains as parameters of the record: each gain's type is its variable's own `REAL` or `LREAL`, and its unit is the output's per
/// the process unit, or the integral and derivative forms of it (C0; `gain` in `ownLawLoopDefinition`).
///
/// # Errors
/// A gain is not compiled, or its variable is not `REAL` or `LREAL`.
fn gain_parameters(
    catalog: &CompiledCatalog,
    draft: &OwnLawLoopDraft,
    parts: Parts<'_>,
    process_variable: &MeasuredSignal,
    manipulated_variable: &MeasuredSignal,
) -> Result<Vec<Parameter>, Failure> {
    let output = manipulated_variable.unit.as_str();
    let process = process_variable.unit.as_str();
    let mut parameters = vec![gain_parameter(
        catalog,
        &draft.proportional_gain,
        format!("{output}/{process}"),
        "parallel proportional gain",
    )?];
    if let Some((gain, _)) = parts.integral {
        parameters.push(gain_parameter(
            catalog,
            gain,
            format!("{output}/({process}*s)"),
            "parallel integral gain",
        )?);
    }
    if let Some((gain, _, _)) = parts.derivative {
        parameters.push(gain_parameter(
            catalog,
            gain,
            format!("{output}*s/{process}"),
            "parallel derivative gain",
        )?);
    }
    Ok(parameters)
}

/// One gain as a parameter: its variable's real type, its unit, its convention and its range.
///
/// # Errors
/// The gain's variable is not compiled, or it is not `REAL` or `LREAL`.
fn gain_parameter(
    catalog: &CompiledCatalog,
    gain: &OwnLawGain,
    unit: String,
    convention: &str,
) -> Result<Parameter, Failure> {
    Ok(Parameter {
        symbol: gain.symbol.clone(),
        iec_type: real_type(compiled(catalog, &gain.symbol)?)?,
        unit,
        convention: convention.to_string(),
        minimum: gain.minimum,
        maximum: gain.maximum,
    })
}

/// A law whose program writes a seeded value needs the startup-done flag (C1 Setup). The integral term and the previous value are the
/// program's own running state, so their writes do not. A draft that names a flag must name a compiled `BOOL`.
///
/// # Errors
/// The program needs the flag, and the draft names none.
fn check_own_law_startup(
    catalog: &CompiledCatalog,
    draft: &OwnLawLoopDraft,
    parts: Parts<'_>,
) -> Result<(), Failure> {
    if let Some(flag) = draft
        .startup_flag
        .as_deref()
        .filter(|flag| !flag.is_empty())
    {
        return startup_flag_is_bool(catalog, flag);
    }
    let seeded = seeded_symbols(draft, parts);
    let no_records: Vec<String> = Vec::new();
    match startup_reason(catalog, None, &seeded, &no_records) {
        Some(reason) => Err(refuse_settings_with(
            &texts::startup_flag_needed(&reason),
            NextAction::OpenLoopSetup,
        )
        .with_field("startupFlag")),
        None => Ok(()),
    }
}

/// The symbols validation seeds for a program's own law (C2 Setup): the setpoint, the gains the law has, the manual output and the
/// mode. The integral term and the previous value are the program's own running state, not seeded (`seededWrite`).
fn seeded_symbols(draft: &OwnLawLoopDraft, parts: Parts<'_>) -> Vec<String> {
    let mut seeded = vec![
        draft.setpoint.symbol.clone(),
        draft.proportional_gain.symbol.clone(),
    ];
    seeded.extend(parts.integral.map(|(gain, _)| gain.symbol.clone()));
    seeded.extend(parts.derivative.map(|(gain, _, _)| gain.symbol.clone()));
    seeded.push(draft.manual_output.clone());
    seeded.push(draft.mode.symbol.clone());
    seeded
}

/// The family's name in the loop file: `P`, `PI` or `PID`.
pub(super) fn family_name(law: OwnLawFamily) -> &'static str {
    match law {
        OwnLawFamily::P => "P",
        OwnLawFamily::Pi => "PI",
        OwnLawFamily::Pid => "PID",
    }
}

/// A variable the draft names: present and not empty, as the TypeScript's truthiness reads it.
fn named(value: Option<&str>) -> Option<&str> {
    value.filter(|name| !name.is_empty())
}

/// A settings refusal that the setup form's next action answers (A11.2).
fn refuse_settings(text: &str) -> Failure {
    refuse_settings_with(text, NextAction::OpenLoopSetup)
}

/// A settings refusal with the next action given.
fn refuse_settings_with(text: &str, next_action: NextAction) -> Failure {
    Failure::new(text, Cause::Settings, next_action)
}

#[cfg(test)]
#[path = "own_law_loop_tests.rs"]
mod tests;
