//! Tests of the commissioning adapter of a PID group (C7): a coherent group is served and exposes both transfer outputs, and each
//! incoherent group is refused before tuning or application. The loops are written as loop files and decoded, so a fixture is valid
//! only when the loop contract accepts it.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::*;
use crate::commissioning::adapters::{controller_adapter, AdapterKind, ControllerGains};
use crate::commissioning::initial_state::ControllerInitialization;
use crate::loop_definition::{InitialValue, InitializedState, LoopDefinition, PostScan, StateType};
use crate::tuning::structure::Structure;

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

/// The application states that each member's program keeps.
const APPLICATION: [&str; 2] = ["Integral", "PreviousError"];

/// The loop of a PID group of `members` members (C7), decoded from its loop file. Member 0 drives the loop's own manipulated variable and
/// measures its process value; every other member has its own setpoint, process value and manipulated variable, and its own Manual
/// command. Every member is a `Control.PID` in the program `P1`, sharing the mode `P1.AutoMode`. With `states`, the project declares
/// each member's seven retained states and its two application states; without, it declares none.
fn group_loop(members: usize, states: bool) -> LoopDefinition {
    LoopDefinition::decode(&group_file(members, states).to_string())
        .expect("the group loop file decodes")
}

/// The loop file of a PID group of `members` members (see [`group_loop`]).
fn group_file(members: usize, states: bool) -> Value {
    let mut group = Vec::new();
    let mut parameters = Vec::new();
    let mut declared: Vec<InitializedState> = Vec::new();
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    let mut additional = Vec::new();
    for index in 0..members {
        group.push(member_entry(index, states));
        parameters.extend(member_parameters(index));
        if states {
            declared.extend(member_states(index));
        }
        inputs.push(input_channel_json(index));
        outputs.push(output_channel_json(index));
        additional.extend(member_signals(index));
    }
    let controller = json!({
        "instance": instance(0), "type": "Control.PID", "revision": "sha256:controller-group",
        "action": "direct", "taskPeriodMs": 10, "design": "project",
        "manualOutputSignal": "P1.ManualMV0", "manualOutputType": "LREAL",
        "project": {
            "family": "paired-PI", "convention": "project-owned-native-controller",
            "initialization": { "kind": "declared-state-v1", "states": declared },
            "seed": { "kind": "decentralized-pid", "family": "PI" }, "axes": [],
            "objective": { "requirementId": "tracking-error", "direction": "minimize" },
            "maximumCandidates": 1, "maximumCycles": 1000, "wallBudgetMs": 1000
        },
        "group": { "members": group }
    });
    json!({
        "schemaVersion": 2, "id": "loop:group", "name": "Two zone group",
        "revision": "sha256:loop-group", "projectId": "project:sha256:abc",
        "runtimeId": "runtime:local", "controller": controller,
        "parameterBinding": {
            "loop_id": "loop:group", "task_name": "MainTask", "parameters": parameters,
            "mode": { "symbol": "P1.AutoMode", "manual_value": false }
        },
        "signals": {
            "setpoint": signal("P1.SP0", "%ML0", "read"),
            "processVariable": signal("P1.PV0", "%IL0", "read"),
            "manipulatedVariable": signal("P1.MV0", "%QL0", "write"),
            "additional": additional
        },
        "channels": { "inputs": inputs, "outputs": outputs },
        "limits": { "outputMinimum": 0, "outputMaximum": 100 },
        "observedSampleTiming": { "nominalPeriodMs": 10, "maximumJitterMs": 1 }
    })
}

/// The instance of member `index`, in the program `P1`.
fn instance(index: usize) -> String {
    format!("P1.Member{index}")
}

/// A loop signal of `%` from 0 to 100 at `address`, read or written.
fn signal(symbol: &str, address: &str, direction: &str) -> Value {
    json!({
        "symbol": symbol, "address": address, "iecType": "LREAL", "direction": direction,
        "unit": "%", "range": { "minimum": 0, "maximum": 100 }
    })
}

