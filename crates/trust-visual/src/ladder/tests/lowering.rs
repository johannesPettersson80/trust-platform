//! The Ladder companion (specification 16 §4 and §5): the expected texts are worked out by hand
//! from the lowering rules in `lower.rs`.

use super::json;
use crate::ladder::generate;
use crate::ladder::lower::initial_literal;
use crate::{Global, Json, Problem};

const HEADER: &str = "(*\n  Auto-generated Ladder Diagram companion (schema v2).\n  Source of truth remains the .ladder.json file.\n  Generated to support .st-first runtime workflows.\n*)\n\n";

fn program(variables: &str, networks: &str) -> Json {
    json(&format!(
        r#"{{"schemaVersion":2,"metadata":{{"name":"meta","description":""}},"variables":[{variables}],"networks":[{networks}]}}"#
    ))
}

fn declarations(globals: &[Global]) -> Vec<String> {
    globals.iter().map(Global::declaration).collect()
}

#[test]
fn series_contacts_and_the_four_coil_kinds() {
    // Specification 15 §4.2 and §6: series contacts AND, NC negates, SET/RESET latch while power
    // reaches them, NEGATED assigns the inverse. A raw output address becomes the global
    // `ld_io_QX0_1` declared AT it. Catches a chain in array order, a SET written as an
    // assignment, or a missing NOT.
    let program = program(
        r#"{"name":"Start","type":"BOOL","address":"%IX0.0"},
           {"name":"Stop","type":"BOOL","scope":"global","address":"%IX0.1"},
           {"name":"Motor","type":"BOOL","address":"%QX0.0"},
           {"name":"Lamp","type":"BOOL","scope":"local","initialValue":true},
           {"name":"Alarm","type":"BOOL","initialValue":"1"}"#,
        r#"{"id":"rung_2","order":1,"layout":{"y":200},"edges":[],"nodes":[
             {"id":"m","type":"contact","contactType":"NO","variable":"Motor","position":{"x":100,"y":200}},
             {"id":"q","type":"coil","coilType":"NEGATED","variable":"%QX0.1","position":{"x":500,"y":200}},
             {"id":"l","type":"coil","coilType":"SET","variable":"Lamp","position":{"x":300,"y":200}},
             {"id":"a","type":"coil","coilType":"RESET","variable":"Alarm","position":{"x":400,"y":200}}]},
           {"id":"rung_1","order":0,"layout":{"y":100},"edges":[],"nodes":[
             {"id":"s","type":"contact","contactType":"NO","variable":"Start","position":{"x":100,"y":100}},
             {"id":"t","type":"contact","contactType":"NC","variable":"Stop","position":{"x":200,"y":100}},
             {"id":"o","type":"coil","coilType":"NORMAL","variable":"Motor","position":{"x":300,"y":100}}]}"#,
    );
    let generated = generate(&program, "demo").expect("companion");
    assert_eq!(
        generated.function_block,
        format!(
            "{HEADER}FUNCTION_BLOCK FB_demo_LADDER\nVAR_EXTERNAL\n  Start : BOOL;\n  Stop : BOOL;\n  Motor : BOOL;\n  Alarm : BOOL;\n  ld_io_QX0_1 : BOOL;\nEND_VAR\n\nVAR\n  Lamp : BOOL := TRUE; (* Ladder local *)\nEND_VAR\n\n(* Network 0: rung_1 *)\nMotor := Start AND NOT (Stop);\n\n(* Network 1: rung_2 *)\nIF Motor THEN\n  Lamp := TRUE;\nEND_IF;\nIF Motor THEN\n  Alarm := FALSE;\nEND_IF;\nld_io_QX0_1 := NOT (Motor);\n\nEND_FUNCTION_BLOCK"
        )
    );
    assert_eq!(
        declarations(&generated.globals),
        [
            "  Start AT %IX0.0 : BOOL;",
            "  Stop AT %IX0.1 : BOOL;",
            "  Motor AT %QX0.0 : BOOL;",
            "  Alarm : BOOL := TRUE;",
            "  ld_io_QX0_1 AT %QX0.1 : BOOL;",
        ]
    );
    assert_eq!(generated.warnings, Vec::<String>::new());
}

