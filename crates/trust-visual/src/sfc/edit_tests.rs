//! Tests of the SFC canvas edits (specification 25 §11.4). Every expected value is worked out by
//! hand from the rules in `edit.rs`: a parallel split has one input and starts each branch once, a
//! parallel join ends each branch once and has one output, and a parallel node keeps at least two
//! branches.

use super::*;

fn json(text: &str) -> Json {
    Json::parse(text).expect("test JSON")
}

/// A splits into B (branch 0); B ends branch 0 of the join; C is loose.
const CANVAS: &str = r#"{
  "nodes": [
    { "id": "a", "type": "step", "position": { "x": 250, "y": 50 }, "data": { "label": "A", "type": "initial", "actions": [] } },
    { "id": "b", "type": "step", "position": { "x": 250, "y": 200 }, "data": { "label": "B", "type": "normal", "actions": [] } },
    { "id": "c", "type": "step", "position": { "x": 400, "y": 200 }, "data": { "label": "C", "type": "normal", "actions": [] } },
    { "id": "sp", "type": "parallelSplit", "position": { "x": 250, "y": 120 }, "data": { "label": "S", "nodeType": "parallelSplit", "branchCount": 2 } },
    { "id": "jn", "type": "parallelJoin", "position": { "x": 250, "y": 300 }, "data": { "label": "J", "nodeType": "parallelJoin", "branchCount": 2 } }
  ],
  "edges": [
    { "id": "e1", "source": "a", "target": "sp", "sourceHandle": "step-source-bottom", "targetHandle": "in", "type": "transition", "data": { "condition": "TRUE", "label": "TRUE" } },
    { "id": "e2", "source": "sp", "target": "b", "sourceHandle": "branch-out-0", "targetHandle": "step-target-top", "type": "transition", "data": { "condition": "TRUE", "label": "TRUE" } },
    { "id": "e3", "source": "b", "target": "jn", "sourceHandle": "step-source-bottom", "targetHandle": "branch-in-0", "type": "transition", "data": { "condition": "TRUE", "label": "TRUE" } }
  ]
}"#;

/// `CANVAS` with more edges.
fn canvas_with(extra_edges: &str) -> Json {
    json(&CANVAS.replacen("\n  ]\n}", &format!(",\n    {extra_edges}\n  ]\n}}"), 1))
}

const NO_FALLBACK: &str = r#"{ "sourceHandle": null, "targetHandle": null }"#;

fn connect_on(canvas: &Json, connection: &str, fallback: &str) -> Result<Json, String> {
    connect(canvas, &json(connection), &json(fallback), "1700")
}

#[test]
fn a_drawn_transition_becomes_a_true_edge_on_its_handles() {
    // Catches an edge that loses the handles the user drew, or a condition other than TRUE.
    let edge = connect_on(
        &json(CANVAS),
        r#"{ "source": "b", "target": "c", "sourceHandle": "step-source-right", "targetHandle": "step-target-left" }"#,
        r#"{ "sourceHandle": "step-source-bottom", "targetHandle": "step-target-top" }"#,
    );
    assert_eq!(
        edge,
        Ok(json(
            r#"{ "id": "trans_1700", "source": "b", "target": "c", "sourceHandle": "step-source-right",
                 "targetHandle": "step-target-left", "type": "transition",
                 "data": { "condition": "TRUE", "label": "TRUE" }, "label": "TRUE" }"#
        ))
    );
}

