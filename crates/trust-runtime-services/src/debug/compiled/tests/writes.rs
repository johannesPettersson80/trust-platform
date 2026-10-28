//! `debug.set`, a debugger's write of a compiled program's variable (specification 11 §6.13.4;
//! specification 44 §13.2; specification 13, "setVariable and setExpression"), over the program of
//! the module's fixture. At a hold the write goes into the held memory at once and the next read
//! shows it; between scans nothing is written and the answer names the path to queue it by.

use alloc::string::String;

use serde_json::{json, Value as Json};

use super::*;
use crate::protocol::ControlRequest;

/// `debug.set` with `params` on `held`, through the shared debug operations.
fn set(held: &Held, params: Json) -> ControlResponse {
    let request = ControlRequest {
        id: 1,
        r#type: String::from("debug.set"),
        params: Some(params),
        auth: None,
        request_id: None,
    };
    crate::debug::answer(&request, held)
}

/// The entry named `name` of the listing of handle `reference`.
fn entry(held: &Held, reference: u32, name: &str) -> Json {
    held.listed(reference)
        .as_array()
        .and_then(|entries| entries.iter().find(|entry| entry["name"] == name))
        .cloned()
        .unwrap_or_else(|| panic!("the listing of {reference} shows {name}"))
}

/// The bytes of area `id` of the held memory.
fn area(held: &Held, id: usize) -> Vec<u8> {
    held.memory.areas.borrow()[id].clone()
}

#[test]
fn a_write_at_a_hold_goes_into_the_held_memory_and_the_next_read_shows_it() {
    // Specification 44 §13.2: held in `Main` (frame 1), the global `Limit`, 7 at G 0, is written 8
    // at once; the answer reports its path, its type and the value it holds now, and the next
    // `debug.variables` of `Globals` shows 8. Catches a write queued for the hold's end, which the
    // next read would not show, and an answer of the requested text instead of the value held.
    let held = Held::new(vec![main_body(1)]);
    held.open_scopes(1);
    let answer = result(set(
        &held,
        json!({"variables_reference": 2, "name": "Limit", "value": "8"}),
    ));
    assert_eq!(
        answer,
        json!({"path": "Limit", "type": "INT", "value": "8", "written": true})
    );
    assert_eq!(entry(&held, 2, "Limit"), scalar("Limit", "8", "INT"));
    assert_eq!(&area(&held, 4)[0..2], &8i16.to_le_bytes());
}

#[test]
fn a_member_of_a_structure_and_an_element_of_an_array_are_written_where_they_lie() {
    // Specification 44 §13.2: a structure and an array of values a listing shows keep their place,
    // so their members are written where they lie: `Origin.Y` at G 4 := -5 and `Levels[2]` (the
    // element at position 2 of ARRAY[0..2], G 6 + 2 * 2 = G 10) := 33; the listing of `Origin`
    // read again shows Y -5. The paths are the ones the program's tables find them by. Catches a
    // structure listed from a copy, which a write could not reach, and an element counted from the
    // array's lower bound twice.
    let held = Held::new(vec![main_body(1)]);
    held.open_scopes(1);
    let globals = held.listed(2);
    let reference = |name: &str| {
        globals
            .as_array()
            .and_then(|entries| entries.iter().find(|entry| entry["name"] == name))
            .and_then(|entry| entry["variablesReference"].as_u64())
            .and_then(|reference| u32::try_from(reference).ok())
            .unwrap_or_else(|| panic!("{name} has a handle"))
    };
    let (origin, levels) = (reference("Origin"), reference("Levels"));
    assert_eq!(
        result(set(
            &held,
            json!({"variables_reference": origin, "name": "Y", "value": "-5"})
        )),
        json!({"path": "Origin.Y", "type": "INT", "value": "-5", "written": true})
    );
    assert_eq!(
        result(set(
            &held,
            json!({"variables_reference": levels, "name": "[2]", "value": "33"})
        )),
        json!({"path": "Levels[2]", "type": "INT", "value": "33", "written": true})
    );
    assert_eq!(entry(&held, origin, "Y"), scalar("Y", "-5", "INT"));
    let globals = area(&held, 4);
    assert_eq!(&globals[4..6], &(-5i16).to_le_bytes());
    assert_eq!(&globals[10..12], &33i16.to_le_bytes());
    assert_eq!(&globals[8..10], &20i16.to_le_bytes(), "Levels[1] keeps 20");
}

#[test]
fn a_local_of_a_held_frame_is_written_but_never_forced() {
    // Specification 44 §13.2: held in `SumTo` (frame 2), its frame variable `N`, 12, is written 13
    // into the frame at once; it has no path, so a force of it is refused, since no force could name
    // it after the frame is gone. A path whose first name is a frame variable is looked up in the
    // frame the request names: `Tally.Count` of frame 2 is the frame's +8, 60 := 61. Catches a frame
    // variable written into an area, and a force of a local accepted that would hold nothing.
    let held = Held::new(vec![sum_to(2), main_body(1)]);
    held.open_scopes(2);
    assert_eq!(
        result(set(
            &held,
            json!({"variables_reference": 1, "name": "N", "value": "13"})
        )),
        json!({"path": null, "type": "INT", "value": "13", "written": true})
    );
    assert_eq!(
        error(&set(
            &held,
            json!({"variables_reference": 1, "name": "N", "value": "14", "force": true})
        )),
        "a variable of a frame on the held call stack cannot be forced: write it instead"
    );
    assert_eq!(
        result(set(
            &held,
            json!({"path": "Tally.Count", "frame_id": 2, "value": "61"})
        )),
        json!({"path": null, "type": "DINT", "value": "61", "written": true})
    );
    let frame = held.memory.variables.borrow()[0].clone();
    assert_eq!(&frame[0..2], &13i16.to_le_bytes());
    assert_eq!(&frame[8..12], &61i32.to_le_bytes());
}

