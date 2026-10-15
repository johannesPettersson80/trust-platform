//! Tests of the cascade's generator (specification 35, C6; `cascadeGenerator.ts`) on the two-PID vessel program of lesson 52: the seed, the
//! block, the channels, the states and the parameters the generator writes, the refusal of a draft whose controllers are the same, and the
//! round trip through the loop file, which the Rust engine reads.

use serde_json::{json, Value};

use super::*;
use crate::setup::catalog::decode_catalog_answer;

fn scope() -> LoopScope {
    LoopScope {
        project_id: "project:setup".to_string(),
        runtime_id: "runtime-1".to_string(),
    }
}

fn signal(symbol: &str, unit: &str, minimum: f64, maximum: f64) -> FormSignal {
    FormSignal {
        symbol: symbol.to_string(),
        unit: unit.to_string(),
        minimum: Some(minimum),
        maximum: Some(maximum),
    }
}

/// The eleven record fields and the seven retained states of one `Control.PID` with its record (`pidSymbols`).
fn controller_symbols(record: &str, instance: &str) -> Vec<Value> {
    let fields = [
        "ProportionalGain",
        "IntegralGain",
        "DerivativeGain",
        "ProportionalSetpointWeight",
        "DerivativeSetpointWeight",
        "Bias",
        "OutputMinimum",
        "OutputMaximum",
        "RateMinimum",
        "RateMaximum",
        "TrackingTime",
    ];
    let states = [
        "Setpoint",
        "SetpointRate",
        "ProcessValue",
        "ProcessValueRate",
        "FeedForward",
        "NominalOutput",
        "SaturatedOutput",
    ];
    fields
        .iter()
        .map(|field| json!({ "symbol": format!("{record}.{field}"), "type": "LREAL" }))
        .chain(states.iter().map(
            |field| json!({ "symbol": format!("{instance}.StateValue.{field}"), "type": "LREAL" }),
        ))
        .collect()
}

/// The vessel program of lesson 52: the outer PID holds the vessel at its setpoint and sets the jacket's setpoint, which the inner PID
/// holds with the heater.
fn vessel() -> CompiledCatalog {
    let mut symbols = vec![
        json!({ "symbol": "P1.VesselC", "type": "LREAL", "address": "%IL0" }),
        json!({ "symbol": "P1.JacketC", "type": "LREAL", "address": "%IL8" }),
        json!({ "symbol": "P1.VesselSetpointC", "type": "LREAL", "address": "%ML0" }),
        json!({ "symbol": "P1.JacketSetpointC", "type": "LREAL", "address": "%ML8" }),
        json!({ "symbol": "P1.HeaterPct", "type": "LREAL", "address": "%QL0" }),
        json!({ "symbol": "P1.OuterAuto", "type": "BOOL", "address": "%MX40.0" }),
        json!({ "symbol": "P1.InnerAuto", "type": "BOOL", "address": "%MX40.1" }),
        json!({ "symbol": "P1.OuterManual", "type": "LREAL", "address": "%ML16" }),
        json!({ "symbol": "P1.InnerManual", "type": "LREAL", "address": "%ML24" }),
        json!({ "symbol": "P1.Initialized", "type": "BOOL" }),
    ];
    symbols.extend(controller_symbols("P1.OuterParameters", "P1.Outer"));
    symbols.extend(controller_symbols("P1.InnerParameters", "P1.Inner"));
    let task = json!([{ "name": "MainTask", "intervalMs": 100 }]);
    let answer = json!({
        "ok": true,
        "processImage": {},
        "controlSymbols": {
            "symbols": symbols,
            "controllers": [
                { "instance": "P1", "type": "Main", "tasks": task },
                { "instance": "P1.Inner", "type": "Control.PID", "tasks": task },
                { "instance": "P1.Outer", "type": "Control.PID", "tasks": task }
            ],
            "links": [
                { "instance": "P1.Outer", "method": "Configure", "argument": "Parameters", "record": "P1.OuterParameters", "output": null, "fields": {} },
                { "instance": "P1.Inner", "method": "Configure", "argument": "Parameters", "record": "P1.InnerParameters", "output": null, "fields": {} },
                { "instance": "P1.Outer", "method": "Update", "argument": "Input", "record": "P1.OuterInput", "output": "P1.JacketSetpointC",
                  "fields": { "ManualOutput": ["P1.OuterManual"], "ProcessValue": ["P1.VesselC"], "Setpoint": ["P1.VesselSetpointC"] } },
                { "instance": "P1.Inner", "method": "Update", "argument": "Input", "record": "P1.InnerInput", "output": "P1.HeaterPct",
                  "fields": { "ManualOutput": ["P1.InnerManual"], "ProcessValue": ["P1.JacketC"], "Setpoint": ["P1.JacketSetpointC"] } }
            ],
            "writes": ["P1.HeaterPct", "P1.JacketSetpointC"]
        }
    });
    decode_catalog_answer(&answer)
        .expect("the compiled answer decodes")
        .control
}

