//! The loop definition of a Smith predictor (specification 35, C9; `smithPredictorGenerator.ts`): a library `Control.PID` whose program feeds
//! it the measurement plus the predictor's correction. Its feedback law is a PI, whose three gains are parameters of the record, and the
//! predictor's model gain, time constant and delay are program variables that Tune sets. The correction is a declared state that starts at
//! zero. When the program uses the library block `Control.SmithPredictorFOPDT`, the block's result, its maximum correction and its
//! operating point are named too (C9). The startup-done flag is an application state, as every Rust generator writes it (C1).

#![allow(clippy::module_name_repetitions)] // `SmithLoopDraft` names the module's subject, as `FeedforwardLoopDraft` does.

use std::collections::BTreeSet;

use serde::Deserialize;
use serde_json::{json, Value};

use super::catalog::{own_measurement, CompiledCatalog, CompiledSymbol, CompiledTask, PID_TYPE};
use super::loop_bindings::{
    binding, compiled, cyclic_task, explicit_experiment, loop_id, manual_output_type,
    measured_signal, measurement_binding, real_type, state_type, stated_signal, stated_unit,
    with_revision, FormSignal, MeasuredSignal,
};
use super::pid_bindings::{
    manual_output_tracking, pid_parameters, pid_states, startup_flag_is_bool, startup_reason,
    startup_state,
};
use super::pid_loop::{pid_seeded_symbols, setpoint_signal, ModeChoice, SignalChoice};
use super::texts::generators as texts;
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    Action, Controller, ControllerProject, Design, Direction, Execution, ExperimentKind,
    InitialValue, InitializedState, Limits, LoopDefinition, LoopScope, ModeBinding, ModeWhenTrue,
    Parameter, ParameterBinding, PostScan, SampleTiming, Signals, Topology, LOOP_SCHEMA_VERSION,
};

/// The family of a Smith predictor's project block in the loop file (C9).
const SMITH_FAMILY: &str = "Smith";
/// The seed kind of a Smith predictor in the loop file (C9).
const SMITH_KIND: &str = "smith-pid";
/// The library type of the controller the predictor is added to (C1).
const CONTROL_PID: &str = "Control.PID";
/// The conventions of the three PID gains as the parameter record names them (C1).
const PID_GAIN_CONVENTIONS: [&str; 3] = [
    "Control.PID ProportionalGain",
    "Control.PID IntegralGain",
    "Control.PID DerivativeGain",
];
/// The largest magnitude of the model gain `K`, in its unit (N10, C9).
const SMITH_GAIN_LIMIT: f64 = 1e6;
/// The smallest model time constant `T`, in seconds (N10, C9).
const SMITH_TIME_CONSTANT_MINIMUM_SECONDS: f64 = 0.001;
/// The largest model time constant `T`, in seconds (N10, C9).
const SMITH_TIME_CONSTANT_MAXIMUM_SECONDS: f64 = 1e5;
/// The largest model delay `θ`, in seconds (N10, C9).
const SMITH_DELAY_MAXIMUM_SECONDS: f64 = 1e4;
/// The most scans a search of the predictor's block may run together (N10, the project block's budget of C2).
const SMITH_MAXIMUM_CYCLES: u64 = 50_000;
/// The wall-clock budget of the predictor's block, in milliseconds (N10, the project block's budget of C2).
const SMITH_WALL_BUDGET_MS: u64 = 60_000;
/// The refusal of a predictor whose variables are not distinct from each other and from the loop's signals (C9).
const VARIABLES_DISTINCT: &str =
    "The predictor's model variables and correction must differ from each other and from the loop's signals.";

/// The program's variables of the predictor's model and correction (C9): the model gain `K`, the time constant `T` and the delay `θ`, and
/// the correction `c` that the program keeps.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SmithModelVariables {
    /// The variable that holds the model gain `K`, in the process unit per the manipulated unit.
    pub gain: String,
    /// The variable that holds the model time constant `T` in seconds.
    pub time_constant: String,
    /// The variable that holds the model delay `θ` in seconds.
    pub delay: String,
    /// The variable that holds the correction `c`, which starts at zero.
    pub correction: String,
}

/// The variables of a program that uses the library block `Control.SmithPredictorFOPDT` (C9): its `Update` result, its `MaximumCorrection`
/// and its operating input and output.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SmithLibraryVariables {
    /// The variable that holds the block's `SmithPredictorOutput`, which Validate reads.
    pub output: String,
    /// The variable of the program's `MaximumCorrection`, which Validate reads and Apply never writes.
    pub maximum_correction: String,
    /// The variable of the block's `OperatingInput`, written from the model's operating input (D-18).
    pub operating_input: String,
    /// The variable of the block's `OperatingOutput`, written from the model's operating output (D-18).
    pub operating_output: String,
}

