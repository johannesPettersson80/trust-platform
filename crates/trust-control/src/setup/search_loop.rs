//! The loop definition of a law tuned by a finite search (specification 35, C5, B1.4 and §17.1): a project-owned law whose coefficients
//! no formula designs. Ported from `searchLawLoopGenerator.ts` and from the search choices of `loopSetupForm.ts`. Tune tries every
//! combination of the coefficient values on the project's own ST (C5); the loop declares the values, the kept values and the objective.

#![allow(clippy::module_name_repetitions)] // `SearchLawLoopDraft` names the module's subject, as the TypeScript does.

use serde::Deserialize;
use serde_json::{json, Value};

use super::catalog::CompiledCatalog;
use super::loop_bindings::{
    binding, compiled, cyclic_task, explicit_experiment, manual_output_type, measurement_binding,
    real_type, state_type, FormSignal, MeasuredSignal,
};
use super::pid_bindings::startup_state;
use super::pid_loop::{setpoint_signal, ModeChoice, SignalChoice};
use super::state_feedback_loop::{
    assemble, check_startup, form_signal, instance_program, json_of, mode_and_manual, mode_binding,
    output_limits, settings, StateLawParts,
};
use super::texts::save as texts;
use crate::failure::Failure;
use crate::loop_definition::{
    Action, Controller, ControllerProject, Design, Direction, Execution, ExperimentKind,
    InitialSource, InitialValue, InitializedState, LoopDefinition, LoopScope, Parameter, PostScan,
    RealType, Signals,
};

/// The families a program's law may name when Tune searches it (`SEARCH_FAMILIES`, C5).
pub(super) const SEARCH_FAMILIES: [&str; 12] = [
    "MPC",
    "ratio",
    "split-range",
    "override",
    "gain-scheduled",
    "feedforward",
    "Smith",
    "lead-lag",
    "adaptive",
    "robust",
    "feedback-linearization",
    "observer-feedback",
];

/// The convention of a project-owned law (`decodeProjectController`).
const PROJECT_CONVENTION: &str = "project-owned-native-controller";

/// The most coefficients a search tries (`searchLawLoopGenerator.ts`, C5).
pub(super) const MAXIMUM_COEFFICIENTS: usize = 4;

/// The most values the program keeps from scan to scan (`searchLawLoopGenerator.ts`, C5).
pub(super) const MAXIMUM_KEPT_VALUES: usize = 3;

/// The most combinations a search tries (C5, A7).
const MAXIMUM_COMBINATIONS: usize = 64;

/// The most cycles one trial runs (`searchLawLoopGenerator.ts`; the `TypeScript`'s budget, A7).
const MAXIMUM_CYCLES: u64 = 50_000;

/// The wall time of the search, in ms (`searchLawLoopGenerator.ts`; the `TypeScript`'s budget, A7).
const WALL_BUDGET_MS: u64 = 60_000;

/// One coefficient the form sends: its variable, its unit, and the values Tune may try as the engineer typed them, separated by commas,
/// semicolons or spaces (`loopSetupForm.ts`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchCoefficient {
    /// The coefficient's variable; blank when the row is not used.
    #[serde(default)]
    pub symbol: String,
    /// The coefficient's unit.
    #[serde(default)]
    pub unit: String,
    /// The values Tune may try, as the engineer typed them.
    #[serde(default)]
    pub values: String,
}

/// A value the program keeps from scan to scan: its variable, its unit and the start that validation uses (C5).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeptValue {
    /// The variable; blank when the row is not used.
    #[serde(default)]
    pub symbol: String,
    /// The variable's unit.
    #[serde(default)]
    pub unit: String,
    /// Where validation starts the value.
    #[serde(default)]
    pub start: KeptStart,
}

/// Where validation starts a kept value: at zero, at the output, or at the measurement (C5, `RetainedValue`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeptStart {
    /// Zero.
    #[default]
    Zero,
    /// The output, through the model input at its operating point.
    Output,
    /// The measurement, through the model output at its operating point.
    Measurement,
}

