//! Tests of the chart an SFC program draws (`sfc-profile.md`, "Execution of a visual SFC"): the
//! IEC transitions of parallel splits and joins, their priority order, the actions and their
//! associations, and the checks Validate adds. Every expected value is worked out by hand from
//! the rules of `sfc-profile.md`.

use super::*;
use crate::sfc::validate;

fn program(text: &str) -> Json {
    Json::parse(text).expect("test program is JSON")
}

fn chart_of(file: &Json) -> Chart<'_> {
    Chart::read(&Workspace::read(file).expect("an SFC"))
}

/// Each IEC transition as `<before> -> <after>[: <conditions>]`, in priority order.
fn described(chart: &Chart<'_>) -> Vec<String> {
    let names = |steps: &[usize]| {
        steps
            .iter()
            .map(|&step| chart.steps[step].name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    chart
        .transitions
        .iter()
        .map(|transition| {
            let ends = format!(
                "{} -> {}",
                names(&transition.before),
                names(&transition.after)
            );
            if transition.conditions.is_empty() {
                ends
            } else {
                format!("{ends}: {}", transition.conditions.join(" & "))
            }
        })
        .collect()
}

/// Each action as `<name>: <qualifier> [<duration>] <step>, ...`.
fn associations(chart: &Chart<'_>) -> Vec<String> {
    chart
        .actions
        .iter()
        .map(|action| {
            let parts: Vec<String> = action
                .associations
                .iter()
                .map(|association| {
                    let step = &chart.steps[association.step].name;
                    match &association.duration {
                        Some(duration) => {
                            format!("{} {duration} {step}", association.qualifier.name())
                        }
                        None => format!("{} {step}", association.qualifier.name()),
                    }
                })
                .collect();
            format!("{}: {}", action.name, parts.join(", "))
        })
        .collect()
}

fn messages(text: &str) -> Vec<String> {
    validate(&program(text))
        .expect("an SFC")
        .into_iter()
        .map(|issue| issue.message)
        .collect()
}

/// Start splits into A and B, which join into D. The file holds B's transition into the join
/// before A's, and numbers some parts.
const PARALLEL: &str = r#"{
  "name": "P",
  "steps": [
    { "id": "s", "name": "Start", "initial": true }, { "id": "a", "name": "A" },
    { "id": "b", "name": "B" }, { "id": "d", "name": "D" }
  ],
  "transitions": [
    { "id": "e0", "name": "Go", "condition": "go", "sourceStepId": "s", "targetStepId": "sp" },
    { "id": "e1", "condition": "TRUE", "sourceStepId": "sp", "targetStepId": "a", "priority": 3 },
    { "id": "e2", "condition": "c_ok", "sourceStepId": "sp", "targetStepId": "b", "priority": 2 },
    { "id": "e3", "condition": "b_done", "sourceStepId": "b", "targetStepId": "jn", "priority": 5 },
    { "id": "e4", "condition": " true ", "sourceStepId": "a", "targetStepId": "jn" },
    { "id": "e5", "condition": "1", "sourceStepId": "jn", "targetStepId": "d" }
  ],
  "parallelSplits": [ { "id": "sp", "name": "Split", "branchIds": ["a", "b"] } ],
  "parallelJoins": [ { "id": "jn", "name": "Join", "branchIds": ["a", "b"], "nextStepId": "d" } ]
}"#;

#[test]
fn a_parallel_split_and_a_parallel_join_are_one_transition_each() {
    // Table 61 4a and 4c: the split's incoming transition and its links are one transition from
    // Start to both branches, with the conditions in order (go, then the branch links in branch
    // order: TRUE adds nothing, c_ok); the join's links in branch order (A's " true " adds
    // nothing, B's b_done) and its outgoing "1" (always true, §6.7.3) are one transition from
    // both branches to D. Each takes the lowest number of its parts (2 and 5) and the file place
    // of its first part (0 and 3). Catches the old companion, which left out every transition
    // that touches a split or a join, a branch condition dropped, and a case-sensitive TRUE.
    let file = program(PARALLEL);
    let chart = chart_of(&file);
    assert_eq!(
        described(&chart),
        ["Start -> A, B: go & c_ok", "A, B -> D: b_done"]
    );
    let priorities: Vec<Option<f64>> = chart
        .transitions
        .iter()
        .map(|transition| transition.priority)
        .collect();
    assert_eq!(priorities, [Some(2.0), Some(5.0)]);
    assert_eq!(chart.transitions[0].name, "Go");
    assert_eq!(chart.transitions[1].name, "");
    assert_eq!(validate(&file), Ok(Vec::new()));
}

#[test]
fn a_join_that_leads_into_a_split_is_one_transition() {
    // The join's outgoing transition enters a split: one transition from the join's branches to
    // the split's branches whose condition is x (A's link), y (the join's outgoing transition),
    // then z (the split's link to C). Catches a join that stops at the split.
    let file = program(
        r#"{
      "name": "J",
      "steps": [
        { "id": "i", "name": "I", "initial": true }, { "id": "a", "name": "A" },
        { "id": "b", "name": "B" }, { "id": "c", "name": "C" }, { "id": "d", "name": "D" }
      ],
      "transitions": [
        { "id": "e0", "condition": "TRUE", "sourceStepId": "i", "targetStepId": "sp1" },
        { "id": "e1", "condition": "TRUE", "sourceStepId": "sp1", "targetStepId": "a" },
        { "id": "e2", "condition": "TRUE", "sourceStepId": "sp1", "targetStepId": "b" },
        { "id": "e3", "condition": "x", "sourceStepId": "a", "targetStepId": "jn" },
        { "id": "e4", "condition": "TRUE", "sourceStepId": "b", "targetStepId": "jn" },
        { "id": "e5", "condition": "y", "sourceStepId": "jn", "targetStepId": "sp2" },
        { "id": "e6", "condition": "z", "sourceStepId": "sp2", "targetStepId": "c" },
        { "id": "e7", "condition": "TRUE", "sourceStepId": "sp2", "targetStepId": "d" }
      ],
      "parallelSplits": [
        { "id": "sp1", "name": "S1", "branchIds": ["a", "b"] },
        { "id": "sp2", "name": "S2", "branchIds": ["c", "d"] }
      ],
      "parallelJoins": [ { "id": "jn", "name": "J", "branchIds": ["a", "b"] } ]
    }"#,
    );
    let chart = chart_of(&file);
    assert_eq!(described(&chart), ["I -> A, B", "A, B -> C, D: x & y & z"]);
    assert_eq!(validate(&file), Ok(Vec::new()));
}

