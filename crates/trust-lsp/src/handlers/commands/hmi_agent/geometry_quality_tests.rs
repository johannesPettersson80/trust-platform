//! Tests of the geometry review: `agent/geometryQuality.ts` (`geometryQuality`, `viewportProfiles`)
//! and the number formatting of its messages (specification 43, §4 and §7 items 2 and 4).
//!
//! The first three tests port the geometry assertions of `editors/vscode/src/test/suite/
//! hmi-agent-quality.test.ts` on the same fixtures: the rotated bounds and overlap rectangle, the
//! containment and attachment exceptions, and the viewport scales and touch targets. The VS Code
//! test drove them through the draft validation service; here the same review runs directly. The
//! rest are the boundaries of each limit, worked out by hand.

#![allow(clippy::float_cmp)]

use serde_json::{json, Value};
use trust_runtime::hmi::catalogue::Catalogue;
use trust_runtime::hmi::page_files::HmiPage;

use super::super::error::INVALID_ARGUMENT;
use super::*;

fn page(value: Value) -> HmiPage {
    serde_json::from_value(value).expect("the fixture is a valid page")
}

/// A design page of the given objects, at the 1280 × 800 design size of the fixtures.
fn design_page(objects: Value) -> HmiPage {
    page(json!({"title": "Quality", "object": objects}))
}

fn review(page: &HmiPage, profiles: &[ViewportProfile]) -> GeometryReview {
    geometry_quality(page, [1280, 800], Catalogue::shipped(), profiles)
}

fn profile(id: &str, width: u32, height: u32) -> ViewportProfile {
    ViewportProfile {
        id: id.to_owned(),
        width,
        height,
    }
}

fn rules(report: &GeometryReview) -> Vec<&'static str> {
    report.findings.iter().map(|finding| finding.rule).collect()
}

fn refusal_code(value: Value) -> Option<&'static str> {
    viewport_profiles(Some(&value))
        .err()
        .map(|error| error.code)
}

#[test]
fn reports_rotated_page_bounds_and_overlaps_with_object_ids_and_rectangles() {
    let page = design_page(json!([
        {"id": "turned", "type": "text", "at": [0, 20], "size": [40, 120], "rotation": 90},
        {"id": "a", "type": "text", "at": [200, 100], "size": [200, 100]},
        {"id": "b", "type": "text", "at": [300, 140], "size": [160, 80]}
    ]));
    let report = review(&page, &[]);
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.rule == "outside-page"
                && finding.objects.iter().any(|id| id == "turned")),
        "rotation must participate in bounds checking"
    );
    let overlap = report
        .findings
        .iter()
        .find(|finding| finding.rule == "object-overlap")
        .expect("a and b overlap");
    assert_eq!(overlap.objects, vec!["a".to_owned(), "b".to_owned()]);
    assert_eq!(
        overlap.rectangle,
        Some(Rect {
            x: 300.0,
            y: 140.0,
            width: 100.0,
            height: 60.0,
        })
    );
    assert!(
        report
            .findings
            .iter()
            .all(|finding| finding.severity == "warning"),
        "visual findings are warnings"
    );
}

#[test]
fn containment_attached_instruments_and_touching_edges_do_not_overlap() {
    let page = design_page(json!([
        {"id": "background", "type": "group", "at": [32, 80], "size": [600, 400]},
        {"id": "unit", "type": "text", "at": [100, 150], "size": [200, 200]},
        {"id": "instrument", "type": "text", "at": [280, 200], "size": [80, 40], "attach": "unit"},
        {"id": "edge", "type": "text", "at": [300, 320], "size": [100, 80]}
    ]));
    let report = review(&page, &[]);
    assert!(!rules(&report).contains(&"object-overlap"));
    assert!(
        !rules(&report).contains(&"outside-page"),
        "every object lies within the 1280 × 800 page"
    );
}

