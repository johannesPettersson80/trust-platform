//! The loop definitions of a gain schedule (specification 35, C10 Setup and B1.4): one loop per operating point. Each is a `gain-scheduled`
//! project law whose `parallel-pid` seed names its point's two gain variables, and whose `schedule` block holds the whole table. Ported
//! from `gainScheduleGenerator.ts`; the scheduling variable, the values, the tolerances and the parameter record are what C10 adds.

#![allow(clippy::module_name_repetitions)] // `ScheduleLoopDraft` names the module's subject, as `OwnLawLoopDraft` does.

use serde::Deserialize;
use serde_json::{json, Value};

use super::catalog::{CompiledCatalog, CompiledSymbol, CompiledTask, PID_TYPE};
use super::loop_bindings::{
    binding, compiled, cyclic_task, explicit_experiment, loop_id, manual_output_type,
    measured_signal, measurement_binding, real_type, stated_signal, with_revision, FormSignal,
    MeasuredSignal,
};
use super::pid_bindings::{
    manual_output_tracking, pid_states, startup_flag_is_bool, startup_reason, startup_state,
};
use super::pid_loop::{setpoint_signal, ModeChoice, SignalChoice};
use super::texts::generators as texts;
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    Action, Controller, ControllerProject, Design, Direction, Execution, ExperimentKind,
    GainSchedule, Limits, LoopDefinition, LoopScope, ModeBinding, ModeWhenTrue, Parameter,
    ParameterBinding, SampleTiming, SchedulePoint, SchedulingVariable, Signals, Topology,
    LOOP_SCHEMA_VERSION,
};
use crate::tuning::structures::gain_schedule::{
    check_operating_points, check_point_order, check_tolerance, scheduled_gain_bounds,
    OperatingPointGains,
};

/// The family a schedule's project block names (C10).
const FAMILY: &str = "gain-scheduled";
/// The convention a program's own law declares (C2).
const CONVENTION: &str = "project-owned-native-controller";
/// The candidates of a single-design structure (N10, C6 to C10).
const CANDIDATES: u32 = 1;
/// The cycle budget of a project block (N10).
const CYCLE_BUDGET: u64 = 50_000;
/// The wall-clock budget of a project block, in milliseconds (N10).
const WALL_BUDGET_MS: u64 = 60_000;
/// The requirement that Tune ranks the schedule's candidates by (C2, C10).
const REQUIREMENT: &str = "target-error";

/// One operating point as the setup form confirmed it (`SchedulePointDraft`, C10 Setup).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchedulePointDraft {
    /// The point's name.
    pub name: String,
    /// The point's value of the scheduling variable, in its unit; absent when the form leaves it blank.
    #[serde(default)]
    pub value: Option<f64>,
    /// The tolerance on that value, in the scheduling variable's unit (D-21); absent when the form leaves it blank.
    #[serde(default)]
    pub tolerance: Option<f64>,
    /// The program's proportional gain variable for the point.
    pub proportional_gain: String,
    /// The program's integral gain variable for the point.
    pub integral_gain: String,
}

/// The scheduling variable as the setup form sends it: its path and its unit. Setup reads its IEC type from the compiled program.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchedulingVariableDraft {
    /// The variable's path.
    pub symbol: String,
    /// The variable's unit.
    pub unit: String,
}

/// A gain schedule as the engineer confirmed it (`ScheduleLoopDraft`, C10 Setup). Setup asks for no experiment step or duration, so
/// Identify chooses them (B3.3, B3.4), as for the other structures.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleLoopDraft {
    /// The schedule's name, from which each point's name and id are made.
    pub name: String,
    /// Whether the output rises (`direct`) or falls (`reverse`) while the measurement is below the setpoint.
    pub action: Action,
    /// The compiled `Control.PID` instance that the program interpolates the gains for.
    pub instance: String,
    /// The record the PID is configured from. Its `ProportionalGain` and `IntegralGain` hold the gains the program interpolates.
    pub parameter_record: String,
    /// The variable the schedule is scheduled by, and its unit.
    pub scheduling: SchedulingVariableDraft,
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
    /// The operating points, in the order of their values: two to four.
    pub points: Vec<SchedulePointDraft>,
    /// The identification experiment's output step; Identify chooses it when absent.
    pub experiment_step: Option<f64>,
    /// How long identification runs, in s; Identify chooses it when absent.
    pub experiment_duration_seconds: Option<f64>,
    /// The BOOL the program sets when its startup code has run.
    pub startup_flag: Option<String>,
}

