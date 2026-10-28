//! Tests of align and distribute (specification 39, §3.3 item 8). Ported from the align assertion of
//! `editors/vscode/src/test/suite/hmi-agent-edits.test.ts` ("align and group reuse editor geometry without
//! changing pipes"); the other tests are closed-form checks of `editor/align.ts`, which has no test file of
//! its own.

use std::collections::BTreeMap;

use trust_runtime::hmi::page_files::{HmiObject, HmiPage, HmiPipe, HmiPipeStyle};

use super::{align_objects, distribute_objects, AlignEdge, DistributeAxis};

fn object(id: &str, at: [i64; 2], size: [u32; 2]) -> HmiObject {
    serde_json::from_value(serde_json::json!({ "id": id, "type": "tank", "at": at, "size": size }))
        .expect("a page object with its required fields")
}

fn page(objects: Vec<HmiObject>, pipes: Vec<HmiPipe>) -> HmiPage {
    HmiPage {
        regions: Vec::new(),
        title: "Process".to_string(),
        order: None,
        icon: None,
        group: None,
        hidden: false,
        incomplete: false,
        pid: None,
        objects,
        pipes,
    }
}

fn pipe(from: &str, to: &str) -> HmiPipe {
    HmiPipe {
        from: from.to_string(),
        to: to.to_string(),
        style: HmiPipeStyle::Process,
        waypoints: Vec::new(),
        bind: BTreeMap::new(),
    }
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| (*name).to_string()).collect()
}

/// The fixture `base()` of `hmi-agent-edits.test.ts`: `a` at (40, 40) and `b` at (240, 80), both 100 by
/// 100, joined by one pipe.
fn base() -> HmiPage {
    page(
        vec![
            object("a", [40, 40], [100, 100]),
            object("b", [240, 80], [100, 100]),
        ],
        vec![pipe("a.outlet", "b.inlet")],
    )
}

// Parsing takes the strings of `AlignEdge` and `DistributeAxis` in align.ts (lines 8-9) and no others.
#[test]
fn the_edges_and_axes_are_read_from_the_builder_strings() {
    assert_eq!(AlignEdge::parse("left"), Some(AlignEdge::Left));
    assert_eq!(AlignEdge::parse("centre"), Some(AlignEdge::Centre));
    assert_eq!(AlignEdge::parse("right"), Some(AlignEdge::Right));
    assert_eq!(AlignEdge::parse("top"), Some(AlignEdge::Top));
    assert_eq!(AlignEdge::parse("middle"), Some(AlignEdge::Middle));
    assert_eq!(AlignEdge::parse("bottom"), Some(AlignEdge::Bottom));
    assert_eq!(
        AlignEdge::parse("center"),
        None,
        "the builder spells it centre"
    );
    assert_eq!(AlignEdge::parse("Left"), None);
    assert_eq!(
        DistributeAxis::parse("horizontal"),
        Some(DistributeAxis::Horizontal)
    );
    assert_eq!(
        DistributeAxis::parse("vertical"),
        Some(DistributeAxis::Vertical)
    );
    assert_eq!(DistributeAxis::parse("diagonal"), None);
}

// hmi-agent-edits.test.ts: "align and group reuse editor geometry without changing pipes", align part
// (specification 39, §3.3 item 8)
#[test]
fn aligning_tops_moves_the_lower_object_up_and_keeps_the_pipes() {
    let aligned = align_objects(&base(), &names(&["a", "b"]), AlignEdge::Top);
    assert_eq!(aligned.objects[1].at[1], 40);
    assert_eq!(aligned.objects[0].at, [40, 40]);
    assert_eq!(aligned.pipes, base().pipes);
}

// Closed form of `alignObjects` (align.ts line 26): the left edges meet the selection's left edge, 40.
#[test]
fn aligning_left_edges_moves_each_object_to_the_leftmost_edge() {
    let aligned = align_objects(&base(), &names(&["a", "b"]), AlignEdge::Left);
    assert_eq!(aligned.objects[0].at, [40, 40]);
    assert_eq!(aligned.objects[1].at, [40, 80]);
}

