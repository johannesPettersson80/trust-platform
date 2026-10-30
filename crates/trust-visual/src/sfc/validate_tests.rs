//! Tests of the SFC structure checks (specification 25 §11.4: "SFC topology validation accepts a
//! well-formed parallel split/join and rejects missing continuations or fewer than two branches").
//! Every expected finding is worked out by hand from the rules in `validate.rs`.

use super::*;

fn program(text: &str) -> Json {
    Json::parse(text).expect("test program is JSON")
}

fn ids(issues: &[Issue]) -> Vec<&str> {
    issues.iter().map(|issue| issue.id.as_str()).collect()
}

/// Init splits into A and B, which join into End.
const PARALLEL: &str = r#"{
  "name": "ParallelValid",
  "steps": [
    { "id": "step_init", "name": "Init", "initial": true, "x": 100, "y": 50, "actions": [] },
    { "id": "step_a", "name": "A", "x": 60, "y": 220, "actions": [] },
    { "id": "step_b", "name": "B", "x": 160, "y": 220, "actions": [] },
    { "id": "step_end", "name": "End", "x": 110, "y": 380, "actions": [] }
  ],
  "transitions": [
    { "id": "t_init_split", "name": "InitToSplit", "condition": "TRUE", "sourceStepId": "step_init", "targetStepId": "split_1" },
    { "id": "t_split_a", "name": "SplitToA", "condition": "TRUE", "sourceStepId": "split_1", "targetStepId": "step_a" },
    { "id": "t_split_b", "name": "SplitToB", "condition": "TRUE", "sourceStepId": "split_1", "targetStepId": "step_b" },
    { "id": "t_a_join", "name": "AToJoin", "condition": "TRUE", "sourceStepId": "step_a", "targetStepId": "join_1" },
    { "id": "t_b_join", "name": "BToJoin", "condition": "TRUE", "sourceStepId": "step_b", "targetStepId": "join_1" },
    { "id": "t_join_end", "name": "JoinToEnd", "condition": "TRUE", "sourceStepId": "join_1", "targetStepId": "step_end" }
  ],
  "parallelSplits": [
    { "id": "split_1", "name": "Split1", "x": 110, "y": 140, "branchIds": ["step_a", "step_b"] }
  ],
  "parallelJoins": [
    { "id": "join_1", "name": "Join1", "x": 110, "y": 300, "branchIds": ["step_a", "step_b"], "nextStepId": "step_end" }
  ],
  "variables": [],
  "metadata": { "version": "1.0" }
}"#;

#[test]
fn a_well_formed_parallel_split_and_join_has_no_findings() {
    // Catches a check that rejects a valid divergence/convergence (the first case of spec 25
    // §11.4), for example by counting the join's outgoing transition as a branch.
    assert_eq!(validate(&program(PARALLEL)), Ok(Vec::new()));
}

#[test]
fn a_split_with_one_branch_is_rejected_and_nothing_else() {
    // The split has one incoming transition and connects to its one branch once, so the only
    // finding is the branch count. Catches a check that accepts one branch (spec 25 §11.4) or that
    // reports follow-on findings for a split that is otherwise wired correctly.
    let text = r#"{
      "name": "ParallelInvalidSplit",
      "steps": [
        { "id": "step_init", "name": "Init", "initial": true, "x": 0, "y": 0, "actions": [] },
        { "id": "step_a", "name": "A", "x": 0, "y": 100, "actions": [] }
      ],
      "transitions": [
        { "id": "t1", "name": "InitToSplit", "condition": "TRUE", "sourceStepId": "step_init", "targetStepId": "split_1" },
        { "id": "t2", "name": "SplitToA", "condition": "TRUE", "sourceStepId": "split_1", "targetStepId": "step_a" }
      ],
      "parallelSplits": [ { "id": "split_1", "name": "Split1", "x": 0, "y": 50, "branchIds": ["step_a"] } ],
      "parallelJoins": [],
      "variables": [],
      "metadata": { "version": "1.0" }
    }"#;
    assert_eq!(
        validate(&program(text)),
        Ok(vec![Issue {
            id: "split_branch_count_split_1".to_owned(),
            kind: IssueKind::Connection,
            message: "Parallel split Split1 must define at least 2 branches".to_owned(),
            element: Some("split_1".to_owned()),
        }])
    );
}

