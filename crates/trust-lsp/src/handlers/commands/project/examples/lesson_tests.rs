//! The lesson contract (specification 38; specification 25 §3.1). The real lessons come from the
//! repository's `examples/control`; the other cases change one field of a real lesson, so each test
//! fails the one check it names. The cases of the extension's former `control-course-navigation`
//! and `control-course-units` suites that tested the contract live here now.

use std::path::{Path, PathBuf};

use super::*;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The `lesson.json` of the lesson folder `relative` below `examples/control`.
fn real_lesson(relative: &str) -> Value {
    let path = repository()
        .join("examples/control")
        .join(relative)
        .join("lesson.json");
    serde_json::from_str(&std::fs::read_to_string(&path).expect("lesson.json")).expect("JSON")
}

const LESSON_28: &str = "06-pid-tuning-methods/28-manual-pi-tuning";

fn lesson_28() -> Value {
    real_lesson(LESSON_28)
}

fn with(mut lesson: Value, pointer: &str, value: Value) -> Value {
    *lesson.pointer_mut(pointer).expect("field") = value;
    lesson
}

fn reason(lesson: &Value, id: &str) -> String {
    availability(lesson, id).reason.expect("a reason")
}

#[test]
fn the_manual_pi_lesson_is_available_with_its_workflow() {
    // Catches: a real, complete lesson refused.
    let lesson = lesson_28();
    let checked = parse_contract(&lesson, "control-lesson-28").expect("the contract holds");
    let Lesson::Contract(contract) = checked else {
        panic!("a schema 4 lesson is a contract");
    };
    assert_eq!(
        contract["workflowStages"],
        json!(["Monitor", "Tune", "Test", "Validate"])
    );
    let available = availability(&lesson, "control-lesson-28");
    assert!(available.available);
    assert_eq!(available.reason, None);
    assert_eq!(available.lesson, Some(lesson));
}

#[test]
fn duplicate_and_unsupported_workflow_stages_are_refused() {
    let duplicate = with(
        lesson_28(),
        "/workflowStages",
        json!(["Monitor", "Tune", "Tune"]),
    );
    assert_eq!(
        parse_contract(&duplicate, "control-lesson-28").expect_err("refused"),
        "control lesson 'control-lesson-28' workflowStages contains duplicate stage 'Tune'"
    );
    let unsupported = with(
        lesson_28(),
        "/workflowStages",
        json!(["Monitor", "Unsupported"]),
    );
    assert_eq!(
        parse_contract(&unsupported, "control-lesson-28").expect_err("refused"),
        "control lesson 'control-lesson-28' workflowStages contains unsupported stage 'Unsupported'"
    );
}

#[test]
fn the_identification_and_energy_balance_lessons_hold_their_contracts() {
    // Lessons 14 and 13 of the course (control-course-units). Catches: a real lesson refused.
    for (folder, id) in [
        (
            "05-experiments-identification-and-validation/21-safe-informative-experiment",
            "control-lesson-21",
        ),
        (
            "01-feedback-foundations-and-temperature-control/15-water-heater-energy-balance",
            "control-water-heater-energy-balance",
        ),
    ] {
        let lesson = real_lesson(folder);
        assert!(
            matches!(parse_contract(&lesson, id), Ok(Lesson::Contract(_))),
            "{folder}"
        );
    }
}

#[test]
fn any_unit_label_is_accepted_but_control_characters_and_long_labels_are_not() {
    // The root-locus lesson's shaft units need no lesson-specific list; a label is refused only for
    // a control character or more than 80 UTF-16 code units. Catches: a closed list of units.
    let id = "control-root-locus";
    let lesson = real_lesson("07-classical-stability-and-frequency-design/45-root-locus");
    assert!(parse_contract(&lesson, id).is_ok());
    let mv = lesson["signals"]
        .as_array()
        .expect("signals")
        .iter()
        .position(|signal| signal["role"] == "MV")
        .expect("an MV signal");
    let pointer = format!("/signals/{mv}/unit");
    assert!(parse_contract(&with(lesson.clone(), &pointer, json!("kg·m²/s²")), id).is_ok());
    assert!(parse_contract(&with(lesson.clone(), &pointer, json!("x".repeat(80))), id).is_ok());
    let too_long = "x".repeat(81);
    for unit in ["N\u{0}m", "N\nm", too_long.as_str(), "N\u{7f}m"] {
        let error =
            parse_contract(&with(lesson.clone(), &pointer, json!(unit)), id).expect_err("refused");
        assert_eq!(
            error, "control lesson 'control-root-locus' signal 'MV' has an invalid unit label",
            "{unit:?}"
        );
    }
}

#[test]
fn a_summary_only_lesson_keeps_its_place_but_cannot_be_copied() {
    // Schema 2 is the summary that orders the course. Catches: a legacy lesson offered as runnable.
    let lesson = json!({
        "schemaVersion": 2, "sequence": 3, "level": "Foundation", "equipment": "Cold room",
        "principalTechnique": "On/off", "prerequisites": ["control-lesson-01"], "extra": true,
    });
    let found = availability(&lesson, "legacy");
    assert!(!found.available);
    assert_eq!(
        found.reason.as_deref(),
        Some("Requires the complete version 3 engineering contract.")
    );
    assert_eq!(
        found.lesson,
        Some(json!({
            "sequence": 3, "level": "Foundation", "equipment": "Cold room",
            "principalTechnique": "On/off", "prerequisites": ["control-lesson-01"],
        }))
    );
}

