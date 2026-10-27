//! Tests of the SFC file <-> canvas graph rules (specification 25 §11.4). The small chart is
//! worked out by hand from the rules in `graph.rs`; the examples are compared with what the
//! TypeScript editor built and saved before the move (`fixtures/*.graph.json`, `*.saved.json`,
//! made from the HEAD 6921e0d6c `sfc/webview/hooks/useSfc.ts` with the drawing-only parts left
//! out of the graph).

use super::*;

const NOW: &str = "2026-10-26T12:00:00.000Z";

fn json(text: &str) -> Json {
    Json::parse(text).expect("test JSON")
}

/// Init splits into Left and Right, which join back into Init. Zero and missing positions,
/// an empty transition name, a `null` priority and a join that names a third branch.
const CHART: &str = r#"{
  "name": "G",
  "steps": [
    { "id": "s0", "name": "Init", "initial": true, "x": 0, "y": 0,
      "actions": [ { "id": "a1", "name": "Go", "qualifier": "N", "body": "x := 1;" } ] },
    { "id": "s1", "name": "Left", "x": 40, "y": 200 },
    { "id": "s2", "name": "Right", "x": 160, "y": 200 }
  ],
  "transitions": [
    { "id": "t0", "name": "", "condition": "start", "sourceStepId": "s0", "targetStepId": "sp", "priority": null },
    { "id": "t1", "name": "ToRight", "condition": "TRUE", "sourceStepId": "sp", "targetStepId": "s2", "priority": 2 },
    { "id": "t2", "sourceStepId": "sp", "targetStepId": "s1" },
    { "id": "t3", "condition": "done", "sourceStepId": "s1", "targetStepId": "jn" },
    { "id": "t4", "condition": "done", "sourceStepId": "s2", "targetStepId": "jn" },
    { "id": "t5", "condition": "TRUE", "sourceStepId": "jn", "targetStepId": "s0" }
  ],
  "parallelSplits": [ { "id": "sp", "name": "Split", "x": 100, "y": 0, "branchIds": ["s1", "s2"] } ],
  "parallelJoins": [ { "id": "jn", "name": "Join", "y": 0, "branchIds": ["s1", "s2", "s9"], "nextStepId": "s0" } ],
  "variables": [ { "name": "x", "type": "INT" } ],
  "metadata": { "author": "kept nowhere", "created": "2026-01-01" }
}"#;

#[test]
fn a_chart_becomes_the_canvas_graph() {
    // Worked by hand: a zero or missing position falls back (steps 250/50, splits 250/150, joins
    // 250/300); a branch keeps its place in branchIds as `branch-out-<i>` / `branch-in-<i>`; a
    // transition is labelled by its name, else its condition; a missing condition is TRUE; a null
    // priority is dropped; the join's three branchIds give three handles. Catches a graph that
    // loses the branch order, keeps a zero position, or labels an unnamed transition "T<n>".
    let expected = json(
        r#"{
      "nodes": [
        { "id": "s0", "type": "step", "position": { "x": 250, "y": 50 },
          "data": { "label": "Init", "type": "initial",
                    "actions": [ { "id": "a1", "name": "Go", "qualifier": "N", "body": "x := 1;" } ] } },
        { "id": "s1", "type": "step", "position": { "x": 40, "y": 200 },
          "data": { "label": "Left", "type": "normal", "actions": [] } },
        { "id": "s2", "type": "step", "position": { "x": 160, "y": 200 },
          "data": { "label": "Right", "type": "normal", "actions": [] } },
        { "id": "sp", "type": "parallelSplit", "position": { "x": 100, "y": 150 },
          "data": { "label": "Split", "nodeType": "parallelSplit", "branchCount": 2 } },
        { "id": "jn", "type": "parallelJoin", "position": { "x": 250, "y": 300 },
          "data": { "label": "Join", "nodeType": "parallelJoin", "branchCount": 3 } }
      ],
      "edges": [
        { "id": "t0", "source": "s0", "target": "sp", "type": "transition",
          "data": { "condition": "start", "label": "start" }, "label": "start", "targetHandle": "in" },
        { "id": "t1", "source": "sp", "target": "s2", "type": "transition",
          "data": { "condition": "TRUE", "label": "ToRight", "priority": 2 }, "label": "ToRight",
          "sourceHandle": "branch-out-1" },
        { "id": "t2", "source": "sp", "target": "s1", "type": "transition",
          "data": { "condition": "TRUE", "label": "TRUE" }, "sourceHandle": "branch-out-0" },
        { "id": "t3", "source": "s1", "target": "jn", "type": "transition",
          "data": { "condition": "done", "label": "done" }, "label": "done", "targetHandle": "branch-in-0" },
        { "id": "t4", "source": "s2", "target": "jn", "type": "transition",
          "data": { "condition": "done", "label": "done" }, "label": "done", "targetHandle": "branch-in-1" },
        { "id": "t5", "source": "jn", "target": "s0", "type": "transition",
          "data": { "condition": "TRUE", "label": "TRUE" }, "label": "TRUE", "sourceHandle": "out" }
      ],
      "variables": [ { "name": "x", "type": "INT" } ]
    }"#,
    );
    assert_eq!(graph(&json(CHART)), Ok(expected));
}

