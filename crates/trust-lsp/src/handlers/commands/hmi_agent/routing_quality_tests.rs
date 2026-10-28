//! Tests of the routing review: `agent/routingQuality.ts` (`routingQuality`) and the body test of its
//! collision rule (specification 43, §7 items 6 and 7).
//!
//! The first four tests port the routing assertions of `editors/vscode/src/test/suite/
//! hmi-agent-quality.test.ts` on the same fixtures, run directly rather than through the draft
//! validation service. The rest are the boundaries of each limit: the 2 px body margin, the 16 px top
//! edge, the 8 px gap, and the 200-finding limit.

#![allow(clippy::float_cmp)]

use serde_json::{json, Value};
use trust_runtime::hmi::page_files::HmiPage;

use super::*;

fn page(value: Value) -> HmiPage {
    serde_json::from_value(value).expect("the fixture is a valid page")
}

/// The four junctions of the crossing fixture: 24 px squares at the left, right, top and bottom.
fn junctions() -> Vec<Value> {
    vec![
        json!({"id": "left", "type": "junction", "at": [40, 100], "size": [24, 24]}),
        json!({"id": "right", "type": "junction", "at": [400, 100], "size": [24, 24]}),
        json!({"id": "top", "type": "junction", "at": [220, 20], "size": [24, 24]}),
        json!({"id": "bottom", "type": "junction", "at": [220, 240], "size": [24, 24]}),
    ]
}

/// Three 100 × 80 faceplate cards of one panel, at the given top-left corners; the third card has
/// the given size.
fn cards(positions: [[i64; 2]; 3], third_size: [u32; 2]) -> HmiPage {
    let card = |id: &str, at: [i64; 2], size: [u32; 2]| json!({"id": id, "type": "faceplate", "faceplate": "pump", "at": at, "size": size});
    page(json!({"title": "Units", "object": [
        card("a", positions[0], [100, 80]),
        card("b", positions[1], [100, 80]),
        card("c", positions[2], third_size)
    ], "pipe": []}))
}

fn rules(findings: &[Finding]) -> Vec<&'static str> {
    findings.iter().map(|finding| finding.rule).collect()
}

#[test]
fn crossing_pipes_and_a_joined_pair_are_not_findings() {
    let page = page(json!({
        "title": "Crossing",
        "object": junctions(),
        "pipe": [
            {"from": "left.right", "to": "right.left"},
            {"from": "top.bottom", "to": "bottom.top"}
        ]
    }));
    assert!(routing_quality(&page, [640, 400]).is_empty());
}

#[test]
fn a_connection_to_a_missing_port_is_a_disconnected_error_naming_both_objects() {
    let page = page(json!({
        "title": "Broken",
        "object": junctions(),
        "pipe": [{"from": "missing.outlet", "to": "right.left"}]
    }));
    let findings = routing_quality(&page, [640, 400]);
    assert_eq!(findings[0].rule, "disconnected-port");
    assert_eq!(findings[0].severity, "error");
    assert_eq!(findings[0].pipe, Some(0));
    assert_eq!(
        findings[0].objects,
        vec!["missing".to_owned(), "right".to_owned()]
    );
    assert_eq!(
        findings[0].message,
        "Connection missing.outlet → right.left has an unavailable endpoint; reconnect it to a declared port."
    );
}

// A waypoint inside a text block is a drawn collision: the route cannot reach the waypoint, so the
// pipe is drawn as its fallback path, which runs through the block.
#[test]
fn a_waypoint_through_a_reading_is_a_collision_with_that_object() {
    let mut objects = junctions();
    objects.push(json!({"id": "reading", "type": "text", "at": [160, 80], "size": [80, 60]}));
    let page = page(json!({
        "title": "Collision",
        "object": objects,
        "pipe": [{"from": "left.right", "to": "right.left", "waypoints": [[200, 112]]}]
    }));
    let findings = routing_quality(&page, [640, 400]);
    let collision = findings
        .iter()
        .find(|finding| finding.rule == "pipe-object-collision")
        .expect("the pipe runs through the reading");
    assert_eq!(collision.object.as_deref(), Some("reading"));
    assert_eq!(collision.pipe, Some(0));
    assert_eq!(
        collision.rectangle,
        Some(Rect {
            x: 160.0,
            y: 80.0,
            width: 80.0,
            height: 60.0,
        })
    );
}

