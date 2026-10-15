//! The loop definition of a cascade (specification 35, C6 and B1.3): two library `Control.PID` instances in one cyclic task. The inner one
//! drives the plant input and measures the inner measurement; the outer one measures the outer measurement against its setpoint, and its
//! output is the inner setpoint. Both controllers are PI (C0), so each record holds its three gains. Ported from `cascadeGenerator.ts`.
//! The one-loop form only is served (D-2). Setup asks for no step or duration, so Identify chooses them (B3.3, B3.4), and the outer
//! controller has no manual output in the draft, because nothing in the structure uses one (D-4).

#![allow(clippy::module_name_repetitions)] // `CascadeLoopDraft` names the module's subject, as `FeedforwardLoopDraft` does.

use serde::Deserialize;
use serde_json::{json, Value};

use super::catalog::{
    own_measurement, CompiledCatalog, CompiledController, CompiledSymbol, CompiledTask, PID_TYPE,
};
use super::loop_bindings::{
    binding, compiled, cyclic_task, explicit_experiment, loop_id, manual_output_type,
    measured_signal, measurement_binding, real_type, stated_signal, with_revision, FormSignal,
    MeasuredSignal,
};
use super::pid_bindings::{
    manual_output_tracking, pid_parameters, pid_states, startup_flag_is_bool, startup_reason,
    startup_state,
};
use super::pid_loop::{pid_seeded_symbols, setpoint_signal, ModeChoice, SignalChoice};
use super::texts::generators as texts;
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    Action, AuxiliaryRole, AuxiliarySignal, ChannelInput, ChannelOutput, Controller,
    ControllerProject, Design, Direction, Execution, ExperimentKind, InitialSource, InitialValue,
    InitializedState, Limits, LoopChannels, LoopDefinition, LoopScope, ModeBinding, ModeWhenTrue,
    Parameter, ParameterBinding, SampleTiming, Signals, Topology, LOOP_SCHEMA_VERSION,
};
use crate::model::{ActuatorCapabilities, ActuatorKind};

/// The seed kind of a cascade in the loop file (C6).
const CASCADE_KIND: &str = "cascade-pid";
/// The family of a cascade's project block in the loop file (C6).
const CASCADE_FAMILY: &str = "cascade";
/// The library type of both controllers (C1).
const CONTROL_PID: &str = "Control.PID";
/// The channel identifiers of the controller output, the inner measurement, the outer measurement and its setpoint, as the loop's signals
/// name them (C6).
const MANIPULATED_VARIABLE: &str = "manipulatedVariable";
const INNER_MEASUREMENT: &str = "inner-measurement";
const PROCESS_VARIABLE: &str = "processVariable";
const SETPOINT: &str = "setpoint";
/// The conventions of the three gains of a PI record, as the parameter record names them (C1).
const PID_GAIN_CONVENTIONS: [&str; 3] = [
    "Control.PID ProportionalGain",
    "Control.PID IntegralGain",
    "Control.PID DerivativeGain",
];
/// The number of identification orders of a cascade: one, so that the vessel follows the jacket as in the process (N10, C6).
pub const CASCADE_MAXIMUM_ORDER: u64 = 1;
/// The cycle budget of a project block (N10: C2, C5 to C10).
const PROJECT_CYCLE_BUDGET: u64 = 50_000;
/// The wall-clock budget of a project block, in milliseconds (N10: C2, C5 to C10).
const PROJECT_WALL_BUDGET_MS: u64 = 60_000;
/// The candidates of a structure with one design (N10: C6 to C10).
const SINGLE_DESIGN_CANDIDATES: u64 = 1;

/// The inner controller of a cascade as the engineer confirmed it (C6): the plant input's controller, with its record, mode, manual output
/// and the measurement it reads.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CascadeInnerDraft {
    /// The `Control.PID` instance path.
    pub instance: String,
    /// The record the inner PI is configured from.
    pub parameter_record: String,
    /// The Auto/Manual signal of the inner controller.
    pub mode: ModeChoice,
    /// The inner controller's manual output, which Identify steps.
    pub manual_output: String,
    /// The inner measurement.
    pub process_variable: FormSignal,
    /// The plant input the inner controller drives.
    pub manipulated_variable: FormSignal,
}

