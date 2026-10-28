//! A Sparkplug B session (specification 32 §9; Eclipse Sparkplug 3.0.0) worked out byte by byte:
//! the CONNECT with its clean session and the NDEATH as its will, the NCMD subscription and the
//! NBIRTH before any NDATA, the sequence numbers, `bdSeq` from one connection to the next, a rebirth
//! that a host asks for, and the acknowledgement of a PUBLISH at quality of service 1.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use trust_runtime_core::error::RuntimeError;

use super::{connect, exchange_of, publish, take, CONNACK};
use crate::io_drivers::mqtt::MqttPoll;

/// The parameters of the node `n` of the group `g`, client identifier `edge`, with one `u16`
/// metric `s` at image byte 0 and `birth_death_seq` `bd_seq`.
fn node(bd_seq: u8) -> String {
    format!(
        concat!(
            r#"{{"broker":"127.0.0.1:1883","client_id":"edge","mappings":[],"#,
            r#""output_points":[{{"data_type":"u16","image_offset":0,"metric_name":"s","#,
            r#""topic":"legacy/s"}}],"sparkplug":{{"birth_death_seq":{},"edge_node_id":"n","#,
            r#""group_id":"g"}}}}"#
        ),
        bd_seq
    )
}

/// The `bdSeq` metric at timestamp 0 with the varint `value`: name, timestamp, `Int64` (4),
/// `long_value`.
fn bd_seq(value: &[u8]) -> Vec<u8> {
    let mut metric = vec![
        0x0A, 0x05, b'b', b'd', b'S', b'e', b'q', 0x18, 0x00, 0x20, 0x04, 0x58,
    ];
    metric.extend_from_slice(value);
    metric
}

/// A payload at timestamp 0 with `metrics`, each a field 2 of its length.
fn payload(metrics: &[Vec<u8>]) -> Vec<u8> {
    let mut payload = vec![0x08, 0x00];
    for metric in metrics {
        payload.push(0x12);
        payload.push(u8::try_from(metric.len()).expect("a short metric"));
        payload.extend_from_slice(metric);
    }
    payload
}

/// The CONNECT of client `edge` whose will is the NDEATH with the `bdSeq` varint `value`: flags
/// 0x0E, a clean session (0x02) and a will (0x04) at quality of service 1 (0x08), not retained;
/// the remaining length is 10 + 6 + 20 for the will topic + 2 + the NDEATH, which has no sequence
/// number.
fn connect_with(value: &[u8]) -> Vec<u8> {
    let death = payload(&[bd_seq(value)]);
    let mut connect = vec![
        0x10,
        u8::try_from(38 + death.len()).expect("a short CONNECT"),
        0x00,
        0x04,
        b'M',
        b'Q',
        b'T',
        b'T',
        0x04,
        0x0E,
        0x00,
        0x05,
    ];
    connect.extend_from_slice(b"\x00\x04edge\x00\x12spBv1.0/g/NDEATH/n\x00");
    connect.push(u8::try_from(death.len()).expect("a short NDEATH"));
    connect.extend_from_slice(&death);
    connect
}

/// The SUBSCRIBE to `spBv1.0/g/NCMD/n` (16 bytes) at quality of service 1: packet identifier 1,
/// remaining length 2 + 2 + 16 + 1 = 21.
fn ncmd_subscribe() -> Vec<u8> {
    let mut subscribe = vec![0x82, 0x15, 0x00, 0x01, 0x00, 0x10];
    subscribe.extend_from_slice(b"spBv1.0/g/NCMD/n");
    subscribe.push(0x01);
    subscribe
}

/// The NBIRTH with the `bdSeq` varint `value` at sequence number `seq`: `bdSeq`, `Node
/// Control/Rebirth` false, and `s` with `int_value` `value_of_s`, or `is_null` without one.
fn birth(value: &[u8], seq: u8, value_of_s: Option<u8>) -> Vec<u8> {
    let mut rebirth = vec![0x0A, 0x14];
    rebirth.extend_from_slice(b"Node Control/Rebirth");
    rebirth.extend_from_slice(&[0x18, 0x00, 0x20, 0x0B, 0x70, 0x00]);
    let mut s = vec![0x0A, 0x01, b's', 0x18, 0x00, 0x20, 0x06];
    match value_of_s {
        Some(raw) => s.extend_from_slice(&[0x50, raw]),
        None => s.extend_from_slice(&[0x38, 0x01]),
    }
    let mut body = payload(&[bd_seq(value), rebirth, s]);
    body.extend_from_slice(&[0x18, seq]);
    publish("spBv1.0/g/NBIRTH/n", &body)
}

