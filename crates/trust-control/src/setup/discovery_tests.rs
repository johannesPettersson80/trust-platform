//! The loop discovery and the binding selection, ported from `control-studio-host-bindings.test.ts` (specification 35, B1.2 to B1.4).
//! The channel path of that file is refused by name in this port (stage V7), so its channel assertions run on a loop without
//! channels, with the same selection rules; the bounds are the signals' ranges here, not channel actuator ranges. Its session
//! assertions (the reply's status, the `sessionId`, the open callback) belong to the engine, not ported.

#![allow(clippy::float_cmp)]

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::{
    adapter_compatibility, control_loops_unreadable, discover_bindings, family_name, in_scope,
    refused, select_binding, target_for_loop, BindingSelection, BindingTarget,
    ControllerCompatibility, Discovery, LoopFileRead, LoopFileText, MAXIMUM_LOOP_BYTES,
    MAXIMUM_LOOP_FILES,
};
use crate::failure::{Cause, NextAction};
use crate::loop_definition::{
    ControllerLaw, LoopScope, ModeBinding, Parameter, ParameterBinding, RealType,
};
use crate::setup::binding_route::answer_selection;
use crate::setup::catalog::{
    CatalogAnswer, CompiledCatalog, CompiledController, CompiledSignal, CompiledSymbol,
    CompiledTask,
};
use crate::setup::discovery_test_support::{
    loop_json, loop_of, simc_amigo_manual, simc_amigo_manual_titles, tank_json, CONTROLLER, RUNTIME,
};
use crate::tuning::structure::Structure;

fn symbol(name: &str, iec_type: &str) -> CompiledSymbol {
    CompiledSymbol {
        symbol: name.to_string(),
        iec_type: iec_type.to_string(),
        address: None,
    }
}

fn signal(name: &str, address: &str, iec_type: &str) -> CompiledSignal {
    CompiledSignal {
        symbol: name.to_string(),
        address: address.to_string(),
        iec_type: iec_type.to_string(),
    }
}

/// The compiled program of the tank: one `Control.PID` in a 10 ms task, and the process image of its signals.
fn catalog() -> CatalogAnswer {
    CatalogAnswer {
        control: CompiledCatalog {
            symbols: Vec::new(),
            controllers: vec![CompiledController {
                instance: CONTROLLER.to_string(),
                type_name: "Control.PID".to_string(),
                tasks: vec![CompiledTask {
                    name: "MainTask".to_string(),
                    interval_ms: 10.0,
                }],
            }],
            links: Vec::new(),
            writes: Vec::new(),
        },
        signals: vec![
            signal("Main.SP", "%ML0", "LREAL"),
            signal("Main.PV", "%IL0", "LREAL"),
            signal("Main.MV", "%QL0", "LREAL"),
        ],
    }
}

fn text_file(path: &str, value: &Value) -> LoopFileText {
    LoopFileText {
        configuration_path: path.to_string(),
        read: LoopFileRead::Text(value.to_string()),
    }
}

fn selection(
    loop_id: &str,
    controller: &str,
    inputs: &[&str],
    outputs: &[&str],
) -> BindingSelection {
    BindingSelection {
        loop_id: loop_id.to_string(),
        controller_instance: controller.to_string(),
        input_ids: inputs.iter().map(|id| (*id).to_string()).collect(),
        output_ids: outputs.iter().map(|id| (*id).to_string()).collect(),
    }
}

fn ids(signals: &[super::SelectableSignal]) -> Vec<&str> {
    signals.iter().map(|signal| signal.id.as_str()).collect()
}

/// The reason of every refusal, in the order the discovery reports them.
fn reasons(discovery: &Discovery) -> Vec<&str> {
    discovery
        .rejected
        .iter()
        .map(|rejection| rejection.reason.as_str())
        .collect()
}