/// The outer controller of a cascade as the engineer confirmed it (C6): its record, its mode, the outer measurement and its setpoint.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CascadeOuterDraft {
    /// The `Control.PID` instance path.
    pub instance: String,
    /// The record the outer PI is configured from.
    pub parameter_record: String,
    /// The Auto/Manual signal of the outer controller, which the transfer switches with the inner one.
    pub mode: ModeChoice,
    /// The outer measurement.
    pub process_variable: FormSignal,
    /// The outer setpoint.
    pub setpoint: SignalChoice,
}

/// A cascade loop as the engineer confirmed it (`CascadeTogetherDraft`, C6). Setup asks for no experiment step or duration, so both may
/// be absent, and Identify chooses them (B3.3, B3.4).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CascadeLoopDraft {
    /// The loop's name, from which its id is made.
    pub name: String,
    /// Whether the output rises (`direct`) or falls (`reverse`) while the measurement is below the setpoint.
    pub action: Action,
    /// The outer controller.
    pub outer: CascadeOuterDraft,
    /// The inner controller.
    pub inner: CascadeInnerDraft,
    /// The plant input's identification step, in its unit: positive and within its span; Identify chooses it when absent.
    pub experiment_step: Option<f64>,
    /// How long identification runs, in s; Identify chooses it when absent.
    pub experiment_duration_seconds: Option<f64>,
    /// The BOOL the program sets when its startup code has run.
    pub startup_flag: Option<String>,
}

/// The loop definition of a cascade, ready for Identify, Tune, Validate and Apply (`cascadeLoopDefinition`, C6).
///
/// # Errors
/// An instance is not a compiled `Control.PID` in exactly one cyclic task; the two instances are the same, or run in different tasks; the
/// two modes are the same signal; a signal, a variable or the manual output is not compiled, or is not `REAL` or `LREAL`; or the seed's
/// variables are not distinct.
pub fn cascade_loop_definition(
    catalog: &CompiledCatalog,
    scope: &LoopScope,
    draft: &CascadeLoopDraft,
) -> Result<LoopDefinition, Failure> {
    let task = cascade_task(catalog, draft)?;
    let process_inner = stated_signal(&measured_signal(
        &draft.inner.process_variable,
        "inner measurement",
        Some("inner.processVariable"),
    )?);
    let process_outer = stated_signal(&measured_signal(
        &draft.outer.process_variable,
        "outer measurement",
        Some("outer.processVariable"),
    )?);
    let manipulated = stated_signal(&measured_signal(
        &draft.inner.manipulated_variable,
        "output",
        Some("inner.manipulatedVariable"),
    )?);
    let setpoint = setpoint_signal(&draft.outer.setpoint, &process_outer);
    let manual = compiled(catalog, &draft.inner.manual_output)?;
    check_startup(catalog, draft)?;
    let states = cascade_states(draft, &process_inner, &process_outer, &manipulated);
    let application: Vec<InitializedState> = draft
        .startup_flag
        .as_deref()
        .filter(|flag| !flag.is_empty())
        .map(startup_state)
        .into_iter()
        .collect();
    let project = ControllerProject::from_value(&project_block(
        draft,
        &process_outer.symbol,
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
        controller: cascade_controller(catalog, draft, manual, task.interval_ms, project)?,
        signals: cascade_signals(
            catalog,
            &setpoint,
            &process_inner,
            &process_outer,
            &manipulated,
        )?,
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
            parameters: cascade_parameters(draft, &process_inner, &process_outer, &manipulated),
            mode: ModeBinding {
                symbol: draft.inner.mode.symbol.clone(),
                manual_value: draft.inner.mode.when_true == ModeWhenTrue::Manual,
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
        channels: Some(cascade_channels(
            draft,
            &manipulated,
            &process_inner,
            &process_outer,
        )),
        analysis: None,
        analysis_parameter_symbols: None,
        cascade: None,
    }))
}

