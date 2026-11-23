//! `trace.record`, `trace.read` and `trace.stop` (specification 11 §6.8.1, "Cycle traces"): their
//! parameters, their answers and their refusals. The expected CRC-32 and base64 were worked out with
//! Python's `zlib` and `base64`, not with this crate.

use alloc::string::ToString;
use serde_json::{json, Value};
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::trace::TraceEnd;
use trust_runtime_core::value::Duration;

use super::{
    parse_read, parse_record, parse_stop, part_json, refused, state_json, RecordRequest, TraceName,
    TraceRefusal, TraceState, Until, INSUFFICIENT_MEMORY, TRACE_BUSY,
};
use crate::tests::runner_ports::Fixture;

fn answer(response: &crate::protocol::ControlResponse) -> Value {
    serde_json::to_value(response).expect("a response serializes")
}

#[test]
fn a_recording_takes_cycles_or_a_duration_and_optionally_a_task() {
    // Catches a request read with both goals or neither, a zero goal that would record nothing and
    // look answered, and a task given with a duration, whose runs nothing would count.
    assert_eq!(
        parse_record(Some(&json!({"cycles": 100}))),
        Ok(RecordRequest {
            until: Until::Cycles(100),
            task: None
        })
    );
    assert_eq!(
        parse_record(Some(&json!({"duration_ns": 2_000_000_000_u64}))),
        Ok(RecordRequest {
            until: Until::Duration(Duration::from_millis(2_000)),
            task: None
        })
    );
    assert_eq!(
        parse_record(Some(&json!({"cycles": 5, "task": " MainTask "}))),
        Ok(RecordRequest {
            until: Until::Cycles(5),
            task: Some("MainTask".to_string())
        })
    );
    let both = Err("give exactly one of cycles and duration_ns".to_string());
    assert_eq!(
        parse_record(Some(&json!({"cycles": 1, "duration_ns": 1}))),
        both
    );
    assert_eq!(parse_record(Some(&json!({}))), both);
    assert!(parse_record(Some(&json!({"cycles": 0}))).is_err());
    assert!(parse_record(Some(&json!({"duration_ns": 0}))).is_err());
    assert!(parse_record(Some(&json!({"duration_ns": -1}))).is_err());
    assert!(parse_record(Some(&json!({"duration_ns": 5, "task": "MainTask"}))).is_err());
    assert!(parse_record(Some(&json!({"cycles": 5, "task": ""}))).is_err());
    assert!(parse_record(Some(&json!({"cycles": 5, "tasks": "MainTask"}))).is_err());
    assert!(parse_record(None).is_err());
}

#[test]
fn a_read_names_a_trace_by_id_or_the_fault_and_optionally_an_offset() {
    // Catches `"fault"` read as an id, an offset taken from another member, and a stop of the fault
    // trace, which the flight recorder keeps and no request ends.
    assert_eq!(
        parse_read(Some(&json!({"trace": 7}))),
        Ok((TraceName::Id(7), None))
    );
    assert_eq!(
        parse_read(Some(&json!({"trace": "fault", "offset": 4096}))),
        Ok((TraceName::Fault, Some(4096)))
    );
    assert!(parse_read(Some(&json!({"trace": "recent"}))).is_err());
    assert!(parse_read(Some(&json!({"trace": 7, "offset": -1}))).is_err());
    assert!(parse_read(Some(&json!({"offset": 0}))).is_err());
    assert_eq!(parse_stop(Some(&json!({"trace": 7}))), Ok(7));
    assert!(parse_stop(Some(&json!({"trace": "fault"}))).is_err());
}