/// What every point's loop shares: the task, the signals, the manual output and the table of the operating points (C10).
struct Shared<'a> {
    catalog: &'a CompiledCatalog,
    draft: &'a ScheduleLoopDraft,
    task: &'a CompiledTask,
    process_variable: MeasuredSignal,
    manipulated_variable: MeasuredSignal,
    setpoint: MeasuredSignal,
    manual: &'a CompiledSymbol,
    scheduling: SchedulingVariable,
    table: Vec<SchedulePoint>,
}

/// The loop definitions of a gain schedule (`gainScheduleLoopDefinitions`, C10): one per operating point, in the order the points are
/// given. Each loop's parameters are its point's two gain variables, its seed is that PI law (C2), and its `schedule` block holds the table.
///
/// # Errors
/// The instance is not a compiled `Control.PID` in exactly one cyclic task; the points break the count, name or symbol rules, are not
/// strictly increasing in the order given, or have a tolerance that is not above zero; the scheduling variable or a gain variable is not
/// compiled as `REAL` or `LREAL`; a signal is not compiled or is a measurement the program computes; or the program writes a seeded value
/// or initializes the PID and the draft names no startup-done flag.
pub fn schedule_loop_definitions(
    catalog: &CompiledCatalog,
    scope: &LoopScope,
    draft: &ScheduleLoopDraft,
) -> Result<Vec<LoopDefinition>, Failure> {
    let task = schedule_task(catalog, draft)?;
    check_points(draft)?;
    check_variables(catalog, draft)?;
    check_startup(catalog, draft)?;
    let shared = Shared::new(catalog, draft, task)?;
    (0..shared.table.len())
        .map(|index| point_loop(&shared, scope, index))
        .collect()
}

impl<'a> Shared<'a> {
    /// The shared parts of the schedule's loops.
    ///
    /// # Errors
    /// The manual output is not compiled.
    fn new(
        catalog: &'a CompiledCatalog,
        draft: &'a ScheduleLoopDraft,
        task: &'a CompiledTask,
    ) -> Result<Self, Failure> {
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
        let scheduling = scheduling_variable(catalog, draft)?;
        Ok(Self {
            catalog,
            draft,
            task,
            process_variable,
            manipulated_variable,
            setpoint,
            manual,
            scheduling,
            table: table(draft)?,
        })
    }
}

/// The compiled `Control.PID` of the draft and its one cyclic task (`gainScheduleLoopDefinitions`, the first checks).
///
/// # Errors
/// No compiled `Control.PID` has the instance, or it does not run in exactly one cyclic task.
fn schedule_task<'a>(
    catalog: &'a CompiledCatalog,
    draft: &ScheduleLoopDraft,
) -> Result<&'a CompiledTask, Failure> {
    let controller = catalog
        .controllers
        .iter()
        .find(|entry| {
            entry.instance == draft.instance && entry.type_name.to_uppercase() == PID_TYPE
        })
        .ok_or_else(|| {
            refuse_with(
                &texts::not_compiled_pid(&draft.instance),
                Cause::Implementation,
            )
        })?;
    cyclic_task(controller)
}

/// The points' count, names, symbols, order and tolerances (C10 Setup, `check_operating_points`, `check_point_order`,
/// `check_tolerance`). The order refusal names the first point out of order.
///
/// # Errors
/// A point breaks one of those rules.
fn check_points(draft: &ScheduleLoopDraft) -> Result<(), Failure> {
    let gains: Vec<OperatingPointGains> = draft
        .points
        .iter()
        .map(|point| OperatingPointGains {
            name: point.name.clone(),
            proportional_gain: point.proportional_gain.clone(),
            integral_gain: point.integral_gain.clone(),
        })
        .collect();
    check_operating_points(&gains).map_err(|refusal| refuse(&refusal.0))?;
    let numbers = draft
        .points
        .iter()
        .map(point_numbers)
        .collect::<Result<Vec<_>, _>>()?;
    let values: Vec<(&str, f64)> = draft
        .points
        .iter()
        .zip(&numbers)
        .map(|(point, (value, _))| (point.name.trim(), *value))
        .collect();
    check_point_order(&values).map_err(|refusal| refuse(&refusal.0))?;
    for (_, tolerance) in &numbers {
        check_tolerance(*tolerance).map_err(|refusal| refuse(&refusal.0))?;
    }
    Ok(())
}

