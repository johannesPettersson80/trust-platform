//! Tests of the SFC companion (specification 17 §3, specification 25 §11.4, `sfc-profile.md`,
//! "Execution of a visual SFC"). Every expected text is worked out by hand from the rules of
//! `sfc-profile.md`; the companion executed on the runtime is tested in `trust-lsp`
//! (`visual_sfc_run_tests.rs`).

use super::*;
use crate::companion::companion;
use crate::Global;

fn program(text: &str) -> Json {
    Json::parse(text).expect("test program is JSON")
}

fn function_block(text: &str, base: &str) -> String {
    generate(&program(text), base)
        .unwrap_or_else(|problems| panic!("a companion: {problems:?}"))
        .function_block
}

const DEMO: &str = r#"{
  "name": "SFC_Demo",
  "steps": [
    { "id": "step_init", "name": "Init", "initial": true, "x": 250, "y": 50, "actions": [] },
    { "id": "step_run", "name": "Run", "x": 250, "y": 200,
      "actions": [ { "id": "act_run", "name": "SetOutput", "qualifier": "N", "body": "%QX0.0 := TRUE;" } ] }
  ],
  "transitions": [
    { "id": "t1", "name": "T1", "condition": "TRUE", "sourceStepId": "step_init", "targetStepId": "step_run" }
  ]
}"#;

#[test]
fn a_two_step_chart_runs_its_action_with_final_scan_and_evolves() {
    // Worked by hand from sfc-profile.md: step flags (TRUE for Init), each step's structure (X
    // TRUE for Init) and timer, the flag structure of SetOutput, one transition flag; each active
    // step's T from its timer; SetOutput's control (A is Q of the previous scan OR Q; Q is
    // Run's flag, its N input) and statements under A; the transition tested against the step
    // flags (TRUE adds nothing); the evolution leaves Init and enters Run; each step's X is its
    // flag; the TYPEs of the step and the flag structures after the block; %QX0.0 lowered to a
    // global. Catches the old companion (an IF per
    // step that ran an action only while its step was active, and moved steps one transition at a
    // time), a missing final scan, and an address left in the body.
    let generated = generate(&program(DEMO), "sfc-demo").expect("companion");
    assert_eq!(
        generated.function_block,
        "FUNCTION_BLOCK FB_sfc_demo_SFC\n\
         VAR_EXTERNAL\n  ld_io_QX0_0 : BOOL;\nEND_VAR\n\n\
         VAR\n\
         \x20 // Steps: TRUE while the step is active (IEC 61131-3 6.7.2)\n\
         \x20 Init_active : BOOL := TRUE;\n\
         \x20 Run_active : BOOL := FALSE;\n\
         \x20 // Steps as Table 54 names them: X the step flag, T the elapsed time, from the timer\n\
         \x20 Init : FB_sfc_demo_SFC_STEP := (X := TRUE);\n\
         \x20 SFC_S1 : TON;\n\
         \x20 Run : FB_sfc_demo_SFC_STEP;\n\
         \x20 SFC_S2 : TON;\n\
         \x20 // Actions: the flags Q and A and the action control (6.7.4.6)\n\
         \x20 SetOutput : FB_sfc_demo_SFC_ACTION;\n\
         \x20 // Transitions: TRUE after a scan in which the transition cleared (6.7.5)\n\
         \x20 SFC_T1 : BOOL;\n\
         END_VAR\n\n\
         // Step elapsed times (IEC 61131-3 6.7.2, Table 54): T counts while the step is active,\n\
         // starts again from T#0s when the step is entered, and keeps its value while it is inactive.\n\
         SFC_S1(IN := Init_active, PT := T#24d20h31m23s647ms);\n\
         IF Init_active THEN\n  Init.T := SFC_S1.ET;\nEND_IF;\n\
         SFC_S2(IN := Run_active, PT := T#24d20h31m23s647ms);\n\
         IF Run_active THEN\n  Run.T := SFC_S2.ET;\nEND_IF;\n\n\
         // Actions with final scan (IEC 61131-3 6.7.4.6, Table 60 feature 1): A is TRUE while Q is\n\
         // and in the scan after Q falls, and an action's statements run while its A is TRUE.\n\n\
         // Action SetOutput: N Run\n\
         SetOutput.A := SetOutput.Q; // Q of the previous scan\n\
         SetOutput.Q := Run_active;\n\
         SetOutput.A := SetOutput.A OR SetOutput.Q;\n\
         IF SetOutput.A THEN\n  ld_io_QX0_0 := TRUE;\nEND_IF;\n\n\
         // Transitions (IEC 61131-3 6.7.5) in priority order, tested against the steps active before\n\
         // the evolution; one that shares a step with a transition before it clears only if that one does not.\n\n\
         // SFC_T1: Init -> Run (T1)\n\
         SFC_T1 := Init_active;\n\n\
         // Evolution: the steps before each clearing transition are left, then the steps after it entered.\n\
         Init_active := Init_active AND NOT SFC_T1;\n\
         Run_active := Run_active OR SFC_T1;\n\n\
         // Step flags X (Table 54): each step's <step>_active after the evolution.\n\
         Init.X := Init_active;\n\
         Run.X := Run_active;\n\n\
         END_FUNCTION_BLOCK\n\n\
         TYPE FB_sfc_demo_SFC_STEP :\n\
         STRUCT\n\
         \x20 X : BOOL; // the step flag: TRUE while the step is active\n\
         \x20 T : TIME; // the step elapsed time: since the step was entered, kept while it is inactive\n\
         END_STRUCT;\n\
         END_TYPE\n\n\
         TYPE FB_sfc_demo_SFC_ACTION :\n\
         STRUCT\n\
         \x20 Q : BOOL; // the action flag\n\
         \x20 A : BOOL; // the activation: the action's statements run while A is TRUE\n\
         END_STRUCT;\n\
         END_TYPE"
    );
    assert_eq!(
        generated.globals,
        vec![Global {
            name: "ld_io_QX0_0".to_owned(),
            address: Some("%QX0.0".to_owned()),
            ty: "BOOL".to_owned(),
            initial: None,
        }]
    );
    assert_eq!(generated.warnings, Vec::<String>::new());
}

