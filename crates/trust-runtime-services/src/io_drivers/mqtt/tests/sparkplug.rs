//! The Sparkplug B outbound profile (specification 32 §9): its topics, its `sparkplug` table and
//! the protobuf bytes of its payloads, worked out by hand from the Sparkplug B `Payload` message:
//! field 1 timestamp, 2 metric, 3 seq; a metric's field 1 name, 3 timestamp, 4 data type, 10
//! `int_value`, 11 `long_value`, 12 `float_value` and 14 `boolean_value`. A key is the field
//! number times 8 plus the wire type: 0x08 timestamp, 0x12 metric, 0x18 seq, 0x0A name, 0x18
//! metric timestamp, 0x20 data type, 0x50 int, 0x58 long, 0x65 float and 0x70 Boolean.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use smol_str::SmolStr;

use crate::io_drivers::mqtt::sparkplug::{encode_nbirth, encode_ndata, encode_ndeath};
use crate::io_drivers::mqtt::{MqttParams, OutputPoint, PayloadFormat, PointType, SparkplugParams};
use crate::io_drivers::ParamsError;

/// The node `runtime-a` of the group `trust`, with `bdSeq` 7.
fn node() -> SparkplugParams {
    SparkplugParams {
        group_id: "trust".into(),
        edge_node_id: "runtime-a".into(),
        birth_death_seq: 7,
    }
}

/// The output point and metric `name` of `data_type` at byte 0, bit 1 for a bool, unscaled.
fn metric(name: &str, data_type: PointType) -> OutputPoint {
    OutputPoint {
        topic: SmolStr::new(name),
        metric_name: SmolStr::new(name),
        image_offset: 0,
        image_bit: (data_type == PointType::Bool).then_some(1),
        data_type,
        payload_format: PayloadFormat::Text,
        scale: 1.0,
        offset: 0.0,
    }
}

/// A payload at timestamp 0 and seq 0 with the one metric `metric`.
fn one_metric(metric: &[u8]) -> Vec<u8> {
    let mut payload = vec![
        0x08,
        0x00,
        0x12,
        u8::try_from(metric.len()).expect("a short metric"),
    ];
    payload.extend_from_slice(metric);
    payload.extend_from_slice(&[0x18, 0x00]);
    payload
}