/// The value and the tolerance of an operating point, which the form may leave blank: each blank one is refused with its sentence (C10
/// Setup). The form sends no row number, so the sentence names the operating point rather than a field.
///
/// # Errors
/// The point has no value, or no tolerance.
fn point_numbers(point: &SchedulePointDraft) -> Result<(f64, f64), Failure> {
    let name = point.name.trim();
    let value = point
        .value
        .ok_or_else(|| refuse(&texts::schedule_value_needed(name)))?;
    let tolerance = point
        .tolerance
        .ok_or_else(|| refuse(&texts::schedule_tolerance_needed(name)))?;
    Ok((value, tolerance))
}

/// The scheduling variable and every gain variable are compiled as `REAL` or `LREAL` (C10 Setup).
///
/// # Errors
/// A variable is not compiled, or is not `REAL` or `LREAL`.
fn check_variables(catalog: &CompiledCatalog, draft: &ScheduleLoopDraft) -> Result<(), Failure> {
    real_type(compiled(catalog, &draft.scheduling.symbol)?)?;
    for point in &draft.points {
        real_type(compiled(catalog, &point.proportional_gain)?)?;
        real_type(compiled(catalog, &point.integral_gain)?)?;
    }
    Ok(())
}

/// A program that writes a seeded value, or initializes the PID, needs the startup-done flag (C1 Setup). A draft that names a flag must name a compiled `BOOL`.
///
/// # Errors
/// The program needs the flag, and the draft names none.
fn check_startup(catalog: &CompiledCatalog, draft: &ScheduleLoopDraft) -> Result<(), Failure> {
    if let Some(flag) = startup_flag(draft) {
        return startup_flag_is_bool(catalog, flag);
    }
    let mut seeded = vec![
        draft.setpoint.symbol.clone(),
        draft.manual_output.clone(),
        draft.mode.symbol.clone(),
    ];
    seeded.extend(
        draft
            .points
            .iter()
            .flat_map(|point| [point.proportional_gain.clone(), point.integral_gain.clone()]),
    );
    let records = vec![draft.parameter_record.clone()];
    match startup_reason(catalog, Some(&draft.instance), &seeded, &records) {
        Some(reason) => Err(refuse(&texts::startup_flag_needed(&reason)).with_field("startupFlag")),
        None => Ok(()),
    }
}

/// The scheduling variable with the IEC type the compiled program gives it (C10 Setup): the runtime reads it only as that type.
///
/// # Errors
/// The variable is not compiled, or it is not `REAL` or `LREAL`.
fn scheduling_variable(
    catalog: &CompiledCatalog,
    draft: &ScheduleLoopDraft,
) -> Result<SchedulingVariable, Failure> {
    Ok(SchedulingVariable {
        symbol: draft.scheduling.symbol.clone(),
        unit: draft.scheduling.unit.clone(),
        iec_type: real_type(compiled(catalog, &draft.scheduling.symbol)?)?,
    })
}

/// The table of the operating points: each point's trimmed name, its loop's id (made from the schedule's name and the point's name, as the
/// TypeScript makes it), its value and tolerance, and its two gain variables.
fn table(draft: &ScheduleLoopDraft) -> Result<Vec<SchedulePoint>, Failure> {
    draft
        .points
        .iter()
        .map(|point| -> Result<SchedulePoint, Failure> {
            let (value, tolerance) = point_numbers(point)?;
            Ok(SchedulePoint {
                name: point.name.trim().to_owned(),
                loop_id: loop_id(&format!("{} {}", draft.name, point.name)),
                value,
                tolerance,
                proportional_symbol: point.proportional_gain.clone(),
                integral_symbol: point.integral_gain.clone(),
            })
        })
        .collect()
}