#[test]
fn target_viewports_use_the_adaptive_layout_and_report_undersized_controls() {
    let page = design_page(json!([
        {"id": "control", "type": "setpoint", "at": [100, 100], "size": [200, 64]}
    ]));
    let report = review(
        &page,
        &[
            profile("small-panel", 640, 400),
            profile("design", 1280, 800),
        ],
    );
    assert!(!rules(&report).contains(&"viewport-touch-target"));
    let scales: Vec<(&str, f64)> = report
        .viewports
        .iter()
        .map(|viewport| (viewport.id.as_str(), viewport.scale))
        .collect();
    assert_eq!(scales, vec![("small-panel", 1.0), ("design", 1.0)]);

    let small = design_page(json!([
        {"id": "tiny", "type": "setpoint", "at": [100, 100], "size": [24, 24]}
    ]));
    let report = review(&small, &[profile("design", 1280, 800)]);
    let warning = report
        .findings
        .iter()
        .find(|finding| finding.rule == "viewport-touch-target")
        .expect("a 24 px control is below the touch target");
    assert_eq!(
        warning.measured,
        Some(Measure::Size {
            width: 24.0,
            height: 24.0,
        })
    );
    assert_eq!(
        warning.required,
        Some(Measure::Target {
            width: 44,
            height: 44,
        })
    );
    assert_eq!(
        refusal_code(json!([{"id": "bad", "width": 0, "height": 400}])),
        Some(INVALID_ARGUMENT)
    );
}

// A 24 px button in a viewport twice the design size is drawn at 48 px: the fit scale enlarges it
// past the touch target, and the fitted page does not scroll.
#[test]
fn a_fitted_viewport_enlarges_small_controls_past_the_touch_target() {
    let page = design_page(json!([
        {"id": "button", "type": "button", "at": [100, 100], "size": [24, 24]}
    ]));
    let report = review(&page, &[profile("wall", 2560, 1600)]);
    let viewport = &report.viewports[0];
    assert_eq!(viewport.scale, 2.0);
    assert!(!viewport.scroll);
    assert_eq!(
        viewport.objects[0].rectangle,
        Rect {
            x: 200.0,
            y: 200.0,
            width: 48.0,
            height: 48.0,
        }
    );
    assert!(!rules(&report).contains(&"viewport-touch-target"));
}

// A 2560 × 1000 viewport fits the 1280 × 800 page at the smaller factor, 1.25, and centres it
// horizontally: the 24 px button is drawn 30 px square at (605, 125), and its message names both
// measured sides with one decimal and the viewport.
#[test]
fn a_letterboxed_viewport_centres_the_page_and_reports_the_shrunk_target() {
    let page = design_page(json!([
        {"id": "button", "type": "button", "at": [100, 100], "size": [24, 24]}
    ]));
    let report = review(&page, &[profile("wide", 2560, 1000)]);
    let viewport = &report.viewports[0];
    assert_eq!(viewport.scale, 1.25);
    assert_eq!(
        viewport.objects[0].rectangle,
        Rect {
            x: 605.0,
            y: 125.0,
            width: 30.0,
            height: 30.0,
        }
    );
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.rule == "viewport-touch-target")
        .expect("a 30 px button is below the touch target");
    assert_eq!(finding.viewport.as_deref(), Some("wide"));
    assert_eq!(
        finding.message,
        "button is 30.0 × 30.0 px in wide; use a larger control, readable scrolling or another page to retain a 44 × 44 px target."
    );
}

// The touch target is inclusive at 44 px: a control of exactly 44 px passes, and 43 px in width does
// not.
#[test]
fn the_touch_target_boundary_is_44_pixels() {
    let at_44 = design_page(json!([
        {"id": "button", "type": "button", "at": [0, 0], "size": [44, 44]}
    ]));
    assert!(
        !rules(&review(&at_44, &[profile("design", 1280, 800)])).contains(&"viewport-touch-target")
    );
    let at_43 = design_page(json!([
        {"id": "button", "type": "button", "at": [0, 0], "size": [43, 44]}
    ]));
    assert!(
        rules(&review(&at_43, &[profile("design", 1280, 800)])).contains(&"viewport-touch-target")
    );
}

