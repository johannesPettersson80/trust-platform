//! The Blockly companion (specification 25 §11.4: "Blockly lowering preserves connected
//! statement order, control-input slots, variable identity, and deterministic inference for
//! supported untyped numeric variables"). Expected texts are worked out from the lowering rules in
//! `blocks.rs`; the example texts are the ones the TypeScript generator wrote before the move.

use super::{check_format, generate};
use crate::companion::companion;
use crate::{Global, Json};

fn workspace(text: &str) -> Json {
    Json::parse(text).expect("valid workspace JSON")
}

fn function_block(text: &str, base: &str) -> String {
    generate(&workspace(text), base)
        .expect("a Blockly workspace")
        .function_block
}

#[test]
fn a_connected_chain_keeps_its_statement_order() {
    // Ported from blockly-engine.test.ts. Catches a lowering that drops the inline `next` block
    // or emits it before the block it follows.
    let text = r#"{
      "blocks": { "languageVersion": 0, "blocks": [
        { "id": "set-1", "type": "variables_set",
          "fields": { "VAR": { "id": "var-counter", "name": "counter" } },
          "inputs": { "VALUE": { "block": { "id": "num-1", "type": "math_number", "fields": { "NUM": 1 } } } },
          "next": { "block": { "id": "set-2", "type": "variables_set",
            "fields": { "VAR": { "id": "var-counter", "name": "counter" } },
            "inputs": { "VALUE": { "block": { "id": "num-2", "type": "math_number", "fields": { "NUM": 2 } } } } } } }
      ] },
      "variables": [ { "id": "var-counter", "name": "counter", "type": "INT" } ],
      "metadata": { "name": "ChainProgram" }
    }"#;
    let generated = generate(&workspace(text), "chain").expect("workspace");
    assert_eq!(
        generated.function_block,
        "FUNCTION_BLOCK FB_chain_BLOCKLY\n\nVAR\n  counter : INT;\nEND_VAR\n\n(* Generated from Blockly *)\n\ncounter := 1;\ncounter := 2;\n\nEND_FUNCTION_BLOCK"
    );
    assert_eq!(generated.warnings, Vec::<String>::new());
    assert_eq!(generated.globals, Vec::<Global>::new());
}

#[test]
fn if_blocks_read_the_if0_and_do0_slots() {
    // Ported from blockly-engine.test.ts: Blockly saves the first condition and body of
    // `controls_if` as IF0/DO0. Catches a lowering that only reads IF/DO and emits `IF FALSE`.
    let text = r#"{
      "blocks": { "languageVersion": 0, "blocks": [
        { "id": "if-1", "type": "controls_if", "inputs": {
          "IF0": { "block": { "id": "bool-1", "type": "logic_boolean", "fields": { "BOOL": "TRUE" } } },
          "DO0": { "block": { "id": "set-inside", "type": "variables_set",
            "fields": { "VAR": { "id": "var-lamp", "name": "lamp" } },
            "inputs": { "VALUE": { "block": { "id": "bool-2", "type": "logic_boolean", "fields": { "BOOL": "TRUE" } } } } } } } }
      ] },
      "variables": [ { "id": "var-lamp", "name": "lamp", "type": "BOOL" } ],
      "metadata": { "name": "IfProgram" }
    }"#;
    assert_eq!(
        function_block(text, "if"),
        "FUNCTION_BLOCK FB_if_BLOCKLY\n\nVAR\n  lamp : BOOL;\nEND_VAR\n\n(* Generated from Blockly *)\n\nIF TRUE THEN\n  lamp := TRUE;\nEND_IF;\n\nEND_FUNCTION_BLOCK"
    );
}

