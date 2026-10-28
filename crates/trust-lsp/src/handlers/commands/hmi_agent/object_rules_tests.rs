//! Tests of deleting objects and naming new ones (specification 39, §3.3 and §6; specification 43,
//! AT-05). Ported from `editors/vscode/src/test/suite/hmi-builder-editing.test.ts`,
//! `hmi-builder-pipes.test.ts` and `hmi-agent-edits.test.ts`; their refusals (`dependent`) belong to the
//! agent's edit operations and are not ported here.

use std::collections::BTreeMap;

use trust_runtime::hmi::page_files::{HmiObject, HmiPage, HmiPipe, HmiPipeStyle};

use super::{delete_objects, next_object_id};

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

fn ids(objects: &[HmiObject]) -> Vec<&str> {
    objects.iter().map(|object| object.id.as_str()).collect()
}

// hmi-builder-editing.test.ts: "deleting removes the selected objects and keeps the rest in order"
// (specification 39, §3.3)
#[test]
fn deleting_removes_the_selected_objects_and_keeps_the_rest_in_order() {
    let before = page(
        vec![
            object("a", "value", [0, 0]),
            object("b", "value", [0, 0]),
            object("c", "value", [0, 0]),
        ],
        Vec::new(),
    );
    let after = delete_objects(&before, &["b".to_string()]);
    assert_eq!(ids(&after.objects), ["a", "c"]);
}

// hmi-agent-edits.test.ts: "deleting connected equipment requires explicit dependent removal", outcome of
// the removal with its dependents included (specification 43, AT-05)
#[test]
fn removing_equipment_with_its_dependents_removes_its_pipe() {
    let before = page(
        vec![
            object("a", "tank", [40, 40]),
            object("b", "tank", [240, 80]),
        ],
        vec![pipe("a.outlet", "b.inlet")],
    );
    let after = delete_objects(&before, &["a".to_string()]);
    assert_eq!(ids(&after.objects), ["b"]);
    assert_eq!(after.pipes, Vec::new());
}

// hmi-agent-edits.test.ts: "equipment and pipe deletion keep surviving instrument attachments on their
// original pipes", removal part (specification 39, §6 item 5)
#[test]
fn removing_equipment_keeps_a_surviving_instrument_on_its_own_pipe() {
    let before = page(
        vec![
            object("a", "tank", [40, 40]),
            object("b", "tank", [240, 80]),
            object("c", "tank", [440, 80]),
            attached(object("sensor", "tank", [400, 240]), "pipe:1"),
        ],
        vec![pipe("a.outlet", "b.inlet"), pipe("b.outlet", "c.inlet")],
    );
    let after = delete_objects(&before, &["a".to_string()]);
    let sensor = after.objects.iter().find(|object| object.id == "sensor");
    assert_eq!(
        sensor.and_then(|object| object.attach.as_deref()),
        Some("pipe:0")
    );
    assert_eq!(
        before.objects[3].attach.as_deref(),
        Some("pipe:1"),
        "the source page is unchanged"
    );
}

// hmi-builder-pipes.test.ts: "deleting an object deletes its pipes; deleting a pipe leaves the objects",
// deleteObjects part (specification 39, §6)
#[test]
fn deleting_an_object_deletes_its_pipes() {
    let joined = page(
        vec![
            object("p-101", "pump", [40, 40]),
            object("v-101", "valve", [480, 40]),
        ],
        vec![
            pipe("p-101.outlet", "v-101.inlet"),
            pipe("v-101.outlet", "p-101.inlet"),
        ],
    );
    assert_eq!(
        delete_objects(&joined, &["v-101".to_string()]).pipes,
        Vec::new()
    );
}

// Boundary: a name on no object still removes the pipes that its ports name, as `deleteObjects` tests
// the names alone (editing.ts line 137).
#[test]
fn a_name_on_no_object_still_removes_the_pipes_its_ports_name() {
    let before = page(
        vec![object("b", "tank", [0, 0])],
        vec![pipe("ghost.outlet", "b.inlet")],
    );
    let after = delete_objects(&before, &["ghost".to_string()]);
    assert_eq!(ids(&after.objects), ["b"]);
    assert_eq!(after.pipes, Vec::new());
}

// Boundary: a port without a dot has no object part of its own. `deleteObjects` takes
// `slice(0, -1)` of it, so `ab` belongs to the object `a` (editing.ts line 137).
#[test]
fn a_port_without_a_dot_belongs_to_the_name_without_its_last_character() {
    let before = page(
        vec![object("a", "tank", [0, 0]), object("c", "tank", [0, 0])],
        vec![pipe("ab", "c.in"), pipe("ab.out", "c.in")],
    );
    let after = delete_objects(&before, &["a".to_string()]);
    assert_eq!(after.pipes, vec![pipe("ab.out", "c.in")]);
}

// Closed form of `nextId` (editing.ts line 20): the first `<kind>-<n>` that no object uses, from 1.
#[test]
fn the_next_id_is_the_first_free_number_of_its_kind() {
    let crowded = page(
        vec![
            object("tank-1", "tank", [0, 0]),
            object("tank-2", "tank", [0, 0]),
            object("tank-4", "tank", [0, 0]),
        ],
        Vec::new(),
    );
    assert_eq!(next_object_id(&crowded, "tank"), "tank-3");
    assert_eq!(next_object_id(&crowded, "valve"), "valve-1");
    assert_eq!(
        next_object_id(&page(Vec::new(), Vec::new()), "tank"),
        "tank-1"
    );
}
