//! Specification 39, §14.4: the project's own symbols, a definition and a cleaned drawing.

use super::*;
use crate::hmi::catalogue::{Catalogue, ComponentCategory, ValueKind};

pub(crate) const DEFINITION: &str = r#"format = 1
title = "Agitator tank"
size = [128, 108]

[properties]
running = "boolean"
level = "number"
fault = "boolean"

[[port]]
name = "inlet"
at = [0, 32]
direction = "left"

[[part]]
id = "rotor"
spin = "running"

[[part]]
id = "liquid"
level = "level"
range = [0, 100]

[[part]]
id = "reading"
show = "level"
"#;

pub(crate) const DRAWING: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" width="64" height="64" viewBox="0 0 64 64" inkscape:version="1.3">
  <sodipodi:namedview id="namedview1" />
  <!-- drawn in Inkscape -->
  <defs><linearGradient id="shine"><stop offset="0" stop-color="#fff"/><stop offset="1" stop-color="#999"/></linearGradient></defs>
  <style>.body { fill: #334; stroke: #889 }</style>
  <rect id="tank" class="body" x="8" y="8" width="48" height="48" rx="6" style="stroke-width:2;fill:url(#shine)" inkscape:label="Tank"/>
  <rect id="liquid" x="10" y="30" width="44" height="24" fill="#3cf"/>
  <g id="rotor"><path d="M32 20 L36 32 L32 44 L28 32 Z" fill="#ccc"/></g>
  <text id="reading" x="32" y="60" font-size="8">0</text>
</svg>
"##;

fn child<'a>(node: &'a SvgNode, id: &str) -> &'a SvgNode {
    fn find<'a>(node: &'a SvgNode, id: &str) -> Option<&'a SvgNode> {
        if node.attrs.get("id").map(String::as_str) == Some(id) {
            return Some(node);
        }
        node.children.iter().find_map(|child| find(child, id))
    }
    find(node, id).unwrap_or_else(|| panic!("{id} in {node:#?}"))
}

#[test]
fn a_symbol_reads_its_definition_and_a_cleaned_drawing() {
    let symbol = parse_symbol("agitator", DEFINITION, DRAWING).expect("valid");
    assert_eq!(symbol.title, "Agitator tank");
    assert_eq!(symbol.size, [128, 108]);
    assert_eq!(symbol.view_box, [0.0, 0.0, 64.0, 64.0]);
    assert_eq!(symbol.properties.get("running"), Some(&ValueKind::Boolean));
    assert_eq!(symbol.ports[0].name, "inlet");
    let rules = serde_json::to_value(&symbol.parts).expect("rules");
    assert_eq!(
        rules,
        serde_json::json!([
            { "part": "rotor", "spin": "running" },
            { "part": "liquid", "level": "level", "range": [0.0, 100.0] },
            { "part": "reading", "show": "level" }
        ])
    );
    let svg = &symbol.svg;
    assert_eq!(svg.tag, "g", "the drawing's root becomes a group");
    let tags: Vec<&str> = svg
        .children
        .iter()
        .map(|child| child.tag.as_str())
        .collect();
    assert_eq!(
        tags,
        ["defs", "rect", "rect", "g", "text"],
        "no editor elements, comments or style"
    );
    let tank = child(svg, "my-agitator-tank");
    assert_eq!(
        tank.attrs.get("fill").map(String::as_str),
        Some("url(#my-agitator-shine)"),
        "the style wins, its reference prefixed"
    );
    assert_eq!(
        tank.attrs.get("stroke").map(String::as_str),
        Some("#889"),
        "from its class"
    );
    assert_eq!(
        tank.attrs.get("stroke-width").map(String::as_str),
        Some("2")
    );
    assert!(
        !tank
            .attrs
            .keys()
            .any(|key| key.contains("label") || key == "class" || key == "style"),
        "{:?}",
        tank.attrs
    );
    child(svg, "my-agitator-shine");
    assert_eq!(
        child(svg, "my-agitator-rotor")
            .attrs
            .get("data-part")
            .map(String::as_str),
        Some("rotor")
    );
    let reading = child(svg, "my-agitator-reading");
    assert_eq!(
        (
            reading.attrs.get("data-part").map(String::as_str),
            reading.text.as_deref()
        ),
        (Some("reading"), Some("0"))
    );
}

