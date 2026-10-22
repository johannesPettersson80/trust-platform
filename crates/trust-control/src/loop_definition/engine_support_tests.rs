//! The Rust engine's support of a loop (specification 35, B1 and C12; `rust_engine_support`): the parked features, the loop without a
//! controller law, the course's observation and open-loop lessons, the schema-one lessons, the reference examples and the loop with two
//! controller structures. They were part of the loop contract tests in `tests.rs`, which keeps the helpers they share.

use serde_json::{json, Value};

use super::tests::{complete_loop, decode, scope, with};
use super::*;

#[test]
fn the_rust_engine_refuses_a_parked_feature_and_a_pid_loop_that_lost_its_law() {
    // A top-level cascade is the two-loop form, which the Rust engine does not run yet (checklist V7). A PID group is served now.
    let cascaded = with(complete_loop(), &["cascade"], json!({}));
    let failure = decode(&cascaded)
        .expect("decoded")
        .rust_engine_support()
        .expect_err("refused");
    assert!(failure.message.contains("a cascade"), "{}", failure.message);
    // A loop the project does not design and that has no law is a loop setup to repair, not a C12 loop (specification 35, C12).
    let mut lawless = complete_loop();
    lawless["controller"]
        .as_object_mut()
        .expect("controller")
        .remove("law");
    let failure = decode(&lawless)
        .expect("decoded")
        .rust_engine_support()
        .expect_err("refused");
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Implementation, NextAction::OpenLoopSetup),
        "{failure:?}"
    );
    assert!(
        failure.message.contains("no controller law"),
        "{}",
        failure.message
    );
}

#[test]
fn the_rust_engine_runs_an_observation_loop_without_a_law_and_its_law_stages_refuse_it() {
    // C12: the project designs the controller and binds no law. The engine runs the loop; Tune, Validate and Apply refuse it.
    let mut observed = complete_loop();
    observed["controller"]
        .as_object_mut()
        .expect("controller")
        .remove("law");
    observed["controller"]["design"] = json!("project");
    observed["experiment"] = json!({
        "execution": "project-scripted", "kind": "observation", "amplitude": 0,
        "durationSeconds": 120, "seed": 1
    });
    let observed = decode(&observed).expect("decoded");
    observed
        .rust_engine_support()
        .expect("the engine runs a loop without a controller law (C12)");
    let refusal = observed
        .require_controller_law()
        .expect_err("nothing to tune");
    assert_eq!(refusal.message, NO_CONTROLLER_LAW, "{refusal:?}");
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::Implementation, NextAction::None),
        "{refusal:?}"
    );
    let with_law = decode(&complete_loop()).expect("decoded");
    assert!(
        with_law.require_controller_law().is_ok(),
        "the PID loop has its law"
    );
}

#[test]
fn the_engine_runs_an_observation_lesson_and_an_open_loop_lesson_of_the_examples() {
    // Specification 35, C12: heater lesson 04 is an observation loop (project-scripted, no law) and process-response lesson 13 is
    // an open-loop experiment loop (`topology: "open-loop"`, schema 2, no law). Both are the examples the course opens (checklist
    // V7, wave 1). The cold-storage lessons, which are schema 1, are tested by
    // `the_cold_storage_lessons_of_schema_one_open_as_observation_loops`.
    for text in [
        include_str!(
            "../../../../examples/control/01-feedback-foundations-and-temperature-control/04-water-heater-p/control-loop.json"
        ),
        include_str!(
            "../../../../examples/control/01-feedback-foundations-and-temperature-control/13-process-response-classes/control-loop.json"
        ),
    ] {
        let value: Value = serde_json::from_str(text).expect("json");
        let loop_definition =
            LoopDefinition::decode(text).unwrap_or_else(|error| panic!("{}: {error}", value["id"]));
        loop_definition
            .validate(&scope(&value))
            .unwrap_or_else(|error| panic!("{}: {error}", value["id"]));
        loop_definition
            .rust_engine_support()
            .unwrap_or_else(|error| panic!("{}: {error:?}", value["id"]));
        assert!(
            loop_definition.require_controller_law().is_err(),
            "{}: no controller law, so Tune, Validate and Apply refuse it",
            value["id"]
        );
    }
}

