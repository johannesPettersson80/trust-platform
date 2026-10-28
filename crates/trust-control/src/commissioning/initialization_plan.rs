//! The plans that an isolated run or a transfer starts from (specification 35, C0 "Controller state initialization", C1 "The
//! `parallel-pid-v1` initialization" and C13 "Continuity"; A6.3, A6.4; annex E6.4). Ported from `pidInitializationPlan.ts`
//! (`pidInitializationPlan`) and `proportionalTransfer.ts` (`proportionalInitializationPlan`). The plan of the declared states
//! is `declared_initialization::declared_initialization_plan`. Each plan is the run's sample grid, its starting values and its
//! events, with the requirements it checks. The runtime request around it, and the units of the observed symbols, belong to
//! the caller (as with `validation::engineer_verification_plan`).

use std::collections::{BTreeMap, BTreeSet};

use super::entry_allowance::{noise_aware_transfer_tolerance, AllowanceTerms};
use super::initial_state::{operating_input, operating_output};
use super::pid_operating_state::pid_operating_state;
use super::project_law::{application_states, entry_application_values};
use super::texts;
use super::transfer::{
    output_limits, parallel_pid_contract, proportional_parameter, proportional_transfer_binding,
    rounded_transfer_value, ParallelPidContract, TransferMember, TransferValueType,
};
use super::transfer_tracking::proportional_tracking_reference;
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    ControllerLaw, LoopDefinition, LoopSignal, ManualOutputType, ParameterBinding, PidLaw, PostScan,
};
use crate::model::GeneralModel;
use crate::validation::engineer_verification_plan::{
    engineer_verification_plan, EngineerRequirements, VerificationPlan,
};
use crate::validation::run_validation::{RunCheck, RunRequirement, Window};

/// A value that a run sets at a time of its run (annex E6.4 events: `at_ms`, `symbol`, `value`).
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedEvent {
    /// The time of the event, in ms from the start of the run.
    pub at_ms: u64,
    /// The variable that the event sets.
    pub symbol: String,
    /// The value it sets.
    pub value: f64,
}

/// The sample grid, the starting values and the events of a run, without its runtime request.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedRun {
    /// The task period, in ms: the time between two scans.
    pub step_ms: u64,
    /// The run's duration, in ms: six task periods, of which the transfer is after the third.
    pub duration_ms: u64,
    /// The scans the run executes: seven, at time zero and after each of the six periods.
    pub max_total_cycles: u64,
    /// The values the run starts from, by symbol.
    pub initial_values: BTreeMap<String, f64>,
    /// The symbols the run observes, each once.
    pub observed_symbols: Vec<String>,
    /// The values the run sets at the transfer.
    pub events: Vec<PlannedEvent>,
}

/// The plan of one start: the reference and the first-output allowance it uses, its run and the requirements it checks.
#[derive(Debug, Clone, PartialEq)]
pub struct StartPlan {
    /// The reference the loop starts from: the process value, or for a proportional start the aligned reference.
    pub reference: f64,
    /// The allowance of the first output after the entry (A6.4), in output units.
    pub tolerance: f64,
    /// The run.
    pub run: PlannedRun,
    /// The requirements the run checks.
    pub requirements: Vec<RunRequirement>,
}

/// The plan of a parallel PID start: its history at the operating point, and the start plan.
#[derive(Debug, Clone, PartialEq)]
pub struct ParallelPidStartPlan {
    /// The integral state at the operating point.
    pub integral_state: f64,
    /// The derivative history at the operating point.
    pub derivative_state: f64,
    /// The start plan.
    pub plan: StartPlan,
}

/// The timing of a start (`duration = step · 6`, `transferAt = step · 3`): the whole run, and the part after the transfer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct StartTiming {
    /// The task period, in ms.
    pub(super) step_ms: u64,
    /// The run's duration, in ms.
    pub(super) duration_ms: u64,
    /// The time of the transfer, in ms.
    pub(super) transfer_at_ms: u64,
    /// The whole run, in seconds.
    pub(super) window: Window,
    /// The run from the transfer to its end, in seconds.
    pub(super) transfer_window: Window,
    /// The run up to the first output after the transfer, in seconds: the Manual scans and the transfer's own scan, whose output
    /// A6.4 bounds. The law's response in the scans after it is the controller acting on its error, which A6.4 does not bound.
    pub(super) first_output_window: Window,
}