#[test]
fn numbered_transitions_come_first_then_the_file_order() {
    // Table 61 2b: the lowest number first (S -> C is 1, S -> B is 2); then the transitions
    // without a number in file order (S -> A, then A -> S). A number written as text is no
    // number. Catches the old file-order-only evaluation, which let S -> A win.
    let file = program(
        r#"{
      "name": "S",
      "steps": [
        { "id": "s", "name": "S", "initial": true }, { "id": "a", "name": "A" },
        { "id": "b", "name": "B" }, { "id": "c", "name": "C" }
      ],
      "transitions": [
        { "id": "e0", "condition": "a", "sourceStepId": "s", "targetStepId": "a" },
        { "id": "e1", "condition": "b", "sourceStepId": "s", "targetStepId": "b", "priority": 2 },
        { "id": "e2", "condition": "c", "sourceStepId": "s", "targetStepId": "c", "priority": 1 },
        { "id": "e3", "condition": "back", "sourceStepId": "a", "targetStepId": "s", "priority": "1" }
      ]
    }"#,
    );
    let chart = chart_of(&file);
    assert_eq!(
        described(&chart),
        ["S -> C: c", "S -> B: b", "S -> A: a", "A -> S: back"]
    );
}

#[test]
fn entries_of_one_name_are_one_action_with_one_body() {
    // Table 57 feature 1: Motor, MOTOR and motor are one action, named as first written, with
    // the associations in chart order and the one body; the R entry has none, the L entry the
    // same text. Actions are in the order of their first association. A body of comments only is
    // no body. A number is a duration in milliseconds. Catches an action per entry (three
    // instances of one name) and a lost reset association.
    let file = program(
        r#"{
      "name": "A",
      "steps": [
        { "id": "s1", "name": "S1", "initial": true, "actions": [
          { "id": "a1", "name": "Motor", "qualifier": "S", "body": "m := TRUE;" },
          { "id": "a2", "name": "lamp", "qualifier": "", "body": "// later" }
        ] },
        { "id": "s2", "name": "S2", "actions": [
          { "id": "a3", "name": "MOTOR", "qualifier": "R", "body": "" },
          { "id": "a4", "name": "Pulse", "qualifier": "p1", "body": "p := p + 1;" }
        ] },
        { "id": "s3", "name": "S3", "actions": [
          { "id": "a5", "name": "motor", "qualifier": "L", "time": 300, "body": "m := TRUE;\r\n" }
        ] }
      ],
      "transitions": [
        { "id": "t1", "condition": "TRUE", "sourceStepId": "s1", "targetStepId": "s2" },
        { "id": "t2", "condition": "TRUE", "sourceStepId": "s2", "targetStepId": "s3" }
      ]
    }"#,
    );
    let chart = chart_of(&file);
    assert_eq!(
        associations(&chart),
        [
            "Motor: S S1, R S2, L T#300ms S3",
            "lamp: N S1",
            "Pulse: P1 S2"
        ]
    );
    assert_eq!(chart.actions[0].body, "m := TRUE;");
    assert!(!chart.actions[1].has_statements());
    assert_eq!(validate(&file), Ok(Vec::new()));
}

