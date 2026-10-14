#![allow(clippy::float_cmp)]

// The TypeScript assertions of `control-studio-keep-in-project.test.ts` that test these rules are ported here: the record and
// its fields (`keepableRecord`), the reason texts (`keptReason`), the hand-written edit (`keepAppliedParameters`, with its
// `applyProjectEdit` text) and the equipment settings a keep writes (`keepEquipmentSettings`). The coordinator and the message
// decoding are presentation and are not ported. The other assertions state Specification 35 B6.5, §0.1b and Specification 14 §7.8.

use serde_json::json;

use super::*;
use crate::commissioning::fixture_loops;
use crate::loop_definition::{Parameter, RealType};

fn decode_ok(answer: Value, command: &str) -> Reply {
    decode_reply(answer, command, "fallback").expect("a well-formed reply")
}

fn record_of(fields: &[&str]) -> KeepableRecord {
    KeepableRecord {
        record: "Main.Parameters".to_owned(),
        fields: fields.iter().map(|field| (*field).to_owned()).collect(),
    }
}

fn text_edit(start: (u32, u32), end: (u32, u32), new_text: &str) -> TextEdit {
    TextEdit {
        range: Range {
            start: Position {
                line: start.0,
                character: start.1,
            },
            end: Position {
                line: end.0,
                character: end.1,
            },
        },
        new_text: new_text.to_owned(),
    }
}

#[test]
fn the_record_is_the_one_the_pid_law_is_configured_from_with_its_bound_fields() {
    // The fixture binds `Main.Parameters.Kp`, `Ki` and `Kd`. The other eight are the record of the TypeScript test's loop,
    // so the record has the eleven numeric fields that `keepableRecord` keeps (keepInProject.ts; B6.5).
    let mut loop_definition = fixture_loops::control_pid_loop();
    let binding = loop_definition
        .parameter_binding
        .as_mut()
        .expect("the fixture binds its record");
    let numeric = [
        "ProportionalSetpointWeight",
        "DerivativeSetpointWeight",
        "Bias",
        "OutputMinimum",
        "OutputMaximum",
        "RateMinimum",
        "RateMaximum",
        "TrackingTime",
    ];
    for field in numeric {
        binding.parameters.push(Parameter {
            symbol: format!("Main.Parameters.{field}"),
            iec_type: RealType::Lreal,
            unit: "%".to_owned(),
            convention: field.to_owned(),
            minimum: -100.0,
            maximum: 100.0,
        });
    }
    // A field of a nested structure is not a field of the record (`!symbol.slice(prefix.length).includes(".")`).
    binding.parameters.push(Parameter {
        symbol: "Main.Parameters.Nested.Gain".to_owned(),
        iec_type: RealType::Lreal,
        unit: "%".to_owned(),
        convention: "nested".to_owned(),
        minimum: -100.0,
        maximum: 100.0,
    });
    let record = keepable_record(&loop_definition).expect("the loop binds its record");
    assert_eq!(record.record, "Main.Parameters");
    assert_eq!(
        record.fields.len(),
        11,
        "every numeric field the loop binds"
    );
    assert_eq!(record.fields[..3], ["Kp", "Ki", "Kd"]);
}

#[test]
fn a_loop_without_a_pid_law_or_a_binding_has_no_record() {
    let mut without_law = fixture_loops::control_pid_loop();
    without_law.controller.law = None;
    let failure = keepable_record(&without_law).expect_err("no law, no record");
    assert_eq!(failure.message, keep_texts::no_record());
    assert_eq!(failure.cause, Cause::Implementation);
    assert_eq!(failure.next_action, NextAction::OpenLoopSetup);

    let mut without_binding = fixture_loops::control_pid_loop();
    without_binding.parameter_binding = None;
    assert!(keepable_record(&without_binding).is_err());
}

#[test]
fn only_finite_running_values_of_the_bound_fields_are_kept() {
    let running = BTreeMap::from([
        ("Main.Parameters.Kp".to_owned(), 2.5),
        ("Main.Parameters.Ki".to_owned(), f64::NAN),
        ("Main.Parameters.Kd".to_owned(), f64::INFINITY),
        ("Main.Parameters.Other".to_owned(), 9.0),
    ]);
    let values = kept_values(&record_of(&["Kp", "Ki", "Kd"]), &running).expect("one value");
    assert_eq!(values, BTreeMap::from([("Kp".to_owned(), 2.5)]));
}

