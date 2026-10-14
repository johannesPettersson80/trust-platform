//! The ports of the objects on a page: where a pipe meets an object and the way it leaves it
//! (specification 39, §6 item 1; §14.2 for the symbols' own pixels).
//!
//! Port of `editors/vscode/src/hmiBuilder/drawing/pipes/ports.ts` (`portPoint` and `turn`). The
//! route needs the geometry of the shipped symbols as well, so this module also carries what the
//! symbol modules declare of each symbol: its design size, where its label stands, and its ports
//! (`SYMBOLS` in `drawing/symbols/index.ts` and the modules it imports; `symbolPlacement` and
//! `LABEL_HEIGHT` from the same index). Their drawing is not ported. Project symbols (`my:<name>`)
//! are registered at run time by the editor and are not in this table.

use trust_runtime::hmi::page_files::HmiObject;

use super::geometry::coord;

/// A direction a pipe leaves a port in (`PortDirection` in `ports.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PortDirection {
    /// Towards smaller x.
    Left,
    /// Towards larger x.
    Right,
    /// Towards smaller y, up the page.
    Up,
    /// Towards larger y, down the page.
    Down,
}

/// Where a port is on the page and the way a pipe leaves it (`PortPoint` in `ports.ts`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PortPoint {
    /// The port's position in page pixels.
    pub(super) at: [f64; 2],
    /// The way the pipe leaves the port.
    pub(super) direction: PortDirection,
}

/// Where a symbol's label stands (`SymbolLabel` in `symbols/index.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SymbolLabel {
    /// Under the symbol, the default.
    Under,
    /// Beside it, as an instrument's label is.
    Beside,
}

/// A port of a shipped symbol, in the symbol's own pixels (`SymbolPort` in `symbols/index.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SymbolPort {
    /// The port's name, as a pipe's end writes it after the object id.
    pub(super) name: &'static str,
    /// Its position in the symbol's own pixels.
    pub(super) at: [u16; 2],
    /// The way a pipe leaves it.
    pub(super) direction: PortDirection,
}

/// The geometry a shipped symbol declares: its design size, its label and its ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SymbolGeometry {
    /// The design size in pixels, without its label.
    pub(super) size: [u32; 2],
    /// Where its label stands.
    pub(super) label: SymbolLabel,
    /// Its connection points; empty for a symbol without ports.
    pub(super) ports: &'static [SymbolPort],
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

/// The room the label takes under a symbol: the tag and the state word, each 14 px (`LABEL_HEIGHT`,
/// §14.2).
pub(super) const LABEL_HEIGHT: f64 = 44.0;

// The port arrays of the shipped symbols, each as its symbol module declares them.

const PUMP_PORTS: &[SymbolPort] = &[
    SymbolPort {
        name: "inlet",
        at: [0, 36],
        direction: PortDirection::Left,
    },
    SymbolPort {
        name: "outlet",
        at: [64, 20],
        direction: PortDirection::Right,
    },
];

const VALVE_PORTS: &[SymbolPort] = &[
    SymbolPort {
        name: "inlet",
        at: [0, 44],
        direction: PortDirection::Left,
    },
    SymbolPort {
        name: "outlet",
        at: [64, 44],
        direction: PortDirection::Right,
    },
];

const TANK_PORTS: &[SymbolPort] = &[
    SymbolPort {
        name: "inlet",
        at: [20, 0],
        direction: PortDirection::Up,
    },
    SymbolPort {
        name: "outlet",
        at: [64, 46],
        direction: PortDirection::Right,
    },
];

const GRAVITY_TANK_PORTS: &[SymbolPort] = &[
    SymbolPort {
        name: "inlet",
        at: [20, 0],
        direction: PortDirection::Up,
    },
    SymbolPort {
        name: "outlet",
        at: [32, 64],
        direction: PortDirection::Down,
    },
];

const TANK_CASCADE_PORTS: &[SymbolPort] = &[
    SymbolPort {
        name: "inlet",
        at: [14, 0],
        direction: PortDirection::Up,
    },
    SymbolPort {
        name: "outlet",
        at: [96, 60],
        direction: PortDirection::Right,
    },
];

const DOSING_PUMP_PORTS: &[SymbolPort] = &[
    SymbolPort {
        name: "inlet",
        at: [14, 64],
        direction: PortDirection::Down,
    },
    SymbolPort {
        name: "outlet",
        at: [14, 0],
        direction: PortDirection::Up,
    },
];

const SUPPLY_PORTS: &[SymbolPort] = &[SymbolPort {
    name: "outlet",
    at: [64, 32],
    direction: PortDirection::Right,
}];

const MIXER_PORTS: &[SymbolPort] = &[
    SymbolPort {
        name: "inlet",
        at: [0, 32],
        direction: PortDirection::Left,
    },
    SymbolPort {
        name: "inlet-2",
        at: [24, 64],
        direction: PortDirection::Down,
    },
    SymbolPort {
        name: "outlet",
        at: [64, 32],
        direction: PortDirection::Right,
    },
];

