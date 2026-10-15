//! Tests of the loop bindings (`loopBindings.ts`): the TypeScript assertions of `control-studio-symbolic-signals.test.ts` and of the
//! setup tests on the bindings, the boundaries of the duration limits (A7), and the revision rule of `withRevision`.

#![allow(clippy::float_cmp)]

use serde_json::json;
use sha2::{Digest, Sha256};

use super::{
    binding, cyclic_task, explicit_experiment, identification_duration, loop_id,
    manual_output_type, measured_signal, measurement_binding, state_type, stated_signal,
    stated_unit, with_revision, FormSignal, MeasuredSignal,
};
use crate::failure::{Cause, Failure};
use crate::loop_definition::{
    Direction, Execution, ExperimentKind, LoopDefinition, LoopScope, ManualOutputType, RealType,
    StateType,
};
use crate::setup::catalog::{
    decode_catalog_answer, CompiledCatalog, CompiledController, CompiledTask,
};
use crate::setup::identify_loop::{identify_loop_definition, IdentifyLoopDraft};
use crate::study::canonical_json;

/// The sentence a refusal shows: its whole text, which is the detail when it says more than its first sentence.
fn text_of(failure: &Failure) -> String {
    failure
        .detail
        .clone()
        .unwrap_or_else(|| failure.message.clone())
}

fn signal(symbol: &str) -> MeasuredSignal {
    MeasuredSignal {
        symbol: symbol.to_string(),
        unit: "degC".to_string(),
        minimum: 0.0,
        maximum: 100.0,
    }
}

/// The program of the symbolic-signal tests: `FilteredC` is computed by the program, `TemperatureC` is addressed and written, and
/// `OperatorC` is a variable the program only reads (`control-studio-symbolic-signals.test.ts`).
fn symbolic_catalog() -> CompiledCatalog {
    decode_catalog_answer(&json!({
        "ok": true,
        "controlSymbols": {
            "symbols": [
                { "symbol": "P1.TemperatureC", "type": "LREAL", "address": "%IL8" },
                { "symbol": "P1.FilteredC", "type": "LREAL" },
                { "symbol": "P1.OperatorC", "type": "LREAL" },
                { "symbol": "P1.AutoMode", "type": "BOOL" },
                { "symbol": "P1.Ki", "type": "REAL" }
            ],
            "controllers": [
                { "instance": "P1", "type": "Main", "tasks": [{ "name": "MainTask", "intervalMs": 100 }] }
            ],
            "writes": ["P1.FilteredC", "P1.TemperatureC"]
        }
    }))
    .expect("the symbolic catalog decodes")
    .control
}

/// An identify-only loop, the smallest valid generated loop (its revision is the one under test).
fn plant_loop(name: &str) -> LoopDefinition {
    let catalog = decode_catalog_answer(&json!({
        "ok": true,
        "controlSymbols": {
            "symbols": [
                { "symbol": "P1.HeaterPct", "type": "LREAL", "address": "%QL0" },
                { "symbol": "P1.JacketC", "type": "LREAL", "address": "%IL8" }
            ],
            "controllers": [
                { "instance": "P1", "type": "Main", "tasks": [{ "name": "MainTask", "intervalMs": 100 }] }
            ]
        }
    }))
    .expect("the catalog decodes")
    .control;
    identify_loop_definition(
        &catalog,
        &LoopScope {
            project_id: "project:bindings".to_string(),
            runtime_id: "runtime-1".to_string(),
        },
        &IdentifyLoopDraft {
            name: name.to_string(),
            manipulated_variable: FormSignal {
                symbol: "P1.HeaterPct".to_string(),
                unit: "%".to_string(),
                minimum: Some(0.0),
                maximum: Some(100.0),
            },
            process_variable: FormSignal {
                symbol: "P1.JacketC".to_string(),
                unit: "degC".to_string(),
                minimum: Some(0.0),
                maximum: Some(90.0),
            },
            experiment_duration_seconds: Some(120.0),
        },
    )
    .expect("the identify-only loop is generated")
}