#[test]
fn a_runtime_that_reports_no_values_refuses_the_keep() {
    let failure = kept_values(&record_of(&["Kp"]), &BTreeMap::new()).expect_err("no values");
    assert_eq!(
        failure.message,
        "The runtime reports no values of Main.Parameters."
    );
    assert_eq!(failure.cause, Cause::Runtime);
    assert_eq!(failure.next_action, NextAction::ConnectRuntime);
}

#[test]
fn a_reply_with_an_edit_gives_its_edits_expected_texts_and_kept_fields() {
    let uri = "file:///project/src/Main.st";
    let reply = decode_ok(
        json!({
            "ok": true,
            "edit": { "changes": { uri: [{ "range": { "start": { "line": 2, "character": 10 },
                "end": { "line": 2, "character": 19 } }, "newText": "LREAL#2.5" }] } },
            "expected": { uri: "PROGRAM Main\n" },
            "kept": ["ProportionalGain"]
        }),
        "trust-lsp.controlKeepParameters",
    );
    let Reply::Edit(edit) = reply else {
        panic!("an edit reply");
    };
    assert_eq!(edit.expected[uri], "PROGRAM Main\n");
    assert_eq!(
        edit.changes[uri],
        [text_edit((2, 10), (2, 19), "LREAL#2.5")]
    );
    assert_eq!(edit.kept, Some(vec!["ProportionalGain".to_owned()]));
    assert!(edit.creates.is_empty());
}

#[test]
fn a_reply_with_a_generated_record_gives_its_instance_and_settings() {
    let reply = decode_ok(
        json!({ "ok": false, "generated": { "instance": "Main.TIC101",
            "settings": { "{instance}_Parameters.ProportionalGain": 33.3, "{instance}_Parameters.IntegralGain": 0.0667 } } }),
        "trust-lsp.controlKeepParameters",
    );
    assert_eq!(
        reply,
        Reply::Generated {
            instance: "Main.TIC101".to_owned(),
            settings: BTreeMap::from([
                ("{instance}_Parameters.IntegralGain".to_owned(), 0.0667),
                ("{instance}_Parameters.ProportionalGain".to_owned(), 33.3),
            ]),
        }
    );
}

#[test]
fn a_refusal_gives_its_first_sentence_or_the_fallback() {
    let first = decode_ok(
        json!({ "ok": false, "errors": [{ "message": "First." }, { "message": "Second." }] }),
        "trust-lsp.controlKeepParameters",
    );
    assert_eq!(first, Reply::Refused("First.".to_owned()));
    let error = decode_ok(
        json!({ "ok": false, "error": "Plain." }),
        "trust-lsp.hmiEquipment",
    );
    assert_eq!(error, Reply::Refused("Plain.".to_owned()));
    let none = decode_ok(json!({ "ok": false }), "trust-lsp.hmiEquipment");
    assert_eq!(none, Reply::Refused("fallback".to_owned()));
}

#[test]
fn an_equipment_description_gives_its_ownership_and_settings_by_member() {
    let reply = decode_ok(
        json!({ "ok": true, "instance": "Main.TIC101", "owned": true, "declared": true,
            "settings": [
                { "member": "{instance}_Parameters.ProportionalGain", "value": 33.3, "unit": null },
                { "member": "{instance}_Parameters.AlarmText", "value": "T-101" }
            ] }),
        "trust-lsp.hmiEquipment",
    );
    let Reply::Described(equipment) = reply else {
        panic!("a description");
    };
    assert!(equipment.owned);
    assert_eq!(
        equipment.settings["{instance}_Parameters.ProportionalGain"],
        json!(33.3)
    );
    assert_eq!(equipment.settings.len(), 2);
}

#[test]
fn a_reply_that_does_not_have_the_protocol_form_is_refused() {
    let failure = decode_reply(json!({ "ok": "yes" }), "trust-lsp.hmiEquipment", "fallback")
        .expect_err("a malformed reply");
    assert_eq!(
        failure.message,
        "The language tool's answer to trust-lsp.hmiEquipment does not match the protocol."
    );
    assert_eq!(failure.cause, Cause::Implementation);
}