#[test]
fn while_and_until_loops_are_pre_tested_while_statements() {
    // Ported from blockly-engine.test.ts. Blockly's UNTIL mode is tested before each pass, so it
    // is `WHILE NOT (…)`, in any case of the mode text. Catches a REPEAT lowering (one pass too
    // many) and an unknown comparison operator that is not `=`.
    let text = r#"{
      "blocks": { "languageVersion": 0, "blocks": [
        { "id": "loop-1", "type": "controls_whileUntil", "fields": { "MODE": "WHILE" }, "inputs": {
          "BOOL": { "block": { "id": "bool-1", "type": "logic_boolean", "fields": { "BOOL": "TRUE" } } },
          "DO": { "block": { "id": "set-1", "type": "variables_set",
            "fields": { "VAR": { "id": "var-count", "name": "count" } },
            "inputs": { "VALUE": { "block": { "id": "num-1", "type": "math_number", "fields": { "NUM": 1 } } } } } } } },
        { "id": "loop-2", "type": "controls_whileUntil", "fields": { "MODE": "until" }, "inputs": {
          "BOOL": { "block": { "id": "compare-1", "type": "logic_compare", "fields": { "OP": "GT" }, "inputs": {
            "A": { "block": { "id": "num-2", "type": "math_number", "fields": { "NUM": 3 } } },
            "B": { "block": { "id": "num-3", "type": "math_number", "fields": { "NUM": 2 } } } } } } } },
        { "id": "set-2", "type": "variables_set", "fields": { "VAR": "flag" }, "inputs": {
          "VALUE": { "block": { "id": "compare-2", "type": "logic_compare", "fields": { "OP": "XYZ" } } } } }
      ] },
      "variables": [ { "id": "var-count", "name": "count", "type": "INT" } ],
      "metadata": { "name": "LoopProgram" }
    }"#;
    let generated = generate(&workspace(text), "loop").expect("workspace");
    assert_eq!(
        generated.function_block,
        "FUNCTION_BLOCK FB_loop_BLOCKLY\n\nVAR\n  count : INT;\n  flag : BOOL;\nEND_VAR\n\n(* Generated from Blockly *)\n\nWHILE TRUE DO\n  count := 1;\nEND_WHILE;\nWHILE NOT ((3 > 2)) DO\nEND_WHILE;\nflag := (0 = 0);\n\nEND_FUNCTION_BLOCK"
    );
    assert_eq!(generated.warnings, Vec::<String>::new());
}

#[test]
fn variable_ids_resolve_to_their_names_and_untyped_numbers_become_int() {
    // Ported from blockly-engine.test.ts: a `VAR` field that holds only the id names the declared
    // variable, and an untyped variable assigned the number 0 is INT. The direct address is
    // lowered to its global (specification 03 §5). Catches `var-position` or `temp` in the code
    // and a missing INT.
    let text = r#"{
      "blocks": { "languageVersion": 0, "blocks": [
        { "id": "set-position", "type": "variables_set", "fields": { "VAR": { "id": "var-position" } },
          "inputs": { "VALUE": { "block": { "id": "num-zero", "type": "math_number", "fields": { "NUM": 0 } } } },
          "next": { "block": { "id": "write-position", "type": "io_digital_write", "fields": { "ADDRESS": "%QX0.0" },
            "inputs": { "VALUE": { "block": { "id": "compare-position", "type": "logic_compare", "fields": { "OP": "EQ" },
              "inputs": {
                "A": { "block": { "id": "get-position", "type": "variables_get", "fields": { "VAR": { "id": "var-position" } } } },
                "B": { "block": { "id": "num-one", "type": "math_number", "fields": { "NUM": 1 } } } } } } } } } }
      ] },
      "variables": [ { "id": "var-position", "name": "position", "type": "" } ],
      "metadata": { "name": "VariableIdProgram" }
    }"#;
    let generated = generate(&workspace(text), "ids").expect("workspace");
    assert_eq!(
        generated.function_block,
        "FUNCTION_BLOCK FB_ids_BLOCKLY\nVAR_EXTERNAL\n  ld_io_QX0_0 : BOOL;\nEND_VAR\n\n\nVAR\n  position : INT;\nEND_VAR\n\n(* Generated from Blockly *)\n\nposition := 0;\nld_io_QX0_0 := (position = 1);\n\nEND_FUNCTION_BLOCK"
    );
    assert_eq!(
        generated.globals,
        vec![Global {
            name: "ld_io_QX0_0".into(),
            address: Some("%QX0.0".into()),
            ty: "BOOL".into(),
            initial: None,
        }]
    );
}

