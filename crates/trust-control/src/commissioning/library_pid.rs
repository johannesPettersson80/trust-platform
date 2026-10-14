//! The initialization of the shipped `Control.PID` (specification 35, C1 "The `control-pid-v1` state initialization"): its
//! seven retained states, each bound to its declared operating-point coordinate, and the application states that accompany
//! them. Ported from `libraryPidInitialization.ts` (`libraryPidInitialization`, `assertLibraryPidStates`). The record is
//! `PidInitialization::ControlPid`, and the loop file checks it at load (`loop_definition::checks`). A PID group's members
//! (C7) are parked and not ported.

use std::collections::BTreeSet;

use super::initial_state::ControllerInitialization;
use super::texts;
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    ControllerLaw, InitialSource, InitialValue, InitializedState, LoopDefinition,
    PidInitialization, PostScan, StateType,
};

/// The longest snapshot age an Auto entry may use, in ms (C1: at most 10 s).
const MAXIMUM_SNAPSHOT_AGE_MS: u64 = 10_000;

/// The initialization of a `Control.PID` loop (`libraryPidInitialization`): the seven retained states, then the application
/// states of the owning program, validated as one declared state list.
///
/// # Errors
/// The loop does not declare `control-pid-v1` with a setpoint, a PID law and a parameter record; its snapshot age is shorter
/// than a task period or longer than 10 s; a retained state is not bound as C1 requires; or the application states are not
/// unique or do not belong to the controller's owning program.
pub fn library_pid_initialization(
    loop_definition: &LoopDefinition,
) -> Result<ControllerInitialization, Failure> {
    let declare = || {
        Failure::new(
            &texts::library_pid_declare_seven(),
            Cause::ControllerCapability,
            NextAction::OpenLoopSetup,
        )
    };
    let Some(PidInitialization::ControlPid {
        maximum_snapshot_age_ms,
        states,
        application_states,
    }) = loop_definition.controller.pid_initialization.as_ref()
    else {
        return Err(declare());
    };
    let Some(setpoint) = loop_definition.signals.setpoint.as_ref() else {
        return Err(declare());
    };
    let has_pid_law = matches!(loop_definition.controller.law, Some(ControllerLaw::Pid(_)));
    if !has_pid_law
        || loop_definition.parameter_binding.is_none()
        || !snapshot_age_fits(
            *maximum_snapshot_age_ms,
            loop_definition.controller.task_period_ms,
        )
    {
        return Err(declare());
    }
    assert_library_pid_states(
        &loop_definition.controller.instance,
        states,
        &setpoint.unit,
        &loop_definition.signals.manipulated_variable.unit,
    )?;
    application_states_check(loop_definition, states, application_states.as_deref())
}

/// Checks the seven retained states of a `Control.PID` (`assertLibraryPidStates`, the scalar loop): each member is bound to
/// its own symbol `<instance>.StateValue.<member>`, is LREAL with its unit, is bounded after the scan, has a range, and is
/// initialized to its declared coordinate: the process coordinate for the setpoint and the process value, the actuator
/// coordinate for the feedforward and the outputs, and zero for the rates and the feedforward.
///
/// # Errors
/// The list is not exactly seven states, or a member is missing or bound in a way that C1 does not allow.
pub fn assert_library_pid_states(
    instance: &str,
    states: &[InitializedState],
    process_unit: &str,
    actuator_unit: &str,
) -> Result<(), Failure> {
    let expected = expected_members(process_unit, actuator_unit);
    if states.len() != expected.len() {
        return Err(Failure::new(
            &texts::library_pid_seven_states(instance),
            Cause::ControllerCapability,
            NextAction::OpenLoopSetup,
        ));
    }
    for (member, unit, initial) in expected {
        let symbol = format!("{instance}.StateValue.{member}");
        let fits = states
            .iter()
            .find(|state| state.symbol == symbol)
            .is_some_and(|state| {
                state.iec_type == StateType::Lreal
                    && state.unit == unit
                    && state.post_scan == PostScan::Bounded
                    && state.minimum < state.maximum
                    && same_coordinate(&state.initial, &initial)
            });
        if !fits {
            return Err(Failure::new(
                &texts::library_pid_state_contract(instance, member),
                Cause::ControllerCapability,
                NextAction::OpenLoopSetup,
            ));
        }
    }
    Ok(())
}

