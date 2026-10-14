//! Tests of the duplicate (specification 43, AT-05 and repair 4; specification 39, §3.3 item 8). Ported from
//! `editors/vscode/src/test/suite/hmi-agent-edits.test.ts` ("duplicate carries internal pipes", "duplicating
//! a connected group rebinds its pipe-attached instruments") and `hmi-agent-pipes.test.ts` ("duplication uses
//! the builder's copy and paste with the chosen ids"). The refusal of a duplicate id belongs to the agent's
//! edit operation and is not ported here.

use std::collections::BTreeMap;

use trust_runtime::hmi::page_files::{
    HmiBinding, HmiBindingDirection, HmiObject, HmiPage, HmiPipe, HmiPipeStyle,
};

use super::duplicate_objects;

fn object(id: &str, kind: &str, at: [i64; 2]) -> HmiObject {
    serde_json::from_value(
        serde_json::json!({ "id": id, "type": kind, "at": at, "size": [100, 100] }),
    )
    .expect("a page object with its required fields")
}

fn attached(mut object: HmiObject, attach: &str) -> HmiObject {
    object.attach = Some(attach.to_string());
    object
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

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| (*name).to_string()).collect()
}

fn find<'a>(page: &'a HmiPage, id: &str) -> &'a HmiObject {
    page.objects
        .iter()
        .find(|object| object.id == id)
        .expect("the page holds an object with this id")
}

/// The fixture `base()` of `hmi-agent-edits.test.ts`: `a` at (40, 40) and `b` at (240, 80), both tanks, joined
/// by one pipe.
fn base() -> HmiPage {
    page(
        vec![
            object("a", "tank", [40, 40]),
            object("b", "tank", [240, 80]),
        ],
        vec![pipe("a.outlet", "b.inlet")],
    )
}

// hmi-agent-edits.test.ts: "duplicate carries internal pipes and rejects duplicate ids" (specification 43,
// AT-05)
#[test]
fn duplicating_two_objects_carries_the_pipe_between_them() {
    let copied = duplicate_objects(&base(), &names(&["a", "b"]), &names(&["c", "d"]), [0, 200])
        .expect("both objects are on the page");
    assert_eq!(copied.objects.len(), 4);
    assert_eq!(
        copied.pipes,
        vec![pipe("a.outlet", "b.inlet"), pipe("c.outlet", "d.inlet")],
        "the pipe between the copies comes after the page's own pipes"
    );
    assert_eq!(find(&copied, "c").at, [40, 240], "a moves by the delta");
    assert_eq!(find(&copied, "d").at, [240, 280], "b moves by the delta");
}

// hmi-agent-edits.test.ts: "duplicating a connected group rebinds its pipe-attached instruments to the
// copied pipe" (specification 43, repair 4)
#[test]
fn an_instrument_on_a_copied_pipe_measures_the_copy_of_that_pipe() {
    let mut source = base();
    source
        .objects
        .push(attached(object("sensor", "tank", [180, 240]), "pipe:0"));
    let copied = duplicate_objects(
        &source,
        &names(&["a", "b", "sensor"]),
        &names(&["c", "d", "sensor-copy"]),
        [0, 320],
    )
    .expect("the group is on the page");
    assert_eq!(
        find(&copied, "sensor-copy").attach.as_deref(),
        Some("pipe:1")
    );
    assert_eq!(
        find(&copied, "sensor").attach.as_deref(),
        Some("pipe:0"),
        "the original keeps its pipe"
    );
}

// hmi-agent-pipes.test.ts: "duplication uses the builder's copy and paste with the chosen ids", the group
// (specification 43, repair 4)
#[test]
fn a_copied_group_takes_the_chosen_ids_and_its_own_pipe() {
    let mut source = page(
        vec![
            object("p-101", "pump", [40, 40]),
            object("v-101", "valve", [480, 40]),
        ],
        vec![pipe("p-101.outlet", "v-101.inlet")],
    );
    source
        .objects
        .push(attached(object("ft-1", "flow-sensor", [240, 0]), "pipe:0"));
    let group = duplicate_objects(
        &source,
        &names(&["p-101", "v-101", "ft-1"]),
        &names(&["p-201", "v-201", "ft-2"]),
        [0, 400],
    )
    .expect("the group is on the page");
    assert_eq!(
        group.pipes[1],
        pipe("p-201.outlet", "v-201.inlet"),
        "the pipe between the copies comes along"
    );
    assert_eq!(
        find(&group, "ft-2").attach.as_deref(),
        Some("pipe:1"),
        "the copied instrument measures the copied pipe"
    );
    assert_eq!(find(&group, "p-201").at, [40, 440]);
}

