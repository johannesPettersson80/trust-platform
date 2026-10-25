//! The Sparkplug B edge node profile through the broker stand-in (specification 32 §9; Eclipse
//! Sparkplug 3.0.0): the clean session that wills its NDEATH at quality of service 1, the NCMD
//! subscription and the NBIRTH before any NDATA, the sequence numbers, a host's rebirth request,
//! and the timestamps of the computer's wall clock, which the computer gives the shared exchange.
//! The payloads are written here by hand from the Sparkplug B `Payload` message: field 1 the
//! timestamp (key 0x08), 2 each metric (0x12) and 3 the sequence number (0x18), which an NDEATH
//! leaves out; a metric's field 1 its name (0x0A), 3 the timestamp (0x18), 4 the data type (0x20)
//! and one value field, 7 `is_null` (0x38), 10 `int_value` (0x50), 11 `long_value` (0x58) or 14
//! `boolean_value` (0x70).

use std::time::{SystemTime, UNIX_EPOCH};

use super::broker::Broker;
use super::{at, degraded, faulted, start};
use crate::error::RuntimeError;
use crate::io::IoDriver;

/// The node's NBIRTH topic.
const NBIRTH: &str = "spBv1.0/trust/NBIRTH/runtime-a";

/// The node's NDATA topic.
const NDATA: &str = "spBv1.0/trust/NDATA/runtime-a";

/// The node's NDEATH topic.
const NDEATH: &str = "spBv1.0/trust/NDEATH/runtime-a";

/// The node's NCMD topic.
const NCMD: &str = "spBv1.0/trust/NCMD/runtime-a";

/// The value field of a metric of an NBIRTH whose value is not known yet: `is_null` true.
const NULL: [u8; 2] = [0x38, 0x01];

/// The data type `UInt16` of a metric.
const UINT16: u8 = 6;

/// The data type `Int64`, of `bdSeq`.
const INT64: u8 = 4;

/// The data type `Boolean`.
const BOOLEAN: u8 = 11;

/// The node `runtime-a` of the group `trust` with `bdSeq` 7 and one metric, `drive/speed`, a `u16`
/// at byte 0 scaled by 0.5 with offset 10.
const SPEED_NODE: &str = r#"client_id = "sparkplug-test"

[sparkplug]
enabled = true
group_id = "trust"
edge_node_id = "runtime-a"
birth_death_seq = 7

[[output_points]]
topic = "legacy/out/speed"
metric_name = "drive/speed"
image_offset = 0
data_type = "u16"
payload_format = "json"
scale = 0.5
offset = 10.0
"#;

/// The milliseconds since the Unix epoch of the computer's clock.
fn utc_ms() -> u64 {
    let since = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("a clock after 1970");
    u64::try_from(since.as_millis()).expect("milliseconds that fit 64 bits")
}

/// `value` as a protobuf varint: seven bits a byte, least significant first, the top bit set on
/// every byte but the last.
fn varint(mut value: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    while value >= 0x80 {
        bytes.push(u8::try_from(value & 0x7F).expect("seven bits") | 0x80);
        value >>= 7;
    }
    bytes.push(u8::try_from(value).expect("seven bits"));
    bytes
}

/// The timestamp of the payload `payload`: the varint after its first key, 0x08.
fn timestamp_of(payload: &[u8]) -> u64 {
    assert_eq!(
        payload.first(),
        Some(&0x08),
        "a payload starts with its timestamp"
    );
    let mut value = 0_u64;
    for (index, byte) in payload.iter().skip(1).enumerate() {
        value |= u64::from(byte & 0x7F) << (7 * index);
        if byte & 0x80 == 0 {
            return value;
        }
    }
    panic!("a timestamp that does not end: {payload:?}")
}

/// A metric named `name` at `timestamp` of the data type `datatype`, with the value field `value`,
/// its key and its bytes, or none in an NBIRTH's definition.
fn metric(name: &str, timestamp: u64, datatype: u8, value: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0x0A, u8::try_from(name.len()).expect("a short name")];
    bytes.extend_from_slice(name.as_bytes());
    bytes.push(0x18);
    bytes.extend(varint(timestamp));
    bytes.extend([0x20, datatype]);
    bytes.extend_from_slice(value);
    bytes
}