/// The draft of lesson 52's cascade: the vessel's outer PI, the jacket's inner PI, and the heater as the plant input (C6).
fn draft() -> CascadeLoopDraft {
    CascadeLoopDraft {
        name: "Vessel temperature".to_string(),
        action: Action::Direct,
        outer: CascadeOuterDraft {
            instance: "P1.Outer".to_string(),
            parameter_record: "P1.OuterParameters".to_string(),
            mode: ModeChoice {
                symbol: "P1.OuterAuto".to_string(),
                when_true: ModeWhenTrue::Auto,
            },
            process_variable: signal("P1.VesselC", "degC", 0.0, 90.0),
            setpoint: SignalChoice {
                symbol: "P1.VesselSetpointC".to_string(),
                unit: None,
                minimum: None,
                maximum: None,
            },
        },
        inner: CascadeInnerDraft {
            instance: "P1.Inner".to_string(),
            parameter_record: "P1.InnerParameters".to_string(),
            mode: ModeChoice {
                symbol: "P1.InnerAuto".to_string(),
                when_true: ModeWhenTrue::Auto,
            },
            manual_output: "P1.InnerManual".to_string(),
            process_variable: signal("P1.JacketC", "degC", 0.0, 90.0),
            manipulated_variable: signal("P1.HeaterPct", "%", 0.0, 100.0),
        },
        experiment_step: Some(10.0),
        experiment_duration_seconds: Some(120.0),
        startup_flag: None,
    }
}

fn generated() -> LoopDefinition {
    cascade_loop_definition(&vessel(), &scope(), &draft()).expect("the cascade is generated")
}

/// The initial source of a state of the block, by its symbol.
fn start_of(loop_definition: &LoopDefinition, symbol: &str) -> Option<InitialSource> {
    let project = loop_definition.controller.project.as_ref()?;
    project
        .initialization
        .states()
        .iter()
        .find(|state| state.symbol == symbol)
        .and_then(|state| match &state.initial {
            InitialValue::From(source) => Some(source.clone()),
            InitialValue::Constant(_) => None,
        })
}

#[test]
fn the_draft_generates_a_cascade_block_whose_seed_names_both_controllers_pi_gains() {
    let loop_definition = generated();
    let project = loop_definition
        .controller
        .project
        .as_ref()
        .expect("a cascade declares its project block");
    assert_eq!(project.family, "cascade");
    let seed = project.cascade().expect("a cascade seed");
    assert_eq!(
        (seed.inner.instance.as_str(), seed.outer.instance.as_str()),
        ("P1.Inner", "P1.Outer")
    );
    assert_eq!(
        seed.variables(),
        vec![
            "P1.InnerParameters.ProportionalGain",
            "P1.InnerParameters.IntegralGain",
            "P1.InnerParameters.DerivativeGain",
            "P1.OuterParameters.ProportionalGain",
            "P1.OuterParameters.IntegralGain",
            "P1.OuterParameters.DerivativeGain",
        ]
    );
    assert_eq!(
        project.offered_families(),
        vec![crate::tuning::structure::Structure::PI]
    );
    assert_eq!(project.objective.requirement_id, "P1.VesselC:target-error");
    assert_eq!(
        project
            .additional_modes
            .iter()
            .map(|mode| (mode.symbol.as_str(), mode.manual_value))
            .collect::<Vec<_>>(),
        vec![("P1.OuterAuto", false)],
        "the outer controller's mode switches with the inner one"
    );
}

#[test]
fn the_channels_bind_the_plant_input_and_the_two_measurements_and_the_shape_is_the_cascade() {
    let loop_definition = generated();
    let channels = loop_definition
        .channels
        .as_ref()
        .expect("a cascade has channels");
    assert_eq!(channels.inputs.len(), 1);
    assert_eq!(channels.inputs[0].signal_id, "manipulatedVariable");
    assert_eq!(channels.outputs.len(), 2);
    assert_eq!(channels.outputs[0].signal_id, "inner-measurement");
    assert_eq!(channels.outputs[0].setpoint_signal_id, None);
    assert_eq!(channels.outputs[1].signal_id, "processVariable");
    assert_eq!(
        channels.outputs[1].setpoint_signal_id.as_deref(),
        Some("setpoint")
    );
    assert!(loop_definition.is_cascade_channels());
    assert_eq!(
        loop_definition
            .identification_policy
            .as_ref()
            .and_then(|policy| policy.get("maximumOrder")),
        Some(&json!(1)),
        "the cascade's model is first order in each measurement (N10)"
    );
}