/// A law tuned by a finite search, as the setup form confirms it (`SearchLawLoopDraft`, C5). Setup asks for no experiment step or
/// duration, so both may be absent, and Identify chooses them (B3.3, B3.4).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchLawLoopDraft {
    /// The loop's name, from which its id is made.
    #[serde(default)]
    pub name: String,
    /// Whether the output rises (`direct`) or falls (`reverse`) while the measurement is below the setpoint.
    pub action: Action,
    /// The kind of law the program computes (`SEARCH_FAMILIES`).
    #[serde(default)]
    pub family: String,
    /// The compiled program or function block instance that computes the law.
    #[serde(default)]
    pub instance: String,
    /// The measurement.
    pub process_variable: FormSignal,
    /// The setpoint.
    pub setpoint: SignalChoice,
    /// The controller output.
    pub manipulated_variable: FormSignal,
    /// The Auto/Manual signal, and the mode its TRUE value means.
    pub mode: ModeChoice,
    /// The manual output.
    #[serde(default)]
    pub manual_output: String,
    /// The coefficients Tune may search, with the values it may try.
    #[serde(default)]
    pub coefficients: Vec<SearchCoefficient>,
    /// The values the program keeps from scan to scan.
    #[serde(default)]
    pub states: Vec<KeptValue>,
    /// The identification experiment's output step; Identify chooses it when absent.
    pub experiment_step: Option<f64>,
    /// How long identification runs, in s; Identify chooses it when absent.
    pub experiment_duration_seconds: Option<f64>,
    /// The BOOL the program sets when its startup code has run.
    pub startup_flag: Option<String>,
}

/// A coefficient the form has checked: its variable, its unit and the values it may try, as typed.
struct CoefficientChoice {
    symbol: String,
    unit: String,
    values: Vec<f64>,
}

/// A coefficient the search tries, with its real type and the values it tries in the order listed, each once.
struct Axis {
    symbol: String,
    real: RealType,
    unit: String,
    values: Vec<f64>,
}

/// The loop definition of a law tuned by a finite search (`searchLawLoopDefinition`): a project-owned controller whose coefficients
/// Tune tries in every combination, its kept values started as validation starts them, and its objective.
///
/// # Errors
/// The form leaves a choice unset or breaks one of its rules (names, signals, mode, coefficients, values, combinations, kept values),
/// the instance is not a compiled program or function block that runs in exactly one cyclic task, the family is not one Tune searches,
/// a variable is not compiled or is not `REAL` or `LREAL`, the variables are not distinct, or the program needs a startup-done flag the
/// draft does not name.
pub fn search_loop_definition(
    catalog: &CompiledCatalog,
    scope: &LoopScope,
    draft: &SearchLawLoopDraft,
) -> Result<LoopDefinition, Failure> {
    let (process, manipulated) = form_choices(draft)?;
    let coefficients = coefficient_choices(draft)?;
    let kept = kept_choices(draft)?;
    check_startup(
        catalog,
        &draft.instance,
        &seeded(draft, &coefficients),
        draft.startup_flag.as_deref(),
    )?;
    search_law(
        catalog,
        scope,
        draft,
        &process,
        &manipulated,
        &coefficients,
        &kept,
    )
}

/// The name, the instance, the measurement, the setpoint, the output and the mode, checked in the order of `loopSetupForm.ts`.
///
/// # Errors
/// One of those choices is missing or breaks its rule.
fn form_choices(draft: &SearchLawLoopDraft) -> Result<(MeasuredSignal, MeasuredSignal), Failure> {
    if draft.name.trim().is_empty() {
        return Err(settings(texts::name_needed()).with_field("name"));
    }
    if draft.instance.trim().is_empty() {
        return Err(settings(texts::search_instance_needed()).with_field("search.instance"));
    }
    let process = form_signal(
        &draft.process_variable,
        "measurement",
        Some("search.processVariable"),
    )?;
    if draft.setpoint.symbol.trim().is_empty() {
        return Err(settings(texts::setpoint_needed()).with_field("search.setpoint"));
    }
    let manipulated = form_signal(
        &draft.manipulated_variable,
        "output",
        Some("search.manipulatedVariable"),
    )?;
    mode_and_manual("search", &draft.mode, &draft.manual_output)?;
    Ok((process, manipulated))
}

