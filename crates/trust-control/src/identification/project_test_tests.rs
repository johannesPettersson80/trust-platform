#![allow(clippy::float_cmp)]

use serde_json::{json, Value};

use super::*;
use crate::loop_definition::{Direction, LoopSignal, Parameter, Range};

/// A loop on the PID of the method tests, with a setpoint, a measured output, a manipulated variable and a Manual command. Its
/// experiment is `experiment`, and `extra` adds keys to the loop file (a parameter binding or an analysis).
fn loop_with(experiment: &Value, extra: &Value) -> LoopDefinition {
    let mut file = json!({
        "schemaVersion": 2, "id": "pid", "name": "PID", "revision": "sha256:pid",
        "projectId": "test", "runtimeId": "test",
        "controller": {
            "instance": "Main.PID", "type": "Control.PID", "revision": "sha256:controller",
            "action": "direct", "taskPeriodMs": 100.0,
            "manualOutputSignal": "Main.PID_Manual", "manualOutputType": "REAL"
        },
        "observedSampleTiming": { "nominalPeriodMs": 100.0 },
        "signals": {
            "setpoint": { "symbol": "Main.SP", "iecType": "LREAL", "direction": "read", "unit": "degC",
                "range": { "minimum": 0.0, "maximum": 100.0 } },
            "processVariable": { "symbol": "Main.PV", "iecType": "LREAL", "direction": "read", "unit": "degC",
                "range": { "minimum": 0.0, "maximum": 100.0 } },
            "manipulatedVariable": { "symbol": "Main.MV", "iecType": "LREAL", "direction": "write", "unit": "%",
                "range": { "minimum": 0.0, "maximum": 100.0 } }
        },
        "limits": { "outputMinimum": 0.0, "outputMaximum": 100.0 },
        "experiment": experiment
    });
    if let (Some(file), Some(extra)) = (file.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            file.insert(key.clone(), value.clone());
        }
    }
    serde_json::from_value(file).expect("the fixture loop decodes")
}

fn project_experiment() -> Value {
    json!({ "execution": "project-scripted", "kind": "observation", "amplitude": 0.0, "seed": 1 })
}

fn binding() -> Value {
    json!({
        "loop_id": "pid", "task_name": "MainTask", "parameters": [],
        "mode": { "symbol": "Main.Run", "manual_value": true }
    })
}

#[test]
fn a_control_studio_loop_has_no_project_test_form() {
    let loop_definition = loop_with(
        &json!({ "execution": "control-studio", "kind": "observation", "amplitude": 0.0, "seed": 1 }),
        &json!({}),
    );
    assert_eq!(project_test_form(&loop_definition), None);
}

#[test]
fn a_project_test_offers_the_setpoint_process_value_and_applied_output_selected() {
    let form = project_test_form(&loop_with(&project_experiment(), &json!({})))
        .expect("a project-scripted loop has a form");
    let signals: Vec<(&str, &str, bool)> = form
        .signals
        .iter()
        .map(|signal| (signal.id.as_str(), signal.label.as_str(), signal.selected))
        .collect();
    assert_eq!(
        signals,
        [
            ("sp", "Main.SP · Setpoint", true),
            ("pv", "Main.PV · Process value", true),
            ("mv", "Main.MV · Applied output", true),
        ]
    );
    assert_eq!(form.signals[0].unit.as_deref(), Some("degC"));
    assert_eq!(form.signals[2].unit.as_deref(), Some("%"));
}

#[test]
fn an_actuator_names_the_applied_output_and_adds_the_unselected_requested_output() {
    let mut loop_definition = loop_with(&project_experiment(), &json!({}));
    loop_definition.signals.actuator = Some(LoopSignal {
        symbol: "Main.ValveAct".to_owned(),
        address: None,
        iec_type: "LREAL".to_owned(),
        direction: Direction::Read,
        unit: "%".to_owned(),
        range: Range {
            minimum: 0.0,
            maximum: 100.0,
        },
    });
    let form = project_test_form(&loop_definition).expect("a project-scripted loop has a form");
    let mv = form
        .signals
        .iter()
        .find(|signal| signal.id == "mv")
        .expect("the applied output");
    assert_eq!(mv.label, "Main.ValveAct · Applied output");
    let requested = form
        .signals
        .iter()
        .find(|signal| signal.id == "requestedMv")
        .expect("the requested output");
    assert_eq!(requested.label, "Main.MV · Requested output");
    assert!(!requested.selected);
}

