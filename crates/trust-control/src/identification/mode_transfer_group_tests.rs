//! Tests of Identify's mode transfer for a PID group (specification 35, C7, A6.2 to A6.4; runtime protocol annex E6.2, E6.4 and E6.9):
//! the tracking that the runtime accepts, the setpoints and Manual commands written back for every member, the output change that
//! write-back is allowed, and the reference step for every setpoint. The loop is the two-member group of the fixture
//! `control-studio-pid-group` (`P1.Controller` and `P1.Secondary`, which share the mode `P1.AutoMode`), written as its loop file and
//! decoded, so that a fixture is valid only when the loop contract accepts it.

#![allow(clippy::float_cmp)] // The values that a transfer carries are compared exactly: they are rounded as their members are typed.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use super::tests::{status_of, symbols};
use super::*;
use crate::commissioning::declared_initialization::{AdditionalCommand, AdditionalReference};
use crate::loop_definition::{InitialValue, InitializedState, PostScan, StateType};

/// The mode that both members share: TRUE in Auto, so its Manual value is FALSE.
const MODE: &str = "P1.AutoMode";
/// The loop's own member: its setpoint, Manual command, output and process value.
const SETPOINT: &str = "P1.Setpoint";
const COMMAND: &str = "P1.ManualOutput";
const OUTPUT: &str = "P1.ManipulatedValue";
const PROCESS_VALUE: &str = "P1.ProcessValue";
/// The second member: its own setpoint, Manual command, output and process value.
const SECOND_SETPOINT: &str = "P1.SecondarySetpoint";
const SECOND_COMMAND: &str = "P1.SecondaryManualOutput";
const SECOND_OUTPUT: &str = "P1.SecondaryManipulatedValue";
const SECOND_PROCESS_VALUE: &str = "P1.SecondaryProcessValue";

/// The seven retained states of one `Control.PID`, by member name.
const RETAINED: [&str; 7] = [
    "Setpoint",
    "SetpointRate",
    "ProcessValue",
    "ProcessValueRate",
    "FeedForward",
    "NominalOutput",
    "SaturatedOutput",
];

/// A state of `%` from -110 to 110, started at zero and bounded after the scan.
fn retained_state(symbol: String) -> InitializedState {
    InitializedState {
        symbol,
        iec_type: StateType::Lreal,
        unit: "%".to_owned(),
        minimum: -110.0,
        maximum: 110.0,
        initial: InitialValue::Constant(0.0),
        post_scan: PostScan::Bounded,
    }
}

/// The startup-done flag of a member (C1): a BOOL application state that the Auto entry sets and the program keeps.
fn startup_flag(symbol: &str) -> InitializedState {
    InitializedState {
        symbol: symbol.to_owned(),
        iec_type: StateType::Bool,
        unit: "1".to_owned(),
        minimum: 0.0,
        maximum: 1.0,
        initial: InitialValue::Constant(1.0),
        post_scan: PostScan::Unchanged,
    }
}

/// The states of both members: each one's seven retained states and its startup flag, in the order the fixture declares them.
fn declared_states_of_group() -> Vec<InitializedState> {
    let mut declared = Vec::new();
    for (instance, flag) in [
        ("P1.Controller", "P1.Initialized"),
        ("P1.Secondary", "P1.SecondaryInitialized"),
    ] {
        declared.extend(
            RETAINED
                .iter()
                .map(|field| retained_state(format!("{instance}.StateValue.{field}"))),
        );
        declared.push(startup_flag(flag));
    }
    declared
}

/// The three gains of the parameter record `record`, bounded by 0 and 100.
fn gains(record: &str) -> Vec<Value> {
    [
        ("ProportionalGain", "%/degC", "Control.PID ProportionalGain"),
        ("IntegralGain", "%/(degC*s)", "Control.PID IntegralGain"),
        ("DerivativeGain", "%*s/degC", "Control.PID DerivativeGain"),
    ]
    .iter()
    .map(|(name, unit, convention)| {
        json!({
            "symbol": format!("{record}.{name}"), "iec_type": "LREAL", "unit": unit,
            "convention": convention, "minimum": 0, "maximum": 100
        })
    })
    .collect()
}

/// One member of the group: its instance, its channels, the shared mode, its three gains of `record` and its startup flag.
fn member(instance: &str, channels: (&str, &str), record: &str, flag: &str) -> Value {
    json!({
        "instance": instance, "type": "Control.PID",
        "inputChannelId": channels.0, "outputChannelId": channels.1, "modeSymbol": MODE,
        "parameterSymbols": [
            format!("{record}.ProportionalGain"), format!("{record}.IntegralGain"), format!("{record}.DerivativeGain")
        ],
        "applicationStateSymbols": [flag]
    })
}

