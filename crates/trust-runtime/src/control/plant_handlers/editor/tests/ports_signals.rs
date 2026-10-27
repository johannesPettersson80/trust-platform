//! The plant's ports and the PLC signals they bind to (specification 47 §8; specification 46 §6.1,
//! §13).

use serde_json::json;

use super::{draft, found};
use crate::control::plant_handlers::editor::ports;
use crate::control::plant_handlers::editor::signals::{self, Candidate, Catalog};

fn candidate(symbol: &str, address: &str, data_type: &str) -> Candidate {
    Candidate {
        symbol: symbol.into(),
        address: address.into(),
        data_type: data_type.into(),
        current_value: None,
    }
}

fn catalog() -> Catalog {
    Catalog {
        ready: true,
        commands: vec![candidate("Main.Valve", "%QW0", "INT")],
        feedbacks: vec![
            candidate("Main.Level", "%IW0", "INT"),
            candidate("Main.Speed", "%IW2", "INT"),
        ],
        physical_drivers: Vec::new(),
        error: None,
    }
}

#[test]
fn each_model_names_its_ports() {
    // Specification 47 §8: the robot joint has one torque command and angle and speed feedback; a
    // state-space model has one port per input and output. Catches a port count that does not
    // follow the model.
    let labels = |value: serde_json::Value| ports::labels(&draft(value));
    assert_eq!(
        labels(json!({"model": "first_order"})),
        (
            vec!["Command / PLC output".to_string()],
            vec!["Feedback / PLC input".to_string()]
        )
    );
    assert_eq!(
        labels(json!({"model": "robot_joint"})),
        (
            vec!["Torque command (N·m)".to_string()],
            vec![
                "Joint angle (rad)".to_string(),
                "Angular speed (rad/s)".to_string()
            ]
        )
    );
    assert_eq!(
        labels(json!({"model": "gravity_tank_cascade"})),
        (
            vec!["Pump flow (m³/s)".to_string()],
            vec![
                "Upstream level (m)".to_string(),
                "Downstream level (m)".to_string()
            ]
        )
    );
    assert_eq!(
        labels(
            json!({"model": "state_space", "parameters": {"a": [[0]], "b": [[1, 1]], "c": [[1], [1], [1]], "d": [[0, 0], [0, 0], [0, 0]]}})
        ),
        (
            vec!["Input 1".to_string(), "Input 2".to_string()],
            vec![
                "Output 1".to_string(),
                "Output 2".to_string(),
                "Output 3".to_string()
            ]
        )
    );
}

#[test]
fn an_address_must_be_a_usable_compiled_signal_of_its_direction() {
    // Specification 47 §8: an address is invalid when absent from the compiled catalog, of the
    // opposite direction, a feedback another plant writes, or owned by enabled physical I/O; the
    // project must have compiled. Each case catches one refusal missing.
    let refused = |catalog: &Catalog, direction: &str, address: &str, used: &[&str]| {
        let used: Vec<String> = used.iter().map(|used| (*used).to_string()).collect();
        ports::check_address(catalog, direction, address, &used).unwrap_err()
    };
    let unavailable = Catalog::default();
    assert_eq!(
        refused(&unavailable, "command", "%QW0", &[]),
        (
            "catalog_unavailable",
            "Compile the project successfully before entering a PLC signal.".to_string()
        )
    );
    assert_eq!(
        refused(&catalog(), "command", "%iw0", &[]),
        (
            "wrong_direction",
            "Use a compiled numeric %Q output or %M memory value for a command.".to_string()
        )
    );
    assert_eq!(
        refused(&catalog(), "command", "%QW9", &[]),
        (
            "not_compiled",
            "Enter or choose a compiled numeric %Q output or %M memory value.".to_string()
        )
    );
    assert_eq!(
        refused(&catalog(), "feedback", "%QW0", &[]),
        (
            "wrong_direction",
            "Use a compiled numeric %I input for feedback.".to_string()
        )
    );
    assert_eq!(
        refused(&catalog(), "feedback", "%IW0", &["%iw0"]),
        (
            "duplicate_feedback",
            "Choose a different %I input; %IW0 is already used by another plant.".to_string()
        )
    );
    let mut physical = catalog();
    physical.physical_drivers = vec!["Modbus TCP".to_string()];
    assert_eq!(
        refused(&physical, "command", "%qw0", &[]),
        (
            "physical_io_enabled",
            "%QW0 is owned by enabled physical I/O (Modbus TCP), so simulated command cannot read it. Disable Modbus TCP in Devices & Connections or choose another compiled %Q output."
                .to_string()
        )
    );
    assert_eq!(
        ports::check_address(&catalog(), "command", " %qw0 ", &[]),
        Ok("%QW0".to_string()),
        "a typed address takes the compiled spelling"
    );
}

