//! The ports of the objects on a page: where a pipe meets an object and the way it leaves it
//! (specification 39, §6 item 1; §14.2 for the symbols' own pixels).
//!
//! Port of `editors/vscode/src/hmiBuilder/drawing/pipes/ports.ts` (`portPoint` and `turn`). The
//! route needs the geometry of the shipped symbols as well: their design size, where their label
//! stands, and their ports, which the runtime's table holds (`hmi::symbol_geometry`) and the
//! editor's drawing reads too; `symbolPlacement` places a symbol in its object. Project symbols
//! (`my:<name>`) are registered at run time by the editor and are not in the table.

use trust_runtime::hmi::page_files::HmiObject;
pub(super) use trust_runtime::hmi::symbol_geometry::{label_height, PortDirection, SymbolLabel};
use trust_runtime::hmi::symbol_geometry::{symbol_shape, SymbolShape};

use super::geometry::coord;

/// Where a port is on the page and the way a pipe leaves it (`PortPoint` in `ports.ts`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PortPoint {
    /// The port's position in page pixels.
    pub(super) at: [f64; 2],
    /// The way the pipe leaves the port.
    pub(super) direction: PortDirection,
}

/// Where a symbol stands in its object (`symbolPlacement` in `symbols/index.ts`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Placement {
    /// The left edge of the symbol within the object.
    pub(super) left: f64,
    /// The top edge of the symbol within the object.
    pub(super) top: f64,
    /// The factor the symbol is drawn at.
    pub(super) scale: f64,
    /// The height the symbol and its label share: the object's height for a label beside it, else
    /// the room above the label.
    pub(super) area: f64,
}

/// The geometry of a shipped symbol by its type, or `None` for a type that is not a shipped symbol
/// (the runtime's table, which the editor's `SYMBOLS` take their geometry from).
#[must_use]
pub(super) fn symbol_geometry(kind: &str) -> Option<&'static SymbolShape> {
    symbol_shape(kind)
}

/// The placement of a symbol in its object: scaled to fit above its label and centred, or, with its
/// label beside it, fitted to the height at the left and taking at most half the width, so the
/// label keeps its room (`symbolPlacement`, §14.2).
#[must_use]
pub(super) fn symbol_placement(
    size: [u32; 2],
    symbol_size: [u32; 2],
    label: SymbolLabel,
) -> Placement {
    let (width, height) = (f64::from(size[0]), f64::from(size[1]));
    let (symbol_width, symbol_height) = (f64::from(symbol_size[0]), f64::from(symbol_size[1]));
    match label {
        SymbolLabel::Beside => {
            let fit = (height / symbol_height)
                .min(width / 2.0 / symbol_width)
                .max(0.0);
            Placement {
                left: 0.0,
                top: round_hundredths((height - symbol_height * fit) / 2.0),
                scale: fit,
                area: height,
            }
        }
        SymbolLabel::Under => {
            let area = (height - label_height()).max(0.0);
            let scale = (width / symbol_width).min(area / symbol_height).max(0.0);
            Placement {
                left: round_hundredths((width - symbol_width * scale) / 2.0),
                top: round_hundredths((area - symbol_height * scale) / 2.0),
                scale,
                area,
            }
        }
    }
}

/// A value to two decimals, half upwards: `Math.round(value * 100) / 100` (`round` in
/// `symbols/style.ts` and `rounded` in `route.ts`).
#[must_use]
pub(super) fn round_hundredths(value: f64) -> f64 {
    super::geometry::js_round(value * 100.0) / 100.0
}

/// The point where the named port of an object stands on the page, and the way a pipe leaves it;
/// `None` when the object has no port of that name (`portPoint` in `ports.ts`). A junction has four
/// ports at the centres of its sides; an equipment symbol has the ports its geometry declares.
#[must_use]
pub(super) fn port_point(object: &HmiObject, port: &str) -> Option<PortPoint> {
    let local = local_port(object, port)?;
    let turned = turn(
        local,
        object.rotation,
        f64::from(object.size[0]),
        f64::from(object.size[1]),
    );
    Some(PortPoint {
        at: [
            coord(object.at[0]) + turned.at[0],
            coord(object.at[1]) + turned.at[1],
        ],
        direction: turned.direction,
    })
}

/// The port of an object before the object is turned, in the object's own frame.
fn local_port(object: &HmiObject, port: &str) -> Option<PortPoint> {
    if object.kind == "junction" {
        let direction = junction_direction(port)?;
        return Some(PortPoint {
            at: [
                f64::from(object.size[0]) / 2.0,
                f64::from(object.size[1]) / 2.0,
            ],
            direction,
        });
    }
    let definition = symbol_geometry(&object.kind)?;
    let symbol_port = definition
        .ports
        .iter()
        .find(|candidate| candidate.name == port)?;
    // `portPoint` places the symbol with its default label, under it.
    let placement = symbol_placement(object.size, definition.size, SymbolLabel::Under);
    Some(PortPoint {
        at: [
            placement.left + f64::from(symbol_port.at[0]) * placement.scale,
            placement.top + f64::from(symbol_port.at[1]) * placement.scale,
        ],
        direction: symbol_port.direction,
    })
}

/// The direction of a junction's port: its left and right sides face left and right, its top faces
/// up and its bottom faces down (`JUNCTION_PORTS` in `ports.ts`).
fn junction_direction(port: &str) -> Option<PortDirection> {
    match port {
        "left" => Some(PortDirection::Left),
        "right" => Some(PortDirection::Right),
        "top" => Some(PortDirection::Up),
        "bottom" => Some(PortDirection::Down),
        _ => None,
    }
}

/// The directions in the order a quarter turn clockwise on the page takes them (`TURNS` in
/// `ports.ts`).
const TURNS: [PortDirection; 4] = [
    PortDirection::Right,
    PortDirection::Down,
    PortDirection::Left,
    PortDirection::Up,
];

/// A point and its direction turned with its object about the object's centre, in quarter turns
/// (`turn` in `ports.ts`). Degrees are rounded to the nearest quarter turn, halves upwards.
fn turn(local: PortPoint, rotation: u16, width: f64, height: f64) -> PortPoint {
    let quarters = ((usize::from(rotation) + 45) / 90) % 4;
    if quarters == 0 {
        return local;
    }
    let (centre_x, centre_y) = (width / 2.0, height / 2.0);
    let (mut x, mut y) = (local.at[0] - centre_x, local.at[1] - centre_y);
    for _ in 0..quarters {
        (x, y) = (-y, x);
    }
    let index = (turn_index(local.direction) + quarters) % 4;
    PortPoint {
        at: [centre_x + x, centre_y + y],
        direction: TURNS[index],
    }
}

/// A direction's place in `TURNS`.
fn turn_index(direction: PortDirection) -> usize {
    match direction {
        PortDirection::Right => 0,
        PortDirection::Down => 1,
        PortDirection::Left => 2,
        PortDirection::Up => 3,
    }
}

#[cfg(test)]
#[path = "ports_tests.rs"]
mod tests;