#[test]
fn loop_ids_are_slugs_of_the_name() {
    // loopBindings.ts `loopId`; the save test writes "Oven" as loop:oven, and "Tank 2 level" as tank-2-level.
    assert_eq!(loop_id("Oven"), "loop:oven");
    assert_eq!(loop_id("Tank 2 level"), "loop:tank-2-level");
    assert_eq!(
        loop_id("Vessel temperature · inner loop"),
        "loop:vessel-temperature-inner-loop"
    );
    assert_eq!(loop_id("  Heater!! "), "loop:heater");
    assert_eq!(
        loop_id("!!!"),
        "loop:loop",
        "a name without a letter or digit is loop"
    );
    assert_eq!(loop_id(""), "loop:loop");
    assert_eq!(
        loop_id("ünïcode loop"),
        "loop:n-code-loop",
        "a letter outside a-z is a separator"
    );
}

#[test]
fn the_identification_duration_admits_one_second_to_a_day_and_refuses_outside() {
    // A7: 1 s to 24 h of process time. loopBindings.ts `identificationDuration`, and the setup test "sets up a loop without an
    // identification step or duration": a missing duration is refused with "identification duration".
    assert_eq!(
        identification_duration("Plant", Some(1.0)).expect("1 s is inside"),
        1.0
    );
    assert_eq!(
        identification_duration("Plant", Some(120.0)).expect("120 s is inside"),
        120.0
    );
    assert_eq!(
        identification_duration("Plant", Some(86_400.0)).expect("24 h is inside"),
        86_400.0
    );
    for outside in [
        Some(0.999),
        Some(86_400.001),
        Some(f64::NAN),
        Some(f64::INFINITY),
        None,
    ] {
        let failure = identification_duration("Plant", outside).expect_err("outside the limits");
        assert_eq!(
            text_of(&failure),
            "Plant needs an identification duration between 1 s and 24 h."
        );
        assert_eq!(failure.cause, Cause::Settings);
    }
}

#[test]
fn the_experiment_is_written_with_both_its_step_and_duration_or_not_at_all() {
    // loopBindings.ts `explicitExperiment`: B1.3, setup writes an experiment only when the engineer set both values.
    assert!(explicit_experiment(
        "Plant",
        Execution::ControlStudio,
        ExperimentKind::MvStep,
        None,
        None
    )
    .expect("neither is no experiment")
    .is_none());
    let experiment = explicit_experiment(
        "Plant",
        Execution::ControlStudio,
        ExperimentKind::MvStep,
        Some(20.0),
        Some(8.0),
    )
    .expect("both are an experiment")
    .expect("an experiment is written");
    assert_eq!(
        (
            experiment.execution,
            experiment.kind,
            experiment.amplitude,
            experiment.duration_seconds,
            experiment.seed
        ),
        (
            Execution::ControlStudio,
            ExperimentKind::MvStep,
            20.0,
            Some(8.0),
            1
        ),
        "the amplitude is the step, the duration the identification duration, and the seed 1"
    );
    for (step, seconds) in [(Some(20.0), None), (None, Some(8.0))] {
        let failure = explicit_experiment(
            "Plant",
            Execution::ControlStudio,
            ExperimentKind::MvStep,
            step,
            seconds,
        )
        .expect_err("only one of the two is set");
        assert_eq!(
            text_of(&failure),
            "Plant needs both the output step and the identification duration, or neither: Identify chooses the one that is left."
        );
    }
    let failure = explicit_experiment(
        "Plant",
        Execution::ControlStudio,
        ExperimentKind::MvStep,
        Some(20.0),
        Some(0.5),
    )
    .expect_err("a half-second duration is outside the limits");
    assert!(text_of(&failure).contains("identification duration"));
}

