//! The edits the Ladder editor stores (specification 25 §11.4: deterministic ids, ordering,
//! declaration reconciliation, branch topology and auto-route geometry). Positions are worked out
//! by hand on the 20-unit grid with the connector offsets: contact and coil −20/+60, blocks
//! −20/+140, topology nodes 0/0.

use super::json;
use crate::ladder::edit::{self, EditError, Wire};
use crate::Json;

/// One rung `rung_1` (y 100): contact `c1` on `%IX0.0` at x 100 and coil `q1` on `%QX0.0` at
/// x 260, no wires; both addresses declared as globals named like their address.
fn sample() -> Json {
    json(
        r#"{"schemaVersion":2,"metadata":{"name":"ops","description":"ops"},
          "variables":[{"name":"%IX0.0","type":"BOOL","address":"%IX0.0"},{"name":"%QX0.0","type":"BOOL","address":"%QX0.0"}],
          "networks":[{"id":"rung_1","order":0,"layout":{"y":100},"edges":[],"nodes":[
            {"id":"c1","type":"contact","contactType":"NO","variable":"%IX0.0","position":{"x":100,"y":100}},
            {"id":"q1","type":"coil","coilType":"NORMAL","variable":"%QX0.0","position":{"x":260,"y":100}}]}]}"#,
    )
}

fn with_second_rung(mut program: Json) -> Json {
    let rung = json(
        r#"{"id":"rung_2","order":1,"layout":{"y":200},"edges":[],"nodes":[
            {"id":"c2","type":"contact","contactType":"NO","variable":"%IX0.0","position":{"x":100,"y":200}},
            {"id":"q2","type":"coil","coilType":"NORMAL","variable":"%QX0.0","position":{"x":260,"y":200}}]}"#,
    );
    if let Some(networks) = program.get_mut("networks").and_then(Json::as_array_mut) {
        networks.push(rung);
    }
    program
}

fn rung(program: &Json, index: usize) -> &Json {
    &program
        .get("networks")
        .and_then(Json::as_array)
        .expect("networks")[index]
}

fn nodes(network: &Json) -> &[Json] {
    network
        .get("nodes")
        .and_then(Json::as_array)
        .expect("nodes")
}

fn edges(network: &Json) -> &[Json] {
    network
        .get("edges")
        .and_then(Json::as_array)
        .expect("edges")
}

fn node<'a>(network: &'a Json, id: &str) -> &'a Json {
    nodes(network)
        .iter()
        .find(|node| node.str_at("id") == Some(id))
        .unwrap_or_else(|| panic!("node {id}"))
}

fn xy(node: &Json) -> (f64, f64) {
    let position = node.get("position").expect("position");
    (
        position.get("x").and_then(Json::as_f64).expect("x"),
        position.get("y").and_then(Json::as_f64).expect("y"),
    )
}