#[test]
fn a_drawing_with_a_script_an_event_or_an_outside_reference_is_refused() {
    for (drawing, line, words) in [
        ("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 64 64\">\n<script>alert(1)</script>\n</svg>", 2, "script"),
        ("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 64 64\">\n<rect onclick=\"alert(1)\" width=\"4\" height=\"4\"/>\n</svg>", 2, "onclick"),
        ("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 64 64\">\n\n<rect fill=\"url(https://evil.test/x)\" width=\"4\" height=\"4\"/>\n</svg>", 3, "outside the drawing"),
        ("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 64 64\">\n<image href=\"logo.png\" width=\"4\" height=\"4\"/>\n</svg>", 2, "image"),
    ] {
        let errors = parse_symbol("agitator", "format = 1\ntitle = \"A\"\nsize = [128, 108]\n", drawing).expect_err(words);
        assert_eq!((errors[0].file.as_str(), errors[0].line), ("hmi/symbols/agitator.svg", line), "{errors:?}");
        assert!(errors[0].message.contains(words), "{words}: {errors:?}");
    }
}

#[test]
fn a_definitions_problems_are_reported_on_their_lines() {
    let broken = DEFINITION
        .replace("id = \"rotor\"", "id = \"stirrer\"")
        .replace("show = \"level\"", "show = \"flow\"")
        .replace("at = [0, 32]", "at = [80, 32]")
        .replace("spin = \"running\"", "spin = \"level\"");
    let errors = parse_symbol("agitator", &broken, DRAWING).expect_err("refused");
    let found: Vec<(usize, &str)> = errors
        .iter()
        .map(|error| (error.line, error.message.as_str()))
        .collect();
    let line = |text: &str| broken.lines().position(|line| line == text).expect(text) + 1;
    assert!(
        errors
            .iter()
            .all(|error| error.file == "hmi/symbols/agitator.toml"),
        "{errors:?}"
    );
    for (at, words) in [
        (line("at = [80, 32]"), "outside the drawing"),
        (line("id = \"stirrer\""), "no element \"stirrer\""),
        (line("spin = \"level\""), "spin needs a boolean"),
        (line("show = \"flow\""), "no property \"flow\""),
    ] {
        assert!(
            found
                .iter()
                .any(|(line, message)| *line == at && message.contains(words)),
            "{words} on {at}: {found:?}"
        );
    }
}

/// The agitator's parts driven the newer ways: the rotor strokes, the liquid travels, the
/// reading shows only in state 2.
fn moving_definition() -> String {
    DEFINITION
        .replace(
            "level = \"number\"\n",
            "level = \"number\"\nstate = \"number\"\n",
        )
        .replace("spin = \"running\"", "stroke = \"running\"")
        .replace(
            "level = \"level\"\nrange = [0, 100]",
            "travel = \"level\"\nrange = [0, 100]\nmove = [0, -12]",
        )
        .replace("show = \"level\"", "equals = \"state\"\nvalue = 2")
}

#[test]
fn a_part_may_stroke_travel_or_show_only_in_one_state() {
    let symbol = parse_symbol("agitator", &moving_definition(), DRAWING).expect("valid");
    let part = |id: &str| symbol.parts.iter().find(|part| part.part == id).expect(id);
    assert_eq!(part("rotor").stroke.as_deref(), Some("running"));
    assert_eq!(
        (part("liquid").travel.as_deref(), part("liquid").move_by),
        (Some("level"), Some([0.0, -12.0]))
    );
    assert_eq!(
        (part("reading").equals.as_deref(), part("reading").value),
        (Some("state"), Some(2.0))
    );
    let written = write::write_symbol(&symbol);
    let again = parse_symbol("agitator", &written, DRAWING).expect("the written definition reads");
    assert_eq!(again.parts, symbol.parts, "{written}");
}

#[test]
fn equals_needs_its_value_and_travel_its_displacement() {
    let broken = moving_definition()
        .replace("\nvalue = 2", "")
        .replace("\nmove = [0, -12]", "");
    let errors = parse_symbol("agitator", &broken, DRAWING).expect_err("refused");
    let line = |text: &str| broken.lines().position(|line| line == text).expect(text) + 1;
    let found: Vec<(usize, &str)> = errors
        .iter()
        .map(|error| (error.line, error.message.as_str()))
        .collect();
    for (at, words) in [
        (line("equals = \"state\""), "give value"),
        (line("travel = \"level\""), "give move"),
    ] {
        assert!(
            found
                .iter()
                .any(|(line, message)| *line == at && message.contains(words)),
            "{words} on {at}: {found:?}"
        );
    }
}

#[test]
fn the_catalogue_takes_the_projects_symbols() {
    let symbol = parse_symbol("agitator", DEFINITION, DRAWING).expect("valid");
    let catalogue = Catalogue::shipped().with_symbols(&[symbol]);
    let component = catalogue
        .component("my:agitator")
        .expect("the symbol is a component");
    assert_eq!(component.category, ComponentCategory::Equipment);
    assert_eq!(component.group.as_deref(), Some("my-symbols"));
    assert_eq!((component.phase, component.size), (3, [128, 108]));
    assert_eq!(component.ports, ["inlet"]);
    assert_eq!(component.properties["level"].accepts, [ValueKind::Number]);
    assert!(
        catalogue.component("pump").is_some(),
        "the shipped components stay"
    );
}