#[test]
fn inference_follows_the_assigned_values() {
    // Worked from the inference rule: `1.5 + 2` has a REAL operand, so `a` is REAL; `b := a`
    // takes a's type, known by then; `c` is never assigned and is BOOL; `d` (typed DINT) keeps its
    // declared type. A field naming an unknown id becomes that id as an identifier, an empty
    // field `temp`. Catches arithmetic typed INT despite a REAL operand, a variable not typed
    // from another, and a declared type overwritten by inference.
    let text = r#"{
      "blocks": { "languageVersion": 0, "blocks": [
        { "id": "s1", "type": "variables_set", "fields": { "VAR": { "id": "v1" } },
          "inputs": { "VALUE": { "block": { "id": "m1", "type": "math_arithmetic", "inputs": {
            "A": { "block": { "id": "n1", "type": "math_number", "fields": { "NUM": 1.5 } } },
            "B": { "block": { "id": "n2", "type": "math_number", "fields": { "NUM": 2 } } } } } } },
          "next": { "block": { "id": "s2", "type": "variables_set", "fields": { "VAR": { "id": "v2" } },
            "inputs": { "VALUE": { "block": { "id": "g1", "type": "variables_get", "fields": { "VAR": { "id": "v1" } } } } },
            "next": { "block": { "id": "s3", "type": "variables_set", "fields": { "VAR": { "id": "v4" } },
              "inputs": { "VALUE": { "block": { "id": "n3", "type": "math_number", "fields": { "NUM": 7 } } } },
              "next": { "block": { "id": "s4", "type": "variables_set", "fields": { "VAR": { "id": "no such id" } },
                "next": { "block": { "id": "s5", "type": "variables_set", "fields": { "VAR": {} } } } } } } } } } }
      ] },
      "variables": [
        { "id": "v1", "name": "a", "type": "" },
        { "id": "v2", "name": "b" },
        { "id": "v3", "name": "c", "type": "" },
        { "id": "v4", "name": "d", "type": "DINT" }
      ]
    }"#;
    assert_eq!(
        function_block(text, "infer"),
        "FUNCTION_BLOCK FB_infer_BLOCKLY\n\nVAR\n  a : REAL;\n  b : REAL;\n  c : BOOL;\n  d : DINT;\nEND_VAR\n\n(* Generated from Blockly *)\n\na := (1.5 + 2);\nb := a;\nd := 7;\nno_such_id := 0;\ntemp := 0;\n\nEND_FUNCTION_BLOCK"
    );
}

#[test]
fn unknown_blocks_and_text_are_lowered_with_warnings_and_iec_escapes() {
    // An unknown block becomes a comment and a warning listed above the function block; text
    // uses the `$'` and `$$` escapes of specification 01 §9 (the earlier `''` doubling reads as
    // two strings in IEC). Catches a silently dropped block and the old escape.
    let text = r#"{
      "blocks": { "languageVersion": 0, "blocks": [
        { "id": "a", "type": "text_print" },
        { "id": "b", "type": "variables_set", "fields": { "VAR": "msg" },
          "inputs": { "VALUE": { "block": { "id": "c", "type": "text", "fields": { "TEXT": "Bob's $5" } } } } }
      ] },
      "variables": []
    }"#;
    let generated = generate(&workspace(text), "misc").expect("workspace");
    assert_eq!(generated.warnings, ["Unknown block type: text_print"]);
    assert_eq!(
        generated.function_block,
        "(*\n  Blockly conversion warnings (1):\n  - Unknown block type: text_print\n*)\n\nFUNCTION_BLOCK FB_misc_BLOCKLY\n\n(* Generated from Blockly *)\n\n(* Unknown block: text_print *)\nmsg := 'Bob$'s $$5';\n\nEND_FUNCTION_BLOCK"
    );
}

