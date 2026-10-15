//! The loop definition of a measured-disturbance feedforward (specification 35, C8 and §17.12): a library `Control.PID` whose program adds
//! a feedforward from a measured disturbance to its `FeedForward` input. Ported from `feedforwardGenerator.ts`. Its feedback law is a PI,
//! whose three gains, the feedforward gain and, when the program has both, its lead and lag times are parameters of the record. The
//! disturbance is measured: the controller never writes it, and Identify steps it only through its command, which Setup checks first
//! (D-5). The startup-done flag is an application state, as every Rust generator writes it (C1, wave 2b).

#![allow(clippy::module_name_repetitions)] // `FeedforwardLoopDraft` names the module's subject, as `PidLoopDraft` does.

use std::collections::BTreeSet;

use serde::Deserialize;
use serde_json::{json, Value};

use super::catalog::{CompiledCatalog, CompiledSymbol, CompiledTask, PID_TYPE};
use super::loop_bindings::{
    binding, compiled, cyclic_task, explicit_experiment, loop_id, manual_output_type,
    measurement_binding, real_type, state_type, stated_signal, stated_unit, with_revision,
    MeasuredSignal,
};
use super::pid_bindings::{manual_output_tracking, pid_parameters, pid_states, startup_state};
use super::pid_loop::{setpoint_signal, ModeChoice, SignalChoice};
use super::texts::generators as texts;
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    Action, AuxiliaryRole, AuxiliarySignal, ChannelInput, ChannelOutput, Controller,
    ControllerProject, Design, Direction, Execution, ExperimentKind, InitialSource, InitialValue,
    InitializedState, Limits, LoopChannels, LoopDefinition, LoopScope, ModeBinding, ModeWhenTrue,
    Parameter, ParameterBinding, PostScan, SampleTiming, Signals, Topology, LOOP_SCHEMA_VERSION,
};
use crate::model::{ActuatorCapabilities, ActuatorKind};

/// The seed kind of a feedforward in the loop file (C8).
const FEEDFORWARD_KIND: &str = "feedforward-pid";
/// The family of the feedforward's project block in the loop file (C8).
const FEEDFORWARD_FAMILY: &str = "feedforward";
/// The library type of the controller the feedforward is added to (C1).
const CONTROL_PID: &str = "Control.PID";
/// The channel identifier and the signal identifier of the measured disturbance (C8).
const MEASURED_DISTURBANCE: &str = "measured-disturbance";
/// The channel identifiers of the controller output, the measurement and the setpoint, as the loop's signals name them.
const MANIPULATED_VARIABLE: &str = "manipulatedVariable";
const PROCESS_VARIABLE: &str = "processVariable";
const SETPOINT: &str = "setpoint";
/// The conventions of the three PID gains as the parameter record names them (C1).
const PID_GAIN_CONVENTIONS: [&str; 3] = [
    "Control.PID ProportionalGain",
    "Control.PID IntegralGain",
    "Control.PID DerivativeGain",
];
/// The state of the `Control.PID`'s feedforward term, whose initial value is `K_ff·d` (C8).
const FEEDFORWARD_STATE: &str = ".StateValue.FeedForward";

/// A feedforward loop as the engineer confirmed it (`FeedforwardDraft`, C8). Setup asks for no experiment step or duration, so both may be
/// absent, and Identify chooses them (B3.3, B3.4).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FeedforwardLoopDraft {
    /// The loop's name, from which its id is made.
    pub name: String,
    /// The `Control.PID` instance path.
    pub instance: String,
    /// Whether the output rises (`direct`) or falls (`reverse`) while the measurement is below the setpoint.
    pub action: Action,
    /// The record the PI is configured from.
    pub parameter_record: String,
    /// The measurement.
    pub process_variable: MeasuredSignal,
    /// The controller output.
    pub manipulated_variable: MeasuredSignal,
    /// The setpoint.
    pub setpoint: SignalChoice,
    /// The Auto/Manual signal.
    pub mode: ModeChoice,
    /// The manual output.
    pub manual_output: String,
    /// The measured disturbance the program's feedforward reads.
    pub disturbance: MeasuredSignal,
    /// The variable that Identify steps to move the disturbance: a simulation or test input the program only reads (C8, D-5).
    pub disturbance_command: String,
    /// The disturbance's identification step, in its unit: positive and at most its span.
    pub disturbance_step: f64,
    /// The feedforward's variables: the gain, and the lead, lag and filtered disturbance together or none.
    pub feedforward: FeedforwardVariables,
    /// The output's identification step; Identify chooses it when absent.
    pub experiment_step: Option<f64>,
    /// How long identification runs, in s; Identify chooses it when absent.
    pub experiment_duration_seconds: Option<f64>,
    /// The BOOL the program sets when its startup code has run.
    pub startup_flag: Option<String>,
}

