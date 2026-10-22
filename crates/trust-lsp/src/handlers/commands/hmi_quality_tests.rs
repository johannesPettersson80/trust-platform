//! The editor's quality warnings (specification 39, §14.1 items 3 and 4, §14.5 item 6, §18 phase 5), ported from the TypeScript
//! checks (`hmi-builder-quality.test.ts`): each case gives the drawing's reported facts where a check needs them, and every expected
//! sentence and number is worked out by hand from the specification's thresholds and the WCAG 2.1 and OKLab formulas.

use std::collections::BTreeMap;

use serde_json::{json, Value};
use trust_runtime::hmi::catalogue::Catalogue;

use super::{
    colour_warnings, display_warning, object_warnings, oklab_distance, text_contrast,
    variable_type, Backing, ColourQuality, DrawnText, LanguageCuts, PageQuality, NO_DISPLAY,
};

/// The variable picker's tree of a pump station (the TypeScript test's `VARIABLES`): REAL, LREAL, DINT and BOOL values, and an array
/// of tanks whose element holds a REAL level.
fn variables() -> Vec<Value> {
    let value = |name: &str, kind: &str| json!({ "name": name, "path": format!("Station.{name}"), "kind": "value", "type": kind, "kinds": [], "writable": true });
    vec![json!({
        "name": "Station", "path": "Station", "kind": "program", "type": "PumpStation", "kinds": [], "writable": false,
        "children": [
            value("Speed", "REAL"), value("Flow", "LREAL"), value("Count", "DINT"), value("Start", "BOOL"),
            { "name": "Tanks", "path": "Station.Tanks", "kind": "array", "type": "ARRAY[1..3] OF Tank", "kinds": [],
              "writable": false, "array": { "dimensions": [[1, 3]], "element": {
                  "name": "Tanks[1]", "path": "Station.Tanks[1]", "kind": "structure", "type": "Tank", "kinds": [],
                  "writable": false, "children": [value("Level", "REAL")] } } }
        ]
    })]
}

/// A page of `objects`, as the file's model writes them, with the drawing's `cuts`.
fn page(objects: Vec<Value>, cuts: Value) -> PageQuality {
    serde_json::from_value(json!({ "objects": objects, "cuts": cuts })).expect("the page decodes")
}

/// An object of `kind` with the component's size, or 160 × 48 px, and `extra` fields.
fn object(id: &str, kind: &str, extra: Value) -> Value {
    let size = Catalogue::shipped()
        .component(kind)
        .map_or([160, 48], |component| component.size);
    let mut value =
        json!({ "id": id, "type": kind, "at": [0, 0], "size": size, "rotation": 0, "bind": {} });
    for (key, field) in extra.as_object().expect("an object").clone() {
        value[key] = field;
    }
    value
}

/// An object whose `property` reads `variable`.
fn bound(id: &str, kind: &str, property: &str, variable: &str, extra: Value) -> Value {
    let mut value = object(id, kind, extra);
    value["bind"] = json!({ property: { "path": variable, "direction": "read" } });
    value
}

/// The objects a kind of warning names, in order.
fn named(warnings: &[super::QualityWarning], kind: &str) -> Vec<String> {
    warnings
        .iter()
        .filter(|warning| warning.kind == kind)
        .filter_map(|warning| warning.object.clone())
        .collect()
}

#[test]
fn a_control_a_page_button_or_an_object_with_a_click_below_44_by_44_px_is_flagged() {
    let page = page(
        vec![
            bound(
                "start",
                "button",
                "target",
                "Station.Start",
                json!({ "size": [120, 36], "text": "Start" }),
            ),
            object(
                "trends",
                "page-button",
                json!({ "size": [160, 40], "text": "Trends", "click": { "page": "trends" } }),
            ),
            bound(
                "speed",
                "value",
                "value",
                "Station.Speed",
                json!({ "size": [160, 40], "unit": "rpm" }),
            ),
            bound(
                "detail",
                "value",
                "value",
                "Station.Speed",
                json!({ "size": [40, 40], "unit": "rpm", "click": { "popup": "pump" } }),
            ),
            bound(
                "stop",
                "button",
                "target",
                "Station.Start",
                json!({ "size": [120, 48], "text": "Stop" }),
            ),
        ],
        json!([]),
    );
    let warnings = object_warnings(&page, Catalogue::shipped(), &variables());
    // Catches a display read as a touch target (speed), and a 48 px tall control flagged (stop).
    assert_eq!(
        named(&warnings, "touch-target"),
        ["start", "trends", "detail"]
    );
    assert_eq!(
        warnings[0].message,
        "Start is 120 × 36 px; a touch target needs at least 44 × 44 px."
    );
}