#[test]
fn the_graph_is_saved_as_the_editor_saved_it() {
    // Worked by hand from the graph above: unnamed transitions take their label, the split's
    // branches come back in handle order (t2 on branch 0 before t1 on branch 1), the join keeps the
    // two branches that have transitions and takes its next step from the `out` transition, and the
    // file gets the name SFC_Program and the metadata { modified, version }. Catches a save that
    // orders branches by transition order, keeps the dangling branch s9 or the old metadata.
    let saved = file(&graph(&json(CHART)).expect("graph"), NOW).expect("file");
    let expected = json(
        r#"{
      "name": "SFC_Program",
      "steps": [
        { "id": "s0", "name": "Init", "initial": true, "x": 250, "y": 50,
          "actions": [ { "id": "a1", "name": "Go", "qualifier": "N", "body": "x := 1;" } ] },
        { "id": "s1", "name": "Left", "initial": false, "x": 40, "y": 200, "actions": [] },
        { "id": "s2", "name": "Right", "initial": false, "x": 160, "y": 200, "actions": [] }
      ],
      "transitions": [
        { "id": "t0", "name": "start", "condition": "start", "sourceStepId": "s0", "targetStepId": "sp" },
        { "id": "t1", "name": "ToRight", "condition": "TRUE", "sourceStepId": "sp", "targetStepId": "s2", "priority": 2 },
        { "id": "t2", "name": "TRUE", "condition": "TRUE", "sourceStepId": "sp", "targetStepId": "s1" },
        { "id": "t3", "name": "done", "condition": "done", "sourceStepId": "s1", "targetStepId": "jn" },
        { "id": "t4", "name": "done", "condition": "done", "sourceStepId": "s2", "targetStepId": "jn" },
        { "id": "t5", "name": "TRUE", "condition": "TRUE", "sourceStepId": "jn", "targetStepId": "s0" }
      ],
      "parallelSplits": [ { "id": "sp", "name": "Split", "x": 100, "y": 150, "branchIds": ["s1", "s2"] } ],
      "parallelJoins": [ { "id": "jn", "name": "Join", "x": 250, "y": 300, "branchIds": ["s1", "s2"], "nextStepId": "s0" } ],
      "variables": [ { "name": "x", "type": "INT" } ],
      "metadata": { "modified": "2026-10-26T12:00:00.000Z", "version": "1.0" }
    }"#,
    );
    assert_eq!(saved, expected);
}

#[test]
fn edits_on_the_canvas_reach_the_file() {
    // A graph as the canvas holds it after editing: a transition without a label is named T<n>
    // after its place, an empty condition becomes TRUE, drawing-only members (step handles,
    // `selected`, `measured`) are not saved, and without splits or joins those members are left
    // out. Catches a save that writes the canvas state or an empty parallelSplits list.
    let canvas = json(
        r#"{
      "nodes": [
        { "id": "a", "type": "step", "position": { "x": 10.5, "y": 20 }, "selected": true,
          "measured": { "width": 200, "height": 56 }, "data": { "label": "A", "type": "initial" } },
        { "id": "b", "type": "step", "position": { "x": 10.5, "y": 170 }, "data": { "label": "B", "type": "normal", "actions": [] } }
      ],
      "edges": [
        { "id": "e1", "source": "a", "target": "b", "sourceHandle": "step-source-bottom",
          "targetHandle": "step-target-top", "data": { "condition": "", "label": "" } }
      ]
    }"#,
    );
    let saved = file(&canvas, NOW).expect("file");
    assert_eq!(
        saved.to_pretty(),
        "{\n  \"name\": \"SFC_Program\",\n  \"steps\": [\n    {\n      \"id\": \"a\",\n      \"name\": \"A\",\n      \"initial\": true,\n      \"x\": 10.5,\n      \"y\": 20,\n      \"actions\": []\n    },\n    {\n      \"id\": \"b\",\n      \"name\": \"B\",\n      \"initial\": false,\n      \"x\": 10.5,\n      \"y\": 170,\n      \"actions\": []\n    }\n  ],\n  \"transitions\": [\n    {\n      \"id\": \"e1\",\n      \"name\": \"T1\",\n      \"condition\": \"TRUE\",\n      \"sourceStepId\": \"a\",\n      \"targetStepId\": \"b\"\n    }\n  ],\n  \"variables\": [],\n  \"metadata\": {\n    \"modified\": \"2026-10-26T12:00:00.000Z\",\n    \"version\": \"1.0\"\n  }\n}"
    );
}

