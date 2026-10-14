//! Tests of the pipe routes: `route.ts` (`routePipe`, the search, the heap) and `pipeRoutes` and
//! `solidBoxes` in `drawing/pipes/draw.ts` (specification 39, §6 items 2 and 3).

#![allow(clippy::float_cmp)]

use serde_json::json;
use trust_runtime::hmi::page_files::{HmiObject, HmiPage};

use super::*;

fn page(value: serde_json::Value) -> HmiPage {
    serde_json::from_value(value).expect("the fixture is a valid page")
}

fn object(value: serde_json::Value) -> HmiObject {
    serde_json::from_value(value).expect("the fixture is a valid object")
}

/// Two 24 px junctions whose facing ports lie on the line y = 112: the left one's right port at
/// (52, 112) and the right one's left port at (412, 112).
fn junction_pair(extra_objects: serde_json::Value, pipe: serde_json::Value) -> HmiPage {
    let mut objects = vec![
        json!({"id": "left", "type": "junction", "at": [40, 100], "size": [24, 24]}),
        json!({"id": "right", "type": "junction", "at": [400, 100], "size": [24, 24]}),
    ];
    if let serde_json::Value::Array(extra) = extra_objects {
        objects.extend(extra);
    }
    page(json!({"title": "Pair", "object": objects, "pipe": [pipe]}))
}

#[test]
fn ports_on_one_clear_line_are_joined_straight() {
    let page = junction_pair(json!([]), json!({"from": "left.right", "to": "right.left"}));
    assert_eq!(
        pipe_routes(&page, [640, 400]),
        vec![Some(vec![[52.0, 112.0], [412.0, 112.0]])]
    );
}

#[test]
fn ports_facing_each_other_down_the_page_are_joined_straight() {
    let page = page(json!({"title": "Column", "object": [
        {"id": "top", "type": "junction", "at": [220, 20], "size": [24, 24]},
        {"id": "bottom", "type": "junction", "at": [220, 240], "size": [24, 24]}
    ], "pipe": [{"from": "top.bottom", "to": "bottom.top"}]}));
    assert_eq!(
        pipe_routes(&page, [640, 400]),
        vec![Some(vec![[232.0, 32.0], [232.0, 252.0]])]
    );
}

#[test]
fn a_connection_to_a_missing_port_has_no_route() {
    let page = junction_pair(
        json!([]),
        json!({"from": "missing.outlet", "to": "right.left"}),
    );
    assert_eq!(pipe_routes(&page, [640, 400]), vec![None]);
}

// A text block stands on the straight line between the ports. The route must leave both ports
// along their facing directions, turn only at right angles, and keep out of the block's body.
#[test]
fn a_route_around_an_object_is_orthogonal_and_keeps_out_of_its_body() {
    let block = json!({"id": "block", "type": "text", "at": [150, 100], "size": [100, 40]});
    let page = junction_pair(
        json!([block]),
        json!({"from": "left.right", "to": "right.left"}),
    );
    let routes = pipe_routes(&page, [1280, 800]);
    let points = routes[0].as_ref().expect("both ports exist");
    assert!(points.len() >= 3, "the straight line is blocked");
    assert_eq!(points.first(), Some(&[52.0, 112.0]));
    assert_eq!(points.last(), Some(&[412.0, 112.0]));
    for pair in points.windows(2) {
        assert!(
            pair[0][0] == pair[1][0] || pair[0][1] == pair[1][1],
            "every segment is horizontal or vertical"
        );
        let middle = [
            (pair[0][0] + pair[1][0]) / 2.0,
            (pair[0][1] + pair[1][1]) / 2.0,
        ];
        let inside_block =
            middle[0] > 150.0 && middle[0] < 250.0 && middle[1] > 100.0 && middle[1] < 140.0;
        assert!(
            !inside_block,
            "a segment runs through the block at {middle:?}"
        );
    }
}

#[test]
fn simplified_drops_repeats_and_the_middle_of_straight_runs() {
    assert_eq!(
        simplified(vec![[0.0, 0.0], [0.0, 5.0], [0.0, 10.0], [3.0, 10.0]]),
        vec![[0.0, 0.0], [0.0, 10.0], [3.0, 10.0]]
    );
    assert_eq!(
        simplified(vec![[1.0, 1.0], [1.0, 1.0], [2.0, 1.0]]),
        vec![[1.0, 1.0], [2.0, 1.0]]
    );
}

#[test]
fn the_queue_pops_the_cheapest_state_then_the_earliest_pushed() {
    let mut queue = Queue::default();
    queue.push(7, 5.0);
    queue.push(8, 5.0);
    queue.push(9, 3.0);
    assert_eq!(queue.pop(), Some(9));
    assert_eq!(queue.pop(), Some(7));
    assert_eq!(queue.pop(), Some(8));
    assert_eq!(queue.pop(), None);
}

// A pump of 64 px places its symbol 0.3125 times its size, 22 px from the left, under a 20 px room
// above its label: the body is the 20 px symbol and the 44 px label below it.
#[test]
fn a_symbol_keeps_its_body_and_its_label_as_separate_obstacles() {
    let pump = object(json!({"id": "p", "type": "pump", "at": [0, 0], "size": [64, 64]}));
    assert_eq!(
        solid_boxes(&pump),
        vec![
            Rect {
                x: 22.0,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            },
            Rect {
                x: 0.0,
                y: 20.0,
                width: 64.0,
                height: 44.0,
            },
        ]
    );
}

#[test]
fn junctions_and_rotated_or_beside_symbols_stand_in_their_boxes() {
    let junction = object(json!({"id": "j", "type": "junction", "at": [0, 0], "size": [16, 16]}));
    assert!(solid_boxes(&junction).is_empty());

    let turned = object(json!({
        "id": "p", "type": "pump", "at": [0, 0], "size": [64, 64], "rotation": 90
    }));
    assert_eq!(
        solid_boxes(&turned),
        vec![Rect {
            x: 0.0,
            y: 0.0,
            width: 64.0,
            height: 64.0,
        }]
    );

    let sensor = object(json!({
        "id": "t", "type": "temperature-sensor", "at": [0, 0], "size": [48, 48]
    }));
    assert_eq!(
        solid_boxes(&sensor),
        vec![Rect {
            x: 0.0,
            y: 0.0,
            width: 48.0,
            height: 48.0,
        }]
    );

    let text = object(json!({"id": "t", "type": "text", "at": [5, 6], "size": [30, 10]}));
    assert_eq!(
        solid_boxes(&text),
        vec![Rect {
            x: 5.0,
            y: 6.0,
            width: 30.0,
            height: 10.0,
        }]
    );
}

// On an empty grid the search from (0, 0) to (100, 0) takes the one step to the right.
#[test]
fn the_search_on_an_open_grid_takes_the_direct_step() {
    let grid = Grid::new(&[[0.0, 0.0], [100.0, 0.0]], &[], [640.0, 400.0]);
    let found = grid
        .search(
            [0.0, 0.0],
            [100.0, 0.0],
            PortDirection::Right,
            Some(PortDirection::Right),
        )
        .expect("an open grid has a path");
    assert_eq!(found.points, vec![[0.0, 0.0], [100.0, 0.0]]);
    assert_eq!(found.heading, PortDirection::Right);
}
