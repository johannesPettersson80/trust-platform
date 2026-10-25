//! The typed points of the `params` table (specification 32 §6 and §7; specification 11, typed
//! protocol point-map integration contract).

use alloc::format;
use alloc::vec::Vec;

use crate::io_drivers::mqtt::{InputPoint, MqttParams, OutputPoint, PayloadFormat, PointType};
use crate::io_drivers::ParamsError;

/// What a topic must be, as a refusal states it.
const EXACT: &str = "an exact MQTT topic: not empty, at most 65535 bytes, without the wildcards \
                     + and # and without control characters";

/// The scalar types, as a refusal lists them.
const TYPES: &str = "bool, u16, i16, u32, i32 or f32, or an alias: boolean, uint16, word, int16, \
                     int, uint32, dword, int32, dint, float or real";

/// The refusal of `key` for `reason`.
fn refused<T>(key: &str, reason: &str) -> Result<T, ParamsError> {
    Err(ParamsError {
        key: key.into(),
        reason: reason.into(),
    })
}

/// The input points of a local broker with the input points `points`, JSON tables.
fn inputs(points: &str) -> Result<Vec<InputPoint>, ParamsError> {
    MqttParams::from_json(&format!(
        r#"{{"broker":"127.0.0.1:1883","input_points":[{points}]}}"#
    ))
    .map(|params| params.input_points)
}

/// The output points of a local broker with the output points `points`, JSON tables.
fn outputs(points: &str) -> Result<Vec<OutputPoint>, ParamsError> {
    MqttParams::from_json(&format!(
        r#"{{"broker":"127.0.0.1:1883","output_points":[{points}]}}"#
    ))
    .map(|params| params.output_points)
}

/// The data type of the one input point `point`.
fn data_type(point: &str) -> Result<PointType, ParamsError> {
    inputs(point).map(|points| points[0].data_type)
}