#[test]
fn a_chain_that_leads_back_to_itself_is_a_warning_not_a_crash() {
    // `a` continues with `b` by id and `b` with `a`. Each chain stops where it meets a block it
    // already passed, with a warning; the TypeScript inference recursed without end here.
    // Catches a converter that hangs or overflows its stack.
    let text = r#"{
      "blocks": { "languageVersion": 0, "blocks": [
        { "id": "a", "type": "variables_set", "fields": { "VAR": "x" }, "next": "b",
          "inputs": { "VALUE": { "block": { "id": "n", "type": "math_number", "fields": { "NUM": 1 } } } } },
        { "id": "b", "type": "variables_set", "fields": { "VAR": "y" }, "next": "a",
          "inputs": { "VALUE": { "block": { "id": "m", "type": "math_number", "fields": { "NUM": 2.5 } } } } }
      ] }
    }"#;
    let generated = generate(&workspace(text), "cycle").expect("workspace");
    assert_eq!(
        generated.warnings,
        [
            "Detected cycle while generating block chain at: a",
            "Detected cycle while generating block chain at: b"
        ]
    );
    assert!(generated.function_block.ends_with(
        "FUNCTION_BLOCK FB_cycle_BLOCKLY\n\nVAR\n  x : INT;\n  y : REAL;\nEND_VAR\n\n(* Generated from Blockly *)\n\nx := 1;\ny := 2.5;\ny := 2.5;\nx := 1;\n\nEND_FUNCTION_BLOCK"
    ));
}

#[test]
fn a_body_that_leads_back_into_its_if_block_stops_with_a_warning() {
    // The DO body of `top` continues by id with `top` itself, which the TypeScript lowered without
    // end. Catches unbounded recursion through nested statement bodies.
    let text = r#"{
      "blocks": { "languageVersion": 0, "blocks": [
        { "id": "top", "type": "controls_if", "inputs": {
          "DO0": { "block": { "id": "inner", "type": "variables_set", "fields": { "VAR": "x" }, "next": "top" } } } }
      ] }
    }"#;
    let generated = generate(&workspace(text), "loop").expect("workspace");
    assert_eq!(
        generated.warnings,
        ["Detected cycle while generating block chain at: top"]
    );
    assert!(generated
        .function_block
        .contains("IF FALSE THEN\n  x := 0;\nEND_IF;"));
}

#[test]
fn a_repeated_id_in_a_chain_ends_the_chain_as_before() {
    // The TypeScript chain tracked block ids, so a second block with the same id ends the chain
    // with the cycle warning; the move keeps that.
    let text = r#"{
      "blocks": { "languageVersion": 0, "blocks": [
        { "id": "x", "type": "variables_set", "fields": { "VAR": "p" },
          "next": { "block": { "id": "x", "type": "variables_set", "fields": { "VAR": "q" } } } }
      ] }
    }"#;
    let generated = generate(&workspace(text), "dup").expect("workspace");
    assert_eq!(
        generated.warnings,
        ["Detected cycle while generating block chain at: x"]
    );
    assert!(generated
        .function_block
        .contains("\np := 0;\n\nEND_FUNCTION_BLOCK"));
    assert!(!generated.function_block.contains("q :="));
}