/// The parameters of a local broker with one output point and the `sparkplug` table `table`.
fn sparkplug(table: &str) -> Result<MqttParams, ParamsError> {
    let local = r#""broker":"127.0.0.1:1883","mappings":[]"#;
    let point = r#""output_points":[{"data_type":"u16","image_offset":0,"topic":"s"}]"#;
    MqttParams::from_json(&format!(r#"{{{local},{point},"sparkplug":{table}}}"#))
}

/// The refusal of `key` for `reason`.
fn refused(key: &str, reason: &str) -> Result<MqttParams, ParamsError> {
    Err(ParamsError {
        key: key.into(),
        reason: reason.into(),
    })
}

/// The topics are `spBv1.0/<group>/<message type>/<node>` (specification 32 §9). Catches the group
/// and the node swapped, and another namespace.
#[test]
fn the_topics_follow_the_namespace_the_group_and_the_node() {
    assert_eq!(node().nbirth_topic(), "spBv1.0/trust/NBIRTH/runtime-a");
    assert_eq!(node().ndata_topic(), "spBv1.0/trust/NDATA/runtime-a");
    assert_eq!(node().ndeath_topic(), "spBv1.0/trust/NDEATH/runtime-a");
}

/// An NDATA of a `u16` metric `s` scaled by 0.5 with offset 10 holds the raw value (60 − 10) ÷
/// 0.5 = 100 at timestamp 123 (0x7B) and seq 5: 0x08 0x7B, a metric of 9 bytes, 0x18 0x05.
/// Catches the engineering value published instead of the raw one, the metric's fields out of
/// order, and a seq not at the end.
#[test]
fn an_ndata_holds_the_raw_value_its_timestamp_and_its_seq() {
    let speed = OutputPoint {
        scale: 0.5,
        offset: 10.0,
        ..metric("s", PointType::U16)
    };
    assert_eq!(
        encode_ndata(&[speed], &[60, 0], 5, 123),
        Ok(vec![
            0x08, 0x7B, // timestamp 123
            0x12, 0x09, // a metric of 9 bytes
            0x0A, 0x01, b's', // name
            0x18, 0x7B, // timestamp 123
            0x20, 0x06, // UInt16
            0x50, 0x64, // int_value 100
            0x18, 0x05, // seq 5
        ])
    );
}

/// Each type takes its value field: `Int16` −1 as the two's complement `0xFFFF_FFFF` in
/// `int_value`, a five-byte varint; `UInt32` `0x1234_5678` as the varint 0xF8 0xAC 0xD1 0x91 0x01;
/// `Int32` −2 as `0xFFFF_FFFE`; `Float` 12.5 as the four bytes of `0x4148_0000`, least significant
/// first; `Boolean` true as 1 (Sparkplug B data types 2, 7, 3, 9 and 11). Catches a signed value
/// encoded as a negative varint of ten bytes, a float as an integer, and the wrong data type.
#[test]
fn each_type_takes_its_value_field() {
    let cases: [(OutputPoint, &[u8], &[u8]); 5] = [
        (
            metric("a", PointType::I16),
            &[0xFF, 0xFF],
            &[
                0x0A, 0x01, b'a', 0x18, 0x00, 0x20, 0x02, 0x50, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F,
            ],
        ),
        (
            metric("u", PointType::U32),
            &[0x78, 0x56, 0x34, 0x12],
            &[
                0x0A, 0x01, b'u', 0x18, 0x00, 0x20, 0x07, 0x50, 0xF8, 0xAC, 0xD1, 0x91, 0x01,
            ],
        ),
        (
            metric("i", PointType::I32),
            &[0xFE, 0xFF, 0xFF, 0xFF],
            &[
                0x0A, 0x01, b'i', 0x18, 0x00, 0x20, 0x03, 0x50, 0xFE, 0xFF, 0xFF, 0xFF, 0x0F,
            ],
        ),
        (
            metric("f", PointType::F32),
            &[0x00, 0x00, 0x48, 0x41],
            &[
                0x0A, 0x01, b'f', 0x18, 0x00, 0x20, 0x09, 0x65, 0x00, 0x00, 0x48, 0x41,
            ],
        ),
        (
            metric("b", PointType::Bool),
            &[0b10],
            &[0x0A, 0x01, b'b', 0x18, 0x00, 0x20, 0x0B, 0x70, 0x01],
        ),
    ];
    for (point, image, expected) in cases {
        assert_eq!(
            encode_ndata(core::slice::from_ref(&point), image, 0, 0),
            Ok(one_metric(expected)),
            "{}",
            point.metric_name
        );
    }
    let halved = OutputPoint {
        scale: 0.5,
        ..metric("h", PointType::U16)
    };
    assert_eq!(
        encode_ndata(&[halved], &[0xFF, 0xFF], 0, 0),
        Err(String::from("mqtt u16 value 131070 is out of range"))
    );
}

/// The NBIRTH holds `bdSeq` (`UInt64`, `long_value` 7) and then each metric's name and data type
/// without a value; the NDEATH holds `bdSeq` alone and seq 0, as trust-runtime writes them; a
/// `bdSeq` of 300 takes the varint 0xAC 0x02. Catches a birth without `bdSeq`, a death whose
/// `bdSeq` differs from the birth's, and a metric left out of the birth.
#[test]
fn the_birth_and_the_death_hold_bd_seq() {
    let bd_seq = [
        0x0A, 0x05, b'b', b'd', b'S', b'e', b'q', 0x18, 0x01, 0x20, 0x08, 0x58, 0x07,
    ];
    let mut birth = vec![0x08, 0x01, 0x12, 0x0D];
    birth.extend_from_slice(&bd_seq);
    birth.extend_from_slice(&[
        0x12, 0x07, 0x0A, 0x01, b's', 0x18, 0x01, 0x20, 0x06, 0x18, 0x00,
    ]);
    assert_eq!(
        encode_nbirth(&node(), &[metric("s", PointType::U16)], 0, 1),
        birth
    );
    let mut death = vec![0x08, 0x01, 0x12, 0x0D];
    death.extend_from_slice(&bd_seq);
    death.extend_from_slice(&[0x18, 0x00]);
    assert_eq!(encode_ndeath(&node(), 1), death);
    let later = SparkplugParams {
        birth_death_seq: 300,
        ..node()
    };
    assert_eq!(
        encode_ndeath(&later, 0),
        vec![
            0x08, 0x00, 0x12, 0x0E, 0x0A, 0x05, b'b', b'd', b'S', b'e', b'q', 0x18, 0x00, 0x20,
            0x08, 0x58, 0xAC, 0x02, 0x18, 0x00,
        ]
    );
}

/// The `sparkplug` table takes its keys trimmed, with `spBv1.0` and `3.0.0` as the only namespace
/// and version, requires `group_id` and `edge_node_id` as single topic levels, and is off with
/// `enabled = false` whatever its values (specification 32 §9). Catches a group with a `/` that
/// would shift the topic levels, another Sparkplug version claimed, and a disabled table checked.
#[test]
fn the_sparkplug_table_is_checked() {
    assert_eq!(
        sparkplug(r#"{"edge_node_id":" n ","group_id":" g ","namespace":" spBv1.0 "}"#)
            .map(|params| params.sparkplug),
        Ok(Some(SparkplugParams {
            group_id: "g".into(),
            edge_node_id: "n".into(),
            birth_death_seq: 0,
        }))
    );
    assert_eq!(
        sparkplug(r#"{"enabled":false,"namespace":"x"}"#).map(|params| params.sparkplug),
        Ok(None)
    );
    let level = "a non-empty MQTT topic level without /, +, # and control characters";
    for (table, key, reason) in [
        (
            r#"{"edge_node_id":"n"}"#,
            "sparkplug.group_id",
            format!("is required, {level}"),
        ),
        (
            r#"{"edge_node_id":"n","group_id":"a/b"}"#,
            "sparkplug.group_id",
            format!(r#"must be {level}, not "a/b""#),
        ),
        (
            r#"{"edge_node_id":" ","group_id":"g"}"#,
            "sparkplug.edge_node_id",
            format!(r#"must be {level}, not " ""#),
        ),
        (
            r#"{"edge_node_id":"n","group_id":"g","namespace":"spBv2.0"}"#,
            "sparkplug.namespace",
            String::from(r#"must be "spBv1.0", not "spBv2.0""#),
        ),
        (
            r#"{"edge_node_id":"n","group_id":"g","spec_version":"2.2"}"#,
            "sparkplug.spec_version",
            String::from(r#"must be "3.0.0", not "2.2""#),
        ),
        (
            r#"{"enabled":"no"}"#,
            "sparkplug.enabled",
            String::from(r#"must be true or false, not "no""#),
        ),
        (
            r#"{"birth_death_seq":-1,"edge_node_id":"n","group_id":"g"}"#,
            "sparkplug.birth_death_seq",
            String::from("must be an integer from 0 to 9223372036854775807, not -1"),
        ),
        ("5", "sparkplug", String::from("must be a table, not 5")),
    ] {
        assert_eq!(sparkplug(table), refused(key, &reason), "{table}");
    }
}

/// The profile publishes output points only, needs at least one, and keeps its session, which an
/// empty client identifier cannot (specification 32 §9; MQTT 3.1.1, MQTT-3.1.3-7). Catches input
/// points that the profile would never read, a birth without metrics, and a CONNECT that a broker
/// must refuse.
#[test]
fn the_profile_takes_outputs_only_and_a_client_identifier() {
    let table = r#""sparkplug":{"edge_node_id":"n","group_id":"g"}"#;
    let output = r#""output_points":[{"data_type":"u16","image_offset":0,"topic":"s"}]"#;
    let input = r#""input_points":[{"data_type":"u16","image_offset":0,"topic":"in"}]"#;
    let local = r#""broker":"127.0.0.1:1883""#;
    assert_eq!(
        MqttParams::from_json(&format!("{{{local},{input},{output},{table}}}")),
        refused(
            "input_points",
            "is not taken with Sparkplug, whose profile publishes output points only"
        )
    );
    assert_eq!(
        MqttParams::from_json(&format!("{{{local},{table}}}")),
        refused(
            "output_points",
            "is required with Sparkplug, at least one output point for a metric"
        )
    );
    assert_eq!(
        MqttParams::from_json(&format!(r#"{{{local},"client_id":"",{output},{table}}}"#)),
        refused(
            "client_id",
            "must not be empty with Sparkplug, whose session is not clean (MQTT 3.1.1, \
             MQTT-3.1.3-7)"
        )
    );
}
