//! The loop definition of one library `Control.PID` (specification 35, B1.3, B1.4, C1 and §17.1): what setup writes for a PID that the
//! program runs. Ported from `pidLoopGenerator.ts`. The project and the runtime come from the scope, never from the draft.

#![allow(clippy::module_name_repetitions)] // `PidLoopDraft` names the module's subject, as the TypeScript does.

use serde::Deserialize;

use super::catalog::{own_measurement, CompiledCatalog, CompiledSymbol, CompiledTask, PID_TYPE};
use super::loop_bindings::{
    binding, compiled, cyclic_task, explicit_experiment, loop_id, manual_output_type,
    measured_signal, measurement_binding, real_type, stated_signal, with_revision, FormSignal,
    MeasuredSignal,
};
use super::pid_bindings::{
    manual_output_tracking, pid_parameters, pid_states, startup_flag_is_bool, startup_reason,
    startup_state,
};
use super::texts::generators as texts;
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    Action, Controller, ControllerLaw, Design, Direction, Execution, ExperimentKind, Limits,
    LoopDefinition, LoopScope, ModeBinding, ModeWhenTrue, ParameterBinding, PidInitialization,
    PidLaw, SampleTiming, Signals, Topology, LOOP_SCHEMA_VERSION,
};

/// The retained states of `Control.PID`, in the order of the `control-pid-v1` table (C1).
const RETAINED_STATES: [&str; 7] = [
    "Setpoint",
    "SetpointRate",
    "ProcessValue",
    "ProcessValueRate",
    "FeedForward",
    "NominalOutput",
    "SaturatedOutput",
];

/// A signal the setup form chose: a symbol, and a unit and range that default to the measurement's (`SignalChoice`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignalChoice {
    /// The variable's path.
    pub symbol: String,
    /// The unit; the measurement's unit when absent.
    pub unit: Option<String>,
    /// The smallest value; the measurement's when absent.
    pub minimum: Option<f64>,
    /// The largest value; the measurement's when absent.
    pub maximum: Option<f64>,
}

/// The Auto/Manual signal the setup form chose, and the mode its TRUE value means (`ModeChoice`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModeChoice {
    /// The BOOL's path.
    pub symbol: String,
    /// The mode the BOOL's TRUE value chooses; the JSON key is `whenTrue`.
    pub when_true: ModeWhenTrue,
}

/// A single-loop PID as the engineer confirmed it (`PidLoopDraft`, B1.3). Setup asks for no experiment step or duration, so both
/// may be absent, and Identify chooses them (B3.3, B3.4).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PidLoopDraft {
    /// The loop's name, from which its id is made.
    pub name: String,
    /// The PID's instance path.
    pub instance: String,
    /// Whether the output rises (`direct`) or falls (`reverse`) while the measurement is below the setpoint.
    pub action: Action,
    /// The record the PID is configured from.
    pub parameter_record: String,
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
    /// The identification experiment's output step, in the output unit; Identify chooses it when absent.
    pub experiment_step: Option<f64>,
    /// How long identification runs, in s; Identify chooses it when absent.
    pub experiment_duration_seconds: Option<f64>,
    /// The BOOL the program sets when its startup code has run.
    pub startup_flag: Option<String>,
}