/// A signal of the loop: an `LREAL` of `unit` from 0 to 100 at `address`.
fn signal(symbol: &str, address: &str, direction: &str, unit: &str) -> Value {
    json!({
        "symbol": symbol, "address": address, "iecType": "LREAL", "direction": direction,
        "unit": unit, "range": { "minimum": 0, "maximum": 100 }
    })
}

/// A further signal of the second member, on its own channel.
fn member_signal(id: &str, role: &str, symbol: &str, address: &str, unit: &str) -> Value {
    let direction = if role == "MV" { "write" } else { "read" };
    let mut entry = signal(symbol, address, direction, unit);
    entry["id"] = json!(id);
    entry["channel"] = json!("secondary");
    entry["role"] = json!(role);
    entry
}

/// The loop file of the group: two `Control.PID` members in program `P1`, the first on the loop's own signals and the second on its
/// own setpoint, process value and output, with the Manual command of its channel (C7). Both actuators run from 0 to 100 %, the loop
/// limits the output rate to 25 % per second, and the task runs every 100 ms.
fn pid_group_file() -> Value {
    let actuator = json!({ "kind": "continuous", "minimum": 0, "maximum": 100 });
    let mut parameters = gains("P1.Parameters");
    parameters.extend(gains("P1.SecondaryParameters"));
    json!({
        "schemaVersion": 2, "id": "loop:group", "name": "Two member PID group",
        "revision": "sha256:loop-group", "projectId": "project:sha256:abc", "runtimeId": "runtime:local",
        "controller": {
            "instance": "P1.Controller", "type": "Control.PID", "revision": "sha256:controller-group",
            "action": "direct", "taskPeriodMs": 100, "design": "project",
            "modeSignal": MODE, "modeWhenTrue": "Auto",
            "manualOutputSignal": COMMAND, "manualOutputType": "LREAL",
            "project": {
                "family": "paired-PI", "convention": "project-owned-native-controller",
                "initialization": { "kind": "declared-state-v1", "states": declared_states_of_group() },
                "seed": { "kind": "decentralized-pid", "family": "PI" }, "axes": [],
                "objective": { "requirementId": "tracking-error", "direction": "minimize" },
                "maximumCandidates": 1, "maximumCycles": 1000, "wallBudgetMs": 1000
            },
            "group": { "members": [
                member("P1.Controller", ("u1", "y1"), "P1.Parameters", "P1.Initialized"),
                member("P1.Secondary", ("u2", "y2"), "P1.SecondaryParameters", "P1.SecondaryInitialized")
            ] }
        },
        "parameterBinding": {
            "loop_id": "loop:group", "task_name": "MainTask", "parameters": parameters,
            "mode": { "symbol": MODE, "manual_value": false }
        },
        "signals": {
            "setpoint": signal(SETPOINT, "%ML0", "read", "degC"),
            "processVariable": signal(PROCESS_VALUE, "%IL0", "read", "degC"),
            "manipulatedVariable": signal(OUTPUT, "%QL0", "write", "%"),
            "additional": [
                member_signal("mv-secondary", "MV", SECOND_OUTPUT, "%QL8", "%"),
                member_signal("pv-secondary", "PV", SECOND_PROCESS_VALUE, "%IL8", "degC"),
                member_signal("sp-secondary", "SP", SECOND_SETPOINT, "%ML32", "degC")
            ]
        },
        "channels": {
            "inputs": [
                { "modelChannelId": "u1", "signalId": "manipulatedVariable", "actuator": actuator },
                { "modelChannelId": "u2", "signalId": "mv-secondary", "manualOutputSignal": SECOND_COMMAND,
                  "manualOutputType": "LREAL", "actuator": actuator }
            ],
            "outputs": [
                { "modelChannelId": "y1", "signalId": "processVariable", "setpointSignalId": "setpoint" },
                { "modelChannelId": "y2", "signalId": "pv-secondary", "setpointSignalId": "sp-secondary" }
            ]
        },
        "limits": { "outputMinimum": 0, "outputMaximum": 100, "rateUpPerS": 25, "rateDownPerS": 25 },
        "observedSampleTiming": { "nominalPeriodMs": 100 }
    })
}