/// A Smith predictor loop as the engineer confirmed it (`SmithPredictorDraft`, C9). Setup asks for no experiment step or duration, so
/// both may be absent, and Identify chooses them (B3.3, B3.4).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SmithLoopDraft {
    /// The loop's name, from which its id is made.
    pub name: String,
    /// The `Control.PID` instance path.
    pub instance: String,
    /// Whether the output rises (`direct`) or falls (`reverse`) while the measurement is below the setpoint.
    pub action: Action,
    /// The record the PI is configured from.
    pub parameter_record: String,
    /// The process measurement, which identification and the requirements use, not the corrected value the PID reads.
    pub process_variable: FormSignal,
    /// The controller output.
    pub manipulated_variable: FormSignal,
    /// The setpoint.
    pub setpoint: SignalChoice,
    /// The Auto/Manual signal.
    pub mode: ModeChoice,
    /// The manual output.
    pub manual_output: String,
    /// The predictor's model and correction variables.
    pub model: SmithModelVariables,
    /// The library block's variables, when the program uses `Control.SmithPredictorFOPDT`; absent for a program that keeps its own predictor.
    #[serde(default)]
    pub library_block: Option<SmithLibraryVariables>,
    /// The output's identification step; Identify chooses it when absent.
    pub experiment_step: Option<f64>,
    /// How long identification runs, in s; Identify chooses it when absent.
    pub experiment_duration_seconds: Option<f64>,
    /// The BOOL the program sets when its startup code has run.
    pub startup_flag: Option<String>,
}

/// The loop definition of a Smith predictor, ready for Identify, Tune, Validate and Apply (`smithPredictorLoopDefinition`, C9).
///
/// # Errors
/// The instance is not a compiled `Control.PID` in exactly one cyclic task; the predictor's variables are not distinct from each other and
/// from the loop's signals; a signal, a variable or the manual output is not compiled, or is not `REAL` or `LREAL`.
pub fn smith_loop_definition(
    catalog: &CompiledCatalog,
    scope: &LoopScope,
    draft: &SmithLoopDraft,
) -> Result<LoopDefinition, Failure> {
    let task = smith_task(catalog, draft)?;
    check_variables(draft)?;
    let process = stated_signal(&measured_signal(
        &draft.process_variable,
        "measurement",
        Some("processVariable"),
    )?);
    let manipulated = stated_signal(&measured_signal(
        &draft.manipulated_variable,
        "output",
        Some("manipulatedVariable"),
    )?);
    let setpoint = setpoint_signal(&draft.setpoint, &process);
    let manual = compiled(catalog, &draft.manual_output)?;
    if let Some(library) = &draft.library_block {
        compiled(catalog, &library.output)?;
        compiled(catalog, &library.maximum_correction)?;
    }
    check_startup(catalog, draft, &process)?;
    let states = smith_states(catalog, draft, &process, &manipulated)?;
    let application: Vec<InitializedState> = draft
        .startup_flag
        .as_deref()
        .filter(|flag| !flag.is_empty())
        .map(startup_state)
        .into_iter()
        .collect();
    let project =
        ControllerProject::from_value(&project_block(&smith_seed(draft), &states, &application))
            .map_err(|message| settings(&message))?;
    let id = loop_id(&draft.name);
    Ok(with_revision(LoopDefinition {
        schema_version: LOOP_SCHEMA_VERSION,
        id: id.clone(),
        name: draft.name.clone(),
        revision: "sha256:pending".to_string(),
        project_id: scope.project_id.clone(),
        runtime_id: scope.runtime_id.clone(),
        controller: smith_controller(catalog, draft, manual, task.interval_ms, project)?,
        signals: smith_signals(catalog, &setpoint, &process, &manipulated)?,
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
            parameters: smith_parameters(catalog, draft, &process, &manipulated)?,
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
        identification_policy: None,
        verification_scenarios: None,
        channels: None,
        analysis: None,
        analysis_parameter_symbols: None,
        cascade: None,
    }))
}