#[test]
fn every_port_is_bound_once_to_a_usable_signal() {
    // Specification 47 §8: an unbound port stays visible and blocks Save; a feedback target has one
    // owner across the plants; an extra binding after a model change is reviewed. Catches a missing
    // binding, a shared feedback or a stale extra port accepted.
    let value =
        json!({"model": "first_order", "commandSources": [""], "feedbackTargets": ["%IW0"]});
    assert_eq!(
        found(&ports::checks(&draft(value), &catalog(), &["%IW0".to_string()])),
        [
            ("command_sources[0]".to_string(), "Choose a PLC output for Command / PLC output.".to_string()),
            (
                "feedback_targets[0]".to_string(),
                "Feedback / PLC input: Choose a different %I input; %IW0 is already used by another plant.".to_string()
            ),
        ]
    );
    let extra = json!({"model": "first_order", "commandSources": ["%QW0"], "feedbackTargets": ["%IW0", "%IW2"]});
    assert_eq!(
        found(&ports::checks(&draft(extra), &catalog(), &[])),
        [(
            "feedback_targets[1]".to_string(),
            "This model has 1 feedback port(s). Review the extra binding before saving."
                .to_string()
        )]
    );
    let joint = json!({"model": "robot_joint", "commandSources": ["%QW0"], "feedbackTargets": ["%IW0", "%IW0"]});
    assert_eq!(
        found(&ports::checks(&draft(joint), &catalog(), &[])),
        [
            (
                "feedback_targets[0]".to_string(),
                "Joint angle (rad): Choose a different %I input; %IW0 is already used by another plant.".to_string()
            ),
            (
                "feedback_targets[1]".to_string(),
                "Angular speed (rad/s): Choose a different %I input; %IW0 is already used by another plant.".to_string()
            ),
        ],
        "two ports of one plant may not write the same input either"
    );
}

#[test]
fn a_binding_keeps_every_other_port_and_grows_its_conversions() {
    // Specification 47 §8: clearing or changing a binding keeps every other port's index and
    // conversion; a new port starts at scale 1 and offset 0. Catches ports that shift on a
    // change, or a new port without its conversion.
    let joint = draft(json!({
        "model": "robot_joint", "commandSources": ["%QW0"], "feedbackTargets": ["%IW0"],
        "outputScale": [2], "outputOffset": [1], "dirty": false,
    }));
    let bound = ports::bind(&joint, "feedback", 1, "%IW2");
    assert_eq!(bound["feedbackTargets"], json!(["%IW0", "%IW2"]));
    assert_eq!(bound["outputScale"], json!([2, 1]));
    assert_eq!(bound["outputOffset"], json!([1, 0]));
    assert_eq!(bound["commandSources"], json!(["%QW0"]));
    assert_eq!(bound["dirty"], json!(true));
    let cleared = ports::bind(&bound, "feedback", 0, "");
    assert_eq!(
        cleared["feedbackTargets"],
        json!(["", "%IW2"]),
        "the other port keeps its index"
    );
    assert_eq!(
        ports::bind(&joint, "feedback", 5, "%IW2"),
        joint,
        "a port the model lacks changes nothing"
    );
}

