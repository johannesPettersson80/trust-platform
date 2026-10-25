//! The life of a session and the health it reports (specification 32 §8; specification 11,
//! safe-state handoff confirmation), with the numbers of trust-runtime's driver and client: a
//! backoff of `reconnect_ms`, 5 s to CONNACK, 5 s for a stalled stream, and a PINGREQ every keep
//! alive.

use alloc::format;
use alloc::vec;

use trust_runtime_core::error::RuntimeError;

use super::{connect, degraded, exchange_of, faulted, publish, take};
use crate::io_drivers::mqtt::codec::encode_subscribe;
use crate::io_drivers::mqtt::MqttPoll;
use crate::status::IoDriverHealth;

/// Before anything happened the health is degraded with `mqtt worker initializing`, as the
/// computer's; a failed connect faults it under `fault` with the transport's reason, and starts a
/// backoff of `reconnect_ms` during which reads and writes report it with its cause and no
/// connect is tried (specification 32 §8: a nonzero, bounded backoff that cannot busy-loop); a
/// threaded runtime's write does not wait during it. Catches a reconnect at every poll, a backoff
/// that never ends, a backoff that hides its cause, and a scan held for the backoff.
#[test]
fn a_failed_connect_starts_the_backoff_of_reconnect_ms() {
    let mut exchange = exchange_of(r#"{"broker":"127.0.0.1:1883","reconnect_ms":100}"#);
    assert_eq!(exchange.health(), degraded("mqtt worker initializing"));
    assert_eq!(exchange.poll(0), MqttPoll::Open);
    exchange.open_failed(0, "connection refused");
    assert_eq!(
        exchange.health(),
        faulted("mqtt connect failed: connection refused")
    );
    let backoff = "mqtt reconnect backoff active (mqtt connect failed: connection refused)";
    let mut inputs = [0x55];
    assert_eq!(
        exchange.read_inputs(50, &mut inputs),
        Err(RuntimeError::IoFreshness(backoff.into()))
    );
    assert_eq!(inputs, [0x55]);
    assert_eq!(
        exchange.write_outputs(50, &[1]),
        Err(RuntimeError::IoTransport(backoff.into()))
    );
    assert!(
        !exchange.output_settling(50),
        "a write in the backoff fails at once"
    );
    assert!(
        exchange.output_settling(100),
        "a write at the backoff's end waits for the connect"
    );
    assert_eq!(exchange.poll(99), MqttPoll::Wait);
    assert_eq!(exchange.poll(100), MqttPoll::Open);
}

/// From the poll that opens the stream to the CONNACK the session has 5 s, as trust-runtime's
/// client, however long the stream took to open; then the stream is closed and the backoff of
/// 500 ms runs. Catches a connect that waits for ever, and a timeout counted from the open
/// stream instead of from the attempt.
#[test]
fn a_session_without_connack_in_5_s_is_closed() {
    let mut exchange = exchange_of(r#"{"broker":"127.0.0.1:1883"}"#);
    assert_eq!(exchange.poll(0), MqttPoll::Open);
    exchange.opened(4_000);
    take(&mut exchange, 4_000);
    assert_eq!(exchange.poll(4_999), MqttPoll::Wait);
    assert_eq!(exchange.poll(5_000), MqttPoll::Close);
    assert_eq!(
        exchange.health(),
        faulted("mqtt connect failed: no session within 5000 ms")
    );
    assert_eq!(exchange.poll(5_499), MqttPoll::Wait);
    assert_eq!(exchange.poll(5_500), MqttPoll::Open);
}

/// A CONNACK that refuses ends the attempt with its return code in words (MQTT 3.1.1 Table 3.1),
/// degraded under `warn`, closes the stream and starts the backoff. Catches a refused session
/// taken as running, and a refusal without its reason.
#[test]
fn a_refused_connack_closes_the_stream_with_its_reason() {
    let mut exchange = exchange_of(r#"{"broker":"127.0.0.1:1883","on_error":"warn"}"#);
    assert_eq!(exchange.poll(0), MqttPoll::Open);
    exchange.opened(0);
    take(&mut exchange, 0);
    exchange.received(1, &[0x20, 0x02, 0x00, 0x04]);
    assert!(!exchange.is_connected());
    assert_eq!(exchange.poll(1), MqttPoll::Close);
    assert_eq!(
        exchange.health(),
        degraded(
            "mqtt connect failed: the broker refused the connection: bad user name or password \
             (return code 4)"
        )
    );
    assert_eq!(exchange.poll(500), MqttPoll::Wait);
    assert_eq!(exchange.poll(501), MqttPoll::Open);
}

/// A running session sends PINGREQ one keep alive after the CONNACK and every keep alive after
/// that, and is lost when the broker has not answered the last PINGREQ by the next, as
/// trust-runtime's client (MQTT 3.1.1 §3.1.2.10). Catches a broker outage that is never noticed,
/// pings that flood the broker, and a session dropped although the broker answered.
#[test]
fn the_keep_alive_pings_and_loses_a_silent_broker() {
    let mut exchange = exchange_of(r#"{"broker":"127.0.0.1:1883","keep_alive_s":5}"#);
    connect(&mut exchange, 0);
    assert_eq!(exchange.poll(4_999), MqttPoll::Wait);
    assert_eq!(exchange.outgoing(), [0u8; 0]);
    assert_eq!(exchange.poll(5_000), MqttPoll::Wait);
    assert_eq!(take(&mut exchange, 5_000), [0xC0, 0x00]);
    exchange.received(5_010, &[0xD0, 0x00]);
    assert_eq!(exchange.poll(10_000), MqttPoll::Wait);
    assert_eq!(take(&mut exchange, 10_000), [0xC0, 0x00]);
    assert_eq!(exchange.poll(14_999), MqttPoll::Wait);
    assert_eq!(exchange.poll(15_000), MqttPoll::Close);
    assert_eq!(
        exchange.health(),
        faulted("mqtt disconnected: no PINGRESP within the keep alive of 5 s")
    );
    assert!(!exchange.is_connected());
}

/// Outgoing bytes that the transport takes none of for 5 s lose the session, as trust-runtime's
/// client's flush timeout; bytes taken in part restart the 5 s. Catches a stalled connection that
/// holds the outputs for ever.
#[test]
fn a_stream_that_takes_no_bytes_for_5_s_is_lost() {
    let mut exchange = exchange_of(concat!(
        r#"{"broker":"127.0.0.1:1883","keep_alive_s":60,"#,
        r#""topic_out":"line/out"}"#
    ));
    connect(&mut exchange, 0);
    assert_eq!(exchange.write_outputs(10, &[1]), Ok(()));
    exchange.sent(3_000, 1);
    assert_eq!(exchange.poll(7_999), MqttPoll::Wait);
    assert_eq!(exchange.poll(8_000), MqttPoll::Close);
    assert_eq!(
        exchange.health(),
        faulted("mqtt disconnected: the connection took no bytes for 5000 ms")
    );
}

/// A lost session starts the backoff without a close, since the transport closed it; a write in
/// the backoff waits as the newest image; after it the session connects again, subscribes again,
/// and then publishes the image that waited (specification 32 §8). Catches a session that never
/// returns, subscriptions lost after a reconnect, and the newest output image lost.
#[test]
fn a_lost_session_reconnects_subscribes_and_publishes_the_waiting_image() {
    let mut exchange = exchange_of(r#"{"broker":"127.0.0.1:1883","topic_out":"line/out"}"#);
    connect(&mut exchange, 0);
    exchange.closed(1_000, "connection reset by peer");
    assert_eq!(
        exchange.health(),
        faulted("mqtt disconnected: connection reset by peer")
    );
    assert_eq!(exchange.poll(1_000), MqttPoll::Wait);
    assert_eq!(
        exchange.write_outputs(1_100, &[4]),
        Err(RuntimeError::IoTransport(
            "mqtt reconnect backoff active (mqtt disconnected: connection reset by peer)".into()
        ))
    );
    assert_eq!(exchange.poll(1_499), MqttPoll::Wait);
    let (_, after_connack) = connect(&mut exchange, 1_500);
    let mut expected = encode_subscribe(1, "trust/io/in").expect("a SUBSCRIBE");
    expected.extend(publish("line/out", &[4]));
    assert_eq!(after_connack, expected);
    assert_eq!(exchange.output_health(), IoDriverHealth::Ok);
}

/// A packet the client does not take ends the session with the decoder's reason, and the stream
/// is closed (MQTT 3.1.1 §4.8: a protocol violation closes the connection). Catches a malformed
/// stream read on as if it were in step.
#[test]
fn a_packet_the_client_does_not_take_ends_the_session() {
    let mut exchange = exchange_of(r#"{"broker":"127.0.0.1:1883"}"#);
    connect(&mut exchange, 0);
    exchange.received(1, &[0x40, 0x02, 0x00, 0x01]);
    assert_eq!(exchange.poll(1), MqttPoll::Close);
    assert_eq!(
        exchange.health(),
        faulted("mqtt disconnected: malformed packet from the broker: unexpected PUBACK")
    );
}

/// Before the session runs, a read and a write report `mqtt connecting`: under `fault` as errors,
/// with the image unchanged; under `warn` as success with a degraded health, and an output path
/// that is not `Ok`, so that a safe state handed off then is unconfirmed; once the session runs the
/// image that waited is published and the output path is `Ok` (specification 32 §8; specification
/// 11). Catches a write confirmed before any session, and a connecting read that invents input.
#[test]
fn before_the_session_runs_reads_and_writes_report_connecting() {
    let mut fault = exchange_of(r#"{"broker":"127.0.0.1:1883"}"#);
    let mut inputs = [0x55];
    assert_eq!(
        fault.read_inputs(0, &mut inputs),
        Err(RuntimeError::IoFreshness("mqtt connecting".into()))
    );
    assert_eq!(inputs, [0x55]);
    assert_eq!(
        fault.write_outputs(0, &[1]),
        Err(RuntimeError::IoTransport("mqtt connecting".into()))
    );
    assert_eq!(fault.health(), faulted("mqtt connecting"));
    let mut warn = exchange_of(concat!(
        r#"{"broker":"127.0.0.1:1883","on_error":"warn","#,
        r#""topic_out":"line/out"}"#
    ));
    assert_eq!(warn.read_inputs(0, &mut inputs), Ok(()));
    assert_eq!(inputs, [0x55]);
    assert_eq!(warn.write_outputs(0, &[0x5A]), Ok(()));
    assert_eq!(warn.output_health(), degraded("mqtt connecting"));
    assert_eq!(warn.health(), degraded("mqtt connecting"));
    let (_, after_connack) = connect(&mut warn, 1);
    assert!(after_connack.ends_with(&publish("line/out", &[0x5A])));
    assert_eq!(warn.output_health(), IoDriverHealth::Ok);
}

/// An orderly end sends DISCONNECT, closes the stream once the transport has taken it, and never
/// connects again; reads then report the stop. Catches a stop that keeps reconnecting, and a
/// stream closed before DISCONNECT went out.
#[test]
fn disconnect_sends_disconnect_then_closes_for_good() {
    let mut exchange = exchange_of(r#"{"broker":"127.0.0.1:1883"}"#);
    connect(&mut exchange, 0);
    exchange.disconnect(1);
    assert_eq!(exchange.outgoing(), [0xE0, 0x00]);
    assert_eq!(exchange.poll(1), MqttPoll::Wait);
    take(&mut exchange, 2);
    assert_eq!(exchange.poll(2), MqttPoll::Close);
    assert_eq!(exchange.poll(60_000), MqttPoll::Wait);
    let mut inputs = [0];
    assert_eq!(
        exchange.read_inputs(3, &mut inputs),
        Err(RuntimeError::IoFreshness(
            "mqtt disconnected: the driver stopped".into()
        ))
    );
}

/// The `Debug` of the exchange never shows the password, which its CONNECT carries (specification
/// 32 §3). Catches the outgoing bytes written to a log.
#[test]
fn the_debug_of_the_exchange_hides_the_password() {
    let mut exchange =
        exchange_of(r#"{"broker":"127.0.0.1:1883","password":"hunter2","username":"plc"}"#);
    assert_eq!(exchange.poll(0), MqttPoll::Open);
    exchange.opened(0);
    assert!(exchange
        .outgoing()
        .windows(7)
        .any(|window| window == b"hunter2"));
    let shown = format!("{exchange:?}");
    assert!(!shown.contains("hunter2"), "{shown}");
}

/// With Sparkplug B the CONNECT keeps the session (clean session 0, as trust-runtime sets it) and
/// carries the NDEATH as its will at quality of service 0; after the CONNACK the NBIRTH, sequence
/// 0, goes before any NDATA; each NDATA takes the next sequence number, and an image that does not
/// convert publishes nothing and takes none (specification 32 §9). Payloads hold timestamp 0 while
/// the clock is not set. Catches data before birth, a will that is not the NDEATH, and a gap in
/// the sequence from a failed preflight.
#[test]
fn a_sparkplug_session_births_before_data_and_counts_its_sequence() {
    let mut exchange = exchange_of(concat!(
        r#"{"broker":"127.0.0.1:1883","client_id":"edge","mappings":[],"#,
        r#""output_points":[{"data_type":"u16","image_offset":0,"metric_name":"s","#,
        r#""topic":"legacy/s"}],"sparkplug":{"birth_death_seq":7,"edge_node_id":"n","#,
        r#""group_id":"g"}}"#
    ));
    let (connect, after_connack) = connect(&mut exchange, 0);
    // bdSeq: name, timestamp 0, data type UInt64 (8), long value 7.
    let bd_seq = [
        0x0A, 0x05, b'b', b'd', b'S', b'e', b'q', 0x18, 0x00, 0x20, 0x08, 0x58, 0x07,
    ];
    let mut death = vec![0x08, 0x00, 0x12, 0x0D];
    death.extend_from_slice(&bd_seq);
    death.extend_from_slice(&[0x18, 0x00]);
    let mut expected = vec![
        0x10, 57, 0x00, 0x04, b'M', b'Q', b'T', b'T', 0x04, 0x04, 0x00, 0x05,
    ];
    expected.extend_from_slice(b"\x00\x04edge\x00\x12spBv1.0/g/NDEATH/n\x00\x13");
    expected.extend_from_slice(&death);
    assert_eq!(connect, expected);
    let mut birth = vec![0x08, 0x00, 0x12, 0x0D];
    birth.extend_from_slice(&bd_seq);
    birth.extend_from_slice(&[
        0x12, 0x07, 0x0A, 0x01, b's', 0x18, 0x00, 0x20, 0x06, 0x18, 0x00,
    ]);
    assert_eq!(after_connack, publish("spBv1.0/g/NBIRTH/n", &birth));
    let ndata = |value: u8, seq: u8| {
        publish(
            "spBv1.0/g/NDATA/n",
            &[
                0x08, 0x00, 0x12, 0x09, 0x0A, 0x01, b's', 0x18, 0x00, 0x20, 0x06, 0x50, value,
                0x18, seq,
            ],
        )
    };
    assert_eq!(exchange.write_outputs(1, &[100, 0]), Ok(()));
    assert_eq!(take(&mut exchange, 1), ndata(100, 1));
    assert_eq!(
        exchange.write_outputs(2, &[1]),
        Err(RuntimeError::IoTransport(
            "mqtt point image range 0..2 exceeds process image size 1".into()
        ))
    );
    assert_eq!(exchange.outgoing(), [0u8; 0]);
    assert_eq!(exchange.write_outputs(3, &[5, 0]), Ok(()));
    assert_eq!(take(&mut exchange, 3), ndata(5, 2));
}

/// Once the clock is set, Sparkplug B timestamps are the milliseconds since the Unix epoch: set to
/// 1,000 at 0 ms, an NDATA at 5 ms carries 1,005, the varint 0xED 0x07. Catches the monotonic time
/// sent as the wall time, and a clock that does not advance.
#[test]
fn sparkplug_timestamps_follow_the_set_clock() {
    let mut exchange = exchange_of(concat!(
        r#"{"broker":"127.0.0.1:1883","client_id":"edge","mappings":[],"#,
        r#""output_points":[{"data_type":"u16","image_offset":0,"metric_name":"s","#,
        r#""topic":"legacy/s"}],"sparkplug":{"edge_node_id":"n","group_id":"g"}}"#
    ));
    connect(&mut exchange, 0);
    exchange.set_clock(0, 1_000);
    assert_eq!(exchange.write_outputs(5, &[100, 0]), Ok(()));
    let ndata = take(&mut exchange, 5);
    assert_eq!(ndata[21..24], [0x08, 0xED, 0x07]);
}