#[test]
fn blocks_call_their_instances_and_division_is_plain() {
    // Timer: IN is the power (no own input), PT the preset rounded half up (1499.5 → 1500 ms).
    // Counter CTUD with its own input on a raw address, preset 9.5 → 10, outputs on raw
    // addresses; `%MW4` is a WORD (specification 03 §5), not the INT the editor once wrote. A
    // compare stores its result in `ld_compare_q_<id>`; DIV is `/` with no zero guard, so a zero
    // divisor faults as Structured Text does (specification 16 §4). Locals keep their file order
    // after the instances. Catches the old `IF (0) = 0 THEN Speed := 0` guard and INT words.
    let program = program(
        r#"{"name":"Go","type":"BOOL"},
           {"name":"Count","type":"INT","scope":"local","initialValue":"7.9"},
           {"name":"Speed","type":"REAL","scope":"local","initialValue":"1e3"},
           {"name":"Done","type":"BOOL","scope":"local"},
           {"name":"Elapsed","type":"TIME","scope":"local","initialValue":"T#2s"}"#,
        r#"{"id":"rung_1","order":0,"layout":{"y":100},"edges":[],"nodes":[
             {"id":"g","type":"contact","contactType":"NO","variable":"Go","position":{"x":100,"y":100}},
             {"id":"tm","type":"timer","timerType":"TON","instance":"T1","input":"","presetMs":1499.5,"qOutput":"Done","etOutput":"Elapsed","position":{"x":200,"y":100}}]},
           {"id":"rung_2","order":1,"layout":{"y":200},"edges":[],"nodes":[
             {"id":"cn","type":"counter","counterType":"CTUD","instance":"C1","input":"%IX0.2","preset":9.5,"qOutput":"%QX0.3","cvOutput":"%mw4","position":{"x":200,"y":200}}]},
           {"id":"rung_3","order":2,"layout":{"y":300},"edges":[],"nodes":[
             {"id":"cmp1","type":"compare","op":"LT","left":"Count","right":"10","position":{"x":100,"y":300}},
             {"id":"div1","type":"math","op":"DIV","left":"Count","right":"0","output":"Speed","position":{"x":300,"y":300}}]}"#,
    );
    let generated = generate(&program, "blocks").expect("companion");
    assert_eq!(
        generated.function_block,
        format!(
            "{HEADER}FUNCTION_BLOCK FB_blocks_LADDER\nVAR_EXTERNAL\n  Go : BOOL;\n  ld_io_IX0_2 : BOOL;\n  ld_io_QX0_3 : BOOL;\n  ld_io_MW4 : WORD;\nEND_VAR\n\nVAR\n  ld_timer_T1 : TON; (* T1 *)\n  ld_counter_C1 : CTUD; (* C1 *)\n  Count : INT := 7; (* Ladder local *)\n  Speed : REAL := 1000; (* Ladder local *)\n  Done : BOOL; (* Ladder local *)\n  Elapsed : TIME := T#2s; (* Ladder local *)\n  ld_compare_q_cmp1 : BOOL; (* Compare result for cmp1 *)\nEND_VAR\n\n(* Network 0: rung_1 *)\nld_timer_T1(IN := Go, PT := T#1500ms);\nDone := ld_timer_T1.Q;\nElapsed := ld_timer_T1.ET;\n\n(* Network 1: rung_2 *)\nld_counter_C1(CU := ld_io_IX0_2, CD := FALSE, R := FALSE, LD := FALSE, PV := 10);\nld_io_QX0_3 := ld_counter_C1.Q;\nld_io_MW4 := ld_counter_C1.CV;\n\n(* Network 2: rung_3 *)\nIF TRUE THEN\n  ld_compare_q_cmp1 := (Count < 10);\nEND_IF;\nIF TRUE THEN\n  Speed := (Count) / (0);\nEND_IF;\n\nEND_FUNCTION_BLOCK"
        )
    );
    assert_eq!(
        declarations(&generated.globals),
        [
            "  Go : BOOL;",
            "  ld_io_IX0_2 AT %IX0.2 : BOOL;",
            "  ld_io_QX0_3 AT %QX0.3 : BOOL;",
            "  ld_io_MW4 AT %MW4 : WORD;",
        ]
    );
}