#[test]
fn variables_keep_a_present_initial_value_and_a_safe_comment() {
    // `initialValue` is written when it is present and not empty or zero ("0" is present, the
    // number 0 is not); a comment cannot be broken onto a second line. A chart without actions or
    // transitions has only its steps: their flags, structures, timers and elapsed times. Catches a generator that drops "0", writes ":= 0" for a
    // numeric zero, or lets a newline end the `//` comment.
    let text = r#"{
      "name": "Vars",
      "steps": [ { "id": "s", "name": "S", "initial": true } ],
      "transitions": [],
      "variables": [
        { "name": "count", "type": "INT", "initialValue": "0", "comment": "a (* tricky *)\nnote" },
        { "name": "flag", "type": "BOOL", "initialValue": 0 },
        { "name": "t", "type": "TIME", "initialValue": "" }
      ]
    }"#;
    assert_eq!(
        function_block(text, "vars"),
        "FUNCTION_BLOCK FB_vars_SFC\n\
         VAR\n  count : INT := 0; // a ( * tricky * ) note\n  flag : BOOL;\n  t : TIME;\nEND_VAR\n\n\
         VAR\n\
         \x20 // Steps: TRUE while the step is active (IEC 61131-3 6.7.2)\n\
         \x20 S_active : BOOL := TRUE;\n\
         \x20 // Steps as Table 54 names them: X the step flag, T the elapsed time, from the timer\n\
         \x20 S : FB_vars_SFC_STEP := (X := TRUE);\n\
         \x20 SFC_S1 : TON;\n\
         END_VAR\n\n\
         // Step elapsed times (IEC 61131-3 6.7.2, Table 54): T counts while the step is active,\n\
         // starts again from T#0s when the step is entered, and keeps its value while it is inactive.\n\
         SFC_S1(IN := S_active, PT := T#24d20h31m23s647ms);\n\
         IF S_active THEN\n  S.T := SFC_S1.ET;\nEND_IF;\n\n\
         // Step flags X (Table 54): each step's <step>_active after the evolution.\n\
         S.X := S_active;\n\n\
         END_FUNCTION_BLOCK\n\n\
         TYPE FB_vars_SFC_STEP :\n\
         STRUCT\n\
         \x20 X : BOOL; // the step flag: TRUE while the step is active\n\
         \x20 T : TIME; // the step elapsed time: since the step was entered, kept while it is inactive\n\
         END_STRUCT;\n\
         END_TYPE"
    );
}