#[test]
fn a_configured_loop_with_auxiliary_signals_is_listed_and_only_an_explicit_selection_resolves() {
    // control-studio-host-bindings.test.ts: "lists exact configured channels and resolves only an explicit controller and mapping".
    // The loop has two auxiliary channels (an MV and a PV) and no channel table, so the selectors offer the primary signals first and
    // then the auxiliary ones in declared order. The bounds are the signals' ranges (the channel table is refused by name).
    let mut loop_value = tank_json("loop:tank");
    loop_value["signals"]["additional"] = json!([
        { "id": "mv-secondary", "channel": "secondary", "role": "MV", "symbol": "Main.MV2", "address": "%QL8",
          "iecType": "LREAL", "direction": "write", "unit": "V", "range": { "minimum": -5, "maximum": 5 } },
        { "id": "pv-secondary", "channel": "secondary", "role": "PV", "symbol": "Main.PV2", "address": "%IL8",
          "iecType": "LREAL", "direction": "read", "unit": "m", "range": { "minimum": -3, "maximum": 3 } }
    ]);
    let mut compiled = catalog();
    compiled.signals.push(signal("Main.MV2", "%QL8", "LREAL"));
    compiled.signals.push(signal("Main.PV2", "%IL8", "LREAL"));
    let files = [text_file("/project/control-loop.json", &loop_value)];
    let discovered = discover_bindings("/project", &compiled, RUNTIME, &files);
    assert_eq!(
        reasons(&discovered),
        Vec::<&str>::new(),
        "the loop is accepted"
    );
    assert_eq!(discovered.targets.len(), 1);
    let target = &discovered.targets[0];
    assert_eq!(target.controller.instance, CONTROLLER);
    assert_eq!(target.controller.type_name, "Control.PID");
    assert_eq!(ids(&target.inputs), ["manipulatedVariable", "mv-secondary"]);
    assert_eq!(target.inputs[0].symbol, "Main.MV");
    assert_eq!(target.inputs[1].symbol, "Main.MV2");
    assert_eq!(target.inputs[0].bounds.minimum, 0.0);
    assert_eq!(target.inputs[0].bounds.maximum, 100.0);
    assert_eq!(target.inputs[1].bounds.minimum, -5.0);
    assert_eq!(target.inputs[1].bounds.maximum, 5.0);
    assert_eq!(ids(&target.outputs), ["processVariable", "pv-secondary"]);
    assert_eq!(target.outputs[1].symbol, "Main.PV2");
    assert_eq!(target.outputs[1].bounds.minimum, -3.0);
    assert_eq!(target.outputs[1].bounds.maximum, 3.0);

    let resolved = select_binding(
        &discovered,
        &selection(
            "loop:tank",
            CONTROLLER,
            &["mv-secondary", "manipulatedVariable"],
            &["pv-secondary", "processVariable"],
        ),
    )
    .expect("the complete mapping resolves");
    assert_eq!(
        ids(&resolved.inputs),
        ["manipulatedVariable", "mv-secondary"],
        "declared channel order is retained"
    );
    assert_eq!(
        ids(&resolved.outputs),
        ["processVariable", "pv-secondary"],
        "declared channel order is retained"
    );

    assert!(
        select_binding(
            &discovered,
            &selection(
                "loop:tank",
                CONTROLLER,
                &["manipulatedVariable"],
                &["processVariable"]
            )
        )
        .is_err(),
        "a partial MIMO selection must not leave the full configured loop active"
    );
    assert!(
        select_binding(
            &discovered,
            &selection(
                "loop:tank",
                "Main.Other",
                &["manipulatedVariable"],
                &["processVariable"]
            )
        )
        .is_err(),
        "a controller that is not the compiled target is refused"
    );
    assert!(
        select_binding(
            &discovered,
            &selection("loop:tank", CONTROLLER, &["Main.MV"], &["processVariable"])
        )
        .is_err(),
        "a guessed signal symbol is refused: only declared identities select"
    );
}

#[test]
fn a_loop_whose_controller_type_or_signal_address_changed_in_the_compiled_program_is_rejected() {
    // control-studio-host-bindings.test.ts: "rejects a configured loop when the compiled controller identity or signal identity changed".
    let loop_value = tank_json("loop:tank");
    let files = [text_file("/project/control-loop.json", &loop_value)];

    let mut changed_type = catalog();
    changed_type.control.controllers[0].type_name = "OtherPID".to_string();
    let discovered = discover_bindings("/project", &changed_type, RUNTIME, &files);
    assert_eq!(discovered.targets, [] as [BindingTarget; 0]);
    assert_eq!(
        reasons(&discovered),
        ["Compiled controller Main.TankController has type OtherPID, expected Control.PID."]
    );

    let mut changed_address = catalog();
    changed_address.signals[2].address = "%QL16".to_string();
    let discovered = discover_bindings("/project", &changed_address, RUNTIME, &files);
    assert_eq!(discovered.targets, [] as [BindingTarget; 0]);
    assert_eq!(
        reasons(&discovered),
        ["Declared signal Main.MV does not match the compiled process-image identity."]
    );
}

