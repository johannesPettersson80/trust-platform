//! The geometry of the shipped equipment symbols (specification 39, §6 item 1 and §14.2): each symbol's design size, where its label
//! stands, and its ports, the points where a pipe meets the symbol with the way the pipe leaves it. The table is
//! `symbol_geometry.json`, which this module reads and which the editor's drawing reads too, through the module the extension
//! generates from it (`editors/vscode/src/hmiBuilder/drawing/symbols/geometry.ts`, `npm run generate:symbol-geometry`), so that
//! the drawing and the language server's routes meet the same ports. Project symbols (`my:<name>`) carry their own geometry in their
//! files (§14.4) and are not in this table.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// The geometry table, as the editor's generator reads it too.
pub const SYMBOL_GEOMETRY_JSON: &str = include_str!("symbol_geometry.json");

/// A direction a pipe leaves a port in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PortDirection {
    /// Towards smaller x.
    Left,
    /// Towards larger x.
    Right,
    /// Towards smaller y, up the page.
    Up,
    /// Towards larger y, down the page.
    Down,
}

/// Where a symbol's tag and state word stand.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SymbolLabel {
    /// Under the symbol, the default.
    #[default]
    Under,
    /// Beside it, as an instrument's label is.
    Beside,
}

/// A port of a shipped symbol, in the symbol's own pixels.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SymbolPort {
    /// The port's name, as a pipe's end writes it after the object id.
    pub name: String,
    /// Its position in the symbol's own pixels.
    pub at: [u16; 2],
    /// The way a pipe leaves it.
    pub direction: PortDirection,
}

/// The geometry a shipped symbol declares: its design size, its label and its ports.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SymbolShape {
    /// The design size in pixels, without its label.
    pub size: [u32; 2],
    /// Where its label stands; under it when the table names none.
    #[serde(default)]
    pub label: SymbolLabel,
    /// Its connection points, in their order; empty for a symbol without ports.
    pub ports: Vec<SymbolPort>,
}

/// The table: its format, the room the label takes under a symbol, the shapes by name, and each symbol's shape by its type.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GeometryTable {
    format: u32,
    label_height: f64,
    shapes: BTreeMap<String, SymbolShape>,
    symbols: BTreeMap<String, String>,
}

fn table() -> &'static GeometryTable {
    static TABLE: OnceLock<GeometryTable> = OnceLock::new();
    TABLE.get_or_init(|| {
        let table: GeometryTable =
            serde_json::from_str(SYMBOL_GEOMETRY_JSON).expect("the symbol geometry table is valid");
        assert_eq!(table.format, 1, "the symbol geometry table's format");
        table
    })
}

/// The geometry of a shipped symbol by its type, or `None` for a type that is not a shipped symbol.
#[must_use]
pub fn symbol_shape(kind: &str) -> Option<&'static SymbolShape> {
    let table = table();
    table
        .symbols
        .get(kind)
        .and_then(|shape| table.shapes.get(shape))
}

/// The room the label takes under a symbol: the tag and the state word, each 14 px (§14.2).
#[must_use]
pub fn label_height() -> f64 {
    table().label_height
}

#[cfg(test)]
#[path = "symbol_geometry_tests.rs"]
mod tests;