/// A gain of a parameter record: `%/%`, bounded by its magnitude (N6.4).
fn gain(symbol: &str, convention: &str) -> Value {
    json!({
        "symbol": symbol, "iec_type": "LREAL", "unit": "%/%", "convention": convention,
        "minimum": -100, "maximum": 100
    })
}

/// The gain variables of member `index`: its proportional, integral and derivative gains.
fn gain_symbols(index: usize) -> Vec<String> {
    ["Kp", "Ki", "Kd"]
        .iter()
        .map(|name| format!("{}.{name}", instance(index)))
        .collect()
}

/// The application states that member `index` names as its own.
fn application_symbols(index: usize) -> Vec<String> {
    APPLICATION
        .iter()
        .map(|name| format!("{}.{name}", instance(index)))
        .collect()
}

/// The group member entry of member `index`: its channels, the shared mode, its gains and, with `states`, its application states.
fn member_entry(index: usize, states: bool) -> Value {
    let application = if states {
        application_symbols(index)
    } else {
        Vec::new()
    };
    json!({
        "instance": instance(index), "type": "Control.PID",
        "inputChannelId": format!("u{index}"), "outputChannelId": format!("y{index}"),
        "modeSymbol": "P1.AutoMode", "parameterSymbols": gain_symbols(index),
        "applicationStateSymbols": application
    })
}

/// The gain parameters of member `index`.
fn member_parameters(index: usize) -> Vec<Value> {
    let owner = instance(index);
    vec![
        gain(&format!("{owner}.Kp"), "proportional gain"),
        gain(&format!("{owner}.Ki"), "integral gain"),
        gain(&format!("{owner}.Kd"), "derivative gain"),
    ]
}

/// A bounded LREAL state of `%` from -110 to 110, started at zero.
fn state(symbol: &str) -> InitializedState {
    InitializedState {
        symbol: symbol.to_owned(),
        iec_type: StateType::Lreal,
        unit: "%".to_owned(),
        minimum: -110.0,
        maximum: 110.0,
        initial: InitialValue::Constant(0.0),
        post_scan: PostScan::Bounded,
    }
}

/// The seven retained states and the two application states of member `index`, in its program.
fn member_states(index: usize) -> Vec<InitializedState> {
    let owner = instance(index);
    RETAINED
        .iter()
        .map(|name| state(&format!("{owner}.StateValue.{name}")))
        .chain(
            APPLICATION
                .iter()
                .map(|name| state(&format!("{owner}.{name}"))),
        )
        .collect()
}

/// The input channel that member `index` drives: the loop's own manipulated variable for member 0, its own signal otherwise.
fn input_channel_json(index: usize) -> Value {
    let actuator = json!({ "kind": "continuous", "minimum": 0, "maximum": 100 });
    if index == 0 {
        json!({
            "modelChannelId": "u0", "signalId": "manipulatedVariable", "actuator": actuator
        })
    } else {
        json!({
            "modelChannelId": format!("u{index}"), "signalId": format!("mv{index}"),
            "manualOutputSignal": format!("P1.Manual{index}"), "manualOutputType": "LREAL",
            "actuator": actuator
        })
    }
}

/// The output channel that member `index` measures, with its own setpoint.
fn output_channel_json(index: usize) -> Value {
    if index == 0 {
        json!({
            "modelChannelId": "y0", "signalId": "processVariable", "setpointSignalId": "setpoint"
        })
    } else {
        json!({
            "modelChannelId": format!("y{index}"), "signalId": format!("pv{index}"),
            "setpointSignalId": format!("sp{index}")
        })
    }
}

/// The additional signals of member `index`: its setpoint, process value and manipulated variable (none for member 0, whose signals
/// are the loop's own).
fn member_signals(index: usize) -> Vec<Value> {
    if index == 0 {
        return Vec::new();
    }
    vec![
        json!({
            "id": format!("sp{index}"), "channel": format!("member{index}"), "role": "SP",
            "symbol": format!("P1.SP{index}"), "address": format!("%ML{}", 8 * index),
            "iecType": "LREAL", "direction": "read", "unit": "%",
            "range": { "minimum": 0, "maximum": 100 }
        }),
        json!({
            "id": format!("pv{index}"), "channel": format!("member{index}"), "role": "PV",
            "symbol": format!("P1.PV{index}"), "address": format!("%IL{}", 8 * index),
            "iecType": "LREAL", "direction": "read", "unit": "%",
            "range": { "minimum": 0, "maximum": 100 }
        }),
        json!({
            "id": format!("mv{index}"), "channel": format!("member{index}"), "role": "MV",
            "symbol": format!("P1.MV{index}"), "address": format!("%QL{}", 8 * index),
            "iecType": "LREAL", "direction": "write", "unit": "%",
            "range": { "minimum": 0, "maximum": 100 }
        }),
    ]
}

