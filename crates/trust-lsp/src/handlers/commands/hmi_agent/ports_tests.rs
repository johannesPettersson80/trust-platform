//! Tests of the port geometry: `ports.ts` (`portPoint`, `turn`), `symbolPlacement` and the shipped
//! symbols' declared geometry (specification 39, §6 item 1 and §14.2).

#![allow(clippy::float_cmp)]

use serde_json::json;
use trust_runtime::hmi::page_files::HmiObject;

use super::*;

fn object(value: serde_json::Value) -> HmiObject {
    serde_json::from_value(value).expect("the fixture is a valid object")
}

// A pump of 64 px in a 64 px object: the 20 px above its label scale the symbol by 0.3125, so the
// symbol stands at 22 px from the left and its outlet at (64, 20) lands at (42, 6.25) in the object.
#[test]
fn pump_outlet_follows_the_scaled_symbol() {
    let pump = object(json!({"id": "p", "type": "pump", "at": [100, 200], "size": [64, 64]}));
    let outlet = port_point(&pump, "outlet").expect("a pump has an outlet");
    assert_eq!(outlet.at, [142.0, 206.25]);
    assert_eq!(outlet.direction, PortDirection::Right);
}

// A quarter turn about the centre (32, 32) takes the port (42, 6.25) to (57.75, 42) and its
// direction from right to down, the way a page turns clockwise.
#[test]
fn a_quarter_turn_moves_the_port_and_turns_its_direction() {
    let pump = object(json!({
        "id": "p", "type": "pump", "at": [100, 200], "size": [64, 64], "rotation": 90
    }));
    let outlet = port_point(&pump, "outlet").expect("a pump has an outlet");
    assert_eq!(outlet.at, [157.75, 242.0]);
    assert_eq!(outlet.direction, PortDirection::Down);
}

// Rotations round to the nearest quarter turn, halves upwards: 45 degrees is one quarter turn,
// 44 degrees is none.
#[test]
fn rotation_rounds_to_quarter_turns_with_halves_upwards() {
    let at_45 = object(json!({
        "id": "p", "type": "pump", "at": [0, 0], "size": [64, 64], "rotation": 45
    }));
    let at_44 = object(json!({
        "id": "p", "type": "pump", "at": [0, 0], "size": [64, 64], "rotation": 44
    }));
    assert_eq!(
        port_point(&at_45, "outlet").map(|point| point.direction),
        Some(PortDirection::Down)
    );
    assert_eq!(
        port_point(&at_44, "outlet").map(|point| point.direction),
        Some(PortDirection::Right)
    );
}

#[test]
fn junction_ports_sit_at_the_centre_of_each_side() {
    let junction = object(json!({"id": "j", "type": "junction", "at": [10, 20], "size": [16, 16]}));
    assert_eq!(
        port_point(&junction, "right"),
        Some(PortPoint {
            at: [18.0, 28.0],
            direction: PortDirection::Right,
        })
    );
    assert_eq!(
        port_point(&junction, "top"),
        Some(PortPoint {
            at: [18.0, 28.0],
            direction: PortDirection::Up,
        })
    );
    assert_eq!(
        port_point(&junction, "bottom").map(|point| point.direction),
        Some(PortDirection::Down)
    );
    assert_eq!(port_point(&junction, "middle"), None);
}

#[test]
fn an_unknown_port_or_a_type_without_ports_has_no_point() {
    let pump = object(json!({"id": "p", "type": "pump", "at": [0, 0], "size": [64, 64]}));
    assert_eq!(port_point(&pump, "nope"), None);
    let text = object(json!({"id": "t", "type": "text", "at": [0, 0], "size": [64, 64]}));
    assert_eq!(port_point(&text, "inlet"), None);
}

#[test]
fn placement_under_the_label_scales_and_centres_the_symbol() {
    assert_eq!(
        symbol_placement([64, 64], [64, 64], SymbolLabel::Under),
        Placement {
            left: 22.0,
            top: 0.0,
            scale: 0.3125,
            area: 20.0,
        }
    );
}

// Beside its label the symbol fits the height and takes at most half the width: 48 px high in a
// 48 px object is drawn at 0.5, centred vertically at 12 px.
#[test]
fn placement_beside_the_label_fits_the_height_at_most_half_the_width() {
    assert_eq!(
        symbol_placement([48, 48], [48, 48], SymbolLabel::Beside),
        Placement {
            left: 0.0,
            top: 12.0,
            scale: 0.5,
            area: 48.0,
        }
    );
}

#[test]
fn round_hundredths_rounds_halves_upwards_like_math_round() {
    assert_eq!(round_hundredths(0.125), 0.13);
    assert_eq!(round_hundredths(-0.125), -0.12);
    assert_eq!(round_hundredths(2.0), 2.0);
}

#[test]
fn shipped_symbols_declare_their_size_label_and_ports() {
    let sensor = symbol_geometry("sensor-pair").expect("a shipped symbol");
    assert_eq!(sensor.size, [112, 48]);
    assert_eq!(sensor.label, SymbolLabel::Beside);
    assert!(sensor.ports.is_empty());

    let coupled = symbol_geometry("coupled-vessels").expect("a shipped symbol");
    assert_eq!(coupled.ports.len(), 4);
    assert_eq!(coupled.ports[3].name, "outlet-2");
    assert_eq!(coupled.ports[3].at, [76, 64]);

    let screen = symbol_geometry("screen").expect("a shipped symbol");
    assert_eq!(screen.ports[1].name, "oversize");
    assert_eq!(screen.ports[1].at, [64, 44]);
    assert_eq!(screen.ports[1].direction, PortDirection::Right);

    assert_eq!(symbol_geometry("text"), None);
    assert_eq!(symbol_geometry("junction"), None);
}