/// The `bdSeq` metric at `timestamp` with the value `seq`, an `Int64` in `long_value`.
fn bd_seq(timestamp: u64, seq: u8) -> Vec<u8> {
    metric("bdSeq", timestamp, INT64, &[0x58, seq])
}

/// The `Node Control/Rebirth` metric of an NBIRTH at `timestamp`: a `Boolean`, false.
fn rebirth(timestamp: u64) -> Vec<u8> {
    metric("Node Control/Rebirth", timestamp, BOOLEAN, &[0x70, 0x00])
}

/// A payload at `timestamp` with `metrics` and, but in an NDEATH, the sequence number `seq`.
fn payload(timestamp: u64, metrics: &[Vec<u8>], seq: Option<u8>) -> Vec<u8> {
    let mut bytes = vec![0x08];
    bytes.extend(varint(timestamp));
    for metric in metrics {
        bytes.push(0x12);
        bytes.push(u8::try_from(metric.len()).expect("a short metric"));
        bytes.extend_from_slice(metric);
    }
    if let Some(seq) = seq {
        bytes.extend([0x18, seq]);
    }
    bytes
}

/// A Sparkplug session is clean (tck-id-principles-persistence-clean-session-311) and carries its
/// NDEATH as the will at quality of service 1 without retain: the CONNECT's flags are 0x0E, the
/// clean session 0x02, the will 0x04 and its quality of service 0x08; the NDEATH holds `bdSeq` 7,
/// an `Int64`, and no sequence number. The node subscribes to its NCMD topic, and its NBIRTH,
/// sequence number 0, holds `bdSeq`, `Node Control/Rebirth` false and `drive/speed`, a `UInt16`
/// whose value is null before the first image (specification 32 §9). Both carry the computer's
/// wall clock when the session opened. Catches a persistent session, a will at quality of service
/// 0 or with a sequence number, a node deaf to its commands, and a timestamp that is not the wall
/// clock.
#[test]
fn sparkplug_profile_configures_namespace_topics_and_last_will() {
    let broker = Broker::start();
    let before = utc_ms();
    let _driver = start(&at(&broker, SPEED_NODE));
    broker.wait_for("NBIRTH", |record| !record.published(NBIRTH).is_empty());
    let after = utc_ms();
    let connect = broker.record(|record| record.connects[0].clone());
    assert_eq!(connect.client_id, "sparkplug-test");
    assert!(connect.clean_session);
    assert_eq!(connect.flags, 0x0E);
    let will = connect.will.expect("the NDEATH as the will");
    assert_eq!(
        (will.topic.as_str(), will.qos, will.retain),
        (NDEATH, 1, false)
    );
    let died = timestamp_of(&will.message);
    assert!(
        (before..=after).contains(&died),
        "{died} not in {before}..={after}"
    );
    assert_eq!(will.message, payload(died, &[bd_seq(died, 7)], None));
    assert_eq!(broker.record(|record| record.subscriptions.clone()), [NCMD]);
    let birth = broker.record(|record| record.published(NBIRTH)[0].clone());
    let born = timestamp_of(&birth);
    assert!(
        (before..=after).contains(&born),
        "{born} not in {before}..={after}"
    );
    assert_eq!(
        birth,
        payload(
            born,
            &[
                bd_seq(born, 7),
                rebirth(born),
                metric("drive/speed", born, UINT16, &NULL)
            ],
            Some(0)
        )
    );
}

