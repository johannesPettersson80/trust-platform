//! Tests of the measured-disturbance feedforward's generator (specification 35, C8; `feedforwardGenerator.ts`) on a compiled heater program
//! with the variables of lesson 84: the seed, the states, the channels and the parameters the generator writes, the refusals of its draft
//! (D-5 among them), and the round trip through the loop file, which the Rust engine serves.

#![allow(clippy::float_cmp)] // The generated gains and bounds are compared with their exact values.

use serde_json::{json, Value};

use super::{feedforward_loop_definition, FeedforwardLoopDraft, FeedforwardVariables};
use crate::failure::Failure;
use crate::loop_definition::{
    Action, InitialSource, InitialValue, LoopDefinition, LoopScope, ModeWhenTrue,
};
use crate::setup::catalog::{decode_catalog_answer, CompiledCatalog};
use crate::setup::loop_bindings::MeasuredSignal;
use crate::setup::pid_loop::{ModeChoice, SignalChoice};

fn scope() -> LoopScope {
    LoopScope {
        project_id: "project:setup".to_string(),
        runtime_id: "runtime-1".to_string(),
    }
}

/// The sentence a refusal shows: its whole text, which is the detail when it says more than its first sentence.
fn text_of(failure: &Failure) -> String {
    failure
        .detail
        .clone()
        .unwrap_or_else(|| failure.message.clone())
}

fn signal(symbol: &str, unit: &str, minimum: f64, maximum: f64) -> MeasuredSignal {
    MeasuredSignal {
        symbol: symbol.to_string(),
        unit: unit.to_string(),
        minimum,
        maximum,
    }
}

#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literal its caller writes in place.
fn decode(control: Value) -> CompiledCatalog {
    decode_catalog_answer(&json!({ "ok": true, "processImage": {}, "controlSymbols": control }))
        .expect("the compiled answer decodes")
        .control
}

/// The heater of lesson 84: the water through the heater command, with the draw-off measured at `%ML8` and written by the test, and the
/// feedforward variables the program declares. The program writes the heater command, the filtered draw-off and its startup flag.
fn heater() -> CompiledCatalog {
    decode(json!({
        "symbols": [
            { "symbol": "P1.WaterC", "type": "LREAL", "address": "%IL0" },
            { "symbol": "P1.HeaterPct", "type": "LREAL", "address": "%QL0" },
            { "symbol": "P1.SetpointC", "type": "LREAL", "address": "%ML0" },
            { "symbol": "P1.ManualPct", "type": "LREAL", "address": "%ML16" },
            { "symbol": "P1.LoadKw", "type": "LREAL", "address": "%ML8" },
            { "symbol": "P1.AutoMode", "type": "BOOL" },
            { "symbol": "P1.Initialized", "type": "BOOL" },
            { "symbol": "P1.FeedforwardGain", "type": "LREAL" },
            { "symbol": "P1.LeadS", "type": "LREAL" },
            { "symbol": "P1.LagS", "type": "LREAL" },
            { "symbol": "P1.FilteredLoadKw", "type": "LREAL" }
        ],
        "controllers": [
            { "instance": "P1.Controller", "type": "Control.PID", "tasks": [{ "name": "MainTask", "intervalMs": 100 }] }
        ],
        "links": [],
        "writes": ["P1.HeaterPct", "P1.FilteredLoadKw", "P1.Initialized"]
    }))
}

/// The draft of lesson 84's feedforward: the PI of the heater, its draw-off as the disturbance, and the lead, lag and filtered draw-off.
fn draft() -> FeedforwardLoopDraft {
    FeedforwardLoopDraft {
        name: "Line heater".to_string(),
        instance: "P1.Controller".to_string(),
        action: Action::Direct,
        parameter_record: "P1.Parameters".to_string(),
        process_variable: signal("P1.WaterC", "degC", 0.0, 80.0),
        manipulated_variable: signal("P1.HeaterPct", "%", 0.0, 100.0),
        setpoint: SignalChoice {
            symbol: "P1.SetpointC".to_string(),
            unit: None,
            minimum: None,
            maximum: None,
        },
        mode: ModeChoice {
            symbol: "P1.AutoMode".to_string(),
            when_true: ModeWhenTrue::Auto,
        },
        manual_output: "P1.ManualPct".to_string(),
        disturbance: signal("P1.LoadKw", "kW", 0.0, 50.0),
        disturbance_command: "P1.LoadKw".to_string(),
        disturbance_step: 10.0,
        feedforward: FeedforwardVariables {
            gain: "P1.FeedforwardGain".to_string(),
            lead: Some("P1.LeadS".to_string()),
            lag: Some("P1.LagS".to_string()),
            filtered: Some("P1.FilteredLoadKw".to_string()),
        },
        experiment_step: Some(10.0),
        experiment_duration_seconds: Some(120.0),
        startup_flag: Some("P1.Initialized".to_string()),
    }
}