#[test]
fn a_file_or_graph_of_another_shape_is_refused() {
    // Catches a reader that turns a file without steps into an empty chart the editor would then
    // save over the user's file.
    assert!(graph(&json("{\"steps\": [], \"transitions\": {}}")).is_err());
    assert!(graph(&json(
        "{\"steps\": [], \"transitions\": [], \"parallelJoins\": {}}"
    ))
    .is_err());
    assert!(file(&json("{\"nodes\": []}"), NOW).is_err());
}

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

fn example(file: &str) -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/sfc/");
    std::fs::read_to_string(format!("{path}{file}")).expect("example SFC")
}

fn fixtures(base: &str) -> (&'static str, &'static str) {
    match base {
        "ethercat-snake-advanced" => (
            include_str!("fixtures/ethercat-snake-advanced.graph.json"),
            include_str!("fixtures/ethercat-snake-advanced.saved.json"),
        ),
        "ethercat-snake-simple" => (
            include_str!("fixtures/ethercat-snake-simple.graph.json"),
            include_str!("fixtures/ethercat-snake-simple.saved.json"),
        ),
        "sfc_parallel_branches_example" => (
            include_str!("fixtures/sfc_parallel_branches_example.graph.json"),
            include_str!("fixtures/sfc_parallel_branches_example.saved.json"),
        ),
        "sfc_simple_parallel" => (
            include_str!("fixtures/sfc_simple_parallel.graph.json"),
            include_str!("fixtures/sfc_simple_parallel.saved.json"),
        ),
        _ => unreachable!("no fixture for {base}"),
    }
}

#[test]
fn the_examples_open_and_save_as_the_typescript_editor_did() {
    // Opening then saving an example writes byte for byte what the editor wrote before the move.
    // Catches any drift in member order, number layout, defaults or branch handles on real charts.
    for (name, base) in EXAMPLES {
        let (graph_fixture, saved_fixture) = fixtures(base);
        let canvas = graph(&json(&example(name))).expect("graph");
        assert_eq!(canvas, json(graph_fixture), "{name}: graph");
        assert_eq!(
            file(&canvas, NOW).expect("file").to_pretty(),
            saved_fixture,
            "{name}: saved"
        );
    }
}

#[test]
fn saving_over_a_file_keeps_its_name_and_metadata() {
    // The canvas has no name or metadata, so a save used to write `SFC_Program` and
    // `{ modified, version: "1.0" }` and drop the chart's name, author, description and created.
    // Saved over the chart, the file keeps them; `modified` is the save time. Catches that loss.
    let mut saved = file(&graph(&json(CHART)).expect("graph"), NOW).expect("file");
    let previous = json(
        r#"{"name":"Filler","metadata":{"author":"me","version":"1.0.0","description":"d","created":"c","modified":"old"}}"#,
    );
    keep_identity(&mut saved, &previous, NOW);
    assert_eq!(saved.str_at("name"), Some("Filler"));
    assert_eq!(
        saved.get("metadata").map(Json::to_compact).as_deref(),
        Some(
            r#"{"author":"me","version":"1.0.0","description":"d","created":"c","modified":"2026-10-26T12:00:00.000Z"}"#
        )
    );
    // A file without a name or metadata leaves the defaults; one without a version gets 1.0.
    let mut plain = file(&graph(&json(CHART)).expect("graph"), NOW).expect("file");
    keep_identity(
        &mut plain,
        &json(r#"{"name":"","metadata":{"author":"x"}}"#),
        NOW,
    );
    assert_eq!(plain.str_at("name"), Some("SFC_Program"));
    assert_eq!(
        plain.get("metadata").map(Json::to_compact).as_deref(),
        Some(r#"{"author":"x","modified":"2026-10-26T12:00:00.000Z","version":"1.0"}"#)
    );
}