#[test]
fn repeated_equipment_measures_alignment_and_spacing_and_keeps_separate_rows() {
    let findings = routing_quality(
        &cards([[0, 80], [120, 88], [280, 80]], [100, 80]),
        [640, 400],
    );
    assert_eq!(
        rules(&findings),
        vec!["repeated-equipment-alignment", "repeated-equipment-spacing"]
    );
    assert_eq!(
        findings[0].measured,
        Some(Measure::Edges {
            top: 80,
            bottom: 88,
        })
    );
    assert_eq!(
        findings[1].measured,
        Some(Measure::Gaps { gaps: vec![20, 60] })
    );

    let aligned = cards([[0, 80], [120, 80], [240, 80]], [100, 80]);
    assert!(routing_quality(&aligned, [640, 400]).is_empty());

    let separate = cards([[0, 80], [120, 80], [240, 240]], [100, 80]);
    assert!(
        routing_quality(&separate, [640, 400]).is_empty(),
        "separate process rows are intentional"
    );

    let resized = cards([[0, 80], [120, 80], [240, 240]], [120, 80]);
    assert_eq!(
        rules(&routing_quality(&resized, [640, 400])),
        vec!["repeated-equipment-size"]
    );
}

// The top edges of a row may differ by up to 16 px: 16 px still forms a row (and is reported as a
// misalignment), 17 px does not.
#[test]
fn a_top_edge_difference_of_16_pixels_is_a_row_and_17_is_not() {
    let within = cards([[0, 80], [120, 80], [240, 96]], [100, 80]);
    assert_eq!(
        rules(&routing_quality(&within, [640, 400])),
        vec!["repeated-equipment-alignment"]
    );
    let beyond = cards([[0, 80], [120, 80], [240, 97]], [100, 80]);
    assert!(routing_quality(&beyond, [640, 400]).is_empty());
}

// The gaps between the cards of a row may differ by up to 8 px: gaps of 20 and 28 pass, 20 and 29
// do not.
#[test]
fn a_gap_difference_of_8_pixels_is_even_and_9_is_uneven() {
    let even = cards([[0, 80], [120, 80], [248, 80]], [100, 80]);
    assert!(routing_quality(&even, [640, 400]).is_empty());
    let uneven = cards([[0, 80], [120, 80], [249, 80]], [100, 80]);
    assert_eq!(
        rules(&routing_quality(&uneven, [640, 400])),
        vec!["repeated-equipment-spacing"]
    );
}

// A route at y = 112 is a collision with a body only when it lies more than 2 px inside the body's
// top edge, and more than 2 px inside its bottom edge.
#[test]
fn a_route_within_2_pixels_of_a_body_edge_is_not_a_collision() {
    let route = [[0.0, 112.0], [300.0, 112.0]];
    let body = |y: f64, height: f64| Rect {
        x: 100.0,
        y,
        width: 50.0,
        height,
    };
    assert!(
        !crosses_body(&route, body(110.0, 50.0)),
        "the top edge is 2 px above the route"
    );
    assert!(
        crosses_body(&route, body(109.0, 50.0)),
        "the route is 3 px inside the top edge"
    );
    assert!(
        !crosses_body(&route, body(60.0, 52.0)),
        "the bottom edge is 2 px below the route"
    );
    assert!(
        crosses_body(&route, body(60.0, 55.0)),
        "the route is 3 px inside the bottom edge"
    );
}

#[test]
fn a_vertical_route_follows_the_same_margin_and_a_diagonal_is_not_checked() {
    let vertical = [[232.0, 0.0], [232.0, 300.0]];
    assert!(crosses_body(
        &vertical,
        Rect {
            x: 200.0,
            y: 50.0,
            width: 50.0,
            height: 50.0,
        }
    ));
    assert!(!crosses_body(
        &vertical,
        Rect {
            x: 300.0,
            y: 50.0,
            width: 50.0,
            height: 50.0,
        }
    ));
    let diagonal = [[0.0, 0.0], [10.0, 10.0]];
    assert!(!crosses_body(
        &diagonal,
        Rect {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 20.0,
        }
    ));
}

// 205 pipes, each with an unavailable endpoint: the first 200 are reported and the rest are cut.
#[test]
fn routing_findings_stop_at_two_hundred() {
    let pipes: Vec<Value> = (0..205)
        .map(|_| json!({"from": "missing.x", "to": "a.left"}))
        .collect();
    let page = page(json!({
        "title": "Many",
        "object": [{"id": "a", "type": "junction", "at": [0, 0], "size": [24, 24]}],
        "pipe": pipes
    }));
    let findings = routing_quality(&page, [640, 400]);
    assert_eq!(findings.len(), 200);
    assert!(findings
        .iter()
        .all(|finding| finding.rule == "disconnected-port"));
    assert_eq!(findings[0].pipe, Some(0));
    assert_eq!(findings[199].pipe, Some(199));
}
