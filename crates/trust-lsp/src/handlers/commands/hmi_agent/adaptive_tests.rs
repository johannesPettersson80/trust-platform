//! Tests of the adaptive layout: `adaptive.ts` (`adaptPage`, `blocksOf`, `readingOrder`, `layOut`,
//! `scaleFor`, `arranged`), specification 39, §4 item 5. The expected positions are worked out by
//! hand from the `TypeScript` steps for each fixture.

#![allow(clippy::float_cmp)]

use serde_json::json;
use trust_runtime::hmi::page_files::HmiPage;

use super::*;

const DESIGN: [u32; 2] = [1280, 800];

fn page(value: serde_json::Value) -> HmiPage {
    serde_json::from_value(value).expect("the fixture is a valid page")
}

// The design fits the area exactly: the page is shown as designed, scaled to fit (factor 1).
#[test]
fn an_area_of_the_design_size_shows_the_page_as_designed() {
    let page = page(json!({"title": "Design", "object": [
        {"id": "a", "type": "text", "at": [10, 20], "size": [100, 50]}
    ]}));
    let adapted = adapt_page(&page, DESIGN, [1280, 800]);
    assert!(adapted.fit);
    assert_eq!(adapted.size, [1280.0, 800.0]);
    assert_eq!(adapted.page.objects[0].at, [10, 20]);
    assert_eq!(
        adapted.boxes,
        vec![Rect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 50.0
        }]
    );
}

// Wider than the design but shorter: the page keeps its design size at scale 1 and scrolls down.
#[test]
fn a_wider_but_shorter_area_keeps_the_design_size_and_scrolls() {
    let page = page(json!({"title": "Wide", "object": []}));
    let adapted = adapt_page(&page, DESIGN, [1280, 600]);
    assert!(!adapted.fit);
    assert_eq!(adapted.size, [1280.0, 800.0]);
}

// A control 200 px wide at (100, 100) in a 640 px area: the layout starts again at the area's width,
// with the one block at its margin, and the content is 640 px wide and 400 px tall (no scrolling
// beyond the area).
#[test]
fn a_narrow_area_lays_the_page_out_again_at_its_width() {
    let page = page(json!({"title": "Narrow", "object": [
        {"id": "control", "type": "setpoint", "at": [100, 100], "size": [200, 64]}
    ]}));
    let adapted = adapt_page(&page, DESIGN, [640, 400]);
    assert!(!adapted.fit);
    assert_eq!(adapted.size, [640.0, 400.0]);
    assert_eq!(adapted.page.objects[0].at, [100, 100]);
    assert_eq!(adapted.page.objects[0].size, [200, 64]);
}

// A 400 px block in a 300 px area scales by 0.75, since no control in it needs its touch size.
#[test]
fn a_block_wider_than_the_area_scales_uniformly_to_its_width() {
    let page = page(json!({"title": "Scaled", "object": [
        {"id": "wide", "type": "text", "at": [0, 0], "size": [400, 100]}
    ]}));
    let adapted = adapt_page(&page, DESIGN, [300, 400]);
    assert_eq!(adapted.page.objects[0].at, [0, 0]);
    assert_eq!(adapted.page.objects[0].size, [300, 75]);
    assert_eq!(adapted.size, [300.0, 400.0]);
}

// Two 200 px blocks side by side in a 300 px area: the second does not fit beside the first, so it
// starts a new row at the left margin, 16 px below the first block.
#[test]
fn a_block_that_overlaps_a_placed_one_starts_a_row_below_it() {
    let page = page(json!({"title": "Rows", "object": [
        {"id": "a", "type": "text", "at": [0, 0], "size": [200, 50]},
        {"id": "b", "type": "text", "at": [200, 0], "size": [200, 50]}
    ]}));
    let adapted = adapt_page(&page, DESIGN, [300, 400]);
    assert_eq!(adapted.page.objects[0].at, [0, 0]);
    assert_eq!(adapted.page.objects[1].at, [0, 66]);
    assert_eq!(adapted.size, [300.0, 400.0]);
}