#[test]
fn a_number_bound_to_a_real_without_a_unit_is_flagged_and_an_empty_unit_says_it_has_none() {
    let page = page(
        vec![
            bound("speed", "value", "value", "Station.Speed", json!({})),
            bound("flow", "gauge", "value", "Station.Flow", json!({})),
            bound("level", "bar", "value", "Station.Tanks[2].Level", json!({})),
            bound("count", "value", "value", "Station.Count", json!({})),
            bound(
                "withUnit",
                "value",
                "value",
                "Station.Speed",
                json!({ "unit": "rpm" }),
            ),
            bound(
                "none",
                "value",
                "value",
                "Station.Speed",
                json!({ "unit": "" }),
            ),
            bound("unknownPath", "value", "value", "Station.Gone", json!({})),
        ],
        json!([]),
    );
    let warnings = object_warnings(&page, Catalogue::shipped(), &variables());
    // Catches a DINT taken for a REAL, an array element not reached through its index, and "No unit" (`unit = ""`) flagged.
    assert_eq!(named(&warnings, "unit"), ["speed", "flow", "level"]);
    assert_eq!(
        warnings
            .iter()
            .find(|warning| warning.kind == "unit")
            .map(|warning| warning.message.as_str()),
        Some("speed shows a REAL value without a unit. Add its unit, or choose No unit if the number has none.")
    );
}

#[test]
fn an_object_that_takes_bindings_but_has_none_is_flagged_and_static_text_layout_and_drawings_are_not(
) {
    let page = page(
        vec![
            object("lonely", "value", json!({ "label": "Level" })),
            object("trend", "trend", json!({})),
            object("title", "text", json!({ "text": "Feed station" })),
            object("frame", "panel", json!({})),
            object("log", "event-log", json!({})),
            object("vessel", "tank", json!({})),
        ],
        json!([]),
    );
    let warnings = object_warnings(&page, Catalogue::shipped(), &variables());
    // Catches the visibility binding, which every component may have, counted as a binding the object needs (frame, log).
    assert_eq!(named(&warnings, "unbound"), ["lonely", "trend"]);
    assert_eq!(
        warnings[0].message,
        "Level is not bound to a variable, so it shows nothing in the runtime."
    );
}

#[test]
fn a_cut_label_is_flagged_once_and_names_the_field_it_was_cut_in() {
    let page = page(
        vec![bound(
            "long",
            "value",
            "value",
            "Station.Speed",
            json!({ "label": "Feed pump P101 speed", "unit": "rpm" }),
        )],
        json!([{ "cuts": [
            { "object": "long", "text": "Feed pump P101 speed" },
            { "object": "long", "text": "Feed pump P101 speed" }
        ] }]),
    );
    let warnings = object_warnings(&page, Catalogue::shipped(), &variables());
    assert_eq!(
        warnings
            .iter()
            .map(|warning| (warning.kind, warning.message.as_str()))
            .collect::<Vec<_>>(),
        [(
            "cut-text",
            "The label \"Feed pump P101 speed\" is cut to fit: make the object wider or the label shorter."
        )]
    );
}

