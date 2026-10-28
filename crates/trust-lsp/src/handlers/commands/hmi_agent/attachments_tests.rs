//! Tests of the attachment rules (specification 39, §6 item 5). Ported from the deletion and attachment
//! assertions of `editors/vscode/src/test/suite/hmi-builder-attachments.test.ts` (its splitting test is
//! canvas interaction and is not ported) and from `hmi-agent-edits.test.ts`.

use std::collections::{BTreeMap, BTreeSet};

use trust_runtime::hmi::page_files::{HmiObject, HmiPage, HmiPipe, HmiPipeStyle};

use super::super::object_rules::delete_objects;
use super::super::pipe_rules::delete_pipes;
use super::{moved_attachments, remap_after_deleting_pipes};

fn object(id: &str, kind: &str, at: [i64; 2]) -> HmiObject {
    serde_json::from_value(
        serde_json::json!({ "id": id, "type": kind, "at": at, "size": [128, 108] }),
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
        title: "Plant".to_string(),
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

/// The attachments of the page's sensors, in the page's order.
fn sensors(page: &HmiPage) -> Vec<(&str, Option<&str>)> {
    page.objects
        .iter()
        .filter(|object| object.kind.ends_with("sensor"))
        .map(|object| (object.id.as_str(), object.attach.as_deref()))
        .collect()
}

/// A pump feeding two valves, with a flow sensor on each pipe and a temperature sensor on the pump
/// (`plant()` in `hmi-builder-attachments.test.ts`).
fn plant() -> HmiPage {
    page(
        vec![
            object("p-101", "pump", [40, 40]),
            object("v-101", "valve", [880, 16]),
            object("v-102", "valve", [880, 400]),
            attached(object("ft-1", "flow-sensor", [300, 200]), "pipe:0"),
            attached(object("ft-2", "flow-sensor", [300, 500]), "pipe:1"),
            attached(object("tt-1", "temperature-sensor", [40, 300]), "p-101"),
        ],
        vec![
            pipe("p-101.outlet", "v-101.inlet"),
            pipe("v-101.outlet", "v-102.inlet"),
        ],
    )
}

// hmi-builder-attachments.test.ts: "deleting a pipe drops the attachments to it and renumbers those to
// later pipes" (specification 39, §6 item 5)
#[test]
fn deleting_a_pipe_drops_its_attachments_and_renumbers_later_ones() {
    let after = delete_pipes(&plant(), &["pipe:0".to_string()]);
    assert_eq!(
        sensors(&after),
        [
            ("ft-1", None),
            ("ft-2", Some("pipe:0")),
            ("tt-1", Some("p-101"))
        ]
    );
}

// hmi-builder-attachments.test.ts: "deleting an object drops the attachments to it and to its pipes"
// (specification 39, §6 item 5)
#[test]
fn deleting_an_object_drops_attachments_to_it_and_to_its_pipes() {
    let after = delete_objects(&plant(), &["p-101".to_string()]);
    assert_eq!(
        sensors(&after),
        [("ft-1", None), ("ft-2", Some("pipe:0")), ("tt-1", None)]
    );
}

// Closed form of `afterDeleting` (attachments.ts line 38): with pipes 1 and 3 deleted, pipes 0 to 5 keep
// the places 0, none, 1, none, 2 and 3.
#[test]
fn a_pipe_keeps_its_place_less_the_pipes_deleted_before_it() {
    let gone = BTreeSet::from([1, 3]);
    let places: Vec<Option<usize>> = (0..6)
        .map(|index| remap_after_deleting_pipes(&gone, index))
        .collect();
    assert_eq!(places, [Some(0), None, Some(1), None, Some(2), Some(3)]);
}

// Boundary of `afterDeleting`: with no pipe deleted, every place is kept.
#[test]
fn with_no_pipe_deleted_every_place_is_kept() {
    let gone = BTreeSet::new();
    assert_eq!(remap_after_deleting_pipes(&gone, 7), Some(7));
}

// Boundary of `movedAttachments` (attachments.ts line 18): `pipe:007` names pipe 7, and the TypeScript
// writes an attachment back in its recomputed form, so it reads `pipe:7`.
#[test]
fn a_pipe_name_with_leading_zeros_comes_back_canonical() {
    let page = page(
        vec![attached(object("ft-1", "flow-sensor", [0, 0]), "pipe:007")],
        Vec::new(),
    );
    let moved = moved_attachments(page, &BTreeSet::new(), &[]);
    assert_eq!(moved.objects[0].attach.as_deref(), Some("pipe:7"));
}

// `movedAttachments` (attachments.ts line 9): an attachment to a deleted equipment goes, and an attachment
// to any other object stays as written.
#[test]
fn an_attachment_goes_with_its_equipment_and_others_stay() {
    let page = page(
        vec![
            object("tank-1", "tank", [0, 0]),
            attached(object("ft-1", "flow-sensor", [0, 0]), "tank-1"),
            attached(object("ft-2", "flow-sensor", [0, 0]), "tank-2"),
        ],
        Vec::new(),
    );
    let moved = moved_attachments(page, &BTreeSet::new(), &["tank-1".to_string()]);
    assert_eq!(sensors(&moved), [("ft-1", None), ("ft-2", Some("tank-2"))]);
}