#[test]
fn page_buttons_and_objects_with_a_click_are_touch_targets() {
    let page = design_page(json!([
        {"id": "link", "type": "text", "at": [10, 10], "size": [40, 40], "click": {"page": "next"}},
        {"id": "nav", "type": "page-button", "at": [10, 100], "size": [40, 40]},
        {"id": "label", "type": "text", "at": [10, 200], "size": [40, 40]}
    ]));
    let report = review(&page, &[profile("design", 1280, 800)]);
    let touched: Vec<&str> = report
        .findings
        .iter()
        .filter(|finding| finding.rule == "viewport-touch-target")
        .map(|finding| finding.object.as_deref().unwrap_or(""))
        .collect();
    assert_eq!(touched, vec!["link", "nav"]);
}

// A single pixel of shared width is an overlap: the boxes touch only when the shared width is zero.
#[test]
fn a_one_pixel_intersection_is_an_overlap_and_touching_edges_are_not() {
    let shared = design_page(json!([
        {"id": "a", "type": "text", "at": [0, 0], "size": [100, 100]},
        {"id": "b", "type": "text", "at": [99, 0], "size": [100, 100]}
    ]));
    let report = review(&shared, &[]);
    let overlap = report
        .findings
        .iter()
        .find(|finding| finding.rule == "object-overlap")
        .expect("one pixel is shared");
    assert_eq!(
        overlap.rectangle,
        Some(Rect {
            x: 99.0,
            y: 0.0,
            width: 1.0,
            height: 100.0,
        })
    );

    let touching = design_page(json!([
        {"id": "a", "type": "text", "at": [0, 0], "size": [100, 100]},
        {"id": "b", "type": "text", "at": [100, 0], "size": [100, 100]}
    ]));
    assert!(!rules(&review(&touching, &[])).contains(&"object-overlap"));
}

// 205 objects outside the page: the first 200 are kept in order, and the other five are counted
// and the review says it is truncated.
#[test]
fn findings_stop_at_two_hundred_and_report_the_rest() {
    let objects: Vec<Value> = (0..205)
        .map(|index| {
            json!({"id": format!("o{index}"), "type": "text", "at": [2000, index * 20], "size": [10, 10]})
        })
        .collect();
    let report = review(&design_page(Value::Array(objects)), &[]);
    assert_eq!(report.findings.len(), 200);
    assert_eq!(report.omitted, 5);
    assert!(report.truncated);
    assert_eq!(report.findings[0].objects, vec!["o0".to_owned()]);
    assert_eq!(report.findings[199].objects, vec!["o199".to_owned()]);
}

#[test]
fn an_absent_or_empty_viewport_list_has_no_profiles() {
    assert!(viewport_profiles(None).expect("absent").is_empty());
    assert!(viewport_profiles(Some(&json!([])))
        .expect("empty")
        .is_empty());
}

#[test]
fn three_viewports_with_unique_identifiers_are_accepted() {
    let profiles = viewport_profiles(Some(&json!([
        {"id": "a", "width": 1280, "height": 800},
        {"id": "tablet_portrait-1", "width": 820, "height": 1180},
        {"id": "wall", "width": 4096, "height": 1.0}
    ])))
    .expect("three valid viewports");
    assert_eq!(profiles.len(), 3);
    assert_eq!(profiles[1], profile("tablet_portrait-1", 820, 1180));
    assert_eq!(profiles[2], profile("wall", 4096, 1));
}