#[test]
fn a_text_cut_in_one_of_several_languages_is_flagged_with_that_language() {
    let languages = json!([
        { "language": { "tag": "en", "name": "English", "texts": {} }, "cuts": [] },
        { "language": { "tag": "de", "name": "Deutsch", "texts": { "Start": "Pumpe jetzt sofort starten" } },
          "cuts": [{ "object": "start", "text": "Pumpe jetzt sofort starten" }] },
        { "language": { "tag": "sv", "name": "Svenska", "texts": { "Start": "Starta" } }, "cuts": [] }
    ]);
    let page = page(
        vec![object(
            "start",
            "text",
            json!({ "size": [120, 40], "text": "Start" }),
        )],
        languages,
    );
    let warnings = object_warnings(&page, Catalogue::shipped(), &variables());
    assert_eq!(named(&warnings, "cut-text"), ["start"]);
    // The cut words are the object's text as German shows it, so the field is its text.
    assert_eq!(
        warnings[0].message,
        "The text \"Pumpe jetzt sofort starten\" is cut to fit in Deutsch: make the object wider or the text shorter."
    );
}

#[test]
fn one_of_trusts_own_words_cut_in_a_language_is_a_word() {
    let languages = json!([
        { "language": { "tag": "en", "name": "English", "texts": {} }, "cuts": [] },
        { "language": { "tag": "sv", "name": "Svenska", "texts": { "Running": "Pumpen går för fullt hela tiden" } },
          "cuts": [{ "object": "pump", "text": "Pumpen går för fullt hela tiden" }] }
    ]);
    let page = page(
        vec![bound(
            "pump",
            "pump",
            "running",
            "Station.Start",
            json!({ "tag": "P-101" }),
        )],
        languages,
    );
    let warnings = object_warnings(&page, Catalogue::shipped(), &variables());
    assert_eq!(
        warnings
            .iter()
            .filter(|warning| warning.kind == "cut-text")
            .map(|warning| warning.message.as_str())
            .collect::<Vec<_>>(),
        ["The word \"Pumpen går för fullt hela tiden\" is cut to fit in Svenska: make the object wider or the word shorter."]
    );
}

#[test]
fn a_pid_warns_of_its_cut_texts_only() {
    // §18 phase 4 item 1: a P&ID is a drawing that never runs, so an unbound value on it is no warning; its cut label is.
    let mut page = page(
        vec![object(
            "lonely",
            "value",
            json!({ "label": "Level of the feed tank" }),
        )],
        json!([{ "cuts": [{ "object": "lonely", "text": "Level of the feed tank" }] }]),
    );
    page.pid = true;
    let warnings = object_warnings(&page, Catalogue::shipped(), &variables());
    assert_eq!(
        warnings
            .iter()
            .map(|warning| warning.kind)
            .collect::<Vec<_>>(),
        ["cut-text"]
    );
}

#[test]
fn a_variable_path_reaches_array_elements_and_remote_members_by_name() {
    let tree = variables();
    assert_eq!(
        variable_type(&tree, "Station.Tanks[2].Level").as_deref(),
        Some("REAL")
    );
    assert_eq!(
        variable_type(&tree, "station.speed").as_deref(),
        Some("REAL"),
        "names match in any case"
    );
    assert_eq!(variable_type(&tree, "Station.Gone"), None);
    assert_eq!(
        variable_type(&tree, "Station.Tanks[2"),
        None,
        "an unclosed index names no variable"
    );
    let remote = vec![json!({ "name": "Speed", "path": "plc2:Speed", "type": "LREAL" })];
    assert_eq!(
        variable_type(&remote, "plc2:speed").as_deref(),
        Some("LREAL")
    );
    assert_eq!(variable_type(&remote, "plc3:speed"), None);
}

#[test]
fn a_program_without_the_display_block_is_warned_of_and_one_without_variables_is_not() {
    let mut tree = variables();
    assert_eq!(display_warning(&tree), Some(NO_DISPLAY));
    tree[0]["children"]
        .as_array_mut()
        .expect("children")
        .push(json!({ "name": "Screens", "path": "Station.Screens", "type": "Hmi.Display" }));
    assert_eq!(
        display_warning(&tree),
        None,
        "a nested Hmi.Display serves the screens"
    );
    assert_eq!(
        display_warning(&[]),
        None,
        "a program that does not compile has no variables"
    );
}