#[test]
fn a_complete_group_is_served_and_exposes_both_transfer_outputs() {
    let loop_definition = group_loop(2, true);
    let adapter = controller_adapter(&loop_definition).expect("the coherent group is served");
    assert!(matches!(adapter.kind(), AdapterKind::Group { .. }));
    assert_eq!(adapter.families(), vec![Structure::PI]);
    let binding = adapter.transfer_binding().expect("the group transfers");
    assert_eq!(
        1 + binding.additional_outputs.len(),
        2,
        "the group retains both PID outputs"
    );
    assert_eq!(binding.additional_outputs[0].symbol, "P1.MV1");
    assert!(
        binding.additional_modes.is_empty(),
        "the members share the loop's mode"
    );
}

#[test]
fn an_unowned_parameter_is_refused_from_the_group_union() {
    let mut loop_definition = group_loop(2, true);
    let record = loop_definition
        .parameter_binding
        .as_mut()
        .expect("the loop declares its parameter record");
    let mut extra = record.parameters[0].clone();
    extra.symbol = "P1.Unowned.Kp".to_owned();
    record.parameters.push(extra);
    let failure =
        controller_adapter(&loop_definition).expect_err("an unowned parameter is refused");
    assert_eq!(failure.message, PARAMETER_UNION);
}

#[test]
fn a_group_missing_the_second_nominal_output_is_refused() {
    let mut loop_definition = group_loop(2, true);
    let project = loop_definition
        .controller
        .project
        .as_mut()
        .expect("the group has its project");
    let kept: Vec<InitializedState> = project
        .initialization
        .states()
        .iter()
        .filter(|state| state.symbol != "P1.Member1.StateValue.NominalOutput")
        .cloned()
        .collect();
    project.initialization =
        ControllerInitialization::new(kept).expect("the remaining states decode");
    let failure =
        controller_adapter(&loop_definition).expect_err("a missing NominalOutput is refused");
    assert_eq!(
        failure.message,
        texts::library_pid_seven_states("P1.Member1")
    );
}

#[test]
fn a_member_mode_that_is_not_the_shared_group_mode_is_refused() {
    let mut loop_definition = group_loop(2, true);
    let group = loop_definition
        .controller
        .group
        .as_mut()
        .expect("the loop declares its group");
    group.members[1].mode_symbol = "P1.SecondaryAutoMode".to_owned();
    let failure = controller_adapter(&loop_definition).expect_err("a second mode is refused");
    assert_eq!(
        failure.message,
        "PID group member P1.Member1 must use the declared shared mode for coherent application."
    );
}

#[test]
fn a_cross_program_group_is_refused_before_tuning_or_application() {
    let mut loop_definition = group_loop(2, true);
    let group = loop_definition
        .controller
        .group
        .as_mut()
        .expect("the loop declares its group");
    group.members[1].instance = "P2.Member1".to_owned();
    let failure =
        controller_adapter(&loop_definition).expect_err("a cross-program group is refused");
    assert_eq!(failure.message, ONE_PROGRAM);
}

#[test]
fn a_group_without_its_project_is_refused_rather_than_tuned_as_a_scalar_loop() {
    let mut loop_definition = group_loop(2, true);
    loop_definition.controller.project = None;
    let failure =
        controller_adapter(&loop_definition).expect_err("a group without its project is refused");
    assert_eq!(failure.message, NEEDS_PROJECT);
}

