//! Tests of the identify-only generator (`identifyLoopGenerator.ts`): the generator assertions of `control-studio-loop-setup-save.test.ts`
//! ("writes an identify-only loop that records the chosen plant input and output"), the identify part of `control-studio-loop-setup.test.ts`
//! ("sets up a loop without an identification step or duration"), and the heater of `control-studio-domain-neutral.test.ts`.

#![allow(clippy::float_cmp)]

use serde_json::{json, Value};

use super::{identify_loop_definition, IdentifyLoopDraft};
use crate::failure::Failure;
use crate::loop_definition::{
    Execution, Experiment, ExperimentKind, LoopDefinition, LoopScope, Topology,
};
use crate::setup::catalog::{decode_catalog_answer, CompiledCatalog};
use crate::setup::loop_bindings::FormSignal;

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

#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literals its callers write in place.
fn decode(control: Value) -> CompiledCatalog {
    decode_catalog_answer(&json!({ "ok": true, "processImage": {}, "controlSymbols": control }))
        .expect("the compiled answer decodes")
        .control
}

/// The program of the save test (`catalog` in `control-studio-loop-setup-save.test.ts`), reduced to the plant's input and output, and a
/// computed variable for the refusal.
fn plant_catalog() -> CompiledCatalog {
    decode(json!({
        "symbols": [
            { "symbol": "P1.JacketC", "type": "LREAL", "address": "%IL8" },
            { "symbol": "P1.HeaterPct", "type": "LREAL", "address": "%QL0" },
            { "symbol": "P1.Filtered", "type": "LREAL" }
        ],
        "controllers": [
            { "instance": "P1", "type": "Main", "tasks": [{ "name": "MainTask", "intervalMs": 100 }] }
        ],
        "writes": ["P1.Filtered"]
    }))
}

fn jacket_draft() -> IdentifyLoopDraft {
    IdentifyLoopDraft {
        name: "Jacket response".to_string(),
        manipulated_variable: signal("P1.HeaterPct", "%", 0.0, 100.0),
        process_variable: signal("P1.JacketC", "degC", 0.0, 90.0),
        experiment_duration_seconds: Some(120.0),
    }
}

#[test]
fn an_identify_loop_is_open_loop_and_records_the_input_and_output() {
    // control-studio-loop-setup-save.test.ts: "writes an identify-only loop that records the chosen plant input and output".
    let loop_definition = identify_loop_definition(&plant_catalog(), &scope(), &jacket_draft())
        .expect("the identify-only loop is generated");
    loop_definition
        .validate(&scope())
        .expect("the loop is accepted");
    assert_eq!(loop_definition.topology, Some(Topology::OpenLoop));
    assert_eq!(loop_definition.controller.instance, "P1");
    assert_eq!(loop_definition.controller.type_name, "Main");
    assert!(
        loop_definition.signals.setpoint.is_none(),
        "an open loop carries no setpoint (C12)"
    );
    assert_eq!(
        (
            loop_definition.signals.process_variable.address.as_deref(),
            loop_definition
                .signals
                .manipulated_variable
                .address
                .as_deref(),
        ),
        (Some("%IL8"), Some("%QL0"))
    );
    assert_eq!(loop_definition.id, "loop:jacket-response");
    assert_eq!(
        loop_definition.experiment,
        Some(Experiment {
            execution: Execution::ProjectScripted,
            kind: ExperimentKind::Observation,
            amplitude: 0.0,
            relay_direction: None,
            duration_seconds: Some(120.0),
            baseline_seconds: None,
            hold_parameter: None,
            seed: 1,
            activation: None,
            abort: None,
        }),
        "the project runs the experiment; Control Studio records it with zero external amplitude"
    );
    let text = serde_json::to_string(&loop_definition).expect("the loop serializes");
    LoopDefinition::decode(&text).expect("the loop file is read unchanged");
}

#[test]
fn the_identify_loop_needs_a_recording_duration_between_one_second_and_a_day() {
    // control-studio-loop-setup.test.ts: "sets up a loop without an identification step or duration, leaving them to Identify and Limits
    // (B1.3)": the identify-only structure asks how long to record (C12), and refuses a missing duration.
    let draft = IdentifyLoopDraft {
        name: "Plant".to_string(),
        experiment_duration_seconds: None,
        ..jacket_draft()
    };
    let failure = identify_loop_definition(&plant_catalog(), &scope(), &draft)
        .expect_err("a missing duration is refused");
    assert!(
        text_of(&failure).contains("identification duration"),
        "{}",
        text_of(&failure)
    );
    let identified = identify_loop_definition(
        &plant_catalog(),
        &scope(),
        &IdentifyLoopDraft {
            name: "Plant".to_string(),
            experiment_duration_seconds: Some(120.0),
            ..jacket_draft()
        },
    )
    .expect("a duration inside the limits is accepted");
    assert_eq!(
        identified
            .experiment
            .as_ref()
            .and_then(|experiment| experiment.duration_seconds),
        Some(120.0)
    );
    let too_short = identify_loop_definition(
        &plant_catalog(),
        &scope(),
        &IdentifyLoopDraft {
            experiment_duration_seconds: Some(0.5),
            ..jacket_draft()
        },
    )
    .expect_err("half a second is below 1 s");
    assert_eq!(
        text_of(&too_short),
        "Jacket response needs an identification duration between 1 s and 24 h."
    );
}

