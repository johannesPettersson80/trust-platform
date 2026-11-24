//! The replay of a cycle trace over the control link (specification 48 RU-60; specification 11
//! §6.8.1, "Replaying a trace"): the upload and the request against a scripted runtime, and the
//! report that a tool prints from the answer. The expected bytes of the upload come from Python's
//! hashlib, base64 and zlib, not from the code under test.

use std::time::Duration;

use serde_json::{json, Value};

use crate::fake_runtime;
use crate::replay::{replay, ReplayAnswer};
use crate::{ControlClient, ControlEndpoint, ControlError};

fn connect(endpoint: &str) -> ControlClient {
    let endpoint = ControlEndpoint::parse(endpoint).expect("a control endpoint");
    ControlClient::connect(&endpoint, None, Duration::from_secs(2))
        .expect("connect to the fake runtime")
}

fn answer(text: &Value) -> ReplayAnswer {
    ReplayAnswer::from_json(text).expect("a replay's answer")
}

#[test]
fn a_trace_is_uploaded_in_the_parts_its_upload_names_then_replayed_by_it() {
    // Specification 11 §6.8.1, "Replaying a trace": the trace is uploaded with `transfer.begin`
    // `{"for": "trace", "bytes", "sha256"}` and its parts, and the request names the upload. Worked
    // by hand for the 5 bytes `abcde` and a part size of 2: the parts are `ab` at 0, `cd` at 2 and
    // `e` at 4, each with its base64 and CRC-32. Catches a trace uploaded for another use, parts of
    // another size than the upload names, a wrong offset or checksum, and a replay that names
    // another upload.
    let runtime = fake_runtime::answering(vec![
        r#"{"id":1,"ok":true,"result":{"transfer":7,"part_bytes":2}}"#.to_string(),
        r#"{"id":2,"ok":true,"result":{"received":2}}"#.to_string(),
        r#"{"id":3,"ok":true,"result":{"received":4}}"#.to_string(),
        r#"{"id":4,"ok":true,"result":{"received":5}}"#.to_string(),
        r#"{"id":5,"ok":true,"result":{"matched":true,"cycles":2,"kind":"same_program","first_difference":null,"later_differing_cycles":0,"notes":[]}}"#.to_string(),
    ]);
    let mut client = connect(&runtime.endpoint);
    let replayed = replay(&mut client, b"abcde").expect("the replay answers");
    assert!(replayed.matched);
    drop(client);
    let requests: Vec<(Value, Value)> = runtime
        .requests()
        .into_iter()
        .map(|request| (request["type"].clone(), request["params"].clone()))
        .collect();
    let part = |offset: usize, data: &str, crc32: &str| {
        (
            json!("transfer.part"),
            json!({"transfer": 7, "offset": offset, "data": data, "crc32": crc32}),
        )
    };
    assert_eq!(
        requests,
        [
            (
                json!("transfer.begin"),
                json!({"bytes": 5, "for": "trace",
                    "sha256": "36bbe50ed96841d10443bcb670d6554f0a34b761be67ec9c4a8ad2c0c44ca42c"})
            ),
            part(0, "YWI=", "9e83486d"),
            part(2, "Y2Q=", "45d68fda"),
            part(4, "ZQ==", "efda7a5a"),
            (json!("simulator.replay"), json!({"transfer": 7})),
        ]
    );
}

#[test]
fn a_replay_that_matches_reports_its_cycles_and_ends_with_0() {
    // RU-60, "The result": "A replay that matches every cycle prints the cycles it ran and ends
    // with 0." Catches a report without the cycles or what was replayed, and a matched replay
    // that ends with 1.
    let matched = answer(
        &json!({"matched": true, "cycles": 4, "kind": "same_program",
        "first_difference": null, "later_differing_cycles": 0, "notes": []}),
    );
    assert_eq!(
        matched.to_string(),
        "Replayed 4 cycles of the same program: every cycle's outputs match the trace.\n"
    );
    assert_eq!(matched.exit_code(), 0);
}