/// The NDATA of `s` = `raw` at sequence number `seq`.
fn ndata(raw: u8, seq: u8) -> Vec<u8> {
    publish(
        "spBv1.0/g/NDATA/n",
        &[
            0x08, 0x00, 0x12, 0x09, 0x0A, 0x01, b's', 0x18, 0x00, 0x20, 0x06, 0x50, raw, 0x18, seq,
        ],
    )
}

/// An NCMD payload with `Node Control/Rebirth` = `value`: the metric's 20-byte name, `Boolean`
/// and `boolean_value`, 26 bytes.
fn rebirth_request(value: u8) -> Vec<u8> {
    let mut metric = vec![0x0A, 0x14];
    metric.extend_from_slice(b"Node Control/Rebirth");
    metric.extend_from_slice(&[0x20, 0x0B, 0x70, value]);
    let mut payload = vec![0x12, 0x1A];
    payload.extend_from_slice(&metric);
    payload
}

/// A PUBLISH from the broker at quality of service 1 (0x32) with the packet identifier `id`.
fn publish_at_least_once(topic: &str, id: u16, message: &[u8]) -> Vec<u8> {
    let remaining = 2 + topic.len() + 2 + message.len();
    let mut packet = vec![0x32, u8::try_from(remaining).expect("a short PUBLISH")];
    packet.extend_from_slice(
        &u16::try_from(topic.len())
            .expect("a short topic")
            .to_be_bytes(),
    );
    packet.extend_from_slice(topic.as_bytes());
    packet.extend_from_slice(&id.to_be_bytes());
    packet.extend_from_slice(message);
    packet
}