#[test]
fn counters_without_own_input_count_the_power_in_each_form() {
    // CTU counts up with R FALSE, CTD counts down with LD FALSE (specification 16 §3); both take
    // the power reaching them. Catches CU and CD swapped.
    let program = program(
        r#"{"name":"In","type":"BOOL"},{"name":"Q1","type":"BOOL"},{"name":"V1","type":"INT"}"#,
        r#"{"id":"r","order":0,"layout":{"y":100},"edges":[],"nodes":[
             {"id":"i","type":"contact","contactType":"NO","variable":"In","position":{"x":100,"y":100}},
             {"id":"up","type":"counter","counterType":"CTU","instance":"U","preset":3,"qOutput":"Q1","cvOutput":"V1","position":{"x":200,"y":100}},
             {"id":"dn","type":"counter","counterType":"CTD","instance":"D","preset":-2,"qOutput":"Q1","cvOutput":"V1","position":{"x":400,"y":100}}]}"#,
    );
    let text = generate(&program, "c").expect("companion").function_block;
    assert!(text.contains("ld_counter_U(CU := In, R := FALSE, PV := 3);\nQ1 := ld_counter_U.Q;\nV1 := ld_counter_U.CV;\n"), "{text}");
    // A negative preset is 0; the second counter is reached through the first.
    assert!(
        text.contains("ld_counter_D(CD := In, LD := FALSE, PV := 0);\n"),
        "{text}"
    );
}

#[test]
fn raw_addresses_become_globals_and_leave_no_address_in_the_body() {
    // Specification 03 §5: each address a body names is one global of the size's type, declared
    // AT it in the runtime entry. Worked by hand: `%QX0.0 := %IX0.0 AND NOT %IX0.1`. Catches an
    // address left in the function block, where the companion would bind it a second time.
    let program = program(
        "",
        r#"{"id":"rung_1","order":0,"layout":{"y":100},"edges":[],"nodes":[
             {"id":"c1","type":"contact","contactType":"NO","variable":"%IX0.0","position":{"x":100,"y":100}},
             {"id":"c2","type":"contact","contactType":"NC","variable":"%IX0.1","position":{"x":200,"y":100}},
             {"id":"q1","type":"coil","coilType":"NORMAL","variable":"%QX0.0","position":{"x":300,"y":100}}]}"#,
    );
    let generated = generate(&program, "simple-start-stop").expect("companion");
    assert!(generated
        .function_block
        .contains("VAR_EXTERNAL\n  ld_io_IX0_0 : BOOL;\n  ld_io_IX0_1 : BOOL;\n  ld_io_QX0_0 : BOOL;\nEND_VAR\n"));
    assert!(generated
        .function_block
        .contains("\nld_io_QX0_0 := ld_io_IX0_0 AND NOT (ld_io_IX0_1);\n"));
    assert!(
        !generated.function_block.contains('%'),
        "{}",
        generated.function_block
    );
    assert_eq!(
        declarations(&generated.globals),
        [
            "  ld_io_IX0_0 AT %IX0.0 : BOOL;",
            "  ld_io_IX0_1 AT %IX0.1 : BOOL;",
            "  ld_io_QX0_0 AT %QX0.0 : BOOL;",
        ]
    );
}

