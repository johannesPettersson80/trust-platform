//! Tests of the Ladder rules (specification 15, specification 16, specification 25 §11.4).
//!
//! Expected values are worked out by hand from the rules in the module documentation; each test
//! names the wrong behaviour it catches.

mod edits;
mod elements;
mod lowering;

use super::*;

/// The fixture `editors/vscode/src/test/fixtures/ladder/schema-v2-valid.ladder.json`.
const VALID: &str = r#"{
  "schemaVersion": 2,
  "metadata": { "name": "valid-v2", "description": "valid fixture" },
  "variables": [],
  "networks": [
    {
      "id": "rung_1",
      "order": 0,
      "layout": { "y": 100 },
      "nodes": [
        { "id": "c1", "type": "contact", "contactType": "NO", "variable": "%IX0.0", "position": { "x": 100, "y": 100 } },
        { "id": "q1", "type": "coil", "coilType": "NORMAL", "variable": "%QX0.0", "position": { "x": 260, "y": 100 } }
      ],
      "edges": []
    }
  ]
}"#;

fn json(text: &str) -> Json {
    Json::parse(text).expect("test JSON")
}

#[test]
fn a_schema_v2_program_passes() {
    assert_eq!(validate(&json(VALID)), Ok(()));
}

#[test]
fn a_legacy_program_is_refused_with_every_problem_named() {
    // Specification 16 §2: no `schemaVersion: 2` is refused, never migrated. The legacy fixture
    // has `rungs` instead of `variables`/`networks`: three problems, one line each. Catches a
    // check that stops at the first problem or silently migrates.
    let legacy = json(
        r#"{"metadata":{"name":"legacy-v1","description":"legacy fixture"},"rungs":[{"id":"legacy_rung_1","elements":[]}]}"#,
    );
    assert_eq!(
        validate(&legacy),
        Err("Unsupported ladder schema. Expected strict schemaVersion: 2 contract.\n- schemaVersion must be exactly 2.\n- variables must be an array.\n- networks must be an array.".to_owned())
    );
    assert_eq!(
        validate(&json("[]")),
        Err("Unsupported ladder schema: root object is missing.".to_owned())
    );
}

#[test]
fn an_unknown_enum_symbol_is_refused_not_coerced() {
    // Specification 16 §2: enum members are strict. Catches a coil type `LATCH` read as NORMAL.
    let mut program = json(VALID);
    let coil = program
        .get_mut("networks")
        .and_then(Json::as_array_mut)
        .and_then(|networks| networks[0].get_mut("nodes"))
        .and_then(Json::as_array_mut)
        .map(|nodes| &mut nodes[1])
        .expect("coil");
    coil.set("coilType", Json::str("LATCH"));
    let error = validate(&program).expect_err("refused");
    assert!(
        error
            .ends_with("\n- networks[0].nodes[1].coilType must be NORMAL, SET, RESET, or NEGATED."),
        "{error}"
    );
}

#[test]
fn members_of_the_wrong_shape_are_each_named() {
    // Worked by hand: a variable of an unknown type and scope, a node without a position, an
    // edge with a point that has no y, and a timer without outputs. Catches a check that skips
    // a member kind.
    let program = json(
        r#"{"schemaVersion":2,"metadata":{"name":"n","description":"","created":5},
          "variables":[{"name":"x","type":"WORD","scope":"static","address":7}],
          "networks":[{"id":"r","order":0,"layout":{"y":0},
            "nodes":[{"id":"a","type":"contact","contactType":"NO","variable":"x"},
                     {"id":"t","type":"timer","timerType":"TON","instance":"T","presetMs":10,"qOutput":" ","position":{"x":0,"y":0}}],
            "edges":[{"id":"e","fromNodeId":"a","toNodeId":"t","points":[{"x":1}]}]}]}"#,
    );
    assert_eq!(
        validate(&program),
        Err([
            "Unsupported ladder schema. Expected strict schemaVersion: 2 contract.",
            "- metadata.created must be a string when provided.",
            "- variables[0].type must be one of BOOL|INT|REAL|TIME|DINT|LREAL.",
            "- variables[0].scope must be 'local' or 'global' when provided.",
            "- variables[0].address must be a string when provided.",
            "- networks[0].nodes[0].position must be an object.",
            "- networks[0].nodes[1].qOutput must be a non-empty string.",
            "- networks[0].nodes[1].etOutput must be a non-empty string.",
            "- networks[0].edges[0].points[0] must contain numeric x/y.",
        ]
        .join("\n"))
    );
}