// The refusals are the same INVALID_ARGUMENT for every malformed request: a fourth viewport, a
// repeated identifier, an identifier that does not match the pattern, a dimension out of range, a
// fraction, a missing or an extra key, and a value that is not an array.
#[test]
fn malformed_viewports_are_refused_as_invalid_arguments() {
    let four = json!([
        {"id": "a", "width": 1, "height": 1},
        {"id": "b", "width": 1, "height": 1},
        {"id": "c", "width": 1, "height": 1},
        {"id": "d", "width": 1, "height": 1}
    ]);
    assert_eq!(refusal_code(four), Some(INVALID_ARGUMENT));
    assert_eq!(
        refusal_code(json!([
            {"id": "a", "width": 1, "height": 1},
            {"id": "a", "width": 2, "height": 2}
        ])),
        Some(INVALID_ARGUMENT)
    );
    assert_eq!(
        refusal_code(json!([{"id": "1a", "width": 1, "height": 1}])),
        Some(INVALID_ARGUMENT)
    );
    assert_eq!(
        refusal_code(json!([{"id": "a b", "width": 1, "height": 1}])),
        Some(INVALID_ARGUMENT)
    );
    assert_eq!(
        refusal_code(json!([{"id": "a", "width": 4097, "height": 1}])),
        Some(INVALID_ARGUMENT)
    );
    assert_eq!(
        refusal_code(json!([{"id": "a", "width": 1280.5, "height": 800}])),
        Some(INVALID_ARGUMENT)
    );
    assert_eq!(
        refusal_code(json!([{"id": "a", "width": 1, "height": 1, "extra": 1}])),
        Some(INVALID_ARGUMENT)
    );
    assert_eq!(
        refusal_code(json!([{"id": "a", "width": 1}])),
        Some(INVALID_ARGUMENT)
    );
    assert_eq!(
        refusal_code(json!({"id": "a", "width": 1, "height": 1})),
        Some(INVALID_ARGUMENT)
    );
    assert_eq!(refusal_code(Value::Null), Some(INVALID_ARGUMENT));
}

// The identifier pattern allows up to 64 characters, the first a letter; a 65-character one fails.
#[test]
fn the_identifier_length_boundary_is_64_characters() {
    let longest = format!("a{}", "b".repeat(63));
    let too_long = format!("a{}", "b".repeat(64));
    let accepted = viewport_profiles(Some(&json!([
        {"id": longest, "width": 1, "height": 1}
    ])));
    assert!(accepted.is_ok());
    assert_eq!(
        refusal_code(json!([{"id": too_long, "width": 1, "height": 1}])),
        Some(INVALID_ARGUMENT)
    );
}

// The dimensions are whole pixels from 1 to 4096, inclusive: 4096 passes, 4097 and 0 do not, and a
// whole number written with a zero fraction is whole, as it is in JavaScript.
#[test]
fn the_dimension_boundaries_are_inclusive_from_1_to_4096() {
    let at_limit = viewport_profiles(Some(&json!([
        {"id": "a", "width": 4096, "height": 1}
    ])))
    .expect("4096 by 1 is inside the limits");
    assert_eq!(at_limit[0], profile("a", 4096, 1));
    let whole_fraction = viewport_profiles(Some(&json!([
        {"id": "a", "width": 1280.0, "height": 800}
    ])))
    .expect("1280.0 is a whole number");
    assert_eq!(whole_fraction[0], profile("a", 1280, 800));
    assert_eq!(
        refusal_code(json!([{"id": "a", "width": 0, "height": 1}])),
        Some(INVALID_ARGUMENT)
    );
}

// `toFixed(1)`: the nearest tenth, an exact quarter (x.25) going to the larger tenth, and the sign
// of a negative value kept.
#[test]
fn to_fixed_one_rounds_ties_upwards_like_javascript() {
    assert_eq!(to_fixed_one(43.25), "43.3");
    assert_eq!(to_fixed_one(43.75), "43.8");
    assert_eq!(to_fixed_one(0.25), "0.3");
    assert_eq!(to_fixed_one(24.0), "24.0");
    assert_eq!(to_fixed_one(2.26), "2.3");
    assert_eq!(to_fixed_one(-43.25), "-43.3");
    assert_eq!(to_fixed_one(-0.04), "-0.0");
}