#[test]
fn a_parallel_branch_feeds_its_coil_the_or_of_its_legs() {
    // Specification 15 §4.2: StartPB and the local dfg are parallel legs between a split and a
    // merge, so MotorRun is `StartPB OR dfg`; the next rung is the NC contact alone. Locals are
    // declared after the instances with their initial values; globals named in the externals.
    let program = program(
        r#"{"name":"StartPB","scope":"global","type":"BOOL","address":"%IX0.0"},
           {"name":"dfg","scope":"local","type":"BOOL","initialValue":false},
           {"name":"StopPB","scope":"global","type":"BOOL","address":"%IX0.1"},
           {"name":"MotorRun","scope":"global","type":"BOOL","address":"%QX0.0"},
           {"name":"AlarmLamp","scope":"global","type":"BOOL","address":"%QX0.1"}"#,
        r#"{"id":"rung_1","order":0,"layout":{"y":100},"nodes":[
             {"id":"split","type":"branchSplit","position":{"x":80,"y":100}},
             {"id":"c_start","type":"contact","contactType":"NO","variable":"StartPB","position":{"x":120,"y":100}},
             {"id":"c_dfg","type":"contact","contactType":"NO","variable":"dfg","position":{"x":120,"y":160}},
             {"id":"merge","type":"branchMerge","position":{"x":220,"y":100}},
             {"id":"q_motor","type":"coil","coilType":"NORMAL","variable":"MotorRun","position":{"x":320,"y":100}}],
           "edges":[
             {"id":"e1","fromNodeId":"split","toNodeId":"c_start"},
             {"id":"e2","fromNodeId":"split","toNodeId":"c_dfg"},
             {"id":"e3","fromNodeId":"c_start","toNodeId":"merge"},
             {"id":"e4","fromNodeId":"c_dfg","toNodeId":"merge"},
             {"id":"e5","fromNodeId":"merge","toNodeId":"q_motor"}]},
           {"id":"rung_2","order":1,"layout":{"y":240},"edges":[],"nodes":[
             {"id":"c_stop","type":"contact","contactType":"NC","variable":"StopPB","position":{"x":120,"y":240}},
             {"id":"q_alarm","type":"coil","coilType":"NORMAL","variable":"AlarmLamp","position":{"x":320,"y":240}}]}"#,
    );
    let text = generate(&program, "simple-start-stop")
        .expect("companion")
        .function_block;
    assert!(
        text.contains("\nVAR\n  dfg : BOOL := FALSE; (* Ladder local *)\nEND_VAR\n"),
        "{text}"
    );
    assert!(
        text.contains("\n(* Network 0: rung_1 *)\nMotorRun := StartPB OR dfg;\n"),
        "{text}"
    );
    assert!(
        text.contains("\n(* Network 1: rung_2 *)\nAlarmLamp := NOT (StopPB);\n"),
        "{text}"
    );
}

#[test]
fn qualified_symbols_resolve_to_their_scope() {
    // Specification 16 §5: `LOCAL::`, `GLOBAL::`, `LOCAL.` (any case, also `L::`/`G::`) name a
    // scope. Catches the old lowering that copied `local::Pump` into the Structured Text.
    let program = program(
        r#"{"name":"Pump","type":"BOOL","scope":"local"},{"name":"Valve","type":"BOOL"}"#,
        r#"{"id":"r","order":0,"layout":{"y":100},"edges":[],"nodes":[
             {"id":"a","type":"contact","contactType":"NO","variable":"local::Pump","position":{"x":100,"y":100}},
             {"id":"b","type":"contact","contactType":"NO","variable":"G::Valve","position":{"x":200,"y":100}},
             {"id":"q","type":"coil","coilType":"NORMAL","variable":"LOCAL.Pump","position":{"x":300,"y":100}}]}"#,
    );
    let text = generate(&program, "q").expect("companion").function_block;
    assert!(text.contains("\nPump := Pump AND Valve;\n"), "{text}");
}