#[test]
fn an_equipment_keep_writes_only_the_values_that_differ_and_names_the_rest() {
    // `keepEquipmentSettings` (equipmentEdits.ts): the equipment's settings are 1 and 0.0667; the PLC holds 33.3, 0.0667 and 1.
    let equipment = KeepEquipment {
        owned: true,
        settings: BTreeMap::from([
            (
                "{instance}_Parameters.ProportionalGain".to_owned(),
                json!(1),
            ),
            (
                "{instance}_Parameters.IntegralGain".to_owned(),
                json!(0.0667),
            ),
        ]),
    };
    let values = BTreeMap::from([
        ("{instance}_Parameters.IntegralGain".to_owned(), 0.0667),
        ("{instance}_Parameters.ProportionalGain".to_owned(), 33.3),
        ("{instance}_Parameters.TrackingTime".to_owned(), 1.0),
    ]);
    let keep = equipment_keep(&equipment, &values);
    assert_eq!(
        keep.settings,
        BTreeMap::from([("{instance}_Parameters.ProportionalGain".to_owned(), 33.3)])
    );
    assert_eq!(
        keep.kept,
        ["{instance}_Parameters.ProportionalGain".to_owned()]
    );
    assert_eq!(
        keep.unknown,
        ["{instance}_Parameters.TrackingTime".to_owned()]
    );
    assert_eq!(member_field(&keep.kept[0]), "ProportionalGain");
}

#[test]
fn a_setting_is_the_same_only_within_a_rounding_difference() {
    assert!(same_setting(&json!(0.1 + 0.2), 0.3));
    assert!(same_setting(&json!(1000.0), 1000.0 + 5e-7));
    assert!(!same_setting(&json!(1000.0), 1000.0 + 2e-6));
    assert!(!same_setting(&json!(1.0), 1.000_001));
    assert!(!same_setting(&json!("1.0"), 1.0));
    assert!(!same_setting(&json!(true), 1.0));
}

#[test]
fn a_hand_written_initializer_takes_the_typed_literal_and_nothing_else_changes() {
    // The TypeScript test's file: the kept field's literal is replaced, `LREAL#` stays, and no other text changes.
    let text = "PROGRAM Main\nVAR\n    Parameters : Control.PIDParameters := (ProportionalGain := LREAL#1.0);\nEND_VAR\nEND_PROGRAM\n";
    let line = text.split('\n').nth(2).expect("the declaration line");
    let at = u32::try_from(line.find("LREAL#1.0").expect("the literal")).expect("a short line");
    let edits = [text_edit((2, at), (2, at + 9), "LREAL#2.5")];
    let edited = apply_text_edits(text, &edits).expect("the edit fits");
    assert_eq!(edited, text.replace("LREAL#1.0", "LREAL#2.5"));
}

#[test]
fn text_edits_are_positioned_in_utf_16_units() {
    // "a" is one unit, the emoji is two, so the character after it is 3.
    let text = "a\u{1F600}b\n";
    let edited = apply_text_edits(text, &[text_edit((0, 3), (0, 4), "c")]).expect("in range");
    assert_eq!(edited, "a\u{1F600}c\n");
    // Character 2 lies inside the emoji's surrogate pair.
    assert!(apply_text_edits(text, &[text_edit((0, 2), (0, 3), "c")]).is_err());
}

#[test]
fn insertions_at_one_position_keep_their_order() {
    let edited = apply_text_edits(
        "ab",
        &[
            text_edit((0, 1), (0, 1), ", X"),
            text_edit((0, 1), (0, 1), ", Y"),
        ],
    )
    .expect("two insertions");
    assert_eq!(edited, "a, X, Yb");
}

#[test]
fn overlapping_edits_are_refused() {
    let failure = apply_text_edits(
        "abcdef",
        &[
            text_edit((0, 1), (0, 4), "x"),
            text_edit((0, 3), (0, 5), "y"),
        ],
    )
    .expect_err("overlap");
    assert_eq!(failure.message, "Two edits of one file overlap.");
}