#[test]
fn a_parallel_split_and_join_activate_and_leave_all_branches_together() {
    // Table 61 4a and 4c: one transition leaves Start and enters A and B together; one leaves A
    // and B together, only when both are active and A's condition holds (the join's "1" adds
    // nothing), and enters Done. Catches the old companion, which left out every transition at a
    // split or join, so no chart with parallel branches ever left its first step.
    let text = r#"{
      "name": "P",
      "steps": [
        { "id": "s", "name": "Start", "initial": true }, { "id": "a", "name": "A" },
        { "id": "b", "name": "B" }, { "id": "d", "name": "Done" }
      ],
      "transitions": [
        { "id": "e0", "condition": "go", "sourceStepId": "s", "targetStepId": "sp" },
        { "id": "e1", "condition": "TRUE", "sourceStepId": "sp", "targetStepId": "a" },
        { "id": "e2", "condition": "TRUE", "sourceStepId": "sp", "targetStepId": "b" },
        { "id": "e3", "condition": "a_done", "sourceStepId": "a", "targetStepId": "jn" },
        { "id": "e4", "condition": "TRUE", "sourceStepId": "b", "targetStepId": "jn" },
        { "id": "e5", "condition": "1", "sourceStepId": "jn", "targetStepId": "d" }
      ],
      "parallelSplits": [ { "id": "sp", "name": "Split", "branchIds": ["a", "b"] } ],
      "parallelJoins": [ { "id": "jn", "name": "Join", "branchIds": ["a", "b"], "nextStepId": "d" } ]
    }"#;
    let text = function_block(text, "p");
    assert!(
        text.contains(
            "// SFC_T1: Start -> A, B\n\
             SFC_T1 := Start_active AND (go);\n\n\
             // SFC_T2: A, B -> Done\n\
             SFC_T2 := A_active AND B_active AND (a_done);\n\n\
             // Evolution: the steps before each clearing transition are left, then the steps after it entered.\n\
             Start_active := Start_active AND NOT SFC_T1;\n\
             A_active := (A_active AND NOT SFC_T2) OR SFC_T1;\n\
             B_active := (B_active AND NOT SFC_T2) OR SFC_T1;\n\
             Done_active := Done_active OR SFC_T2;\n\n\
             // Step flags X (Table 54): each step's <step>_active after the evolution.\n\
             Start.X := Start_active;\n\
             A.X := A_active;\n\
             B.X := B_active;\n\
             Done.X := Done_active;\n\n\
             END_FUNCTION_BLOCK"
        ),
        "{text}"
    );
}

#[test]
fn a_step_named_with_a_keyword_has_only_its_flag() {
    // An IEC 61131-3 keyword is no variable name: the step On (ON is the RESOURCE separator) keeps
    // On_active but is no variable `On`, while Run, the second step, is the variable Run with the
    // timer SFC_S2 of its position. Catches a companion that declares `On : FB_k_SFC_STEP`, which
    // the IEC rules refuse to compile, and timers numbered by the readable steps only.
    let text = r#"{
      "name": "K",
      "steps": [ { "id": "on", "name": "On", "initial": true }, { "id": "run", "name": "Run" } ],
      "transitions": [ { "id": "t1", "condition": "go", "sourceStepId": "on", "targetStepId": "run" } ]
    }"#;
    let text = function_block(text, "k");
    assert!(text.contains("  On_active : BOOL := TRUE;\n"), "{text}");
    assert!(
        text.contains("  Run : FB_k_SFC_STEP;\n  SFC_S2 : TON;\n"),
        "{text}"
    );
    assert!(text.contains("Run.X := Run_active;"), "{text}");
    assert!(
        !text.contains("On : ") && !text.contains("On.X") && !text.contains("SFC_S1"),
        "{text}"
    );
}