#[test]
fn the_draft_generates_a_feedforward_project_whose_seed_and_states_follow_lesson_84() {
    let loop_definition = feedforward_loop_definition(&heater(), &scope(), &draft())
        .expect("the feedforward is generated");
    let project = loop_definition
        .controller
        .project
        .as_ref()
        .expect("a feedforward declares its project block");
    assert_eq!(project.family, "feedforward");
    let seed = project.feedforward().expect("a feedforward seed");
    assert_eq!(seed.gain_symbol, "P1.FeedforwardGain");
    let lead_lag = seed.lead_lag.as_ref().expect("the lead and lag are named");
    assert_eq!(lead_lag.lead_symbol, "P1.LeadS");
    assert_eq!(lead_lag.lag_symbol, "P1.LagS");
    let states = project.initialization.states();
    let feed_forward = states
        .iter()
        .find(|state| state.symbol == "P1.Controller.StateValue.FeedForward")
        .expect("the feedforward state");
    assert_eq!(
        feed_forward.initial,
        InitialValue::From(InitialSource::ScaledModelInput {
            index: 1,
            gain_symbol: "P1.FeedforwardGain".to_string(),
        })
    );
    let filtered = states
        .iter()
        .find(|state| state.symbol == "P1.FilteredLoadKw")
        .expect("the filtered disturbance state");
    assert_eq!(
        filtered.initial,
        InitialValue::From(InitialSource::ModelInput { index: 1 })
    );
    assert!(project.is_application_state("P1.Initialized"));
}

#[test]
fn the_draft_writes_the_disturbance_as_a_channel_with_its_command_and_the_gains_as_parameters() {
    let loop_definition = feedforward_loop_definition(&heater(), &scope(), &draft())
        .expect("the feedforward is generated");
    let channels = loop_definition.channels.as_ref().expect("two model inputs");
    assert_eq!(channels.inputs.len(), 2);
    assert_eq!(channels.outputs.len(), 1);
    let disturbance = &channels.inputs[1];
    assert_eq!(disturbance.signal_id, "measured-disturbance");
    assert_eq!(
        disturbance.manual_output_signal.as_deref(),
        Some("P1.LoadKw")
    );
    assert_eq!(disturbance.experiment_step, Some(10.0));
    let binding = loop_definition
        .parameter_binding
        .as_ref()
        .expect("a parameter binding");
    let gain = binding
        .parameters
        .iter()
        .find(|parameter| parameter.symbol == "P1.FeedforwardGain")
        .expect("the feedforward gain is a parameter");
    assert_eq!(gain.unit, "%/kW");
    assert_eq!((gain.minimum, gain.maximum), (-1e6, 1e6));
    let lag = binding
        .parameters
        .iter()
        .find(|parameter| parameter.symbol == "P1.LagS")
        .expect("the lag is a parameter");
    assert_eq!(lag.unit, "s");
    assert_eq!((lag.minimum, lag.maximum), (0.001, 1e4));
}

#[test]
fn the_generated_feedforward_round_trips_through_the_loop_file_and_is_served_by_the_engine() {
    let loop_definition = feedforward_loop_definition(&heater(), &scope(), &draft())
        .expect("the feedforward is generated");
    let text = serde_json::to_string(&loop_definition).expect("the loop serializes");
    let decoded = LoopDefinition::decode(&text).expect("the loop file decodes");
    decoded
        .rust_engine_support()
        .expect("the Rust engine serves the feedforward");
    decoded.validate(&scope()).expect("the loop file validates");
}

#[test]
fn a_disturbance_command_the_program_writes_or_that_has_no_memory_address_is_refused() {
    // D-5: a command must be a simulation or test input, which the program only reads at a memory address.
    for command in ["P1.HeaterPct", "P1.WaterC"] {
        let mut refused = draft();
        refused.disturbance_command = command.to_string();
        let refusal = feedforward_loop_definition(&heater(), &scope(), &refused)
            .expect_err("the command is not a simulation or test input");
        assert!(
            text_of(&refusal).contains("simulation or test input"),
            "{command}: {}",
            text_of(&refusal)
        );
    }
}

#[test]
fn a_lead_without_its_lag_and_filtered_disturbance_is_refused_and_so_is_a_step_beyond_the_span() {
    let mut partial = draft();
    partial.feedforward.filtered = None;
    let refusal = feedforward_loop_definition(&heater(), &scope(), &partial)
        .expect_err("the lead, lag and filtered disturbance are all or none");
    assert_eq!(
        refusal.message,
        "Choose the lead time, the lag time and the filtered disturbance together, or none of them."
    );

    let mut too_large = draft();
    too_large.disturbance_step = 60.0;
    let refusal = feedforward_loop_definition(&heater(), &scope(), &too_large)
        .expect_err("the step is larger than the span");
    assert_eq!(
        refusal.message,
        "The disturbance step must be positive and at most 50 kW."
    );
}

#[test]
fn without_a_startup_flag_the_feedforward_has_no_application_state() {
    let mut unflagged = draft();
    unflagged.startup_flag = None;
    let loop_definition = feedforward_loop_definition(&heater(), &scope(), &unflagged)
        .expect("the feedforward is generated");
    let project = loop_definition
        .controller
        .project
        .as_ref()
        .expect("a project block");
    assert!(!project.is_application_state("P1.Initialized"));
}