#[test]
fn the_states_start_the_outer_output_at_the_inner_measurement_so_no_switch_bumps() {
    let loop_definition = generated();
    assert!(matches!(
        start_of(&loop_definition, "P1.Outer.StateValue.NominalOutput"),
        Some(InitialSource::ModelOutput { index: 0 })
    ));
    assert!(matches!(
        start_of(&loop_definition, "P1.Outer.StateValue.ProcessValue"),
        Some(InitialSource::ModelOutput { index: 1 })
    ));
    assert!(matches!(
        start_of(&loop_definition, "P1.Inner.StateValue.NominalOutput"),
        Some(InitialSource::ModelInput { index: 0 })
    ));
    assert!(matches!(
        start_of(&loop_definition, "P1.Inner.StateValue.Setpoint"),
        Some(InitialSource::ModelOutput { index: 0 })
    ));
}

#[test]
fn the_parameter_record_binds_the_six_gains_of_both_controllers_with_the_units_of_their_loops() {
    let loop_definition = generated();
    let binding = loop_definition
        .parameter_binding
        .as_ref()
        .expect("a parameter binding");
    let parameters: Vec<(&str, &str)> = binding
        .parameters
        .iter()
        .map(|parameter| (parameter.symbol.as_str(), parameter.unit.as_str()))
        .collect();
    assert_eq!(
        parameters,
        vec![
            ("P1.InnerParameters.ProportionalGain", "%/degC"),
            ("P1.InnerParameters.IntegralGain", "%/(degC*s)"),
            ("P1.InnerParameters.DerivativeGain", "%*s/degC"),
            ("P1.OuterParameters.ProportionalGain", "degC/degC"),
            ("P1.OuterParameters.IntegralGain", "degC/(degC*s)"),
            ("P1.OuterParameters.DerivativeGain", "degC*s/degC"),
        ]
    );
    assert_eq!(binding.mode.symbol, "P1.InnerAuto");
}

#[test]
fn the_written_loop_reads_back_as_the_same_loop() {
    let loop_definition = generated();
    let text = serde_json::to_string(&loop_definition).expect("the loop serializes");
    let read = LoopDefinition::decode(&text).expect("the written loop decodes");
    assert_eq!(
        read, loop_definition,
        "the loop file holds the loop the generator wrote"
    );
}

#[test]
fn a_program_that_writes_a_seeded_setpoint_needs_the_startup_flag() {
    // C1 Setup: validation seeds the outer setpoint, so a program that writes it needs the startup-done flag, as a PID loop does.
    let mut catalog = vessel();
    catalog.writes.push("P1.VesselSetpointC".to_string());
    let failure = cascade_loop_definition(&catalog, &scope(), &draft())
        .expect_err("the program writes a seeded value");
    assert_eq!(
        failure.message,
        "The program writes P1.VesselSetpointC in its own code."
    );
    assert_eq!(
        failure.detail.as_deref(),
        Some("The program writes P1.VesselSetpointC in its own code. Choose the flag that marks its startup as done, so validation keeps the values it starts from.")
    );
    let flagged = CascadeLoopDraft {
        startup_flag: Some("P1.Initialized".to_string()),
        ..draft()
    };
    cascade_loop_definition(&catalog, &scope(), &flagged)
        .expect("with the flag the cascade is generated");
}

#[test]
fn a_startup_flag_that_is_not_a_bool_is_refused_for_a_cascade() {
    // C1 Setup: the startup-done flag is a BOOL. A REAL variable of the vessel named as the flag is refused under the form's field.
    let mut flagged = draft();
    flagged.startup_flag = Some("P1.VesselC".to_string());
    let failure = cascade_loop_definition(&vessel(), &scope(), &flagged)
        .expect_err("a REAL variable is not a startup-done flag");
    assert_eq!(failure.message, "P1.VesselC is not a compiled BOOL symbol.");
    assert_eq!(failure.field.as_deref(), Some("startupFlag"));
}

#[test]
fn two_controllers_that_are_the_same_instance_are_refused() {
    let mut same = draft();
    same.outer.instance = "P1.Inner".to_string();
    let refusal = cascade_loop_definition(&vessel(), &scope(), &same)
        .expect_err("a cascade needs two controllers");
    assert_eq!(
        refusal
            .detail
            .clone()
            .unwrap_or_else(|| refusal.message.clone()),
        "Choose two different controllers."
    );
}