// A panel that arranges its objects in a column, laid out in 200 px: the title-less padding of 16 px
// holds the objects at (16, 16) and (16, 72), and the panel grows to 128 px.
#[test]
fn an_arranged_panel_rearranges_its_objects_in_a_column() {
    let page = page(json!({"title": "Column", "object": [
        {"id": "p", "type": "panel", "at": [0, 0], "size": [300, 200], "arrange": "column"},
        {"id": "a", "type": "text", "at": [16, 16], "size": [100, 40]},
        {"id": "b", "type": "text", "at": [16, 80], "size": [100, 40]}
    ]}));
    let adapted = adapt_page(&page, DESIGN, [200, 400]);
    assert_eq!(adapted.page.objects[0].at, [0, 0]);
    assert_eq!(adapted.page.objects[0].size, [200, 128]);
    assert_eq!(adapted.page.objects[1].at, [16, 16]);
    assert_eq!(adapted.page.objects[1].size, [100, 40]);
    assert_eq!(adapted.page.objects[2].at, [16, 72]);
    assert_eq!(adapted.size, [200.0, 400.0]);
}

// A button keeps its touch size of 44 px at least, so a 200 px block that must shrink to a 100 px
// area keeps its size: the content grows to 200 px and scrolls sideways.
#[test]
fn a_control_keeps_its_size_and_widens_the_content() {
    let page = page(json!({"title": "Touch", "object": [
        {"id": "button", "type": "button", "at": [0, 0], "size": [200, 40]}
    ]}));
    let adapted = adapt_page(&page, DESIGN, [100, 400]);
    assert_eq!(adapted.page.objects[0].size, [200, 40]);
    assert_eq!(adapted.size, [200.0, 400.0]);
}

// A container narrower than its margins has a negative width, which the layout keeps exactly in the
// drawn box (the page's own size is zero, as an unsigned size cannot hold it).
#[test]
fn a_container_narrower_than_its_margins_keeps_its_negative_width_in_its_box() {
    let page = page(json!({"title": "Margins", "object": [
        {"id": "p", "type": "panel", "at": [300, 0], "size": [300, 200], "arrange": "row"},
        {"id": "a", "type": "text", "at": [316, 16], "size": [100, 40]}
    ]}));
    let adapted = adapt_page(&page, DESIGN, [200, 400]);
    assert_eq!(adapted.page.objects[0].size, [0, 32]);
    assert_eq!(
        adapted.boxes[0],
        Rect {
            x: 300.0,
            y: 0.0,
            width: -400.0,
            height: 32.0,
        }
    );
    assert_eq!(
        adapted.boxes[1],
        Rect {
            x: 316.0,
            y: 16.0,
            width: 0.0,
            height: 0.0,
        }
    );
    assert_eq!(adapted.size, [200.0, 400.0]);
}

// Shared regions keep the authored layout, at the design size, whatever the area.
#[test]
fn shared_regions_keep_the_authored_layout() {
    let page = page(json!({
        "title": "Regions",
        "regions": [{"at": [0, 0], "size": [640, 400], "objects": ["a"], "pipes": []}],
        "object": [{"id": "a", "type": "text", "at": [100, 100], "size": [200, 64]}]
    }));
    let adapted = adapt_page(&page, DESIGN, [640, 400]);
    assert!(!adapted.fit);
    assert_eq!(adapted.size, [1280.0, 800.0]);
    assert_eq!(adapted.page.objects[0].at, [100, 100]);
}

#[test]
fn a_page_without_objects_keeps_the_height_of_the_area() {
    let page = page(json!({"title": "Empty"}));
    let adapted = adapt_page(&page, DESIGN, [640, 400]);
    assert!(!adapted.fit);
    assert_eq!(adapted.size, [640.0, 400.0]);
    assert!(adapted.boxes.is_empty());
}