/// The timing of a start from its task period.
#[must_use]
pub(super) fn start_timing(step_ms: u64) -> StartTiming {
    let duration_ms = step_ms * 6;
    let transfer_at_ms = step_ms * 3;
    let window = Window {
        start: 0.0,
        end: seconds_of(duration_ms),
    };
    StartTiming {
        step_ms,
        duration_ms,
        transfer_at_ms,
        window,
        transfer_window: Window {
            start: seconds_of(transfer_at_ms),
            end: window.end,
        },
        // Up to the transfer's own sample: a run samples each scan after it ran, so that sample holds the first output after
        // the transfer, and a window's edges lie on the run's sample times (N8.4).
        first_output_window: Window {
            start: 0.0,
            end: seconds_of(transfer_at_ms),
        },
    }
}

/// An event that sets `symbol` to `value` at `at_ms`.
#[must_use]
pub(super) fn event(at_ms: u64, symbol: &str, value: f64) -> PlannedEvent {
    PlannedEvent {
        at_ms,
        symbol: symbol.to_owned(),
        value,
    }
}

/// The run of a start (the `TypeScript`'s `request`, without the runtime's own fields). The base plan's starting values are
/// kept; the mode starts in its Manual value; the start's own values are set; the observed symbols are the base plan's and then
/// `observed`; and the transfer's events are `events`, then the mode switches to Auto. Returns the run and the base plan's
/// requirements, which the caller windows.
#[must_use]
pub(super) fn start_run(
    timing: &StartTiming,
    base: VerificationPlan,
    mode: (&str, bool),
    starting: Vec<(String, f64)>,
    observed: Vec<String>,
    mut events: Vec<PlannedEvent>,
) -> (PlannedRun, Vec<RunRequirement>) {
    let (mode_symbol, manual_value) = mode;
    let mut initial_values = base.initial_values;
    initial_values.insert(mode_symbol.to_owned(), f64::from(u8::from(manual_value)));
    initial_values.extend(starting);
    let observed_symbols = unique_symbols(base.observed_symbols.into_iter().chain(observed));
    events.push(event(
        timing.transfer_at_ms,
        mode_symbol,
        f64::from(u8::from(!manual_value)),
    ));
    let run = PlannedRun {
        step_ms: timing.step_ms,
        duration_ms: timing.duration_ms,
        max_total_cycles: 7,
        initial_values,
        observed_symbols,
        events,
    };
    (run, base.requirements)
}

/// The bindings of a parallel PID start, checked: its contract, its law, its record, its setpoint and its Manual command.
struct PidBindings<'a> {
    contract: ParallelPidContract<'a>,
    law: &'a PidLaw,
    parameters: &'a ParameterBinding,
    reference: &'a LoopSignal,
    manual: &'a str,
    manual_type: TransferValueType,
}

/// The checked bindings of a parallel PID start (`pidInitializationPlan`, its first checks).
///
/// # Errors
/// The loop has no `parallel-pid-v1` contract, or its PID law, parameter record, setpoint or Manual command does not fit, or
/// the Manual command is a BOOL.
fn pid_bindings(loop_definition: &LoopDefinition) -> Result<PidBindings<'_>, Failure> {
    let contract = parallel_pid_contract(loop_definition)?;
    let (Some(ControllerLaw::Pid(law)), Some(parameters), Some(reference), Some(manual)) = (
        loop_definition.controller.law.as_ref(),
        loop_definition.parameter_binding.as_ref(),
        loop_definition.signals.setpoint.as_ref(),
        loop_definition.controller.manual_output_signal.as_ref(),
    ) else {
        return Err(incompatible_pid());
    };
    let manual_type = match loop_definition.controller.manual_output_type {
        Some(ManualOutputType::Real) => TransferValueType::Real,
        Some(ManualOutputType::Lreal) => TransferValueType::Lreal,
        Some(ManualOutputType::Bool) | None => return Err(incompatible_pid()),
    };
    Ok(PidBindings {
        contract,
        law,
        parameters,
        reference,
        manual,
        manual_type,
    })
}