/// The seven retained states of a `Control.PID`, each with its member name, its unit and its declared initial value.
fn expected_members(
    process_unit: &str,
    actuator_unit: &str,
) -> Vec<(&'static str, String, InitialValue)> {
    vec![
        ("Setpoint", process_unit.to_owned(), model_output()),
        (
            "SetpointRate",
            format!("{process_unit}/s"),
            InitialValue::Constant(0.0),
        ),
        ("ProcessValue", process_unit.to_owned(), model_output()),
        (
            "ProcessValueRate",
            format!("{process_unit}/s"),
            InitialValue::Constant(0.0),
        ),
        (
            "FeedForward",
            actuator_unit.to_owned(),
            InitialValue::Constant(0.0),
        ),
        ("NominalOutput", actuator_unit.to_owned(), model_input()),
        ("SaturatedOutput", actuator_unit.to_owned(), model_input()),
    ]
}

/// The operating-point coordinate of the model's first output.
fn model_output() -> InitialValue {
    InitialValue::From(InitialSource::ModelOutput { index: 0 })
}

/// The operating-point coordinate of the model's first input.
fn model_input() -> InitialValue {
    InitialValue::From(InitialSource::ModelInput { index: 0 })
}

/// Whether two initial values name the same coordinate: the same constant, or the same model channel and index.
#[allow(clippy::float_cmp)] // A constant initial value matches only the same number, as the TypeScript compares it (`===`).
fn same_coordinate(actual: &InitialValue, expected: &InitialValue) -> bool {
    match (actual, expected) {
        (InitialValue::Constant(actual), InitialValue::Constant(expected)) => actual == expected,
        (InitialValue::From(actual), InitialValue::From(expected)) => {
            coordinate(actual).is_some() && coordinate(actual) == coordinate(expected)
        }
        _ => false,
    }
}

/// The model channel and index of a coordinate source that C1 uses for a retained state.
fn coordinate(source: &InitialSource) -> Option<(&'static str, usize)> {
    match source {
        InitialSource::ModelInput { index } => Some(("model-input", *index)),
        InitialSource::ModelOutput { index } => Some(("model-output", *index)),
        _ => None,
    }
}

/// Whether a snapshot age is at least one task period and at most 10 s (`maximumSnapshotAgeMs`).
#[allow(clippy::cast_precision_loss)] // A snapshot age is a whole number of milliseconds below 10 s, exact as f64.
fn snapshot_age_fits(age_ms: u64, task_period_ms: f64) -> bool {
    age_ms as f64 >= task_period_ms && age_ms <= MAXIMUM_SNAPSHOT_AGE_MS
}

/// The seven retained states followed by the application states: the application states must belong to the controller's
/// owning program (the instance's first path element), and every symbol must be unique without regard to case.
fn application_states_check(
    loop_definition: &LoopDefinition,
    states: &[InitializedState],
    application_states: Option<&[InitializedState]>,
) -> Result<ControllerInitialization, Failure> {
    let refusal = || {
        Failure::new(
            &texts::library_pid_application_states(),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        )
    };
    let instance = &loop_definition.controller.instance;
    let root = format!(
        "{}.",
        instance
            .split('.')
            .next()
            .unwrap_or_default()
            .to_uppercase()
    );
    let application = application_states.unwrap_or_default();
    let owned = application
        .iter()
        .all(|state| state.symbol.to_uppercase().starts_with(&root));
    let all: Vec<InitializedState> = states.iter().chain(application).cloned().collect();
    let mut symbols = BTreeSet::new();
    let unique = all
        .iter()
        .all(|state| symbols.insert(state.symbol.to_uppercase()));
    if !owned || !unique {
        return Err(refusal());
    }
    ControllerInitialization::new(all).map_err(|_| refusal())
}

#[cfg(test)]
#[path = "library_pid_tests.rs"]
mod tests;