/// A chart whose step S has one L action with the `time` value `time`.
fn timed(time: &str) -> String {
    format!(
        r#"{{ "name": "T", "steps": [ {{ "id": "s", "name": "S", "initial": true, "actions": [
          {{ "id": "a", "name": "Lamp", "qualifier": "L", "time": {time}, "body": "x := TRUE;" }}
        ] }} ], "transitions": [] }}"#
    )
}

#[test]
fn a_duration_is_a_time_literal_or_milliseconds() {
    // §6.7.4.5 and Table 8: T# and TIME# literals in any case, blanks around them trimmed, and a
    // number of milliseconds are durations; text without T#, an LTIME literal, a negative
    // duration and a value of another kind are not, and a missing or blank one is asked for.
    // Catches a companion that writes `PT := 2s` (not Structured Text) or an LTIME into a TIME
    // input.
    for (time, duration) in [
        ("\" t#1.5s \"", "t#1.5s"),
        ("\"TIME#1m30s\"", "TIME#1m30s"),
        ("300", "T#300ms"),
        ("0.5", "T#0.5ms"),
    ] {
        let file = program(&timed(time));
        assert_eq!(validate(&file), Ok(Vec::new()), "{time}");
        let chart = chart_of(&file);
        assert_eq!(
            chart.actions[0].associations[0].duration.as_deref(),
            Some(duration)
        );
    }
    for (time, message) in [
        (
            "\"2s\"",
            "Action Lamp in step S: 2s is not a duration; write a TIME literal such as T#2s, or a number of milliseconds.",
        ),
        (
            "\"LT#2s\"",
            "Action Lamp in step S: LT#2s is not a duration; write a TIME literal such as T#2s, or a number of milliseconds.",
        ),
        (
            "\"T#-1s\"",
            "Action Lamp in step S: T#-1s is not a duration; write a TIME literal such as T#2s, or a number of milliseconds.",
        ),
        (
            "-5",
            "Action Lamp in step S: -5 is not a duration; write a TIME literal such as T#2s, or a number of milliseconds.",
        ),
        (
            "true",
            "Action Lamp in step S: true is not a duration; write a TIME literal such as T#2s, or a number of milliseconds.",
        ),
        (
            "\" \"",
            "Action Lamp in step S: qualifier L needs a duration such as T#2s.",
        ),
        (
            "null",
            "Action Lamp in step S: qualifier L needs a duration such as T#2s.",
        ),
    ] {
        assert_eq!(messages(&timed(time)), [message], "{time}");
    }
}