// hmi-agent-pipes.test.ts: the same test, the instrument copied alone (specification 43, repair 4)
#[test]
fn an_instrument_copied_without_what_it_measures_stands_free() {
    let mut source = page(
        vec![
            object("p-101", "pump", [40, 40]),
            object("v-101", "valve", [480, 40]),
        ],
        vec![pipe("p-101.outlet", "v-101.inlet")],
    );
    source
        .objects
        .push(attached(object("ft-1", "flow-sensor", [240, 0]), "pipe:0"));
    let alone = duplicate_objects(&source, &names(&["ft-1"]), &names(&["ft-2"]), [0, 120])
        .expect("ft-1 is on the page");
    let copy = find(&alone, "ft-2");
    assert_eq!(
        copy.attach, None,
        "an instrument copied without its pipe stands free, as in the builder"
    );
    assert_eq!(copy.at, [240, 120]);
    assert_eq!(alone.pipes.len(), 1);
}

// Closed form of `copyText` and `pasted` (clipboard.ts lines 40-46 and 137): an instrument on the second
// pipe names it by its place among the copied pipes, so its copy names the copy of that pipe. The pipe
// copies come after the page's three pipes, so the copy is pipe 3.
#[test]
fn an_instrument_names_a_copied_pipe_by_its_place_among_the_copied_pipes() {
    let source = page(
        vec![
            object("a", "tank", [40, 40]),
            object("b", "tank", [240, 80]),
            object("c", "tank", [40, 400]),
            object("d", "tank", [240, 400]),
            attached(object("sensor", "flow-sensor", [140, 200]), "pipe:1"),
        ],
        vec![
            pipe("c.outlet", "d.inlet"),
            pipe("a.outlet", "b.inlet"),
            pipe("d.outlet", "c.inlet"),
        ],
    );
    let copied = duplicate_objects(
        &source,
        &names(&["a", "b", "sensor"]),
        &names(&["a2", "b2", "s2"]),
        [0, 300],
    )
    .expect("the group is on the page");
    assert_eq!(find(&copied, "s2").attach.as_deref(), Some("pipe:3"));
    assert_eq!(copied.pipes[3], pipe("a2.outlet", "b2.inlet"));
}

// Boundary of `copyText` (clipboard.ts line 40): an instrument attached to equipment that was copied keeps
// the copy of that equipment, and one attached to equipment that was not copied stands free.
#[test]
fn an_instrument_keeps_what_it_measures_only_when_that_was_copied_too() {
    let source = page(
        vec![
            object("a", "tank", [40, 40]),
            attached(object("ft", "flow-sensor", [240, 0]), "a"),
        ],
        Vec::new(),
    );
    let with_it = duplicate_objects(
        &source,
        &names(&["a", "ft"]),
        &names(&["a2", "ft2"]),
        [0, 200],
    )
    .expect("on the page");
    assert_eq!(find(&with_it, "ft2").attach.as_deref(), Some("a2"));
    let without_it = duplicate_objects(&source, &names(&["ft"]), &names(&["ft2"]), [0, 200])
        .expect("on the page");
    assert_eq!(find(&without_it, "ft2").attach, None);
}