#[test]
fn a_changed_program_reports_its_first_differing_outputs_the_later_cycles_and_its_notes() {
    // RU-60, "The result": at the first cycle whose outputs differ, the report gives the cycle and
    // its logical time, each differing output with its name, its address, the trace's value and the
    // build's, and how many later cycles differ, and it ends with 1. The answer is the changed
    // build's of the end-to-end test: 25 against 18 at cycle 1, 30 ms, one later cycle, three
    // notes. A text is written without its quotes and a missing value as `no value`. Catches a
    // value, an address or the time left out, a later count of one written as many, and notes lost.
    let changed = answer(
        &json!({"matched": false, "cycles": 3, "kind": "changed_program",
        "first_difference": {"cycle": 1, "time_ns": 30_000_000, "outputs": [
            {"path": "level", "address": "%QW0", "trace": 25, "build": 18},
            {"path": "Main.mode", "address": "%QB4", "trace": "Run", "build": null}]},
        "later_differing_cycles": 1,
        "notes": ["limit: declaration changed: the trace's value converted to it",
            "the build has no variable gain: its changes are skipped"]}),
    );
    assert_eq!(
        changed.to_string(),
        "Replayed 3 cycles of a changed program.\n\
         The first difference is at cycle 1, at the logical time 30 ms:\n\
         \x20 level (%QW0): the trace has 25, the build 18\n\
         \x20 Main.mode (%QB4): the trace has Run, the build no value\n\
         1 later cycle differs too.\n\
         Notes:\n\
         \x20 limit: declaration changed: the trace's value converted to it\n\
         \x20 the build has no variable gain: its changes are skipped\n"
    );
    assert_eq!(changed.exit_code(), 1);
}

#[test]
fn the_same_program_that_differs_reports_a_determinism_fault_or_an_input_the_trace_lacks() {
    // RU-60, "What was replayed": for the same program, and the same program for another device,
    // "NX-04 makes every difference a determinism fault or an input the trace did not hold, and the
    // report says so". A trace that ended with a fault expects the build's fault at the same cycle,
    // so the report names both faults. Worked by hand: 40.5 ms for 40 500 000 ns, no later cycle.
    // Catches the sentence left out, or given for a changed program, a fault missing, and a time
    // cut to whole milliseconds.
    let faulted = answer(&json!({"matched": false, "cycles": 5,
        "kind": "same_sources_other_target",
        "first_difference": {"cycle": 4, "time_ns": 40_500_000, "faults": {
            "trace": "DivisionByZero in POU 2 at statement 3", "build": null}},
        "later_differing_cycles": 0, "notes": []}));
    assert_eq!(
        faulted.to_string(),
        "Replayed 5 cycles of the same program for another device.\n\
         The first difference is at cycle 4, at the logical time 40.5 ms:\n\
         \x20 the trace's cycle ended with DivisionByZero in POU 2 at statement 3, the build's \
         with no fault\n\
         No later cycle differs.\n\
         The build is the same program for another device, so each difference is a determinism \
         fault or an input the trace does not hold (specification 44 NX-04).\n"
    );
}

#[test]
fn an_answer_that_is_not_a_replays_is_malformed() {
    // Specification 11 §6.8.1: the answer holds `matched`, `cycles`, `kind`, `first_difference`,
    // `later_differing_cycles` and `notes`. Catches an unknown kind, missing notes, and a first
    // difference with both outputs and faults taken as an answer.
    let whole = json!({"matched": true, "cycles": 1, "kind": "same_program",
        "first_difference": null, "later_differing_cycles": 0, "notes": []});
    let mut unknown_kind = whole.clone();
    unknown_kind["kind"] = json!("another_program");
    let mut no_notes = whole.clone();
    no_notes.as_object_mut().expect("an object").remove("notes");
    let mut both = whole;
    both["first_difference"] = json!({"cycle": 0, "time_ns": 0, "outputs": [],
        "faults": {"trace": null, "build": null}});
    for broken in [unknown_kind, no_notes, both] {
        assert!(
            matches!(
                ReplayAnswer::from_json(&broken),
                Err(ControlError::Malformed(_))
            ),
            "{broken}"
        );
    }
}