/// The loop of the group; the rest's tests (`campaign/rest_tests.rs`) watch it too.
pub(crate) fn pid_group_loop() -> LoopDefinition {
    LoopDefinition::decode(&pid_group_file().to_string()).expect("the group loop file decodes")
}

/// The value that a status reports for each symbol: the listed one when `overrides` lists it, else 1, which every member of the
/// fixture holds (a BOOL, a gain, a state and a signal alike).
fn holding<'a>(overrides: &'a [(&'a str, f64)]) -> impl Fn(&str) -> f64 + 'a {
    move |symbol| {
        overrides
            .iter()
            .find(|(name, _)| *name == symbol)
            .map_or(1.0, |(_, value)| *value)
    }
}

/// What the runtime requires of a declared-state tracking before it accepts a transfer (`validate_declared_states` in
/// `trust-runtime`'s `host/debug/control/transfer/tracking.rs`, annex E6.4): the same checks in the same order, with the runtime's
/// sentences. `trust-control` does not depend on the runtime, so the rule is restated here; `control_studio_pid_group.rs` proves it
/// against the runtime itself.
fn runtime_accepts(
    binding: &TransferBinding,
    tracking: &DeclaredStateTracking,
    manual: bool,
) -> Result<(), String> {
    let values = &tracking.initialization_values;
    let bounded = &tracking.bounded_state_symbols;
    let state = |symbol: &str| binding.states.iter().any(|member| member.symbol == symbol);
    let guard = |symbol: &str| binding.guards.iter().any(|member| member.symbol == symbol);
    if manual && tracking.align_output
        || tracking.reference_symbol == tracking.manual_command_symbol
        || !state(&tracking.reference_symbol)
        || !state(&tracking.manual_command_symbol)
        || !guard(&tracking.process_variable)
        || values.len()
            + 2
            + tracking.additional_references.len()
            + tracking.additional_commands.len()
            != binding.states.len()
        || values.contains_key(&tracking.reference_symbol)
        || values.contains_key(&tracking.manual_command_symbol)
        || bounded.len() > values.len() + 1
        || bounded.iter().collect::<BTreeSet<_>>().len() != bounded.len()
        || bounded
            .iter()
            .any(|symbol| !values.contains_key(symbol) && *symbol != tracking.manual_command_symbol)
    {
        return Err(
            "initialization must name the complete, distinct declared controller states".to_owned(),
        );
    }
    if tracking.additional_measurements.len() > 16
        || tracking
            .additional_measurements
            .iter()
            .any(|symbol| !guard(symbol))
    {
        return Err("additional measurements must name declared process guards".to_owned());
    }
    if tracking.additional_commands.len() != binding.additional_outputs.len()
        || tracking.additional_references.len() > 15
    {
        return Err(
            "declared tracking must cover every additional actuator exactly once".to_owned(),
        );
    }
    let mut roles = BTreeSet::from([
        tracking.reference_symbol.as_str(),
        tracking.manual_command_symbol.as_str(),
    ]);
    for channel in &tracking.additional_references {
        if !roles.insert(channel.reference_symbol.as_str())
            || values.contains_key(&channel.reference_symbol)
            || !state(&channel.reference_symbol)
            || !guard(&channel.process_variable)
        {
            return Err("invalid additional reference tracking role".to_owned());
        }
    }
    let mut held = BTreeSet::new();
    for channel in &tracking.additional_commands {
        if !roles.insert(channel.manual_command_symbol.as_str())
            || values.contains_key(&channel.manual_command_symbol)
            || !state(&channel.manual_command_symbol)
            || !held.insert(channel.output_symbol.as_str())
            || !binding
                .additional_outputs
                .iter()
                .any(|output| output.symbol == channel.output_symbol)
        {
            return Err("invalid additional actuator tracking role".to_owned());
        }
    }
    Ok(())
}

/// The declared-state tracking of `loop_definition` at the states `status` reports.
fn tracking_of(
    loop_definition: &LoopDefinition,
    binding: &TransferBinding,
    status: &TransferStatus,
) -> DeclaredStateTracking {
    let TransferTracking::DeclaredState(tracking) =
        mode_tracking(loop_definition, binding, status).expect("the tracking")
    else {
        panic!("a mode transfer tracks declared states");
    };
    tracking
}

/// C7, E6.2: the group transfers on a loop of Identify's own, with the setpoint and the Manual command of the loop's member, then of
/// the other member, as its leading states, the initialized states of both members after them, and the output of each member.
#[test]
fn the_mode_binding_of_a_group_holds_every_member() {
    let binding = mode_binding(&pid_group_loop()).expect("the group transfers");
    assert_eq!(binding.loop_id, "loop:group:identification-mode");
    assert_eq!(
        (binding.mode.symbol.as_str(), binding.mode.manual_value),
        (MODE, false)
    );
    assert_eq!(
        symbols(&binding.states[..4]),
        [SETPOINT, COMMAND, SECOND_SETPOINT, SECOND_COMMAND]
    );
    assert_eq!(
        binding.states.len(),
        4 + 16,
        "fourteen retained states and two startup flags"
    );
    assert_eq!(binding.output.symbol, OUTPUT);
    assert_eq!(symbols(&binding.additional_outputs), [SECOND_OUTPUT]);
    let measurements: Vec<&str> = binding
        .guards
        .iter()
        .map(|guard| guard.symbol.as_str())
        .filter(|symbol| symbol.ends_with("ProcessValue"))
        .collect();
    assert_eq!(measurements, [PROCESS_VALUE, SECOND_PROCESS_VALUE]);
}

/// E6.4, `validate_declared_states`: the tracking of a group names the other member's setpoint with its process value, its Manual
/// command with the output it holds, and both process values; the initialized states are the sixteen that are not a setpoint or a
/// Manual command; and the runtime accepts the record. The record that the loop's member alone would give (every other state
/// initialized, nothing further named) is refused, because the group holds an additional output that no tracked command covers.
#[test]
fn the_tracking_of_a_group_names_every_other_member_as_the_runtime_requires() {
    let loop_definition = pid_group_loop();
    let binding = mode_binding(&loop_definition).expect("a binding");
    let status = status_of(&binding, true, &holding(&[(SECOND_COMMAND, 8.0)]));
    let tracking = tracking_of(&loop_definition, &binding, &status);
    assert_eq!(
        (
            tracking.reference_symbol.as_str(),
            tracking.manual_command_symbol.as_str(),
            tracking.process_variable.as_str()
        ),
        (SETPOINT, COMMAND, PROCESS_VALUE)
    );
    assert!(!tracking.align_output, "the output is held, not aligned");
    assert_eq!(
        tracking.additional_references,
        [AdditionalReference {
            reference_symbol: SECOND_SETPOINT.to_owned(),
            process_variable: SECOND_PROCESS_VALUE.to_owned(),
        }]
    );
    assert_eq!(
        tracking.additional_commands,
        [AdditionalCommand {
            manual_command_symbol: SECOND_COMMAND.to_owned(),
            output_symbol: SECOND_OUTPUT.to_owned(),
        }]
    );
    assert_eq!(
        tracking.additional_measurements,
        [PROCESS_VALUE, SECOND_PROCESS_VALUE]
    );
    let initialized: BTreeSet<&str> = binding.states[4..]
        .iter()
        .map(|member| member.symbol.as_str())
        .collect();
    assert_eq!(
        tracking
            .initialization_values
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        initialized,
        "the initialized states are those that are neither a setpoint nor a Manual command"
    );
    assert!(
        tracking.initialization_values.values().all(|value| *value == 1.0),
        "each is initialized at the value the status reports, and the Manual command of the second member is not one of them"
    );
    assert_eq!(
        tracking
            .bounded_state_symbols
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        initialized,
        "the program moves every initialized state, and no Manual command follows its output here"
    );
    assert_eq!(runtime_accepts(&binding, &tracking, true), Ok(()));
    assert_eq!(runtime_accepts(&binding, &tracking, false), Ok(()));

    let scalar = DeclaredStateTracking {
        initialization_values: binding.states[2..]
            .iter()
            .map(|member| (member.symbol.clone(), 1.0))
            .collect(),
        bounded_state_symbols: binding.states[2..]
            .iter()
            .map(|member| member.symbol.clone())
            .collect(),
        additional_references: Vec::new(),
        additional_commands: Vec::new(),
        ..tracking
    };
    assert_eq!(
        runtime_accepts(&binding, &scalar, true),
        Err("declared tracking must cover every additional actuator exactly once".to_owned()),
        "the record of a loop with one controller output is not a group's"
    );
}

/// A6.4, §17.1: where the program copies the output into the loop's Manual command, that command is bounded as well; the second
/// member's Manual command never is, since the runtime bounds only the loop's own beside the initialized states.
#[test]
fn only_the_loops_own_manual_command_is_bounded_when_the_program_copies_the_output() {
    let mut loop_definition = pid_group_loop();
    loop_definition.controller.manual_output_follows_output = Some(true);
    let binding = mode_binding(&loop_definition).expect("a binding");
    let status = status_of(&binding, true, &holding(&[]));
    let tracking = tracking_of(&loop_definition, &binding, &status);
    assert!(tracking.bounded_state_symbols.contains(&COMMAND.to_owned()));
    assert!(!tracking
        .bounded_state_symbols
        .contains(&SECOND_COMMAND.to_owned()));
    assert_eq!(tracking.bounded_state_symbols.len(), 16 + 1);
    assert_eq!(runtime_accepts(&binding, &tracking, false), Ok(()));
}

/// A6.2: a group found in Manual gets every member's setpoint and Manual command back, on a transfer loop of its own that keeps the
/// group's outputs, with the parameters of the record as its guards; a group found in Auto gets its setpoints back only (C1),
/// since each Manual command written in Manual would move its member's output before the Auto entry.
#[test]
fn the_command_return_writes_back_every_members_setpoint_and_manual_command() {
    let mut loop_definition = pid_group_loop();
    let binding = mode_binding(&loop_definition).expect("a binding");
    let found = [
        (SETPOINT, 45.0),
        (SECOND_SETPOINT, 40.0),
        (COMMAND, 12.0),
        (SECOND_COMMAND, 8.0),
    ];
    let in_manual = status_of(&binding, false, &holding(&found));
    let back = command_return(&loop_definition, &binding, &in_manual).expect("found in Manual");
    assert_eq!(
        back.binding.loop_id,
        "loop:group:identification-mode:commands"
    );
    assert_eq!(
        symbols(&back.binding.states),
        [SETPOINT, SECOND_SETPOINT, COMMAND, SECOND_COMMAND]
    );
    assert_eq!(
        back.values,
        found
            .iter()
            .map(|(symbol, value)| ((*symbol).to_owned(), *value))
            .collect::<BTreeMap<String, f64>>()
    );
    assert_eq!(
        back.commands,
        [
            WrittenCommand {
                command: COMMAND.to_owned(),
                output: OUTPUT.to_owned()
            },
            WrittenCommand {
                command: SECOND_COMMAND.to_owned(),
                output: SECOND_OUTPUT.to_owned()
            },
        ]
    );
    assert_eq!(
        symbols(&back.binding.guards),
        [
            "P1.Parameters.ProportionalGain",
            "P1.Parameters.IntegralGain",
            "P1.Parameters.DerivativeGain",
            "P1.SecondaryParameters.ProportionalGain",
            "P1.SecondaryParameters.IntegralGain",
            "P1.SecondaryParameters.DerivativeGain"
        ]
    );
    assert_eq!(back.binding.output, binding.output);
    assert_eq!(back.binding.additional_outputs, binding.additional_outputs);

    loop_definition.controller.manual_output_follows_output = Some(true);
    let in_auto = status_of(&binding, true, &holding(&found));
    let kept = command_return(&loop_definition, &binding, &in_auto).expect("found in Auto");
    assert_eq!(symbols(&kept.binding.states), [SETPOINT, SECOND_SETPOINT]);
    assert_eq!(
        kept.values,
        BTreeMap::from([
            (SETPOINT.to_owned(), 45.0),
            (SECOND_SETPOINT.to_owned(), 40.0)
        ])
    );
    assert_eq!(
        kept.commands,
        Vec::<WrittenCommand>::new(),
        "no member's Manual command moves its output"
    );
    let found_in_manual = command_return(&loop_definition, &binding, &in_manual)
        .expect("found in Manual, the program's copy is overwritten");
    assert_eq!(
        symbols(&found_in_manual.binding.states),
        [SETPOINT, SECOND_SETPOINT, COMMAND, SECOND_COMMAND],
        "a loop found in Manual returns to Manual: its Manual command is written back"
    );
}

/// A6.2, E6.9: writing the found Manual commands back moves each member's output, and the runtime allows an additional output the
/// tolerance scaled by the ratio of the output spans, so the allowance is the largest of the changes in the span of the loop's own
/// output. With both actuators from 0 to 100 the changes are 8 and 22. With the second actuator from 0 to 50, its change of 22 is
/// 44 in the span of the loop's output. The allowance never falls below the mode change's own, and an output the status does not
/// report is not counted.
#[test]
fn the_allowance_of_the_write_back_is_the_largest_change_in_the_span_of_the_loops_output() {
    let mut loop_definition = pid_group_loop();
    let binding = mode_binding(&loop_definition).expect("a binding");
    let found = status_of(
        &binding,
        false,
        &holding(&[(COMMAND, 12.0), (SECOND_COMMAND, 8.0)]),
    );
    let now = status_of(
        &binding,
        false,
        &holding(&[(OUTPUT, 20.0), (SECOND_OUTPUT, 30.0)]),
    );
    let back = command_return(&loop_definition, &binding, &found).expect("the write-back");
    assert!((back.allowance(1e-6, &now) - 22.0).abs() < 1e-12);
    assert!(
        (back.allowance(100.0, &now) - 100.0).abs() < 1e-12,
        "a larger allowance of the mode change stays"
    );
    let mut silent = now.clone();
    silent.values.remove(SECOND_OUTPUT);
    assert!(
        (back.allowance(1e-6, &silent) - 8.0).abs() < 1e-12,
        "only the loop's own output is counted when the other is not reported"
    );

    loop_definition
        .channels
        .as_mut()
        .expect("the group declares its channels")
        .inputs[1]
        .actuator
        .maximum = 50.0;
    let narrow = mode_binding(&loop_definition).expect("a binding");
    let narrow_found = status_of(
        &narrow,
        false,
        &holding(&[(COMMAND, 12.0), (SECOND_COMMAND, 8.0)]),
    );
    let narrow_now = status_of(
        &narrow,
        false,
        &holding(&[(OUTPUT, 20.0), (SECOND_OUTPUT, 30.0)]),
    );
    let narrowed =
        command_return(&loop_definition, &narrow, &narrow_found).expect("the write-back");
    assert!((narrowed.allowance(1e-6, &narrow_now) - 44.0).abs() < 1e-12);
}

/// C1, A6.2: a program that sets each setpoint from its measurement in Manual leaves the group entering Auto at the measurements.
/// One step in Auto returns every setpoint the group no longer holds, each at its found value, with every other state as the group
/// holds it, the mode transfer's tracking and the allowance of a target change across the outputs. A group that holds its found
/// setpoints needs no step, and a step returns only the setpoints that moved.
#[test]
fn every_setpoint_the_program_overwrote_returns_after_the_auto_entry() {
    let loop_definition = pid_group_loop();
    let binding = mode_binding(&loop_definition).expect("a binding");
    let original = status_of(
        &binding,
        true,
        &holding(&[(SETPOINT, 45.0), (SECOND_SETPOINT, 40.0)]),
    );
    let entered = status_of(
        &binding,
        true,
        &holding(&[
            (SETPOINT, 44.25),
            (SECOND_SETPOINT, 39.5),
            (COMMAND, 3.0),
            (SECOND_COMMAND, 4.0),
        ]),
    );
    let step = reference_return(&loop_definition, &binding, &original, &entered)
        .expect("the step")
        .expect("both setpoints return");
    let mut values = mode_states(&binding, &entered).expect("the states");
    assert_eq!(values.insert(SETPOINT.to_owned(), 45.0), Some(44.25));
    assert_eq!(values.insert(SECOND_SETPOINT.to_owned(), 40.0), Some(39.5));
    assert_eq!(step.values, values);
    assert_eq!(
        (step.values[COMMAND], step.values[SECOND_COMMAND]),
        (3.0, 4.0),
        "every other state is as the group holds it now, not as it was found"
    );
    assert_eq!(
        step.tracking,
        mode_tracking(&loop_definition, &binding, &entered).expect("the tracking")
    );
    assert_eq!(
        Some(step.tolerance),
        transfer_target_allowance(&binding),
        "a target change across the outputs"
    );

    let second_moved = status_of(
        &binding,
        true,
        &holding(&[(SETPOINT, 45.0), (SECOND_SETPOINT, 39.5)]),
    );
    let step = reference_return(&loop_definition, &binding, &original, &second_moved)
        .expect("the step")
        .expect("the second setpoint returns");
    assert_eq!(step.values[SETPOINT], 45.0);
    assert_eq!(step.values[SECOND_SETPOINT], 40.0);

    let holding_both = status_of(
        &binding,
        true,
        &holding(&[(SETPOINT, 45.0), (SECOND_SETPOINT, 40.0)]),
    );
    assert_eq!(
        reference_return(&loop_definition, &binding, &original, &holding_both).expect("no step"),
        None
    );
}