/// The one cyclic task of the compiled `Control.PID` the draft names (`smithPredictorGenerator.ts`, the first checks).
///
/// # Errors
/// No compiled `Control.PID` has the instance, or it does not run in exactly one cyclic task.
fn smith_task<'a>(
    catalog: &'a CompiledCatalog,
    draft: &SmithLoopDraft,
) -> Result<&'a CompiledTask, Failure> {
    let controller = catalog
        .controllers
        .iter()
        .find(|entry| {
            entry.instance == draft.instance && entry.type_name.to_uppercase() == PID_TYPE
        })
        .ok_or_else(|| {
            Failure::new(
                &format!("{} is not a compiled Control.PID instance.", draft.instance),
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            )
        })?;
    cyclic_task(controller)
}

/// The predictor's variables are distinct from each other and from the loop's signals (C9): the model's gain, time constant, delay and
/// correction, and the library block's four when the program names them.
///
/// # Errors
/// Two of the variables are the same.
fn check_variables(draft: &SmithLoopDraft) -> Result<(), Failure> {
    let mut owned = vec![
        draft.model.gain.as_str(),
        draft.model.time_constant.as_str(),
        draft.model.delay.as_str(),
        draft.model.correction.as_str(),
        draft.process_variable.symbol.as_str(),
        draft.manipulated_variable.symbol.as_str(),
    ];
    if let Some(library) = &draft.library_block {
        owned.push(library.output.as_str());
        owned.push(library.maximum_correction.as_str());
        owned.push(library.operating_input.as_str());
        owned.push(library.operating_output.as_str());
    }
    let distinct: BTreeSet<String> = owned.iter().map(|symbol| symbol.to_uppercase()).collect();
    if distinct.len() != owned.len() {
        return Err(settings(VARIABLES_DISTINCT));
    }
    Ok(())
}

/// The retained states of the `Control.PID` with the predictor's correction (C9): the PI's states, then the correction, which is bounded
/// by the measurement's span and starts at zero, so that the switch to Auto starts the predictor at zero.
///
/// # Errors
/// The correction is not compiled, or is not `REAL` or `LREAL`.
fn smith_states(
    catalog: &CompiledCatalog,
    draft: &SmithLoopDraft,
    process: &MeasuredSignal,
    manipulated: &MeasuredSignal,
) -> Result<Vec<InitializedState>, Failure> {
    let span = process.maximum - process.minimum;
    let correction = compiled(catalog, &draft.model.correction)?;
    let mut states = pid_states(&draft.instance, process, manipulated, 0);
    states.push(InitializedState {
        symbol: draft.model.correction.clone(),
        iec_type: state_type(real_type(correction)?),
        unit: stated_unit(&process.unit),
        minimum: -span,
        maximum: span,
        initial: InitialValue::Constant(0.0),
        post_scan: PostScan::Bounded,
    });
    Ok(states)
}

/// The seed that names the PI's three gains, the model's three values and, when the program uses the library block, its four variables
/// (C9). Its kind and keys are those `ControllerProject` reads (`smith_seed`).
fn smith_seed(draft: &SmithLoopDraft) -> Value {
    let record = draft.parameter_record.as_str();
    let mut seed = json!({
        "kind": SMITH_KIND,
        "family": "PI",
        "proportionalSymbol": format!("{record}.ProportionalGain"),
        "integralSymbol": format!("{record}.IntegralGain"),
        "derivativeSymbol": format!("{record}.DerivativeGain"),
        "gainSymbol": draft.model.gain,
        "timeConstantSymbol": draft.model.time_constant,
        "delaySymbol": draft.model.delay,
    });
    if let Some(library) = &draft.library_block {
        seed["predictorOutputSymbol"] = json!(library.output);
        seed["maximumCorrectionSymbol"] = json!(library.maximum_correction);
        seed["operatingInputSymbol"] = json!(library.operating_input);
        seed["operatingOutputSymbol"] = json!(library.operating_output);
    }
    seed
}

/// The program's predictor block as the loop file holds it (C9): its family, its declared states with the startup flag as an application
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
        "family": SMITH_FAMILY,
        "convention": "project-owned-native-controller",
        "initialization": initialization,
        "seed": seed,
        // One controlled output, so the verification campaign names the error requirement without a channel prefix.
        "axes": [],
        "objective": { "requirementId": "target-error", "direction": "minimize" },
        "maximumCandidates": 1,
        "maximumCycles": SMITH_MAXIMUM_CYCLES,
        "wallBudgetMs": SMITH_WALL_BUDGET_MS,
    })
}

