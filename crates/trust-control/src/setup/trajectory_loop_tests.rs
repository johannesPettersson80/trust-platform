//! The setup of a trajectory-following law (specification 35, C11): the loop that the draft writes, its trajectory seed, its feedforward
//! state and planned trajectory, its two feedforward gains, and the refusals of its law.

use serde_json::{json, Value};

use super::*;
use crate::failure::Failure;
use crate::loop_definition::{Action, LoopScope, ModeWhenTrue};
use crate::setup::catalog::{decode_catalog_answer, CompiledCatalog};
use crate::setup::own_law_loop::DerivativeOn;
use crate::setup::pid_loop::{ModeChoice, SignalChoice};
use crate::setup::save::{parse_setup_draft, plan_setup_save, SetupDraft};

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

fn signal(symbol: &str, unit: &str, minimum: f64, maximum: f64) -> FormSignal {
    FormSignal {
        symbol: symbol.to_string(),
        unit: unit.to_string(),
        minimum: Some(minimum),
        maximum: Some(maximum),
    }
}

fn gain(symbol: &str, minimum: f64, maximum: f64) -> OwnLawGain {
    OwnLawGain {
        symbol: symbol.to_string(),
        minimum,
        maximum,
    }
}

#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literal its callers write in place.
fn decode(control: Value) -> CompiledCatalog {
    decode_catalog_answer(&json!({ "ok": true, "processImage": {}, "controlSymbols": control }))
        .expect("the compiled answer decodes")
        .control
}

/// The drum axis of lesson 90 as the program declares its symbols: a PID with a rate and an acceleration feedforward.
fn drum_axis() -> CompiledCatalog {
    decode(json!({
        "symbols": [
            { "symbol": "P1.AngleRad", "type": "LREAL", "address": "%IL0" },
            { "symbol": "P1.TorqueNm", "type": "LREAL", "address": "%QL0" },
            { "symbol": "P1.SetpointRad", "type": "LREAL", "address": "%ML0" },
            { "symbol": "P1.ManualTorqueNm", "type": "LREAL", "address": "%ML8" },
            { "symbol": "P1.ReferenceRad", "type": "LREAL" },
            { "symbol": "P1.FeedForward", "type": "LREAL" },
            { "symbol": "P1.AutoMode", "type": "BOOL" },
            { "symbol": "P1.Initialized", "type": "BOOL" },
            { "symbol": "P1.Parameters.Kp", "type": "LREAL" },
            { "symbol": "P1.Parameters.Ki", "type": "LREAL" },
            { "symbol": "P1.Parameters.Kd", "type": "LREAL" },
            { "symbol": "P1.Integral", "type": "LREAL" },
            { "symbol": "P1.PreviousErrorRad", "type": "LREAL" },
            { "symbol": "P1.RateGain", "type": "LREAL" },
            { "symbol": "P1.AccelerationGain", "type": "LREAL" }
        ],
        "controllers": [
            { "instance": "P1", "type": "Main", "tasks": [{ "name": "MainTask", "intervalMs": 20 }] }
        ],
        "links": [],
        "writes": ["P1.TorqueNm", "P1.Integral", "P1.PreviousErrorRad", "P1.FeedForward", "P1.Initialized", "P1.ReferenceRad"]
    }))
}

/// The draft of the drum axis: the PID its program computes, the two feedforward gains and the planned trajectory it follows.
fn draft() -> TrajectoryLoopDraft {
    TrajectoryLoopDraft {
        own_law: OwnLawLoopDraft {
            name: "Drum axis".to_string(),
            action: Action::Direct,
            law: OwnLawFamily::Pid,
            instance: "P1".to_string(),
            process_variable: signal("P1.AngleRad", "rad", -2.0, 2.0),
            manipulated_variable: signal("P1.TorqueNm", "N·m", -20.0, 20.0),
            setpoint: SignalChoice {
                symbol: "P1.SetpointRad".to_string(),
                unit: None,
                minimum: None,
                maximum: None,
            },
            mode: ModeChoice {
                symbol: "P1.AutoMode".to_string(),
                when_true: ModeWhenTrue::Auto,
            },
            manual_output: "P1.ManualTorqueNm".to_string(),
            proportional_gain: gain("P1.Parameters.Kp", 0.0, 100.0),
            integral_gain: Some(gain("P1.Parameters.Ki", 0.0, 100.0)),
            integral_term: Some("P1.Integral".to_string()),
            derivative_gain: Some(gain("P1.Parameters.Kd", 0.0, 100.0)),
            derivative_state: Some("P1.PreviousErrorRad".to_string()),
            derivative_on: Some(DerivativeOn::Error),
            experiment_step: None,
            experiment_duration_seconds: None,
            startup_flag: Some("P1.Initialized".to_string()),
        },
        rate_gain: gain("P1.RateGain", 0.0, 10.0),
        acceleration_gain: gain("P1.AccelerationGain", 0.0, 10.0),
        planned_trajectory: signal("P1.ReferenceRad", "rad", -2.0, 2.0),
        feedforward_term: "P1.FeedForward".to_string(),
    }
}