#[test]
fn a_sixteen_member_group_holds_fifteen_additional_outputs_and_no_further_modes() {
    let loop_definition = group_loop(16, false);
    let project = loop_definition
        .controller
        .project
        .as_ref()
        .expect("the group has its project");
    let group = loop_definition
        .controller
        .group
        .as_ref()
        .expect("the loop declares its group");
    let binding =
        group_transfer_binding(&loop_definition, project, group).expect("the group transfers");
    assert_eq!(
        binding.additional_outputs.len(),
        15,
        "the runtime holds at most fifteen additional outputs"
    );
    assert_eq!(
        binding.additional_modes,
        [] as [crate::commissioning::transfer::TransferModeBinding; 0]
    );
}

#[test]
fn a_group_writes_coefficients_inside_their_bounds_and_only_for_its_parameters() {
    let loop_definition = group_loop(2, true);
    let adapter = controller_adapter(&loop_definition).expect("the coherent group is served");
    let observed = BTreeMap::new();
    let inside = BTreeMap::from([("P1.Member1.Kp".to_owned(), 2.5)]);
    let written = adapter
        .materialize_coefficients(&inside, &observed)
        .expect("the value is inside its bounds");
    assert!(written
        .get("P1.Member1.Kp")
        .is_some_and(|value| (value - 2.5).abs() < 1e-12));
    let unheld = BTreeMap::from([("P1.Unheld.Kp".to_owned(), 1.0)]);
    let refused = adapter
        .materialize_coefficients(&unheld, &observed)
        .expect_err("a symbol that the binding does not hold is refused");
    assert!(refused
        .message
        .contains("is not a parameter of the loop's record"));
    let outside = BTreeMap::from([("P1.Member1.Kp".to_owned(), 500.0)]);
    assert!(adapter
        .materialize_coefficients(&outside, &observed)
        .is_err());
}

#[test]
fn a_group_refuses_gains_because_it_writes_its_values_by_symbol() {
    let loop_definition = group_loop(2, true);
    let adapter = controller_adapter(&loop_definition).expect("the coherent group is served");
    let failure = adapter
        .materialize(&ControllerGains::default(), &BTreeMap::new())
        .expect_err("a group takes no gains");
    assert_eq!(failure.message, NOT_BY_GAINS);
}

/// B6.2, C7: Apply waits for every member of a PID group, not only for the primary one. The starting point compares each member's plant
/// input and measured output, in the model's channel order, which the operating point's values follow. Wrong behaviour caught: the
/// starting point named the primary member's pair alone, so Apply entered Auto while another member's output was still moving.
#[test]
fn the_starting_point_of_a_group_compares_every_member_in_the_models_channel_order() {
    use crate::commissioning::pid_commissioning::starting_point_symbols;
    use crate::model::general::decode_general_model;
    let channel = |id: &str| json!({ "id": id, "unit": "%", "operatingRange": { "minimum": 0, "maximum": 100 } });
    let model = decode_general_model(&json!({
        "kind": "continuous-state-space", "revision": "group-model", "source": "provided", "validation": "validated",
        "evidence": ["group"], "delaySeconds": 0.0,
        "inputs": [channel("u0"), channel("u1")], "outputs": [channel("y0"), channel("y1")],
        "states": [channel("x0"), channel("x1")],
        "a": [[-1.0, 0.0], [0.0, -1.0]], "b": [[1.0, 0.0], [0.0, 1.0]], "c": [[1.0, 0.0], [0.0, 1.0]], "d": [[0.0, 0.0], [0.0, 0.0]],
        "operatingPoint": { "inputs": [40.0, 60.0], "outputs": [40.0, 60.0] }
    }))
    .expect("the group's model decodes");
    let loop_definition = group_loop(2, true);
    let binding = controller_adapter(&loop_definition)
        .expect("the coherent group is served")
        .transfer_binding()
        .expect("the group transfers");
    let (inputs, outputs) = starting_point_symbols(&loop_definition, &model, &binding)
        .expect("the symbols of both members");
    assert_eq!(inputs, vec!["P1.MV0".to_owned(), "P1.MV1".to_owned()]);
    assert_eq!(outputs, vec!["P1.PV0".to_owned(), "P1.PV1".to_owned()]);
}
