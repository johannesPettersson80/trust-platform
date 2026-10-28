#![allow(clippy::float_cmp)] // The shipped values are decimals that the mapping and the rounding keep exactly, compared as written.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::*;
use crate::commissioning::parameter_status::RecordState;
use crate::loop_definition::LoopDefinition;

/// Lesson 45 (root locus): Kp is swept and selected at 16, Kd is fixed at 7, and both are mapped to program symbols.
const LESSON_45: &str = include_str!(
    "../../../../examples/control/07-classical-stability-and-frequency-design/45-root-locus/control-loop.json"
);

fn lesson_45() -> Value {
    serde_json::from_str(LESSON_45).expect("the shipped lesson is JSON")
}

fn loop_of(value: &Value) -> LoopDefinition {
    LoopDefinition::decode(&value.to_string()).expect("the loop decodes")
}

/// Lesson 45 as the engine reads it.
fn shaft() -> LoopDefinition {
    loop_of(&lesson_45())
}

fn writable_of_shaft() -> Vec<WritableParameter> {
    writable_parameters(&shaft()).expect("the mapping of lesson 45 is writable")
}

/// The record of the runtime, with the two mapped parameters and one unmapped one.
fn observed(kp: f64, kd: f64) -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("P1.Kp".to_string(), kp),
        ("P1.Kd".to_string(), kd),
        ("P1.Other".to_string(), 5.0),
    ])
}

fn proposed(kp: f64, kd: f64) -> BTreeMap<String, f64> {
    BTreeMap::from([("P1.Kp".to_string(), kp), ("P1.Kd".to_string(), kd)])
}

fn status(values: BTreeMap<String, f64>, active_revision: &str) -> ParameterStatus {
    ParameterStatus {
        supports_request_id: Some(true),
        transaction_id: None,
        program_revision: "program-1".to_string(),
        binding_revision: "binding-1".to_string(),
        active_revision: active_revision.to_string(),
        values,
        status: RecordState::Idle,
        previous_revision: None,
        previous_values: None,
        reason: None,
    }
}

/// The refusal of a loop file whose mapping or binding is changed by `change`. The loop is read as its fields alone: the open-time rules
/// of the mapping (`analysis_rules.rs`) refuse such a file before any write, so this pins the write's own check.
fn refused_after(change: impl FnOnce(&mut Value)) -> Failure {
    let mut value = lesson_45();
    change(&mut value);
    let changed: LoopDefinition =
        serde_json::from_value(value).expect("the changed loop's fields decode");
    writable_parameters(&changed).expect_err("the change is refused")
}

#[test]
fn the_mapped_parameters_of_lesson_45_are_the_writable_ones_in_name_order() {
    let writable = writable_of_shaft();
    let pairs: Vec<(&str, &str)> = writable
        .iter()
        .map(|parameter| (parameter.name.as_str(), parameter.symbol.as_str()))
        .collect();
    assert_eq!(pairs, vec![("Kd", "P1.Kd"), ("Kp", "P1.Kp")]);
    assert_eq!(writable[0].minimum, 7.0);
    assert_eq!(writable[0].maximum, 7.0);
    assert_eq!(writable[1].minimum, 0.0);
    assert_eq!(writable[1].maximum, 20.0);
    assert_eq!(writable[1].unit, "N·m/rad");
}

#[test]
fn the_page_is_shown_only_the_writable_parameters_with_the_bounds_the_analysis_allows() {
    // C12: the page shows the engine's binding, not the loop's whole record and not the binding's own bounds.
    let shaft = shaft();
    let binding = writable_binding(&shaft, &writable_of_shaft()).expect("lesson 45 has a binding");
    let shown: Vec<(&str, f64, f64)> = binding
        .parameters
        .iter()
        .map(|parameter| {
            (
                parameter.symbol.as_str(),
                parameter.minimum,
                parameter.maximum,
            )
        })
        .collect();
    assert_eq!(shown, [("P1.Kd", 7.0, 7.0), ("P1.Kp", 0.0, 20.0)]);
    let declared = shaft
        .parameter_binding
        .as_ref()
        .expect("the loop's binding");
    assert_eq!(binding.mode, declared.mode, "the mode stays the loop's");
    assert_eq!(binding.task_name, declared.task_name);
}