const HEATER_PORTS: &[SymbolPort] = &[
    SymbolPort {
        name: "inlet",
        at: [20, 0],
        direction: PortDirection::Up,
    },
    SymbolPort {
        name: "outlet",
        at: [64, 16],
        direction: PortDirection::Right,
    },
];

const COOLER_PORTS: &[SymbolPort] = &[
    SymbolPort {
        name: "inlet",
        at: [0, 18],
        direction: PortDirection::Left,
    },
    SymbolPort {
        name: "outlet",
        at: [64, 18],
        direction: PortDirection::Right,
    },
];

const JACKETED_VESSEL_PORTS: &[SymbolPort] = &[
    SymbolPort {
        name: "inlet",
        at: [32, 0],
        direction: PortDirection::Up,
    },
    SymbolPort {
        name: "outlet",
        at: [32, 64],
        direction: PortDirection::Down,
    },
    SymbolPort {
        name: "jacket-inlet",
        at: [0, 52],
        direction: PortDirection::Left,
    },
    SymbolPort {
        name: "jacket-outlet",
        at: [64, 26],
        direction: PortDirection::Right,
    },
];

const COUPLED_VESSELS_PORTS: &[SymbolPort] = &[
    SymbolPort {
        name: "inlet-1",
        at: [20, 0],
        direction: PortDirection::Up,
    },
    SymbolPort {
        name: "outlet-1",
        at: [20, 64],
        direction: PortDirection::Down,
    },
    SymbolPort {
        name: "inlet-2",
        at: [76, 0],
        direction: PortDirection::Up,
    },
    SymbolPort {
        name: "outlet-2",
        at: [76, 64],
        direction: PortDirection::Down,
    },
];

const PIPED_PORTS: &[SymbolPort] = &[
    SymbolPort {
        name: "inlet",
        at: [0, 32],
        direction: PortDirection::Left,
    },
    SymbolPort {
        name: "outlet",
        at: [64, 32],
        direction: PortDirection::Right,
    },
];

const VESSEL_PORTS: &[SymbolPort] = &[
    SymbolPort {
        name: "inlet",
        at: [32, 0],
        direction: PortDirection::Up,
    },
    SymbolPort {
        name: "outlet",
        at: [32, 64],
        direction: PortDirection::Down,
    },
];

const HEAT_EXCHANGER_PORTS: &[SymbolPort] = &[
    SymbolPort {
        name: "inlet",
        at: [0, 32],
        direction: PortDirection::Left,
    },
    SymbolPort {
        name: "outlet",
        at: [64, 32],
        direction: PortDirection::Right,
    },
    SymbolPort {
        name: "medium-inlet",
        at: [32, 0],
        direction: PortDirection::Up,
    },
    SymbolPort {
        name: "medium-outlet",
        at: [32, 64],
        direction: PortDirection::Down,
    },
];

const CYCLONE_PORTS: &[SymbolPort] = &[
    SymbolPort {
        name: "inlet",
        at: [0, 14],
        direction: PortDirection::Left,
    },
    SymbolPort {
        name: "gas-outlet",
        at: [32, 0],
        direction: PortDirection::Up,
    },
    SymbolPort {
        name: "solids-outlet",
        at: [32, 64],
        direction: PortDirection::Down,
    },
];

const SCREEN_PORTS: &[SymbolPort] = &[
    SymbolPort {
        name: "inlet",
        at: [8, 0],
        direction: PortDirection::Up,
    },
    SymbolPort {
        name: "oversize",
        at: [64, 44],
        direction: PortDirection::Right,
    },
    SymbolPort {
        name: "undersize",
        at: [32, 64],
        direction: PortDirection::Down,
    },
];

// The geometry of the shipped symbols, one constant for each shape. The symbols that share a
// shape share its constant.

