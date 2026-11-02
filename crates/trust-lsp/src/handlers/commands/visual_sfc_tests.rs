//! The SFC commands at the protocol boundary (specification 25 §11.4): the shape of each answer the
//! SFC editor's host reads. The SFC rules themselves are tested in `trust-visual` (`sfc`).

use serde_json::{json, Value};

use super::*;

/// A splits into B (one branch only); B is the open end of the chart.
const CHART: &str = r#"{
  "name": "Line",
  "steps": [
    { "id": "a", "name": "A", "initial": true, "x": 0, "y": 0 },
    { "id": "b", "name": "B", "x": 0, "y": 200 }
  ],
  "transitions": [ { "id": "t", "condition": "go", "sourceStepId": "a", "targetStepId": "sp" } ],
  "parallelSplits": [ { "id": "sp", "name": "S", "branchIds": ["b"] } ]
}"#;

fn graph_text() -> String {
    let answer = execute(
        VISUAL_PARSE_COMMAND,
        vec![json!({ "kind": "sfc", "text": CHART })],
    );
    answer["graphText"]
        .as_str()
        .unwrap_or_else(|| panic!("a graph: {answer}"))
        .to_owned()
}

fn edit(arguments: Value) -> Value {
    execute(VISUAL_EDIT_COMMAND, vec![arguments])
}

#[test]
fn the_graph_travels_as_text_and_formats_back_to_the_saved_file() {
    // The graph keeps the file's member order only as text (serde_json's map would sort it); the
    // file comes back in the editor's two-space layout with the name SFC_Program and the metadata
    // the host's time stamps. A wrong answer would make the editor reorder a user's file.
    let graph = graph_text();
    assert!(
        graph.starts_with(
            "{\"nodes\":[{\"id\":\"a\",\"type\":\"step\",\"position\":{\"x\":250,\"y\":50}"
        ),
        "{graph}"
    );
    let saved = execute(
        VISUAL_FORMAT_COMMAND,
        vec![json!({ "kind": "sfc", "graphText": graph, "now": "2026-10-26T12:00:00.000Z" })],
    );
    assert_eq!(saved["ok"], json!(true), "{saved}");
    let text = saved["text"].as_str().expect("file text");
    assert!(
        text.starts_with("{\n  \"name\": \"SFC_Program\",\n  \"steps\": [\n"),
        "{text}"
    );
    assert!(
        text.ends_with(
            "\"metadata\": {\n    \"modified\": \"2026-10-26T12:00:00.000Z\",\n    \"version\": \"1.0\"\n  }\n}"
        ),
        "{text}"
    );
    let broken = execute(
        VISUAL_PARSE_COMMAND,
        vec![json!({ "kind": "sfc", "text": "{\"steps\": 1}" })],
    );
    assert_eq!(broken["ok"], json!(false));
    assert!(broken["errors"][0]["message"].is_string(), "{broken}");
}

#[test]
fn canvas_edits_answer_what_changes() {
    // connect: the new edge, or `refused`; setBranchCount: the count and the removed edges;
    // addNode: the node and the wiring edge from the open end (B). An unknown node type is refused.
    let graph = graph_text();
    let connected = edit(json!({
        "kind": "sfc", "op": "connect", "graphText": graph,
        "connection": { "source": "sp", "target": "a", "sourceHandle": "branch-out-1", "targetHandle": "step-target-top" },
        "fallback": { "sourceHandle": null, "targetHandle": null }, "stamp": "5"
    }));
    assert_eq!(connected["ok"], json!(true), "{connected}");
    assert_eq!(connected["edge"]["id"], json!("trans_5"));
    assert_eq!(
        connected["edge"]["data"],
        json!({ "condition": "TRUE", "label": "TRUE" })
    );
    let refused = edit(json!({
        "kind": "sfc", "op": "connect", "graphText": graph,
        "connection": { "source": "sp", "target": "a", "sourceHandle": "branch-out-2" },
        "fallback": {}, "stamp": "6"
    }));
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["error"], json!("refused"));

    let counted = edit(json!({
        "kind": "sfc", "op": "setBranchCount", "graphText": graph, "nodeId": "sp", "branchCount": 3.7
    }));
    assert_eq!(
        counted,
        json!({ "ok": true, "branchCount": 3, "removedEdgeIds": [] })
    );

    let added = edit(json!({
        "kind": "sfc", "op": "addNode", "graphText": graph, "nodeType": "step",
        "position": { "x": 250, "y": 350 }, "wire": true, "stamp": "9"
    }));
    assert_eq!(added["ok"], json!(true), "{added}");
    assert_eq!(added["node"]["id"], json!("step_9"));
    assert_eq!(added["node"]["data"]["label"], json!("Step3"));
    assert_eq!(added["edge"]["source"], json!("b"));
    let unknown = edit(json!({
        "kind": "sfc", "op": "addNode", "graphText": graph, "nodeType": "transition",
        "position": { "x": 0, "y": 0 }, "wire": false, "stamp": "1"
    }));
    assert_eq!(unknown["ok"], json!(false));
}

#[test]
fn deleting_a_selection_answers_the_nodes_and_transitions_that_go() {
    // deleteSelection: the selected nodes and every transition into or out of them, by id; a
    // wrong argument shape is refused. The rule is trust-visual's test.
    let graph = graph_text();
    let deleted = edit(json!({
        "kind": "sfc", "op": "deleteSelection", "graphText": graph, "nodeIds": ["sp"], "edgeIds": []
    }));
    assert_eq!(deleted["ok"], json!(true), "{deleted}");
    assert_eq!(deleted["removedNodeIds"], json!(["sp"]));
    assert!(
        deleted["removedEdgeIds"]
            .as_array()
            .is_some_and(|ids| !ids.is_empty()),
        "{deleted}"
    );
    let shapeless = edit(json!({
        "kind": "sfc", "op": "deleteSelection", "graphText": graph, "nodeIds": "sp", "edgeIds": []
    }));
    assert_eq!(shapeless["ok"], json!(false), "{shapeless}");
}

#[test]
fn validate_answers_the_findings_the_editor_lists() {
    // The split has one branch and never reaches it: two findings, each with the editor's
    // `id`, `type`, `message` and `elementId`, and the same messages as `errors`.
    let answer = execute(
        VISUAL_VALIDATE_COMMAND,
        vec![json!({ "kind": "sfc", "text": CHART })],
    );
    assert_eq!(answer["ok"], json!(false), "{answer}");
    assert_eq!(answer["issues"][0]["id"], json!("split_branch_count_sp"));
    assert_eq!(answer["issues"][0]["type"], json!("connection"));
    assert_eq!(answer["issues"][0]["elementId"], json!("sp"));
    assert_eq!(answer["issues"][1]["id"], json!("split_outgoing_sp_b"));
    assert_eq!(
        answer["errors"][0]["message"],
        json!("Parallel split S must define at least 2 branches")
    );
}