/// The variables a feedforward holds (C8): the gain `K_ff`, and the lead time, the lag time and the filtered disturbance `z`, which are all
/// present or all absent.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FeedforwardVariables {
    /// The variable that holds the feedforward gain `K_ff`.
    pub gain: String,
    /// The variable that holds the lead time `T_lead`.
    pub lead: Option<String>,
    /// The variable that holds the lag time `T_lag`.
    pub lag: Option<String>,
    /// The variable that holds the disturbance filtered through the lag, `z`.
    pub filtered: Option<String>,
}

/// The loop definition of a measured-disturbance feedforward, ready for Identify, Tune, Validate and Apply (`feedforwardLoopDefinition`).
///
/// # Errors
/// The instance is not a compiled `Control.PID` in exactly one cyclic task; the lead, lag and filtered variables are not all or none; the
/// variables are not distinct; the disturbance step is not positive and at most the disturbance's span; the disturbance command is not a
/// simulation or test input (D-5); a signal, a variable or the manual output is not compiled, or is not `REAL` or `LREAL`.
pub fn feedforward_loop_definition(
    catalog: &CompiledCatalog,
    scope: &LoopScope,
    draft: &FeedforwardLoopDraft,
) -> Result<LoopDefinition, Failure> {
    let task = feedforward_task(catalog, draft)?;
    check_variables(draft)?;
    check_step(draft)?;
    let process = stated_signal(&draft.process_variable);
    let manipulated = stated_signal(&draft.manipulated_variable);
    let setpoint = setpoint_signal(&draft.setpoint, &process);
    let manual = compiled(catalog, &draft.manual_output)?;
    let command = disturbance_command(catalog, &draft.disturbance_command)?;
    let states = feedforward_states(catalog, draft, &process, &manipulated)?;
    let application: Vec<InitializedState> = draft
        .startup_flag
        .as_deref()
        .filter(|flag| !flag.is_empty())
        .map(startup_state)
        .into_iter()
        .collect();
    let project = ControllerProject::from_value(&project_block(
        &feedforward_seed(draft),
        &states,
        &application,
    ))
    .map_err(|message| settings(&message))?;
    let id = loop_id(&draft.name);
    Ok(with_revision(LoopDefinition {
        schema_version: LOOP_SCHEMA_VERSION,
        id: id.clone(),
        name: draft.name.clone(),
        revision: "sha256:pending".to_string(),
        project_id: scope.project_id.clone(),
        runtime_id: scope.runtime_id.clone(),
        controller: feedforward_controller(catalog, draft, manual, task.interval_ms, project)?,
        signals: feedforward_signals(catalog, draft, &setpoint, &process, &manipulated)?,
        limits: Limits {
            output_minimum: Some(manipulated.minimum),
            output_maximum: Some(manipulated.maximum),
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
            parameters: feedforward_parameters(catalog, draft, &process, &manipulated)?,
            mode: ModeBinding {
                symbol: draft.mode.symbol.clone(),
                manual_value: draft.mode.when_true == ModeWhenTrue::Manual,
            },
        }),
        parameter_purpose: None,
        topology: Some(Topology::ClosedLoop),
        experiment: explicit_experiment(
            &draft.name,
            Execution::ControlStudio,
            ExperimentKind::MvStep,
            draft.experiment_step,
            draft.experiment_duration_seconds,
        )?,
        plant_id: None,
        operating_point: None,
        identification_policy: Some(identification_policy()),
        verification_scenarios: None,
        channels: Some(feedforward_channels(
            draft,
            command,
            &manipulated,
            &process,
        )?),
        analysis: None,
        analysis_parameter_symbols: None,
        cascade: None,
    }))
}

/// The one cyclic task of the compiled `Control.PID` the draft names (`feedforwardGenerator.ts`, the first checks).
///
/// # Errors
/// No compiled `Control.PID` has the instance, or it does not run in exactly one cyclic task.
fn feedforward_task<'a>(
    catalog: &'a CompiledCatalog,
    draft: &FeedforwardLoopDraft,
) -> Result<&'a CompiledTask, Failure> {
    let controller = catalog
        .controllers
        .iter()
        .find(|entry| {
            entry.instance == draft.instance && entry.type_name.to_uppercase() == PID_TYPE
        })
        .ok_or_else(|| {
            Failure::new(
                &texts::not_compiled_pid(&draft.instance),
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            )
        })?;
    cyclic_task(controller)
}