#[test]
fn a_selection_tests_its_transitions_in_priority_order() {
    // Table 61 2a and 2b: S -> B (priority 1) first, then S -> A and S -> C in file order; each
    // yields to the ones before it that leave S. All three leave S; each enters its own step.
    // Catches a selection that clears two branches at once and the old file-order-only rule.
    let text = r#"{
      "name": "Sel",
      "steps": [
        { "id": "s", "name": "S", "initial": true }, { "id": "a", "name": "A" },
        { "id": "b", "name": "B" }, { "id": "c", "name": "C" }
      ],
      "transitions": [
        { "id": "e0", "condition": "a", "sourceStepId": "s", "targetStepId": "a" },
        { "id": "e1", "name": "Urgent", "condition": "b", "sourceStepId": "s", "targetStepId": "b", "priority": 1 },
        { "id": "e2", "condition": "c", "sourceStepId": "s", "targetStepId": "c" }
      ]
    }"#;
    let text = function_block(text, "sel");
    assert!(
        text.contains(
            "// SFC_T1: S -> B (Urgent, priority 1)\n\
             SFC_T1 := S_active AND (b);\n\n\
             // SFC_T2: S -> A\n\
             SFC_T2 := S_active AND (a) AND NOT SFC_T1;\n\n\
             // SFC_T3: S -> C\n\
             SFC_T3 := S_active AND (c) AND NOT (SFC_T1 OR SFC_T2);\n\n\
             // Evolution: the steps before each clearing transition are left, then the steps after it entered.\n\
             S_active := S_active AND NOT (SFC_T1 OR SFC_T2 OR SFC_T3);\n\
             A_active := A_active OR SFC_T2;\n\
             B_active := B_active OR SFC_T1;\n\
             C_active := C_active OR SFC_T3;\n"
        ),
        "{text}"
    );
}

/// Steps S1 to S5 in a row; Motor is set in S1, time limited in S2 and reset in S3; Valve has the
/// other qualifiers; Lamp has two time-limited associations of different durations.
const QUALIFIERS: &str = r#"{
  "name": "Q",
  "steps": [
    { "id": "s1", "name": "S1", "initial": true, "actions": [
      { "id": "m1", "name": "Motor", "qualifier": "S", "body": "m := Motor.Q;" },
      { "id": "v1", "name": "Valve", "qualifier": "D", "time": "T#1s", "body": "v := TRUE;" },
      { "id": "l1", "name": "Lamp", "qualifier": "L", "time": "T#1s", "body": "l := Lamp.Q;" }
    ] },
    { "id": "s2", "name": "S2", "actions": [
      { "id": "m2", "name": "Motor", "qualifier": "L", "time": "T#5s" },
      { "id": "v2", "name": "Valve", "qualifier": "P" },
      { "id": "v3", "name": "Valve", "qualifier": "DS", "time": "T#3s" },
      { "id": "l2", "name": "Lamp", "qualifier": "L", "time": "T#2s" }
    ] },
    { "id": "s3", "name": "S3", "actions": [
      { "id": "m3", "name": "Motor", "qualifier": "R" },
      { "id": "v4", "name": "Valve", "qualifier": "SD", "time": "T#2s" },
      { "id": "v5", "name": "Valve", "qualifier": "P1" }
    ] },
    { "id": "s4", "name": "S4", "actions": [
      { "id": "v6", "name": "Valve", "qualifier": "N" },
      { "id": "v7", "name": "Valve", "qualifier": "SL", "time": "T#4s" },
      { "id": "v8", "name": "Valve", "qualifier": "P0" }
    ] },
    { "id": "s5", "name": "S5", "actions": [ { "id": "v9", "name": "Valve", "qualifier": "R" } ] }
  ],
  "transitions": [
    { "id": "t1", "condition": "x1", "sourceStepId": "s1", "targetStepId": "s2" },
    { "id": "t2", "condition": "x2", "sourceStepId": "s2", "targetStepId": "s3" },
    { "id": "t3", "condition": "x3", "sourceStepId": "s3", "targetStepId": "s4" },
    { "id": "t4", "condition": "x4", "sourceStepId": "s4", "targetStepId": "s5" }
  ]
}"#;