#[test]
fn validate_refuses_actions_the_companion_cannot_run() {
    // In order: a name with a blank, a keyword name, an empty name, an unknown qualifier, a
    // second body that differs, and a second time-related association in one step (L and D are
    // always active together, an error by §6.7.4.6). The refused entries are not associations.
    // Catches a companion that declares `Fill Tank : …`, an instance named STEP, or that runs one
    // of two bodies silently.
    let text = r#"{
      "name": "Bad",
      "steps": [
        { "id": "s1", "name": "S1", "initial": true, "actions": [
          { "id": "a1", "name": "Fill Tank", "qualifier": "N", "body": "x := 1;" },
          { "id": "a2", "name": "STEP", "qualifier": "N", "body": "x := 2;" },
          { "id": "a3", "name": " ", "qualifier": "N", "body": "x := 3;" },
          { "id": "a4", "name": "Valve", "qualifier": "X", "body": "x := 4;" },
          { "id": "a5", "name": "Pump", "qualifier": "N", "body": "p := TRUE;" },
          { "id": "a6", "name": "Pump", "qualifier": "N", "body": "p := FALSE;" },
          { "id": "a7", "name": "Lamp", "qualifier": "L", "time": "T#1s", "body": "l := TRUE;" },
          { "id": "a8", "name": "lamp", "qualifier": "D", "time": "T#2s" }
        ] }
      ],
      "transitions": []
    }"#;
    let issues = validate(&program(text)).expect("an SFC");
    let ids: Vec<&str> = issues.iter().map(|issue| issue.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "action_name_a1",
            "action_name_a2",
            "action_name_a3",
            "action_qualifier_a4",
            "action_body_a6",
            "action_timed_twice_a8"
        ]
    );
    assert_eq!(
        issues[0].message,
        "Action Fill Tank in step S1: an action name is an identifier (letters, digits and underscores, not a keyword)."
    );
    assert_eq!(issues[2].message, "Step S1 has an action without a name.");
    assert_eq!(
        issues[3].message,
        "Action Valve in step S1: X is not an action qualifier (N, R, S, L, D, P, SD, DS, SL, P1 or P0)."
    );
    assert_eq!(
        issues[4].message,
        "Action Pump has different statements in step S1; an action's statements are written once."
    );
    assert_eq!(
        issues[5].message,
        "Action Lamp has two time-related qualifiers in step S1 (L and D); they would always be active together."
    );
    assert!(issues
        .iter()
        .all(|issue| issue.kind == IssueKind::Action && issue.element.as_deref() == Some("s1")));
    let file = program(text);
    let chart = chart_of(&file);
    assert_eq!(
        associations(&chart),
        ["Pump: N S1, N S1", "Lamp: L T#1s S1"]
    );
}