/// The lead, lag and filtered variables are all named or all absent, and the variables are distinct from each other and from the loop's
/// signals (C8). The disturbance command is not one of them: it is the disturbance itself, as the program reads it.
///
/// # Errors
/// The lead, lag and filtered variables are not all or none, or two of the variables are the same.
fn check_variables(draft: &FeedforwardLoopDraft) -> Result<(), Failure> {
    let optional = [
        draft.feedforward.lead.as_deref(),
        draft.feedforward.lag.as_deref(),
        draft.feedforward.filtered.as_deref(),
    ];
    let named = optional.iter().filter(|symbol| symbol.is_some()).count();
    if named != 0 && named != optional.len() {
        return Err(settings(texts::feedforward_gain_group()));
    }
    let mut owned = vec![
        draft.feedforward.gain.as_str(),
        draft.process_variable.symbol.as_str(),
        draft.manipulated_variable.symbol.as_str(),
        draft.disturbance.symbol.as_str(),
    ];
    owned.extend(optional.into_iter().flatten());
    let distinct: BTreeSet<String> = owned.iter().map(|symbol| symbol.to_uppercase()).collect();
    if distinct.len() != owned.len() {
        return Err(settings(texts::feedforward_variables_distinct()));
    }
    Ok(())
}

/// The disturbance step is positive and at most the disturbance's span (`feedforwardGenerator.ts`).
///
/// # Errors
/// The step is not positive, or it is larger than the disturbance's span.
fn check_step(draft: &FeedforwardLoopDraft) -> Result<(), Failure> {
    let span = draft.disturbance.maximum - draft.disturbance.minimum;
    if draft.disturbance_step > 0.0 && draft.disturbance_step <= span {
        return Ok(());
    }
    Err(settings(&texts::feedforward_step_span(
        span,
        &draft.disturbance.unit,
    )))
}

/// The variable that moves the measured disturbance must be a simulation or test input (C8, D-5): a REAL or LREAL variable that the
/// program only reads, and that sits at a memory address (`%M`), where the simulation or the test writes it. Setup refuses any other
/// variable before a write.
///
/// # Errors
/// The variable is not compiled, is not `REAL` or `LREAL`, is written by the program, or has no memory address.
fn disturbance_command<'a>(
    catalog: &'a CompiledCatalog,
    symbol: &str,
) -> Result<&'a CompiledSymbol, Failure> {
    let entry = compiled(catalog, symbol)?;
    let written = catalog
        .writes
        .iter()
        .any(|written| written.to_uppercase() == entry.symbol.to_uppercase());
    let memory = entry
        .address
        .as_deref()
        .is_some_and(|address| address.starts_with("%M"));
    if written || !memory {
        return Err(Failure::new(
            &texts::feedforward_command_not_actuated(&entry.symbol),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ));
    }
    real_type(entry)?;
    Ok(entry)
}

/// The retained states of the `Control.PID` with the feedforward's own (C8): the feedforward term starts at `K_ff·d`, and the filtered
/// disturbance `z` starts at `d`, so that the switch to Auto does not bump.
///
/// # Errors
/// The filtered variable is not compiled, or is not `REAL` or `LREAL`.
fn feedforward_states(
    catalog: &CompiledCatalog,
    draft: &FeedforwardLoopDraft,
    process: &MeasuredSignal,
    manipulated: &MeasuredSignal,
) -> Result<Vec<InitializedState>, Failure> {
    let mut states: Vec<InitializedState> = pid_states(&draft.instance, process, manipulated, 0)
        .into_iter()
        .map(|state| scaled_feedforward_state(state, &draft.feedforward.gain))
        .collect();
    if let Some(filtered) = &draft.feedforward.filtered {
        states.push(InitializedState {
            symbol: filtered.clone(),
            iec_type: state_type(real_type(compiled(catalog, filtered)?)?),
            unit: stated_unit(&draft.disturbance.unit),
            minimum: draft.disturbance.minimum,
            maximum: draft.disturbance.maximum,
            initial: InitialValue::From(InitialSource::ModelInput { index: 1 }),
            post_scan: PostScan::Bounded,
        });
    }
    Ok(states)
}

/// The feedforward term's state, which starts at the gain times the measured disturbance at the operating point (C8); other states are
/// kept as the `Control.PID` declares them.
fn scaled_feedforward_state(state: InitializedState, gain: &str) -> InitializedState {
    if !state.symbol.ends_with(FEEDFORWARD_STATE) {
        return state;
    }
    InitializedState {
        initial: InitialValue::From(InitialSource::ScaledModelInput {
            index: 1,
            gain_symbol: gain.to_owned(),
        }),
        ..state
    }
}