/// Every type and each of its aliases are taken, trimmed and in any case, and an omitted or
/// unknown type is refused (specification 32 §6; specification 11: `boolean`, `word`, `int`,
/// `dword`, `dint` and `real` and the explicit-width aliases); a value uses 2 or 4 bytes of the
/// image, a bool the byte of its bit. Catches an alias that selects another width or signedness,
/// such as `int` read as 32 bits, a point without a type, and a width counted wrong when an online
/// change counts the wired bytes.
#[test]
fn types_and_their_aliases_select_the_scalar_in_any_case() {
    for (word, expected) in [(" BOOLEAN ", PointType::Bool), ("bool", PointType::Bool)] {
        let point =
            format!(r#"{{"data_type":"{word}","image_bit":0,"image_offset":0,"topic":"a"}}"#);
        assert_eq!(data_type(&point), Ok(expected), "{word}");
    }
    for (word, expected) in [
        ("u16", PointType::U16),
        ("Uint16", PointType::U16),
        ("WORD", PointType::U16),
        ("i16", PointType::I16),
        ("INT16", PointType::I16),
        ("int", PointType::I16),
        ("u32", PointType::U32),
        ("UINT32", PointType::U32),
        ("dword", PointType::U32),
        ("i32", PointType::I32),
        ("Int32", PointType::I32),
        ("DINT", PointType::I32),
        ("f32", PointType::F32),
        ("FLOAT", PointType::F32),
        ("real", PointType::F32),
    ] {
        let point = format!(r#"{{"data_type":"{word}","image_offset":0,"topic":"a"}}"#);
        assert_eq!(data_type(&point), Ok(expected), "{word}");
    }
    assert_eq!(
        [
            PointType::Bool,
            PointType::U16,
            PointType::I16,
            PointType::U32,
            PointType::I32,
            PointType::F32,
        ]
        .map(PointType::image_bytes),
        [1, 2, 2, 4, 4, 4]
    );
    assert_eq!(
        data_type(r#"{"image_offset":0,"topic":"a"}"#),
        refused(
            "input_points[0].data_type",
            &format!("is required, {TYPES}")
        )
    );
    assert_eq!(
        data_type(r#"{"data_type":"f64","image_offset":0,"topic":"a"}"#),
        refused(
            "input_points[0].data_type",
            &format!(r#"must be {TYPES}, not "f64""#)
        )
    );
}

/// Each payload format and its aliases are taken trimmed, in any case and with `-` as `_`, and an
/// omitted format is text (specification 32 §7). Catches a big-endian alias read as
/// little-endian, and a missing format that is not text.
#[test]
fn payload_formats_and_their_aliases_are_taken() {
    let format_of = |member: &str| {
        inputs(&format!(
            r#"{{"data_type":"u16","image_offset":0,{member}"topic":"a"}}"#
        ))
        .map(|points| points[0].payload_format)
    };
    for (given, expected) in [
        ("", PayloadFormat::Text),
        (r#""payload_format":"TEXT","#, PayloadFormat::Text),
        (r#""payload_format":"string","#, PayloadFormat::Text),
        (r#""payload_format":"Json","#, PayloadFormat::Json),
        (r#""payload_format":"binary-le","#, PayloadFormat::BinaryLe),
        (
            r#""payload_format":"little_endian","#,
            PayloadFormat::BinaryLe,
        ),
        (r#""payload_format":" LE ","#, PayloadFormat::BinaryLe),
        (r#""payload_format":"binary_be","#, PayloadFormat::BinaryBe),
        (r#""payload_format":"Big-Endian","#, PayloadFormat::BinaryBe),
        (r#""payload_format":"be","#, PayloadFormat::BinaryBe),
    ] {
        assert_eq!(format_of(given), Ok(expected), "{given}");
    }
    assert_eq!(
        format_of(r#""payload_format":"native","#),
        refused(
            "input_points[0].payload_format",
            "must be text, json, binary_le or binary_be, or an alias: string, little_endian, \
             le, big_endian or be, not \"native\""
        )
    );
}

/// A point reads into its values: the topic trimmed, scale and offset as given, the metric name of
/// an output point trimmed and the topic when absent. Catches a key read into the wrong field and
/// a metric name that is not the topic by default.
#[test]
fn a_point_reads_into_its_values() {
    assert_eq!(
        inputs(concat!(
            r#"{"data_type":"i16","image_offset":3,"offset":-10,"payload_format":"json","#,
            r#""scale":0.5,"topic":" line/temp "}"#
        )),
        Ok(Vec::from([InputPoint {
            topic: "line/temp".into(),
            image_offset: 3,
            image_bit: None,
            data_type: PointType::I16,
            payload_format: PayloadFormat::Json,
            scale: 0.5,
            offset: -10.0,
        }]))
    );
    assert_eq!(
        outputs(concat!(
            r#"{"data_type":"bool","image_bit":7,"image_offset":4294967295,"topic":"a"},"#,
            r#"{"data_type":"u16","image_offset":0,"metric_name":" drive/speed ","topic":"b"}"#
        )),
        Ok(Vec::from([
            OutputPoint {
                topic: "a".into(),
                metric_name: "a".into(),
                image_offset: 4_294_967_295,
                image_bit: Some(7),
                data_type: PointType::Bool,
                payload_format: PayloadFormat::Text,
                scale: 1.0,
                offset: 0.0,
            },
            OutputPoint {
                topic: "b".into(),
                metric_name: "drive/speed".into(),
                image_offset: 0,
                image_bit: None,
                data_type: PointType::U16,
                payload_format: PayloadFormat::Text,
                scale: 1.0,
                offset: 0.0,
            },
        ]))
    );
}

/// A bool point needs its bit, 0 to 7, and takes no scale or offset; a numeric point takes no bit
/// and a finite scale other than 0 and a finite offset (specification 32 §7). Catches a bool
/// without its bit, a ninth bit, a number given a bit, a scale of 0 that would divide by zero on
/// output, and scaling on a bool.
#[test]
fn the_shape_and_the_scaling_of_a_point_are_checked() {
    for (point, key, reason) in [
        (
            r#"{"data_type":"bool","image_offset":0,"topic":"a"}"#,
            "image_bit",
            "is required for a bool point, the bit from 0 to 7",
        ),
        (
            r#"{"data_type":"bool","image_bit":8,"image_offset":0,"topic":"a"}"#,
            "image_bit",
            "must be an integer from 0 to 7, not 8",
        ),
        (
            r#"{"data_type":"u16","image_bit":0,"image_offset":0,"topic":"a"}"#,
            "image_bit",
            "is only for bool points, not for a u16 point",
        ),
        (
            r#"{"data_type":"bool","image_bit":1,"image_offset":0,"scale":1,"topic":"a"}"#,
            "scale",
            "is not taken by a bool point",
        ),
        (
            r#"{"data_type":"bool","image_bit":1,"image_offset":0,"offset":0,"topic":"a"}"#,
            "offset",
            "is not taken by a bool point",
        ),
        (
            r#"{"data_type":"u16","image_offset":0,"scale":0,"topic":"a"}"#,
            "scale",
            "must be a finite number other than 0, not 0",
        ),
        (
            r#"{"data_type":"u16","image_offset":0,"scale":-0.0,"topic":"a"}"#,
            "scale",
            "must be a finite number other than 0, not -0.0",
        ),
        (
            r#"{"data_type":"u16","image_offset":0,"offset":"1","topic":"a"}"#,
            "offset",
            r#"must be a finite number, not "1""#,
        ),
        (
            r#"{"data_type":"u16","topic":"a"}"#,
            "image_offset",
            "is required, an integer from 0 to 4294967295",
        ),
        (
            r#"{"data_type":"u16","image_offset":-1,"topic":"a"}"#,
            "image_offset",
            "must be an integer from 0 to 4294967295, not -1",
        ),
    ] {
        assert_eq!(
            inputs(point),
            refused(&format!("input_points[0].{key}"), reason),
            "{point}"
        );
    }
    assert_eq!(
        inputs("5"),
        refused("input_points[0]", "must be a table, not 5")
    );
}

/// A topic is trimmed and must be an exact topic: not empty, without the wildcards `+` and `#` and
/// without control characters (specification 32 §7); an output's metric name is not empty and
/// has no control characters. Catches a subscription filter taken as a delivery topic, and an
/// empty or blank topic.
#[test]
fn a_topic_is_an_exact_topic_and_a_metric_name_a_plain_name() {
    for (topic, shown) in [
        (r#""line/+""#, r#""line/+""#),
        (r#""line/#""#, r#""line/#""#),
        (r#""  ""#, r#""  ""#),
        (r#""""#, r#""""#),
        (r#""line\nvalue""#, r#""line\nvalue""#),
        (r#""line\u0000value""#, r#""line\u0000value""#),
        ("7", "7"),
    ] {
        let point = format!(r#"{{"data_type":"u16","image_offset":0,"topic":{topic}}}"#);
        assert_eq!(
            inputs(&point),
            refused(
                "input_points[0].topic",
                &format!("must be {EXACT}, not {shown}")
            ),
            "{topic}"
        );
    }
    assert_eq!(
        inputs(r#"{"data_type":"u16","image_offset":0}"#),
        refused("input_points[0].topic", &format!("is required, {EXACT}"))
    );
    assert_eq!(
        outputs(r#"{"data_type":"u16","image_offset":0,"metric_name":" ","topic":"a"}"#),
        refused(
            "output_points[0].metric_name",
            r#"must be a non-empty name without control characters, not " ""#
        )
    );
}

/// Within one list two points take neither one topic nor one bit of the image: a 32-bit value at
/// byte 0 uses bytes 0 to 3, so a 16-bit value at byte 2 overlaps it, and a bool at bit 0 of byte
/// 2 lies in a 16-bit value at byte 1; two bits of one byte, adjacent words, and an input and an
/// output at one offset are distinct (specification 32 §7; specification 11). Catches two points
/// that would write the same storage, and distinct storage refused.
#[test]
fn points_of_a_list_take_topics_and_storage_of_their_own() {
    let two_inputs = |first: &str, second: &str| inputs(&format!("{first},{second}"));
    assert_eq!(
        two_inputs(
            r#"{"data_type":"u16","image_offset":0,"topic":"line/value"}"#,
            r#"{"data_type":"u16","image_offset":2,"topic":"line/value"}"#
        ),
        refused(
            "input_points[1].topic",
            concat!(
                r#""line/value" is used by input_points[0].topic already; "#,
                "each point of a list has a topic of its own"
            )
        )
    );
    assert_eq!(
        outputs(concat!(
            r#"{"data_type":"u32","image_offset":0,"topic":"a"},"#,
            r#"{"data_type":"u16","image_offset":2,"topic":"b"}"#
        )),
        refused(
            "output_points[1].image_offset",
            "%QW2 overlaps %QD0 of output_points[0]; the points of a list use storage of their own"
        )
    );
    assert_eq!(
        two_inputs(
            r#"{"data_type":"bool","image_bit":3,"image_offset":0,"topic":"a"}"#,
            r#"{"data_type":"bool","image_bit":3,"image_offset":0,"topic":"b"}"#
        ),
        refused(
            "input_points[1].image_offset",
            "%IX0.3 overlaps %IX0.3 of input_points[0]; the points of a list use storage of their \
             own"
        )
    );
    assert_eq!(
        two_inputs(
            r#"{"data_type":"i16","image_offset":1,"topic":"a"}"#,
            r#"{"data_type":"bool","image_bit":0,"image_offset":2,"topic":"b"}"#
        ),
        refused(
            "input_points[1].image_offset",
            "%IX2.0 overlaps %IW1 of input_points[0]; the points of a list use storage of their own"
        )
    );
    for (first, second) in [
        (
            r#"{"data_type":"bool","image_bit":3,"image_offset":0,"topic":"a"}"#,
            r#"{"data_type":"bool","image_bit":4,"image_offset":0,"topic":"b"}"#,
        ),
        (
            r#"{"data_type":"u16","image_offset":0,"topic":"a"}"#,
            r#"{"data_type":"u16","image_offset":2,"topic":"b"}"#,
        ),
    ] {
        assert!(two_inputs(first, second).is_ok(), "{first} {second}");
    }
    let both = MqttParams::from_json(concat!(
        r#"{"broker":"127.0.0.1:1883","#,
        r#""input_points":[{"data_type":"u32","image_offset":0,"topic":"a"}],"#,
        r#""output_points":[{"data_type":"u32","image_offset":0,"topic":"a"}]}"#
    ));
    assert!(
        both.is_ok(),
        "an input and an output at one offset: {both:?}"
    );
}