#[test]
fn a_join_without_its_continuation_is_rejected() {
    // The join has no outgoing transition and names a next step that does not exist: exactly the
    // two continuation findings, in this order. Catches a check that accepts a missing
    // continuation (spec 25 §11.4) or skips the unknown next step.
    let text = r#"{
      "name": "ParallelInvalidJoin",
      "steps": [
        { "id": "step_init", "name": "Init", "initial": true, "x": 0, "y": 0, "actions": [] },
        { "id": "step_a", "name": "A", "x": 0, "y": 100, "actions": [] },
        { "id": "step_b", "name": "B", "x": 100, "y": 100, "actions": [] }
      ],
      "transitions": [
        { "id": "t1", "name": "InitToSplit", "condition": "TRUE", "sourceStepId": "step_init", "targetStepId": "split_1" },
        { "id": "t2", "name": "SplitToA", "condition": "TRUE", "sourceStepId": "split_1", "targetStepId": "step_a" },
        { "id": "t3", "name": "SplitToB", "condition": "TRUE", "sourceStepId": "split_1", "targetStepId": "step_b" },
        { "id": "t4", "name": "AToJoin", "condition": "TRUE", "sourceStepId": "step_a", "targetStepId": "join_1" },
        { "id": "t5", "name": "BToJoin", "condition": "TRUE", "sourceStepId": "step_b", "targetStepId": "join_1" }
      ],
      "parallelSplits": [ { "id": "split_1", "name": "Split1", "x": 0, "y": 50, "branchIds": ["step_a", "step_b"] } ],
      "parallelJoins": [ { "id": "join_1", "name": "Join1", "x": 50, "y": 150, "branchIds": ["step_a", "step_b"], "nextStepId": "step_missing" } ],
      "variables": [],
      "metadata": { "version": "1.0" }
    }"#;
    let issues = validate(&program(text)).expect("an SFC");
    assert_eq!(
        ids(&issues),
        ["join_outgoing_join_1", "join_next_missing_join_1"]
    );
    assert_eq!(
        issues[0].message,
        "Parallel join Join1 must have exactly one outgoing transition"
    );
    assert_eq!(
        issues[1].message,
        "Parallel join Join1 references unknown next step: step_missing"
    );
    assert_eq!(issues[1].element.as_deref(), Some("join_1"));
}

#[test]
fn steps_need_an_initial_step_and_unique_names() {
    // No step is initial and two steps are called "Run": the initial-step finding first (it has
    // no element), then one duplicate finding naming the second step. Catches a check that names
    // the first occurrence or misses a missing initial step.
    let text = r#"{
      "steps": [
        { "id": "s1", "name": "Run" },
        { "id": "s2", "name": "Run", "initial": false }
      ],
      "transitions": []
    }"#;
    let issues = validate(&program(text)).expect("an SFC");
    assert_eq!(ids(&issues), ["no_initial_step", "duplicate_step_s2"]);
    assert_eq!(issues[0].kind, IssueKind::Step);
    assert_eq!(issues[0].element, None);
    assert_eq!(issues[1].message, "Duplicate step name: Run");
    assert_eq!(issues[1].element.as_deref(), Some("s2"));
}