#[test]
fn the_selected_values_are_the_root_locus_selection_and_the_declared_fixed_values() {
    let selected = selected_values(&writable_of_shaft());
    assert_eq!(
        selected,
        BTreeMap::from([("P1.Kd".to_string(), 7.0), ("P1.Kp".to_string(), 16.0)])
    );
}

#[test]
fn a_mapping_that_misses_an_analysis_parameter_is_refused() {
    let error = refused_after(|value| {
        value["analysisParameterSymbols"]
            .as_object_mut()
            .expect("the mapping")
            .remove("Kd");
    });
    assert_eq!(error.message, MAPPING_INVALID);
    assert_eq!(error.next_action, NextAction::OpenLoopSetup);
}

#[test]
fn a_mapping_that_names_one_symbol_twice_is_refused() {
    let error = refused_after(|value| {
        value["analysisParameterSymbols"]["Kd"] = json!("P1.Kp");
    });
    assert_eq!(error.message, MAPPING_INVALID);
}

#[test]
fn a_mapping_to_a_symbol_outside_the_binding_is_refused() {
    let error = refused_after(|value| {
        value["analysisParameterSymbols"]["Kp"] = json!("P1.Missing");
    });
    assert_eq!(error.message, MAPPING_INVALID);
}

#[test]
fn a_mapped_symbol_with_another_unit_is_refused() {
    let error = refused_after(|value| {
        value["parameterBinding"]["parameters"][0]["unit"] = json!("rad");
    });
    assert_eq!(
        error.message,
        "P1.Kp does not have the unit or the range of its analysis parameter Kp."
    );
}

#[test]
fn a_binding_range_that_does_not_hold_the_analysis_range_is_refused() {
    let error = refused_after(|value| {
        value["parameterBinding"]["parameters"][0]["maximum"] = json!(15.0);
    });
    assert!(error
        .message
        .contains("does not have the unit or the range"));
}

#[test]
fn a_loop_without_a_mapping_writes_nothing() {
    let error = refused_after(|value| {
        value
            .as_object_mut()
            .expect("the loop")
            .remove("analysisParameterSymbols");
    });
    assert_eq!(error.message, NO_MAPPING);
}

#[test]
fn a_review_must_name_exactly_the_mapped_symbols() {
    let writable = writable_of_shaft();
    let missing = BTreeMap::from([("P1.Kp".to_string(), 4.0)]);
    let error = written_record(&writable, &observed(1.0, 7.0), &missing)
        .expect_err("a missing symbol is refused");
    assert_eq!(error.message, NOT_WRITABLE);
    let mut unmapped = proposed(4.0, 7.0);
    unmapped.insert("P1.Other".to_string(), 6.0);
    let error = written_record(&writable, &observed(1.0, 7.0), &unmapped)
        .expect_err("an unmapped symbol is refused");
    assert_eq!(error.message, NOT_WRITABLE);
}

#[test]
fn a_value_outside_the_analysis_range_is_refused() {
    let error = written_record(
        &writable_of_shaft(),
        &observed(1.0, 7.0),
        &proposed(21.0, 7.0),
    )
    .expect_err("21 is above the analysis maximum of 20");
    assert_eq!(
        error.message,
        "P1.Kp = 21 N·m/rad is outside the range 0 to 20 that its analysis parameter Kp allows."
    );
    assert_eq!(error.cause, Cause::Settings);
}

#[test]
fn a_real_value_is_checked_after_single_precision_rounding() {
    // 0.1 is not exact in single precision: the REAL rounds it above the analysis maximum of 0.1, the LREAL keeps it.
    let real = WritableParameter {
        name: "Kp".to_string(),
        symbol: "P1.Kp".to_string(),
        iec_type: RealType::Real,
        unit: "N·m/rad".to_string(),
        minimum: 0.0,
        maximum: 0.1,
        selected: 0.1,
    };
    let lreal = WritableParameter {
        iec_type: RealType::Lreal,
        ..real.clone()
    };
    let values = BTreeMap::from([("P1.Kp".to_string(), 0.1)]);
    assert!(written_record(&[real], &BTreeMap::new(), &values).is_err());
    let record =
        written_record(&[lreal], &BTreeMap::new(), &values).expect("0.1 is an LREAL value");
    assert_eq!(record["P1.Kp"], 0.1);
}