/// The refusal of a PID whose adapter, record, setpoint or Manual command does not fit.
fn incompatible_pid() -> Failure {
    Failure::new(
        &texts::pid_state_incompatible(),
        Cause::ControllerCapability,
        NextAction::OpenLoopSetup,
    )
}

/// The values of a parallel PID start at the operating point, checked and rounded (`pidInitializationPlan`, its values).
struct PidStartPoint {
    /// The history of the PID at the operating point, by state symbol (`pidOperatingState`).
    history: BTreeMap<String, f64>,
    /// The integral state at the operating point.
    integral: f64,
    /// The derivative history at the operating point.
    derivative: f64,
    /// The applied output at the operating point.
    applied: f64,
    /// The process value at the operating point.
    process: f64,
    /// The Manual command at the start, rounded as it is typed and bounded by the output limits.
    initial_manual: f64,
    /// The proportional gain, which the allowance uses.
    proportional_gain: f64,
}

/// The values of a parallel PID start at the operating point, checked (`pidInitializationPlan`, its values).
///
/// # Errors
/// The proportional gain is not finite, the history does not fit its states, or the Manual command does not fit its range
/// after rounding.
fn pid_start_point(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    values: &BTreeMap<String, f64>,
    bindings: &PidBindings<'_>,
) -> Result<PidStartPoint, Failure> {
    let proportional_gain = values
        .get(&bindings.law.proportional_gain_symbol)
        .copied()
        .filter(|gain| gain.is_finite())
        .ok_or_else(|| {
            Failure::new(
                &texts::pid_state_gain_unavailable(),
                Cause::Settings,
                NextAction::Tune,
            )
        })?;
    let history = pid_operating_state(loop_definition, model, values)?;
    let integral = history
        .get(&bindings.contract.integral_state.symbol)
        .copied()
        .ok_or_else(incompatible_pid)?;
    let derivative = history
        .get(&bindings.contract.derivative_state.symbol)
        .copied()
        .ok_or_else(incompatible_pid)?;
    let applied = operating_input(model, 0);
    let process = operating_output(model, 0);
    let (minimum, maximum) = output_limits(loop_definition)?;
    let command = TransferMember {
        symbol: bindings.manual.to_owned(),
        iec_type: bindings.manual_type,
        unit: loop_definition.signals.manipulated_variable.unit.clone(),
        minimum,
        maximum,
    };
    let initial_manual = rounded_transfer_value(&command, applied).map_err(|_| {
        Failure::new(
            &texts::pid_state_outside_range(
                bindings.manual,
                applied,
                minimum,
                maximum,
                &command.unit,
            ),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        )
    })?;
    Ok(PidStartPoint {
        history,
        integral,
        derivative,
        applied,
        process,
        initial_manual,
        proportional_gain,
    })
}

