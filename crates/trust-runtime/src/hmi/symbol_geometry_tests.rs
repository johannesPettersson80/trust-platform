//! The shipped symbols' geometry table (specification 39, §6 item 1, §14.2): every symbol names a shape the table holds, every port
//! lies on its symbol, and the values the drawing draws the pump, the valve and an instrument with are the table's.

use super::{label_height, symbol_shape, PortDirection, SymbolLabel};

#[test]
fn the_pump_valve_and_an_instrument_have_the_geometry_the_drawing_draws() {
    let pump = symbol_shape("pump").expect("the pump");
    assert_eq!(pump.size, [64, 64]);
    assert_eq!(pump.label, SymbolLabel::Under);
    assert_eq!(
        pump.ports
            .iter()
            .map(|port| (port.name.as_str(), port.at, port.direction))
            .collect::<Vec<_>>(),
        [
            ("inlet", [0, 36], PortDirection::Left),
            ("outlet", [64, 20], PortDirection::Right)
        ]
    );
    // A control valve and a motor valve share the valve's shape.
    assert_eq!(symbol_shape("control-valve"), symbol_shape("valve"));
    assert_eq!(symbol_shape("motor-valve"), symbol_shape("valve"));
    let sensor = symbol_shape("temperature-sensor").expect("an instrument");
    assert_eq!((sensor.size, sensor.label), ([48, 48], SymbolLabel::Beside));
    assert!(
        sensor.ports.is_empty(),
        "an instrument has no ports: it attaches (§6 item 5)"
    );
    assert_eq!(
        label_height(),
        44.0,
        "the tag and the state word, each 14 px, with their spacing (§14.2)"
    );
}

#[test]
fn every_port_lies_on_its_symbols_edge_and_names_differ() {
    for kind in [
        "pump",
        "valve",
        "tank",
        "gravity-tank",
        "tank-cascade",
        "dosing-pump",
        "supply",
        "mixer",
        "heater",
        "cooler",
        "jacketed-vessel",
        "coupled-vessels",
        "blower",
        "vessel",
        "heat-exchanger",
        "cyclone",
        "screen",
    ] {
        let shape = symbol_shape(kind).unwrap_or_else(|| panic!("{kind} is a shipped symbol"));
        let [width, height] = shape.size;
        let mut names: Vec<&str> = shape.ports.iter().map(|port| port.name.as_str()).collect();
        for port in &shape.ports {
            let [x, y] = [u32::from(port.at[0]), u32::from(port.at[1])];
            // A pipe leaves a port outwards, so the port lies on the edge it leaves through.
            let on_edge = match port.direction {
                PortDirection::Left => x == 0,
                PortDirection::Right => x == width,
                PortDirection::Up => y == 0,
                PortDirection::Down => y == height,
            };
            assert!(
                on_edge && x <= width && y <= height,
                "{kind}.{} at {:?} leaves {:?}",
                port.name,
                port.at,
                port.direction
            );
        }
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), shape.ports.len(), "{kind}'s port names differ");
    }
}

#[test]
fn a_type_that_is_no_shipped_symbol_has_no_geometry() {
    assert_eq!(symbol_shape("value"), None);
    assert_eq!(
        symbol_shape("my:skid"),
        None,
        "a project symbol carries its own geometry (§14.4)"
    );
}