#[test]
fn the_cold_storage_lessons_of_schema_one_open_as_observation_loops() {
    // Specification 35, B1 and C12: lessons 01, 02, 03 and 80 are version 1 files, each a project-scripted observation with no law.
    // The engine reads them by the version 1 rules, so they open and Identify serves them; Tune, Validate and Apply do not apply.
    use crate::stages::{stage_availability, Stage, StageEvidence, StageStatus};

    for text in [
        include_str!(
            "../../../../examples/control/01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy/control-loop.json"
        ),
        include_str!(
            "../../../../examples/control/01-feedback-foundations-and-temperature-control/02-cold-storage-on-off/control-loop.json"
        ),
        include_str!(
            "../../../../examples/control/01-feedback-foundations-and-temperature-control/03-cold-storage-hysteresis-dwell/control-loop.json"
        ),
        include_str!(
            "../../../../examples/control/14-integrated-capstones/80-cold-storage/control-loop.json"
        ),
    ] {
        let value: Value = serde_json::from_str(text).expect("json");
        let id = &value["id"];
        assert_eq!(value["schemaVersion"], 1, "{id} is a version 1 file");
        let loop_definition =
            LoopDefinition::decode(text).unwrap_or_else(|error| panic!("{id}: {error}"));
        loop_definition
            .validate(&scope(&value))
            .unwrap_or_else(|error| panic!("{id}: {error}"));
        loop_definition
            .rust_engine_support()
            .unwrap_or_else(|error| panic!("{id}: {error:?}"));
        assert!(loop_definition.is_observation_or_open_loop(), "{id}");
        let stages = stage_availability(&StageEvidence {
            loop_bound: true,
            controller_stages_apply: !loop_definition.is_observation_or_open_loop(),
            ..StageEvidence::default()
        });
        let status = |wanted: Stage| {
            stages
                .iter()
                .find(|stage| stage.stage == wanted)
                .map(|stage| stage.status)
        };
        assert_eq!(
            status(Stage::Identify),
            Some(StageStatus::NeedsData),
            "{id}: Identify is served"
        );
        for stage in [Stage::Tune, Stage::Validate, Stage::Apply] {
            assert_eq!(
                status(stage),
                Some(StageStatus::NotApplicable),
                "{id}: {stage:?} does not apply"
            );
        }
        let refusal = loop_definition
            .require_controller_law()
            .expect_err("no controller law");
        assert_eq!(refusal.message, NO_CONTROLLER_LAW, "{id}");
    }
}

#[test]
fn the_reference_examples_loops_decode_validate_and_run_on_the_rust_engine() {
    for text in [
        include_str!("../../../../examples/room_heating/control-loops/tank-temperature.json"),
        include_str!("../../../../examples/control-pid-commissioning/control-loop.json"),
    ] {
        let value: Value = serde_json::from_str(text).expect("json");
        let loop_definition =
            LoopDefinition::decode(text).unwrap_or_else(|error| panic!("{}: {error}", value["id"]));
        loop_definition
            .validate(&scope(&value))
            .unwrap_or_else(|error| panic!("{}: {error}", value["id"]));
        loop_definition
            .rust_engine_support()
            .unwrap_or_else(|error| panic!("{}: {error}", value["id"]));
    }
}

/// A loop declares one controller structure: a controller law beside state feedback is refused by its sentence.
#[test]
fn a_loop_with_two_controller_structures_is_refused() {
    // The complete loop has a controller law; state feedback beside it is a second structure.
    let mut mixed = decode(&complete_loop()).expect("decoded");
    mixed.controller.state_feedback = Some(json!({}));
    let refusal = mixed
        .rust_engine_support()
        .expect_err("a law beside state feedback is refused");
    assert!(
        refusal
            .detail
            .as_deref()
            .unwrap_or(refusal.message.as_str())
            .contains("more than one controller structure"),
        "{refusal:?}"
    );
}
