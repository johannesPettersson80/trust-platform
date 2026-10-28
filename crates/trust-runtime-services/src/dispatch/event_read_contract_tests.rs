//! Event reads for a link that moves replies of a bounded length (specification 34, PORT-05):
//! written event by event within the bound, they are byte for byte the reply of the unbounded
//! pipeline when it fits and the refusal `reply longer than <bound> bytes` when it does not, for
//! random stores, limits and bounds, with the same audit records; reads that are refused answer
//! as the unbounded pipeline answers them.

use alloc::collections::VecDeque;
use alloc::format;
use alloc::string::String;
use alloc::vec;

use serde_json::json;
use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::value::Duration;

use super::{prepare_line, prepare_line_within};
use crate::events::RuntimeEvent;
use crate::link::DEVICE_FRAME_LEN;
use crate::tests::fake_host::FakeHost;
use crate::tests::rng::Rng;

const UNIX: Option<&str> = Some("unix");

/// A running host whose event store holds `events`.
fn host_with(events: &VecDeque<RuntimeEvent>) -> FakeHost {
    let host = FakeHost::running();
    host.events.borrow_mut().clone_from(events);
    host
}

/// The refusal that replaces the reply `whole` on a link of `bound` bytes: its id and audit
/// record id, and `reply longer than <bound> bytes`.
fn refusal(whole: &str, bound: usize) -> String {
    let value: serde_json::Value = serde_json::from_str(whole).expect("every reply is JSON");
    let audit_id = value
        .get("audit_id")
        .map_or_else(String::new, |audit_id| format!(r#","audit_id":{audit_id}"#));
    format!(
        r#"{{"id":{},"ok":false,"error":"reply longer than {bound} bytes"{audit_id}}}"#,
        value["id"]
    )
}

/// Answers `line` through the unbounded pipeline and through the pipeline for a link of `bound`
/// bytes, each on its own host from `host`, and checks that the bounded reply is the unbounded
/// one as such a link delivers it, with the same audit records. Returns the bounded reply.
fn assert_bounded_matches(line: &str, bound: usize, host: impl Fn() -> FakeHost) -> String {
    let whole_host = host();
    let whole = prepare_line(line, &whole_host, UNIX);
    let expected = if whole.response.len() <= bound {
        whole.response.clone()
    } else {
        refusal(&whole.response, bound)
    };
    let bounded_host = host();
    let bounded = prepare_line_within(line, &bounded_host, UNIX, bound);
    assert_eq!(bounded.response, expected, "line={line} bound={bound}");
    assert_eq!(bounded.after_response, whole.after_response);
    assert_eq!(
        format!("{:?}", bounded_host.audits()),
        format!("{:?}", whole_host.audits()),
        "line={line} bound={bound}: the same audit records"
    );
    bounded.response
}

/// Text with the characters JSON escapes, and others it does not.
fn text(rng: &mut Rng) -> String {
    let pieces = [
        "fault",
        "division by zero",
        "\"quoted\"",
        "back\\slash",
        "line\nbreak",
        "tab\t",
        "\u{1}",
        "ünïcødé ✓",
        "%QX0.0",
        " ",
    ];
    (0..rng.between(0, 6)).map(|_| *rng.pick(&pieces)).collect()
}

fn time(rng: &mut Rng) -> Duration {
    match rng.below(4) {
        0 => Duration::from_nanos(i64::MIN),
        1 => Duration::from_nanos(-5),
        2 => Duration::from_millis(i64::try_from(rng.below(1_000_000)).expect("a small number")),
        _ => Duration::from_nanos(i64::MAX),
    }
}

/// An event of any kind with random contents; faults come twice as often, so `faults` has some.
fn event(rng: &mut Rng) -> RuntimeEvent {
    let name = SmolStr::new(text(rng));
    let priority = u32::try_from(rng.below(300)).expect("a small number");
    match rng.below(13) {
        0 => RuntimeEvent::CycleStart {
            cycle: rng.next_u64(),
            time: time(rng),
        },
        1 => RuntimeEvent::CycleEnd {
            cycle: rng.next_u64(),
            time: time(rng),
        },
        2 => RuntimeEvent::TaskStart {
            name,
            priority,
            time: time(rng),
        },
        3 => RuntimeEvent::TaskEnd {
            name,
            priority,
            time: time(rng),
        },
        4 => RuntimeEvent::TaskOverrun {
            name,
            missed: rng.next_u64(),
            time: time(rng),
        },
        5 => RuntimeEvent::WriteRejected {
            target: name,
            error: text(rng),
            time: time(rng),
        },
        6 | 7 => RuntimeEvent::fault(
            &RuntimeError::SimulationFault(SmolStr::new(text(rng))),
            time(rng),
        ),
        8 => RuntimeEvent::SafeStateFailed {
            root: text(rng),
            error: text(rng),
            time: time(rng),
        },
        9 => RuntimeEvent::RetainOrphanDropped {
            name,
            time: time(rng),
        },
        10 => RuntimeEvent::RetainMigrationApplied {
            name,
            detail: text(rng),
            time: time(rng),
        },
        11 => RuntimeEvent::AuditDropped {
            request_id: rng.next_u64(),
            request_type: name,
            error: text(rng),
            time: rng.one_in(2).then(|| time(rng)),
        },
        _ => RuntimeEvent::FeatureDisabled {
            feature: name,
            request_type: rng.one_in(2).then(|| SmolStr::new(text(rng))),
            time: rng.one_in(2).then(|| time(rng)),
        },
    }
}

/// A read of the event store, by any of its three names, with or without a limit.
fn read_request(rng: &mut Rng, id: u64) -> String {
    let kind = *rng.pick(&["events.tail", "events", "faults"]);
    match rng.below(4) {
        0 => format!(r#"{{"id":{id},"type":"{kind}"}}"#),
        1 => format!(r#"{{"id":{id},"type":"{kind}","params":{{}}}}"#),
        _ => format!(
            r#"{{"id":{id},"type":"{kind}","params":{{"limit":{}}}}}"#,
            rng.between(1, 1_000)
        ),
    }
}

#[test]
fn bounded_event_reads_are_the_unbounded_reply_when_it_fits_and_the_refusal_when_not() {
    let mut rng = Rng::new(0x5EED_0002);
    let mut fitted = 0;
    let mut refused = 0;
    for case in 0..40_u64 {
        let size = *rng.pick(&[0_usize, 1, 7, 60, 400, 1_000]);
        let events: VecDeque<RuntimeEvent> = (0..size).map(|_| event(&mut rng)).collect();
        let line = read_request(&mut rng, case);
        let whole = prepare_line(&line, &host_with(&events), UNIX).response;
        let len = whole.len();
        for bound in [
            len,
            len - 1,
            len + 1,
            rng.between(0, len),
            DEVICE_FRAME_LEN,
            usize::MAX,
        ] {
            let reply = assert_bounded_matches(&line, bound, || host_with(&events));
            if reply.contains("reply longer than") {
                refused += 1;
            } else {
                fitted += 1;
            }
        }
    }
    assert!(
        fitted > 0 && refused > 0,
        "both outcomes occur: {fitted} fitted, {refused} refused"
    );
}

#[test]
fn refused_event_reads_answer_as_the_unbounded_pipeline_does() {
    for line in [
        r#"{"id":1,"type":"events.tail","params":{"limit":0}}"#,
        r#"{"id":2,"type":"faults","params":{"limit":"5"}}"#,
        r#"{"id":3,"type":"events","params":{"count":5}}"#,
        r#"{"id":4,"type":"events.tail","params":{"limit":1001}}"#,
        r#"{"id":5,"type":"faults","params":[]}"#,
        r#"{"id":"six","type":"faults"}"#,
        "not json",
    ] {
        for bound in [DEVICE_FRAME_LEN, 30] {
            assert_bounded_matches(line, bound, FakeHost::running);
        }
    }
    let unreadable = || {
        let mut host = FakeHost::running();
        host.unreadable = vec!["events"];
        host
    };
    assert_eq!(
        assert_bounded_matches(r#"{"id":7,"type":"faults"}"#, DEVICE_FRAME_LEN, unreadable),
        r#"{"id":7,"ok":false,"error":"event store unavailable","audit_id":"audit-1"}"#
    );
    let token_required = || {
        let mut host = FakeHost::running();
        host.control_token = Some("secret");
        host.requires_auth = true;
        host
    };
    assert_eq!(
        assert_bounded_matches(
            r#"{"id":8,"type":"events.tail"}"#,
            DEVICE_FRAME_LEN,
            token_required
        ),
        r#"{"id":8,"ok":false,"error":"missing auth token","error_code":"missing_auth_token","audit_id":"audit-1"}"#
    );
}

#[test]
fn fifty_overruns_fit_a_chips_frame_and_a_thousand_are_refused_but_served() {
    // The store of a task that overruns at every scan of 10 ms: one overrun a scan, the events that
    // every scan makes not kept (specification 11, the event store).
    let events: VecDeque<_> = (0..1_000_u64)
        .map(|scan| RuntimeEvent::TaskOverrun {
            name: SmolStr::new("MainTask"),
            missed: 1,
            time: Duration::from_millis(i64::try_from(scan * 10).expect("a small number")),
        })
        .collect();
    let host = host_with(&events);
    let fifty = prepare_line_within(
        r#"{"id":1,"type":"events.tail"}"#,
        &host,
        UNIX,
        DEVICE_FRAME_LEN,
    );
    let reply: serde_json::Value = serde_json::from_str(&fifty.response).expect("a JSON reply");
    let returned = reply["result"]["events"]
        .as_array()
        .expect("the default 50 events fit a frame of 8,192 bytes");
    assert_eq!(returned.len(), 50);
    assert_eq!(
        returned[0],
        json!({"type": "task_overrun", "name": "MainTask", "missed": 1, "time_ns": 9_990_000_000_i64})
    );
    let thousand = prepare_line_within(
        r#"{"id":2,"type":"events.tail","params":{"limit":1000}}"#,
        &host,
        UNIX,
        DEVICE_FRAME_LEN,
    );
    assert_eq!(
        thousand.response,
        r#"{"id":2,"ok":false,"error":"reply longer than 8192 bytes","audit_id":"audit-2"}"#
    );
    let audits = host.audits();
    assert_eq!(audits.len(), 2);
    assert!(
        audits[1].ok,
        "the read was served; only its reply does not fit the link"
    );
}