#[test]
fn a_lesson_without_its_plant_names_the_missing_plant_contract() {
    // The gallery's own wording for this case. Catches: the raw check text shown to a learner.
    let mut lesson = lesson_28();
    lesson.as_object_mut().expect("object").remove("plant");
    let found = availability(&lesson, "control-lesson-28");
    assert_eq!(
        found.reason.as_deref(),
        Some("Missing the executable plant contract.")
    );
    assert_eq!(
        found
            .lesson
            .as_ref()
            .map(|summary| summary["sequence"].clone()),
        Some(json!(11)),
        "the summary still orders the course"
    );
}

#[test]
fn a_failed_check_becomes_a_sentence_and_an_unreadable_summary_drops_the_lesson() {
    // `{}` fails the first summary check; a text is no object. Catches: an unavailable lesson shown
    // with a summary it does not have.
    let empty = availability(&json!({}), "x");
    assert_eq!(empty.reason.as_deref(), Some("Missing string 'level'."));
    assert_eq!(empty.lesson, None);
    let text = availability(&json!("lesson"), "x");
    assert_eq!(text.reason.as_deref(), Some("Metadata is not an object."));
    assert_eq!(
        reason(
            &with(lesson_28(), "/id", json!("other")),
            "control-lesson-28"
        ),
        "Id does not match manifest."
    );
    assert_eq!(
        reason(
            &with(lesson_28(), "/schemaVersion", json!(5)),
            "control-lesson-28"
        ),
        "Unsupported schemaVersion."
    );
    assert_eq!(
        reason(
            &with(lesson_28(), "/sequence", json!(1.5)),
            "control-lesson-28"
        ),
        "Has invalid sequence."
    );
}

#[test]
fn signals_need_a_measured_and_a_manipulated_variable_and_distinct_bindings() {
    // Catches: a loop without PV or MV, and two signals on one address.
    let mut lesson = lesson_28();
    let signals = lesson["signals"].as_array().expect("signals").clone();
    let without_pv: Vec<Value> = signals
        .iter()
        .filter(|signal| signal["role"] != "PV")
        .cloned()
        .collect();
    lesson["signals"] = json!(without_pv);
    assert_eq!(
        reason(&lesson, "control-lesson-28"),
        "Signals missing required role 'PV'."
    );
    let mut doubled = signals.clone();
    let mut copy = doubled[0].clone();
    copy["role"] = json!("state");
    doubled.push(copy);
    let binding = doubled[0]["binding"].as_str().expect("binding").to_owned();
    assert_eq!(
        reason(
            &with(lesson_28(), "/signals", json!(doubled)),
            "control-lesson-28"
        ),
        format!("Signals have duplicate binding '{binding}'.")
    );
}

#[test]
fn the_proof_command_must_name_the_native_test_as_an_argument() {
    // `cargo test x lesson28_extra` holds `lesson28` as text but not as an argument. Catches: a proof
    // that names another test whose name only begins like this one.
    let lesson = with(lesson_28(), "/proof/nativeTest", json!("lesson28"));
    let lesson = with(
        lesson,
        "/proof/nativeCommand",
        json!("cargo test x lesson28_extra"),
    );
    assert_eq!(
        parse_contract(&lesson, "control-lesson-28").expect_err("refused"),
        "control lesson 'control-lesson-28' proof nativeCommand does not name nativeTest 'lesson28' as an exact argument"
    );
    let lesson = with(lesson, "/proof/nativeCommand", json!("cargo test x"));
    assert_eq!(
        parse_contract(&lesson, "control-lesson-28").expect_err("refused"),
        "control lesson 'control-lesson-28' proof nativeCommand does not name nativeTest 'lesson28'"
    );
}

#[test]
fn an_experiment_settles_within_its_duration() {
    // Catches: a settling time longer than the experiment.
    let lesson = with(lesson_28(), "/experiment/durationSeconds", json!(10));
    let lesson = with(lesson, "/experiment/settlingSeconds", json!(10.5));
    assert_eq!(
        reason(&lesson, "control-lesson-28"),
        "Experiment has invalid settlingSeconds."
    );
}

#[test]
fn a_schema_4_lesson_needs_its_learning_sequence_and_may_end_the_course() {
    // `nextLesson: null` ends the course; an empty theory list is refused. Catches: the last lesson
    // refused, or a lesson without theory accepted.
    let last = with(lesson_28(), "/nextLesson", Value::Null);
    assert!(availability(&last, "control-lesson-28").available);
    let no_theory = with(lesson_28(), "/learning/theory", json!([]));
    assert_eq!(
        reason(&no_theory, "control-lesson-28"),
        "Has empty learning.theory."
    );
    let schema_3 = with(no_theory, "/schemaVersion", json!(3));
    assert!(
        availability(&schema_3, "control-lesson-28").available,
        "schema 3 has no learning sequence to check"
    );
}

#[test]
fn sentences_keep_quotes_and_drop_final_dots() {
    assert_eq!(sentence("  has invalid signals.. "), "Has invalid signals.");
    assert_eq!(sentence(""), "The lesson contract is invalid.");
    assert_eq!(sentence("..."), "The lesson contract is invalid.");
}