#[test]
fn the_action_control_is_figure_23_a_with_only_the_parts_used() {
    // Figure 23 a): S_FF = RS(S, R), L_TMR = TON(L, T), and Q = NOT R AND (S_FF.Q1 OR (L AND NOT
    // L_TMR.Q)); A = Q of the previous scan OR Q. Only S_FF and L_TMR are declared. Catches a
    // reset that does not override, a time limit that is not ANDed with its input, and a control
    // without final scan.
    let text = function_block(QUALIFIERS, "q");
    assert!(
        text.contains(
            "// Action Motor: S S1, L T#5s S2, R S3\n\
             Motor_S_FF(S := S1_active, R1 := S3_active);\n\
             Motor_L_TMR(IN := S2_active, PT := T#5s);\n\
             Motor.A := Motor.Q; // Q of the previous scan\n\
             Motor.Q := NOT S3_active AND (Motor_S_FF.Q1 OR (S2_active AND NOT Motor_L_TMR.Q));\n\
             Motor.A := Motor.A OR Motor.Q;\n\
             IF Motor.A THEN\n  m := Motor.Q;\nEND_IF;\n"
        ),
        "{text}"
    );
    assert!(text.contains(
        "  Motor : FB_q_SFC_ACTION;\n  Motor_S_FF : RS;\n  Motor_L_TMR : TON;\n  Valve : FB_q_SFC_ACTION;"
    ));
}

#[test]
fn every_qualifier_of_table_59_has_its_part_of_figure_23_a() {
    // Valve: D in S1, P and DS in S2, SD and P1 in S3, N, SL and P0 in S4, R in S5. Worked from
    // Figure 23 a): D_TMR = TON(D); P_TRIG = R_TRIG(P); SD_FF = RS(SD, R) timed by SD_TMR; DS_TMR
    // = TON(DS) setting DS_FF = RS(DS_TMR.Q, R); SL_FF = RS(SL, R) timed by SL_TMR;
    // Q = NOT R AND (N OR D_TMR.Q OR P_TRIG.Q OR SD_TMR.Q OR DS_FF.Q1 OR (SL_FF.Q1 AND NOT
    // SL_TMR.Q)); A adds P1_TRIG.Q and the falling edge of P0 (P0_M AND NOT P0), and P0_M keeps
    // P0 for the next scan; the statements written in S1 run while A. Catches any qualifier read
    // as N (the old companion), a P1 or P0 that sets Q, and a P0 edge without memory.
    let text = function_block(QUALIFIERS, "q");
    assert!(
        text.contains(
            "// Action Valve: D T#1s S1, P S2, DS T#3s S2, SD T#2s S3, P1 S3, N S4, SL T#4s S4, P0 S4, R S5\n\
             Valve_D_TMR(IN := S1_active, PT := T#1s);\n\
             Valve_P_TRIG(CLK := S2_active);\n\
             Valve_SD_FF(S := S3_active, R1 := S5_active);\n\
             Valve_SD_TMR(IN := Valve_SD_FF.Q1, PT := T#2s);\n\
             Valve_DS_TMR(IN := S2_active, PT := T#3s);\n\
             Valve_DS_FF(S := Valve_DS_TMR.Q, R1 := S5_active);\n\
             Valve_SL_FF(S := S4_active, R1 := S5_active);\n\
             Valve_SL_TMR(IN := Valve_SL_FF.Q1, PT := T#4s);\n\
             Valve.A := Valve.Q; // Q of the previous scan\n\
             Valve.Q := NOT S5_active AND (S4_active OR Valve_D_TMR.Q OR Valve_P_TRIG.Q OR Valve_SD_TMR.Q OR Valve_DS_FF.Q1 OR (Valve_SL_FF.Q1 AND NOT Valve_SL_TMR.Q));\n\
             Valve_P1_TRIG(CLK := S3_active);\n\
             Valve.A := Valve.A OR Valve.Q OR Valve_P1_TRIG.Q OR (Valve_P0_M AND NOT S4_active);\n\
             Valve_P0_M := S4_active;\n\
             IF Valve.A THEN\n  v := TRUE;\nEND_IF;\n\n"
        ),
        "{text}"
    );
    assert!(text.contains(
        "  Valve : FB_q_SFC_ACTION;\n  Valve_D_TMR : TON;\n  Valve_P_TRIG : R_TRIG;\n  \
         Valve_SD_FF : RS;\n  Valve_SD_TMR : TON;\n  Valve_DS_TMR : TON;\n  Valve_DS_FF : RS;\n  \
         Valve_SL_FF : RS;\n  Valve_SL_TMR : TON;\n  Valve_P1_TRIG : R_TRIG;\n  Valve_P0_M : BOOL;\n"
    ));
}