#[test]
fn a_measurement_without_an_address_is_a_compiled_variable_of_its_type_and_no_other() {
    // control-studio-host-bindings.test.ts: "lists a loop whose measurement is a compiled variable without an address, such as a PID's own".
    let mut loop_value = tank_json("loop:tank");
    loop_value["signals"]["processVariable"]
        .as_object_mut()
        .expect("the measurement is an object")
        .remove("address");
    loop_value["signals"]["processVariable"]["symbol"] = json!("Main.StateValue.ProcessValue");
    let files = [text_file("/project/control-loop.json", &loop_value)];

    let mut compiled = catalog();
    let missing = discover_bindings("/project", &compiled, RUNTIME, &files);
    assert!(
        reasons(&missing).contains(
            &"Declared signal Main.StateValue.ProcessValue is not a compiled LREAL variable."
        ),
        "{:?}",
        reasons(&missing)
    );

    compiled
        .control
        .symbols
        .push(symbol("Main.StateValue.ProcessValue", "LREAL"));
    let discovered = discover_bindings("/project", &compiled, RUNTIME, &files);
    assert_eq!(reasons(&discovered), Vec::<&str>::new());
    let outputs: Vec<(&str, Option<&str>)> = discovered.targets[0]
        .outputs
        .iter()
        .map(|entry| (entry.symbol.as_str(), entry.address.as_deref()))
        .collect();
    assert_eq!(outputs, [("Main.StateValue.ProcessValue", None)]);
}

#[test]
fn duplicate_loop_ids_leave_no_selectable_target_and_name_every_file() {
    // control-studio-host-bindings.test.ts: "rejects duplicate loop IDs instead of exposing an ambiguous target".
    let loop_value = tank_json("loop:tank");
    let files = [
        text_file("/project/control-loop.json", &loop_value),
        text_file("/project/control-loops/same-id.json", &loop_value),
    ];
    let discovered = discover_bindings("/project", &catalog(), RUNTIME, &files);
    assert!(
        discovered.targets.is_empty(),
        "duplicate IDs must not leave one selectable target"
    );
    assert_eq!(
        reasons(&discovered),
        [
            "Duplicate control loop ID loop:tank; the target is ambiguous.",
            "Duplicate control loop ID loop:tank; the target is ambiguous.",
        ]
    );
    assert_eq!(
        discovered.rejected[0].configuration_path,
        "/project/control-loop.json"
    );
    assert_eq!(
        discovered.rejected[1].configuration_path,
        "/project/control-loops/same-id.json"
    );
}

#[test]
fn a_loop_of_another_runtime_still_counts_for_the_duplicate_check() {
    // hostBindingDiscovery.ts: the id is recorded before the runtime is compared, so a foreign loop makes its duplicate ambiguous.
    let files = [
        text_file("/project/control-loop.json", &tank_json("loop:tank")),
        text_file(
            "/project/control-loops/foreign.json",
            &loop_json("loop:tank", "project:sha256:abc", "runtime:other"),
        ),
    ];
    let discovered = discover_bindings("/project", &catalog(), RUNTIME, &files);
    assert_eq!(discovered.targets, [] as [BindingTarget; 0]);
    assert_eq!(
        reasons(&discovered),
        [
            "The configured loop uses runtime:other, but runtime:local is the selected target.",
            "Duplicate control loop ID loop:tank; the target is ambiguous.",
            "Duplicate control loop ID loop:tank; the target is ambiguous.",
        ]
    );
}

#[test]
fn each_refused_file_says_why_in_the_typescript_wording() {
    // hostBindingDiscovery.ts: too large, not JSON, unreadable, not a loop, invalid by its rules, and a loop whose signal units differ.
    let not_json = LoopFileText {
        configuration_path: "/project/control-loops/a.json".to_string(),
        read: LoopFileRead::Text("{ invalid".to_string()),
    };
    let not_loop = text_file("/project/control-loops/b.json", &json!({}));
    let too_large = LoopFileText {
        configuration_path: "/project/control-loops/c.json".to_string(),
        read: LoopFileRead::TooLarge {
            bytes: MAXIMUM_LOOP_BYTES + 1,
        },
    };
    let unreadable = LoopFileText {
        configuration_path: "/project/control-loops/d.json".to_string(),
        read: LoopFileRead::Unreadable,
    };
    let mut mismatched_units = tank_json("loop:units");
    mismatched_units["signals"]["setpoint"]["unit"] = json!("degC");
    let invalid = text_file("/project/control-loops/e.json", &mismatched_units);
    let discovered = discover_bindings(
        "/project",
        &catalog(),
        RUNTIME,
        &[not_json, not_loop, too_large, unreadable, invalid],
    );
    assert_eq!(discovered.targets, [] as [BindingTarget; 0]);
    assert_eq!(
        reasons(&discovered),
        [
            "a.json could not be read as valid JSON.",
            "b.json does not contain a complete Control Studio loop definition.",
            "c.json exceeds the 524288 byte loop-definition bound.",
            "d.json could not be read as valid JSON.",
            "Loop definition is invalid: incompatible units.",
        ]
    );
}