#[test]
fn a_path_names_a_member_of_an_element_of_an_instance_array_and_a_retained_variable() {
    // Specification 11 §6.13.4 and specification 44 §13.3: `plant.ctrs[2].step`, compared without
    // case, is the `Step` of the second element of `Ctrs` (P 12 + 8 = P 20), 3 := 4; the retained
    // `Hits`, which the `Retain` scope lists as `@program/Plant/Hits`, has the path `Plant.Hits`.
    // Catches a path compared with case and a retained variable answered with its checkpoint key,
    // which no force could name.
    let held = Held::new(vec![main_body(1)]);
    held.open_scopes(1);
    assert_eq!(
        result(set(
            &held,
            json!({"path": "plant.ctrs[2].step", "value": "4"})
        )),
        json!({"path": "Plant.Ctrs[2].Step", "type": "INT", "value": "4", "written": true})
    );
    assert_eq!(
        result(set(
            &held,
            json!({"variables_reference": 3, "name": "@program/Plant/Hits", "value": "10"})
        )),
        json!({"path": "Plant.Hits", "type": "DINT", "value": "10", "written": true})
    );
    assert_eq!(&area(&held, 6)[20..22], &4i16.to_le_bytes());
    assert_eq!(&area(&held, 5)[4..8], &10i32.to_le_bytes());
}

#[test]
fn a_refused_write_says_why_and_leaves_the_memory_as_it_was() {
    // Specification 11 §6.13.4: an instance, a value of another type, a handle the table does not
    // hold, a name the listing does not show, both forms of the variable at once and an instance
    // the `Instances` scope lists are refused with their reason, and nothing is written. Catches a
    // refusal that wrote first, and an instance written as its first member.
    let held = Held::new(vec![main_body(1)]);
    held.open_scopes(1);
    let before = (area(&held, 4), area(&held, 6));
    for (params, reason) in [
        (
            json!({"path": "Plant.Ctr", "value": "1"}),
            "Plant.Ctr is an instance, which holds no value",
        ),
        (
            json!({"variables_reference": 2, "name": "Limit", "value": "TRUE"}),
            "'TRUE' is not a value of type INT",
        ),
        (
            json!({"variables_reference": 99, "name": "Limit", "value": "1"}),
            "unknown variables reference 99",
        ),
        (
            json!({"variables_reference": 2, "name": "Nothing", "value": "1"}),
            "the listing shows no variable Nothing",
        ),
        (
            json!({"variables_reference": 2, "name": "Limit", "path": "Limit", "value": "1"}),
            "debug.set names its variable by variables_reference and name, or by path",
        ),
        (
            json!({"variables_reference": 4, "name": "Counter#13", "value": "1"}),
            "Counter#13 is an instance, which holds no value",
        ),
    ] {
        assert_eq!(error(&set(&held, params.clone())), reason, "{params}");
    }
    assert_eq!((area(&held, 4), area(&held, 6)), before);
}

#[test]
fn between_scans_nothing_is_written_and_the_answer_names_the_path_to_queue() {
    // Specification 11 §6.13.4: with no scan held, `debug.set` writes nothing and answers the
    // variable's path and the admitted value, for the client to queue with `set`; without a value
    // it reads the variable. `Speed` is 5 at P 8. Catches a write between scans that bypassed the
    // cycle start, and an answer without the path a client queues by.
    let held = Held::new(Vec::new());
    assert_eq!(
        result(set(&held, json!({"path": "Plant.Speed", "value": "6"}))),
        json!({"path": "Plant.Speed", "type": "INT", "value": "6", "written": false})
    );
    assert_eq!(
        result(set(&held, json!({"path": "Plant.Speed"}))),
        json!({"path": "Plant.Speed", "type": "INT", "value": "5", "written": false})
    );
    assert_eq!(&area(&held, 6)[8..10], &5i16.to_le_bytes());
}

#[test]
fn a_row_of_the_io_scope_is_answered_with_its_address() {
    // Specification 11 §6.13.4: a row of the I/O scope is written with the I/O requests at its
    // address, so `debug.set` answers the row `start` with `%IX0.0` and writes nothing. Catches a
    // row taken for a variable of the same name.
    let mut held = Held::new(vec![main_body(1)]);
    held.io = one_input();
    held.open_scopes(1);
    let io_root = 4;
    let inputs = entry(&held, io_root, "Inputs")["variablesReference"]
        .as_u64()
        .and_then(|reference| u32::try_from(reference).ok())
        .expect("the inputs have a handle");
    assert_eq!(
        result(set(
            &held,
            json!({"variables_reference": inputs, "name": "start", "value": "FALSE"})
        )),
        json!({"address": "%IX0.0", "path": null, "written": false})
    );
}