#[test]
fn only_workspaces_with_a_block_list_are_blockly_programs() {
    // Catches a check that accepts `blocks.blocks` as an object, which the editor's own Save
    // once wrote, or a variables member that is not a list.
    assert!(check_format(&workspace(r#"{"blocks":{"blocks":[]}}"#)).is_ok());
    assert_eq!(
        check_format(&workspace(r#"{"blocks":{"blocks":{"languageVersion":0}}}"#)),
        Err("Invalid Blockly workspace format. Expected object with blocks.blocks[]".to_owned())
    );
    let problems = generate(
        &workspace(r#"{"blocks":{"blocks":[]},"variables":{"a":1}}"#),
        "x",
    )
    .expect_err("variables must be a list");
    assert_eq!(problems[0].message, "The Blockly variables must be a list.");
}

#[test]
fn the_function_block_is_named_after_the_file_then_the_metadata() {
    // Specification 17 §3: FB_<base>_BLOCKLY; without a base name the metadata name, then
    // `Blockly`.
    let text = r#"{"blocks":{"blocks":[]},"metadata":{"name":"my prog"}}"#;
    assert!(function_block(text, "").starts_with("FUNCTION_BLOCK FB_my_prog_BLOCKLY\n"));
    assert!(function_block(r#"{"blocks":{"blocks":[]}}"#, "")
        .starts_with("FUNCTION_BLOCK FB_Blockly_BLOCKLY\n"));
    assert!(function_block(text, "2-axis").starts_with("FUNCTION_BLOCK FB_2_axis_BLOCKLY\n"));
}

fn example(name: &str) -> String {
    let path = format!(
        "{}/../../examples/blockly/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"))
}

#[test]
fn the_snake_example_is_reproduced_as_the_typescript_wrote_it() {
    // The companion text the TypeScript generator wrote for examples/blockly; the runtime entry
    // equals the checked-in one. Catches any drift of the lowering, the layout or the runtime
    // entry.
    let source = example("snake-simple-v2.blockly.json");
    let files = companion("snake-simple-v2.blockly.json", &source).expect("companion");
    assert_eq!(files.companion_text, SNAKE_COMPANION);
    assert_eq!(
        files.runtime_entry_text,
        example("snake-simple-v2.visual.runtime.st")
    );
    assert_eq!(files.warnings, Vec::<String>::new());

    let files = companion(
        "test-io-write.blockly.json",
        &example("test-io-write.blockly.json"),
    )
    .expect("companion");
    assert_eq!(
        files.companion_text,
        "(*\n  Auto-generated from blockly source: test-io-write.blockly.json\n  Source of truth is the visual file unless migrated fully to ST.\n*)\nFUNCTION_BLOCK FB_test_io_write_BLOCKLY\nVAR_EXTERNAL\n  ld_io_QX0_0 : BOOL;\nEND_VAR\n\n\n(* Generated from Blockly *)\n\nld_io_QX0_0 := TRUE;\n\nEND_FUNCTION_BLOCK\n"
    );
}

#[test]
fn the_checked_in_snake_companion_is_the_generated_one() {
    // Specification 17 §3: saving a visual program writes `<base>.st` and
    // `<base>.visual.runtime.st` beside it, so both checked-in files of examples/blockly are
    // exactly what the generator writes. Catches a checked-in companion left from an older
    // generator (the earlier file had a blank line after the banner).
    let source = example("snake-simple-v2.blockly.json");
    let files = companion("snake-simple-v2.blockly.json", &source).expect("companion");
    assert_eq!(files.companion_text, example("snake-simple-v2.st"));
    assert_eq!(
        files.runtime_entry_text,
        example("snake-simple-v2.visual.runtime.st")
    );
}

const SNAKE_COMPANION: &str = "(*
  Auto-generated from blockly source: snake-simple-v2.blockly.json
  Source of truth is the visual file unless migrated fully to ST.
*)
FUNCTION_BLOCK FB_snake_simple_v2_BLOCKLY
VAR_EXTERNAL
  ld_io_QX0_0 : BOOL;
  ld_io_QX0_1 : BOOL;
  ld_io_QX0_2 : BOOL;
  ld_io_QX0_3 : BOOL;
  ld_io_QX0_4 : BOOL;
  ld_io_QX0_5 : BOOL;
  ld_io_QX0_6 : BOOL;
  ld_io_QX0_7 : BOOL;
END_VAR


VAR
  position : INT;
  direction : INT;
END_VAR

(* Generated from Blockly *)

position := 0;
direction := 1;
WHILE TRUE DO
  ld_io_QX0_0 := (position = 0);
  ld_io_QX0_1 := (position = 1);
  ld_io_QX0_2 := (position = 2);
  ld_io_QX0_3 := (position = 3);
  ld_io_QX0_4 := (position = 4);
  ld_io_QX0_5 := (position = 5);
  ld_io_QX0_6 := (position = 6);
  ld_io_QX0_7 := (position = 7);
  IF (direction = 1) THEN
    position := (position + 1);
    IF (position > 7) THEN
      position := 7;
      direction := -1;
    END_IF;
  END_IF;
  IF (direction = -1) THEN
    position := (position - 1);
    IF (position < 0) THEN
      position := 0;
      direction := 1;
    END_IF;
  END_IF;
END_WHILE;

END_FUNCTION_BLOCK
";
