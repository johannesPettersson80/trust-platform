//! The exchange of the process images through a running session (specification 32 §4, §7 and §8;
//! specification 11, deadline integrity and safe-state handoff confirmation; specification 34
//! §11.1).

use alloc::vec;

use trust_runtime_core::error::RuntimeError;

use super::{connect, degraded, deliver, exchange_of, faulted, publish, take};
use crate::io_drivers::mqtt::codec::encode_subscribe;
use crate::io_drivers::mqtt::MqttPoll;
use crate::status::IoDriverHealth;

/// Two typed inputs: a `u16` count at byte 0 and an F32 temperature at byte 2, both as text.
const COUNT_AND_TEMPERATURE: &str = concat!(
    r#"{"broker":"127.0.0.1:1883","input_points":["#,
    r#"{"data_type":"u16","image_offset":0,"topic":"line/in/count"},"#,
    r#"{"data_type":"f32","image_offset":2,"topic":"line/in/temp"}]}"#
);

/// The error of a read that finds no message since the last one.
fn not_fresh() -> RuntimeError {
    RuntimeError::IoFreshness("mqtt input not fresh".into())
}

/// The session sends CONNECT with protocol level 4, a clean session, the keep alive of 5 s and the
/// client identifier, and after the CONNACK one SUBSCRIBE at quality of service 0 to the raw input
/// topic `trust/io/in` (MQTT 3.1.1 §3.1 and §3.8; specification 32 §4): CONNECT's remaining length
/// is 10 + 2 + 15 = 27 and SUBSCRIBE's 2 + 2 + 11 + 1 = 16. Catches a subscription sent before the
/// CONNACK, the wrong default topic, and a CONNECT that differs from the computer's.
#[test]
fn a_session_sends_connect_and_subscribes_to_the_raw_input_after_connack() {
    let mut exchange = exchange_of(r#"{"broker":"127.0.0.1:1883"}"#);
    let (connect, subscribe) = connect(&mut exchange, 0);
    let mut expected = vec![
        0x10, 27, 0x00, 0x04, b'M', b'Q', b'T', b'T', 0x04, 0x02, 0x00, 0x05, 0x00, 15,
    ];
    expected.extend_from_slice(b"trust-runtime-7");
    assert_eq!(connect, expected);
    let mut subscription = vec![0x82, 16, 0x00, 0x01, 0x00, 11];
    subscription.extend_from_slice(b"trust/io/in\x00");
    assert_eq!(subscribe, subscription);
}

/// The client sends an empty password as trust-runtime's client does: not at all, with only the
/// user name flag 0x80 (MQTT 3.1.1 §3.1.2.8 and §3.1.2.9). Catches an empty password field sent,
/// which some brokers check as a password.
#[test]
fn an_empty_password_is_not_sent() {
    let mut exchange = exchange_of(r#"{"broker":"127.0.0.1:1883","password":"","username":"plc"}"#);
    let (connect, _) = connect(&mut exchange, 0);
    let mut expected = vec![
        0x10, 32, 0x00, 0x04, b'M', b'Q', b'T', b'T', 0x04, 0x82, 0x00, 0x05, 0x00, 15,
    ];
    expected.extend_from_slice(b"trust-runtime-7\x00\x03plc");
    assert_eq!(connect, expected);
}

/// In raw mode a read copies the newest payload of the batch, zero past its end, and a write
/// publishes the whole output image to `topic_out` (specification 32 §4 and §8). Catches the
/// oldest payload of a batch taken, stale bytes left past a short payload, and an image published
/// to another topic.
#[test]
fn raw_mode_reads_the_newest_payload_and_publishes_the_image() {
    let mut exchange =
        exchange_of(r#"{"broker":"127.0.0.1:1883","topic_in":"line/in","topic_out":"line/out"}"#);
    connect(&mut exchange, 0);
    deliver(
        &mut exchange,
        1,
        &[("line/in", &[1, 2, 3]), ("line/in", &[7, 8])],
    );
    let mut inputs = [0xAA; 4];
    assert_eq!(exchange.read_inputs(2, &mut inputs), Ok(()));
    assert_eq!(inputs, [7, 8, 0, 0]);
    assert_eq!(exchange.health(), IoDriverHealth::Ok);
    assert_eq!(exchange.write_outputs(3, &[9, 8, 7]), Ok(()));
    assert_eq!(take(&mut exchange, 3), publish("line/out", &[9, 8, 7]));
    assert_eq!(exchange.output_health(), IoDriverHealth::Ok);
}

/// Under `on_error = "fault"` a read without a new message fails with `mqtt input not fresh` and
/// leaves the image as it was; the condition is the input path's, so the output path stays `Ok`,
/// no backoff starts, and the next write publishes at once; a fresh read clears it
/// (specification 32 §8). Catches a stale read that exposes old bytes, that unconfirms a safe
/// state, or that holds the outputs back.
#[test]
fn a_stale_read_under_fault_fails_without_touching_the_image_or_the_output_path() {
    let mut exchange =
        exchange_of(r#"{"broker":"127.0.0.1:1883","topic_in":"line/in","topic_out":"line/out"}"#);
    connect(&mut exchange, 0);
    deliver(&mut exchange, 1, &[("line/in", &[7])]);
    let mut inputs = [0_u8; 2];
    assert_eq!(exchange.read_inputs(2, &mut inputs), Ok(()));
    assert_eq!(inputs, [7, 0]);
    let mut stale = [0x55; 2];
    assert_eq!(exchange.read_inputs(3, &mut stale), Err(not_fresh()));
    assert_eq!(stale, [0x55; 2]);
    assert_eq!(exchange.health(), faulted("mqtt input not fresh"));
    assert_eq!(exchange.output_health(), IoDriverHealth::Ok);
    assert_eq!(exchange.write_outputs(4, &[2]), Ok(()));
    assert_eq!(take(&mut exchange, 4), publish("line/out", &[2]));
    assert_eq!(exchange.poll(5), MqttPoll::Wait);
    assert_eq!(exchange.health(), faulted("mqtt input not fresh"));
    deliver(&mut exchange, 6, &[("line/in", &[8])]);
    assert_eq!(exchange.read_inputs(7, &mut inputs), Ok(()));
    assert_eq!(inputs, [8, 0]);
    assert_eq!(exchange.health(), IoDriverHealth::Ok);
}

/// Under `on_error = "warn"` a read without a new message succeeds with the last accepted snapshot
/// and degrades the health with `mqtt input not fresh`; before any snapshot it leaves the image and
/// says `mqtt input snapshot unavailable` (specification 32 §8). Catches a warned read that
/// faults, invents input bytes, or drops the snapshot.
#[test]
fn a_stale_read_under_warn_keeps_the_last_snapshot() {
    let mut exchange =
        exchange_of(r#"{"broker":"127.0.0.1:1883","on_error":"warn","topic_in":"line/in"}"#);
    connect(&mut exchange, 0);
    let mut inputs = [0x55; 2];
    assert_eq!(exchange.read_inputs(1, &mut inputs), Ok(()));
    assert_eq!(inputs, [0x55; 2]);
    assert_eq!(
        exchange.health(),
        degraded("mqtt input snapshot unavailable")
    );
    deliver(&mut exchange, 2, &[("line/in", &[7])]);
    assert_eq!(exchange.read_inputs(3, &mut inputs), Ok(()));
    assert_eq!(inputs, [7, 0]);
    assert_eq!(exchange.health(), IoDriverHealth::Ok);
    let mut stale = [0x55; 2];
    assert_eq!(exchange.read_inputs(4, &mut stale), Ok(()));
    assert_eq!(stale, [7, 0]);
    assert_eq!(exchange.health(), degraded("mqtt input not fresh"));
}

/// A batch of typed inputs with one value that does not convert changes neither the image nor the
/// snapshot (specification 32 §7): after a count of 7 and 21.5 °C (`0x41AC_0000`), a batch of 9 and
/// `NaN` is refused, and a later batch of 22.5 °C (`0x41B4_0000`) alone leaves the count at 7, not
/// the refused 9. Each point subscribes in order, with packet identifiers 1 and 2. Catches a
/// partially converted batch committed, which would show a count that never came with a valid
/// temperature.
#[test]
fn a_typed_batch_with_one_bad_value_changes_nothing() {
    let mut exchange = exchange_of(COUNT_AND_TEMPERATURE);
    let (_, subscribe) = connect(&mut exchange, 0);
    let mut expected = encode_subscribe(1, "line/in/count").expect("a SUBSCRIBE");
    expected.extend(encode_subscribe(2, "line/in/temp").expect("a SUBSCRIBE"));
    assert_eq!(subscribe, expected);
    let mut image = [0_u8; 6];
    deliver(
        &mut exchange,
        1,
        &[("line/in/count", b"7"), ("line/in/temp", b"21.5")],
    );
    assert_eq!(exchange.read_inputs(2, &mut image), Ok(()));
    assert_eq!(image, [7, 0, 0x00, 0x00, 0xAC, 0x41]);
    deliver(
        &mut exchange,
        3,
        &[("line/in/count", b"9"), ("line/in/temp", b"NaN")],
    );
    assert_eq!(
        exchange.read_inputs(4, &mut image),
        Err(RuntimeError::IoDriver(
            "mqtt numeric payload must be finite".into()
        ))
    );
    assert_eq!(image, [7, 0, 0x00, 0x00, 0xAC, 0x41]);
    assert_eq!(
        exchange.health(),
        faulted("mqtt numeric payload must be finite")
    );
    deliver(&mut exchange, 5, &[("line/in/temp", b"22.5")]);
    assert_eq!(exchange.read_inputs(6, &mut image), Ok(()));
    assert_eq!(image, [7, 0, 0x00, 0x00, 0xB4, 0x41]);
    assert_eq!(exchange.health(), IoDriverHealth::Ok);
}

/// Typed inputs write their own storage only: a bool sets or clears bit 2 of byte 0 and a `u16`
/// takes bytes 1 and 2, while bits 0, 1 and 3 to 7 of byte 0 and byte 3, which other drivers own,
/// stay (specification 11: writes preserve every other bit in the byte). Catches the whole input
/// image overwritten by the driver's own copy of it.
#[test]
fn typed_inputs_leave_the_bytes_and_bits_of_other_drivers() {
    let mut exchange = exchange_of(concat!(
        r#"{"broker":"127.0.0.1:1883","input_points":["#,
        r#"{"data_type":"bool","image_bit":2,"image_offset":0,"topic":"line/in/ready"},"#,
        r#"{"data_type":"u16","image_offset":1,"payload_format":"binary_be","#,
        r#""topic":"line/in/count"}]}"#
    ));
    connect(&mut exchange, 0);
    let mut image = [0b1111_1011, 0xAA, 0xAA, 0xCC];
    deliver(
        &mut exchange,
        1,
        &[("line/in/ready", b"true"), ("line/in/count", &[0x12, 0x34])],
    );
    assert_eq!(exchange.read_inputs(2, &mut image), Ok(()));
    assert_eq!(image, [0xFF, 0x34, 0x12, 0xCC]);
    deliver(&mut exchange, 3, &[("line/in/ready", b"false")]);
    assert_eq!(exchange.read_inputs(4, &mut image), Ok(()));
    assert_eq!(image, [0b1111_1011, 0x34, 0x12, 0xCC]);
}

/// A handoff completes when its PUBLISH is in the outgoing bytes of a running session; while the
/// transport has not taken it all, a new image waits in place of any older one with `mqtt output
/// handoff pending`, and when the transport has taken it the newest image follows (specification
/// 11: level and latest-value handoff, a pending handoff is unconfirmed). Catches an unbounded
/// queue of images, an intermediate image published, and a pending handoff reported as done.
#[test]
fn a_handoff_waits_for_the_transport_and_publishes_only_the_newest_image() {
    let mut exchange = exchange_of(r#"{"broker":"127.0.0.1:1883","topic_out":"line/out"}"#);
    connect(&mut exchange, 0);
    let first = publish("line/out", &[1]);
    assert_eq!(exchange.write_outputs(1, &[1]), Ok(()));
    assert_eq!(exchange.outgoing(), first.as_slice());
    assert!(exchange.output_settling(1));
    let pending = Err(RuntimeError::IoTransport(
        "mqtt output handoff pending".into(),
    ));
    assert_eq!(exchange.write_outputs(2, &[2]), pending);
    assert_eq!(exchange.write_outputs(3, &[3]), pending);
    assert_eq!(
        exchange.output_health(),
        faulted("mqtt output handoff pending")
    );
    exchange.sent(4, 3);
    assert_eq!(exchange.outgoing(), &first[3..]);
    assert!(exchange.output_settling(4));
    exchange.sent(5, first.len() - 3);
    assert_eq!(exchange.outgoing(), publish("line/out", &[3]).as_slice());
    assert_eq!(exchange.output_health(), IoDriverHealth::Ok);
    take(&mut exchange, 6);
    assert!(!exchange.output_settling(6));
}

/// A typed output that does not convert publishes nothing and sets the publish condition under the
/// policy, but starts no backoff: the session stays, and the next valid image publishes at once
/// (specification 32 §8). Catches a preflight failure treated as a lost connection, and a partial
/// set of publications.
#[test]
fn a_typed_output_that_does_not_convert_publishes_nothing_and_keeps_the_session() {
    let mut exchange = exchange_of(concat!(
        r#"{"broker":"127.0.0.1:1883","output_points":[{"data_type":"u16","image_offset":4,"#,
        r#""payload_format":"binary_le","topic":"line/out"}]}"#
    ));
    connect(&mut exchange, 0);
    assert_eq!(
        exchange.write_outputs(1, &[1]),
        Err(RuntimeError::IoTransport(
            "mqtt point image range 4..6 exceeds process image size 1".into()
        ))
    );
    assert_eq!(exchange.outgoing(), [0u8; 0]);
    assert_eq!(exchange.poll(2), MqttPoll::Wait);
    assert!(exchange.is_connected());
    assert_eq!(exchange.write_outputs(3, &[1, 2, 3, 4, 5, 6]), Ok(()));
    assert_eq!(take(&mut exchange, 3), publish("line/out", &[5, 6]));
    assert_eq!(exchange.output_health(), IoDriverHealth::Ok);
}

/// The presence of `mappings` selects mapped mode (specification 32 §4): an empty list exchanges
/// nothing and never connects, and a lowered write mapping subscribes to nothing, leaves the input
/// image to other drivers, and publishes its point. Catches an implicit `trust/io/in`
/// subscription, input bytes of another driver erased, and a connection for a driver with
/// nothing to exchange.
#[test]
fn mapped_mode_exchanges_only_the_mapped_directions() {
    let mut idle = exchange_of(r#"{"broker":"127.0.0.1:1883","mappings":[]}"#);
    assert_eq!(idle.poll(0), MqttPoll::Wait);
    assert_eq!(idle.poll(60_000), MqttPoll::Wait);
    let mut image = [0xA5];
    assert_eq!(idle.read_inputs(1, &mut image), Ok(()));
    assert_eq!(idle.write_outputs(1, &[1]), Ok(()));
    assert_eq!(image, [0xA5]);
    assert_eq!(idle.outgoing(), [0u8; 0]);
    assert_eq!(idle.health(), IoDriverHealth::Ok);
    let mut traffic = exchange_of(concat!(
        r#"{"broker":"127.0.0.1:1883","mappings":[{"direction":"write","#,
        r#""tag":"MainInstance.Green","topic":"traffic/north/green"}],"output_points":[{"#,
        r#""data_type":"bool","image_bit":0,"image_offset":0,"topic":"traffic/north/green"}]}"#
    ));
    let (_, after_connack) = connect(&mut traffic, 0);
    assert!(after_connack.is_empty(), "no SUBSCRIBE: {after_connack:?}");
    assert_eq!(traffic.read_inputs(1, &mut image), Ok(()));
    assert_eq!(image, [0xA5]);
    assert_eq!(traffic.write_outputs(2, &[0b1]), Ok(()));
    assert_eq!(
        take(&mut traffic, 2),
        publish("traffic/north/green", b"true")
    );
    assert_eq!(traffic.health(), IoDriverHealth::Ok);
}

/// The driver keeps the newest 256 received messages; the 257th drops the oldest, and from the
/// first drop on the health is degraded with `incoming queue overflow: <n> messages dropped`, the
/// count since the start, while reads succeed and the output path stays `Ok` (specification 34
/// §11.1). After 257 messages one is dropped and the newest, 256 (0x01 0x00), is read; 300 more
/// drop 44, 45 in all, and the newest is 299 (0x01 0x2B). Catches an unbounded queue, the newest
/// message dropped instead of the oldest, a count that restarts, and an overflow that faults.
#[test]
fn the_incoming_queue_keeps_the_newest_256_messages_and_counts_drops() {
    let mut exchange = exchange_of(r#"{"broker":"127.0.0.1:1883","topic_in":"line/in"}"#);
    connect(&mut exchange, 0);
    for index in 0..257_u16 {
        exchange.received(1, &publish("line/in", &index.to_be_bytes()));
    }
    let mut inputs = [0_u8; 2];
    assert_eq!(exchange.read_inputs(2, &mut inputs), Ok(()));
    assert_eq!(inputs, [0x01, 0x00]);
    assert_eq!(
        exchange.health(),
        degraded("incoming queue overflow: 1 messages dropped")
    );
    assert_eq!(exchange.output_health(), IoDriverHealth::Ok);
    for index in 0..300_u16 {
        exchange.received(3, &publish("line/in", &index.to_be_bytes()));
    }
    assert_eq!(exchange.read_inputs(4, &mut inputs), Ok(()));
    assert_eq!(inputs, [0x01, 0x2B]);
    assert_eq!(
        exchange.health(),
        degraded("incoming queue overflow: 45 messages dropped")
    );
}

/// A message on a topic the driver did not subscribe to is not input, and a refused subscription
/// is named in the stale read's text. Catches another topic's payload copied into the image, and
/// a broker's refusal hidden behind a bare `not fresh`.
#[test]
fn other_topics_are_ignored_and_a_refused_subscription_is_named() {
    let mut exchange = exchange_of(r#"{"broker":"127.0.0.1:1883","topic_in":"line/in"}"#);
    connect(&mut exchange, 0);
    deliver(&mut exchange, 1, &[("line/other", &[9])]);
    let mut inputs = [0_u8; 1];
    assert_eq!(exchange.read_inputs(2, &mut inputs), Err(not_fresh()));
    exchange.received(3, &[0x90, 0x03, 0x00, 0x01, 0x80]);
    assert_eq!(
        exchange.read_inputs(4, &mut inputs),
        Err(RuntimeError::IoFreshness(
            r#"mqtt input not fresh; the broker refused the subscription to "line/in""#.into()
        ))
    );
    assert_eq!(inputs, [0]);
}
