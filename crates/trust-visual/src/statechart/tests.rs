//! The statechart companion, Validate, and the editor graph (specification 25 §11.4:
//! "Companion generation preserves declared symbols, addressed globals, topology, and
//! event/action inputs"). Expected texts are worked out from the rules in `generate.rs` and
//! `graph.rs`; the example texts are the ones the TypeScript wrote before the move.

use super::generate::normalize_guard;
use super::{
    action_report, add_state, check_format, file, generate, graph, keep_identity, validate,
};
use crate::companion::companion;
use crate::{Global, Json};

fn parse(text: &str) -> Json {
    Json::parse(text).expect("valid JSON")
}

const DOOR: &str = r#"{
  "id": "door",
  "initial": "Closed",
  "states": {
    "Closed": {
      "entry": ["lockOn"],
      "exit": ["lockOff"],
      "on": {
        "OPEN": { "target": "Opening", "guard": "enabled == true && !(fault != false)", "actions": ["log"] },
        "TICK": { "target": "Closed", "after": 1500.4 }
      }
    },
    "Opening": {
      "on": {
        "STOP": "Closed",
        "open": { "target": "Nowhere" },
        "SKIP": { "target": "" }
      }
    },
    "Open": { "type": "final" }
  },
  "actionMappings": {
    "lockOn": { "action": "write_output", "address": "%QX0.1", "value": true },
    "lockOff": { "action": "WRITE_OUTPUT", "address": "Q out", "value": false },
    "log": { "action": "LOG", "message": "door's *) opening" }
  }
}"#;

#[test]
fn a_chart_becomes_a_function_block_with_events_timers_guards_and_actions() {
    // Worked by hand from the lowering rules: states numbered in file order, one input per
    // event (`open` after `OPEN` becomes EV_OPEN_1), one TON per timed transition with the delay
    // rounded (1500.4 ms is T#1500ms), the guard rewritten (`==`, `!=`, `&&`, `true`, `false`),
    // exit + transition + entry actions in that order, an entry-less missing target as a
    // comment, and the target `""` skipped. `Q out` is no variable, so it is an invalid target
    // and no global; a LOG text that holds `*)` cannot end the comment. The timer runs once per
    // scan while its state, Closed (0), is active; each transition that leaves Closed resets it
    // after the exit actions, and each that enters Closed (TICK and Opening's STOP) starts it once
    // the state has changed, so it counts from the scan that enters Closed and the self-transition
    // TICK starts it again. Catches a changed order of the actions, a lost event input, a timer
    // reset in every scan before it runs, which never reached its delay (a transition with
    // `after` never fired), and a timer that starts only in the scan after its state was entered,
    // which fired one task interval late (Codex's review of #147, 31 October 2026).
    let generated = generate(&parse(DOOR), "door").expect("a statechart");
    assert_eq!(generated.function_block, DOOR_COMPANION);
    assert_eq!(
        generated.globals,
        vec![Global {
            name: "ld_io_QX0_1".into(),
            address: Some("%QX0.1".into()),
            ty: "BOOL".into(),
            initial: None,
        }]
    );
}

const DOOR_COMPANION: &str = "(*
  Auto-generated Statechart companion.
  Source of truth remains the .statechart.json file.
  Generated to support .st-first runtime workflows.
*)

FUNCTION_BLOCK FB_door_STATECHART
VAR_EXTERNAL
  ld_io_QX0_1 : BOOL;
END_VAR

VAR_INPUT
  EV_OPEN : BOOL;
  EV_STOP : BOOL;
  EV_OPEN_1 : BOOL;
END_VAR
VAR_OUTPUT
  STATE_INDEX : INT;
  STATE_NAME : STRING[80];
END_VAR
VAR
  _initialized : BOOL := FALSE;
  _state : INT := 0;
  sc_timer_Closed_TICK_1 : TON; (* Closed.TICK *)
END_VAR

(* State encoding *)
(*  0 = Closed *)
(*  1 = Opening *)
(*  2 = Open *)

IF NOT _initialized THEN
  _state := 0;
  _initialized := TRUE;
END_IF;

