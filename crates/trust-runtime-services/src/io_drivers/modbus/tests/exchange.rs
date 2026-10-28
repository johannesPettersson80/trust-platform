//! The scan side of the exchange (specification 11, "Modbus default-fault and deadline integrity"
//! and "Safe-state handoff confirmation"): the deadline under each `on_error`, the accepted
//! snapshot, the level/latest output handoff, the reconnect backoff, the timeouts and the health.

use alloc::format;

use trust_runtime_core::error::RuntimeError;

use super::{degraded, faulted, transport, Bench, INPUT_PENDING, OUTPUT_PENDING, SERVER};
use crate::io_drivers::modbus::ModbusPoll;
use crate::status::IoDriverHealth;

/// The read of 1 input register from address 0 in transaction `id`, unit 1: the MBAP header
/// (V1.0b §3.1.3: transaction, protocol 0, length = unit + PDU = 6, unit) and the PDU 04 0000 0001
/// (V1.1b3 §6.4).
fn read_one(id: u8) -> [u8; 12] {
    [
        0x00, id, 0x00, 0x00, 0x00, 0x06, 0x01, 0x04, 0x00, 0x00, 0x00, 0x01,
    ]
}

/// The answer to `read_one(id)` with the register `high`, `low`: length 1 + 4 = 5, PDU 04 02 and the
/// two bytes.
fn one_register(id: u8, high: u8, low: u8) -> [u8; 11] {
    [
        0x00, id, 0x00, 0x00, 0x00, 0x05, 0x01, 0x04, 0x02, high, low,
    ]
}

/// The write of the register `high`, `low` to holding register 0 in transaction `id`: PDU 10 0000
/// 0001 02 and the two bytes (V1.1b3 §6.12), so the length is 1 + 8 = 9.
fn write_one(id: u8, high: u8, low: u8) -> [u8; 15] {
    [
        0x00, id, 0x00, 0x00, 0x00, 0x09, 0x01, 0x10, 0x00, 0x00, 0x00, 0x01, 0x02, high, low,
    ]
}

/// The server's answer to `write_one(id, ..)`: the echo of address 0 and quantity 1.
fn written_one(id: u8) -> [u8; 12] {
    [
        0x00, id, 0x00, 0x00, 0x00, 0x06, 0x01, 0x10, 0x00, 0x00, 0x00, 0x01,
    ]
}

/// The positive control of the deadline contract, a default register loopback: an input image of
/// 4 bytes is 2 input registers, read as 04 0000 0002 with length 1 + 5 = 6, and the answer's
/// bytes 11 22 33 44 are the image's in wire order; the output image AA BB CC DD is written as
/// 10 0000 0002 04 AA BB CC DD with length 1 + 10 = 11 in transaction 2. Both settle before their
/// deadline, 40 ms after each call, and leave the driver ok, which was degraded with `modbus worker
/// initializing` before. Catches a quantity in bytes, swapped register bytes, a length without the
/// unit, a transaction number not counted on, and a health never ok.
#[test]
fn a_default_register_loopback_completes_within_the_deadline() {
    let mut bench = Bench::new("");
    assert_eq!(
        bench.exchange.health(),
        degraded("modbus worker initializing")
    );
    assert_eq!(bench.exchange.request_inputs(0, 4), 40);
    bench.connect(0);
    assert_eq!(
        bench.sent(1),
        [0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x01, 0x04, 0x00, 0x00, 0x00, 0x02]
    );
    // The answer may take timeout_ms = 500 from the request at 1 ms.
    assert_eq!(bench.poll(1), ModbusPoll::Wait(Some(501)));
    bench.exchange.received(
        2,
        &[
            0x00, 0x01, 0x00, 0x00, 0x00, 0x07, 0x01, 0x04, 0x04, 0x11, 0x22, 0x33, 0x44,
        ],
    );
    assert!(bench.exchange.inputs_settled());
    let mut image = [0; 4];
    assert_eq!(bench.exchange.read_inputs(2, &mut image), Ok(()));
    assert_eq!(image, [0x11, 0x22, 0x33, 0x44]);
    assert_eq!(
        bench.exchange.write_outputs(3, &[0xAA, 0xBB, 0xCC, 0xDD]),
        43
    );
    assert_eq!(
        bench.sent(3),
        [
            0x00, 0x02, 0x00, 0x00, 0x00, 0x0B, 0x01, 0x10, 0x00, 0x00, 0x00, 0x02, 0x04, 0xAA,
            0xBB, 0xCC, 0xDD,
        ]
    );
    bench.exchange.received(
        4,
        &[
            0x00, 0x02, 0x00, 0x00, 0x00, 0x06, 0x01, 0x10, 0x00, 0x00, 0x00, 0x02,
        ],
    );
    assert!(bench.exchange.outputs_settled());
    assert_eq!(bench.exchange.output_result(4), Ok(()));
    assert_eq!(bench.exchange.health(), IoDriverHealth::Ok);
    assert_eq!(bench.exchange.output_health(), IoDriverHealth::Ok);
    assert_eq!(bench.poll(4), ModbusPoll::Wait(None));
}