#[test]
fn transitions_need_existing_ends_and_a_condition() {
    // t1 starts at a step that does not exist, t2 ends at one that does not exist and has a blank
    // condition, t3 has no condition at all. Catches a check that only looks at steps (a split or
    // join id is a valid end) or that accepts a condition of blanks.
    let text = r#"{
      "steps": [ { "id": "s1", "name": "A", "initial": true } ],
      "transitions": [
        { "id": "t1", "condition": "x", "sourceStepId": "ghost", "targetStepId": "s1" },
        { "id": "t2", "condition": "  \n", "sourceStepId": "s1", "targetStepId": "nowhere" },
        { "id": "t3", "sourceStepId": "s1", "targetStepId": "s1" }
      ]
    }"#;
    let issues = validate(&program(text)).expect("an SFC");
    assert_eq!(
        ids(&issues),
        [
            "invalid_source_t1",
            "invalid_target_t2",
            "empty_condition_t2",
            "empty_condition_t3"
        ]
    );
    assert_eq!(
        issues[0].message,
        "Transition references non-existent source node: ghost"
    );
    assert_eq!(
        issues[1].message,
        "Transition references non-existent target node: nowhere"
    );
    assert_eq!(issues[2].kind, IssueKind::Transition);
    assert_eq!(issues[3].element.as_deref(), Some("t3"));
}

#[test]
fn a_transition_priority_is_a_whole_number_from_zero() {
    // IEC 61131-3 Table 61 feature 2b numbers a selection's transitions, the lowest tested first
    // (sfc-profile.md, "Rules of evolution"). 0, 2 and the texts "1" and " 3 " (as the editor's
    // Priority field keeps them) are priorities; null and the blank text of an emptied field are
    // none; -1, 1.5 and the texts "x" and "1.5" are refused, on the transition, with the value as
    // written. Catches a negative or fractional number taken as a priority, a typed number refused
    // as text, and a text that holds no whole number accepted.
    let text = r#"{
      "steps": [
        { "id": "s", "name": "S", "initial": true }, { "id": "a", "name": "A" },
        { "id": "b", "name": "B" }, { "id": "c", "name": "C" }
      ],
      "transitions": [
        { "id": "t1", "condition": "x", "sourceStepId": "s", "targetStepId": "a", "priority": 0 },
        { "id": "t2", "condition": "y", "sourceStepId": "s", "targetStepId": "b", "priority": 2 },
        { "id": "t3", "condition": "z", "sourceStepId": "a", "targetStepId": "s", "priority": null },
        { "id": "t4", "condition": "w", "sourceStepId": "b", "targetStepId": "s", "priority": -1 },
        { "id": "t5", "condition": "v", "sourceStepId": "b", "targetStepId": "a", "priority": 1.5 },
        { "id": "t6", "condition": "u", "sourceStepId": "a", "targetStepId": "b", "priority": "1" },
        { "id": "t7", "condition": "q", "sourceStepId": "s", "targetStepId": "c", "priority": "" },
        { "id": "t8", "condition": "r", "sourceStepId": "c", "targetStepId": "a", "priority": " 3 " },
        { "id": "t9", "condition": "p", "sourceStepId": "c", "targetStepId": "b", "priority": "x" },
        { "id": "t10", "condition": "o", "sourceStepId": "a", "targetStepId": "c", "priority": "1.5" }
      ]
    }"#;
    let issues = validate(&program(text)).expect("an SFC");
    assert_eq!(
        ids(&issues),
        ["priority_t4", "priority_t5", "priority_t9", "priority_t10"]
    );
    assert_eq!(
        issues
            .iter()
            .map(|issue| issue.message.as_str())
            .collect::<Vec<_>>(),
        [
            "Transition priority -1 is not a whole number from 0; the lowest number is tested first.",
            "Transition priority 1.5 is not a whole number from 0; the lowest number is tested first.",
            "Transition priority \"x\" is not a whole number from 0; the lowest number is tested first.",
            "Transition priority \"1.5\" is not a whole number from 0; the lowest number is tested first."
        ]
    );
    assert!(issues
        .iter()
        .all(|issue| issue.kind == IssueKind::Transition));
    assert_eq!(issues[2].element.as_deref(), Some("t9"));
}