#[test]
fn every_untranslatable_operand_is_reported_with_its_element() {
    // Worked by hand in the order contacts, then coils: an undeclared symbol, an address the
    // runtime refuses (bit 9 of a byte), a global hidden by a local of the same name, and a
    // local that does not exist. Catches a lowering that stops at the first problem or emits
    // `%IX0.9` for the runtime entry to refuse.
    let program = program(
        r#"{"name":"Pump","type":"BOOL","scope":"local"},{"name":"Pump","type":"BOOL"}"#,
        r#"{"id":"r","order":0,"layout":{"y":100},"edges":[],"nodes":[
             {"id":"c1","type":"contact","contactType":"NO","variable":"Ghost","position":{"x":100,"y":100}},
             {"id":"c2","type":"contact","contactType":"NO","variable":"%IX0.9","position":{"x":200,"y":100}},
             {"id":"q1","type":"coil","coilType":"NORMAL","variable":"GLOBAL::Pump","position":{"x":300,"y":100}},
             {"id":"q2","type":"coil","coilType":"NORMAL","variable":"LOCAL::Nope","position":{"x":400,"y":100}}]}"#,
    );
    assert_eq!(
        generate(&program, "p").expect_err("untranslatable"),
        vec![
            Problem::at("c1", "Undeclared ladder symbol 'Ghost' referenced by contact 'c1'. Declare it in variables[] or use a direct address."),
            Problem::at("c2", "'%IX0.9' is not a valid direct address."),
            Problem::at("q1", "'GLOBAL::Pump' cannot be reached: the local variable 'Pump' hides the global."),
            Problem::at("q2", "'LOCAL::Nope' names no local variable 'Nope'."),
        ]
    );
}

#[test]
fn a_block_on_the_left_rail_gets_constant_power() {
    // A timer first on its rung is reached by the rail: IN := TRUE. The old lowering refused it
    // as "Undeclared ladder symbol 'TRUE'", reading the literal as a symbol.
    let program = program(
        r#"{"name":"Q","type":"BOOL"},{"name":"E","type":"TIME"}"#,
        r#"{"id":"r","order":0,"layout":{"y":100},"edges":[],"nodes":[
             {"id":"t","type":"timer","timerType":"TOF","instance":"Off","presetMs":250,"qOutput":"Q","etOutput":"E","position":{"x":100,"y":100}}]}"#,
    );
    let text = generate(&program, "rail")
        .expect("companion")
        .function_block;
    assert!(text.contains("\nld_timer_Off(IN := TRUE, PT := T#250ms);\nQ := ld_timer_Off.Q;\nE := ld_timer_Off.ET;\n"), "{text}");
}

#[test]
fn an_output_that_cannot_be_written_is_reported() {
    // A timer's ET output `a b` is no variable. The old lowering made a hidden local for it and
    // then refused that local as undeclared; the message now names the output itself.
    let program = program(
        r#"{"name":"Q","type":"BOOL"}"#,
        r#"{"id":"r","order":0,"layout":{"y":100},"edges":[],"nodes":[
             {"id":"t","type":"timer","timerType":"TP","instance":"P","presetMs":5,"qOutput":"Q","etOutput":"a b","position":{"x":100,"y":100}}]}"#,
    );
    assert_eq!(
        generate(&program, "t").expect_err("refused"),
        vec![Problem::at(
            "t",
            "timer 't' ET output 'a b' is not a variable or direct address that can be written."
        )]
    );
}

#[test]
fn a_coil_without_target_is_skipped_with_a_warning() {
    // The companion still compiles: the coil becomes a comment and the editor is told.
    let program = program(
        "",
        r#"{"id":"r","order":0,"layout":{"y":100},"edges":[],"nodes":[
             {"id":"c","type":"contact","contactType":"NO","variable":"%IX0.0","position":{"x":100,"y":100}},
             {"id":"q","type":"coil","coilType":"NORMAL","variable":"","position":{"x":200,"y":100}}]}"#,
    );
    let generated = generate(&program, "s").expect("companion");
    assert!(generated
        .function_block
        .contains("\n(* Skipped coil 'q' due to non-assignable target:  *)\n"));
    assert_eq!(
        generated.warnings,
        ["Coil 'q' is skipped: '' cannot be written."]
    );
}

