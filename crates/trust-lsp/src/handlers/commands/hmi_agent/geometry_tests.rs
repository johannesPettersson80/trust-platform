//! Closed-form and boundary tests of the drawn geometry (specification 39 §6; specification 43 §7).
//! These are new tests of the formulas in `geometry.rs`; the TypeScript suite has no direct ones.

use super::{box_of, contains, drawn_box, js_round, overlaps, whole_pixel, Rect};
use serde_json::json;
use trust_runtime::hmi::page_files::HmiObject;

/// An object of the given position, size and rotation.
fn object(at: [i64; 2], size: [u32; 2], rotation: u16) -> HmiObject {
    serde_json::from_value(json!({
        "id": "o",
        "type": "tank",
        "at": at,
        "size": size,
        "rotation": rotation,
    }))
    .expect("the object is valid")
}

/// A box of the given edges.
fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn an_unrotated_object_is_drawn_where_it_is_placed() {
    assert_eq!(
        drawn_box(&object([40, 24], [96, 48], 0)),
        rect(40.0, 24.0, 96.0, 48.0)
    );
}

#[test]
fn a_half_turn_keeps_the_box_and_a_quarter_turn_swaps_width_and_height_about_the_centre() {
    assert_eq!(
        drawn_box(&object([40, 24], [96, 48], 180)),
        rect(40.0, 24.0, 96.0, 48.0)
    );
    // The 96 by 48 box is centred on (88, 48). A quarter turn leaves it 48 wide and 96 tall
    // about that centre, so its top-left corner is (88 - 24, 48 - 48).
    assert_eq!(
        drawn_box(&object([40, 24], [96, 48], 90)),
        rect(64.0, 0.0, 48.0, 96.0)
    );
}

#[test]
fn box_of_matches_drawn_box_for_the_same_position_and_size() {
    let placed = object([40, 24], [96, 48], 90);
    assert_eq!(box_of([40.0, 24.0], [96.0, 48.0], 90), drawn_box(&placed));
}

#[test]
fn containment_includes_the_edges_and_excludes_anything_past_them() {
    let outer = rect(0.0, 0.0, 100.0, 100.0);
    assert!(contains(outer, outer), "a box holds itself");
    assert!(contains(outer, rect(0.0, 0.0, 100.0, 100.0)));
    assert!(!contains(outer, rect(1.0, 0.0, 100.0, 100.0)));
    assert!(!contains(outer, rect(0.0, 0.0, 100.0, 100.5)));
}

#[test]
fn touching_edges_do_not_overlap_and_a_gap_makes_them_overlap() {
    let left = rect(0.0, 0.0, 10.0, 10.0);
    let right = rect(10.0, 0.0, 10.0, 10.0);
    assert!(
        !overlaps(left, right, 0.0),
        "touching edges alone are not overlap"
    );
    assert!(
        overlaps(left, right, 1.0),
        "a gap of one pixel reaches the neighbour"
    );
    assert!(!overlaps(left, rect(0.0, 10.0, 10.0, 10.0), 0.0));
}

#[test]
fn js_round_rounds_halves_toward_positive_infinity_as_math_round_does() {
    assert_eq!(js_round(2.5), 3.0);
    assert_eq!(js_round(2.4), 2.0);
    assert_eq!(js_round(-2.5), -2.0);
    assert_eq!(js_round(-2.6), -3.0);
    assert_eq!(js_round(7.0), 7.0);
    // Math.round(0.49999999999999994) is 0; adding 0.5 first would round it up to 1.
    assert_eq!(js_round(0.49999999999999994), 0.0);
}

#[test]
fn whole_pixel_rounds_a_position_to_the_model_integer() {
    assert_eq!(whole_pixel(314.5), 315);
    assert_eq!(whole_pixel(314.4), 314);
}