#[test]
fn a_project_declaring_more_loop_files_than_the_bound_is_refused_once_at_its_root() {
    // hostBindingDiscovery.ts: "The project declares more than 128 loop definitions." at the project root (a product limit).
    let over: Vec<LoopFileText> = (0..=MAXIMUM_LOOP_FILES)
        .map(|index| text_file(&format!("/project/control-loops/{index}.json"), &json!({})))
        .collect();
    let refusal = discover_bindings("/project", &catalog(), RUNTIME, &over);
    assert_eq!(refusal.targets, [] as [BindingTarget; 0]);
    assert_eq!(refusal.rejected.len(), 1);
    assert_eq!(refusal.rejected[0].configuration_path, "/project");
    assert_eq!(
        refusal.rejected[0].reason,
        "The project declares more than 128 loop definitions."
    );

    let at_bound: Vec<LoopFileText> = over.into_iter().take(MAXIMUM_LOOP_FILES).collect();
    let discovered = discover_bindings("/project", &catalog(), RUNTIME, &at_bound);
    assert_eq!(
        discovered.rejected.len(),
        MAXIMUM_LOOP_FILES,
        "at the bound every file is read and refused on its own"
    );
}

#[test]
fn a_directory_that_cannot_be_listed_names_the_control_loops_folder() {
    // hostBindingDiscovery.ts: the control-loops directory could not be read; nothing else is discovered.
    let unreadable = control_loops_unreadable("/project");
    assert_eq!(unreadable.targets, [] as [BindingTarget; 0]);
    assert_eq!(
        unreadable.rejected[0].configuration_path,
        "/project/control-loops"
    );
    assert_eq!(
        unreadable.rejected[0].reason,
        "The control-loops directory could not be read."
    );
    let refused_at_root = refused("/project", "Compiled control catalog is unavailable.");
    assert_eq!(refused_at_root.rejected[0].configuration_path, "/project");
}

#[test]
fn a_loop_that_uses_channels_is_refused_by_name_until_the_rust_engine_runs_them() {
    // hostBindingDiscovery.ts's channel path is parked (stage V7): the refusal names the channels.
    let mut loop_definition = loop_of(&tank_json("loop:tank"));
    loop_definition.channels = Some(
        serde_json::from_value(json!({ "inputs": [], "outputs": [] })).expect("typed channels"),
    );
    let refusal = target_for_loop("/project/control-loop.json", loop_definition, &catalog())
        .expect_err("a loop with channels is refused");
    assert_eq!(
        refusal,
        "This loop uses channels, which are not yet supported by the Rust engine."
    );
}

#[test]
fn a_loop_that_declares_a_law_and_the_programs_own_law_is_refused_with_the_gates_sentence() {
    // The engine's gate refuses two controller structures (specification 35, C2). Discovery offers no such loop, and says the gate's
    // sentence, because it calls the gate's check.
    let mut value = tank_json("loop:tank");
    value["controller"]["design"] = json!("project");
    value["controller"]["project"] = json!({
        "family": "PI", "convention": "project-owned-native-controller",
        "initialization": { "kind": "declared-state-v1", "states": [] },
        "seed": { "kind": "parallel-pid", "family": "PI", "proportionalSymbol": "Main.Kp", "integralSymbol": "Main.Ki" },
        "axes": [], "objective": { "requirementId": "target-error", "direction": "minimize" },
        "maximumCandidates": 4, "maximumCycles": 50000, "wallBudgetMs": 60000
    });
    // The program's own law writes its settings through the loop's parameter binding (a complete loop declares one).
    value["parameterBinding"] = json!({
        "loop_id": "loop:tank", "task_name": "MainTask",
        "parameters": [
            { "symbol": "Main.Kp", "iec_type": "LREAL", "unit": "%/%", "convention": "proportional gain", "minimum": 0, "maximum": 100 },
            { "symbol": "Main.Ki", "iec_type": "LREAL", "unit": "%/(%*s)", "convention": "integral gain", "minimum": 0, "maximum": 100 }
        ],
        "mode": { "symbol": "Main.Auto", "manual_value": false }
    });
    let mut both = loop_of(&value);
    both.controller.law = Some(ControllerLaw::Proportional {
        gain_symbol: "Main.Kp".to_string(),
    });
    let refusal = target_for_loop("/project/control-loop.json", both, &catalog())
        .expect_err("a loop with a law and the program's own law is refused");
    assert_eq!(
        refusal,
        "This loop declares both a controller law and the program's own law, so Control Studio cannot tell which one tunes it."
    );
}

