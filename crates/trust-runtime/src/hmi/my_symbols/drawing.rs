//! A symbol's drawing, `hmi/symbols/<name>.svg` (specification 39, §14.4 item 3), cleaned when it
//! is read: shapes, text and definitions with their geometry, paint, text and transform
//! attributes, the declarations of `style` attributes and of class rules among those; references
//! only into the drawing; each id prefixed with the symbol's name. Scripts, event attributes,
//! embedded objects and references outside the drawing are refused.

use std::collections::{BTreeMap, BTreeSet};

use roxmltree::{Document, Node, ParsingOptions};

use crate::hmi::page_files::{error_at, PageFileError};

use super::SvgNode;

const SVG: &str = "http://www.w3.org/2000/svg";
const XLINK: &str = "http://www.w3.org/1999/xlink";
/// The most bytes and elements a drawing may have.
const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_NODES: u32 = 50_000;

/// The elements kept.
const KEPT: [&str; 17] = [
    "svg",
    "g",
    "path",
    "rect",
    "circle",
    "ellipse",
    "line",
    "polyline",
    "polygon",
    "use",
    "text",
    "tspan",
    "defs",
    "linearGradient",
    "radialGradient",
    "stop",
    "clipPath",
];
/// The elements that refuse the drawing: they run code, embed or link to what lies outside it.
const REFUSED: [&str; 9] = [
    "script",
    "foreignObject",
    "image",
    "iframe",
    "object",
    "embed",
    "audio",
    "video",
    "a",
];
/// The attributes kept, besides `id` and an internal `href`.
const ATTRIBUTES: [&str; 50] = [
    "d",
    "x",
    "y",
    "x1",
    "y1",
    "x2",
    "y2",
    "cx",
    "cy",
    "r",
    "rx",
    "ry",
    "fx",
    "fy",
    "width",
    "height",
    "points",
    "transform",
    "fill",
    "fill-opacity",
    "fill-rule",
    "stroke",
    "stroke-width",
    "stroke-opacity",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-miterlimit",
    "stroke-dasharray",
    "stroke-dashoffset",
    "opacity",
    "clip-path",
    "clip-rule",
    "clipPathUnits",
    "font-size",
    "font-family",
    "font-weight",
    "font-style",
    "text-anchor",
    "dominant-baseline",
    "letter-spacing",
    "offset",
    "stop-color",
    "stop-opacity",
    "gradientUnits",
    "gradientTransform",
    "spreadMethod",
    "visibility",
    "display",
    "vector-effect",
    "preserveAspectRatio",
];
/// The root's attributes that place the drawing rather than paint it.
const ROOT_PLACEMENT: [&str; 5] = ["x", "y", "width", "height", "preserveAspectRatio"];

/// The cleaned drawing: its coordinates, its tree and the ids it had.
pub(super) struct Drawing {
    pub(super) view_box: [f64; 4],
    pub(super) svg: SvgNode,
    pub(super) ids: BTreeSet<String>,
}

/// Cleans a drawing; the elements whose ids name parts take `data-part`.
pub(super) fn clean(
    file: &str,
    name: &str,
    text: &str,
    parts: &BTreeSet<String>,
) -> Result<Drawing, Vec<PageFileError>> {
    if text.len() > MAX_BYTES {
        return Err(vec![error_at(
            file,
            1,
            format!("the drawing is larger than {} MB", MAX_BYTES / 1024 / 1024),
        )]);
    }
    let options = ParsingOptions {
        allow_dtd: true,
        nodes_limit: MAX_NODES,
        ..ParsingOptions::default()
    };
    let document = Document::parse_with_options(text, options)
        .map_err(|error| vec![error_at(file, error.pos().row as usize, error.to_string())])?;
    let root = document.root_element();
    if root.tag_name().name() != "svg" || !matches!(root.tag_name().namespace(), None | Some(SVG)) {
        return Err(vec![error_at(
            file,
            1,
            "the drawing's root must be an svg element".to_string(),
        )]);
    }
    let Some(view_box) = view_box(root) else {
        return Err(vec![error_at(
            file,
            line(&document, root),
            "the drawing needs a viewBox, or a width and a height".to_string(),
        )]);
    };
    let mut cleaner = Cleaner {
        document: &document,
        file,
        prefix: format!("my-{name}-"),
        parts,
        classes: class_rules(&document),
        ids: BTreeSet::new(),
        errors: Vec::new(),
    };
    let mut svg = cleaner.element(root).unwrap_or_else(|| SvgNode {
        tag: "g".to_string(),
        attrs: BTreeMap::new(),
        children: Vec::new(),
        text: None,
    });
    svg.tag = "g".to_string();
    svg.attrs
        .retain(|key, _| !ROOT_PLACEMENT.contains(&key.as_str()) && key != "id");
    if cleaner.errors.is_empty() {
        Ok(Drawing {
            view_box,
            svg,
            ids: cleaner.ids,
        })
    } else {
        Err(cleaner.errors)
    }
}

struct Cleaner<'a, 'input> {
    document: &'a Document<'input>,
    file: &'a str,
    prefix: String,
    parts: &'a BTreeSet<String>,
    classes: BTreeMap<String, Vec<(String, String)>>,
    ids: BTreeSet<String>,
    errors: Vec<PageFileError>,
}