/// The coefficients the form sends with a variable: each with its unit and its values, at least one, and at most 64 combinations.
///
/// # Errors
/// A coefficient has no unit or no readable value list, none is chosen, or the values make more combinations than Tune tries.
fn coefficient_choices(draft: &SearchLawLoopDraft) -> Result<Vec<CoefficientChoice>, Failure> {
    let mut coefficients = Vec::new();
    for entry in draft
        .coefficients
        .iter()
        .filter(|entry| !entry.symbol.trim().is_empty())
    {
        if entry.unit.trim().is_empty() {
            return Err(settings(&texts::search_unit_needed(&entry.symbol)));
        }
        let Some(values) = parse_values(&entry.values) else {
            return Err(settings(&texts::search_values_unreadable(&entry.symbol)));
        };
        coefficients.push(CoefficientChoice {
            symbol: entry.symbol.clone(),
            unit: entry.unit.clone(),
            values,
        });
    }
    if coefficients.is_empty() {
        return Err(settings(texts::search_coefficient_needed()));
    }
    let combinations = combination_count(
        coefficients
            .iter()
            .map(|coefficient| coefficient.values.len()),
    );
    if combinations > MAXIMUM_COMBINATIONS {
        return Err(settings(&texts::search_combinations(
            combinations,
            MAXIMUM_COMBINATIONS,
        )));
    }
    Ok(coefficients)
}

/// The values a coefficient's text lists, separated by commas, semicolons or spaces (`loopSetupForm.ts`). `None` when the list is empty
/// or one of its entries is not a finite number.
fn parse_values(text: &str) -> Option<Vec<f64>> {
    let mut values = Vec::new();
    for entry in text
        .split(|character: char| character == ',' || character == ';' || character.is_whitespace())
        .filter(|entry| !entry.is_empty())
    {
        values.push(
            entry
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())?,
        );
    }
    (!values.is_empty()).then_some(values)
}

/// The number of combinations of the value lists: their product, saturating (C5).
fn combination_count(lengths: impl Iterator<Item = usize>) -> usize {
    lengths.fold(1, usize::saturating_mul)
}

/// The kept values the form sends with a variable: each with its unit (`loopSetupForm.ts`).
///
/// # Errors
/// A kept value has no unit.
fn kept_choices(draft: &SearchLawLoopDraft) -> Result<Vec<KeptValue>, Failure> {
    let mut kept = Vec::new();
    for entry in draft
        .states
        .iter()
        .filter(|entry| !entry.symbol.trim().is_empty())
    {
        if entry.unit.trim().is_empty() {
            return Err(settings(&texts::search_unit_needed(&entry.symbol)));
        }
        kept.push(entry.clone());
    }
    Ok(kept)
}

/// The symbols that validation seeds for a searched law, in the order of `loopSetupForm.ts`'s `seededWrite`.
fn seeded(draft: &SearchLawLoopDraft, coefficients: &[CoefficientChoice]) -> Vec<String> {
    let mut seeded = vec![draft.setpoint.symbol.clone()];
    seeded.extend(
        coefficients
            .iter()
            .map(|coefficient| coefficient.symbol.clone()),
    );
    seeded.push(draft.manual_output.clone());
    seeded.push(draft.mode.symbol.clone());
    seeded
}