#[test]
fn a_pid_group_member_that_the_compiled_program_lacks_is_refused_with_its_name() {
    // hostBindingDiscovery.ts: "Compiled PID group member … is unavailable in the declared task." (C7; the group is read as declared).
    let mut loop_definition = loop_of(&tank_json("loop:tank"));
    loop_definition.controller.group = Some(
        serde_json::from_value(json!({ "members": [{ "instance": "Main.Other", "type": "Control.PID",
            "inputChannelId": "u1", "outputChannelId": "y1", "modeSymbol": "Main.Auto", "parameterSymbols": ["Main.Kp"] }] }))
            .expect("a typed group member"),
    );
    let refusal = target_for_loop("/project/control-loop.json", loop_definition, &catalog())
        .expect_err("the member is missing");
    assert_eq!(
        refusal,
        "Compiled PID group member Main.Other is unavailable in the declared task."
    );
}

#[test]
fn a_declared_controller_symbol_must_exist_with_the_declared_type() {
    // hostBindingDiscovery.ts: the parameter symbols and their types (C0): missing, then the wrong type.
    let mut loop_definition = loop_of(&tank_json("loop:tank"));
    loop_definition.parameter_binding = Some(ParameterBinding {
        loop_id: "loop:tank".to_string(),
        task_name: "MainTask".to_string(),
        parameters: vec![Parameter {
            symbol: "Main.Parameters.Kp".to_string(),
            iec_type: RealType::Lreal,
            unit: "%/degC".to_string(),
            convention: "proportional gain".to_string(),
            minimum: 0.0,
            maximum: 10.0,
        }],
        mode: ModeBinding {
            symbol: "Main.Auto".to_string(),
            manual_value: false,
        },
    });
    let mut compiled = catalog();
    compiled.control.symbols.push(symbol("Main.Auto", "BOOL"));
    let refusal = target_for_loop(
        "/project/control-loop.json",
        loop_definition.clone(),
        &compiled,
    )
    .expect_err("the parameter is missing");
    assert_eq!(
        refusal,
        "Declared controller symbol Main.Parameters.Kp is unavailable in the compiled catalog."
    );

    compiled
        .control
        .symbols
        .push(symbol("Main.Parameters.Kp", "REAL"));
    let refusal = target_for_loop("/project/control-loop.json", loop_definition, &compiled)
        .expect_err("the type differs");
    assert_eq!(
        refusal,
        "Compiled controller symbol Main.Parameters.Kp has type REAL, expected LREAL."
    );
}

#[test]
fn a_loop_with_no_parameter_record_or_law_is_unconfigured_and_a_refused_adapter_keeps_its_reason() {
    // hostBindingDiscovery.ts's adapterCompatibility: unconfigured without a binding, law, program controller or ON/OFF or state
    // controller; an adapter's refusal is kept as the loop's reason.
    let plain = loop_of(&tank_json("loop:tank"));
    assert_eq!(
        adapter_compatibility(&plain),
        ControllerCompatibility::Unconfigured
    );

    let mut proportional = loop_of(&tank_json("loop:tank"));
    proportional.controller.law = Some(crate::loop_definition::ControllerLaw::Proportional {
        gain_symbol: "Main.Kp".to_string(),
    });
    match adapter_compatibility(&proportional) {
        ControllerCompatibility::Rejected { reason } => {
            assert!(!reason.is_empty(), "the adapter's reason is kept");
        }
        other => panic!("a law without its parameter record is refused, not accepted: {other:?}"),
    }
}