impl Cleaner<'_, '_> {
    /// An element cleaned, or nothing when it is left out.
    fn element(&mut self, node: Node<'_, '_>) -> Option<SvgNode> {
        let tag = node.tag_name();
        if !matches!(tag.namespace(), None | Some(SVG)) {
            return None;
        }
        let name = tag.name();
        let at = line(self.document, node);
        if REFUSED.contains(&name) {
            self.refuse(at, format!("the drawing may not hold an {name} element"));
            return None;
        }
        if !KEPT.contains(&name) {
            return None;
        }
        // Presentation attributes, then class rules, then the style attribute (CSS's order).
        let mut attrs = BTreeMap::new();
        let mut styled = Vec::new();
        for attribute in node.attributes() {
            let key = match attribute.namespace() {
                None => attribute.name(),
                Some(XLINK) if attribute.name() == "href" => "href",
                Some(_) => continue,
            };
            if key.len() > 2 && key[..2].eq_ignore_ascii_case("on") {
                self.refuse(
                    at,
                    format!("the drawing may not hold an event attribute ({key})"),
                );
                continue;
            }
            match key {
                "style" => styled.extend(declarations(attribute.value())),
                "class" => {
                    for class in attribute.value().split_whitespace() {
                        if let Some(rules) = self.classes.get(class) {
                            styled.splice(0..0, rules.iter().cloned());
                        }
                    }
                }
                "id" | "href" => {
                    attrs.insert(key.to_string(), attribute.value().to_string());
                }
                _ if ATTRIBUTES.contains(&key) => {
                    attrs.insert(key.to_string(), attribute.value().to_string());
                }
                _ => {}
            }
        }
        for (key, value) in styled {
            if ATTRIBUTES.contains(&key.as_str()) {
                attrs.insert(key, value);
            }
        }
        for (key, value) in attrs.iter_mut() {
            match self.internal(key, value) {
                Ok(rewritten) => *value = rewritten,
                Err(message) => self.refuse(at, message),
            }
        }
        if let Some(id) = attrs.get("id").cloned() {
            if self.parts.contains(&id) {
                attrs.insert("data-part".to_string(), id.clone());
            }
            attrs.insert("id".to_string(), format!("{}{id}", self.prefix));
            self.ids.insert(id);
        }
        let text = matches!(name, "text" | "tspan")
            .then(|| {
                node.children()
                    .filter(|child| child.is_text())
                    .filter_map(|child| child.text())
                    .collect::<String>()
            })
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty());
        let children = node
            .children()
            .filter(Node::is_element)
            .filter_map(|child| self.element(child))
            .collect();
        Some(SvgNode {
            tag: name.to_string(),
            attrs,
            children,
            text,
        })
    }

    /// An attribute's value with its references to the drawing prefixed; a reference outside it
    /// refuses the drawing.
    fn internal(&self, key: &str, value: &str) -> Result<String, String> {
        if key == "href" {
            return match value.strip_prefix('#') {
                Some(id) => Ok(format!("#{}{id}", self.prefix)),
                None => Err(format!(
                    "the drawing may not hold a reference outside the drawing ({value})"
                )),
            };
        }
        let mut rest = value;
        let mut out = String::new();
        while let Some(start) = rest.find("url(") {
            out.push_str(&rest[..start]);
            let after = &rest[start + 4..];
            let Some(end) = after.find(')') else {
                return Err(format!(
                    "the drawing may not hold a reference outside the drawing ({value})"
                ));
            };
            let target = after[..end].trim().trim_matches(|c| c == '"' || c == '\'');
            match target.strip_prefix('#') {
                Some(id) if !id.is_empty() => out.push_str(&format!("url(#{}{id})", self.prefix)),
                _ => {
                    return Err(format!(
                        "the drawing may not hold a reference outside the drawing ({value})"
                    ))
                }
            }
            rest = &after[end + 1..];
        }
        out.push_str(rest);
        Ok(out)
    }

    fn refuse(&mut self, line: usize, message: String) {
        self.errors.push(error_at(self.file, line, message));
    }
}

fn line(document: &Document<'_>, node: Node<'_, '_>) -> usize {
    document.text_pos_at(node.range().start).row as usize
}

/// The drawing's coordinates: its viewBox, or else its width and height from 0, 0.
pub(super) fn view_box(root: Node<'_, '_>) -> Option<[f64; 4]> {
    if let Some(text) = root.attribute("viewBox") {
        let numbers: Vec<f64> = text
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|part| !part.is_empty())
            .filter_map(|part| part.parse().ok())
            .collect();
        return match numbers.as_slice() {
            [x, y, width, height] if *width > 0.0 && *height > 0.0 => {
                Some([*x, *y, *width, *height])
            }
            _ => None,
        };
    }
    let length = |name: &str| {
        root.attribute(name)
            .and_then(|value| value.trim_end_matches("px").trim().parse::<f64>().ok())
            .filter(|value| *value > 0.0)
    };
    Some([0.0, 0.0, length("width")?, length("height")?])
}

/// The declarations of a style: `fill: red; stroke: none`.
fn declarations(style: &str) -> Vec<(String, String)> {
    style
        .split(';')
        .filter_map(|declaration| declaration.split_once(':'))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .filter(|(key, value)| !key.is_empty() && !value.is_empty())
        .collect()
}

/// The class rules of the drawing's `<style>` elements: `.name { … }`, also grouped as
/// `.a, .b { … }`; other selectors are left out.
fn class_rules(document: &Document<'_>) -> BTreeMap<String, Vec<(String, String)>> {
    let mut rules: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for style in document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "style")
    {
        let css: String = style.children().filter_map(|child| child.text()).collect();
        for rule in css.split('}') {
            let Some((selectors, body)) = rule.split_once('{') else {
                continue;
            };
            let found = declarations(body);
            for selector in selectors.split(',').map(str::trim) {
                if let Some(class) = selector.strip_prefix('.').filter(|class| {
                    class
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                        && !class.is_empty()
                }) {
                    rules
                        .entry(class.to_string())
                        .or_default()
                        .extend(found.iter().cloned());
                }
            }
        }
    }
    rules
}