#[test]
fn a_write_sets_the_mapped_values_and_keeps_every_other_value_of_the_record() {
    let record = written_record(
        &writable_of_shaft(),
        &observed(1.0, 7.0),
        &proposed(4.0, 7.0),
    )
    .expect("4 is inside the range");
    assert_eq!(record["P1.Kp"], 4.0);
    assert_eq!(record["P1.Kd"], 7.0);
    assert_eq!(
        record["P1.Other"], 5.0,
        "an unmapped parameter keeps its value"
    );
}

#[test]
fn the_changes_show_the_value_in_force_and_the_value_written() {
    let writable = writable_of_shaft();
    let before = observed(1.0, 7.0);
    let record = written_record(&writable, &before, &proposed(4.0, 7.0)).expect("in range");
    let changes = written_changes(&writable, &before, &record);
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[1].symbol, "P1.Kp");
    assert_eq!(changes[1].running, 1.0);
    assert_eq!(changes[1].next, 4.0);
    assert_eq!(changes[0].symbol, "P1.Kd");
    assert_eq!(changes[0].running, changes[0].next);
}

#[test]
fn the_identity_of_a_review_is_the_same_for_the_same_record_and_changes_with_it() {
    let loop_definition = shaft();
    let writable = writable_of_shaft();
    let before = status(observed(1.0, 7.0), "active-1");
    let record = written_record(&writable, &before.values, &proposed(4.0, 7.0)).expect("in range");
    let identity = |record: &BTreeMap<String, f64>, observed: &ParameterStatus| {
        write_identity(
            &loop_definition,
            ParameterWriteAction::Apply,
            observed,
            None,
            &writable,
            record,
        )
    };
    let first = identity(&record, &before);
    assert!(first.starts_with("sha256:"));
    assert_eq!(
        first,
        identity(&record, &before),
        "the same settings give the same identity"
    );
    let other_record =
        written_record(&writable, &before.values, &proposed(5.0, 7.0)).expect("in range");
    assert_ne!(
        first,
        identity(&other_record, &before),
        "another value gives another identity"
    );
    let moved = status(observed(1.0, 7.0), "active-2");
    assert_ne!(
        first,
        identity(&record, &moved),
        "a record that changed since the review gives another identity"
    );
}

#[test]
fn a_restore_identity_names_the_revision_it_returns_to_and_differs_from_an_apply() {
    let loop_definition = shaft();
    let writable = writable_of_shaft();
    let record = observed(1.0, 7.0);
    let current = status(observed(4.0, 7.0), "active-2");
    let apply = write_identity(
        &loop_definition,
        ParameterWriteAction::Apply,
        &current,
        None,
        &writable,
        &record,
    );
    let restore = write_identity(
        &loop_definition,
        ParameterWriteAction::Restore,
        &current,
        Some("active-1"),
        &writable,
        &record,
    );
    assert_ne!(apply, restore);
    let elsewhere = write_identity(
        &loop_definition,
        ParameterWriteAction::Restore,
        &current,
        Some("active-0"),
        &writable,
        &record,
    );
    assert_ne!(
        restore, elsewhere,
        "the revision a Restore returns to is part of its identity"
    );
}

#[test]
fn a_changed_loop_revision_changes_the_identity() {
    let writable = writable_of_shaft();
    let current = status(observed(1.0, 7.0), "active-1");
    let identity = |loop_definition: &LoopDefinition| {
        write_identity(
            loop_definition,
            ParameterWriteAction::Apply,
            &current,
            None,
            &writable,
            &observed(1.0, 7.0),
        )
    };
    let mut edited = lesson_45();
    edited["revision"] = json!("sha256:edited-after-review");
    assert_ne!(identity(&shaft()), identity(&loop_of(&edited)));
}

#[test]
fn the_write_actions_have_their_wire_words() {
    assert_eq!(
        serde_json::to_value(ParameterWriteAction::Apply).expect("apply"),
        json!("apply")
    );
    assert_eq!(
        serde_json::to_value(ParameterWriteAction::Restore).expect("restore"),
        json!("restore")
    );
    let written = WrittenValue {
        symbol: "P1.Kp".to_string(),
        unit: "N·m/rad".to_string(),
        running: 1.0,
        next: 4.0,
    };
    assert_eq!(
        serde_json::to_value(written).expect("the value"),
        json!({"symbol": "P1.Kp", "unit": "N·m/rad", "running": 1.0, "next": 4.0})
    );
}
