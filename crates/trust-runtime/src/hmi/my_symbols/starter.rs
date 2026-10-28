//! The definition a newly imported symbol starts with (specification 39, §14.4 item 5): its title,
//! a new object's size in the drawing's proportions, and each element id of the drawing as a part
//! to name, with commented examples of the properties, ports and drives the engineer completes. The
//! drawing is read as the symbol reader reads it, so the proportions are those the symbol is drawn
//! in.

use roxmltree::{Document, ParsingOptions};

use super::drawing::view_box;
use crate::hmi::page_files::toml_string as quoted;

/// A new object's width; the symbol is drawn this wide above its tag and state word.
const SYMBOL_WIDTH: f64 = 64.0;
/// The object's width, and the height of its tag and state word under the symbol.
const OBJECT_WIDTH: u32 = 128;
const LABEL_HEIGHT: u32 = 44;
/// The lowest height a symbol is drawn at.
const MIN_SYMBOL_HEIGHT: f64 = 16.0;

/// The definition to complete for the drawing `drawing` titled `title`. A drawing without a
/// viewBox, a width and a height is taken as square.
///
/// # Errors
///
/// Why the drawing is no XML document.
pub fn starter_definition(title: &str, drawing: &str) -> Result<String, String> {
    let options = ParsingOptions {
        allow_dtd: true,
        ..ParsingOptions::default()
    };
    let document = Document::parse_with_options(drawing, options)
        .map_err(|error| format!("the drawing does not load: {error}"))?;
    let [_, _, width, height] = view_box(document.root_element()).unwrap_or([0.0, 0.0, 1.0, 1.0]);
    // As wide as the symbol is drawn, as high as the drawing's proportions make it.
    let symbol_height = (SYMBOL_WIDTH * height / width)
        .round()
        .max(MIN_SYMBOL_HEIGHT) as u32;
    let mut ids: Vec<&str> = Vec::new();
    for id in document
        .descendants()
        .filter_map(|node| node.attribute("id"))
    {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    let mut lines = vec![
        "format = 1".to_owned(),
        format!("title = {}", quoted(title)),
        format!("size = [{OBJECT_WIDTH}, {}]", symbol_height + LABEL_HEIGHT),
        String::new(),
        "# The bindings the symbol takes, each boolean, number or text, for example:".to_owned(),
        "[properties]".to_owned(),
        "# running = \"boolean\"".to_owned(),
        "# level = \"number\"".to_owned(),
        String::new(),
        "# Ports where pipes connect, in the drawing's coordinates, for example:".to_owned(),
        "# [[port]]".to_owned(),
        "# name = \"inlet\"".to_owned(),
        "# at = [0, 32]".to_owned(),
        "# direction = \"left\"".to_owned(),
        String::new(),
        "# The drawing's elements. Name the ones the bindings drive, each one way:".to_owned(),
        "# spin, angle, level, show, blink, alarm or active.".to_owned(),
    ];
    for id in ids {
        lines.extend([
            String::new(),
            "# [[part]]".to_owned(),
            format!("# id = {}", quoted(id)),
            "# spin = \"running\"".to_owned(),
        ]);
    }
    Ok(format!("{}\n", lines.join("\n")))
}

#[cfg(test)]
#[path = "starter_tests.rs"]
mod tests;