/// The CONNECT is a clean session (tck-id-principles-persistence-clean-session-311) with the
/// NDEATH as its will at quality of service 1, not retained, and the NDEATH holds `bdSeq` and no
/// sequence number (tck-id-message-flow-edge-node-birth-publish-will-message-qos, -will-retained,
/// tck-id-payloads-ndeath-seq). After the CONNACK the node subscribes to its NCMD topic at
/// quality of service 1 and then publishes its NBIRTH, sequence number 0, with `bdSeq` 7 and `s`
/// null, as no image has come yet (tck-id-message-flow-edge-node-ncmd-subscribe); each NDATA
/// takes the next sequence number, and an image that does not convert publishes nothing and takes
/// none. Payloads hold timestamp 0 while the clock is not set. Catches a persistent session, a
/// will at quality of service 0, a death with a sequence number, a birth before the NCMD
/// subscription, data before birth, and a gap in the sequence from a failed preflight.
#[test]
fn a_sparkplug_session_births_before_data_and_counts_its_sequence() {
    let mut exchange = exchange_of(&node(7));
    let (connect, after_connack) = connect(&mut exchange, 0);
    assert_eq!(connect, connect_with(&[0x07]));
    let mut expected = ncmd_subscribe();
    expected.extend(birth(&[0x07], 0, None));
    assert_eq!(after_connack, expected);
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

/// Each new connection takes the next `bdSeq`, in its CONNECT's will and its NBIRTH: 7, then 8
/// after the session was lost and the 500 ms backoff passed; from 255 the next is 0, 255 taking
/// the varint 0xFF 0x01 (tck-id-message-flow-edge-node-birth-publish-will-message-payload-bdSeq,
/// tck-id-payloads-nbirth-bdseq-repeat). Catches a fixed `bdSeq`, by which a host takes a new
/// session's death for the old one's, and one that runs past 255.
#[test]
fn each_connection_takes_the_next_bd_seq() {
    for (first, next) in [(7_u8, [0x08_u8].as_slice()), (255, [0x00].as_slice())] {
        let first_varint: &[u8] = if first == 255 {
            &[0xFF, 0x01]
        } else {
            &[first]
        };
        let mut exchange = exchange_of(&node(first));
        let (connect, _) = connect(&mut exchange, 0);
        assert_eq!(connect, connect_with(first_varint), "bdSeq {first}");
        exchange.closed(10, "the server closed the connection");
        assert_eq!(exchange.poll(510), MqttPoll::Open);
        exchange.opened(510);
        assert_eq!(
            take(&mut exchange, 510),
            connect_with(next),
            "after {first}"
        );
        exchange.received(511, &CONNACK);
        let mut expected = ncmd_subscribe();
        expected.extend(birth(next, 1, None));
        assert_eq!(take(&mut exchange, 511), expected, "after {first}");
    }
}

/// A host's NCMD with `Node Control/Rebirth` true births the node again within its session: the
/// NBIRTH keeps `bdSeq` 7, takes the next sequence number and carries the last image's value, and
/// the next NDATA follows it (tck-id-operational-behavior-data-commands-rebirth-action-1 to -3).
/// An NCMD at quality of service 1, packet identifier 0x1234, is acknowledged with PUBACK first;
/// an NCMD with false asks for nothing. Catches a rebirth request ignored, a rebirth with a new
/// `bdSeq`, a birth without the current values, and a delivery at quality of service 1 left
/// unacknowledged, which a broker sends again.
#[test]
fn a_rebirth_request_births_the_node_again() {
    let mut exchange = exchange_of(&node(7));
    connect(&mut exchange, 0);
    assert_eq!(exchange.write_outputs(1, &[100, 0]), Ok(()));
    assert_eq!(take(&mut exchange, 1), ndata(100, 1));
    exchange.received(2, &publish("spBv1.0/g/NCMD/n", &rebirth_request(1)));
    assert_eq!(take(&mut exchange, 2), birth(&[0x07], 2, Some(100)));
    assert_eq!(exchange.write_outputs(3, &[5, 0]), Ok(()));
    assert_eq!(take(&mut exchange, 3), ndata(5, 3));
    exchange.received(
        4,
        &publish_at_least_once("spBv1.0/g/NCMD/n", 0x1234, &rebirth_request(1)),
    );
    let mut expected = vec![0x40, 0x02, 0x12, 0x34];
    expected.extend(birth(&[0x07], 4, Some(5)));
    assert_eq!(take(&mut exchange, 4), expected);
    exchange.received(5, &publish("spBv1.0/g/NCMD/n", &rebirth_request(0)));
    assert_eq!(exchange.outgoing(), [0u8; 0]);
}

/// Once the clock is set, Sparkplug B timestamps are the milliseconds since the Unix epoch: set to
/// 1,000 at 0 ms, an NDATA at 5 ms carries 1,005, the varint 0xED 0x07, after the 21 bytes of the
/// PUBLISH's fixed header and topic. Catches the monotonic time sent as the wall time, and a clock
/// that does not advance.
#[test]
fn sparkplug_timestamps_follow_the_set_clock() {
    let mut exchange = exchange_of(&node(0));
    connect(&mut exchange, 0);
    exchange.set_clock(0, 1_000);
    assert_eq!(exchange.write_outputs(5, &[100, 0]), Ok(()));
    let ndata = take(&mut exchange, 5);
    assert_eq!(ndata[21..24], [0x08, 0xED, 0x07]);
}

/// A PUBLISH at quality of service 1, which a broker may send on any subscription, is
/// acknowledged with PUBACK of its packet identifier and read as any message: packet identifier 7
/// on `trust/io/in` with 0x12 0x34. Catches a session that ends at a delivery at quality of
/// service 1, and one that never acknowledges it.
#[test]
fn a_publish_at_qos_1_is_acknowledged_and_read() {
    let mut exchange = exchange_of(r#"{"broker":"127.0.0.1:1883"}"#);
    connect(&mut exchange, 0);
    exchange.received(1, &publish_at_least_once("trust/io/in", 7, &[0x12, 0x34]));
    assert_eq!(take(&mut exchange, 1), vec![0x40, 0x02, 0x00, 0x07]);
    let mut inputs = [0_u8; 2];
    assert_eq!(exchange.read_inputs(2, &mut inputs), Ok(()));
    assert_eq!(inputs, [0x12, 0x34]);
}