#[test]
fn the_identify_loop_belongs_to_the_program_that_owns_its_input() {
    // identifyLoopGenerator.ts: the owner is the first segment of the manipulated variable's path.
    let draft = IdentifyLoopDraft {
        manipulated_variable: signal("P9.HeaterPct", "%", 0.0, 100.0),
        ..jacket_draft()
    };
    let failure = identify_loop_definition(&plant_catalog(), &scope(), &draft)
        .expect_err("P9 is not a compiled program");
    assert_eq!(
        text_of(&failure),
        "P9.HeaterPct does not belong to a compiled program instance."
    );
}

#[test]
fn a_heater_in_watts_and_degrees_is_an_accepted_open_loop() {
    // control-studio-domain-neutral.test.ts: the heater helper (W and °C, 60 s) is set up as an identify-only loop with no physical model.
    let catalog = decode(json!({
        "symbols": [
            { "symbol": "P1.HeaterW", "type": "LREAL", "address": "%QL0" },
            { "symbol": "P1.TemperatureC", "type": "LREAL", "address": "%IL0" }
        ],
        "controllers": [
            { "instance": "P1", "type": "Main", "tasks": [{ "name": "MainTask", "intervalMs": 100 }] }
        ],
        "links": []
    }));
    let draft = IdentifyLoopDraft {
        name: "Heater".to_string(),
        manipulated_variable: signal("P1.HeaterW", "W", 0.0, 2000.0),
        process_variable: signal("P1.TemperatureC", "degC", 0.0, 100.0),
        experiment_duration_seconds: Some(60.0),
    };
    let loop_definition =
        identify_loop_definition(&catalog, &scope(), &draft).expect("the heater loop is generated");
    loop_definition
        .validate(&scope())
        .expect("the heater loop is accepted");
    assert_eq!(loop_definition.signals.manipulated_variable.unit, "W");
    assert_eq!(loop_definition.signals.process_variable.unit, "degC");
    assert!(
        loop_definition.controller.law.is_none(),
        "no controller law is designed"
    );
}

#[test]
fn a_measurement_the_program_computes_is_refused_for_identification() {
    // identifyLoopGenerator.ts: the measured output goes through the same rule as a loop's measurement.
    let draft = IdentifyLoopDraft {
        process_variable: signal("P1.Filtered", "degC", 0.0, 90.0),
        ..jacket_draft()
    };
    let failure = identify_loop_definition(&plant_catalog(), &scope(), &draft)
        .expect_err("a computed measurement is refused");
    assert!(
        text_of(&failure).starts_with("P1.Filtered is computed by the program"),
        "{}",
        text_of(&failure)
    );
}

#[test]
fn the_program_that_owns_the_input_runs_in_exactly_one_task() {
    let catalog = decode(json!({
        "symbols": [
            { "symbol": "P1.JacketC", "type": "LREAL", "address": "%IL8" },
            { "symbol": "P1.HeaterPct", "type": "LREAL", "address": "%QL0" }
        ],
        "controllers": [
            { "instance": "P1", "type": "Main", "tasks": [
                { "name": "MainTask", "intervalMs": 100 }, { "name": "Fast", "intervalMs": 10 }] }
        ]
    }));
    let failure = identify_loop_definition(&catalog, &scope(), &jacket_draft())
        .expect_err("two tasks are refused");
    assert_eq!(text_of(&failure), "P1 must run in exactly one cyclic task.");
}

#[test]
fn the_form_sends_the_recording_duration_as_typed_text_or_a_number() {
    // The form posts the duration as typed: unit words such as "15 min" or "1 h 30 min", a plain number of seconds, or null when blank (C12).
    let draft = |duration: Value| -> IdentifyLoopDraft {
        serde_json::from_value(json!({
            "name": "Jacket response",
            "manipulatedVariable": { "symbol": "P1.HeaterPct", "unit": "%", "minimum": 0.0, "maximum": 100.0 },
            "processVariable": { "symbol": "P1.JacketC", "unit": "degC", "minimum": 0.0, "maximum": 90.0 },
            "experimentDurationSeconds": duration
        }))
        .expect("the draft decodes")
    };
    assert_eq!(
        draft(json!("15 min")).experiment_duration_seconds,
        Some(900.0)
    );
    assert_eq!(
        draft(json!("1 h 30 min")).experiment_duration_seconds,
        Some(5_400.0)
    );
    assert_eq!(draft(json!("900")).experiment_duration_seconds, Some(900.0));
    assert_eq!(draft(json!(90)).experiment_duration_seconds, Some(90.0));
    assert_eq!(draft(Value::Null).experiment_duration_seconds, None);
    assert_eq!(draft(json!("soon")).experiment_duration_seconds, None);
}

#[test]
fn a_duration_the_form_sends_as_text_is_refused_under_its_field_when_it_is_not_a_duration() {
    let mut unreadable = jacket_draft();
    unreadable.experiment_duration_seconds = None;
    let refusal = identify_loop_definition(&plant_catalog(), &scope(), &unreadable)
        .expect_err("a missing duration is refused");
    assert_eq!(refusal.field.as_deref(), Some("identify.durationSeconds"));
}