/// The palette of the dark theme that these cases need (the drawing's tokens).
fn dark_defaults() -> BTreeMap<String, String> {
    [
        ("page", "#06080c"),
        ("surface", "#0d1117"),
        ("text", "#eceff4"),
        ("muted", "#8a94a8"),
        ("accent", "#22d3ee"),
        ("pen-3", "#818cf8"),
        ("stale", "#e3b24f"),
        ("warning", "#fbbf24"),
        ("alarm", "#fb4f6e"),
        ("alarm-high", "#fb4f6e"),
        ("alarm-medium", "#fb923c"),
        ("alarm-low", "#facc15"),
    ]
    .into_iter()
    .map(|(token, colour)| (token.to_owned(), colour.to_owned()))
    .collect()
}

/// A text in `colour` drawn on one opaque layer of `behind`.
fn drawn(text: &str, colour: &str, behind: &str, size: f64, bold: bool, sign: bool) -> DrawnText {
    DrawnText {
        text: text.to_owned(),
        colour: colour.to_owned(),
        behind: vec![vec![Backing {
            token: behind.to_owned(),
            opacity: 1.0,
            mix: None,
        }]],
        size,
        bold,
        sign,
    }
}

#[test]
fn the_contrast_ratio_is_the_wcag_ratio_and_large_or_bold_text_and_signs_need_three_to_one() {
    let palette: BTreeMap<String, String> = [
        ("black", "#000000"),
        ("white", "#ffffff"),
        ("grey", "#777777"),
    ]
    .into_iter()
    .map(|(token, colour)| (token.to_owned(), colour.to_owned()))
    .collect();
    // Black on white: (1 + 0.05) / (0 + 0.05) = 21.
    assert_eq!(
        text_contrast(&drawn("A", "black", "white", 14.0, false, false), &palette),
        (21.0, 4.5)
    );
    // #777 on white, the WCAG reference 4.48:1: luminance 0.18448, so 1.05 / 0.23448 = 4.478.
    let (grey, needed) = text_contrast(&drawn("A", "grey", "white", 14.0, false, false), &palette);
    assert!(
        (grey - 4.478).abs() < 1e-3 && needed == 4.5,
        "{grey} {needed}"
    );
    assert_eq!(
        text_contrast(&drawn("A", "grey", "white", 24.0, false, false), &palette).1,
        3.0
    );
    assert_eq!(
        text_contrast(&drawn("A", "grey", "white", 18.66, true, false), &palette).1,
        3.0
    );
    assert_eq!(
        text_contrast(&drawn("A", "grey", "white", 18.0, true, false), &palette).1,
        4.5
    );
    assert_eq!(
        text_contrast(&drawn("!", "grey", "white", 14.0, false, true), &palette).1,
        3.0
    );
    // Black text on a half-transparent black layer over white: the layer gives grey 0.5, luminance 0.214041, so the black text has
    // (0.214041 + 0.05) / (0 + 0.05) = 5.2808.
    let mut faint = drawn("A", "black", "white", 14.0, false, false);
    faint.behind = vec![
        vec![Backing {
            token: "black".to_owned(),
            opacity: 0.5,
            mix: None,
        }],
        vec![Backing {
            token: "white".to_owned(),
            opacity: 1.0,
            mix: None,
        }],
    ];
    let (ratio, _) = text_contrast(&faint, &palette);
    assert!(
        (ratio - 5.2808).abs() < 1e-3,
        "black text on white under half-transparent black: {ratio}"
    );
}

#[test]
fn the_shipped_colours_raise_no_colour_warnings() {
    let colours = ColourQuality {
        defaults: BTreeMap::from([("dark".to_owned(), dark_defaults())]),
        own: BTreeMap::new(),
        texts: vec![drawn("Level", "muted", "surface", 14.0, false, false)],
    };
    assert_eq!(colour_warnings(&colours), []);
}