#[test]
fn a_traces_state_is_answered_as_specification_11_gives_it() {
    // Catches a state answered with another member or name: the end's name from Appendix C, the
    // SHA-256 as 64 hexadecimal digits (here the SHA-256 of nothing).
    assert_eq!(state_json(&TraceState::None), json!({"state": "none"}));
    assert_eq!(
        state_json(&TraceState::Recording { cycles: 12 }),
        json!({"state": "recording", "cycles": 12})
    );
    let mut sha256 = [0_u8; 32];
    for (index, byte) in hex::decode_sha256().iter().enumerate() {
        sha256[index] = *byte;
    }
    assert_eq!(
        state_json(&TraceState::Ended {
            reason: TraceEnd::MemoryFull,
            cycles: 40,
            bytes: 9_000,
            sha256,
        }),
        json!({
            "state": "ended",
            "reason": "memory_full",
            "cycles": 40,
            "bytes": 9_000,
            "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        })
    );
}

#[test]
fn a_part_carries_its_bytes_as_base64_with_their_crc32() {
    // Catches a part whose data, checksum or next offset a client cannot check: the bytes 1 to 5
    // are "AQIDBAU=" in base64 and 0x470b99f4 in CRC-32 (Python's base64 and zlib).
    assert_eq!(
        part_json(8_192, &[1, 2, 3, 4, 5]),
        json!({"offset": 8_192, "data": "AQIDBAU=", "crc32": "470b99f4", "next_offset": 8_197})
    );
}

#[test]
fn each_refusal_carries_its_code_and_its_figures() {
    // Catches a refusal without the code a client acts on, or without the bytes it names
    // (specification 48 RU-59, "Memory").
    let memory = answer(&refused(
        1,
        TraceRefusal::InsufficientMemory {
            needed: 1_048_576,
            free: Some(4_096),
        },
    ));
    assert_eq!(memory["error_code"], json!(INSUFFICIENT_MEMORY));
    assert_eq!(
        memory["error"],
        json!("the recording needs 1048576 bytes, and 4096 are free")
    );
    let unknown_free = answer(&refused(
        1,
        TraceRefusal::InsufficientMemory {
            needed: 64,
            free: None,
        },
    ));
    assert_eq!(
        unknown_free["error"],
        json!("the recording needs 64 bytes, which this runtime cannot reserve")
    );
    let busy = answer(&refused(2, TraceRefusal::Busy { running: 9 }));
    assert_eq!(busy["error_code"], json!(TRACE_BUSY));
    assert!(busy["error"].as_str().unwrap().contains("trace 9"));
    let stopped = answer(&refused(
        3,
        TraceRefusal::NotRunning(ResourceState::Stopped),
    ));
    assert_eq!(stopped["error_code"], json!("resource_not_running"));
    let task = answer(&refused(4, TraceRefusal::UnknownTask("Fast".to_string())));
    assert!(task["error"].as_str().unwrap().contains("no task Fast"));
}

#[test]
fn a_runtime_that_records_no_trace_refuses_and_holds_none() {
    // Every runtime answers the trace requests (specification 34 STATE-08): one that records no
    // trace yet, this runner's default, refuses `trace.record` with its reason and holds no trace,
    // so `trace.read` and `trace.stop` answer `unknown_trace`. Catches a request a runtime does not
    // know, which it would refuse as unknown instead.
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let record = fixture.call(
        &mut runner,
        &json!({"id": 2, "type": "trace.record", "params": {"cycles": 3}}),
    );
    assert_eq!(record["ok"], json!(false));
    assert_eq!(
        record["error"],
        json!("this runtime records no cycle trace")
    );
    let read = fixture.call(
        &mut runner,
        &json!({"id": 3, "type": "trace.read", "params": {"trace": 1}}),
    );
    assert_eq!(read["error_code"], json!("unknown_trace"));
    let stop = fixture.call(
        &mut runner,
        &json!({"id": 4, "type": "trace.stop", "params": {"trace": 1}}),
    );
    assert_eq!(stop["error_code"], json!("unknown_trace"));
}

/// The SHA-256 of nothing, from its hexadecimal digits.
mod hex {
    pub(super) fn decode_sha256() -> alloc::vec::Vec<u8> {
        let digits = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        (0..digits.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&digits[at..at + 2], 16).unwrap())
            .collect()
    }
}