/// The start plan of a parallel PID (`pidInitializationPlan`, C1): the run starts in Manual at the operating point, with the
/// setpoint at the process value, the Manual command at the applied output and the PID's history at the operating point, and
/// it enters Auto at the transfer. The allowance is that of A6.4 with the law's three gains: the `TypeScript` left out the
/// integral and derivative terms, which the specification requires (see the report).
///
/// # Errors
/// The loop has no `parallel-pid-v1` contract (Auto is then blocked); its PID law, parameter record, setpoint, mode or Manual
/// command do not fit (a BOOL Manual command is refused); the proportional gain is not finite; the Manual command does not fit
/// its range after rounding; or the run or the allowance is refused (see `engineer_verification_plan`).
pub fn pid_initialization_plan(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
    values: &BTreeMap<String, f64>,
    process_noise: Option<f64>,
) -> Result<ParallelPidStartPlan, Failure> {
    let bindings = pid_bindings(loop_definition)?;
    let point = pid_start_point(loop_definition, model, values, &bindings)?;
    let base = engineer_verification_plan(loop_definition, model, task, values, &point.history)?;
    let timing = start_timing(base.step_ms);
    let terms = AllowanceTerms {
        integral_gain: values.get(&bindings.law.integral_gain_symbol).copied(),
        derivative_gain: values.get(&bindings.law.derivative_gain_symbol).copied(),
        cycle_seconds: Some(loop_definition.controller.task_period_ms / 1000.0),
        entry_error: None,
    };
    let tolerance = noise_aware_transfer_tolerance(
        loop_definition,
        task.transfer_tolerance,
        Some(point.proportional_gain),
        process_noise,
        terms,
    )?;
    let mode = &bindings.parameters.mode;
    let auto = f64::from(u8::from(!mode.manual_value));
    let reference_symbol = bindings.reference.symbol.clone();
    let integral_symbol = bindings.contract.integral_state.symbol.clone();
    let derivative_symbol = bindings.contract.derivative_state.symbol.clone();
    let applied_symbol = loop_definition.signals.manipulated_variable.symbol.clone();
    let (run, base_requirements) = start_run(
        &timing,
        base,
        (mode.symbol.as_str(), mode.manual_value),
        vec![
            (reference_symbol.clone(), point.process),
            (bindings.manual.to_owned(), point.initial_manual),
            (integral_symbol.clone(), point.integral),
            (derivative_symbol.clone(), point.derivative),
        ],
        vec![
            mode.symbol.clone(),
            bindings.manual.to_owned(),
            integral_symbol.clone(),
            derivative_symbol.clone(),
        ],
        vec![event(
            timing.transfer_at_ms,
            &reference_symbol,
            point.process,
        )],
    );
    let mut requirements = vec![
        bounds(
            "transfer-continuity",
            &applied_symbol,
            timing.first_output_window,
            point.applied - tolerance,
            point.applied + tolerance,
        ),
        bounds(
            "auto-mode",
            &mode.symbol,
            timing.transfer_window,
            auto,
            auto,
        ),
        bounds(
            "integral-state",
            &integral_symbol,
            timing.window,
            point.integral,
            point.integral,
        ),
        bounds(
            "derivative-state",
            &derivative_symbol,
            timing.window,
            point.derivative,
            point.derivative,
        ),
    ];
    requirements.extend(bounds_on(base_requirements, timing.window));
    Ok(ParallelPidStartPlan {
        integral_state: point.integral,
        derivative_state: point.derivative,
        plan: StartPlan {
            reference: point.process,
            tolerance,
            run,
            requirements,
        },
    })
}

/// The start plan of a proportional commissioning (`proportionalInitializationPlan`, C13): the run starts in Manual with the
/// process value as the reference and the applied output as the command, and it enters Auto at the transfer with the reference
/// aligned so that the first output equals the held output. The allowance is `2·|Kp|·n` (C13, item 5) with the measured noise
/// `n`, or the exact check without a noise measurement; a reviewed tolerance replaces it.
///
/// # Errors
/// The loop has no proportional commissioning binding (see `proportional_transfer_binding`); the gain is not finite or zero
/// (the reference cannot be aligned); the reference or the command does not fit its bounds; the reviewed tolerance is outside
/// the output span; or the run is refused (see `engineer_verification_plan`).
pub fn proportional_initialization_plan(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
    values: &BTreeMap<String, f64>,
    process_noise: Option<f64>,
) -> Result<StartPlan, Failure> {
    let binding = proportional_transfer_binding(loop_definition)?;
    let parameter = proportional_parameter(loop_definition)?;
    // An absent gain is refused as a non-finite gain, as the TypeScript does.
    let gain = values.get(&parameter.symbol).copied().unwrap_or(f64::NAN);
    let applied = operating_input(model, 0);
    let process = operating_output(model, 0);
    let reference = proportional_tracking_reference(&binding, process, applied, gain)?;
    let base = engineer_verification_plan(loop_definition, model, task, values, &BTreeMap::new())?;
    let timing = start_timing(base.step_ms);
    let tolerance = noise_aware_transfer_tolerance(
        loop_definition,
        task.transfer_tolerance,
        Some(gain),
        process_noise,
        AllowanceTerms::default(),
    )?;
    let mode = &binding.mode;
    let auto = f64::from(u8::from(!mode.manual_value));
    let reference_member = &binding.states[0];
    let command_member = &binding.states[1];
    let command_value = rounded_transfer_value(command_member, applied)?;
    // The application states take their declared values, and the same transfer writes them with the reference and the mode (C1,
    // C13 Apply step 4). The run starts from those values, as Validate starts the flag TRUE.
    let application = entry_application_values(loop_definition, &binding)?;
    let mut starting = vec![
        (reference_member.symbol.clone(), process),
        (command_member.symbol.clone(), command_value),
    ];
    starting.extend(
        application
            .iter()
            .map(|(symbol, value)| (symbol.clone(), *value)),
    );
    let mut written = vec![event(
        timing.transfer_at_ms,
        &reference_member.symbol,
        reference,
    )];
    written.extend(
        application
            .iter()
            .map(|(symbol, value)| event(timing.transfer_at_ms, symbol, *value)),
    );
    let observed: Vec<String> = std::iter::once(mode.symbol.clone())
        .chain(application.keys().cloned())
        .collect();
    let (run, base_requirements) = start_run(
        &timing,
        base,
        (mode.symbol.as_str(), mode.manual_value),
        starting,
        observed,
        written,
    );
    let mut requirements = vec![
        bounds(
            "transfer-continuity",
            &binding.output.symbol,
            timing.first_output_window,
            applied - tolerance,
            applied + tolerance,
        ),
        bounds(
            "auto-mode",
            &mode.symbol,
            timing.transfer_window,
            auto,
            auto,
        ),
    ];
    requirements.extend(application_requirements(
        loop_definition,
        &timing,
        &application,
    ));
    requirements.extend(bounds_on(base_requirements, timing.window));
    Ok(StartPlan {
        reference,
        tolerance,
        run,
        requirements,
    })
}