#[test]
fn a_project_colour_that_gives_text_too_little_contrast_is_flagged_at_the_colour_with_the_ratio() {
    let colours = ColourQuality {
        defaults: BTreeMap::from([("dark".to_owned(), dark_defaults())]),
        own: BTreeMap::from([(
            "dark".to_owned(),
            vec![("muted".to_owned(), "#1f2430".to_owned())],
        )]),
        texts: vec![
            drawn("Level", "muted", "surface", 14.0, false, false),
            drawn("Title", "text", "surface", 14.0, false, false),
        ],
    };
    let warnings = colour_warnings(&colours);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0].kind, "contrast");
    let colour = warnings[0].colour.as_ref().expect("the colour");
    assert_eq!(
        (colour.theme.as_str(), colour.token.as_str()),
        ("dark", "muted")
    );
    // #1f2430 has luminance 0.017663 and #0d1117 0.005485: (0.067663) / (0.055485) = 1.22.
    assert_eq!(
        warnings[0].message,
        "With this colour, text in \"muted\" on \"surface\" has a contrast of 1.22:1 in the dark theme; it needs 4.5:1."
    );
}

#[test]
fn a_project_colour_that_looks_like_the_warning_or_an_alarm_colour_is_flagged() {
    let colours = ColourQuality {
        defaults: BTreeMap::from([("dark".to_owned(), dark_defaults())]),
        own: BTreeMap::from([(
            "dark".to_owned(),
            vec![
                ("accent".to_owned(), "#f97316".to_owned()),
                ("pen-3".to_owned(), "#ff5470".to_owned()),
                ("stale".to_owned(), "#fb4f6e".to_owned()),
                ("font".to_owned(), "Inter".to_owned()),
            ],
        )]),
        texts: Vec::new(),
    };
    let warnings = colour_warnings(&colours);
    // The stale colour is no data colour, and `font` is no colour.
    assert_eq!(
        warnings
            .iter()
            .map(|warning| {
                let colour = warning.colour.as_ref().expect("a colour");
                (warning.kind, colour.token.as_str())
            })
            .collect::<Vec<_>>(),
        [("look-alike", "accent"), ("look-alike", "pen-3")]
    );
    assert!(warnings[0]
        .message
        .starts_with("accent looks like the medium-priority alarm colour in the dark theme ("));
    assert!(warnings[1]
        .message
        .starts_with("pen-3 looks like the alarm colour in the dark theme ("));
    assert!(warnings[1]
        .message
        .ends_with(" apart), so data could be taken for an alarm."));
}

#[test]
fn the_oklab_distance_is_zero_for_one_colour_and_one_from_black_to_white() {
    assert_eq!(oklab_distance("#336699", "#336699"), 0.0);
    assert!((oklab_distance("#000000", "#ffffff") - 1.0).abs() < 1e-5);
    assert_eq!(oklab_distance("#000000", "not a colour"), f64::INFINITY);
}

#[test]
fn the_command_decodes_the_drawings_report() {
    let cuts: Vec<LanguageCuts> = serde_json::from_value(json!([
        { "language": { "tag": "de", "name": "Deutsch", "texts": { "Start": "Starten" } },
          "cuts": [{ "object": "start", "text": "Starten" }] }
    ]))
    .expect("the cuts decode");
    assert_eq!(cuts[0].cuts[0].object, "start");
    let colours: ColourQuality = serde_json::from_value(json!({
        "defaults": { "dark": { "text": "#eceff4" } },
        "own": { "dark": [["muted", "#1f2430"]] },
        "texts": [{ "text": "Level", "colour": "muted", "behind": [[{ "token": "surface", "opacity": 1 }]],
            "size": 14, "bold": false, "sign": false }]
    }))
    .expect("the colours decode");
    assert_eq!(
        colours.own["dark"][0],
        ("muted".to_owned(), "#1f2430".to_owned())
    );
}

#[test]
fn an_alarm_colour_the_project_changes_is_no_look_alike_of_the_alarms() {
    let colours = ColourQuality {
        defaults: BTreeMap::from([("dark".to_owned(), dark_defaults())]),
        own: BTreeMap::from([(
            "dark".to_owned(),
            vec![("alarm".to_owned(), "#ff0000".to_owned())],
        )]),
        texts: Vec::new(),
    };
    assert_eq!(colour_warnings(&colours), []);
}