#[test]
fn a_measurement_the_program_computes_is_refused_unless_it_is_addressed() {
    // control-studio-symbolic-signals.test.ts: "refuses a measurement the program computes, which a simulated measurement could not
    // reach", and "accepts a measurement without an address that the program only reads".
    let catalog = symbolic_catalog();
    let failure = measurement_binding(&catalog, &signal("P1.FilteredC"))
        .expect_err("a computed variable is refused");
    let text = text_of(&failure);
    assert!(
        text.starts_with("P1.FilteredC is computed by the program")
            && text.contains("overwritten")
            && text.contains("measurement it is computed from"),
        "{text}"
    );
    assert_eq!(failure.cause, Cause::Settings);
    let read = measurement_binding(&catalog, &signal("P1.OperatorC"))
        .expect("a variable the program only reads is accepted");
    assert_eq!(read.address, None);
    assert_eq!(read.direction, Direction::Read);
    let addressed = measurement_binding(&catalog, &signal("P1.TemperatureC"))
        .expect("an addressed variable is never computed for this rule");
    assert_eq!(addressed.address.as_deref(), Some("%IL8"));
}

#[test]
fn a_signal_must_be_a_real_compiled_variable() {
    let catalog = symbolic_catalog();
    let real = binding(&catalog, &signal("P1.Ki"), Direction::Read).expect("REAL is a real type");
    assert_eq!(real.iec_type, "REAL");
    let failure = binding(&catalog, &signal("P1.AutoMode"), Direction::Write)
        .expect_err("a BOOL is not a real signal");
    assert_eq!(text_of(&failure), "P1.AutoMode must be REAL or LREAL.");
    let missing = binding(&catalog, &signal("P1.Missing"), Direction::Read)
        .expect_err("an uncompiled symbol is refused");
    assert_eq!(
        text_of(&missing),
        "P1.Missing is not a compiled symbol of this project."
    );
}

#[test]
fn the_revision_is_the_sha256_of_the_canonical_loop_without_its_revision() {
    // withRevision: `sha256:` and the hex SHA-256 of the loop without its revision. Rust hashes canonical JSON (sorted keys).
    let loop_definition = plant_loop("Plant");
    let mut value = serde_json::to_value(&loop_definition).expect("the loop serializes");
    value
        .as_object_mut()
        .expect("a loop is an object")
        .remove("revision");
    let digest = Sha256::digest(canonical_json(&value).as_bytes());
    assert_eq!(loop_definition.revision, format!("sha256:{digest:x}"));
    assert_eq!(loop_definition.revision.len(), "sha256:".len() + 64);
    let mut other = loop_definition.clone();
    other.revision = "sha256:0".to_string();
    assert_eq!(
        with_revision(other).revision,
        loop_definition.revision,
        "the revision is not part of its own digest"
    );
    let mut renamed = loop_definition.clone();
    renamed.name = "Other".to_string();
    assert_ne!(
        with_revision(renamed).revision,
        loop_definition.revision,
        "the content is part of the digest"
    );
}

#[test]
fn a_form_signal_without_a_bound_is_refused_under_the_field_that_holds_it() {
    let form = |minimum: Option<f64>, maximum: Option<f64>| FormSignal {
        symbol: "P1.Level".to_string(),
        unit: "m".to_string(),
        minimum,
        maximum,
    };
    let checked = measured_signal(
        &form(Some(0.0), Some(2.0)),
        "measurement",
        Some("processVariable"),
    )
    .expect("both bounds are numbers and the maximum is above the minimum");
    assert_eq!((checked.minimum, checked.maximum), (0.0, 2.0));
    assert_eq!(checked.unit, "m", "the unit is kept as the form sent it");
    for (minimum, maximum, bound) in [(None, Some(2.0), "minimum"), (Some(0.0), None, "maximum")] {
        let failure = measured_signal(
            &form(minimum, maximum),
            "measurement",
            Some("processVariable"),
        )
        .expect_err("a missing bound is refused");
        assert_eq!(
            failure.message,
            "Enter the measurement minimum and maximum."
        );
        let field = format!("processVariable.{bound}");
        assert_eq!(failure.field.as_deref(), Some(field.as_str()));
    }
    let order = measured_signal(&form(Some(3.0), Some(3.0)), "measurement", None)
        .expect_err("a maximum that is not above the minimum is refused");
    assert_eq!(
        order.message,
        "The measurement maximum must be above its minimum."
    );
    assert_eq!(
        order.field, None,
        "without a page field the sentence stands in the message area"
    );
}