#[test]
fn an_additional_setpoint_signal_is_selected_only_when_the_loop_has_no_setpoint() {
    let candidate = json!({
        "id": "sp2", "channel": "reference", "role": "SP", "symbol": "Main.SP2", "iecType": "LREAL",
        "direction": "read", "unit": "degC", "range": { "minimum": 0.0, "maximum": 100.0 }
    });
    let mut without_setpoint = loop_with(&project_experiment(), &json!({}));
    without_setpoint.signals.setpoint = None;
    without_setpoint.signals.additional = Some(vec![
        serde_json::from_value(candidate.clone()).expect("the signal decodes")
    ]);
    let form = project_test_form(&without_setpoint).expect("a project-scripted loop has a form");
    let added = form.signals.last().expect("the additional signal");
    assert_eq!(added.id, "sp2");
    assert_eq!(added.label, "Main.SP2 · SP");
    assert_eq!(added.unit.as_deref(), Some("degC"));
    assert!(added.selected);

    let mut with_setpoint = without_setpoint.clone();
    with_setpoint.signals.setpoint = Some(
        serde_json::from_value(
            json!({ "symbol": "Main.SP", "iecType": "LREAL", "direction": "read", "unit": "degC",
            "range": { "minimum": 0.0, "maximum": 100.0 } }),
        )
        .expect("the setpoint decodes"),
    );
    let form = project_test_form(&with_setpoint).expect("a project-scripted loop has a form");
    assert!(!form.signals.last().expect("the additional signal").selected);
}

#[test]
fn the_duration_defaults_to_thirty_seconds_and_the_baseline_shows_only_when_positive() {
    let undeclared =
        project_test_form(&loop_with(&project_experiment(), &json!({}))).expect("a form");
    assert_eq!(undeclared.duration_seconds, DEFAULT_PROJECT_TEST_SECONDS);
    assert_eq!(undeclared.baseline_seconds, None);

    let declared = project_test_form(&loop_with(
        &json!({ "execution": "project-scripted", "kind": "observation", "amplitude": 0.0, "seed": 1,
            "durationSeconds": 45.0, "baselineSeconds": 5.0 }),
        &json!({}),
    ))
    .expect("a form");
    assert_eq!(declared.duration_seconds, 45.0);
    assert_eq!(declared.baseline_seconds, Some(5.0));

    let zero_baseline = project_test_form(&loop_with(
        &json!({ "execution": "project-scripted", "kind": "observation", "amplitude": 0.0, "seed": 1, "baselineSeconds": 0.0 }),
        &json!({}),
    ))
    .expect("a form");
    assert_eq!(zero_baseline.baseline_seconds, None);
}

// Specification 35, §0.1a, B6.1 and C12: the form names the tool `controlStudio/parameters` serves: the parameters of a declared analysis
// when the loop maps any, else the experiment's declared settings. A loop with neither, or without a binding, has no tool.
#[test]
fn the_parameter_tool_is_the_analysis_for_a_declared_analysis_the_experiment_for_experiment_settings_and_none_otherwise(
) {
    let none = project_test_form(&loop_with(&project_experiment(), &json!({}))).expect("a form");
    assert_eq!(none.parameter_tool, None, "no parameter binding, no tool");

    let unnamed = loop_with(
        &project_experiment(),
        &json!({ "parameterBinding": binding() }),
    );
    assert_eq!(
        parameter_tool(&unnamed),
        None,
        "a binding that declares no purpose and maps no analysis is the controller's, not a tool"
    );

    let experiment = project_test_form(&loop_with(
        &project_experiment(),
        &json!({ "parameterBinding": binding(), "parameterPurpose": "experiment" }),
    ))
    .expect("a form");
    assert_eq!(experiment.parameter_tool, Some(ParameterTool::Experiment));

    let controller = loop_with(
        &project_experiment(),
        &json!({ "parameterBinding": binding(), "parameterPurpose": "controller" }),
    );
    assert_eq!(
        parameter_tool(&controller),
        None,
        "the controller's own record is no tool"
    );

    let analysis = loop_with(
        &project_experiment(),
        &json!({ "parameterBinding": binding(), "parameterPurpose": "experiment",
            "analysisParameterSymbols": { "Kp": "P1.Kp" } }),
    );
    assert_eq!(
        parameter_tool(&analysis),
        Some(ParameterTool::Analysis),
        "a loop that maps an analysis has the analysis's tool, and no other"
    );
    let without_binding = loop_with(
        &project_experiment(),
        &json!({ "parameterPurpose": "experiment" }),
    );
    assert_eq!(
        parameter_tool(&without_binding),
        None,
        "a tool needs the binding it writes through"
    );
}