#[test]
fn a_split_needs_one_incoming_transition_and_only_branch_targets() {
    // Two transitions enter the split, it reaches branch A twice, never reaches B, and also leads
    // to C, which is not a branch. Worked by hand: incoming count, then branch A (twice), branch B
    // (never), then the extra target. Catches a check that counts any outgoing transition as a
    // branch.
    let text = r#"{
      "steps": [
        { "id": "i", "name": "I", "initial": true },
        { "id": "a", "name": "A" }, { "id": "b", "name": "B" }, { "id": "c", "name": "C" }
      ],
      "transitions": [
        { "id": "in1", "condition": "TRUE", "sourceStepId": "i", "targetStepId": "sp" },
        { "id": "in2", "condition": "TRUE", "sourceStepId": "c", "targetStepId": "sp" },
        { "id": "o1", "condition": "TRUE", "sourceStepId": "sp", "targetStepId": "a" },
        { "id": "o2", "condition": "TRUE", "sourceStepId": "sp", "targetStepId": "a" },
        { "id": "o3", "condition": "TRUE", "sourceStepId": "sp", "targetStepId": "c" }
      ],
      "parallelSplits": [ { "id": "sp", "name": "S", "branchIds": ["a", "b"] } ]
    }"#;
    let issues = validate(&program(text)).expect("an SFC");
    assert_eq!(
        ids(&issues),
        [
            "split_incoming_sp",
            "split_outgoing_sp_a",
            "split_outgoing_sp_b",
            "split_extra_target_sp_o3"
        ]
    );
    assert_eq!(
        issues[3].message,
        "Parallel split S has transition o3 to non-branch target c"
    );
}

#[test]
fn a_join_needs_each_branch_once_and_a_matching_next_step() {
    // Branch A enters twice, B never, step C (not a branch) enters, and the one outgoing
    // transition goes to C while nextStepId names D. Catches a join check that ignores the
    // nextStepId/transition mismatch or a source that is not a branch.
    let text = r#"{
      "steps": [
        { "id": "a", "name": "A", "initial": true }, { "id": "b", "name": "B" },
        { "id": "c", "name": "C" }, { "id": "d", "name": "D" }
      ],
      "transitions": [
        { "id": "j1", "condition": "TRUE", "sourceStepId": "a", "targetStepId": "jn" },
        { "id": "j2", "condition": "TRUE", "sourceStepId": "a", "targetStepId": "jn" },
        { "id": "j3", "condition": "TRUE", "sourceStepId": "c", "targetStepId": "jn" },
        { "id": "out", "condition": "TRUE", "sourceStepId": "jn", "targetStepId": "c" }
      ],
      "parallelJoins": [ { "id": "jn", "name": "J", "branchIds": ["a", "b"], "nextStepId": "d" } ]
    }"#;
    let issues = validate(&program(text)).expect("an SFC");
    assert_eq!(
        ids(&issues),
        [
            "join_incoming_jn_a",
            "join_incoming_jn_b",
            "join_extra_source_jn_j3",
            "join_next_mismatch_jn"
        ]
    );
    assert_eq!(
        issues[3].message,
        "Parallel join J nextStepId (d) does not match outgoing transition target (c)"
    );
}

#[test]
fn a_file_that_is_no_sfc_is_refused() {
    // Catches a check that reports "no initial step" for a file without steps instead of saying
    // the file is not an SFC, or that crashes on a split without branches.
    assert!(validate(&program("{\"steps\": {}, \"transitions\": []}")).is_err());
    assert!(validate(&program("[]")).is_err());
    let message = validate(&program(
        "{\"steps\": [], \"transitions\": [], \"parallelSplits\": [{\"id\": \"s\", \"name\": \"S\"}]}",
    ))
    .expect_err("a split without branchIds");
    assert!(
        message.contains("Parallel split S needs a branchIds array"),
        "{message}"
    );
}

#[test]
fn findings_serialise_as_the_editor_reads_them() {
    // The editor's validation list reads `id`, `type`, `message` and `elementId`; a finding without
    // an element has no `elementId`.
    let issue = Issue {
        id: "no_initial_step".to_owned(),
        kind: IssueKind::Step,
        message: "SFC must have at least one initial step".to_owned(),
        element: None,
    };
    assert_eq!(
        serde_json::to_string(&issue).expect("serialises"),
        "{\"id\":\"no_initial_step\",\"type\":\"step\",\"message\":\"SFC must have at least one initial step\"}"
    );
}