/// "Cold-start coverage must exercise all three policies before any first snapshot exists and must
/// leave caller bytes unchanged": the read is on the wire and unanswered at the deadline, so under
/// `fault` the scan gets `IoTransport` `modbus input refresh pending` and the driver is faulted,
/// under `warn` and `ignore` the scan runs on and the driver is degraded, and under all three the
/// image keeps AA BB. Catches invented input bytes, a pending refresh that faults under `warn`, and
/// one that does not fault under `fault`.
#[test]
fn cold_start_follows_each_policy_and_invents_no_input_bytes() {
    for (policy, fault) in [("fault", true), ("warn", false), ("ignore", false)] {
        let mut bench = Bench::new(&format!(r#""on_error":"{policy}""#));
        assert_eq!(bench.exchange.request_inputs(0, 2), 40);
        bench.connect(0);
        assert_eq!(bench.sent(1), read_one(1));
        assert!(!bench.exchange.inputs_settled());
        let mut image = [0xAA, 0xBB];
        let result = bench.exchange.read_inputs(40, &mut image);
        assert_eq!(image, [0xAA, 0xBB], "{policy}");
        if fault {
            assert_eq!(result, transport(INPUT_PENDING));
            assert_eq!(bench.exchange.health(), faulted(INPUT_PENDING));
        } else {
            assert_eq!(result, Ok(()), "{policy}");
            assert_eq!(bench.exchange.health(), degraded(INPUT_PENDING), "{policy}");
        }
    }
}

/// After a refresh that gave 11 22, the next stays unanswered at its deadline: under `fault` the
/// image keeps AA BB "even when an older accepted snapshot is cached", and under `warn` the
/// accepted snapshot 11 22 is copied. Catches stale data handed to a faulting scan, and a running
/// scan under `warn` left without its last good inputs.
#[test]
fn a_pending_refresh_copies_the_accepted_snapshot_under_warn_only() {
    for policy in ["fault", "warn"] {
        let mut bench = Bench::new(&format!(r#""on_error":"{policy}""#));
        assert_eq!(bench.exchange.request_inputs(0, 2), 40);
        bench.connect(0);
        assert_eq!(bench.sent(1), read_one(1));
        bench.exchange.received(2, &one_register(1, 0x11, 0x22));
        let mut image = [0; 2];
        assert_eq!(bench.exchange.read_inputs(2, &mut image), Ok(()));
        assert_eq!(image, [0x11, 0x22]);
        assert_eq!(bench.exchange.request_inputs(10, 2), 50);
        assert_eq!(bench.sent(10), read_one(2));
        let mut image = [0xAA, 0xBB];
        let result = bench.exchange.read_inputs(50, &mut image);
        if policy == "fault" {
            assert_eq!(result, transport(INPUT_PENDING));
            assert_eq!(image, [0xAA, 0xBB]);
        } else {
            assert_eq!(result, Ok(()));
            assert_eq!(image, [0x11, 0x22]);
        }
    }
}

/// "A timed-out output was already accepted into the asynchronous level/latest handoff. Deadline
/// reporting does not cancel that pending or in-flight output; a later worker result governs its
/// eventual transport outcome": at its deadline the write of 12 34 is pending, faulted under
/// `fault` and degraded under `warn`; it stays on the wire, and the server's answer at 60 ms makes
/// the output path ok. Catches a pending write cancelled or sent again, and a late answer that
/// leaves the path faulted.
#[test]
fn an_output_handoff_pending_at_its_deadline_is_not_cancelled() {
    for (policy, fault) in [("fault", true), ("warn", false)] {
        let mut bench = Bench::new(&format!(r#""on_error":"{policy}""#));
        assert_eq!(bench.exchange.write_outputs(0, &[0x12, 0x34]), 40);
        bench.connect(0);
        assert_eq!(bench.sent(1), write_one(1, 0x12, 0x34));
        let result = bench.exchange.output_result(40);
        if fault {
            assert_eq!(result, transport(OUTPUT_PENDING));
            assert_eq!(bench.exchange.output_health(), faulted(OUTPUT_PENDING));
        } else {
            assert_eq!(result, Ok(()));
            assert_eq!(bench.exchange.output_health(), degraded(OUTPUT_PENDING));
        }
        assert_eq!(bench.poll(41), ModbusPoll::Wait(Some(501)), "{policy}");
        bench.exchange.received(60, &written_one(1));
        assert!(bench.exchange.outputs_settled());
        assert_eq!(bench.exchange.output_health(), IoDriverHealth::Ok);
    }
}

/// "Output handoff is level/latest-value semantics for `%Q`, not an edge or pulse delivery
/// guarantee": while the write of 00 01 is on the wire, the handoffs of 00 02 and 00 03 return at
/// once, their deadline being their own time, as the scan does not wait behind the server; when
/// the server answers, the next write carries 00 03, the latest, and 00 02 is never sent. Catches
/// every handoff queued and sent in turn, and a scan that waits behind a slow server.
#[test]
fn a_handoff_behind_one_on_the_wire_does_not_wait_and_the_latest_wins() {
    let mut bench = Bench::new(r#""on_error":"warn""#);
    assert_eq!(bench.exchange.write_outputs(0, &[0x00, 0x01]), 40);
    bench.connect(0);
    assert_eq!(bench.sent(1), write_one(1, 0x00, 0x01));
    assert_eq!(bench.exchange.write_outputs(10, &[0x00, 0x02]), 10);
    assert_eq!(bench.exchange.write_outputs(20, &[0x00, 0x03]), 20);
    assert_eq!(bench.exchange.output_result(20), Ok(()));
    assert_eq!(bench.exchange.output_health(), degraded(OUTPUT_PENDING));
    bench.exchange.received(30, &written_one(1));
    assert!(!bench.exchange.outputs_settled());
    assert_eq!(bench.sent(30), write_one(2, 0x00, 0x03));
    bench.exchange.received(31, &written_one(2));
    assert!(bench.exchange.outputs_settled());
    assert_eq!(bench.exchange.output_result(31), Ok(()));
    assert_eq!(bench.exchange.output_health(), IoDriverHealth::Ok);
    assert_eq!(bench.poll(31), ModbusPoll::Wait(None));
}

/// "A driver's safe-state attempt is confirmed only when `write_outputs` returns successfully and
/// the driver's immediately observed output-path health is `Ok`", and "input-path conditions
/// degrade or fault the reported driver health but not the output path": a written handoff makes
/// the output path ok; an input refresh refused with exception 2 at input register 7 faults the
/// driver, as the typed `IoAddress` error, and leaves the output path ok and the connection open;
/// a handoff still on the wire at its deadline leaves the output path faulted, unconfirmed.
/// Catches a safe state confirmed by a pending write, a safe state unconfirmed by an input fault,
/// an exception rewritten to a transport error, and a connection dropped after a valid exception.
#[test]
fn only_a_written_handoff_confirms_the_output_path() {
    let mut bench = Bench::new(r#""input_start":7"#);
    assert_eq!(bench.exchange.write_outputs(0, &[0x12, 0x34]), 40);
    bench.connect(0);
    assert_eq!(bench.sent(1), write_one(1, 0x12, 0x34));
    bench.exchange.received(2, &written_one(1));
    assert_eq!(bench.exchange.output_result(2), Ok(()));
    assert_eq!(bench.exchange.output_health(), IoDriverHealth::Ok);
    // 04 0007 0001, and the exception response 84 02 (V1.1b3 §7): length 1 + 2 = 3.
    assert_eq!(bench.exchange.request_inputs(3, 2), 43);
    assert_eq!(
        bench.sent(3),
        [0x00, 0x02, 0x00, 0x00, 0x00, 0x06, 0x01, 0x04, 0x00, 0x07, 0x00, 0x01]
    );
    bench
        .exchange
        .received(4, &[0x00, 0x02, 0x00, 0x00, 0x00, 0x03, 0x01, 0x84, 0x02]);
    let exception =
        "modbus exception code 2 (ILLEGAL DATA ADDRESS): 0x04 Read Input Registers at address 7";
    assert_eq!(
        bench.exchange.read_inputs(4, &mut [0; 2]),
        Err(RuntimeError::IoAddress(exception.into()))
    );
    assert_eq!(bench.exchange.health(), faulted(exception));
    assert_eq!(bench.exchange.output_health(), IoDriverHealth::Ok);
    // The connection stays: the next handoff is sent at once, without a connect or a close.
    assert_eq!(bench.exchange.write_outputs(5, &[0x00, 0x00]), 45);
    assert_eq!(bench.sent(5), write_one(3, 0x00, 0x00));
    assert_eq!(bench.exchange.output_result(45), transport(OUTPUT_PENDING));
    assert_eq!(bench.exchange.output_health(), faulted(OUTPUT_PENDING));
}

/// "Reconnect uses a non-zero bounded backoff and cannot spin on an unavailable endpoint": a
/// connection refused at 5 ms fails the refresh with the computer's text,
/// `modbus tcp connect <address>: <reason>`; the next connection is asked for at 105 ms, 100 ms
/// later, not before, and
/// a scan at 10 ms does not wait for it (its deadline is its own time). Catches a connection asked
/// for again at once, a backoff that never ends, and a scan held for a deadline that cannot be met.
#[test]
fn a_transport_failure_waits_the_backoff_before_it_connects_again() {
    let mut bench = Bench::new("");
    assert_eq!(bench.exchange.request_inputs(0, 2), 40);
    assert_eq!(bench.poll(0), ModbusPoll::Connect(SERVER));
    bench
        .exchange
        .failed(5, "Connection refused (os error 111)");
    assert!(bench.exchange.inputs_settled());
    assert_eq!(
        bench.exchange.read_inputs(5, &mut [0; 2]),
        transport("modbus tcp connect 10.0.0.5:502: Connection refused (os error 111)")
    );
    assert_eq!(bench.poll(5), ModbusPoll::Close);
    assert_eq!(bench.exchange.request_inputs(10, 2), 10);
    assert_eq!(bench.poll(10), ModbusPoll::Wait(Some(105)));
    assert_eq!(bench.poll(104), ModbusPoll::Wait(Some(105)));
    assert_eq!(
        bench.exchange.read_inputs(104, &mut [0; 2]),
        transport(INPUT_PENDING)
    );
    assert_eq!(bench.poll(105), ModbusPoll::Connect(SERVER));
}

/// `timeout_ms` bounds the connection and each response: with 500 ms, a request sent at 1 ms is
/// still awaited at 500 ms and fails at 501 ms with `modbus tcp <address>: no response within 500
/// ms`; the exchange asks to close the connection, and an answer after that is not taken. With
/// 50 ms, a connection asked for at 0 ms fails at 50 ms. Catches a timeout a millisecond early or
/// late, a connection kept after a lost answer, and a late answer taken as the next one's.
#[test]
fn a_connection_or_a_response_past_timeout_ms_fails_and_closes() {
    let mut bench = Bench::new(r#""on_error":"warn""#);
    assert_eq!(bench.exchange.request_inputs(0, 2), 40);
    bench.connect(0);
    assert_eq!(bench.sent(1), read_one(1));
    assert_eq!(bench.poll(500), ModbusPoll::Wait(Some(501)));
    assert_eq!(bench.poll(501), ModbusPoll::Close);
    bench.exchange.received(502, &one_register(1, 0x11, 0x22));
    let mut image = [0xAA, 0xBB];
    assert_eq!(bench.exchange.read_inputs(502, &mut image), Ok(()));
    assert_eq!(image, [0xAA, 0xBB]);
    assert_eq!(
        bench.exchange.health(),
        degraded("modbus tcp 10.0.0.5:502: no response within 500 ms")
    );
    let mut bench = Bench::new(r#""timeout_ms":50"#);
    assert_eq!(bench.exchange.request_inputs(0, 2), 40);
    assert_eq!(bench.poll(0), ModbusPoll::Connect(SERVER));
    assert_eq!(bench.poll(49), ModbusPoll::Wait(Some(50)));
    assert_eq!(bench.poll(50), ModbusPoll::Close);
    assert_eq!(
        bench.exchange.read_inputs(50, &mut [0; 2]),
        transport("modbus tcp connect 10.0.0.5:502: no connection within 50 ms")
    );
}

/// Empty images exchange nothing: a quantity of 0 is not Modbus (V1.1b3 §6), so an empty raw input
/// and output image settle at once, with an ok health and no connection. Catches a connection or a
/// request of quantity 0 for a program without Modbus bytes.
#[test]
fn empty_images_exchange_nothing() {
    let mut bench = Bench::new("");
    assert_eq!(bench.exchange.request_inputs(0, 0), 0);
    assert!(bench.exchange.inputs_settled());
    assert_eq!(bench.exchange.read_inputs(0, &mut []), Ok(()));
    assert_eq!(bench.exchange.write_outputs(0, &[]), 0);
    assert_eq!(bench.exchange.output_result(0), Ok(()));
    assert_eq!(bench.poll(0), ModbusPoll::Wait(None));
    assert_eq!(bench.exchange.health(), IoDriverHealth::Ok);
}

/// One request is on the wire at a time, and when an input refresh and an output handoff both
/// wait, the side that did not go last goes first, the output side first at the start as in the
/// computer's worker. A handoff waits its 40 ms while a read of the other side is on the wire, as
/// only an earlier handoff of its own makes it return at once. Catches two requests on the wire at
/// once, and a side that starves the other.
#[test]
fn inputs_and_outputs_take_turns() {
    let mut bench = Bench::new("");
    assert_eq!(bench.exchange.request_inputs(0, 2), 40);
    assert_eq!(bench.exchange.write_outputs(0, &[0x00, 0x07]), 40);
    bench.connect(0);
    assert_eq!(bench.sent(1), write_one(1, 0x00, 0x07));
    assert_eq!(bench.poll(1), ModbusPoll::Wait(Some(501)));
    bench.exchange.received(2, &written_one(1));
    assert_eq!(bench.sent(2), read_one(2));
    assert_eq!(bench.exchange.write_outputs(3, &[0x00, 0x08]), 43);
    assert_eq!(bench.exchange.request_inputs(3, 2), 43);
    bench.exchange.received(4, &one_register(2, 0x00, 0x01));
    assert_eq!(bench.sent(4), write_one(3, 0x00, 0x08));
    bench.exchange.received(5, &written_one(3));
    assert_eq!(bench.sent(5), read_one(4));
}

/// Specification 42, §4 item 9: the exchange gives the code of the exception response that the path
/// its health reports failed with. A read of 1 input register answered with the exception 84 02
/// (V1.1b3 §7, ILLEGAL DATA ADDRESS) gives 2; the next read, answered with the register, gives none
/// again. A connection refused, a transport failure, gives none. Catches a code read from the
/// health's words, a code that outlives the failure it belongs to, and a transport failure taken
/// for an exception response.
#[test]
fn the_exception_code_follows_the_reported_failure() {
    let mut bench = Bench::new("");
    assert_eq!(bench.exchange.exception_code(), None);
    assert_eq!(bench.exchange.request_inputs(0, 2), 40);
    bench.connect(0);
    assert_eq!(bench.sent(1), read_one(1));
    // The exception response: length 1 + 2 = 3, PDU 84 02.
    bench
        .exchange
        .received(2, &[0x00, 0x01, 0x00, 0x00, 0x00, 0x03, 0x01, 0x84, 0x02]);
    assert!(bench.exchange.read_inputs(2, &mut [0; 2]).is_err());
    assert_eq!(bench.exchange.exception_code(), Some(2));
    assert_eq!(bench.exchange.request_inputs(3, 2), 43);
    assert_eq!(bench.sent(3), read_one(2));
    bench.exchange.received(4, &one_register(2, 0x12, 0x34));
    let mut image = [0; 2];
    assert_eq!(bench.exchange.read_inputs(4, &mut image), Ok(()));
    assert_eq!(image, [0x12, 0x34]);
    assert_eq!(bench.exchange.exception_code(), None);

    let mut refused = Bench::new("");
    assert_eq!(refused.exchange.request_inputs(0, 2), 40);
    assert_eq!(refused.poll(0), ModbusPoll::Connect(SERVER));
    refused
        .exchange
        .failed(5, "Connection refused (os error 111)");
    assert!(refused.exchange.read_inputs(5, &mut [0; 2]).is_err());
    assert_eq!(refused.exchange.exception_code(), None);
}
