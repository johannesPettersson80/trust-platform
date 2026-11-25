//! A cycle trace read over the control link (specification 11 §6.8.1, "Cycle traces";
//! specification 34 PORT-05): its state, its parts and the whole file checked, against a scripted
//! runtime. The expected bytes, base64, CRC-32s and SHA-256s come from Python's hashlib, base64 and
//! zlib, not from the code under test.

use std::time::Duration;

use serde_json::json;

use crate::fake_runtime;
use crate::trace::{fault_trace, part, EndedTrace, TraceState};
use crate::{ControlClient, ControlEndpoint, ControlError};

fn connect(endpoint: &str) -> ControlClient {
    let endpoint = ControlEndpoint::parse(endpoint).expect("a control endpoint");
    ControlClient::connect(&endpoint, None, Duration::from_secs(2))
        .expect("connect to the fake runtime")
}

/// The SHA-256 of the bytes 01 to 05.
const ONE_TO_FIVE_SHA256: &str = "74f81fe167d99b4cb41d6d0ccda82278caee9f3e2f25d5e5a3936ff3dcec60d0";

#[test]
fn a_state_is_none_recording_or_ended_with_all_its_figures() {
    // Specification 11: `trace.read` answers `none` for a fault's trace before a fault,
    // `recording` with its cycles, or `ended` with the reason, cycles, bytes and SHA-256. Catches
    // an ended trace taken without its figures, which the file could not be checked against, and
    // an answer without a state taken as one.
    assert_eq!(
        TraceState::from_json(&json!({"state": "none"})).unwrap(),
        TraceState::None
    );
    assert_eq!(
        TraceState::from_json(&json!({"state": "recording", "cycles": 3})).unwrap(),
        TraceState::Recording { cycles: 3 }
    );
    let sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    assert_eq!(
        TraceState::from_json(&json!({
            "state": "ended", "reason": "stopped", "cycles": 2, "bytes": 900, "sha256": sha256
        }))
        .unwrap(),
        TraceState::Ended(EndedTrace {
            reason: "stopped".to_owned(),
            cycles: 2,
            bytes: 900,
            sha256: sha256.to_owned(),
        })
    );
    assert!(TraceState::from_json(
        &json!({"state": "ended", "reason": "complete", "cycles": 2, "bytes": 900})
    )
    .is_err());
    assert!(TraceState::from_json(&json!({"cycles": 2})).is_err());
}

#[test]
fn a_part_is_taken_only_whole_and_at_its_offset() {
    // Specification 34 PORT-05: a part carries its offset, its bytes in base64, their CRC-32 and the
    // next offset. The bytes 01 to 05 are "AQIDBAU=" with the CRC-32 470b99f4 (Python's base64 and
    // zlib). Catches a damaged part kept, a part from another offset spliced in, and an empty part,
    // on which the reading would never end.
    let good =
        json!({"offset": 8192, "data": "AQIDBAU=", "crc32": "470b99f4", "next_offset": 8197});
    assert_eq!(part(&good, 8192).unwrap(), vec![1, 2, 3, 4, 5]);
    assert!(part(&good, 4096).is_err());
    let damaged =
        json!({"offset": 8192, "data": "AQIDBAU=", "crc32": "470b99f5", "next_offset": 8197});
    assert!(part(&damaged, 8192).is_err());
    let next =
        json!({"offset": 8192, "data": "AQIDBAU=", "crc32": "470b99f4", "next_offset": 8196});
    assert!(part(&next, 8192).is_err());
    let empty = json!({"offset": 8192, "data": "", "crc32": "00000000", "next_offset": 8192});
    assert!(part(&empty, 8192).is_err());
    let bare = json!({"offset": 8192, "data": "AQIDBAU="});
    assert!(part(&bare, 8192).is_err());
}

#[test]
fn a_faults_trace_is_read_whole_in_its_parts() {
    // Specification 11 §6.8.1: `{"trace": "fault"}` reads the flight recorder's trace of the last
    // fault; once its state has ended, its file is read part by part from offset 0. Worked by hand
    // for a fault's trace of 2 cycles whose file is the bytes 01 to 05, served in parts of 3 and 2
    // bytes: "AQID" with the CRC-32 55bc801d, then "BAU=" with 55df2374; the whole file's SHA-256
    // is 74f81fe1... (Python's hashlib). Catches a trace read from another offset, a part dropped
    // or repeated, and requests that name another trace.
    let runtime = fake_runtime::answering(vec![
        format!(r#"{{"id":1,"ok":true,"result":{{"state":"ended","reason":"faulted","cycles":2,"bytes":5,"sha256":"{ONE_TO_FIVE_SHA256}"}}}}"#),
        r#"{"id":2,"ok":true,"result":{"offset":0,"data":"AQID","crc32":"55bc801d","next_offset":3}}"#.to_string(),
        r#"{"id":3,"ok":true,"result":{"offset":3,"data":"BAU=","crc32":"55df2374","next_offset":5}}"#.to_string(),
    ]);
    let mut client = connect(&runtime.endpoint);
    let (ended, file) = fault_trace(&mut client)
        .expect("the fault's trace reads")
        .expect("a fault's trace");
    drop(client);
    assert_eq!(
        ended,
        EndedTrace {
            reason: "faulted".to_owned(),
            cycles: 2,
            bytes: 5,
            sha256: ONE_TO_FIVE_SHA256.to_owned(),
        }
    );
    assert_eq!(file, [1, 2, 3, 4, 5]);
    let params: Vec<_> = runtime
        .requests()
        .into_iter()
        .map(|request| (request["type"].clone(), request["params"].clone()))
        .collect();
    assert_eq!(
        params,
        [
            (json!("trace.read"), json!({"trace": "fault"})),
            (json!("trace.read"), json!({"trace": "fault", "offset": 0})),
            (json!("trace.read"), json!({"trace": "fault", "offset": 3})),
        ]
    );
}

#[test]
fn no_faults_trace_is_none_and_a_file_that_differs_is_refused() {
    // Specification 11 §6.8.1: before a fault, or without a flight recorder, `trace.read` of
    // "fault" answers `none`, which is no trace and no error; a file whose SHA-256 is not the one
    // its state gave is refused. Worked by hand: the bytes 01 to 05 served whole, against a state
    // that names the SHA-256 of no bytes (e3b0c442...). Catches `none` taken as an error or as an
    // empty trace, and a damaged file kept.
    let none = fake_runtime::answering(vec![
        r#"{"id":1,"ok":true,"result":{"state":"none"}}"#.to_string()
    ]);
    let mut client = connect(&none.endpoint);
    assert_eq!(fault_trace(&mut client).expect("an answer"), None);
    drop(client);
    assert_eq!(none.requests().len(), 1);

    let differs = fake_runtime::answering(vec![
        r#"{"id":1,"ok":true,"result":{"state":"ended","reason":"faulted","cycles":2,"bytes":5,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"}}"#.to_string(),
        r#"{"id":2,"ok":true,"result":{"offset":0,"data":"AQIDBAU=","crc32":"470b99f4","next_offset":5}}"#.to_string(),
    ]);
    let mut client = connect(&differs.endpoint);
    assert!(matches!(
        fault_trace(&mut client),
        Err(ControlError::Malformed(_))
    ));
}