#[test]
fn a_blank_file_opens_as_an_empty_program() {
    let program = load("  \n", "2026-10-26T10:00:00.000Z").expect("empty program");
    assert_eq!(
        program.to_compact(),
        r#"{"schemaVersion":2,"networks":[],"variables":[],"metadata":{"name":"New Ladder Program","description":"Ladder logic program","created":"2026-10-26T10:00:00.000Z","modified":"2026-10-26T10:00:00.000Z"}}"#
    );
}

#[test]
fn opening_keeps_member_order_reduces_metadata_and_declares_contact_symbols() {
    // The editor opens the program with metadata name/description/created/modified only
    // (specification 16 §2) and the undeclared contact symbol `Run` declared as an implicit
    // local. Catches a reader that sorts members or keeps `version`.
    let text = r#"{"schemaVersion":2,"metadata":{"version":"1.0.0","name":"m","description":"d","created":"c"},
      "variables":[],"networks":[{"id":"r","order":0,"layout":{"y":100},"edges":[],
      "nodes":[{"id":"a","type":"contact","contactType":"NO","variable":"Run","position":{"x":100,"y":100}}]}]}"#;
    let program = load(text, "now").expect("valid program");
    assert_eq!(
        program.to_compact(),
        r#"{"schemaVersion":2,"metadata":{"name":"m","description":"d","created":"c"},"variables":[{"name":"Run","type":"BOOL","scope":"local","initialValue":false}],"networks":[{"id":"r","order":0,"layout":{"y":100},"edges":[],"nodes":[{"id":"a","type":"contact","contactType":"NO","variable":"Run","position":{"x":100,"y":100}}]}]}"#
    );
    assert!(load("{", "now").is_err());
    assert!(load(r#"{"schemaVersion":1}"#, "now")
        .expect_err("refused")
        .starts_with("Unsupported ladder schema."));
}

#[test]
fn saving_orders_rungs_stamps_modified_and_writes_the_editor_layout() {
    // Rungs are saved in ascending `order`, equal orders keep their place; `modified` is the
    // save time; the text is JSON.stringify(program, null, 2) and a newline. Catches an unstable
    // sort or a missing final newline.
    let program = json(
        r#"{"schemaVersion":2,"networks":[
          {"id":"b","order":1,"nodes":[],"edges":[],"layout":{"y":200}},
          {"id":"a","order":0,"nodes":[],"edges":[],"layout":{"y":100}},
          {"id":"c","order":1,"nodes":[],"edges":[],"layout":{"y":300}}],
          "variables":[],"metadata":{"name":"s","description":""}}"#,
    );
    let text = save_text(&program, "T").expect("saved");
    let saved = json(&text);
    let ids: Vec<&str> = saved
        .get("networks")
        .and_then(Json::as_array)
        .expect("networks")
        .iter()
        .filter_map(|network| network.str_at("id"))
        .collect();
    assert_eq!(ids, ["a", "b", "c"]);
    assert_eq!(
        saved.get("metadata").and_then(|m| m.str_at("modified")),
        Some("T")
    );
    assert!(
        text.ends_with("}\n") && text.starts_with("{\n  \"schemaVersion\": 2,\n  \"networks\": [")
    );
    let invalid = json(r#"{"schemaVersion":2}"#);
    assert!(save_text(&invalid, "T").is_err());
}