#[test]
fn a_global_named_like_an_address_alias_moves_the_alias_aside() {
    // A declared global already called `ld_io_IX0_0` keeps its name; the alias of `%IX0.0`
    // becomes `ld_io_IX0_0_2`. Catches two declarations of one name.
    let program = program(
        r#"{"name":"ld_io_IX0_0","type":"BOOL"}"#,
        r#"{"id":"r","order":0,"layout":{"y":100},"edges":[],"nodes":[
             {"id":"c","type":"contact","contactType":"NO","variable":"%ix0.0","position":{"x":100,"y":100}},
             {"id":"q","type":"coil","coilType":"NORMAL","variable":"ld_io_IX0_0","position":{"x":200,"y":100}}]}"#,
    );
    let generated = generate(&program, "a").expect("companion");
    assert!(generated
        .function_block
        .contains("\nld_io_IX0_0 := ld_io_IX0_0_2;\n"));
    assert_eq!(
        declarations(&generated.globals),
        ["  ld_io_IX0_0 : BOOL;", "  ld_io_IX0_0_2 AT %IX0.0 : BOOL;"]
    );
}

#[test]
fn initial_values_become_literals_of_their_type() {
    // Worked from the rules: BOOL from a flag, a number (non-zero is TRUE) or TRUE/FALSE/1/0
    // text; INT/DINT truncated; REAL/LREAL as written by JavaScript; TIME text as written or a
    // number of milliseconds (negative is 0). Catches rounding instead of truncation and Rust's
    // own float format (`1000.0`).
    let literal = |ty: &str, value: &str| initial_literal(ty, Some(&json(value)));
    assert_eq!(literal("BOOL", "true").as_deref(), Some("TRUE"));
    assert_eq!(literal("BOOL", "0").as_deref(), Some("FALSE"));
    assert_eq!(literal("bool", "\" 1 \"").as_deref(), Some("TRUE"));
    assert_eq!(literal("BOOL", "\"yes\""), None);
    assert_eq!(literal("INT", "12.9").as_deref(), Some("12"));
    assert_eq!(literal("DINT", "\"-3.7\"").as_deref(), Some("-3"));
    assert_eq!(literal("INT", "\"0x10\"").as_deref(), Some("16"));
    assert_eq!(literal("INT", "\"abc\""), None);
    assert_eq!(literal("REAL", "0.1").as_deref(), Some("0.1"));
    assert_eq!(literal("LREAL", "\"1e3\"").as_deref(), Some("1000"));
    assert_eq!(literal("TIME", "1500").as_deref(), Some("T#1500ms"));
    assert_eq!(literal("TIME", "-5").as_deref(), Some("T#0ms"));
    assert_eq!(literal("TIME", "\" T#2s \"").as_deref(), Some("T#2s"));
    assert_eq!(literal("TIME", "\"\""), None);
    assert_eq!(literal("WORD", "1"), None);
    assert_eq!(initial_literal("BOOL", None), None);
}

#[test]
fn the_checked_in_ethercat_example_is_reproduced_exactly() {
    // `examples/ladder/ethercat-snake.st` and `.visual.runtime.st` were written by the
    // TypeScript generator; no decision of the move touches a series-only bit program. Catches
    // any drift of the lowering or the runtime entry.
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/ladder/");
    let read = |name: &str| std::fs::read_to_string(format!("{root}{name}")).expect(name);
    let files = crate::companion::companion(
        "ethercat-snake.ladder.json",
        &read("ethercat-snake.ladder.json"),
    )
    .expect("companion");
    assert_eq!(files.companion_text, read("ethercat-snake.st"));
    assert_eq!(
        files.runtime_entry_text,
        read("ethercat-snake.visual.runtime.st")
    );
}