// Specification 43, repair 4: the agent's duplicate passes no program variables to the paste, so a binding
// keeps its path even when the next number of its variable exists (clipboard.ts `renumbered`).
#[test]
fn bindings_of_a_copy_are_unchanged() {
    let mut source = base();
    source.objects[0].bind.insert(
        "running".to_string(),
        HmiBinding {
            quality: None,
            path: "Main.P101.Running".to_string(),
            unit: None,
            range: None,
            direction: HmiBindingDirection::Read,
        },
    );
    let copied = duplicate_objects(&source, &names(&["a"]), &names(&["c"]), [0, 200])
        .expect("a is on the page");
    assert_eq!(find(&copied, "c").bind["running"].path, "Main.P101.Running");
    assert_eq!(find(&copied, "a").bind["running"].path, "Main.P101.Running");
}

// Closed form: the bends of a copied pipe move by the same delta as its objects (clipboard.ts line 117).
#[test]
fn the_bends_of_a_copied_pipe_move_with_the_copies() {
    let mut source = base();
    source.pipes[0].waypoints = vec![[10, 20]];
    let copied = duplicate_objects(&source, &names(&["a", "b"]), &names(&["c", "d"]), [0, 200])
        .expect("on the page");
    assert_eq!(copied.pipes[1].waypoints, vec![[10, 220]]);
    assert_eq!(
        copied.pipes[0].waypoints,
        vec![[10, 20]],
        "the original bend stays"
    );
}

// Boundary: a pipe to an object that is not copied is dropped from the copy (clipboard.ts line 35).
#[test]
fn a_pipe_to_an_object_not_copied_is_not_copied() {
    let copied = duplicate_objects(&base(), &names(&["a"]), &names(&["c"]), [0, 200])
        .expect("a is on the page");
    assert_eq!(copied.pipes, base().pipes);
}

// Copies take the chosen ids by position in the agent's list, not by the page's order (`ids.indexOf` in
// `duplicateObjects`): `b` is listed first, so its copy takes `x`.
#[test]
fn the_chosen_ids_pair_with_the_selection_by_position() {
    let copied = duplicate_objects(&base(), &names(&["b", "a"]), &names(&["x", "y"]), [0, 10])
        .expect("on the page");
    assert_eq!(find(&copied, "x").at, [240, 90], "the copy of b takes x");
    assert_eq!(find(&copied, "y").at, [40, 50], "the copy of a takes y");
}

// Closed form: the pipes between the copies keep their order, after the page's own pipes (clipboard.ts
// line 120).
#[test]
fn the_pipes_between_copies_keep_their_order() {
    let source = page(
        vec![
            object("a", "tank", [40, 40]),
            object("b", "tank", [240, 80]),
        ],
        vec![pipe("a.outlet", "b.inlet"), pipe("b.outlet", "a.inlet")],
    );
    let copied = duplicate_objects(&source, &names(&["a", "b"]), &names(&["c", "d"]), [0, 200])
        .expect("on the page");
    assert_eq!(
        copied.pipes,
        vec![
            pipe("a.outlet", "b.inlet"),
            pipe("b.outlet", "a.inlet"),
            pipe("c.outlet", "d.inlet"),
            pipe("d.outlet", "c.inlet"),
        ]
    );
}

// hmi-agent-edits.test.ts and hmi-agent-pipes.test.ts: the refusal "provide objects on this page to
// duplicate" (pipeEdits.ts line 100), for a selection with no object on the page and for an empty one.
#[test]
fn a_selection_with_no_object_on_the_page_is_refused() {
    for selection in [names(&["missing"]), names(&[])] {
        let error = duplicate_objects(&base(), &selection, &names(&["x"]), [0, 0])
            .expect_err("nothing to copy");
        assert_eq!(error.code, "INVALID_ARGUMENT");
        assert_eq!(
            error.to_string(),
            "INVALID_ARGUMENT: provide objects on this page to duplicate."
        );
    }
}

// Boundary: a copy that would move past the range of a page coordinate is refused rather than wrapped. This
// refusal has no sentence in the TypeScript, whose numbers do not overflow.
#[test]
fn a_copy_past_the_range_of_a_page_coordinate_is_refused() {
    let error = duplicate_objects(&base(), &names(&["a"]), &names(&["c"]), [i64::MAX, 0])
        .expect_err("out of range");
    assert_eq!(error.code, "INVALID_ARGUMENT");
}