/// The one cyclic task that both controllers of the draft run in (C6): each instance is a compiled `Control.PID` in exactly one cyclic
/// task, the instances differ, the two modes are different signals, and the tasks are the same.
///
/// # Errors
/// An instance is not a compiled `Control.PID` in one cyclic task, the instances are the same, the modes are the same, or the tasks differ.
fn cascade_task<'a>(
    catalog: &'a CompiledCatalog,
    draft: &CascadeLoopDraft,
) -> Result<&'a CompiledTask, Failure> {
    let inner = cyclic_task(pid_controller(catalog, &draft.inner.instance)?)?;
    let outer = cyclic_task(pid_controller(catalog, &draft.outer.instance)?)?;
    if draft.inner.instance == draft.outer.instance {
        return Err(settings(TWO_CONTROLLERS));
    }
    if inner.name != outer.name {
        return Err(settings(ONE_TASK));
    }
    if draft.inner.mode.symbol == draft.outer.mode.symbol {
        return Err(settings(TWO_MODES));
    }
    Ok(inner)
}

/// The compiled `Control.PID` with the instance path.
///
/// # Errors
/// No compiled `Control.PID` has the instance.
fn pid_controller<'a>(
    catalog: &'a CompiledCatalog,
    instance: &str,
) -> Result<&'a CompiledController, Failure> {
    catalog
        .controllers
        .iter()
        .find(|entry| entry.instance == instance && entry.type_name.to_uppercase() == PID_TYPE)
        .ok_or_else(|| {
            Failure::new(
                &format!("{instance} is not a compiled Control.PID instance."),
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            )
        })
}

/// The states of both controllers (C6): the inner's seven states at its own measurement and plant input, and the outer's seven at the
/// outer measurement, whose output starts at the inner measurement, so that neither switch bumps. The startup flag is an application
/// state, which the project block holds apart.
fn cascade_states(
    draft: &CascadeLoopDraft,
    process_inner: &MeasuredSignal,
    process_outer: &MeasuredSignal,
    manipulated: &MeasuredSignal,
) -> Vec<InitializedState> {
    let inner = pid_states(&draft.inner.instance, process_inner, manipulated, 0);
    let outer = pid_states(&draft.outer.instance, process_outer, process_inner, 1)
        .into_iter()
        .map(outer_start)
        .collect::<Vec<_>>();
    inner.into_iter().chain(outer).collect()
}

/// The outer controller's state that starts from the plant input (`pidStates`, C6): its output starts at the inner measurement, the
/// model's first output, so it is started from the measurement the inner controller holds (C6). Every other state keeps its start.
fn outer_start(state: InitializedState) -> InitializedState {
    if matches!(
        state.initial,
        InitialValue::From(InitialSource::ModelInput { .. })
    ) {
        InitializedState {
            initial: InitialValue::From(InitialSource::ModelOutput { index: 0 }),
            ..state
        }
    } else {
        state
    }
}

/// The seed that names both controllers' gain variables (C6): each instance and its record's three gains, the PI family for both.
fn seed(draft: &CascadeLoopDraft) -> Value {
    json!({
        "kind": CASCADE_KIND,
        "family": "PI",
        "inner": controller_gains(&draft.inner.instance, &draft.inner.parameter_record),
        "outer": controller_gains(&draft.outer.instance, &draft.outer.parameter_record),
    })
}

/// One controller's gain variables in its record (`symbols` of `cascadeGenerator.ts`).
fn controller_gains(instance: &str, record: &str) -> Value {
    json!({
        "instance": instance,
        "proportionalSymbol": format!("{record}.ProportionalGain"),
        "integralSymbol": format!("{record}.IntegralGain"),
        "derivativeSymbol": format!("{record}.DerivativeGain"),
    })
}

/// The program's cascade block as the loop file holds it (C6): its family, its declared states with the startup flag as an application
/// state, its seed, the outer controller's mode as an additional mode, and the requirement that Tune ranks its candidates by: the outer
/// measurement's error, named with its output's prefix since the loop has two outputs (`P1.VesselC:target-error`).
fn project_block(
    draft: &CascadeLoopDraft,
    outer_symbol: &str,
    states: &[InitializedState],
    application: &[InitializedState],
) -> Value {
    let mut initialization = json!({
        "kind": "declared-state-v1",
        "states": serde_json::to_value(states).unwrap_or_default(),
    });
    if !application.is_empty() {
        initialization["applicationStates"] = serde_json::to_value(application).unwrap_or_default();
    }
    json!({
        "family": CASCADE_FAMILY,
        "convention": "project-owned-native-controller",
        "initialization": initialization,
        "seed": seed(draft),
        "additionalModes": [{
            "symbol": draft.outer.mode.symbol,
            "manualValue": draft.outer.mode.when_true == ModeWhenTrue::Manual,
        }],
        "axes": [],
        "objective": { "requirementId": format!("{outer_symbol}:target-error"), "direction": "minimize" },
        "maximumCandidates": SINGLE_DESIGN_CANDIDATES,
        "maximumCycles": PROJECT_CYCLE_BUDGET,
        "wallBudgetMs": PROJECT_WALL_BUDGET_MS,
    })
}