// Closed form: the right edges meet the selection's right edge, 340.
#[test]
fn aligning_right_edges_moves_each_object_to_the_rightmost_edge() {
    let aligned = align_objects(&base(), &names(&["a", "b"]), AlignEdge::Right);
    assert_eq!(aligned.objects[0].at, [240, 40]);
    assert_eq!(aligned.objects[1].at, [240, 80]);
}

// Closed form: the centres meet at (40 + 340) / 2 = 190, so each left edge is 190 less half its width.
#[test]
fn aligning_centres_puts_both_centres_on_the_middle_of_the_selection() {
    let aligned = align_objects(&base(), &names(&["a", "b"]), AlignEdge::Centre);
    assert_eq!(aligned.objects[0].at, [140, 40]);
    assert_eq!(aligned.objects[1].at, [140, 80]);
}

// Closed form: the bottom edges meet the selection's bottom edge, 180.
#[test]
fn aligning_bottoms_moves_each_object_to_the_lowest_edge() {
    let aligned = align_objects(&base(), &names(&["a", "b"]), AlignEdge::Bottom);
    assert_eq!(aligned.objects[0].at, [40, 80]);
    assert_eq!(aligned.objects[1].at, [240, 80]);
}

// Closed form: the middles meet at (40 + 180) / 2 = 110.
#[test]
fn aligning_middles_puts_both_middles_on_the_middle_of_the_selection() {
    let aligned = align_objects(&base(), &names(&["a", "b"]), AlignEdge::Middle);
    assert_eq!(aligned.objects[0].at, [40, 60]);
    assert_eq!(aligned.objects[1].at, [240, 60]);
}

// Rounding (`js_round`, the `Math.round` of align.ts line 19): `a` is 101 wide, so its centre move is 99.5
// and 40 + 99.5 rounds half up to 140, the same x as `b`.
#[test]
fn a_half_pixel_position_rounds_up_as_math_round_does() {
    let mut wide = base();
    wide.objects[0].size = [101, 100];
    let aligned = align_objects(&wide, &names(&["a", "b"]), AlignEdge::Centre);
    assert_eq!(aligned.objects[0].at, [140, 40]);
    assert_eq!(aligned.objects[1].at, [140, 80]);
}

// Rounding for a negative half: the centre is 3, so `a` moves by -2.5, and `Math.round(-2.5)` is -2.
#[test]
fn a_negative_half_pixel_rounds_toward_positive_infinity() {
    let source = page(
        vec![
            object("a", [0, 0], [11, 10]),
            object("b", [-5, 0], [10, 10]),
        ],
        Vec::new(),
    );
    let aligned = align_objects(&source, &names(&["a", "b"]), AlignEdge::Centre);
    assert_eq!(aligned.objects[0].at, [-2, 0]);
    assert_eq!(aligned.objects[1].at, [-2, 0]);
}

// `drawnBox` (editing.ts line 148): a quarter turn swaps the width and height about the centre, so the
// turned 100 by 20 object is drawn at (40, -40) and its top edge is at -40.
#[test]
fn alignment_uses_the_drawn_box_of_a_quarter_turn() {
    let mut turned = object("q", [0, 0], [100, 20]);
    turned.rotation = 90;
    let source = page(vec![object("p", [0, 0], [10, 10]), turned], Vec::new());
    let aligned = align_objects(&source, &names(&["p", "q"]), AlignEdge::Top);
    assert_eq!(aligned.objects[0].at, [0, -40]);
    assert_eq!(
        aligned.objects[1].at,
        [0, 0],
        "the turned object already sits on the top edge"
    );
}

// Boundary: fewer than two objects on the page leave it as it was, and a repeated name counts once.
#[test]
fn fewer_than_two_selected_objects_leave_the_page_unchanged() {
    assert_eq!(
        align_objects(&base(), &names(&["a"]), AlignEdge::Left),
        base()
    );
    assert_eq!(
        align_objects(&base(), &names(&["a", "missing"]), AlignEdge::Left),
        base()
    );
    assert_eq!(
        align_objects(&base(), &names(&["a", "a"]), AlignEdge::Left),
        base()
    );
}