#[test]
fn the_catalog_offers_compiled_numeric_signals_with_current_values() {
    // Specification 47 §8: candidates come from a successful compilation (commands from %Q
    // outputs and %M memory, feedbacks from %I inputs), current values are an overlay matched by
    // canonical address, and a failed compilation offers nothing and says why. Catches a stale or
    // unresolved live value shown as current, or a symbol-less binding offered.
    let compiled = json!({
        "ok": true,
        "processImage": {
            "commands": [{"symbol": "Main.Valve", "address": "%QW0", "dataType": "int"}],
            "memory": [{"symbol": "Main.Setpoint", "address": "%MD4", "type": "REAL"}],
            "feedbacks": [
                {"symbol": "Main.Level", "address": "%IW0", "dataType": "INT"},
                {"symbol": "", "address": "%IW2", "dataType": "INT"},
            ],
        },
    });
    let live = json!({
        "outputs": [{"address": "%qw0", "value": "12"}],
        "inputs": [{"address": "%IW0", "value": "unresolved"}],
        "memory": [],
    });
    let built = signals::build(Some(&compiled), Some(&live), None, None);
    let mut valve = candidate("Main.Valve", "%QW0", "INT");
    valve.current_value = Some("12".into());
    assert!(built.ready);
    assert_eq!(
        built.commands,
        [valve, candidate("Main.Setpoint", "%MD4", "REAL")]
    );
    assert_eq!(built.feedbacks, [candidate("Main.Level", "%IW0", "INT")]);
    let failed = signals::build(
        Some(&json!({"ok": false, "error": " boom "})),
        None,
        None,
        None,
    );
    assert!(!failed.ready && failed.commands.is_empty());
    assert_eq!(failed.error.as_deref(), Some("boom"));
    assert_eq!(
        signals::build(Some(&json!({"ok": false})), None, None, None)
            .error
            .as_deref(),
        Some("The current project did not compile.")
    );
    assert_eq!(
        signals::build(None, None, None, None).error.as_deref(),
        Some("Compiled signal metadata is unavailable.")
    );
    assert_eq!(
        Catalog::from_json(Some(&built.to_json())),
        built,
        "the catalog the editor sends back reads as it was built"
    );
}

#[test]
fn only_an_enabled_driver_that_touches_real_io_is_physical() {
    // Specification 47 §8 with specification 46 §6.1 item 5: drivers named simulated, sim, noop,
    // loopback or none touch no real I/O, and a disabled driver does not block. Catches a
    // `noop`, `sim` or `none` driver blocking plant signals, or a disabled one.
    let schema = json!({"protocols": [
        {"id": "modbus_tcp", "title": "Modbus TCP", "category": "field_device", "instances": [{"enabled": true}]},
        {"id": "simulated", "category": "field_device", "instances": [{}]},
        {"id": "noop", "category": "field_device", "instances": [{}]},
        {"id": "none", "category": "field_device", "instances": [{}]},
        {"id": "ethercat", "title": "EtherCAT", "category": "field_device", "instances": [{"enabled": false}]},
        {"id": "mqtt", "title": "MQTT", "category": "connector", "instances": [{}]},
    ]});
    let topology = json!({"hosts": [{
        "runtimes": [{"endpoints": [
            {"protocol": "mqtt", "category": "field_device", "display_name": "MQTT broker"},
            {"protocol": "sim", "kind": "field"},
            {"protocol": "modbus_tcp", "category": "field_device", "health": "disabled", "name": "x"},
        ]}],
        "containers": [{"runtimes": [{"endpoints": [{"protocol": "opcua", "kind": "field", "name": "OPC UA"}]}]}],
    }]});
    assert_eq!(
        signals::physical_drivers(Some(&schema), Some(&topology)),
        ["Modbus TCP", "MQTT broker", "OPC UA"]
    );
}