/// The identification policy of a cascade (C6): one order, as the vessel follows the jacket as in the process.
fn identification_policy() -> Value {
    json!({
        "maximumOrder": CASCADE_MAXIMUM_ORDER,
        "openLoopPermitted": true,
        "sustainedHoldPermitted": true,
        "information": "basic",
    })
}

/// The inner controller of a cascade: a library `Control.PID` designed by the project, with its manual output and the project block (C6).
///
/// # Errors
/// The manual output is not `REAL` or `LREAL`.
fn cascade_controller(
    catalog: &CompiledCatalog,
    draft: &CascadeLoopDraft,
    manual: &CompiledSymbol,
    period_ms: f64,
    project: ControllerProject,
) -> Result<Controller, Failure> {
    Ok(Controller {
        instance: draft.inner.instance.clone(),
        type_name: CONTROL_PID.to_string(),
        revision: "sha256:unverified".to_string(),
        action: draft.action,
        task_period_ms: period_ms,
        design: Some(Design::Project),
        law: None,
        mode_signal: Some(draft.inner.mode.symbol.clone()),
        mode_when_true: Some(draft.inner.mode.when_true),
        tracking_signal: None,
        manual_output_signal: Some(manual.symbol.clone()),
        manual_output_address: manual.address.clone().filter(|address| !address.is_empty()),
        manual_output_type: Some(manual_output_type(real_type(manual)?)),
        manual_output_follows_output: manual_output_tracking(
            catalog,
            &draft.inner.instance,
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

/// The signals of a cascade (B1.4, C6): the setpoint is read, the output is written, the outer measurement is read, and the inner
/// measurement is a further signal in the role of a state.
///
/// # Errors
/// A signal is not compiled, is not `REAL` or `LREAL`, or is a measurement the program computes.
fn cascade_signals(
    catalog: &CompiledCatalog,
    setpoint: &MeasuredSignal,
    process_inner: &MeasuredSignal,
    process_outer: &MeasuredSignal,
    manipulated: &MeasuredSignal,
) -> Result<Signals, Failure> {
    let inner = measurement_binding(catalog, process_inner)?;
    Ok(Signals {
        setpoint: Some(binding(catalog, setpoint, Direction::Read)?),
        process_variable: measurement_binding(catalog, process_outer)?,
        manipulated_variable: binding(catalog, manipulated, Direction::Write)?,
        actuator: None,
        additional: Some(vec![AuxiliarySignal {
            id: INNER_MEASUREMENT.to_owned(),
            channel: INNER_MEASUREMENT.to_owned(),
            role: AuxiliaryRole::State,
            symbol: inner.symbol,
            address: inner.address,
            iec_type: inner.iec_type,
            direction: inner.direction,
            unit: inner.unit,
            range: inner.range,
        }]),
    })
}

/// The channels of a cascade (C6, `loopChannels`): the controller output, the inner measurement without a setpoint, and the outer
/// measurement with its setpoint. The plant input's step is the draft's experiment step, and Identify chooses it when there is none.
fn cascade_channels(
    draft: &CascadeLoopDraft,
    manipulated: &MeasuredSignal,
    process_inner: &MeasuredSignal,
    process_outer: &MeasuredSignal,
) -> LoopChannels {
    LoopChannels {
        inputs: vec![ChannelInput {
            model_channel_id: manipulated.symbol.clone(),
            signal_id: MANIPULATED_VARIABLE.to_owned(),
            manual_output_signal: None,
            manual_output_type: None,
            applied_signal_id: None,
            actuator: ActuatorCapabilities {
                kind: ActuatorKind::Continuous,
                minimum: manipulated.minimum,
                maximum: manipulated.maximum,
                rate_up_per_s: None,
                rate_down_per_s: None,
                minimum_on_seconds: None,
                minimum_off_seconds: None,
            },
            experiment_step: draft.experiment_step,
        }],
        outputs: vec![
            ChannelOutput {
                model_channel_id: process_inner.symbol.clone(),
                signal_id: INNER_MEASUREMENT.to_owned(),
                setpoint_signal_id: None,
                requirements: None,
            },
            ChannelOutput {
                model_channel_id: process_outer.symbol.clone(),
                signal_id: PROCESS_VARIABLE.to_owned(),
                setpoint_signal_id: Some(SETPOINT.to_owned()),
                requirements: None,
            },
        ],
        initial_state: None,
    }
}

/// The gains of both PI records as parameters (C0, C6): the inner's three gains, then the outer's, each with the output per the process it
/// controls, and each variable's own type. The record's other fields stay as the program holds them, so only the gains are bound.
fn cascade_parameters(
    draft: &CascadeLoopDraft,
    process_inner: &MeasuredSignal,
    process_outer: &MeasuredSignal,
    manipulated: &MeasuredSignal,
) -> Vec<Parameter> {
    let gains = |parameters: Vec<Parameter>| -> Vec<Parameter> {
        parameters
            .into_iter()
            .filter(|parameter| PID_GAIN_CONVENTIONS.contains(&parameter.convention.as_str()))
            .collect()
    };
    let mut parameters = gains(pid_parameters(
        &draft.inner.parameter_record,
        process_inner,
        manipulated,
    ));
    parameters.extend(gains(pid_parameters(
        &draft.outer.parameter_record,
        process_outer,
        process_inner,
    )));
    parameters
}

/// A cascade whose program writes a seeded value, or initializes either PID, needs the startup-done flag (C1 Setup). The seeded values
/// are, for each PID, its setpoint or inner manual output, its mode and its states, and the two parameter records, as for a PID loop.
/// A draft that names a flag must name a compiled `BOOL`.
///
/// # Errors
/// The program needs the flag, and the draft names none.
fn check_startup(catalog: &CompiledCatalog, draft: &CascadeLoopDraft) -> Result<(), Failure> {
    if let Some(flag) = draft
        .startup_flag
        .as_deref()
        .filter(|flag| !flag.is_empty())
    {
        return startup_flag_is_bool(catalog, flag);
    }
    let outer_own = draft.outer.process_variable.symbol.to_uppercase()
        == own_measurement(&draft.outer.instance).to_uppercase();
    let inner_own = draft.inner.process_variable.symbol.to_uppercase()
        == own_measurement(&draft.inner.instance).to_uppercase();
    // The outer setpoint is the seed; the inner setpoint is the outer output, and the outer law has no manual output.
    let mut seeded = pid_seeded_symbols(
        &draft.outer.instance,
        [
            draft.outer.setpoint.symbol.as_str(),
            "",
            draft.outer.mode.symbol.as_str(),
        ],
        outer_own,
    );
    seeded.extend(pid_seeded_symbols(
        &draft.inner.instance,
        [
            "",
            draft.inner.manual_output.as_str(),
            draft.inner.mode.symbol.as_str(),
        ],
        inner_own,
    ));
    let records = [
        draft.outer.parameter_record.clone(),
        draft.inner.parameter_record.clone(),
    ];
    let reason = [draft.outer.instance.as_str(), draft.inner.instance.as_str()]
        .into_iter()
        .find_map(|instance| startup_reason(catalog, Some(instance), &seeded, &records));
    match reason {
        Some(reason) => {
            Err(settings(&texts::startup_flag_needed(&reason)).with_field("startupFlag"))
        }
        None => Ok(()),
    }
}

/// A settings refusal that the setup form's next action answers (A11.2).
fn settings(text: &str) -> Failure {
    Failure::new(text, Cause::Settings, NextAction::OpenLoopSetup)
}

/// The refusal of two controllers that are the same instance.
const TWO_CONTROLLERS: &str = "Choose two different controllers.";
/// The refusal of two controllers that run in different tasks.
const ONE_TASK: &str = "Both controllers of a cascade must run in the same task.";
/// The refusal of two controllers that share one Auto/Manual signal.
const TWO_MODES: &str = "Each controller of a cascade needs its own Auto/Manual signal.";

#[cfg(test)]
#[path = "cascade_loop_tests.rs"]
mod tests;