/// An NDATA of `drive/speed` holds the raw value (60 − 10) ÷ 0.5 = 100 as `int_value` (0x50 0x64),
/// the sequence number 1 after the NBIRTH's 0, and the wall clock's milliseconds when the scan
/// handed the image off (specification 32 §9). Catches the monotonic time sent as a timestamp, a
/// timestamp of 0, and an NDATA that does not follow its NBIRTH's sequence.
#[test]
fn sparkplug_payload_encoding_matches_tahu_wire_shape_for_scalar_metrics() {
    let broker = Broker::start();
    let mut driver = start(&at(&broker, SPEED_NODE));
    broker.wait_for("NBIRTH", |record| !record.published(NBIRTH).is_empty());
    let before = utc_ms();
    assert_eq!(driver.write_outputs(&60_u16.to_le_bytes()), Ok(()));
    let after = utc_ms();
    broker.wait_for("NDATA", |record| !record.published(NDATA).is_empty());
    let data = broker.record(|record| record.published(NDATA)[0].clone());
    let sent = timestamp_of(&data);
    assert!(
        (before..=after).contains(&sent),
        "{sent} not in {before}..={after}"
    );
    assert_eq!(
        data,
        payload(
            sent,
            &[metric("drive/speed", sent, UINT16, &[0x50, 0x64])],
            Some(1)
        )
    );
}

/// With two metrics the NBIRTH goes before the NDATA, and the NDATA holds each metric in the order
/// of the points: `drive/enabled` true (`boolean_value` 0x70 0x01) and `drive/speed` 100
/// (specification 32 §9: birth precedes data for each accepted session). Catches data published
/// before birth, and a metric left out.
#[test]
fn sparkplug_profile_publishes_birth_then_data_payload() {
    let broker = Broker::start();
    let mut driver = start(&at(
        &broker,
        r#"client_id = "sparkplug-runtime"

[sparkplug]
enabled = true
group_id = "trust"
edge_node_id = "runtime-a"
birth_death_seq = 11

[[output_points]]
topic = "legacy/out/enabled"
metric_name = "drive/enabled"
image_offset = 0
image_bit = 1
data_type = "bool"
payload_format = "json"

[[output_points]]
topic = "legacy/out/speed"
metric_name = "drive/speed"
image_offset = 1
data_type = "u16"
payload_format = "json"
scale = 0.5
offset = 10.0
"#,
    ));
    broker.wait_for("NBIRTH", |record| !record.published(NBIRTH).is_empty());
    let mut outputs = [0_u8; 3];
    outputs[0] = 0b0000_0010;
    outputs[1..3].copy_from_slice(&60_u16.to_le_bytes());
    assert_eq!(driver.write_outputs(&outputs), Ok(()));
    broker.wait_for("NDATA", |record| !record.published(NDATA).is_empty());
    broker.record(|record| {
        assert_eq!(record.topics(), [NBIRTH, NDATA]);
        let birth = &record.publishes[0].payload;
        let born = timestamp_of(birth);
        assert_eq!(
            *birth,
            payload(
                born,
                &[
                    bd_seq(born, 11),
                    rebirth(born),
                    metric("drive/enabled", born, BOOLEAN, &NULL),
                    metric("drive/speed", born, UINT16, &NULL),
                ],
                Some(0)
            )
        );
        let data = &record.publishes[1].payload;
        let sent = timestamp_of(data);
        assert_eq!(
            *data,
            payload(
                sent,
                &[
                    metric("drive/enabled", sent, BOOLEAN, &[0x70, 0x01]),
                    metric("drive/speed", sent, UINT16, &[0x50, 0x64]),
                ],
                Some(1)
            )
        );
    });
}