#[test]
fn a_served_program_law_is_accepted_with_the_families_its_seed_names() {
    // C2 Setup: P always, and PI where the seed names the integral variable; the engine serves the shape and offers these families.
    // C4: the program's own law takes SIMC, AMIGO and Manual, as a PID law does, and the page offers exactly the methods it is given.
    let mut value = tank_json("loop:tank");
    value["controller"]["design"] = json!("project");
    value["controller"]["project"] = json!({
        "family": "PI", "convention": "project-owned-native-controller",
        "initialization": { "kind": "declared-state-v1", "states": [] },
        "seed": { "kind": "parallel-pid", "family": "PI", "proportionalSymbol": "Main.Kp", "integralSymbol": "Main.Ki" },
        "axes": [], "objective": { "requirementId": "target-error", "direction": "minimize" },
        "maximumCandidates": 4, "maximumCycles": 50000, "wallBudgetMs": 60000
    });
    value["parameterBinding"] = json!({
        "loop_id": "loop:tank", "task_name": "MainTask",
        "parameters": [
            { "symbol": "Main.Kp", "iec_type": "LREAL", "unit": "%/%", "convention": "proportional gain", "minimum": 0, "maximum": 100 },
            { "symbol": "Main.Ki", "iec_type": "LREAL", "unit": "%/(%*s)", "convention": "integral gain", "minimum": 0, "maximum": 100 }
        ],
        "mode": { "symbol": "Main.Auto", "manual_value": false }
    });
    assert_eq!(
        adapter_compatibility(&loop_of(&value)),
        ControllerCompatibility::Accepted {
            recommended_family: None,
            methods: Some(simc_amigo_manual()),
            method_titles: Some(simc_amigo_manual_titles()),
            families: vec!["P".to_string(), "PI".to_string()],
            family_settings: BTreeMap::from([
                ("P".to_string(), vec!["Main.Kp".to_string()]),
                (
                    "PI".to_string(),
                    vec!["Main.Kp".to_string(), "Main.Ki".to_string()]
                ),
            ]),
        }
    );
}

#[test]
fn a_served_pid_seed_names_the_settings_each_of_its_families_takes() {
    // C2 Setup and C0: a PID seed offers P, PI, PD and PID, and each family's manual settings and pins take its own variables.
    // C4: the program's own PID takes SIMC, AMIGO and Manual, as a PID law does.
    let mut value = tank_json("loop:tank");
    value["controller"]["design"] = json!("project");
    value["controller"]["project"] = json!({
        "family": "PID", "convention": "project-owned-native-controller",
        "initialization": { "kind": "declared-state-v1", "states": [] },
        "seed": { "kind": "parallel-pid", "family": "PID", "proportionalSymbol": "Main.Kp",
            "integralSymbol": "Main.Ki", "derivativeSymbol": "Main.Kd" },
        "axes": [], "objective": { "requirementId": "target-error", "direction": "minimize" },
        "maximumCandidates": 4, "maximumCycles": 50000, "wallBudgetMs": 60000
    });
    value["parameterBinding"] = json!({
        "loop_id": "loop:tank", "task_name": "MainTask",
        "parameters": [
            { "symbol": "Main.Kp", "iec_type": "LREAL", "unit": "%/%", "convention": "proportional gain", "minimum": 0, "maximum": 100 },
            { "symbol": "Main.Ki", "iec_type": "LREAL", "unit": "%/(%*s)", "convention": "integral gain", "minimum": 0, "maximum": 100 },
            { "symbol": "Main.Kd", "iec_type": "LREAL", "unit": "%*s/%", "convention": "derivative gain", "minimum": 0, "maximum": 100 }
        ],
        "mode": { "symbol": "Main.Auto", "manual_value": false }
    });
    let kp = || "Main.Kp".to_string();
    assert_eq!(
        adapter_compatibility(&loop_of(&value)),
        ControllerCompatibility::Accepted {
            recommended_family: None,
            methods: Some(simc_amigo_manual()),
            method_titles: Some(simc_amigo_manual_titles()),
            families: vec!["P".into(), "PI".into(), "PD".into(), "PID".into()],
            family_settings: BTreeMap::from([
                ("P".to_string(), vec![kp()]),
                ("PI".to_string(), vec![kp(), "Main.Ki".to_string()]),
                ("PD".to_string(), vec![kp(), "Main.Kd".to_string()]),
                (
                    "PID".to_string(),
                    vec![kp(), "Main.Ki".to_string(), "Main.Kd".to_string()]
                ),
            ]),
        }
    );
}

#[test]
fn the_compatibility_of_a_loop_serializes_with_its_kind_and_families() {
    // hostBindingDiscovery.ts's ControllerAdapterCompatibility: { kind: "unconfigured" } | { kind: "accepted", families } | { kind: "rejected", reason }.
    assert_eq!(
        serde_json::to_value(ControllerCompatibility::Unconfigured).expect("json"),
        json!({ "kind": "unconfigured" })
    );
    assert_eq!(
        serde_json::to_value(ControllerCompatibility::Accepted {
            recommended_family: None,
            methods: None,
            method_titles: None,
            families: vec!["P".to_string(), "PID".to_string()],
            family_settings: BTreeMap::from([
                ("P".to_string(), vec!["Main.Kp".to_string()]),
                (
                    "PID".to_string(),
                    vec!["Main.Kp".to_string(), "Main.Ki".to_string()]
                ),
            ]),
        })
        .expect("json"),
        json!({
            "kind": "accepted",
            "families": ["P", "PID"],
            "familySettings": { "P": ["Main.Kp"], "PID": ["Main.Kp", "Main.Ki"] }
        })
    );
    assert_eq!(
        serde_json::to_value(ControllerCompatibility::Rejected {
            reason: "No law.".to_string()
        })
        .expect("json"),
        json!({ "kind": "rejected", "reason": "No law." })
    );
}