/// The wires of a network as `from>to@x1,y1;x2,y2;…`.
fn wires(network: &Json) -> Vec<String> {
    edges(network)
        .iter()
        .map(|edge| {
            let points: Vec<String> = edge
                .get("points")
                .and_then(Json::as_array)
                .map(|points| {
                    points
                        .iter()
                        .map(|point| {
                            format!(
                                "{},{}",
                                point.get("x").and_then(Json::as_f64).unwrap_or(f64::NAN),
                                point.get("y").and_then(Json::as_f64).unwrap_or(f64::NAN)
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            format!(
                "{} {}>{}@{}",
                edge.str_at("id").unwrap_or_default(),
                edge.str_at("fromNodeId").unwrap_or_default(),
                edge.str_at("toNodeId").unwrap_or_default(),
                points.join(";")
            )
        })
        .collect()
}

#[test]
fn auto_route_chains_the_nodes_by_position_on_the_grid() {
    // c1 right connector 100+60 = 160, q1 left connector 260−20 = 240, elbow on the grid point
    // nearest 200. Catches a route that ignores the connector offsets or an id with a time.
    let mut program = with_second_rung(sample());
    edit::auto_route(&mut program, Some(0));
    assert_eq!(
        wires(rung(&program, 0)),
        ["edge_rung_1_0 c1>q1@160,100;200,100;200,100;240,100"]
    );
    assert!(
        edges(rung(&program, 1)).is_empty(),
        "only the requested rung is routed"
    );
    edit::auto_route(&mut program, None);
    assert_eq!(
        wires(rung(&program, 1)),
        ["edge_rung_2_0 c2>q2@160,200;200,200;200,200;240,200"]
    );
}

#[test]
fn find_and_replace_counts_every_occurrence() {
    // `%IX0.0` occurs in the first variable's name and address and in the contact: 3. Catches a
    // replace that skips variable names or counts only fields.
    let mut program = sample();
    assert_eq!(edit::replace_symbol(&mut program, "%IX0.0", "%IX1.0"), 3);
    let variables = program
        .get("variables")
        .and_then(Json::as_array)
        .expect("variables");
    assert_eq!(variables[0].str_at("name"), Some("%IX1.0"));
    assert_eq!(variables[0].str_at("address"), Some("%IX1.0"));
    assert_eq!(
        node(rung(&program, 0), "c1").str_at("variable"),
        Some("%IX1.0")
    );
    assert_eq!(edit::replace_symbol(&mut program, "", "x"), 0);
    assert_eq!(edit::replace_symbol(&mut program, "nothing", "x"), 0);
}

#[test]
fn reconcile_keeps_the_typed_symbol_and_drops_the_typing_artifacts() {
    // Typing `test` letter by letter left implicit locals `t`, `te`, `tes`; only `test`, which
    // the contact names, stays; an explicit local (initial TRUE) stays although no one names it.
    let mut program = sample();
    if let Some(variables) = program.get_mut("variables").and_then(Json::as_array_mut) {
        for name in ["t", "te", "tes"] {
            variables.push(json(&format!(
                r#"{{"name":"{name}","type":"BOOL","scope":"local","initialValue":false}}"#
            )));
        }
        variables.push(json(
            r#"{"name":"Configured","type":"BOOL","scope":"local","initialValue":true}"#,
        ));
    }
    if let Some(Json::Array(networks)) = program.get_mut("networks")
        && let Some(Json::Array(nodes)) = networks[0].get_mut("nodes")
    {
        nodes[0].set("variable", Json::str("test"));
    }
    edit::reconcile_declarations(&mut program);
    let names: Vec<&str> = program
        .get("variables")
        .and_then(Json::as_array)
        .expect("variables")
        .iter()
        .filter_map(|variable| variable.str_at("name"))
        .collect();
    assert_eq!(names, ["%IX0.0", "%QX0.0", "Configured", "test"]);
}

#[test]
fn reconcile_declares_a_qualified_global_in_its_scope() {
    // `GLOBAL::Pump` on a coil declares a global Pump even when a local Pump exists; an
    // unqualified name already declared in any scope declares nothing.
    let mut program = sample();
    if let Some(variables) = program.get_mut("variables").and_then(Json::as_array_mut) {
        variables.push(json(
            r#"{"name":"Pump","type":"BOOL","scope":"local","initialValue":true}"#,
        ));
    }
    if let Some(Json::Array(networks)) = program.get_mut("networks")
        && let Some(Json::Array(nodes)) = networks[0].get_mut("nodes")
    {
        nodes[1].set("variable", Json::str("GLOBAL::Pump"));
        nodes[0].set("variable", Json::str("pump"));
    }
    edit::reconcile_declarations(&mut program);
    let variables = program
        .get("variables")
        .and_then(Json::as_array)
        .expect("variables");
    assert_eq!(variables.len(), 4);
    assert_eq!(
        variables[3].to_compact(),
        r#"{"name":"Pump","type":"BOOL","scope":"global","initialValue":false}"#
    );
}

#[test]
fn paste_gives_a_new_id_and_moves_the_copy_right_on_the_rung() {
    // The copy of c1 (x 100) lands at the grid point nearest 140 on the rung's y; its id is the
    // smallest free `contact_<n>`. Catches a copy that keeps the id.
    let mut program = sample();
    let element = node(rung(&program, 0), "c1").clone();
    assert_eq!(edit::paste_element(&mut program, 0, &element), Ok(2));
    let copy = &nodes(rung(&program, 0))[2];
    assert_eq!(copy.str_at("id"), Some("contact_1"));
    assert_eq!(xy(copy), (140.0, 100.0));
    assert_eq!(copy.str_at("variable"), Some("%IX0.0"));
    assert!(edit::paste_element(&mut program, 5, &element).is_err());
    assert!(edit::paste_element(&mut program, 0, &json(r#"{"type":"wire"}"#)).is_err());
}

#[test]
fn paste_rung_inserts_a_copy_and_renumbers_every_rung() {
    // After rung 0: index 1; orders 0, 1 and rows 100, 200; the copy's id is `rung_2` (rung_1
    // exists), its nodes get new ids and move 100 down, its wires are dropped.
    let mut program = sample();
    if let Some(Json::Array(networks)) = program.get_mut("networks") {
        let routed = edit::routed_network(&networks[0]);
        networks[0] = routed;
    }
    let source = rung(&program, 0).clone();
    assert_eq!(edit::paste_rung(&mut program, &source, 0), 1);
    let copy = rung(&program, 1);
    assert_eq!(copy.str_at("id"), Some("rung_2"));
    assert_eq!(copy.get("order").and_then(Json::as_f64), Some(1.0));
    assert_eq!(
        copy.get("layout")
            .and_then(|l| l.get("y"))
            .and_then(Json::as_f64),
        Some(200.0)
    );
    assert_eq!(
        rung(&program, 0)
            .get("layout")
            .and_then(|l| l.get("y"))
            .and_then(Json::as_f64),
        Some(100.0)
    );
    let ids: Vec<&str> = nodes(copy).iter().filter_map(|n| n.str_at("id")).collect();
    assert_eq!(ids, ["contact_1", "coil_1"]);
    assert_eq!(xy(&nodes(copy)[0]), (100.0, 200.0));
    assert_eq!(edges(copy), []);
    // A negative position pastes before the first rung.
    assert_eq!(edit::paste_rung(&mut program, &source, -1), 0);
    assert_eq!(rung(&program, 0).str_at("id"), Some("rung_3"));
}

#[test]
fn a_parallel_contact_builds_a_branch_around_a_series_contact() {
    // c1 at x 100 has no left neighbour and q1 at 260 on its right: the split may go from 80
    // (rail 50 + 20, on the grid) and the merge up to 260 − 80 = 180. Preferred 0 and 200 clamp
    // to split 80 and merge 180. The new leg is 60 below. Wires: split→c1, split→leg, c1→merge,
    // leg→merge, merge→q1, numbered 0..4; legs turn at the split's and merge's x. Catches a
    // branch without the merge→q1 continuation or legs routed through the midpoint.
    let mut program = sample();
    let leg = edit::add_parallel_contact(&mut program, 0, "c1").expect("branch");
    assert_eq!(leg.selected_node_id, "contact_1");
    assert_eq!(leg.total_parallel_legs, 2);
    assert_eq!(leg.added_vertical_span_px, 60.0);
    assert_eq!(leg.rung_shift_px, 0.0);
    let network = rung(&program, 0);
    let ids: Vec<&str> = nodes(network)
        .iter()
        .filter_map(|n| n.str_at("id"))
        .collect();
    assert_eq!(
        ids,
        ["c1", "q1", "contact_1", "branchSplit_1", "branchMerge_1"]
    );
    assert_eq!(xy(node(network, "branchSplit_1")), (80.0, 100.0));
    assert_eq!(xy(node(network, "branchMerge_1")), (180.0, 100.0));
    assert_eq!(xy(node(network, "contact_1")), (100.0, 160.0));
    assert_eq!(node(network, "contact_1").str_at("variable"), Some(""));
    assert_eq!(node(network, "c1").str_at("variable"), Some("%IX0.0"));
    assert_eq!(
        wires(network),
        [
            "edge_rung_1_0 branchSplit_1>c1@80,100;80,100;80,100;80,100",
            "edge_rung_1_1 branchSplit_1>contact_1@80,100;80,100;80,160;80,160",
            "edge_rung_1_2 c1>branchMerge_1@160,100;180,100;180,100;180,100",
            "edge_rung_1_3 contact_1>branchMerge_1@160,160;180,160;180,100;180,100",
            "edge_rung_1_4 branchMerge_1>q1@180,100;220,100;220,100;240,100",
        ]
    );
}

#[test]
fn another_parallel_contact_adds_a_leg_below_the_lowest() {
    // On the new leg (y 160): one more leg at y 220, wired split→leg→merge as edges 5 and 6.
    let mut program = sample();
    let first = edit::add_parallel_contact(&mut program, 0, "c1").expect("branch");
    let second = edit::add_parallel_contact(&mut program, 0, &first.selected_node_id).expect("leg");
    assert_eq!(second.selected_node_id, "contact_2");
    assert_eq!(second.total_parallel_legs, 3);
    assert_eq!(second.added_vertical_span_px, 120.0);
    let network = rung(&program, 0);
    let mut ys: Vec<f64> = nodes(network)
        .iter()
        .filter(|n| n.str_at("type") == Some("contact"))
        .map(|n| xy(n).1)
        .collect();
    ys.sort_by(f64::total_cmp);
    assert_eq!(ys, [100.0, 160.0, 220.0]);
    let wires = wires(network);
    assert_eq!(
        wires[5],
        "edge_rung_1_5 branchSplit_1>contact_2@80,100;80,100;80,220;80,220"
    );
    assert_eq!(
        wires[6],
        "edge_rung_1_6 contact_2>branchMerge_1@160,220;180,220;180,100;180,100"
    );
}

#[test]
fn a_deeper_branch_pushes_the_rungs_below_down() {
    // Rung 2 is at 200. After the first leg the lowest element is at 160: the next rung must
    // start at 160 + 40 + 40 = 240, so it moves 40. After the second leg (220) it must start at
    // 300: it moves 60 more. Catches a shift that ignores the margin or leaves the nodes behind.
    let mut program = with_second_rung(sample());
    let first = edit::add_parallel_contact(&mut program, 0, "c1").expect("branch");
    assert_eq!(first.rung_shift_px, 40.0);
    assert_eq!(
        rung(&program, 1)
            .get("layout")
            .and_then(|l| l.get("y"))
            .and_then(Json::as_f64),
        Some(240.0)
    );
    assert_eq!(xy(node(rung(&program, 1), "c2")), (100.0, 240.0));
    let second = edit::add_parallel_contact(&mut program, 0, &first.selected_node_id).expect("leg");
    assert_eq!(second.rung_shift_px, 60.0);
    assert_eq!(
        rung(&program, 1)
            .get("layout")
            .and_then(|l| l.get("y"))
            .and_then(Json::as_f64),
        Some(300.0)
    );
}

#[test]
fn a_branch_that_does_not_fit_changes_nothing() {
    // Neighbours at 140 and 200 leave the split ≥ 220 and the merge ≤ 120: no room. A rung that
    // already has a junction has no plain series path. Unknown rung and element are refused.
    let mut program = json(
        r#"{"schemaVersion":2,"metadata":{"name":"c","description":""},"variables":[],
          "networks":[{"id":"cramped","order":0,"layout":{"y":100},"edges":[],"nodes":[
            {"id":"left","type":"coil","coilType":"NORMAL","variable":"%QX0.0","position":{"x":140,"y":100}},
            {"id":"c1","type":"contact","contactType":"NO","variable":"%IX0.0","position":{"x":160,"y":100}},
            {"id":"right","type":"coil","coilType":"NORMAL","variable":"%QX0.1","position":{"x":200,"y":100}}]}]}"#,
    );
    let before = program.clone();
    assert_eq!(
        edit::add_parallel_contact(&mut program, 0, "c1"),
        Err(EditError::InsufficientHorizontalSpace)
    );
    assert_eq!(program, before);
    assert_eq!(
        edit::add_parallel_contact(&mut program, 3, "c1"),
        Err(EditError::RungNotFound)
    );
    assert_eq!(
        edit::add_parallel_contact(&mut program, 0, "left"),
        Err(EditError::ContactNotFound)
    );
    let mut junction = sample();
    edit::insert_element(&mut junction, 0, "junction", 400.0, 100.0).expect("junction");
    assert_eq!(
        edit::add_parallel_contact(&mut junction, 0, "c1"),
        Err(EditError::UnsupportedTopology)
    );
    assert_eq!(
        EditError::UnsupportedTopology.code(),
        "unsupported-topology"
    );
    assert_eq!(
        EditError::InsufficientHorizontalSpace.to_string(),
        "Not enough horizontal space in this rung; move the contact right and retry."
    );
}

#[test]
fn a_new_block_names_its_instance_and_declares_its_operands() {
    // A timer gets `timer_1`, instance `T_1`, a 1000 ms TON and the locals T_1_Q (BOOL FALSE)
    // and T_1_ET (TIME 0); a second timer takes T_2. Catches a time-stamped instance or a
    // missing declaration (the companion would refuse the undeclared outputs).
    let mut program = sample();
    assert_eq!(
        edit::insert_element(&mut program, 0, "timer", 400.0, 100.0),
        Ok(2)
    );
    assert_eq!(
        nodes(rung(&program, 0))[2].to_compact(),
        r#"{"id":"timer_1","type":"timer","timerType":"TON","instance":"T_1","input":"","presetMs":1000,"qOutput":"T_1_Q","etOutput":"T_1_ET","position":{"x":400,"y":100}}"#
    );
    let variables = program
        .get("variables")
        .and_then(Json::as_array)
        .expect("variables");
    assert_eq!(
        variables[2..]
            .iter()
            .map(Json::to_compact)
            .collect::<Vec<_>>(),
        [
            r#"{"name":"T_1_Q","type":"BOOL","scope":"local","initialValue":false}"#,
            r#"{"name":"T_1_ET","type":"TIME","scope":"local","initialValue":0}"#,
        ]
    );
    edit::insert_element(&mut program, 0, "timer", 600.0, 100.0).expect("second timer");
    assert_eq!(nodes(rung(&program, 0))[3].str_at("instance"), Some("T_2"));
    edit::insert_element(&mut program, 0, "contact", 50.0, 100.0).expect("contact");
    assert_eq!(
        nodes(rung(&program, 0))[4].to_compact(),
        r#"{"id":"contact_1","type":"contact","contactType":"NO","variable":"%IX0.0","position":{"x":50,"y":100}}"#
    );
    assert!(edit::insert_element(&mut program, 0, "wire", 0.0, 0.0).is_err());
    assert!(edit::insert_element(&mut program, 9, "coil", 0.0, 0.0).is_err());
}

#[test]
fn rungs_are_added_and_removed_on_the_rung_grid() {
    // Adding at 0 puts `rung_2` first (rung_1 exists); both are renumbered to orders 0, 1 and
    // rows 100, 200. Removing the first renumbers the rest. Catches a stale order.
    let mut program = sample();
    assert_eq!(edit::add_rung(&mut program, 0), 0);
    assert_eq!(
        rung(&program, 0).to_compact(),
        r#"{"id":"rung_2","order":0,"nodes":[],"edges":[],"layout":{"y":100}}"#
    );
    assert_eq!(
        rung(&program, 1).get("order").and_then(Json::as_f64),
        Some(1.0)
    );
    assert_eq!(
        rung(&program, 1)
            .get("layout")
            .and_then(|l| l.get("y"))
            .and_then(Json::as_f64),
        Some(200.0)
    );
    assert_eq!(edit::add_rung(&mut program, 99), 2);
    assert_eq!(edit::remove_rung(&mut program, 0), Ok(()));
    assert_eq!(rung(&program, 0).str_at("id"), Some("rung_1"));
    assert_eq!(
        rung(&program, 0)
            .get("layout")
            .and_then(|l| l.get("y"))
            .and_then(Json::as_f64),
        Some(100.0)
    );
    assert!(edit::remove_rung(&mut program, 7).is_err());
}

#[test]
fn a_wire_is_added_once_on_top_of_the_routed_chain() {
    // In a rung without wires, c1→q1 is already the routed chain: nothing changes. q1→c1 routes
    // the chain first and adds `edge_rung_1_1` from q1's right connector (320) to c1's left
    // connector (80) with the elbow at 200. A wire to itself or to an unknown node is invalid.
    let mut program = sample();
    assert_eq!(
        edit::connect(&mut program, 0, "c1", "q1"),
        Wire::AlreadyConnected
    );
    assert_eq!(edges(rung(&program, 0)), []);
    assert_eq!(edit::connect(&mut program, 0, "q1", "c1"), Wire::Added);
    assert_eq!(
        wires(rung(&program, 0)),
        [
            "edge_rung_1_0 c1>q1@160,100;200,100;200,100;240,100",
            "edge_rung_1_1 q1>c1@320,100;200,100;200,100;80,100",
        ]
    );
    assert_eq!(
        edit::connect(&mut program, 0, "q1", "c1"),
        Wire::AlreadyConnected
    );
    assert_eq!(edit::connect(&mut program, 0, "c1", "c1"), Wire::Invalid);
    assert_eq!(edit::connect(&mut program, 0, "c1", "nope"), Wire::Invalid);
    assert_eq!(edit::connect(&mut program, 4, "c1", "q1"), Wire::Invalid);
}