// Closed form of `distributeObjects` (align.ts line 59): the page holds c, a, b, but the objects are
// spread in the order of their starts, a at 0, b at 100 and c at 300 with 50 wide, so the gap is
// (350 - 150) / 2 = 100. b moves to 150; the first (a) and last (c) stay.
#[test]
fn distributing_horizontally_spreads_by_start_with_equal_gaps() {
    let source = page(
        vec![
            object("c", [300, 0], [50, 50]),
            object("a", [0, 0], [50, 50]),
            object("b", [100, 0], [50, 50]),
        ],
        Vec::new(),
    );
    let spread = distribute_objects(
        &source,
        &names(&["a", "b", "c"]),
        DistributeAxis::Horizontal,
    );
    assert_eq!(spread.objects[0].at, [300, 0], "c is last, so it stays");
    assert_eq!(spread.objects[1].at, [0, 0], "a is first, so it stays");
    assert_eq!(spread.objects[2].at, [150, 0], "b moves to 0 + 50 + 100");
}

// Closed form, vertical: the starts 0, 40 and 200 with 20 high give the gap (220 - 60) / 2 = 80, so the
// middle object moves from 40 to 100 and the bottom one stays at 200.
#[test]
fn distributing_vertically_spreads_by_top_edge() {
    let source = page(
        vec![
            object("x1", [0, 200], [50, 20]),
            object("x2", [0, 0], [50, 20]),
            object("x3", [0, 40], [50, 20]),
        ],
        Vec::new(),
    );
    let spread = distribute_objects(
        &source,
        &names(&["x1", "x2", "x3"]),
        DistributeAxis::Vertical,
    );
    assert_eq!(spread.objects[0].at, [0, 200]);
    assert_eq!(spread.objects[1].at, [0, 0]);
    assert_eq!(spread.objects[2].at, [0, 100]);
}

// Boundary: objects with the same start keep the page's order (the sort of align.ts line 64 is stable).
// `p` and `q` both start at 0, so `p` comes first and `q` moves to 0 + 10 + 90 = 100.
#[test]
fn objects_with_equal_starts_keep_the_page_order() {
    let source = page(
        vec![
            object("p", [0, 0], [10, 10]),
            object("q", [0, 0], [10, 10]),
            object("r", [200, 0], [10, 10]),
        ],
        Vec::new(),
    );
    let spread = distribute_objects(
        &source,
        &names(&["p", "q", "r"]),
        DistributeAxis::Horizontal,
    );
    assert_eq!(spread.objects[0].at, [0, 0]);
    assert_eq!(spread.objects[1].at, [100, 0]);
    assert_eq!(spread.objects[2].at, [200, 0]);
}

// Rounding: the gap is (31 + 10 - 0 - 30) / 2 = 5.5, so `b` moves to 15.5, which rounds half up to 16.
#[test]
fn a_fractional_gap_places_each_object_at_a_whole_pixel() {
    let source = page(
        vec![
            object("a", [0, 0], [10, 10]),
            object("b", [10, 0], [10, 10]),
            object("c", [31, 0], [10, 10]),
        ],
        Vec::new(),
    );
    let spread = distribute_objects(
        &source,
        &names(&["a", "b", "c"]),
        DistributeAxis::Horizontal,
    );
    assert_eq!(spread.objects[0].at, [0, 0]);
    assert_eq!(spread.objects[1].at, [16, 0]);
    assert_eq!(spread.objects[2].at, [31, 0]);
}

// Boundary: fewer than three objects on the page leave it unchanged, and names on no object are ignored.
#[test]
fn fewer_than_three_selected_objects_leave_the_page_unchanged() {
    assert_eq!(
        distribute_objects(&base(), &names(&["a", "b"]), DistributeAxis::Horizontal),
        base()
    );
    assert_eq!(
        distribute_objects(
            &base(),
            &names(&["a", "b", "missing"]),
            DistributeAxis::Vertical
        ),
        base()
    );
}