#[test]
fn each_structure_names_its_family_as_the_setup_page_shows_it() {
    // The adapter realizes P, PI, PD and PID; the setup names them in the same words.
    assert_eq!(family_name(Structure::P), "P");
    assert_eq!(family_name(Structure::PI), "PI");
    assert_eq!(family_name(Structure::PD), "PD");
    assert_eq!(family_name(Structure::PID), "PID");
}

#[test]
fn the_listing_of_one_scope_keeps_the_other_projects_out_and_the_rejections_whole() {
    // hostBindingSession.ts: the list and the selection come from the targets of the scope (project and runtime); a loop of another
    // project is not offered, and the rejections are reported whole.
    let files = [
        text_file("/project/control-loop.json", &tank_json("loop:tank")),
        text_file(
            "/other/control-loop.json",
            &loop_json("loop:other", "project:sha256:other", RUNTIME),
        ),
        text_file("/project/control-loops/broken.json", &json!({})),
    ];
    let discovered = discover_bindings("/project", &catalog(), RUNTIME, &files);
    assert_eq!(discovered.targets.len(), 2, "both loops are discovered");
    let scope = LoopScope {
        project_id: "project:sha256:abc".to_string(),
        runtime_id: RUNTIME.to_string(),
    };
    let listed = in_scope(discovered.clone(), &scope);
    assert_eq!(listed.targets.len(), 1);
    assert_eq!(listed.targets[0].loop_definition.id, "loop:tank");
    assert_eq!(
        listed.rejected, discovered.rejected,
        "the rejections are reported whole"
    );

    assert!(
        select_binding(
            &listed,
            &selection(
                "loop:other",
                CONTROLLER,
                &["manipulatedVariable"],
                &["processVariable"]
            )
        )
        .is_err(),
        "a loop outside the scope cannot be selected"
    );
    assert!(select_binding(
        &listed,
        &selection(
            "loop:tank",
            CONTROLLER,
            &["manipulatedVariable"],
            &["processVariable"]
        )
    )
    .is_ok());
}

#[test]
fn a_selection_names_every_declared_channel_once_and_says_which_is_missing() {
    // hostBindingDiscovery.ts's selectedInDeclaredOrder: at least one, every declared, each declared once.
    let discovered = discover_bindings(
        "/project",
        &catalog(),
        RUNTIME,
        &[text_file(
            "/project/control-loop.json",
            &tank_json("loop:tank"),
        )],
    );
    let refusal = |inputs: &[&str], outputs: &[&str]| {
        select_binding(
            &discovered,
            &selection("loop:tank", CONTROLLER, inputs, outputs),
        )
        .expect_err("the selection is refused")
        .message
    };
    assert_eq!(
        refusal(&[], &["processVariable"]),
        "Choose at least one input channel."
    );
    assert_eq!(
        refusal(&["manipulatedVariable"], &[]),
        "Choose at least one output channel."
    );
    assert_eq!(
        refusal(
            &["manipulatedVariable", "manipulatedVariable"],
            &["processVariable"]
        ),
        "Select every declared input channel before opening this loop."
    );
    assert_eq!(
        refusal(&["Main.MV"], &["processVariable"]),
        "The selected input channel identity is not declared by the loop."
    );
    assert_eq!(
        select_binding(
            &discovered,
            &selection(
                "loop:none",
                CONTROLLER,
                &["manipulatedVariable"],
                &["processVariable"]
            )
        )
        .expect_err("the loop is not listed")
        .message,
        "No discovered loop has ID loop:none."
    );
}

#[test]
fn a_selection_decodes_camel_case_and_refuses_an_unknown_field() {
    // messages: the selection travels as camelCase JSON, and an unknown field is refused, never ignored.
    let decoded: BindingSelection = serde_json::from_value(json!({
        "loopId": "loop:tank", "controllerInstance": CONTROLLER, "inputIds": ["manipulatedVariable"], "outputIds": ["processVariable"],
    }))
    .expect("the selection decodes");
    assert_eq!(
        decoded,
        selection(
            "loop:tank",
            CONTROLLER,
            &["manipulatedVariable"],
            &["processVariable"]
        )
    );
    assert!(serde_json::from_value::<BindingSelection>(json!({
        "loopId": "loop:tank", "controllerInstance": CONTROLLER, "inputIds": [], "outputIds": [], "bogus": 1,
    }))
    .is_err());
}