/// A binding of two record fields, so that a tool that lists none can be told from one that lists both.
fn two_parameter_binding() -> Value {
    json!({
        "loop_id": "pid", "task_name": "MainTask",
        "parameters": [
            { "symbol": "P1.Kp", "iec_type": "LREAL", "unit": "1", "convention": "gain", "minimum": 0.0, "maximum": 10.0 },
            { "symbol": "P1.Hold", "iec_type": "LREAL", "unit": "s", "convention": "hold", "minimum": 1.0, "maximum": 60.0 }
        ],
        "mode": { "symbol": "Main.Run", "manual_value": true }
    })
}

// Specification 35, B6.1 and C12: the page fills the parameter tool from the form and keeps no copy of the rule. The experiment's
// settings are the loop's whole binding, each parameter with its own bounds. A declared analysis's tool starts with no parameter,
// because the engine's answers name the ones the analysis may write; a loop without a tool has no binding.
#[test]
fn the_form_names_the_parameters_the_tool_starts_from() {
    let none = project_test_form(&loop_with(&project_experiment(), &json!({}))).expect("a form");
    assert_eq!(none.parameter_binding, None, "no tool, no binding");

    let experiment = project_test_form(&loop_with(
        &project_experiment(),
        &json!({ "parameterBinding": two_parameter_binding(), "parameterPurpose": "experiment" }),
    ))
    .expect("a form");
    let settings = experiment
        .parameter_binding
        .expect("the experiment's settings are the loop's binding");
    assert_eq!(
        settings
            .parameters
            .iter()
            .map(|parameter| (
                parameter.symbol.as_str(),
                parameter.minimum,
                parameter.maximum
            ))
            .collect::<Vec<_>>(),
        [("P1.Kp", 0.0, 10.0), ("P1.Hold", 1.0, 60.0)],
        "each setting with its own bounds, in the binding's order"
    );
    assert_eq!(settings.mode.symbol, "Main.Run");

    let unnamed = project_test_form(&loop_with(
        &project_experiment(),
        &json!({ "parameterBinding": two_parameter_binding() }),
    ))
    .expect("a form");
    assert_eq!(
        unnamed.parameter_binding, None,
        "a loop named no tool starts from no binding"
    );

    let analysis = project_test_form(&loop_with(
        &project_experiment(),
        &json!({ "parameterBinding": two_parameter_binding(), "parameterPurpose": "experiment",
            "analysisParameterSymbols": { "Kp": "P1.Kp" } }),
    ))
    .expect("a form");
    let listed = analysis
        .parameter_binding
        .expect("a declared analysis's tool has a binding");
    assert_eq!(
        listed.parameters,
        [] as [Parameter; 0],
        "it lists no parameter until an answer names them"
    );
    assert_eq!(listed.task_name, "MainTask");
    assert_eq!(listed.loop_id, "pid");
    assert_eq!(listed.mode.symbol, "Main.Run", "the mode stays the loop's");
}

// Specification 35, C12: the experiment settings' ownership is released only after the terminal restoration. The answer says so in
// words, and only for a campaign whose restoration is kept.
#[test]
fn a_finished_test_releases_the_settings_only_with_a_confirmed_restoration() {
    for restoration in [
        CampaignRestoration::Restored,
        CampaignRestoration::NotRequired,
    ] {
        assert_eq!(
            settings_released(Some(restoration)).as_deref(),
            Some(SETTINGS_RELEASED),
            "{restoration:?}"
        );
    }
    assert_eq!(
        settings_released(None),
        None,
        "a campaign that kept no restoration releases nothing"
    );
}

#[test]
fn the_form_serializes_in_camel_case_and_omits_what_is_absent() {
    let value = serde_json::to_value(
        project_test_form(&loop_with(&project_experiment(), &json!({}))).expect("a form"),
    )
    .expect("the form serializes");
    assert_eq!(value["durationSeconds"], json!(30.0));
    assert!(value.get("baselineSeconds").is_none());
    assert!(value.get("parameterTool").is_none());
    assert!(value.get("parameterBinding").is_none());
    assert_eq!(value["signals"][0]["id"], json!("sp"));
    assert_eq!(value["signals"][0]["selected"], json!(true));
}