const PUMP: SymbolGeometry = SymbolGeometry {
    size: [64, 64],
    label: SymbolLabel::Under,
    ports: PUMP_PORTS,
};
const VALVE: SymbolGeometry = SymbolGeometry {
    size: [64, 64],
    label: SymbolLabel::Under,
    ports: VALVE_PORTS,
};
const TANK: SymbolGeometry = SymbolGeometry {
    size: [64, 64],
    label: SymbolLabel::Under,
    ports: TANK_PORTS,
};
const GRAVITY_TANK: SymbolGeometry = SymbolGeometry {
    size: [64, 64],
    label: SymbolLabel::Under,
    ports: GRAVITY_TANK_PORTS,
};
const TANK_CASCADE: SymbolGeometry = SymbolGeometry {
    size: [96, 72],
    label: SymbolLabel::Under,
    ports: TANK_CASCADE_PORTS,
};
const DOSING_PUMP: SymbolGeometry = SymbolGeometry {
    size: [64, 64],
    label: SymbolLabel::Under,
    ports: DOSING_PUMP_PORTS,
};
const SUPPLY: SymbolGeometry = SymbolGeometry {
    size: [64, 64],
    label: SymbolLabel::Under,
    ports: SUPPLY_PORTS,
};
const MIXER: SymbolGeometry = SymbolGeometry {
    size: [64, 64],
    label: SymbolLabel::Under,
    ports: MIXER_PORTS,
};
/// The instruments: a bubble with its letters, and its label beside it.
const INSTRUMENT: SymbolGeometry = SymbolGeometry {
    size: [48, 48],
    label: SymbolLabel::Beside,
    ports: &[],
};
const SENSOR_PAIR: SymbolGeometry = SymbolGeometry {
    size: [112, 48],
    label: SymbolLabel::Beside,
    ports: &[],
};
const LARGE_EQUIPMENT: SymbolGeometry = SymbolGeometry {
    size: [96, 64],
    label: SymbolLabel::Under,
    ports: &[],
};
const HEATER: SymbolGeometry = SymbolGeometry {
    size: [64, 64],
    label: SymbolLabel::Under,
    ports: HEATER_PORTS,
};
const COOLER: SymbolGeometry = SymbolGeometry {
    size: [64, 64],
    label: SymbolLabel::Under,
    ports: COOLER_PORTS,
};
const JACKETED_VESSEL: SymbolGeometry = SymbolGeometry {
    size: [64, 64],
    label: SymbolLabel::Under,
    ports: JACKETED_VESSEL_PORTS,
};
const COUPLED_VESSELS: SymbolGeometry = SymbolGeometry {
    size: [96, 64],
    label: SymbolLabel::Under,
    ports: COUPLED_VESSELS_PORTS,
};
const EQUIPMENT: SymbolGeometry = SymbolGeometry {
    size: [64, 64],
    label: SymbolLabel::Under,
    ports: &[],
};
const WIDE_EQUIPMENT: SymbolGeometry = SymbolGeometry {
    size: [96, 40],
    label: SymbolLabel::Under,
    ports: &[],
};
const PIPED: SymbolGeometry = SymbolGeometry {
    size: [64, 64],
    label: SymbolLabel::Under,
    ports: PIPED_PORTS,
};
const VESSEL: SymbolGeometry = SymbolGeometry {
    size: [64, 64],
    label: SymbolLabel::Under,
    ports: VESSEL_PORTS,
};
const HEAT_EXCHANGER: SymbolGeometry = SymbolGeometry {
    size: [64, 64],
    label: SymbolLabel::Under,
    ports: HEAT_EXCHANGER_PORTS,
};
const CYCLONE: SymbolGeometry = SymbolGeometry {
    size: [64, 64],
    label: SymbolLabel::Under,
    ports: CYCLONE_PORTS,
};
const SCREEN: SymbolGeometry = SymbolGeometry {
    size: [64, 64],
    label: SymbolLabel::Under,
    ports: SCREEN_PORTS,
};

/// The geometry of a shipped symbol by its type, or `None` for a type that is not a shipped symbol
/// (`SYMBOLS` in `symbols/index.ts`).
#[must_use]
pub(super) fn symbol_geometry(kind: &str) -> Option<SymbolGeometry> {
    let geometry = match kind {
        "pump" => PUMP,
        "valve" | "control-valve" | "motor-valve" | "heat-draw-off" => VALVE,
        "tank" => TANK,
        "gravity-tank" => GRAVITY_TANK,
        "tank-cascade" => TANK_CASCADE,
        "dosing-pump" => DOSING_PUMP,
        "supply" => SUPPLY,
        "mixer" => MIXER,
        "temperature-sensor" | "level-sensor" | "flow-sensor" | "pressure-sensor"
        | "position-sensor" | "speed-sensor" | "analyzer" | "controller" | "level-switch"
        | "pressure-switch" | "digital-switch" => INSTRUMENT,
        "sensor-pair" => SENSOR_PAIR,
        "cold-room" | "refrigeration-unit" | "servo-motor" | "coupling" => LARGE_EQUIPMENT,
        "heater" => HEATER,
        "cooler" => COOLER,
        "jacketed-vessel" => JACKETED_VESSEL,
        "coupled-vessels" => COUPLED_VESSELS,
        "motor" | "device" | "shaft" | "drum" | "encoder" | "rotary-arm" | "axis-indicator"
        | "network-link" | "fan" | "agitator" | "weigh-scale" | "vfd" | "drive" | "contactor"
        | "breaker" => EQUIPMENT,
        "conveyor" | "cylinder" => WIDE_EQUIPMENT,
        "blower" | "damper" | "filter" => PIPED,
        "vessel" | "crusher" => VESSEL,
        "heat-exchanger" => HEAT_EXCHANGER,
        "cyclone" => CYCLONE,
        "screen" => SCREEN,
        _ => return None,
    };
    Some(geometry)
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
            let area = (height - LABEL_HEIGHT).max(0.0);
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