/// The searched law's loop, after the generator's checks (`searchLawLoopDefinition`).
///
/// # Errors
/// The instance, the family, the counts, the variables, the values or the combinations break a rule of the search, or a variable is not
/// compiled or is not `REAL` or `LREAL`.
fn search_law(
    catalog: &CompiledCatalog,
    scope: &LoopScope,
    draft: &SearchLawLoopDraft,
    process: &MeasuredSignal,
    manipulated: &MeasuredSignal,
    coefficients: &[CoefficientChoice],
    kept: &[KeptValue],
) -> Result<LoopDefinition, Failure> {
    let program = instance_program(catalog, &draft.instance)?;
    let task = cyclic_task(program)?;
    if !SEARCH_FAMILIES.contains(&draft.family.as_str()) {
        return Err(settings(texts::search_family_needed()).with_field("search.family"));
    }
    if coefficients.len() > MAXIMUM_COEFFICIENTS {
        return Err(settings(texts::search_coefficient_limit()));
    }
    if kept.len() > MAXIMUM_KEPT_VALUES {
        return Err(settings(texts::search_state_limit()));
    }
    if !distinct_variables(coefficients, kept) {
        return Err(settings(texts::search_variables_distinct()));
    }
    let axes = search_axes(catalog, coefficients)?;
    let combinations = combination_count(axes.iter().map(|axis| axis.values.len()));
    if combinations > MAXIMUM_COMBINATIONS {
        return Err(settings(&texts::search_combinations(
            combinations,
            MAXIMUM_COMBINATIONS,
        )));
    }
    let manual = compiled(catalog, &draft.manual_output)?;
    let setpoint = setpoint_signal(&draft.setpoint, process);
    let states = kept_states(catalog, kept, manipulated, process)?;
    let startup = draft
        .startup_flag
        .as_deref()
        .filter(|flag| !flag.is_empty())
        .map(startup_state);
    let project = project_block(&draft.family, &states, startup, &axes)?;
    let controller = Controller {
        instance: draft.instance.clone(),
        type_name: program.type_name.clone(),
        revision: "sha256:unverified".to_string(),
        action: draft.action,
        task_period_ms: task.interval_ms,
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
        project: Some(project),
        group: None,
    };
    let signals = Signals {
        setpoint: Some(binding(catalog, &setpoint, Direction::Read)?),
        process_variable: measurement_binding(catalog, process)?,
        manipulated_variable: binding(catalog, manipulated, Direction::Write)?,
        actuator: None,
        additional: None,
    };
    let experiment = explicit_experiment(
        &draft.name,
        Execution::ControlStudio,
        ExperimentKind::MvStep,
        draft.experiment_step,
        draft.experiment_duration_seconds,
    )?;
    Ok(assemble(
        scope,
        StateLawParts {
            name: draft.name.clone(),
            controller,
            signals,
            channels: None,
            identification_policy: None,
            limits: output_limits(manipulated),
            period_ms: task.interval_ms,
            task_name: task.name.clone(),
            mode: mode_binding(&draft.mode),
            parameters: axis_parameters(&axes),
            experiment,
        },
    ))
}

/// Whether the searched coefficients and the kept values name different variables, ignoring case (`searchLawLoopGenerator.ts`).
fn distinct_variables(coefficients: &[CoefficientChoice], kept: &[KeptValue]) -> bool {
    let mut names: Vec<String> = coefficients
        .iter()
        .map(|coefficient| coefficient.symbol.to_uppercase())
        .chain(kept.iter().map(|value| value.symbol.to_uppercase()))
        .collect();
    let count = names.len();
    names.sort();
    names.dedup();
    names.len() == count
}

/// The coefficients the search tries, each with its real type and its values. A `REAL` variable holds the nearest single-precision
/// value, so that is the value Tune tries, and each value is listed once (`searchLawLoopGenerator.ts`).
///
/// # Errors
/// A variable is not compiled or is not `REAL` or `LREAL`, or a value is not finite once it is rounded to its type.
#[allow(clippy::cast_possible_truncation)] // A REAL holds the nearest single-precision value, which is what Tune tries.
fn search_axes(
    catalog: &CompiledCatalog,
    coefficients: &[CoefficientChoice],
) -> Result<Vec<Axis>, Failure> {
    let mut axes = Vec::with_capacity(coefficients.len());
    for coefficient in coefficients {
        let real = real_type(compiled(catalog, &coefficient.symbol)?)?;
        let mut values: Vec<f64> = Vec::new();
        for value in &coefficient.values {
            let value = if matches!(real, RealType::Real) {
                f64::from(*value as f32)
            } else {
                *value
            };
            if !value.is_finite() {
                return Err(settings(&texts::search_values_needed(&coefficient.symbol)));
            }
            if !values.contains(&value) {
                values.push(value);
            }
        }
        axes.push(Axis {
            symbol: coefficient.symbol.clone(),
            real,
            unit: coefficient.unit.clone(),
            values,
        });
    }
    Ok(axes)
}