#[test]
fn the_discovery_of_a_loop_and_its_target_serialize_with_the_setup_page_names() {
    // The setup page reads the target's controller type as `type` and its signals in camelCase.
    let discovered = discover_bindings(
        "/project",
        &catalog(),
        RUNTIME,
        &[text_file(
            "/project/control-loop.json",
            &tank_json("loop:tank"),
        )],
    );
    let value = serde_json::to_value(&discovered).expect("json");
    assert_eq!(value["targets"][0]["controller"]["type"], "Control.PID");
    assert_eq!(value["targets"][0]["inputs"][0]["iecType"], "LREAL");
    assert_eq!(
        value["targets"][0]["controllerCompatibility"]["kind"],
        "unconfigured"
    );
    assert_eq!(value["rejected"], json!([]));
}

#[test]
fn an_empty_discovery_has_no_target_and_no_refusal() {
    assert_eq!(Discovery::default().targets.len(), 0);
    let discovered = discover_bindings("/project", &catalog(), RUNTIME, &[]);
    assert_eq!(discovered, Discovery::default());
}

#[test]
fn the_engine_answers_a_selection_with_the_loop_it_names_or_with_the_refusal_that_stops_it() {
    // The engine's rule for the three-page panel's selection (B1.3, C0): the shape, the listed loop, its channels, and the family.
    let discovered = discover_bindings(
        "/project",
        &catalog(),
        RUNTIME,
        &[text_file(
            "/project/control-loop.json",
            &tank_json("loop:tank"),
        )],
    );
    let named = answer_selection(
        &discovered,
        &selection(
            "loop:tank",
            CONTROLLER,
            &["manipulatedVariable"],
            &["processVariable"],
        ),
        None,
    )
    .expect("the selection names the listed loop");
    assert_eq!(named.configuration_path, "/project/control-loop.json");

    let refusal = |chosen: BindingSelection, family: Option<&str>| {
        answer_selection(&discovered, &chosen, family).expect_err("the selection is refused")
    };
    let shape = refusal(
        selection("loop:tank", CONTROLLER, &[], &["processVariable"]),
        None,
    );
    assert_eq!(
        (shape.cause, shape.next_action),
        (Cause::Implementation, NextAction::OpenLoopSetup)
    );
    let unknown_family = refusal(
        selection(
            "loop:tank",
            CONTROLLER,
            &["manipulatedVariable"],
            &["processVariable"],
        ),
        Some("XYZ"),
    );
    assert_eq!(unknown_family.cause, Cause::Implementation);
    let unlisted = refusal(
        selection(
            "loop:none",
            CONTROLLER,
            &["manipulatedVariable"],
            &["processVariable"],
        ),
        None,
    );
    assert_eq!(unlisted.message, "No discovered loop has ID loop:none.");
    // The tank has no parameter record or law, so its adapter realizes no family (C0).
    let unrealized = refusal(
        selection(
            "loop:tank",
            CONTROLLER,
            &["manipulatedVariable"],
            &["processVariable"],
        ),
        Some("PI"),
    );
    assert_eq!(
        (unrealized.cause, unrealized.next_action),
        (Cause::ControllerCapability, NextAction::ChooseMethod)
    );
}

/// A project-designed loop's type describes the controller its program owns (B1.2), as the course's loops name theirs ("LQR state
/// feedback" on the program `Main`): its type is not compared with the compiled one, while a PID-designed loop's still is.
#[test]
fn a_project_designed_loop_is_not_refused_for_the_type_that_describes_its_controller() {
    let mut project_designed = tank_json("loop:tank");
    project_designed["controller"]["design"] = json!("project");
    project_designed["controller"]["type"] = json!("Tank level observation");
    let controller = project_designed["controller"]
        .as_object_mut()
        .expect("controller");
    controller.remove("law");
    controller.remove("pidInitialization");
    project_designed["experiment"] = json!({
        "execution": "project-scripted", "kind": "observation", "amplitude": 0, "durationSeconds": 20, "seed": 1
    });
    let files = [text_file("/project/control-loop.json", &project_designed)];
    let discovered = discover_bindings("/project", &catalog(), RUNTIME, &files);
    assert!(
        !reasons(&discovered)
            .iter()
            .any(|reason| reason.contains("has type")),
        "{:?}",
        reasons(&discovered)
    );
}