#[test]
fn a_timer_with_different_durations_uses_the_active_association() {
    // Lamp is limited to T#1s in S1 and T#2s in S2: Lamp_L_PT starts at the first duration, takes
    // the duration of the first active association and keeps it while none is active; the timer's
    // input is S1 OR S2. Catches one duration for all associations of a qualifier.
    let text = function_block(QUALIFIERS, "q");
    assert!(
        text.contains(
            "IF S1_active THEN\n  Lamp_L_PT := T#1s;\nELSIF S2_active THEN\n  Lamp_L_PT := T#2s;\nEND_IF;\n\
             Lamp_L_TMR(IN := (S1_active OR S2_active), PT := Lamp_L_PT);\n"
        ),
        "{text}"
    );
    assert!(text.contains("  Lamp_L_TMR : TON;\n  Lamp_L_PT : TIME := T#1s;\n"));
}

#[test]
fn a_chart_that_validate_refuses_has_no_companion() {
    // An L action without a duration: the companion is refused with Validate's finding, at the
    // step. Catches a companion written with `PT := ` and nothing after it.
    let text = r#"{ "name": "R", "steps": [ { "id": "s", "name": "S", "initial": true, "actions": [
      { "id": "a", "name": "Lamp", "qualifier": "L", "body": "x := TRUE;" } ] } ], "transitions": [] }"#;
    let problems = generate(&program(text), "r").expect_err("refused");
    assert_eq!(
        problems,
        [Problem::at(
            "s",
            "Action Lamp in step S: qualifier L needs a duration such as T#2s."
        )]
    );
}

#[test]
fn a_crlf_body_gets_lf_lines() {
    // An action body saved with CRLF line ends becomes LF lines. Catches a companion with stray
    // carriage returns.
    let text = r#"{ "name": "C", "steps": [ { "id": "s", "name": "S", "initial": true,
      "actions": [ { "id": "a", "name": "Two", "qualifier": "N", "body": "a := 1;\r\nb := 2;" } ] } ],
      "transitions": [] }"#;
    let text = function_block(text, "c");
    assert!(text.contains("IF Two.A THEN\n  a := 1;\n  b := 2;\nEND_IF;"));
    assert!(!text.contains('\r'));
}