#[test]
fn a_position_outside_the_text_is_refused() {
    let text = "one\ntwo\n";
    // Line 5 does not exist.
    assert!(apply_text_edits(text, &[text_edit((5, 0), (5, 0), "x")]).is_err());
    // Character 4 is past the end of "one".
    assert!(apply_text_edits(text, &[text_edit((0, 4), (0, 4), "x")]).is_err());
    // The line after the last break is empty and exists.
    assert_eq!(
        apply_text_edits(text, &[text_edit((2, 0), (2, 0), "end")]).expect("the end"),
        "one\ntwo\nend"
    );
}

#[test]
fn a_crlf_line_ends_before_its_break() {
    // Line 0 is "ab" (the break is not part of it); the range from its end to the next line's start removes the break.
    let edited = apply_text_edits("ab\r\ncd", &[text_edit((0, 2), (1, 0), "")]).expect("in range");
    assert_eq!(edited, "abcd");
}

#[test]
fn file_uris_become_paths_with_their_escapes_decoded() {
    assert_eq!(
        file_uri_path("file:///tmp/my%20folder/Main.st").expect("a file URI"),
        PathBuf::from("/tmp/my folder/Main.st")
    );
    assert_eq!(
        file_uri_path("file://localhost/tmp/Main.st").expect("a localhost URI"),
        PathBuf::from("/tmp/Main.st")
    );
    assert!(file_uri_path("https://example.com/Main.st").is_err());
    assert!(file_uri_path("file://host/tmp/Main.st").is_err());
    assert!(file_uri_path("file:///tmp/a%ZZ").is_err());
    assert!(file_uri_path("file:///tmp/a%FF").is_err(), "not UTF-8");
}

#[cfg(windows)]
#[test]
fn a_windows_drive_letter_keeps_no_leading_slash() {
    assert_eq!(
        file_uri_path("file:///C:/work/Main.st").expect("a drive path"),
        PathBuf::from("C:/work/Main.st")
    );
}

#[test]
fn the_review_identity_is_the_same_for_the_same_inputs_and_changes_with_each_of_them() {
    let values = BTreeMap::from([("Kp".to_owned(), 2.5)]);
    let files = [("/p/src/Main.st".to_owned(), Some("text".to_owned()))];
    let identity = review_identity("Main.Parameters", &values, &files);
    assert!(identity.starts_with("sha256:"));
    assert_eq!(identity.len(), "sha256:".len() + 64);
    assert_eq!(
        identity,
        review_identity("Main.Parameters", &values, &files)
    );
    assert_ne!(identity, review_identity("Main.Other", &values, &files));
    assert_ne!(
        identity,
        review_identity(
            "Main.Parameters",
            &BTreeMap::from([("Kp".to_owned(), 2.6)]),
            &files
        )
    );
    let changed = [("/p/src/Main.st".to_owned(), Some("text!".to_owned()))];
    assert_ne!(
        identity,
        review_identity("Main.Parameters", &values, &changed)
    );
    let created = [("/p/src/Main.st".to_owned(), None)];
    assert_ne!(
        identity,
        review_identity("Main.Parameters", &values, &created)
    );
}

#[test]
fn the_kept_report_names_the_fields_and_the_settings_it_skipped() {
    // `keptReason` (keepInProject.ts), as the TypeScript test states it.
    assert_eq!(
        keep_texts::kept_reason(
            "P1.Parameters",
            &["ProportionalGain".to_owned()],
            &["TrackingTime".to_owned()]
        ),
        "Kept in the project: ProportionalGain of P1.Parameters. A restart or a new download keeps them. \
         The equipment has no setting for TrackingTime; copy it into the PLC code by hand."
    );
    assert_eq!(
        keep_texts::kept_reason("P1.Parameters", &[], &[]),
        "The project already has these values of P1.Parameters."
    );
    assert_eq!(
        keep_texts::kept_reason(
            "Main.TIC101_Parameters",
            &["IntegralGain".to_owned()],
            &["A".to_owned(), "B".to_owned()]
        ),
        "Kept in the project: IntegralGain of Main.TIC101_Parameters. A restart or a new download keeps them. \
         The equipment has no setting for A, B; copy it into the PLC code by hand."
    );
}