/// The loop of one operating point (`gainScheduleLoopDefinitions`, one `map` step): its name, its id, its controller, its signals, its
/// limits, its parameter binding with the point's two gains, and its experiment.
///
/// # Errors
/// The controller, a signal, a gain or the experiment does not generate (see the callers).
fn point_loop(
    shared: &Shared<'_>,
    scope: &LoopScope,
    index: usize,
) -> Result<LoopDefinition, Failure> {
    let draft = shared.draft;
    let point = &shared.table[index];
    let name = format!("{} · {}", draft.name, point.name);
    let schedule = GainSchedule {
        parameter_record: draft.parameter_record.clone(),
        scheduling: shared.scheduling.clone(),
        point: point.name.clone(),
        points: shared.table.clone(),
    };
    let experiment = explicit_experiment(
        &name,
        Execution::ControlStudio,
        ExperimentKind::MvStep,
        draft.experiment_step,
        draft.experiment_duration_seconds,
    )?;
    let id = point.loop_id.clone();
    Ok(with_revision(LoopDefinition {
        schema_version: LOOP_SCHEMA_VERSION,
        id: id.clone(),
        name,
        revision: "sha256:pending".to_string(),
        project_id: scope.project_id.clone(),
        runtime_id: scope.runtime_id.clone(),
        controller: point_controller(shared, point, &schedule)?,
        signals: point_signals(shared)?,
        limits: Limits {
            output_minimum: Some(shared.manipulated_variable.minimum),
            output_maximum: Some(shared.manipulated_variable.maximum),
            rate_up_per_s: None,
            rate_down_per_s: None,
            expected_settling_seconds: None,
        },
        observed_sample_timing: SampleTiming {
            nominal_period_ms: shared.task.interval_ms,
            maximum_jitter_ms: None,
        },
        parameter_binding: Some(ParameterBinding {
            loop_id: id,
            task_name: shared.task.name.clone(),
            mode: ModeBinding {
                symbol: draft.mode.symbol.clone(),
                manual_value: draft.mode.when_true == ModeWhenTrue::Manual,
            },
            parameters: gain_parameters(shared, point)?,
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

/// The controller of one point: a project-owned `Control.PID` whose project block is the schedule's (C10).
///
/// # Errors
/// The project block does not decode, or the manual output is not `REAL` or `LREAL`.
fn point_controller(
    shared: &Shared<'_>,
    point: &SchedulePoint,
    schedule: &GainSchedule,
) -> Result<Controller, Failure> {
    let draft = shared.draft;
    let block = project_block(shared, point, schedule);
    let project = ControllerProject::from_value(&block).map_err(|message| refuse(&message))?;
    Ok(Controller {
        instance: draft.instance.clone(),
        type_name: "Control.PID".to_string(),
        revision: "sha256:unverified".to_string(),
        action: draft.action,
        task_period_ms: shared.task.interval_ms,
        design: Some(Design::Project),
        law: None,
        mode_signal: Some(draft.mode.symbol.clone()),
        mode_when_true: Some(draft.mode.when_true),
        tracking_signal: None,
        manual_output_signal: Some(shared.manual.symbol.clone()),
        manual_output_address: shared
            .manual
            .address
            .clone()
            .filter(|address| !address.is_empty()),
        manual_output_type: Some(manual_output_type(real_type(shared.manual)?)),
        manual_output_follows_output: manual_output_tracking(
            shared.catalog,
            &draft.instance,
            &shared.manual.symbol,
        )
        .then_some(true),
        pid_initialization: None,
        state_feedback: None,
        on_off: None,
        project: Some(project),
        group: None,
    })
}

/// The project block of one point as the loop file holds it (C10): the schedule's family, the PID's states and the startup flag as an
/// application state (C1), the point's PI seed, one candidate, the requirement Tune ranks by, and the budgets of N10. The block carries the
/// whole table, so the decoder can check it.
fn project_block(shared: &Shared<'_>, point: &SchedulePoint, schedule: &GainSchedule) -> Value {
    let draft = shared.draft;
    let states = pid_states(
        &draft.instance,
        &shared.process_variable,
        &shared.manipulated_variable,
        0,
    );
    let mut initialization = json!({
        "kind": "declared-state-v1",
        "states": serde_json::to_value(&states).unwrap_or_default(),
    });
    if let Some(flag) = startup_flag(draft) {
        initialization["applicationStates"] =
            serde_json::to_value(vec![startup_state(flag)]).unwrap_or_default();
    }
    json!({
        "family": FAMILY,
        "convention": CONVENTION,
        "initialization": initialization,
        "seed": {
            "kind": "parallel-pid",
            "family": "PI",
            "proportionalSymbol": point.proportional_symbol,
            "integralSymbol": point.integral_symbol,
        },
        "axes": [],
        "objective": { "requirementId": REQUIREMENT, "direction": "minimize" },
        "maximumCandidates": CANDIDATES,
        "maximumCycles": CYCLE_BUDGET,
        "wallBudgetMs": WALL_BUDGET_MS,
        "schedule": schedule.to_value(),
    })
}

/// The signals of one point's loop: the setpoint is read, the measurement is read, and the output is written (`binding`).
///
/// # Errors
/// A signal is not compiled, is not `REAL` or `LREAL`, or is a measurement the program computes.
fn point_signals(shared: &Shared<'_>) -> Result<Signals, Failure> {
    Ok(Signals {
        setpoint: Some(binding(shared.catalog, &shared.setpoint, Direction::Read)?),
        process_variable: measurement_binding(shared.catalog, &shared.process_variable)?,
        manipulated_variable: binding(
            shared.catalog,
            &shared.manipulated_variable,
            Direction::Write,
        )?,
        actuator: None,
        additional: None,
    })
}

/// The two gains of one point as parameters: the proportional gain in the output's unit per the measurement's, the integral gain in that
/// per second; both bounded by the N10 magnitude (`gainScheduleLoopDefinitions`, `gain`).
///
/// # Errors
/// A gain variable is not compiled, or is not `REAL` or `LREAL`.
fn gain_parameters(shared: &Shared<'_>, point: &SchedulePoint) -> Result<Vec<Parameter>, Failure> {
    let output = shared.manipulated_variable.unit.as_str();
    let process = shared.process_variable.unit.as_str();
    Ok(vec![
        gain_parameter(
            shared.catalog,
            &point.proportional_symbol,
            format!("{output}/{process}"),
            &format!("Scheduled proportional gain at {}", point.name),
        )?,
        gain_parameter(
            shared.catalog,
            &point.integral_symbol,
            format!("{output}/({process}*s)"),
            &format!("Scheduled integral gain at {}", point.name),
        )?,
    ])
}

/// One gain as a parameter: its variable's real type, its unit, its convention and its N10 bounds.
///
/// # Errors
/// The gain's variable is not compiled, or it is not `REAL` or `LREAL`.
fn gain_parameter(
    catalog: &CompiledCatalog,
    symbol: &str,
    unit: String,
    convention: &str,
) -> Result<Parameter, Failure> {
    let bounds = scheduled_gain_bounds();
    Ok(Parameter {
        symbol: symbol.to_owned(),
        iec_type: real_type(compiled(catalog, symbol)?)?,
        unit,
        convention: convention.to_owned(),
        minimum: bounds.minimum,
        maximum: bounds.maximum,
    })
}

/// The startup-done flag the draft names, when it names one.
fn startup_flag(draft: &ScheduleLoopDraft) -> Option<&str> {
    draft
        .startup_flag
        .as_deref()
        .filter(|flag| !flag.is_empty())
}

/// A setup refusal that the setup form's next action answers (A11.2).
fn refuse(text: &str) -> Failure {
    refuse_with(text, Cause::Settings)
}

/// A refusal with its cause; the next action is the setup's, which the form answers.
fn refuse_with(text: &str, cause: Cause) -> Failure {
    Failure::new(text, cause, NextAction::OpenLoopSetup)
}

#[cfg(test)]
#[path = "schedule_loop_tests.rs"]
mod tests;