/// The seed that names the PI's three gains and the feedforward gain, with the lead and the lag when the program has both (C8).
fn feedforward_seed(draft: &FeedforwardLoopDraft) -> Value {
    let record = draft.parameter_record.as_str();
    let mut seed = json!({
        "kind": FEEDFORWARD_KIND,
        "family": "PI",
        "proportionalSymbol": format!("{record}.ProportionalGain"),
        "integralSymbol": format!("{record}.IntegralGain"),
        "derivativeSymbol": format!("{record}.DerivativeGain"),
        "gainSymbol": draft.feedforward.gain,
    });
    if let (Some(lead), Some(lag)) = (&draft.feedforward.lead, &draft.feedforward.lag) {
        seed["leadSymbol"] = json!(lead);
        seed["lagSymbol"] = json!(lag);
    }
    seed
}

/// The program's feedforward block as the loop file holds it (C8): its family, its declared states with the startup flag as an application
/// state, its seed, and the requirement that Tune ranks its candidates by.
fn project_block(
    seed: &Value,
    states: &[InitializedState],
    application: &[InitializedState],
) -> Value {
    let states = serde_json::to_value(states).unwrap_or_default();
    let mut initialization = json!({ "kind": "declared-state-v1", "states": states });
    if !application.is_empty() {
        initialization["applicationStates"] = serde_json::to_value(application).unwrap_or_default();
    }
    json!({
        "family": FEEDFORWARD_FAMILY,
        "convention": "project-owned-native-controller",
        "initialization": initialization,
        "seed": seed,
        // One controlled output, so the verification campaign names the error requirement without a channel prefix.
        "axes": [],
        "objective": { "requirementId": "target-error", "direction": "minimize" },
        "maximumCandidates": 1,
        "maximumCycles": 50_000,
        "wallBudgetMs": 60_000,
    })
}

/// The identification policy of a feedforward (C8): two orders at most, and the normalized error limit of 0.05, so that both paths keep
/// their own time constants.
fn identification_policy() -> Value {
    json!({
        "maximumOrder": 2,
        "maximumNormalizedError": 0.05,
        "openLoopPermitted": true,
        "sustainedHoldPermitted": true,
        "information": "basic",
    })
}

/// The controller of a feedforward: a library `Control.PID` designed by the project, with its manual output and its project block (C8).
///
/// # Errors
/// The manual output is not `REAL` or `LREAL`.
fn feedforward_controller(
    catalog: &CompiledCatalog,
    draft: &FeedforwardLoopDraft,
    manual: &CompiledSymbol,
    period_ms: f64,
    project: ControllerProject,
) -> Result<Controller, Failure> {
    Ok(Controller {
        instance: draft.instance.clone(),
        type_name: CONTROL_PID.to_string(),
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
        manual_output_follows_output: manual_output_tracking(
            catalog,
            &draft.instance,
            &manual.symbol,
        )
        .then_some(true),
        pid_initialization: None,
        state_feedback: None,
        on_off: None,
        project: Some(project),
        group: None,
    })
}

/// The signals of a feedforward (B1.4, C8): the setpoint is read, the output is written, the measurement is read, and the measured
/// disturbance is a further signal in the role of a disturbance.
///
/// # Errors
/// A signal is not compiled, is not `REAL` or `LREAL`, or is a measurement the program computes.
fn feedforward_signals(
    catalog: &CompiledCatalog,
    draft: &FeedforwardLoopDraft,
    setpoint: &MeasuredSignal,
    process: &MeasuredSignal,
    manipulated: &MeasuredSignal,
) -> Result<Signals, Failure> {
    let disturbance = measurement_binding(catalog, &draft.disturbance)?;
    Ok(Signals {
        setpoint: Some(binding(catalog, setpoint, Direction::Read)?),
        process_variable: measurement_binding(catalog, process)?,
        manipulated_variable: binding(catalog, manipulated, Direction::Write)?,
        actuator: None,
        additional: Some(vec![AuxiliarySignal {
            id: MEASURED_DISTURBANCE.to_owned(),
            channel: MEASURED_DISTURBANCE.to_owned(),
            role: AuxiliaryRole::Disturbance,
            symbol: disturbance.symbol,
            address: disturbance.address,
            iec_type: disturbance.iec_type,
            direction: disturbance.direction,
            unit: disturbance.unit,
            range: disturbance.range,
        }]),
    })
}