/// The loop definition of one library `Control.PID`, ready for Identify, Tune, Validate and Apply (`pidLoopDefinition`).
///
/// # Errors
/// The instance is not a compiled `Control.PID`, or does not run in exactly one cyclic task; a signal is not compiled, is not
/// `REAL` or `LREAL`, or is a measurement the program computes; the manual output is not `REAL` or `LREAL`; only one of the
/// experiment's step and duration is set; or the program writes a value that validation seeds, or initializes the PID, and the
/// draft names no startup-done flag.
pub fn pid_loop_definition(
    catalog: &CompiledCatalog,
    scope: &LoopScope,
    draft: &PidLoopDraft,
) -> Result<LoopDefinition, Failure> {
    let task = pid_task(catalog, draft)?;
    let process_variable = stated_signal(&measured_signal(
        &draft.process_variable,
        "measurement",
        Some("processVariable"),
    )?);
    let manipulated_variable = stated_signal(&measured_signal(
        &draft.manipulated_variable,
        "output",
        Some("manipulatedVariable"),
    )?);
    let setpoint = setpoint_signal(&draft.setpoint, &process_variable);
    let manual = compiled(catalog, &draft.manual_output)?;
    let startup = draft
        .startup_flag
        .as_deref()
        .filter(|flag| !flag.is_empty());
    let controller = pid_controller(
        catalog,
        draft,
        manual,
        startup,
        &process_variable,
        &manipulated_variable,
        task.interval_ms,
    )?;
    // The measurement is the PID's own ProcessValue whatever the program computed it from, so it is never refused as computed (C1).
    let measurement_is_own =
        process_variable.symbol.to_uppercase() == own_measurement(&draft.instance).to_uppercase();
    let signals = pid_signals(
        catalog,
        &setpoint,
        &process_variable,
        &manipulated_variable,
        measurement_is_own,
    )?;
    let experiment = explicit_experiment(
        &draft.name,
        Execution::ControlStudio,
        ExperimentKind::MvStep,
        draft.experiment_step,
        draft.experiment_duration_seconds,
    )?;
    check_startup(catalog, draft, measurement_is_own)?;

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
            parameters: pid_parameters(
                &draft.parameter_record,
                &process_variable,
                &manipulated_variable,
            ),
            mode: ModeBinding {
                symbol: draft.mode.symbol.clone(),
                manual_value: draft.mode.when_true == ModeWhenTrue::Manual,
            },
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

/// The one cyclic task of the compiled `Control.PID` the draft names (`pidLoopGenerator.ts`, the first checks).
///
/// # Errors
/// No compiled `Control.PID` has the instance, or it does not run in exactly one cyclic task.
fn pid_task<'a>(
    catalog: &'a CompiledCatalog,
    draft: &PidLoopDraft,
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

/// The setpoint the setup form chose, with the measurement's unit and range where it gives none (`pidLoopGenerator.ts`, `setpoint`).
pub(super) fn setpoint_signal(
    choice: &SignalChoice,
    process_variable: &MeasuredSignal,
) -> MeasuredSignal {
    stated_signal(&MeasuredSignal {
        symbol: choice.symbol.clone(),
        unit: choice
            .unit
            .clone()
            .unwrap_or_else(|| process_variable.unit.clone()),
        minimum: choice.minimum.unwrap_or(process_variable.minimum),
        maximum: choice.maximum.unwrap_or(process_variable.maximum),
    })
}

/// The signals of a PID loop (B1.4): the setpoint and the output are the program's variables, and the measurement is read. A
/// measurement the program computes is refused, unless it is the PID's own `ProcessValue` (C1 Setup, B5.3).
///
/// # Errors
/// A signal is not compiled, is not `REAL` or `LREAL`, or is a measurement the program computes.
fn pid_signals(
    catalog: &CompiledCatalog,
    setpoint: &MeasuredSignal,
    process_variable: &MeasuredSignal,
    manipulated_variable: &MeasuredSignal,
    measurement_is_own: bool,
) -> Result<Signals, Failure> {
    let setpoint_binding = binding(catalog, setpoint, Direction::Read)?;
    let process_signal = if measurement_is_own {
        binding(catalog, process_variable, Direction::Read)?
    } else {
        measurement_binding(catalog, process_variable)?
    };
    let manipulated_signal = binding(catalog, manipulated_variable, Direction::Write)?;
    Ok(Signals {
        setpoint: Some(setpoint_binding),
        process_variable: process_signal,
        manipulated_variable: manipulated_signal,
        actuator: None,
        additional: None,
    })
}

/// The controller of a PID loop: its law, its manual output and its initialization (B1.4, C1 Setup).
///
/// # Errors
/// The manual output is not `REAL` or `LREAL`.
fn pid_controller(
    catalog: &CompiledCatalog,
    draft: &PidLoopDraft,
    manual: &CompiledSymbol,
    startup: Option<&str>,
    process_variable: &MeasuredSignal,
    manipulated_variable: &MeasuredSignal,
    period_ms: f64,
) -> Result<Controller, Failure> {
    let record = draft.parameter_record.as_str();
    Ok(Controller {
        instance: draft.instance.clone(),
        type_name: "Control.PID".to_string(),
        revision: "sha256:unverified".to_string(),
        action: draft.action,
        task_period_ms: period_ms,
        design: Some(Design::Pid),
        law: Some(ControllerLaw::Pid(Box::new(PidLaw {
            parameter_record_symbol: record.to_string(),
            parameter_revision_symbol: None,
            proportional_gain_symbol: format!("{record}.ProportionalGain"),
            integral_gain_symbol: format!("{record}.IntegralGain"),
            derivative_gain_symbol: format!("{record}.DerivativeGain"),
            proportional_setpoint_weight_symbol: None,
            derivative_setpoint_weight_symbol: None,
            measurement_filter_time_symbol: None,
            anti_windup: None,
        }))),
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
        pid_initialization: Some(PidInitialization::ControlPid {
            maximum_snapshot_age_ms: snapshot_age_ms(period_ms),
            states: pid_states(&draft.instance, process_variable, manipulated_variable, 0),
            application_states: startup.map(|flag| vec![startup_state(flag)]),
        }),
        state_feedback: None,
        on_off: None,
        project: None,
        group: None,
    })
}

/// A PID loop whose program writes a seeded value, or initializes the PID, needs the startup-done flag (C1 Setup). A draft that
/// names a flag must name a compiled `BOOL`.
///
/// # Errors
/// The program needs the flag, and the draft names none.
fn check_startup(
    catalog: &CompiledCatalog,
    draft: &PidLoopDraft,
    measurement_is_own: bool,
) -> Result<(), Failure> {
    if let Some(flag) = draft
        .startup_flag
        .as_deref()
        .filter(|flag| !flag.is_empty())
    {
        return startup_flag_is_bool(catalog, flag);
    }
    let seeded = seeded_symbols(draft, measurement_is_own);
    let records = std::slice::from_ref(&draft.parameter_record);
    match startup_reason(catalog, Some(draft.instance.as_str()), &seeded, records) {
        Some(reason) => Err(Failure::new(
            &texts::startup_flag_needed(&reason),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        )
        .with_field("startupFlag")),
        None => Ok(()),
    }
}

/// The symbols validation seeds for a PID loop (C1 Setup): the setpoint, the manual output, the mode and the PID's states. When the
/// measurement is the PID's own `ProcessValue`, that state is the measurement, which the transfer reads and never seeds.
fn seeded_symbols(draft: &PidLoopDraft, measurement_is_own: bool) -> Vec<String> {
    pid_seeded_symbols(
        &draft.instance,
        [
            draft.setpoint.symbol.as_str(),
            draft.manual_output.as_str(),
            draft.mode.symbol.as_str(),
        ],
        measurement_is_own,
    )
}

/// The symbols validation seeds for one `Control.PID` (C1 Setup): the setpoint, the manual output and the mode that are given (a blank
/// one is none), and the PID's retained states. When the measurement is the PID's own `ProcessValue`, that state is the measurement,
/// which the transfer reads and never seeds. A cascade and a Smith predictor seed each of their PIDs this way.
pub(super) fn pid_seeded_symbols(
    instance: &str,
    values: [&str; 3],
    measurement_is_own: bool,
) -> Vec<String> {
    let mut seeded: Vec<String> = values
        .into_iter()
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect();
    seeded.extend(
        RETAINED_STATES
            .iter()
            .filter(|state| !measurement_is_own || **state != "ProcessValue")
            .map(|state| format!("{instance}.StateValue.{state}")),
    );
    seeded
}

/// The oldest snapshot an Auto entry may use, in whole ms: 500 ms, or five task periods when that is longer (`pidLoopGenerator.ts`).
/// The age is rounded up, so it is never shorter than the periods it covers.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // A clamped, rounded-up value is a non-negative whole number of ms.
fn snapshot_age_ms(period_ms: f64) -> u64 {
    (5.0 * period_ms).max(500.0).ceil() as u64
}

#[cfg(test)]
#[path = "pid_loop_tests.rs"]
mod tests;