/// The requirements that each application state of a program's own law holds its value after the entry (`initialized-state:`, C1): a
/// state the program does not move is held to its value, and a bounded one stays within its bounds, as a declared initialization
/// checks its states.
fn application_requirements(
    loop_definition: &LoopDefinition,
    timing: &StartTiming,
    values: &BTreeMap<String, f64>,
) -> Vec<RunRequirement> {
    application_states(loop_definition)
        .into_iter()
        .filter_map(|state| {
            let value = *values.get(&state.symbol)?;
            let (minimum, maximum) = match state.post_scan {
                PostScan::Unchanged => (value, value),
                PostScan::Bounded => (state.minimum, state.maximum),
            };
            Some(RunRequirement {
                id: format!("initialized-state:{}", state.symbol),
                channel: state.symbol.clone(),
                window: timing.transfer_window,
                check: RunCheck::StateBounds { minimum, maximum },
            })
        })
        .collect()
}

/// A run's whole milliseconds as seconds, as the `TypeScript` divides them (`ms / 1000`). A run lasts at most 24 h, far below
/// 2⁵³ ms, so the conversion is exact.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub(super) fn seconds_of(milliseconds: u64) -> f64 {
    milliseconds as f64 / 1000.0
}

/// A requirement that bounds `channel` over `window` (`kind: "bounds"`, N8.3).
#[must_use]
pub(super) fn bounds(
    id: &str,
    channel: &str,
    window: Window,
    minimum: f64,
    maximum: f64,
) -> RunRequirement {
    RunRequirement {
        id: id.to_owned(),
        channel: channel.to_owned(),
        window,
        check: RunCheck::Bounds { minimum, maximum },
    }
}

/// The bounds of a base plan, each over `window`: the base plan's own bounds are kept, and the run's window replaces theirs
/// (`filter(kind === "bounds").map(window)`).
#[must_use]
pub(super) fn bounds_on(requirements: Vec<RunRequirement>, window: Window) -> Vec<RunRequirement> {
    requirements
        .into_iter()
        .filter(|requirement| matches!(requirement.check, RunCheck::Bounds { .. }))
        .map(|requirement| RunRequirement {
            window,
            ..requirement
        })
        .collect()
}

/// The symbols, each once, in the order they first appear (the `TypeScript`'s map of observed signals).
#[must_use]
pub(super) fn unique_symbols(symbols: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    symbols
        .into_iter()
        .filter(|symbol| seen.insert(symbol.clone()))
        .collect()
}

#[cfg(test)]
#[path = "initialization_plan_tests.rs"]
mod tests;