#[test]
fn an_end_without_a_handle_takes_the_drawn_fallback_and_null_is_left_out() {
    // A connection without handles takes the step handles the editor drew; a null handle (an end
    // that is not a step) is left out of the edge. Catches an edge written with `null` handles.
    let edge = connect_on(
        &json(CANVAS),
        r#"{ "source": "c", "target": "a", "sourceHandle": null, "targetHandle": null }"#,
        r#"{ "sourceHandle": "step-source-left", "targetHandle": "step-target-left" }"#,
    )
    .expect("edge");
    assert_eq!(edge.str_at("sourceHandle"), Some("step-source-left"));
    assert_eq!(edge.str_at("targetHandle"), Some("step-target-left"));
    let edge = connect_on(
        &json(CANVAS),
        r#"{ "source": "sp", "target": "c", "sourceHandle": "branch-out-1", "targetHandle": null }"#,
        NO_FALLBACK,
    )
    .expect("edge");
    assert_eq!(edge.str_at("sourceHandle"), Some("branch-out-1"));
    assert_eq!(edge.get("targetHandle"), None);
}

#[test]
fn connections_that_break_the_chart_are_refused() {
    // Each refusal worked by hand from one rule. Catches a missing rule: a loop on one node, a
    // second input to the split, a branch handle beyond the branch count, a reused branch, a join
    // leaving through a branch handle, a second output of the join.
    let canvas = json(CANVAS);
    let refused = [
        (r#"{ "source": "a", "target": null }"#, "no target"),
        (r#"{ "source": "a", "target": "a" }"#, "a loop on one node"),
        (r#"{ "source": "a", "target": "zz" }"#, "an unknown node"),
        (
            r#"{ "source": "sp", "target": "c", "sourceHandle": "in" }"#,
            "a split leaving by its input",
        ),
        (
            r#"{ "source": "sp", "target": "c", "sourceHandle": "branch-out-2" }"#,
            "a third branch of two",
        ),
        (
            r#"{ "source": "sp", "target": "c", "sourceHandle": "branch-out-0" }"#,
            "a reused branch",
        ),
        (
            r#"{ "source": "jn", "target": "c", "sourceHandle": "branch-in-1" }"#,
            "a join leaving by a branch",
        ),
        (
            r#"{ "source": "c", "target": "sp", "targetHandle": "branch-out-1" }"#,
            "a split entered by a branch",
        ),
        (
            r#"{ "source": "b", "target": "sp", "targetHandle": "in" }"#,
            "a second split input",
        ),
        (
            r#"{ "source": "c", "target": "jn", "targetHandle": "branch-in-2" }"#,
            "a third join branch of two",
        ),
        (
            r#"{ "source": "c", "target": "jn", "targetHandle": "branch-in-0" }"#,
            "a reused join branch",
        ),
    ];
    for (connection, why) in refused {
        assert!(
            connect_on(&canvas, connection, NO_FALLBACK).is_err(),
            "{why} is refused"
        );
    }
    let to_join = connect_on(
        &canvas,
        r#"{ "source": "c", "target": "jn", "sourceHandle": "step-source-bottom", "targetHandle": "branch-in-1" }"#,
        NO_FALLBACK,
    );
    assert!(
        to_join.is_ok(),
        "the free branch of the join is accepted: {to_join:?}"
    );
    let join_out = connect_on(
        &canvas,
        r#"{ "source": "jn", "target": "c", "sourceHandle": "out" }"#,
        NO_FALLBACK,
    );
    assert!(
        join_out.is_ok(),
        "the join's free output is accepted: {join_out:?}"
    );
}

#[test]
fn a_join_has_one_output() {
    // With jn -> a on `out`, a second output is refused. Catches a join with two continuations.
    let canvas = canvas_with(
        r#"{ "id": "e4", "source": "jn", "target": "a", "sourceHandle": "out", "type": "transition", "data": { "condition": "TRUE" } }"#,
    );
    assert!(connect_on(
        &canvas,
        r#"{ "source": "jn", "target": "c", "sourceHandle": "out" }"#,
        NO_FALLBACK
    )
    .is_err());
}

#[test]
fn the_same_transition_is_not_drawn_twice() {
    // The same ends on the same handles exist already: refused. The same ends on other handles are
    // a second transition: accepted, also when the handles come from the editor's drawn fallback.
    // With no handle at all, a missing handle matches the empty one of e5: refused. Catches a
    // duplicate check that compares ends only, or handles only exactly.
    let canvas = canvas_with(
        r#"{ "id": "e4", "source": "b", "target": "c", "sourceHandle": "step-source-right", "targetHandle": "step-target-left", "type": "transition", "data": {} },
    { "id": "e5", "source": "c", "target": "b", "sourceHandle": "", "type": "transition", "data": {} }"#,
    );
    assert!(connect_on(
        &canvas,
        r#"{ "source": "b", "target": "c", "sourceHandle": "step-source-right", "targetHandle": "step-target-left" }"#,
        NO_FALLBACK
    )
    .is_err());
    assert!(connect_on(
        &canvas,
        r#"{ "source": "b", "target": "c", "sourceHandle": "step-source-bottom", "targetHandle": "step-target-top" }"#,
        NO_FALLBACK
    )
    .is_ok());
    assert!(connect_on(
        &canvas,
        r#"{ "source": "c", "target": "b", "sourceHandle": null, "targetHandle": null }"#,
        r#"{ "sourceHandle": "step-source-bottom", "targetHandle": "step-target-top" }"#
    )
    .is_ok());
    assert!(connect_on(
        &canvas,
        r#"{ "source": "c", "target": "b", "sourceHandle": null, "targetHandle": null }"#,
        NO_FALLBACK
    )
    .is_err());
}

#[test]
fn a_branch_count_is_at_least_two_and_drops_transitions_beyond_it() {
    // 3.9 rounds down to 3, 1.7 and anything that is not a finite number give 2. Lowering the
    // split to 2 removes only its own transition on branch 2; lowering the join removes its
    // transition into branch 2. Catches a count of 1, a rounded-up count, or pruning another
    // node's branch.
    let canvas = canvas_with(
        r#"{ "id": "x2", "source": "sp", "target": "c", "sourceHandle": "branch-out-2", "type": "transition", "data": {} },
    { "id": "y2", "source": "sp2", "target": "c", "sourceHandle": "branch-out-2", "type": "transition", "data": {} },
    { "id": "j2", "source": "c", "target": "jn", "targetHandle": "branch-in-2", "type": "transition", "data": {} }"#,
    );
    // The counts are compared as JSON numbers, as the editor receives them.
    let count = |value: Option<&str>| {
        let value = value.map(json);
        Json::Number(
            set_branch_count(&canvas, "sp", value.as_ref())
                .expect("count")
                .branch_count,
        )
    };
    assert_eq!(count(Some("3.9")), Json::Number(3.0));
    assert_eq!(count(Some("1.7")), Json::Number(2.0));
    assert_eq!(count(Some("null")), Json::Number(2.0));
    assert_eq!(count(Some("\"5\"")), Json::Number(2.0));
    assert_eq!(count(None), Json::Number(2.0));
    assert_eq!(
        set_branch_count(&canvas, "sp", Some(&json("2")))
            .expect("count")
            .removed_edges,
        vec![Json::str("x2")]
    );
    assert_eq!(
        set_branch_count(&canvas, "sp", Some(&json("3")))
            .expect("count")
            .removed_edges,
        Vec::<Json>::new()
    );
    assert_eq!(
        set_branch_count(&canvas, "jn", Some(&json("1")))
            .expect("count")
            .removed_edges,
        vec![Json::str("j2")]
    );
}

fn position() -> Json {
    json(r#"{ "x": 250, "y": 350 }"#)
}

#[test]
fn a_step_added_from_the_toolbar_is_wired_onto_the_open_end_of_the_chart() {
    // A and B have outgoing transitions, C has none: C is the open end, so the new fourth step
    // ("Step4") gets a TRUE transition from C. Catches wiring from the lowest step regardless of
    // the open end, or a wrong step number.
    let added = add_node(&json(CANVAS), NodeType::Step, &position(), true, "42").expect("node");
    assert_eq!(
        added.node,
        json(
            r#"{ "id": "step_42", "type": "step", "position": { "x": 250, "y": 350 },
                 "data": { "label": "Step4", "type": "normal", "actions": [] } }"#
        )
    );
    assert_eq!(
        added.edge,
        Some(json(
            r#"{ "id": "trans_42", "source": "c", "target": "step_42", "type": "transition",
                 "data": { "condition": "TRUE", "label": "TRUE" }, "label": "TRUE" }"#
        ))
    );
}

#[test]
fn the_open_end_is_the_lowest_open_step_and_the_first_of_equals() {
    // B and C are both open at y 200 (A's only transition goes to the split): B, the first of the
    // two, is the open end. With every step closed, the lowest step of all is used. Catches the
    // last of equals or the highest step being chosen.
    let open = json(&CANVAS.replace(
        r#""id": "e3", "source": "b""#,
        r#""id": "e3", "source": "sp""#,
    ));
    let added = add_node(&open, NodeType::Step, &position(), true, "1").expect("node");
    assert_eq!(
        added.edge.and_then(|edge| edge.get("source").cloned()),
        Some(Json::str("b"))
    );
    let closed = canvas_with(
        r#"{ "id": "e4", "source": "c", "target": "a", "type": "transition", "data": {} }"#,
    );
    let added = add_node(&closed, NodeType::Step, &position(), true, "2").expect("node");
    assert_eq!(
        added.edge.and_then(|edge| edge.get("source").cloned()),
        Some(Json::str("b"))
    );
}

#[test]
fn only_a_toolbar_step_is_wired() {
    // A dropped step (no wire), a split and a join are added alone; a chart without steps has no
    // open end. Catches an edge created for a parallel node.
    let canvas = json(CANVAS);
    assert_eq!(
        add_node(&canvas, NodeType::Step, &position(), false, "3")
            .expect("node")
            .edge,
        None
    );
    let split = add_node(&canvas, NodeType::ParallelSplit, &position(), true, "4").expect("node");
    assert_eq!(
        split.node,
        json(
            r#"{ "id": "split_4", "type": "parallelSplit", "position": { "x": 250, "y": 350 },
                 "data": { "label": "Parallel Split", "nodeType": "parallelSplit", "branchCount": 2 } }"#
        )
    );
    assert_eq!(split.edge, None);
    let join = add_node(&canvas, NodeType::ParallelJoin, &position(), true, "5").expect("node");
    assert_eq!(join.node.str_at("id"), Some("join_5"));
    assert_eq!(join.edge, None);
    let empty = json(r#"{ "nodes": [], "edges": [] }"#);
    let first = add_node(&empty, NodeType::Step, &position(), true, "6").expect("node");
    assert_eq!(first.edge, None);
    assert_eq!(
        member(Some(&first.node), "data").and_then(|data| data.str_at("label")),
        Some("Step1")
    );
    assert_eq!(
        NodeType::from_name("parallelJoin"),
        Some(NodeType::ParallelJoin)
    );
    assert_eq!(NodeType::from_name("transition"), None);
}

/// Deleting the split removes it with every transition into or out of it (e1 into it, e2 out of
/// it), and a selected transition goes too (e3); an id the graph does not have is left out.
/// Catches a transition left with an end that is gone, which the chart would then reach through.
#[test]
fn deleting_a_node_removes_its_transitions() {
    let canvas = json(CANVAS);
    let deleted = delete_selection(
        &canvas,
        &["sp".to_owned(), "ghost".to_owned()],
        &["e3".to_owned()],
    )
    .expect("delete");
    assert_eq!(deleted.node_ids, ["sp"]);
    assert_eq!(deleted.edge_ids, ["e1", "e2", "e3"]);
    let only_edge = delete_selection(&canvas, &[], &["e2".to_owned()]).expect("delete");
    assert_eq!(
        (only_edge.node_ids.len(), only_edge.edge_ids),
        (0, vec!["e2".to_owned()])
    );
}