#[test]
fn a_step_name_gives_an_identifier_flag() {
    // The flag is `<trimmed name>_active`: " Fill " gives Fill_active and is accepted; "Fill Tank"
    // and "1st" do not give identifiers; a step without a name is refused. A keyword name (STEP)
    // gives STEP_active, an identifier. Catches `Fill Tank_active` in the companion.
    let text = r#"{
      "name": "N",
      "steps": [
        { "id": "s1", "name": " Fill ", "initial": true },
        { "id": "s2", "name": "Fill Tank" },
        { "id": "s3", "name": "1st" },
        { "id": "s4" },
        { "id": "s5", "name": "STEP" }
      ],
      "transitions": [
        { "id": "t1", "condition": "TRUE", "sourceStepId": "s1", "targetStepId": "s2" },
        { "id": "t2", "condition": "TRUE", "sourceStepId": "s2", "targetStepId": "s3" },
        { "id": "t3", "condition": "TRUE", "sourceStepId": "s3", "targetStepId": "s4" },
        { "id": "t4", "condition": "TRUE", "sourceStepId": "s4", "targetStepId": "s5" }
      ]
    }"#;
    assert_eq!(
        messages(text),
        [
            "Step Fill Tank: its flag Fill Tank_active is not an identifier; a step name has letters, digits and underscores.",
            "Step 1st: its flag 1st_active is not an identifier; a step name has letters, digits and underscores.",
            "Step s4 has no name."
        ]
    );
}

#[test]
fn a_network_has_one_initial_step() {
    // §6.7.2: A and B are joined by a transition and both initial: one finding, on B. C and D form
    // a second network with its own initial step C, which is allowed. Catches a chart that starts
    // with two active steps in one sequence.
    let text = r#"{
      "name": "Two",
      "steps": [
        { "id": "a", "name": "A", "initial": true }, { "id": "b", "name": "B", "initial": true },
        { "id": "c", "name": "C", "initial": true }, { "id": "d", "name": "D" }
      ],
      "transitions": [
        { "id": "t1", "condition": "TRUE", "sourceStepId": "a", "targetStepId": "b" },
        { "id": "t2", "condition": "TRUE", "sourceStepId": "c", "targetStepId": "d" }
      ]
    }"#;
    let issues = validate(&program(text)).expect("an SFC");
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert_eq!(issues[0].id, "network_initial_b");
    assert_eq!(
        issues[0].message,
        "Steps A and B are initial steps of one network; a network has exactly one initial step."
    );
    assert_eq!(issues[0].element.as_deref(), Some("b"));
}

#[test]
fn every_declared_name_is_declared_once() {
    // Ignoring case: the variable Run_active and the flag of step Run; the action Motor and the
    // variable motor; the part Pump_S_FF of the S action Pump and the variable PUMP_S_FF; the
    // first transition flag and the variable sfc_t1. Two steps of one name are reported once, as
    // a duplicate step name. Catches a companion that declares a name twice and fails to compile.
    let text = r#"{
      "name": "Clash",
      "steps": [
        { "id": "s1", "name": "Run", "initial": true, "actions": [
          { "id": "a1", "name": "Motor", "qualifier": "N", "body": "x := 1;" },
          { "id": "a2", "name": "Pump", "qualifier": "S", "body": "y := 1;" }
        ] },
        { "id": "s2", "name": "Stop" },
        { "id": "s3", "name": "Stop" }
      ],
      "transitions": [
        { "id": "t1", "condition": "TRUE", "sourceStepId": "s1", "targetStepId": "s2" },
        { "id": "t2", "condition": "TRUE", "sourceStepId": "s2", "targetStepId": "s3" }
      ],
      "variables": [
        { "name": "Run_active", "type": "BOOL" },
        { "name": "motor", "type": "BOOL" },
        { "name": "PUMP_S_FF", "type": "BOOL" },
        { "name": "sfc_t1", "type": "BOOL" }
      ]
    }"#;
    assert_eq!(
        messages(text),
        [
            "Duplicate step name: Stop",
            "The companion would declare Run_active twice: variable Run_active and the flag of step Run; rename one of them.",
            "The companion would declare Motor twice: variable motor and action Motor; rename one of them.",
            "The companion would declare Pump_S_FF twice: variable PUMP_S_FF and the control of action Pump; rename one of them.",
            "The companion would declare SFC_T1 twice: variable sfc_t1 and the flag of transition SFC_T1; rename one of them."
        ]
    );
}