/// The kept values as declared states: each starts as validation starts it, and is bounded about the output's or the measurement's
/// range, or about zero (`searchLawLoopDefinition`'s `retained`).
///
/// # Errors
/// A kept value is not compiled, or it is not `REAL` or `LREAL`.
fn kept_states(
    catalog: &CompiledCatalog,
    kept: &[KeptValue],
    manipulated: &MeasuredSignal,
    process: &MeasuredSignal,
) -> Result<Vec<InitializedState>, Failure> {
    let output_span = manipulated.maximum - manipulated.minimum;
    let measurement_span = process.maximum - process.minimum;
    let mut states = Vec::with_capacity(kept.len());
    for value in kept {
        let (minimum, maximum, initial) = match value.start {
            KeptStart::Output => (
                manipulated.minimum - 10.0 * output_span,
                manipulated.maximum + 10.0 * output_span,
                InitialValue::From(InitialSource::ModelInput { index: 0 }),
            ),
            KeptStart::Measurement => (
                process.minimum - measurement_span,
                process.maximum + measurement_span,
                InitialValue::From(InitialSource::ModelOutput { index: 0 }),
            ),
            KeptStart::Zero => (-1e9, 1e9, InitialValue::Constant(0.0)),
        };
        states.push(InitializedState {
            symbol: value.symbol.clone(),
            iec_type: state_type(real_type(compiled(catalog, &value.symbol)?)?),
            unit: value.unit.clone(),
            minimum,
            maximum,
            initial,
            post_scan: PostScan::Bounded,
        });
    }
    Ok(states)
}

/// The program's own law as `controller.project` holds it (C5): its family, its kept values with the startup-done flag as an
/// application state (C1, C5 Setup), its axes, its objective and its budgets. It is read back through `ControllerProject::from_value`.
///
/// # Errors
/// The block breaks a rule of the project contract (for example, the kept values and the flag name one variable).
fn project_block(
    family: &str,
    states: &[InitializedState],
    startup: Option<InitializedState>,
    axes: &[Axis],
) -> Result<ControllerProject, Failure> {
    let application: Vec<InitializedState> = startup.into_iter().collect();
    let mut initialization = json!({ "kind": "declared-state-v1", "states": json_of(states) });
    if !application.is_empty() {
        initialization["applicationStates"] = json_of(&application);
    }
    let axes: Vec<Value> = axes
        .iter()
        .map(|axis| json!({ "symbol": axis.symbol, "values": axis.values }))
        .collect();
    ControllerProject::from_value(&json!({
        "family": family,
        "convention": PROJECT_CONVENTION,
        "initialization": initialization,
        "axes": axes,
        // One controlled output: its verification campaign names the error requirement without a channel prefix.
        "objective": { "requirementId": "target-error", "direction": "minimize" },
        "maximumCandidates": MAXIMUM_COMBINATIONS,
        "maximumCycles": MAXIMUM_CYCLES,
        "wallBudgetMs": WALL_BUDGET_MS,
    }))
    .map_err(|message| settings(&message))
}

/// The coefficients as parameters of the record: each with its own variable's type, its unit, its last name segment as its convention,
/// and the smallest and largest value it tries (C0, C5).
fn axis_parameters(axes: &[Axis]) -> Vec<Parameter> {
    axes.iter()
        .map(|axis| Parameter {
            symbol: axis.symbol.clone(),
            iec_type: axis.real,
            unit: axis.unit.clone(),
            convention: axis
                .symbol
                .rsplit('.')
                .next()
                .unwrap_or(axis.symbol.as_str())
                .to_string(),
            minimum: axis.values.iter().copied().fold(f64::INFINITY, f64::min),
            maximum: axis
                .values
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max),
        })
        .collect()
}

#[cfg(test)]
#[path = "search_loop_tests.rs"]
mod tests;