#[test]
fn the_block_is_named_after_the_file_then_the_program() {
    // FB_<base>_SFC; without a base the trimmed program name, and without that "SFC". Catches a
    // name taken from the program when the file has one (two files would clash).
    let named = |name: &str, base: &str| {
        let text = format!(
            "{{\"name\": \"{name}\", \"steps\": [{{\"id\": \"s\", \"name\": \"S\", \"initial\": true}}], \"transitions\": []}}"
        );
        function_block(&text, base)
            .lines()
            .next()
            .expect("header")
            .to_owned()
    };
    assert_eq!(named("Other", "my-chart"), "FUNCTION_BLOCK FB_my_chart_SFC");
    assert_eq!(named("  Demo  ", ""), "FUNCTION_BLOCK FB_Demo_SFC");
    assert_eq!(named("   ", ""), "FUNCTION_BLOCK FB_SFC_SFC");
}

#[test]
fn a_word_address_is_a_word_global() {
    // Specification 03 §5: the size letter W is WORD. Catches the old INT typing of %MW.
    let text = r#"{
      "name": "W",
      "steps": [ { "id": "s", "name": "S", "initial": true,
        "actions": [ { "id": "a", "name": "Count", "qualifier": "N", "body": "%MW2 := WORD#5;" } ] } ],
      "transitions": []
    }"#;
    let generated = generate(&program(text), "w").expect("companion");
    assert!(generated
        .function_block
        .contains("VAR_EXTERNAL\n  ld_io_MW2 : WORD;\nEND_VAR"));
    assert_eq!(generated.globals[0].ty, "WORD");
}

#[test]
fn a_file_that_is_no_sfc_has_no_companion() {
    // The name, steps and transitions are required; the message is the one users saw before.
    for text in [
        r#"{"steps": [], "transitions": []}"#,
        r#"{"name": "x", "steps": {}, "transitions": []}"#,
        r#"{"name": "x", "steps": []}"#,
    ] {
        let problems = generate(&program(text), "x").expect_err("not an SFC");
        assert_eq!(
            problems[0].message,
            "Invalid SFC workspace format. Expected JSON object with name, steps, and transitions."
        );
    }
}

#[test]
fn the_runtime_entry_declares_only_the_addresses_the_companion_names() {
    // Specification 17 §3 and 25 §11.4: an `.sfc` file gets `<base>.st` and
    // `<base>.visual.runtime.st`, and the entry declares each address the generated code names.
    // Catches the old scan of the raw file, which also declared %QW9 from the description.
    let text = DEMO.replacen(
        "\"steps\"",
        "\"metadata\": { \"description\": \"spare output %QW9\" },\n  \"steps\"",
        1,
    );
    let files = companion("/work/snake.sfc", &text).expect("companion");
    assert_eq!(files.companion_file, "snake.st");
    assert_eq!(files.runtime_entry_file, "snake.visual.runtime.st");
    assert!(files
        .companion_text
        .starts_with("(*\n  Auto-generated from sfc source: snake.sfc\n"));
    assert!(files
        .runtime_entry_text
        .contains("  fb_snake : FB_snake_SFC;\n  ld_io_QX0_0 AT %QX0.0 : BOOL;\nEND_VAR"));
    assert!(!files.runtime_entry_text.contains("%QW9"));
}

/// The SFC examples of the repository and their base names.
const EXAMPLES: [(&str, &str); 4] = [
    (
        "ethercat-snake-advanced.sfc.json",
        "ethercat-snake-advanced",
    ),
    ("ethercat-snake-simple.sfc.json", "ethercat-snake-simple"),
    (
        "sfc_parallel_branches_example.sfc",
        "sfc_parallel_branches_example",
    ),
    ("sfc_simple_parallel.sfc", "sfc_simple_parallel"),
];

#[test]
fn every_example_validates_and_has_a_companion() {
    // The examples of `examples/sfc` are the charts the documentation tells users to open and run;
    // trust-lsp runs their companions on the runtime. Catches an example that Validate refuses.
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/sfc/");
    for (file, base) in EXAMPLES {
        let text = std::fs::read_to_string(format!("{path}{file}")).expect("example SFC");
        assert_eq!(validate(&program(&text)), Ok(Vec::new()), "{file}");
        generate(&program(&text), base).unwrap_or_else(|problems| panic!("{file}: {problems:?}"));
    }
}
