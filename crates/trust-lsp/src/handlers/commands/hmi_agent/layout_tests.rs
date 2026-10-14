//! Boundary tests of the layout limits (specification 43 §6 and the 8-pixel grid of specification
//! 39). New tests of `layout.rs`; the TypeScript suite covers the same rules through composition.

use serde_json::json;

use super::{
    constrained_regions, gap_and_flow, parse_layout, place_region, snap, CompositionRole,
    EqualSize, Flow, Region, SizedItem,
};

/// The region of the page below the heading, as composition uses it.
const BOUNDS: Region = Region {
    x: 32,
    y: 88,
    width: 1216,
    height: 680,
};

#[test]
fn the_gap_defaults_to_24_pixels_and_the_flow_to_horizontal() {
    assert_eq!(
        gap_and_flow(None, None).expect("the defaults are valid"),
        (24, Flow::Horizontal)
    );
}

#[test]
fn the_gap_accepts_the_grid_from_8_to_128_pixels_and_refuses_the_rest() {
    assert_eq!(
        gap_and_flow(Some(&json!(8)), None).expect("8 is valid").0,
        8
    );
    assert_eq!(
        gap_and_flow(Some(&json!(128)), None)
            .expect("128 is valid")
            .0,
        128
    );
    for refused in [0, 4, 12, 136] {
        let error = gap_and_flow(Some(&json!(refused)), None).expect_err("outside the limits");
        assert_eq!(
            error.message, "layout gap must be 8–128 pixels on the 8-pixel grid.",
            "gap {refused}"
        );
    }
}

#[test]
fn the_flow_is_horizontal_or_vertical_only() {
    assert_eq!(
        gap_and_flow(None, Some(&json!("vertical")))
            .expect("vertical is valid")
            .1,
        Flow::Vertical
    );
    let error = gap_and_flow(None, Some(&json!("diagonal"))).expect_err("not a flow");
    assert_eq!(error.message, "layout flow must be horizontal or vertical.");
}

#[test]
fn the_padding_is_on_the_grid_between_0_and_128_pixels() {
    for accepted in [0, 8, 128] {
        let layout = parse_layout(Some(&json!({"padding": accepted}))).expect("valid padding");
        assert_eq!(layout.padding, Some(accepted));
    }
    for refused in [4, 136, -8] {
        let error = parse_layout(Some(&json!({"padding": refused}))).expect_err("refused");
        assert_eq!(
            error.message, "padding must use the 8-pixel grid between 0 and 128.",
            "padding {refused}"
        );
    }
}

#[test]
fn unknown_layout_keys_and_unknown_equal_sizes_are_refused() {
    let error = parse_layout(Some(&json!({"columns": 3}))).expect_err("unknown key");
    assert_eq!(error.message, "unsupported layout constraints.");
    let error = parse_layout(Some(&json!({"equalSize": "diagonal"}))).expect_err("unknown rule");
    assert_eq!(error.message, "equalSize must be width, height or both.");
    assert_eq!(
        parse_layout(Some(&json!({"equalSize": "both"})))
            .expect("both is valid")
            .equal_size,
        Some(EqualSize::Both)
    );
}

#[test]
fn a_region_is_exactly_four_grid_values_with_a_positive_size() {
    let refused = [
        json!({"x": 31, "y": 88, "width": 704, "height": 672}),
        json!({"x": 32, "y": 88, "width": 0, "height": 672}),
        json!({"x": 32, "y": 88, "width": 704}),
        json!({"x": 32, "y": 88, "width": 704, "height": 672, "z": 1}),
    ];
    for region in refused {
        let error = constrained_regions(Vec::new(), BOUNDS, Some(&json!({"process": region})))
            .expect_err("malformed region");
        assert_eq!(
            error.message,
            "process region must use positive dimensions on the 8-pixel grid."
        );
    }
}

#[test]
fn a_region_outside_the_bounds_or_over_another_region_is_a_validation_failure() {
    let outside = json!({"process": {"x": 1200, "y": 88, "width": 704, "height": 672}});
    let error = constrained_regions(Vec::new(), BOUNDS, Some(&outside)).expect_err("outside");
    assert_eq!(
        error.message,
        "process region does not fit below the heading and inside page margins."
    );
    let defaults = vec![
        (
            CompositionRole::Process,
            Region {
                x: 32,
                y: 88,
                width: 400,
                height: 400,
            },
        ),
        (
            CompositionRole::Measurements,
            Region {
                x: 200,
                y: 88,
                width: 400,
                height: 400,
            },
        ),
    ];
    let error = constrained_regions(defaults, BOUNDS, None).expect_err("overlapping");
    assert_eq!(
        error.message,
        "process and measurements regions overlap; move or resize the regions."
    );
}

#[test]
fn snap_rounds_down_to_the_grid_and_keeps_grid_values() {
    assert_eq!(snap(7.9), 0);
    assert_eq!(snap(8.0), 8);
    assert_eq!(snap(15.9), 8);
    assert_eq!(
        snap(-0.5),
        -8,
        "a negative value rounds down as Math.floor does"
    );
}

#[test]
fn an_item_that_exactly_fills_its_region_is_placed_at_the_corner() {
    let area = Region {
        x: 32,
        y: 88,
        width: 160,
        height: 120,
    };
    let items = [SizedItem {
        id: "only".to_owned(),
        size: [160, 120],
        rotation: 0,
        pinned_at: None,
    }];
    let placed = place_region(
        &items,
        area,
        CompositionRole::Process,
        24,
        Flow::Horizontal,
        &[],
    )
    .expect("it fits");
    assert_eq!(placed["only"], [32, 88]);
}

#[test]
fn an_item_one_pixel_too_wide_fails_with_its_name_and_region() {
    let area = Region {
        x: 32,
        y: 88,
        width: 160,
        height: 120,
    };
    let items = [SizedItem {
        id: "only".to_owned(),
        size: [168, 120],
        rotation: 0,
        pinned_at: None,
    }];
    let error = place_region(
        &items,
        area,
        CompositionRole::Process,
        24,
        Flow::Horizontal,
        &[],
    )
    .expect_err("too wide");
    assert!(
        error
            .message
            .starts_with("only does not fit in process at readable sizes"),
        "{error}"
    );
}

#[test]
fn a_pinned_position_must_be_on_the_grid_and_inside_its_region() {
    let area = Region {
        x: 32,
        y: 88,
        width: 160,
        height: 120,
    };
    let off_grid = [SizedItem {
        id: "fixed".to_owned(),
        size: [160, 120],
        rotation: 0,
        pinned_at: Some(json!([33, 88])),
    }];
    let error = place_region(
        &off_grid,
        area,
        CompositionRole::Process,
        24,
        Flow::Horizontal,
        &[],
    )
    .expect_err("off grid");
    assert_eq!(
        error.message,
        "fixed pinned position must use the 8-pixel grid."
    );
    let outside = [SizedItem {
        id: "fixed".to_owned(),
        size: [160, 120],
        rotation: 0,
        pinned_at: Some(json!([200, 88])),
    }];
    let error = place_region(
        &outside,
        area,
        CompositionRole::Process,
        24,
        Flow::Horizontal,
        &[],
    )
    .expect_err("outside");
    assert_eq!(error.message, "fixed does not fit in the process region.");
}