/// The controller of a Smith predictor: a library `Control.PID` designed by the project, with its manual output and its project block (C9).
///
/// # Errors
/// The manual output is not `REAL` or `LREAL`.
fn smith_controller(
    catalog: &CompiledCatalog,
    draft: &SmithLoopDraft,
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

/// The signals of a Smith predictor (B1.4, C9): the setpoint is read, the output is written, and the measurement is read.
///
/// # Errors
/// A signal is not compiled, or is not `REAL` or `LREAL`.
fn smith_signals(
    catalog: &CompiledCatalog,
    setpoint: &MeasuredSignal,
    process: &MeasuredSignal,
    manipulated: &MeasuredSignal,
) -> Result<Signals, Failure> {
    Ok(Signals {
        setpoint: Some(binding(catalog, setpoint, Direction::Read)?),
        process_variable: measurement_binding(catalog, process)?,
        manipulated_variable: binding(catalog, manipulated, Direction::Write)?,
        actuator: None,
        additional: None,
    })
}

/// The parameters of the record: the PI's three gains with the output per the process, the predictor's model values with their bounds
/// (N10, C9), and, when the program uses the library block, the operating point with the bounds of the input and the output (D-18).
///
/// # Errors
/// A variable is not compiled, or is not `REAL` or `LREAL`.
fn smith_parameters(
    catalog: &CompiledCatalog,
    draft: &SmithLoopDraft,
    process: &MeasuredSignal,
    manipulated: &MeasuredSignal,
) -> Result<Vec<Parameter>, Failure> {
    let mut parameters: Vec<Parameter> =
        pid_parameters(&draft.parameter_record, process, manipulated)
            .into_iter()
            .filter(|parameter| PID_GAIN_CONVENTIONS.contains(&parameter.convention.as_str()))
            .collect();
    let gain_unit = format!("{}/{}", process.unit, manipulated.unit);
    parameters.push(model_parameter(
        catalog,
        &draft.model.gain,
        gain_unit,
        "Smith predictor model gain",
        -SMITH_GAIN_LIMIT,
        SMITH_GAIN_LIMIT,
    )?);
    parameters.push(model_parameter(
        catalog,
        &draft.model.time_constant,
        "s".to_owned(),
        "Smith predictor model time constant",
        SMITH_TIME_CONSTANT_MINIMUM_SECONDS,
        SMITH_TIME_CONSTANT_MAXIMUM_SECONDS,
    )?);
    parameters.push(model_parameter(
        catalog,
        &draft.model.delay,
        "s".to_owned(),
        "Smith predictor model delay",
        0.0,
        SMITH_DELAY_MAXIMUM_SECONDS,
    )?);
    if let Some(library) = &draft.library_block {
        parameters.push(model_parameter(
            catalog,
            &library.operating_input,
            manipulated.unit.clone(),
            "Smith predictor operating input",
            manipulated.minimum,
            manipulated.maximum,
        )?);
        parameters.push(model_parameter(
            catalog,
            &library.operating_output,
            process.unit.clone(),
            "Smith predictor operating output",
            process.minimum,
            process.maximum,
        )?);
    }
    Ok(parameters)
}

/// One predictor variable as a parameter: its own real type, its unit, its convention and its bounds.
///
/// # Errors
/// The variable is not compiled, or is not `REAL` or `LREAL`.
fn model_parameter(
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

/// A Smith predictor whose program writes a seeded value, or initializes its PID, needs the startup-done flag (C1 Setup). The seeded values
/// are the setpoint, the manual output, the mode, the parameter record and the PID's states, as for a PID loop. A draft that names a flag
/// must name a compiled `BOOL`.
///
/// # Errors
/// The program needs the flag, and the draft names none.
fn check_startup(
    catalog: &CompiledCatalog,
    draft: &SmithLoopDraft,
    process: &MeasuredSignal,
) -> Result<(), Failure> {
    if let Some(flag) = draft
        .startup_flag
        .as_deref()
        .filter(|flag| !flag.is_empty())
    {
        return startup_flag_is_bool(catalog, flag);
    }
    let measurement_is_own =
        process.symbol.to_uppercase() == own_measurement(&draft.instance).to_uppercase();
    let seeded = pid_seeded_symbols(
        &draft.instance,
        [
            draft.setpoint.symbol.as_str(),
            draft.manual_output.as_str(),
            draft.mode.symbol.as_str(),
        ],
        measurement_is_own,
    );
    let records = std::slice::from_ref(&draft.parameter_record);
    match startup_reason(catalog, Some(draft.instance.as_str()), &seeded, records) {
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

#[cfg(test)]
#[path = "smith_loop_tests.rs"]
mod tests;
