//! The project's own symbols (specification 39, §14.4): `hmi/symbols/<name>.svg`, the drawing,
//! cleaned when it is read, and `hmi/symbols/<name>.toml`, its definition: the bindings it takes,
//! its ports and the parts the bindings drive. A page places one as an object of type
//! `my:<name>`.

mod definition;
mod drawing;
mod starter;
mod write;

pub use starter::starter_definition;
pub use write::write_symbol;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::hmi::catalogue::{Catalogue, ComponentCategory, ComponentDef, PropertyDef, ValueKind};
use crate::hmi::page_files::{error_at, HmiBindingDirection, PageFileError};

/// Where the project's symbols are.
pub const SYMBOLS_FOLDER: &str = "hmi/symbols/";
/// The Components panel's heading for them.
pub const GROUP: &str = "my-symbols";

/// A symbol of the project.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct HmiSymbol {
    /// Its file name without `.toml`; pages name it `my:<name>`.
    pub name: String,
    /// Its name in the Components panel.
    pub title: String,
    /// A new object's size.
    pub size: [u32; 2],
    /// The drawing's own coordinates: x, y, width and height.
    #[serde(rename = "viewBox")]
    pub view_box: [f64; 4],
    /// The bindings it takes, with their kinds.
    pub properties: BTreeMap<String, ValueKind>,
    /// Where pipes connect, in the drawing's coordinates.
    pub ports: Vec<SymbolPort>,
    /// The parts the bindings drive, as the drawing's visualization rules.
    pub parts: Vec<SymbolPart>,
    /// The cleaned drawing, its root a group.
    pub svg: SvgNode,
}

/// A port of a symbol.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SymbolPort {
    /// Its name, which a pipe's end names.
    pub name: String,
    /// Where it lies, in the drawing's coordinates.
    pub at: [f64; 2],
    /// `left`, `right`, `up` or `down`.
    pub direction: String,
}

/// A part of the drawing and the binding that drives it, written as a visualization rule
/// (specification 39, §5.3 item 5): `{ "part": "rotor", "spin": "running" }`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SymbolPart {
    /// The element's id in the definition, before the drawing's ids are prefixed.
    pub part: String,
    /// It turns while this boolean property is TRUE.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spin: Option<String>,
    /// It moves to and fro while this boolean property is TRUE.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<String>,
    /// It stands at this number property, in degrees.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub angle: Option<String>,
    /// It is filled to this number property's share of the range.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    /// It is moved by this number property's share of the range times `move`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub travel: Option<String>,
    /// Its text shows this property.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show: Option<String>,
    /// It blinks while this boolean property is TRUE.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blink: Option<String>,
    /// It is painted in the alarm colour while this boolean property is TRUE.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alarm: Option<String>,
    /// It is painted in the moving colour while this boolean property is TRUE.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<String>,
    /// It is shown only while this number property equals `value`: one state among several.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub equals: Option<String>,
    /// For `level` and `travel`: the values of empty and full; the object's range when not given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<[f64; 2]>,
    /// For `travel`: how far the part moves at full travel, in the drawing's units.
    #[serde(rename = "move", skip_serializing_if = "Option::is_none")]
    pub move_by: Option<[f64; 2]>,
    /// For `equals`: the number the part is shown at.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
}

/// An element of a drawing, as the drawing module's nodes are written.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SvgNode {
    /// The element's name, `rect`.
    pub tag: String,
    /// Its attributes.
    pub attrs: BTreeMap<String, String>,
    /// Its child elements.
    pub children: Vec<SvgNode>,
    /// Its own text, for `text` and `tspan`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Whether a symbol's name is letters, digits, `-` and `_`.