/// The channels of a feedforward (C8, `loopChannels`): the controller output, the measured disturbance with its Manual command, and the
/// measurement with its setpoint. The disturbance's command is the one Identify steps (C8).
///
/// # Errors
/// The command's type is not `REAL` or `LREAL`.
fn feedforward_channels(
    draft: &FeedforwardLoopDraft,
    command: &CompiledSymbol,
    manipulated: &MeasuredSignal,
    process: &MeasuredSignal,
) -> Result<LoopChannels, Failure> {
    let actuator = |minimum: f64, maximum: f64| ActuatorCapabilities {
        kind: ActuatorKind::Continuous,
        minimum,
        maximum,
        rate_up_per_s: None,
        rate_down_per_s: None,
        minimum_on_seconds: None,
        minimum_off_seconds: None,
    };
    Ok(LoopChannels {
        inputs: vec![
            ChannelInput {
                model_channel_id: manipulated.symbol.clone(),
                signal_id: MANIPULATED_VARIABLE.to_owned(),
                manual_output_signal: None,
                manual_output_type: None,
                applied_signal_id: None,
                actuator: actuator(manipulated.minimum, manipulated.maximum),
                experiment_step: draft.experiment_step,
            },
            ChannelInput {
                model_channel_id: draft.disturbance.symbol.clone(),
                signal_id: MEASURED_DISTURBANCE.to_owned(),
                manual_output_signal: Some(command.symbol.clone()),
                manual_output_type: Some(manual_output_type(real_type(command)?)),
                applied_signal_id: None,
                actuator: actuator(draft.disturbance.minimum, draft.disturbance.maximum),
                experiment_step: Some(draft.disturbance_step),
            },
        ],
        outputs: vec![ChannelOutput {
            model_channel_id: process.symbol.clone(),
            signal_id: PROCESS_VARIABLE.to_owned(),
            setpoint_signal_id: Some(SETPOINT.to_owned()),
            requirements: None,
        }],
        initial_state: None,
    })
}

/// The gains, the feedforward gain and its lead and lag as parameters of the record (C0, C8): each variable's own type, the PI's gains
/// with the output per the process, the feedforward gain per the disturbance, and the times in seconds.
///
/// # Errors
/// A variable is not compiled, or is not `REAL` or `LREAL`.
fn feedforward_parameters(
    catalog: &CompiledCatalog,
    draft: &FeedforwardLoopDraft,
    process: &MeasuredSignal,
    manipulated: &MeasuredSignal,
) -> Result<Vec<Parameter>, Failure> {
    let mut parameters: Vec<Parameter> =
        pid_parameters(&draft.parameter_record, process, manipulated)
            .into_iter()
            .filter(|parameter| PID_GAIN_CONVENTIONS.contains(&parameter.convention.as_str()))
            .collect();
    let gain_unit = format!("{}/{}", manipulated.unit, draft.disturbance.unit);
    parameters.push(feedforward_parameter(
        catalog,
        &draft.feedforward.gain,
        gain_unit,
        "Feedforward gain",
        -1e6,
        1e6,
    )?);
    if let (Some(lead), Some(lag)) = (&draft.feedforward.lead, &draft.feedforward.lag) {
        parameters.push(feedforward_parameter(
            catalog,
            lead,
            "s".to_owned(),
            "Feedforward lead time",
            0.0,
            1e4,
        )?);
        parameters.push(feedforward_parameter(
            catalog,
            lag,
            "s".to_owned(),
            "Feedforward lag time",
            0.001,
            1e4,
        )?);
    }
    Ok(parameters)
}

/// One feedforward variable as a parameter: its own real type, its unit, its convention and its bounds (N10, C8).
///
/// # Errors
/// The variable is not compiled, or is not `REAL` or `LREAL`.
fn feedforward_parameter(
    catalog: &CompiledCatalog,
    symbol: &str,
    unit: String,
    convention: &str,
    minimum: f64,
    maximum: f64,
) -> Result<Parameter, Failure> {
    Ok(Parameter {
        symbol: symbol.to_owned(),
        iec_type: real_type(compiled(catalog, symbol)?)?,
        unit,
        convention: convention.to_owned(),
        minimum,
        maximum,
    })
}

/// A settings refusal that the setup form's next action answers (A11.2).
fn settings(text: &str) -> Failure {
    Failure::new(text, Cause::Settings, NextAction::OpenLoopSetup)
}

#[cfg(test)]
#[path = "feedforward_loop_tests.rs"]
mod tests;