/// A Sparkplug output whose point lies beyond the image (a `u16` at byte 4 of a 1-byte image)
/// follows `on_error` with the conversion's text, publishes no NDATA and takes no sequence number:
/// the next valid image is NDATA 1, 60 (`int_value` 0x50 0x3C) (specification 32 §8 and §9).
/// Catches a failed conversion that publishes or that leaves a gap in the sequence.
#[test]
fn mqtt_policy_preserves_sparkplug_output_preflight_failure() {
    let text = "mqtt point image range 4..6 exceeds process image size 1";
    for (policy, fails) in [("fault", true), ("warn", false), ("ignore", false)] {
        let broker = Broker::start();
        let mut driver = start(&at(
            &broker,
            &format!(
                "client_id = \"sparkplug-preflight-{policy}\"\non_error = \"{policy}\"\n\
                 [sparkplug]\nenabled = true\ngroup_id = \"trust\"\nedge_node_id = \"runtime-a\"\n\
                 [[output_points]]\ntopic = \"legacy/out/speed\"\nmetric_name = \"drive/speed\"\n\
                 image_offset = 4\ndata_type = \"u16\"\npayload_format = \"json\"\n"
            ),
        ));
        broker.wait_for("NBIRTH", |record| !record.published(NBIRTH).is_empty());
        let result = driver.write_outputs(&[1]);
        if fails {
            assert_eq!(result, Err(RuntimeError::IoTransport(text.into())));
            assert_eq!(driver.health(), faulted(text));
        } else {
            assert_eq!(result, Ok(()), "{policy}");
            assert_eq!(driver.health(), degraded(text), "{policy}");
        }
        assert_eq!(
            driver.write_outputs(&[0, 0, 0, 0, 60, 0]),
            Ok(()),
            "{policy}"
        );
        broker.wait_for("NDATA", |record| !record.published(NDATA).is_empty());
        broker.record(|record| {
            assert_eq!(record.topics(), [NBIRTH, NDATA], "{policy}");
            let data = &record.publishes[1].payload;
            let sent = timestamp_of(data);
            assert_eq!(
                *data,
                payload(
                    sent,
                    &[metric("drive/speed", sent, UINT16, &[0x50, 0x3C])],
                    Some(1)
                ),
                "{policy}"
            );
        });
    }
}

/// Dropping the driver ends a Sparkplug session in order: the NDEATH, `bdSeq` 7 and no sequence
/// number, as the will would have published it, then DISCONNECT, after which the broker discards
/// the will (MQTT 3.1.1 §3.1.2.5; specification 32 §9;
/// tck-id-operational-behavior-edge-node-intentional-disconnect-ndeath). Catches a node that leaves
/// without its death certificate.
#[test]
fn dropping_the_driver_publishes_the_ndeath_then_disconnects() {
    let broker = Broker::start();
    let driver = start(&at(&broker, SPEED_NODE));
    broker.wait_for("NBIRTH", |record| !record.published(NBIRTH).is_empty());
    let before = utc_ms();
    drop(driver);
    broker.wait_for("DISCONNECT", |record| record.disconnects == 1);
    let after = utc_ms();
    broker.record(|record| {
        assert_eq!(record.topics(), [NBIRTH, NDEATH]);
        let death = &record.publishes[1].payload;
        let died = timestamp_of(death);
        assert!(
            (before..=after).contains(&died),
            "{died} not in {before}..={after}"
        );
        assert_eq!(*death, payload(died, &[bd_seq(died, 7)], None));
    });
}

/// A host's NCMD with `Node Control/Rebirth` true births the node again: a second NBIRTH with the
/// session's `bdSeq` 7, the next sequence number, 2 after the NDATA 1, and the last image's value
/// of `drive/speed`, 100 (tck-id-operational-behavior-data-commands-rebirth-action-2, -3). Catches a
/// computer whose node ignores a rebirth request, which a SCADA host sends after it missed a
/// birth.
#[test]
fn a_rebirth_request_births_the_node_again() {
    let broker = Broker::start();
    let mut driver = start(&at(&broker, SPEED_NODE));
    broker.wait_for("NBIRTH", |record| !record.published(NBIRTH).is_empty());
    assert_eq!(driver.write_outputs(&60_u16.to_le_bytes()), Ok(()));
    broker.wait_for("NDATA", |record| !record.published(NDATA).is_empty());
    let mut request = vec![0x12, 0x1A, 0x0A, 0x14];
    request.extend_from_slice(b"Node Control/Rebirth");
    request.extend_from_slice(&[0x20, BOOLEAN, 0x70, 0x01]);
    broker.deliver(&[(NCMD, request.as_slice())]);
    broker.wait_for("a second NBIRTH", |record| {
        record.published(NBIRTH).len() == 2
    });
    let birth = broker.record(|record| record.published(NBIRTH)[1].clone());
    let born = timestamp_of(&birth);
    assert_eq!(
        birth,
        payload(
            born,
            &[
                bd_seq(born, 7),
                rebirth(born),
                metric("drive/speed", born, UINT16, &[0x50, 0x64])
            ],
            Some(2)
        )
    );
}