sc_timer_Closed_TICK_1(IN := (_state = 0), PT := T#1500ms);

CASE _state OF
  0:
    IF (EV_OPEN) AND (enabled = TRUE  AND  !(fault <> FALSE)) THEN
      (* Invalid WRITE_OUTPUT target for action lockOff: Q out *)
      sc_timer_Closed_TICK_1(IN := FALSE, PT := T#1500ms);
      (* LOG: door's * ) opening *)
      _state := 1;
    ELSIF sc_timer_Closed_TICK_1.Q THEN
      (* Invalid WRITE_OUTPUT target for action lockOff: Q out *)
      sc_timer_Closed_TICK_1(IN := FALSE, PT := T#1500ms);
      ld_io_QX0_1 := TRUE;
      _state := 0;
      sc_timer_Closed_TICK_1(IN := TRUE, PT := T#1500ms);
    END_IF;
  1:
    IF EV_STOP THEN
      ld_io_QX0_1 := TRUE;
      _state := 0;
      sc_timer_Closed_TICK_1(IN := TRUE, PT := T#1500ms);
    ELSIF EV_OPEN_1 THEN
      (* Invalid transition target: Nowhere *)
    END_IF;
  2:
END_CASE;

STATE_INDEX := _state;
CASE _state OF
  0: STATE_NAME := 'Closed';
  1: STATE_NAME := 'Opening';
  2: STATE_NAME := 'Open';
  ELSE
    STATE_NAME := 'UNKNOWN';
END_CASE;
END_FUNCTION_BLOCK";

#[test]
fn states_are_numbered_in_the_order_javascript_enumerates_them() {
    // JavaScript lists the array-index name "1" before "b" and "a", so state 0 is "1". Without
    // `initial` the first state is the start state. Catches numbering by file order alone.
    let chart = r#"{"id":"n","states":{"b":{},"1":{},"a":{}}}"#;
    let text = generate(&parse(chart), "").expect("chart").function_block;
    assert!(text.contains("FUNCTION_BLOCK FB_n_STATECHART\n"), "{text}");
    assert!(
        text.contains("(*  0 = 1 *)\n(*  1 = b *)\n(*  2 = a *)"),
        "{text}"
    );
    assert!(text.contains("  _state := 0;\n"), "{text}");
    assert!(
        text.contains("  0: STATE_NAME := '1';\n  1: STATE_NAME := 'b';"),
        "{text}"
    );
}

#[test]
fn action_values_become_iec_literals() {
    // Specification 01 §9: text is an IEC string with `$'`; a number is written as JavaScript
    // writes it; `null` and a missing value are 0; any other value is the IEC string of its JSON
    // (members in JavaScript order). Catches the earlier `''` doubling.
    let chart = r#"{
      "id": "v", "states": { "S": { "entry": ["set"], "on": { "GO": "S" } } },
      "actionMappings": { "set": { "action": "SET_MULTIPLE", "targets": [
        { "address": "Text", "value": "it's" },
        { "address": "Count", "value": 2.50 },
        { "address": "Gone", "value": null },
        { "address": "Missing" },
        { "address": "Record", "value": { "b": 1, "2": [true] } },
        { "address": "Fb.Out", "value": false },
        { "address": "not valid", "value": 1 }
      ] } }
    }"#;
    let text = generate(&parse(chart), "v").expect("chart").function_block;
    let expected = [
        "      Text := 'it$'s';",
        "      Count := 2.5;",
        "      Gone := 0;",
        "      Missing := 0;",
        "      Record := '{\"2\":[true],\"b\":1}';",
        "      Fb.Out := FALSE;",
        "      (* Invalid SET_MULTIPLE target for action set: not valid *)",
    ]
    .join("\n");
    assert!(text.contains(&expected), "{text}");
}

#[test]
fn guards_are_rewritten_in_order() {
    // `==` before `!=`, so `!==` becomes `<>`; the words true/false in any case, whole words
    // only. Catches a rewrite of `untrue` or of `true_flag`.
    assert_eq!(normalize_guard("a !== b"), "a <> b");
    assert_eq!(normalize_guard("x==True||y"), "x=TRUE OR y");
    assert_eq!(
        normalize_guard("untrue && true_flag && FALSE"),
        "untrue  AND  true_flag  AND  FALSE"
    );
}

#[test]
fn validate_answers_what_the_companion_needs() {
    // The editor's Validate checks that the companion can be generated: the format, at least one
    // state, and readable action mappings. Catches a Validate that only parses JSON.
    assert_eq!(validate(&parse(DOOR)), Ok(()));
    assert_eq!(
        validate(&parse(r#"{"states":{}}"#)),
        Err("Invalid statechart format. Expected JSON object with id and states.".to_owned())
    );
    assert_eq!(
        validate(&parse(r#"{"id":"x","states":{}}"#)),
        Err("Statechart has no states.".to_owned())
    );
    assert_eq!(
        validate(&parse(
            r#"{"id":"x","states":{"A":{"entry":["go"],"on":{"E":"A"}}},"actionMappings":{"go":{"address":"%QX0.0"}}}"#
        )),
        Err("The action mapping \"go\" has no action type.".to_owned())
    );
    assert!(check_format(&parse(r#"{"id":"x","states":[]}"#)).is_err());
}

fn example(name: &str) -> String {
    let path = format!(
        "{}/../../examples/statecharts/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"))
}

#[test]
fn the_checked_in_statechart_examples_are_reproduced_exactly() {
    // examples/statecharts holds the companion and runtime entry of three charts, as the
    // TypeScript wrote them before the move, except the timers of ethercat-snake, which run once
    // per scan while their state is active since 30 October 2026 and start in the transition that
    // enters their state since 31 October 2026 (specification 17 §3.2).
    // Catches any drift of the lowering, the layout, or the globals of the runtime entry.
    for base in ["ethercat-snake-simple", "ethercat-snake", "traffic-light"] {
        let source = format!("{base}.statechart.json");
        let files = companion(&source, &example(&source)).expect("companion");
        assert_eq!(
            files.companion_text,
            example(&format!("{base}.st")),
            "{base}"
        );
        assert_eq!(
            files.runtime_entry_text,
            example(&format!("{base}.visual.runtime.st")),
            "{base}"
        );
    }
}

#[test]
fn a_file_becomes_the_editor_graph() {
    // Worked from the import rules: one node per state (`state_<name>`, its type from `initial`
    // and `type`, entry/exit as stored or empty), one edge per transition (a text transition is
    // its target; guard/actions/after only when the file has them, `after: null` kept), the
    // action mappings as they are. Catches a dropped `null` delay or a lost text transition.
    let text = r#"{
      "id": "g", "initial": "A",
      "states": {
        "A": { "entry": ["x"], "on": { "GO": "B", "T": { "target": "A", "after": null, "guard": "ok" } } },
        "B": { "type": "final" },
        "C": { "type": "compound", "exit": "y" }
      }
    }"#;
    let graph = graph(&parse(text)).expect("graph");
    assert_eq!(
        graph.to_compact(),
        concat!(
            r#"{"states":["#,
            r#"{"id":"state_A","label":"A","type":"initial","entry":["x"],"exit":[]},"#,
            r#"{"id":"state_B","label":"B","type":"final","entry":[],"exit":[]},"#,
            r#"{"id":"state_C","label":"C","type":"compound","entry":[],"exit":"y"}],"#,
            r#""transitions":["#,
            r#"{"id":"edge_state_A_state_B_GO","source":"state_A","target":"state_B","event":"GO"},"#,
            r#"{"id":"edge_state_A_state_A_T","source":"state_A","target":"state_A","event":"T","guard":"ok","after":null}],"#,
            r#""actionMappings":{}}"#
        )
    );
    assert_eq!(
        super::graph(&parse(r#"{"id":"g"}"#)),
        Err("Invalid statechart format. Expected JSON object with id and states.".to_owned())
    );
}

#[test]
fn a_graph_is_written_as_the_editor_wrote_it() {
    // Worked from the save rules: the machine is `stateMachine`; `initial` from the initial
    // node; empty entry/exit left out, text kept; an edge without event is `NEXT` and a later edge
    // of the same event replaces the earlier in its place; an edge to a missing node is dropped; a
    // falsy guard and empty actions are left out, `after` kept when present (even null); names
    // that are array indices come first. Catches any member order that differs from
    // JSON.stringify, which would rewrite a file the user did not change.
    let graph = parse(
        r#"{
      "states": [
        { "id": "s1", "label": "Idle", "type": "initial", "entry": [], "exit": "bye" },
        { "id": "s2", "label": "Run", "type": "normal", "entry": ["go"] },
        { "id": "s3", "label": "7", "type": "final" }
      ],
      "transitions": [
        { "id": "e1", "source": "s1", "target": "s2", "event": "", "guard": "", "actions": [] },
        { "id": "e2", "source": "s1", "target": "s3", "event": "B", "after": null },
        { "id": "e3", "source": "s1", "target": "s2", "guard": "ok", "actions": ["a"], "after": 500 },
        { "id": "e4", "source": "s2", "target": "gone", "event": "X" }
      ],
      "actionMappings": { "go": { "value": 1, "action": "LOG" } }
    }"#,
    );
    assert_eq!(
        file(&graph).expect("file").to_pretty(),
        r#"{
  "id": "stateMachine",
  "initial": "Idle",
  "states": {
    "7": {
      "type": "final"
    },
    "Idle": {
      "exit": "bye",
      "on": {
        "NEXT": {
          "target": "Run",
          "guard": "ok",
          "actions": [
            "a"
          ],
          "after": 500
        },
        "B": {
          "target": "7",
          "after": null
        }
      }
    },
    "Run": {
      "entry": [
        "go"
      ]
    }
  },
  "actionMappings": {
    "go": {
      "value": 1,
      "action": "LOG"
    }
  }
}"#
    );
    assert!(file(&parse(r#"{"states":[]}"#)).is_err());
}

#[test]
fn opening_and_saving_an_example_writes_what_the_editor_wrote() {
    // `fixtures/<name>.saved.json` is what the TypeScript editor wrote when it opened and saved
    // examples/statecharts/<name>.statechart.json (its import and export functions run as they
    // were), for the six example charts. Catches any difference of the moved rules on real
    // charts, and a fixture whose example is gone.
    let fixtures = concat!(env!("CARGO_MANIFEST_DIR"), "/src/statechart/fixtures");
    let mut compared = 0;
    for entry in std::fs::read_dir(fixtures).expect("fixtures") {
        let path = entry.expect("fixture").path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let Some(base) = name.strip_suffix(".saved.json") else {
            continue;
        };
        let source = example(&format!("{base}.statechart.json"));
        let saved = std::fs::read_to_string(&path).expect("fixture text");
        let graph = graph(&parse(&source)).expect("graph");
        assert_eq!(file(&graph).expect("file").to_pretty(), saved, "{base}");
        compared += 1;
    }
    assert_eq!(compared, 6);
}

#[test]
fn saving_over_a_file_keeps_its_id() {
    // The graph has no id, so a save used to replace the chart's id (`door`, or the name a new
    // statechart got) with `stateMachine`. Saved over the chart, the file keeps it; an empty id
    // leaves `stateMachine`. Catches that loss.
    let mut saved = file(&graph(&parse(DOOR)).expect("graph")).expect("file");
    assert_eq!(saved.str_at("id"), Some("stateMachine"));
    keep_identity(&mut saved, &parse(DOOR));
    assert_eq!(saved.str_at("id"), Some("door"));
    assert_eq!(
        saved.as_object().map(|members| members[0].0.as_str()),
        Some("id"),
        "the id keeps its place"
    );
    let mut unnamed = file(&graph(&parse(DOOR)).expect("graph")).expect("file");
    keep_identity(&mut unnamed, &parse(r#"{"id":""}"#));
    assert_eq!(unnamed.str_at("id"), Some("stateMachine"));
}

#[test]
fn the_action_report_counts_state_and_transition_actions() {
    // The mapping panel warns about actions no mapping defines and mappings nothing names.
    // Worked by hand: `go` is named only by a transition, so it is used (the old panel looked for
    // transitions inside the states and called it unused); `beep` is named but not mapped;
    // `spare` is mapped but not named. Catches a report that ignores transition actions.
    let graph = parse(
        r#"{"states":[{"id":"state_A","label":"A","entry":["lamp",""],"exit":["beep"]}],
            "transitions":[{"id":"t","source":"state_A","target":"state_A","actions":["go","lamp"]}],
            "actionMappings":{"spare":{"action":"LOG"},"go":{"action":"LOG"},"lamp":{"action":"LOG"}}}"#,
    );
    let report = action_report(&graph);
    assert_eq!(report.unmapped, ["beep"]);
    assert_eq!(report.unused, ["spare"]);
    assert_eq!(action_report(&parse("{}")), super::ActionReport::default());
}

#[test]
fn an_address_the_runtime_refuses_makes_the_chart_untranslatable() {
    // `%QX0.9` names bit 9 of a byte, which the runtime refuses (specification 03 §5). An action
    // that writes it is refused like a Ladder operand, instead of becoming a comment that never
    // writes the output. Catches a silently skipped output.
    let chart = DOOR.replace("\"Q out\"", "\"%QX0.9\"");
    assert_eq!(
        generate(&parse(&chart), "door").expect_err("refused"),
        vec![crate::Problem::new(
            "Action mapping \"lockOff\": '%QX0.9' is not a valid direct address."
        )]
    );
}

/// A new state is named `State<n>` with the smallest n no state's name or id uses, and its id is
/// the one the editor gives that name. Worked by hand: State1 and State3 exist, so the new state
/// is State2; with State2's id taken by a renamed state it is State4. Catches the count-based name
/// (State3 again after State2 was deleted), which the file would write over the other State3.
#[test]
fn a_new_state_gets_the_smallest_free_name() {
    let with = |states: &str| parse(&format!(r#"{{"states":[{states}],"transitions":[]}}"#));
    let graph = with(
        r#"{"id":"state_State1","label":"State1","type":"initial"},{"id":"state_State3","label":"State3","type":"normal"}"#,
    );
    let added = add_state(&graph, "normal").expect("state");
    assert_eq!(
        added,
        parse(r#"{"id":"state_State2","label":"State2","type":"normal","entry":[],"exit":[]}"#)
    );
    let renamed = with(
        r#"{"id":"state_State1","label":"State1","type":"initial"},{"id":"state_State2","label":"Filling","type":"normal"},{"id":"state_State3","label":"State3","type":"normal"}"#,
    );
    assert_eq!(
        add_state(&renamed, "final").expect("state").str_at("label"),
        Some("State4")
    );
    assert_eq!(
        add_state(&graph, "initial"),
        Err("The statechart starts in \"State1\" already; it starts in one state.".to_owned())
    );
    assert_eq!(
        add_state(&graph, "parallel"),
        Err("A state is normal, initial, final or compound, not \"parallel\".".to_owned())
    );
}

/// The file names each state once and starts in one, so a graph with two states of one name or
/// two initial states is refused and nothing is written. Catches the second state written over
/// the first (its transitions lost), and the first initial state quietly made normal.
#[test]
fn a_graph_with_two_states_of_one_name_or_two_starts_is_not_written() {
    let with = |states: &str| parse(&format!(r#"{{"states":[{states}],"transitions":[]}}"#));
    assert_eq!(
        file(&with(
            r#"{"id":"a","label":"Fill","type":"initial"},{"id":"b","label":"Fill","type":"normal"}"#
        )),
        Err("Two states are named \"Fill\"; give each state its own name.".to_owned())
    );
    assert_eq!(
        file(&with(
            r#"{"id":"a","label":"Fill","type":"initial"},{"id":"b","label":"Drain","type":"initial"}"#
        )),
        Err(
            "The states \"Fill\", \"Drain\" are all initial; a statechart starts in one state."
                .to_owned()
        )
    );
    assert!(file(&with(
        r#"{"id":"a","label":"Fill","type":"initial"},{"id":"b","label":"fill","type":"normal"}"#
    ))
    .is_ok());
}