pub fn is_symbol_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// A symbol from its definition and its drawing; each problem on its file's line.
pub fn parse_symbol(
    name: &str,
    definition: &str,
    drawing: &str,
) -> Result<HmiSymbol, Vec<PageFileError>> {
    let definition_file = format!("{SYMBOLS_FOLDER}{name}.toml");
    let drawing_file = format!("{SYMBOLS_FOLDER}{name}.svg");
    if !is_symbol_name(name) {
        return Err(vec![error_at(
            &definition_file,
            1,
            "a symbol's name has letters, digits, - and _ only".to_string(),
        )]);
    }
    let (definition, mut errors) = definition::read(&definition_file, definition)?;
    let parts: BTreeSet<String> = definition
        .parts
        .iter()
        .map(|(part, _)| part.part.clone())
        .collect();
    let drawing = match drawing::clean(&drawing_file, name, drawing, &parts) {
        Ok(drawing) => drawing,
        Err(mut refused) => {
            refused.extend(errors);
            return Err(refused);
        }
    };
    let [x, y, width, height] = drawing.view_box;
    for (port, line) in &definition.ports {
        let [px, py] = port.at;
        if px < x || py < y || px > x + width || py > y + height {
            errors.push(error_at(
                &definition_file,
                *line,
                format!("the port \"{}\" lies outside the drawing", port.name),
            ));
        }
    }
    for (part, line) in &definition.parts {
        if !drawing.ids.contains(&part.part) {
            errors.push(error_at(
                &definition_file,
                *line,
                format!("the drawing has no element \"{}\"", part.part),
            ));
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(HmiSymbol {
        name: name.to_string(),
        title: definition.title,
        size: definition.size,
        view_box: drawing.view_box,
        properties: definition.properties,
        ports: definition.ports.into_iter().map(|(port, _)| port).collect(),
        parts: definition.parts.into_iter().map(|(part, _)| part).collect(),
        svg: drawing.svg,
    })
}

/// The symbols among a set of files by path (a bundle's, or a project's `hmi/` folder), and the
/// problems of those that do not load.
pub fn symbols_in(files: &BTreeMap<String, Vec<u8>>) -> (Vec<HmiSymbol>, Vec<PageFileError>) {
    let mut symbols = Vec::new();
    let mut errors = Vec::new();
    for (path, bytes) in files {
        let Some(name) = path
            .strip_prefix(SYMBOLS_FOLDER)
            .and_then(|rest| rest.strip_suffix(".toml"))
        else {
            continue;
        };
        let drawing_path = format!("{SYMBOLS_FOLDER}{name}.svg");
        let Some(drawing) = files.get(&drawing_path) else {
            errors.push(error_at(
                path,
                1,
                format!("the symbol has no drawing ({drawing_path})"),
            ));
            continue;
        };
        match parse_symbol(
            name,
            &String::from_utf8_lossy(bytes),
            &String::from_utf8_lossy(drawing),
        ) {
            Ok(symbol) => symbols.push(symbol),
            Err(problems) => errors.extend(problems),
        }
    }
    (symbols, errors)
}

/// The symbols of a project's `hmi/symbols/` folder, and the problems of those that do not load.
pub fn project_symbols(project: &std::path::Path) -> (Vec<HmiSymbol>, Vec<PageFileError>) {
    let mut files = BTreeMap::new();
    if let Ok(entries) = std::fs::read_dir(project.join(SYMBOLS_FOLDER)) {
        for entry in entries.flatten() {
            let file = entry.file_name().to_string_lossy().into_owned();
            if file.ends_with(".toml") || file.ends_with(".svg") {
                let path = format!("{SYMBOLS_FOLDER}{file}");
                if let Some(bytes) = crate::hmi::bundle::read_hmi_file(project, &path) {
                    files.insert(path, bytes);
                }
            }
        }
    }
    symbols_in(&files)
}

impl HmiSymbol {
    /// The symbol as a component of the catalogue: `my:<name>`, under My Symbols, its bindings
    /// read-only.
    pub fn component(&self) -> ComponentDef {
        ComponentDef {
            kind: format!("my:{}", self.name),
            category: ComponentCategory::Equipment,
            phase: 3,
            size: self.size,
            variants: Vec::new(),
            properties: self
                .properties
                .iter()
                .map(|(name, kind)| {
                    (
                        name.clone(),
                        PropertyDef {
                            accepts: vec![*kind],
                            directions: vec![HmiBindingDirection::Read],
                        },
                    )
                })
                .collect(),
            additional: None,
            ports: self.ports.iter().map(|port| port.name.clone()).collect(),
            attaches: false,
            group: Some(GROUP.to_string()),
        }
    }
}

impl Catalogue {
    /// The catalogue with the project's symbols as components, `my:<name>`, under My Symbols.
    pub fn with_symbols(&self, symbols: &[HmiSymbol]) -> Catalogue {
        let mut catalogue = self.clone();
        catalogue.components.extend(symbols.iter().map(|symbol| {
            let mut component = symbol.component();
            component.with_visibility();
            component
        }));
        catalogue
    }
}

#[cfg(test)]
mod tests;