#[test]
fn a_signal_with_no_symbol_or_unit_is_refused_under_its_field_before_its_range() {
    let form = |symbol: &str, unit: &str| FormSignal {
        symbol: symbol.to_string(),
        unit: unit.to_string(),
        minimum: Some(0.0),
        maximum: Some(1.0),
    };
    let field = Some("feedback.manipulatedVariable");
    let no_symbol = measured_signal(&form("", ""), "plant input", field)
        .expect_err("a signal with no symbol is refused first");
    assert_eq!(no_symbol.message, "Choose the plant input.");
    assert_eq!(
        no_symbol.field.as_deref(),
        Some("feedback.manipulatedVariable")
    );
    let no_unit = measured_signal(&form("P1.HeaterPct", "  "), "plant input", field)
        .expect_err("a signal with no unit is refused");
    assert_eq!(no_unit.message, "Enter the plant input unit.");
    assert_eq!(
        no_unit.field.as_deref(),
        Some("feedback.manipulatedVariable.unit")
    );
}

#[test]
fn a_blank_unit_is_dimensionless_and_a_stated_unit_is_kept() {
    // The loop reader writes a blank signal unit as `1` (checks.rs `state_dimensionless_units`); C0 needs a parameter unit.
    assert_eq!(stated_unit(""), "1");
    assert_eq!(stated_unit("   "), "1");
    assert_eq!(stated_unit("%"), "%");
    let blank = stated_signal(&MeasuredSignal {
        symbol: "P1.Ki".to_string(),
        unit: String::new(),
        minimum: 0.0,
        maximum: 1.0,
    });
    assert_eq!(blank.unit, "1");
    let catalog = symbolic_catalog();
    let dimensionless = binding(&catalog, &blank, Direction::Read).expect("a REAL with a unit");
    assert_eq!(dimensionless.unit, "1");
}

#[test]
fn the_manual_output_and_state_types_follow_the_real_type() {
    assert_eq!(manual_output_type(RealType::Real), ManualOutputType::Real);
    assert_eq!(manual_output_type(RealType::Lreal), ManualOutputType::Lreal);
    assert_eq!(state_type(RealType::Real), StateType::Real);
    assert_eq!(state_type(RealType::Lreal), StateType::Lreal);
}

#[test]
fn the_controller_runs_in_exactly_one_task_with_a_positive_interval() {
    // loopBindings.ts and the three generators: "must run in exactly one cyclic task".
    let controller = |tasks: Vec<CompiledTask>| CompiledController {
        instance: "P1".to_string(),
        type_name: "Main".to_string(),
        tasks,
    };
    let task = |interval_ms: f64| CompiledTask {
        name: "MainTask".to_string(),
        interval_ms,
    };
    assert_eq!(
        cyclic_task(&controller(vec![task(100.0)]))
            .expect("one task")
            .interval_ms,
        100.0
    );
    for refused in [
        controller(vec![]),
        controller(vec![task(100.0), task(10.0)]),
        controller(vec![task(0.0)]),
        controller(vec![task(f64::NAN)]),
    ] {
        let failure = cyclic_task(&refused).expect_err("not one positive cyclic task");
        assert_eq!(text_of(&failure), "P1 must run in exactly one cyclic task.");
    }
}