#[test]
fn a_trajectory_loop_names_its_five_variables_in_its_seed_and_its_gains_as_parameters() {
    let loop_definition = trajectory_loop_definition(&drum_axis(), &scope(), &draft())
        .expect("the drum axis is written");
    let project = loop_definition
        .controller
        .project
        .as_ref()
        .expect("the program's own law");
    let seed = project.trajectory().expect("the trajectory seed");
    assert_eq!(
        seed.pid.integral_symbol.as_deref(),
        Some("P1.Parameters.Ki")
    );
    assert_eq!(seed.rate_symbol, "P1.RateGain");
    assert_eq!(seed.acceleration_symbol, "P1.AccelerationGain");
    let parameters = &loop_definition
        .parameter_binding
        .as_ref()
        .expect("the binding")
        .parameters;
    let rate = parameters
        .iter()
        .find(|parameter| parameter.symbol == "P1.RateGain")
        .expect("the rate gain is a parameter");
    assert_eq!(rate.unit, "N·m*s/rad");
    assert_eq!(rate.convention, "Rate feedforward gain");
    assert!(parameters
        .iter()
        .any(|parameter| parameter.symbol == "P1.AccelerationGain"));
}

#[test]
fn a_trajectory_loop_is_served_by_the_engine_and_writes_the_seven_keys_of_its_seed() {
    let loop_definition = trajectory_loop_definition(&drum_axis(), &scope(), &draft())
        .expect("the drum axis is written");
    loop_definition
        .rust_engine_support()
        .expect("the engine serves the loop that Setup writes");
    let written = loop_definition
        .controller
        .project
        .as_ref()
        .expect("the program's own law")
        .to_value();
    assert_eq!(written["seed"]["kind"], json!("trajectory-feedforward-pid"));
    assert_eq!(
        written["seed"].as_object().map(serde_json::Map::len),
        Some(7)
    );
}

#[test]
fn the_feedforward_term_starts_at_zero_and_the_startup_flag_is_an_application_state() {
    let loop_definition = trajectory_loop_definition(&drum_axis(), &scope(), &draft())
        .expect("the drum axis is written");
    let written = loop_definition
        .controller
        .project
        .as_ref()
        .expect("the program's own law")
        .to_value();
    let states = written["initialization"]["states"]
        .as_array()
        .expect("the law's states");
    assert!(
        states
            .iter()
            .any(|state| state["symbol"] == json!("P1.FeedForward")
                && state["initial"] == json!(0.0)),
        "the feedforward term starts at rest, at zero"
    );
    assert_eq!(
        written["initialization"]["applicationStates"][0]["symbol"],
        json!("P1.Initialized"),
        "the startup-done flag is an application state (C1)"
    );
}

#[test]
fn the_planned_trajectory_is_a_state_signal_of_the_loop_that_the_setpoint_starts() {
    let loop_definition = trajectory_loop_definition(&drum_axis(), &scope(), &draft())
        .expect("the drum axis is written");
    let additional = loop_definition
        .signals
        .additional
        .as_ref()
        .expect("the planned trajectory is a further signal");
    assert_eq!(additional.len(), 1);
    assert_eq!(additional[0].id, "planned-trajectory");
    assert_eq!(additional[0].symbol, "P1.ReferenceRad");
    assert_eq!(
        loop_definition
            .signals
            .setpoint
            .as_ref()
            .map(|setpoint| setpoint.symbol.as_str()),
        Some("P1.SetpointRad"),
        "the setpoint is the loop's signal, not the seed's"
    );
}

#[test]
fn a_trajectory_law_that_is_not_a_pid_is_refused_by_its_structure() {
    let mut not_pid = draft();
    not_pid.own_law.law = OwnLawFamily::Pi;
    let failure = trajectory_loop_definition(&drum_axis(), &scope(), &not_pid)
        .expect_err("the feedforward is added to a PID");
    assert!(text_of(&failure).contains("PID"), "{}", text_of(&failure));
}

#[test]
fn the_setup_reads_the_structure_and_plans_one_loop_file() {
    let failure = parse_setup_draft(&json!({ "structure": "trajectory-feedforward" }))
        .expect_err("an empty draft is not complete");
    assert!(
        text_of(&failure).contains("trajectory-feedforward"),
        "the structure is recognized: {}",
        text_of(&failure)
    );
    let planned = plan_setup_save(
        &drum_axis(),
        &scope(),
        &SetupDraft::TrajectoryFeedforward(draft()),
    )
    .expect("the drum axis is planned");
    assert_eq!(planned.len(), 1);
    assert!(planned[0].file_name.starts_with("control-loops/"));
}
