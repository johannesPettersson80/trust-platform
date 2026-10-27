//! The typed point maps of `io.toml` as the computer reads them (specification 32 §6 and §7;
//! specification 11, typed protocol point-map integration contract), and their payloads carried
//! through the broker stand-in. These are the computer's point-map contract tests, moved onto the
//! shared rules: their expected texts are the shared rules' words, which replace the computer's own
//! (`mqtt input_points...`) because the driver's logic is one implementation on every runtime
//! (specification 34 ARCH-11). The shared module's tests prove each conversion byte for byte; these
//! prove that the computer's driver carries them between the broker and the process image.

use trust_runtime_services::io_drivers::mqtt::{MqttParams, PayloadFormat, PointType};

use super::broker::Broker;
use super::{at, read, read_fresh, refused, start, table, MqttIoDriver, TYPES};
use crate::error::RuntimeError;
use crate::io::IoDriver;

/// What a topic must be, as a refusal states it (specification 32 §7).
const EXACT: &str = "an exact MQTT topic: not empty, at most 65535 bytes, without the wildcards + \
                     and # and without control characters";

/// The storage rule, as a refusal states it.
const STORAGE_ONCE: &str = "the points of a list use storage of their own";

/// The parameters of a local broker with one input point whose keys are `fields`.
fn input_point(fields: &str) -> Result<MqttParams, RuntimeError> {
    read(&format!(
        "broker = \"127.0.0.1:1883\"\n[[input_points]]\ntopic = \"a\"\nimage_offset = 0\n{fields}\n"
    ))
}

/// The table of a local broker with one point in `list`, `input_points` or `output_points`, whose
/// keys are `fields` and whose `key` is the string `value`, set as a value so that it may hold
/// any character.
fn with_string(list: &str, fields: &str, key: &str, value: &str) -> toml::Value {
    let mut params = table(&format!(
        "broker = \"127.0.0.1:1883\"\n[[{list}]]\n{fields}\n"
    ));
    params[list][0]
        .as_table_mut()
        .expect("a point")
        .insert(key.into(), toml::Value::String(value.into()));
    params
}

/// A driver of the broker stand-in with the points `points`, its session running and each of its
/// `topics` subscribed to.
fn point_driver(points: &str, topics: &[&str]) -> (Broker, MqttIoDriver) {
    let broker = Broker::start();
    let driver = start(&at(&broker, points));
    broker.wait_for("subscriptions", |record| record.subscriptions == topics);
    (broker, driver)
}

/// Delivers `payload` on `topic` and reads it: the image after a fresh read, or the error of a
/// payload that does not convert, with the image as it was.
fn exchange(
    broker: &Broker,
    driver: &mut MqttIoDriver,
    image: &mut [u8],
    topic: &str,
    payload: &[u8],
) -> Result<(), RuntimeError> {
    let before = image.to_vec();
    broker.deliver(&[(topic, payload)]);
    let result = read_fresh(driver, image);
    if result.is_err() {
        assert_eq!(
            image,
            before.as_slice(),
            "{topic} {payload:?}: a refused payload"
        );
    }
    result
}

/// The refusal of a payload that does not convert, with `text`.
fn refused_payload(text: &str) -> Result<(), RuntimeError> {
    Err(RuntimeError::IoDriver(text.into()))
}

/// Whether `result` is the refusal of a payload whose JSON does not parse, whose text after `mqtt
/// JSON payload: ` is the JSON parser's.
fn json_refused(result: &Result<(), RuntimeError>) -> bool {
    matches!(result, Err(RuntimeError::IoDriver(text)) if text.starts_with("mqtt JSON payload: "))
}

/// Every type and each of its aliases select the scalar, trimmed and in any case, and an omitted or
/// unknown type is refused (specification 32 §6). Catches an alias that selects another width or
/// signedness, and a point without a type.
#[test]
fn mqtt_point_contract_type_aliases_are_case_insensitive() {
    for (alias, expected) in [
        ("bool", PointType::Bool),
        ("BOOLEAN", PointType::Bool),
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
        ("INT32", PointType::I32),
        ("dint", PointType::I32),
        ("f32", PointType::F32),
        ("FLOAT", PointType::F32),
        ("real", PointType::F32),
    ] {
        let bit = if expected == PointType::Bool {
            "image_bit = 0\n"
        } else {
            ""
        };
        let params = input_point(&format!("{bit}data_type = \"{alias}\""))
            .unwrap_or_else(|error| panic!("{alias}: {error}"));
        assert_eq!(params.input_points[0].data_type, expected, "{alias}");
    }
    assert_eq!(
        input_point(""),
        refused(
            "input_points[0].data_type",
            &format!("is required, {TYPES}")
        )
    );
    assert_eq!(
        input_point("data_type = \"f64\""),
        refused(
            "input_points[0].data_type",
            &format!("must be {TYPES}, not \"f64\"")
        )
    );
}

/// Each payload format and its aliases are taken trimmed, in any case and with `-` as `_`, and an
/// omitted format is text (specification 32 §7). Catches a big-endian alias read as little-endian.
#[test]
fn mqtt_point_contract_payload_format_aliases_are_case_insensitive() {
    for (given, expected) in [
        ("", PayloadFormat::Text),
        ("payload_format = \"TEXT\"", PayloadFormat::Text),
        ("payload_format = \"string\"", PayloadFormat::Text),
        ("payload_format = \"Json\"", PayloadFormat::Json),
        ("payload_format = \"binary-le\"", PayloadFormat::BinaryLe),
        (
            "payload_format = \"little_endian\"",
            PayloadFormat::BinaryLe,
        ),
        ("payload_format = \"LE\"", PayloadFormat::BinaryLe),
        ("payload_format = \"binary_be\"", PayloadFormat::BinaryBe),
        ("payload_format = \"big-endian\"", PayloadFormat::BinaryBe),
        ("payload_format = \"be\"", PayloadFormat::BinaryBe),
    ] {
        let params = input_point(&format!("data_type = \"u16\"\n{given}"))
            .unwrap_or_else(|error| panic!("{given}: {error}"));
        assert_eq!(params.input_points[0].payload_format, expected, "{given}");
    }
    assert_eq!(
        input_point("data_type = \"u16\"\npayload_format = \"native\""),
        refused(
            "input_points[0].payload_format",
            "must be text, json, binary_le or binary_be, or an alias: string, little_endian, le, \
             big_endian or be, not \"native\""
        )
    );
}

/// A topic is trimmed and must be an exact topic: not empty, without the wildcards `+` and `#` and
/// without control characters (specification 32 §7); the refusal shows the value as JSON. Catches a
/// subscription filter taken as a delivery topic.
#[test]
fn mqtt_point_contract_topics_are_exact_noncontrol_names_not_filters() {
    let fields = "image_offset = 0\ndata_type = \"u16\"";
    let params = MqttIoDriver::read_params(&with_string(
        "input_points",
        fields,
        "topic",
        " line/temperature ",
    ))
    .expect("a valid topic");
    assert_eq!(params.input_points[0].topic, "line/temperature");
    for (topic, shown) in [
        ("", r#""""#),
        (" \t ", r#"" \t ""#),
        ("line/+", r#""line/+""#),
        ("line/#", r#""line/#""#),
        ("line/\nvalue", r#""line/\nvalue""#),
        ("line/\0value", r#""line/\u0000value""#),
    ] {
        assert_eq!(
            MqttIoDriver::read_params(&with_string("input_points", fields, "topic", topic)),
            refused(
                "input_points[0].topic",
                &format!("must be {EXACT}, not {shown}")
            ),
            "{topic:?}"
        );
    }
}

/// An output point's metric name is trimmed and must be a non-empty name without control
/// characters (specification 32 §9). Catches an empty metric name taken.
#[test]
fn mqtt_point_contract_metric_names_are_trimmed_and_control_free() {
    let fields = "topic = \"line/out\"\nimage_offset = 0\ndata_type = \"u16\"";
    let params = MqttIoDriver::read_params(&with_string(
        "output_points",
        fields,
        "metric_name",
        " drive/speed ",
    ))
    .expect("a valid metric name");
    assert_eq!(params.output_points[0].metric_name, "drive/speed");
    for (name, shown) in [
        ("", r#""""#),
        (" \t ", r#"" \t ""#),
        ("drive\nspeed", r#""drive\nspeed""#),
        ("drive\0speed", r#""drive\u0000speed""#),
    ] {
        assert_eq!(
            MqttIoDriver::read_params(&with_string("output_points", fields, "metric_name", name)),
            refused(
                "output_points[0].metric_name",
                &format!("must be a non-empty name without control characters, not {shown}")
            ),
            "{name:?}"
        );
    }
}

/// A bool point needs its bit, 0 to 7, and a numeric point takes none (specification 32 §7).
/// Catches a bool without its bit, a ninth bit, and a number given a bit.
#[test]
fn mqtt_point_contract_bool_and_numeric_shapes_are_disjoint() {
    for bit in 0..=7 {
        let params = input_point(&format!("image_bit = {bit}\ndata_type = \"bool\""))
            .expect("a bit from 0 to 7");
        assert_eq!(params.input_points[0].image_bit, Some(bit));
    }
    assert_eq!(
        input_point("data_type = \"bool\""),
        refused(
            "input_points[0].image_bit",
            "is required for a bool point, the bit from 0 to 7"
        )
    );
    assert_eq!(
        input_point("image_bit = 8\ndata_type = \"bool\""),
        refused(
            "input_points[0].image_bit",
            "must be an integer from 0 to 7, not 8"
        )
    );
    for name in ["u16", "i16", "u32", "i32", "f32"] {
        assert_eq!(
            input_point(&format!("image_bit = 0\ndata_type = \"{name}\"")),
            refused(
                "input_points[0].image_bit",
                &format!("is only for bool points, not for a {name} point")
            )
        );
    }
}

/// Scale is finite and not 0, offset is finite, 1 and 0 when absent, and a bool point takes
/// neither (specification 32 §7). TOML writes a float that JSON cannot, `nan` and `inf`, which the
/// computer refuses with its key before the shared rules read the table (specification 34 §5.1.1,
/// section 8). Catches a scale of 0 that cannot be inverted, a NaN or an infinity taken, and
/// scaling on a bool.
#[test]
fn mqtt_point_contract_scale_offset_are_finite_nonzero_and_numeric_only() {
    let point = |fields: &str| input_point(&format!("data_type = \"i16\"\n{fields}"));
    let scaling = |fields: &str| {
        point(fields).map(|params| (params.input_points[0].scale, params.input_points[0].offset))
    };
    assert_eq!(scaling(""), Ok((1.0, 0.0)));
    assert_eq!(scaling("scale = -0.5\noffset = 10.0"), Ok((-0.5, 10.0)));
    for (given, shown) in [("0.0", "0.0"), ("-0.0", "-0.0")] {
        assert_eq!(
            point(&format!("scale = {given}")),
            refused(
                "input_points[0].scale",
                &format!("must be a finite number other than 0, not {shown}")
            )
        );
    }
    for key in ["scale", "offset"] {
        for (given, shown) in [("nan", "NaN"), ("inf", "inf"), ("-inf", "-inf")] {
            assert_eq!(
                point(&format!("{key} = {given}")),
                refused(
                    &format!("input_points[0].{key}"),
                    &format!("{shown} is not a finite number")
                ),
                "{key} = {given}"
            );
        }
    }
    for (fields, key) in [("scale = 2.0", "scale"), ("offset = 1.0", "offset")] {
        assert_eq!(
            input_point(&format!("image_bit = 0\ndata_type = \"bool\"\n{fields}")),
            refused(
                &format!("input_points[0].{key}"),
                "is not taken by a bool point"
            )
        );
    }
}

/// Within one list two points take neither one topic nor one bit of storage: a `u16` at byte 2
/// lies inside a `u32` at byte 0, and two bools take bit 3 of byte 0 (specification 32 §7;
/// specification 11). Catches two points that would write the same storage.
#[test]
fn mqtt_point_contract_duplicate_topics_and_overlapping_image_ranges_are_rejected() {
    let broker = "broker = \"127.0.0.1:1883\"\n";
    assert_eq!(
        read(&format!(
            "{broker}[[input_points]]\ntopic = \"line/value\"\nimage_offset = 0\n\
             data_type = \"u16\"\n[[input_points]]\ntopic = \"line/value\"\nimage_offset = 2\n\
             data_type = \"u16\"\n"
        )),
        refused(
            "input_points[1].topic",
            "\"line/value\" is used by input_points[0].topic already; each point of a list has a \
             topic of its own"
        )
    );
    assert_eq!(
        read(&format!(
            "{broker}[[output_points]]\ntopic = \"line/first\"\nimage_offset = 0\n\
             data_type = \"u32\"\n[[output_points]]\ntopic = \"line/second\"\nimage_offset = 2\n\
             data_type = \"u16\"\n"
        )),
        refused(
            "output_points[1].image_offset",
            &format!("%QW2 overlaps %QD0 of output_points[0]; {STORAGE_ONCE}")
        )
    );
    assert_eq!(
        read(&format!(
            "{broker}[[input_points]]\ntopic = \"line/first\"\nimage_offset = 0\nimage_bit = 3\n\
             data_type = \"bool\"\n[[input_points]]\ntopic = \"line/second\"\nimage_offset = 0\n\
             image_bit = 3\ndata_type = \"bool\"\n"
        )),
        refused(
            "input_points[1].image_offset",
            &format!("%IX0.3 overlaps %IX0.3 of input_points[0]; {STORAGE_ONCE}")
        )
    );
}

/// A bool payload is `true`, `1` or `on` and `false`, `0` or `off` as text in any case and with
/// whitespace; in JSON a Boolean, a number or such a string; one byte in binary; each sets or
/// clears its own bit (specification 32 §7). A payload of another shape is refused and changes
/// nothing. Catches a binary bool of two bytes taken, and a JSON object read as true.
#[test]
fn mqtt_point_contract_bool_payload_formats_are_scalar_and_exact() {
    let (broker, mut driver) = point_driver(
        "[[input_points]]\ntopic = \"b/text\"\nimage_offset = 0\nimage_bit = 0\n\
         data_type = \"bool\"\n[[input_points]]\ntopic = \"b/json\"\nimage_offset = 0\n\
         image_bit = 1\ndata_type = \"bool\"\npayload_format = \"json\"\n[[input_points]]\n\
         topic = \"b/binary\"\nimage_offset = 0\nimage_bit = 2\ndata_type = \"bool\"\n\
         payload_format = \"binary_le\"\n",
        &["b/text", "b/json", "b/binary"],
    );
    let mut image = [0_u8];
    for (topic, mask, payloads) in [
        ("b/text", 0b001, [&b"true"[..], b" TRUE ", b"1", b"on"]),
        ("b/json", 0b010, [&b"true"[..], b"1", br#""on""#, b"2.5"]),
    ] {
        for payload in payloads {
            assert_eq!(
                exchange(&broker, &mut driver, &mut image, topic, payload),
                Ok(())
            );
            assert_eq!(image[0] & mask, mask, "{topic} {payload:?}");
        }
    }
    for (topic, mask, payloads) in [
        ("b/text", 0b001, [&b"false"[..], b" FALSE ", b"0", b"off"]),
        ("b/json", 0b010, [&b"false"[..], b"0", br#""off""#, b"0.0"]),
    ] {
        for payload in payloads {
            assert_eq!(
                exchange(&broker, &mut driver, &mut image, topic, payload),
                Ok(())
            );
            assert_eq!(image[0] & mask, 0, "{topic} {payload:?}");
        }
    }
    assert_eq!(
        exchange(&broker, &mut driver, &mut image, "b/binary", &[1]),
        Ok(())
    );
    assert_eq!(image[0] & 0b100, 0b100);
    for payload in [&[][..], &[0, 1], &[1, 0]] {
        assert_eq!(
            exchange(&broker, &mut driver, &mut image, "b/binary", payload),
            refused_payload("mqtt bool binary payload must contain exactly one byte"),
            "{payload:?}"
        );
    }
    assert_eq!(
        exchange(&broker, &mut driver, &mut image, "b/text", b"maybe"),
        refused_payload("mqtt bool payload 'maybe' is not true/false or 1/0")
    );
    for payload in [&b"{}"[..], b"[]"] {
        assert_eq!(
            exchange(&broker, &mut driver, &mut image, "b/json", payload),
            refused_payload("mqtt bool JSON payload must be bool, number, or string")
        );
    }
    assert!(json_refused(&exchange(
        &broker,
        &mut driver,
        &mut image,
        "b/json",
        b"maybe"
    )));
}

/// A numeric text or JSON payload is one complete finite number, with surrounding whitespace: text
/// ` -12.5 ` is the `f32` `0xC148_0000`, and JSON `"42"` and `42` are 42 (specification 32 §7).
/// Trailing text, NaN, an infinity and a JSON value of another shape are refused and change
/// nothing. Catches trailing text taken and a non-finite value taken.
#[test]
fn mqtt_point_contract_numeric_text_and_json_require_one_finite_scalar() {
    let (broker, mut driver) = point_driver(
        "[[input_points]]\ntopic = \"n/text\"\nimage_offset = 0\ndata_type = \"f32\"\n\
         [[input_points]]\ntopic = \"n/json-i16\"\nimage_offset = 4\ndata_type = \"i16\"\n\
         payload_format = \"json\"\n[[input_points]]\ntopic = \"n/json-u16\"\nimage_offset = 6\n\
         data_type = \"u16\"\npayload_format = \"json\"\n[[input_points]]\n\
         topic = \"n/json-f32\"\nimage_offset = 8\ndata_type = \"f32\"\n\
         payload_format = \"json\"\n",
        &["n/text", "n/json-i16", "n/json-u16", "n/json-f32"],
    );
    let mut image = [0_u8; 12];
    assert_eq!(
        exchange(&broker, &mut driver, &mut image, "n/text", b" -12.5 "),
        Ok(())
    );
    assert_eq!(image[0..4], [0x00, 0x00, 0x48, 0xC1]);
    assert_eq!(
        exchange(&broker, &mut driver, &mut image, "n/json-i16", br#""42""#),
        Ok(())
    );
    assert_eq!(image[4..6], [42, 0]);
    assert_eq!(
        exchange(&broker, &mut driver, &mut image, "n/json-u16", b"42"),
        Ok(())
    );
    assert_eq!(image[6..8], [42, 0]);
    for payload in [&b"1 trailing"[..], b"NaN", b"Infinity"] {
        let result = exchange(&broker, &mut driver, &mut image, "n/json-f32", payload);
        assert!(json_refused(&result), "{payload:?}: {result:?}");
    }
    for payload in [&b"{}"[..], b"[]", b"true"] {
        assert_eq!(
            exchange(&broker, &mut driver, &mut image, "n/json-f32", payload),
            refused_payload("mqtt numeric JSON payload must be number or string"),
            "{payload:?}"
        );
    }
}

/// A binary payload holds exactly the bytes of its type in its byte order, and the image holds the
/// value little-endian whatever the payload's order (specification 32 §7): 0x1234, −1234
/// (`0xFB2E`), `0x1234_5678`, −123,456 (`0xFFFE_1DC0`) and 12.5 (`0x4148_0000`). A payload of one
/// byte more or less is refused. Catches the payload's byte order used for the image, and a
/// truncated or padded payload taken.
#[test]
fn mqtt_point_contract_binary_numeric_width_and_endianness_are_exact() {
    let mut points = String::new();
    let cases: [(&str, &[u8]); 5] = [
        ("u16", &[0x34, 0x12]),
        ("i16", &[0x2E, 0xFB]),
        ("u32", &[0x78, 0x56, 0x34, 0x12]),
        ("i32", &[0xC0, 0x1D, 0xFE, 0xFF]),
        ("f32", &[0x00, 0x00, 0x48, 0x41]),
    ];
    let mut offset = 0;
    let mut topics = Vec::new();
    for (name, little) in cases {
        for order in ["le", "be"] {
            points.push_str(&format!(
                "[[input_points]]\ntopic = \"{name}/{order}\"\nimage_offset = {offset}\n\
                 data_type = \"{name}\"\npayload_format = \"binary_{order}\"\n"
            ));
            topics.push(format!("{name}/{order}"));
            offset += little.len();
        }
    }
    let topics: Vec<&str> = topics.iter().map(String::as_str).collect();
    let (broker, mut driver) = point_driver(&points, &topics);
    let mut image = [0_u8; 32];
    let mut offset = 0;
    for (name, little) in cases {
        let big: Vec<u8> = little.iter().rev().copied().collect();
        for (order, payload) in [("le", little), ("be", big.as_slice())] {
            let topic = format!("{name}/{order}");
            assert_eq!(
                exchange(&broker, &mut driver, &mut image, &topic, payload),
                Ok(()),
                "{topic}"
            );
            assert_eq!(&image[offset..offset + little.len()], little, "{topic}");
            offset += little.len();
        }
        let width = little.len();
        let type_name = name.to_ascii_uppercase();
        let mut longer = little.to_vec();
        longer.push(0);
        for wrong in [longer.as_slice(), &little[..width - 1]] {
            assert_eq!(
                exchange(
                    &broker,
                    &mut driver,
                    &mut image,
                    &format!("{name}/le"),
                    wrong
                ),
                refused_payload(&format!(
                    "mqtt binary payload for {type_name} must be exactly {width} byte(s), got {}",
                    wrong.len()
                )),
                "{name} {wrong:?}"
            );
        }
    }
}

/// An `f32` text payload `NaN` is refused before it becomes a value of the point, and the image
/// stays as it was (specification 11, floating-point boundary admission; specification 32 §7: one
/// complete finite scalar). Catches a non-finite `REAL` read from the broker.
#[test]
fn mapped_f32_text_payload_rejects_non_finite() {
    let (broker, mut driver) = point_driver(
        "[[input_points]]\ntopic = \"line/temp\"\nimage_offset = 0\ndata_type = \"f32\"\n",
        &["line/temp"],
    );
    let mut image = [0xA5; 4];
    assert_eq!(
        exchange(&broker, &mut driver, &mut image, "line/temp", b"NaN"),
        refused_payload("mqtt numeric payload must be finite")
    );
}

/// Inbound scaling is `raw × scale + offset`: 100 × 0.5 − 10 = 40 in bytes 1 and 2, the bytes
/// around it untouched; 999,999 × 0.5 − 10 = 499,989.5 rounds to 499,990, beyond `i16`, and is
/// refused with the image as it was (specification 32 §7). Catches a scaled value written over its
/// neighbours, and an out-of-range value written in part.
#[test]
fn mqtt_point_contract_decode_scaling_and_image_encoding_are_atomic() {
    let (broker, mut driver) = point_driver(
        "[[input_points]]\ntopic = \"line/in\"\nimage_offset = 1\ndata_type = \"i16\"\n\
         payload_format = \"text\"\nscale = 0.5\noffset = -10.0\n",
        &["line/in"],
    );
    let mut image = [0xA5; 4];
    assert_eq!(
        exchange(&broker, &mut driver, &mut image, "line/in", b"100"),
        Ok(())
    );
    assert_eq!(image, [0xA5, 40, 0, 0xA5]);
    assert_eq!(
        exchange(&broker, &mut driver, &mut image, "line/in", b"999999"),
        refused_payload("mqtt i16 value 499990 is out of range")
    );
}

/// Outbound scaling is `(engineering − offset) ÷ scale`: (40 + 10) ÷ 0.5 = 100, published as JSON;
/// the point keeps its metric name (specification 32 §7). Catches the inverse scaling applied
/// backwards.
#[test]
fn mqtt_point_contract_output_inverse_scaling_preserves_declared_format() {
    let points = "[[output_points]]\ntopic = \"line/out\"\nimage_offset = 0\ndata_type = \"i16\"\n\
                  payload_format = \"json\"\nscale = 0.5\noffset = -10.0\n\
                  metric_name = \"line/speed\"\n";
    let params = read(&format!("broker = \"127.0.0.1:1883\"\n{points}")).expect("a point");
    assert_eq!(params.output_points[0].metric_name, "line/speed");
    let (broker, mut driver) = point_driver(points, &["trust/io/in"]);
    assert_eq!(driver.write_outputs(&40_i16.to_le_bytes()), Ok(()));
    broker.wait_for("publication", |record| !record.publishes.is_empty());
    assert_eq!(
        broker.record(|record| record.published("line/out")),
        [b"100".to_vec()]
    );
}

/// An integer rounds to the nearest whole number, a half away from zero, and is then checked
/// against its range (specification 32 §7): 1.49 is 1, 1.5 is 2, −1.5 is −2, 65,535.49 is 65,535;
/// −32,768.5 rounds to −32,769 and 32,767.5 to 32,768, beyond `i16`, and −0.5 to −1 and 65,535.5
/// to 65,536, beyond `u16`. Catches rounding half to even, truncation, and a saturated value.
#[test]
fn mqtt_point_contract_integer_rounding_is_ties_away_and_range_checked() {
    let (broker, mut driver) = point_driver(
        "[[input_points]]\ntopic = \"r/i16\"\nimage_offset = 0\ndata_type = \"i16\"\n\
         [[input_points]]\ntopic = \"r/u16\"\nimage_offset = 2\ndata_type = \"u16\"\n",
        &["r/i16", "r/u16"],
    );
    let mut image = [0_u8; 4];
    for (payload, expected) in [(&b"1.49"[..], 1_i16), (b"1.5", 2), (b"-1.5", -2)] {
        assert_eq!(
            exchange(&broker, &mut driver, &mut image, "r/i16", payload),
            Ok(())
        );
        assert_eq!(image[0..2], expected.to_le_bytes(), "{payload:?}");
    }
    assert_eq!(
        exchange(&broker, &mut driver, &mut image, "r/u16", b"65535.49"),
        Ok(())
    );
    assert_eq!(image[2..4], [0xFF, 0xFF]);
    for (topic, payload, text) in [
        (
            "r/i16",
            &b"-32768.5"[..],
            "mqtt i16 value -32769 is out of range",
        ),
        ("r/i16", b"32767.5", "mqtt i16 value 32768 is out of range"),
        ("r/u16", b"-0.5", "mqtt u16 value -1 is out of range"),
        ("r/u16", b"65535.5", "mqtt u16 value 65536 is out of range"),
    ] {
        assert_eq!(
            exchange(&broker, &mut driver, &mut image, topic, payload),
            refused_payload(text),
            "{topic} {payload:?}"
        );
    }
}

/// A point that does not lie within its image is refused before a byte is written: a `u32` at
/// byte 1 of a 3-byte input image, a bool at the last byte offset that the parameters take,
/// 4,294,967,295, and a `u16` output at byte 3 of a 3-byte output image (specification 11: every
/// offset-plus-width is checked against the image before mutation). Catches a value cut to fit
/// the image, and a write past its end.
#[test]
fn mqtt_point_contract_image_ranges_fail_without_partial_mutation() {
    let (broker, mut driver) = point_driver(
        "[[input_points]]\ntopic = \"wide\"\nimage_offset = 1\ndata_type = \"u32\"\n\
         [[input_points]]\ntopic = \"far\"\nimage_offset = 4294967295\nimage_bit = 0\n\
         data_type = \"bool\"\n[[output_points]]\ntopic = \"out\"\nimage_offset = 3\n\
         data_type = \"u16\"\n",
        &["wide", "far"],
    );
    let mut image = [0xA5; 3];
    assert_eq!(
        exchange(&broker, &mut driver, &mut image, "wide", b"1"),
        refused_payload("mqtt point image range 1..5 exceeds process image size 3")
    );
    assert_eq!(
        exchange(&broker, &mut driver, &mut image, "far", b"true"),
        refused_payload("mqtt point image_offset 4294967295 is outside input image")
    );
    assert_eq!(image, [0xA5; 3]);
    assert_eq!(
        driver.write_outputs(&[0xA5; 3]),
        Err(RuntimeError::IoTransport(
            "mqtt point image range 3..5 exceeds process image size 3".into()
        ))
    );
}

/// An `f32` output whose image value is not finite, −∞ (`0xFF80_0000`), is refused before
/// publication (specification 11, floating-point boundary admission). Catches an infinite `REAL`
/// published to the broker.
#[test]
fn mapped_f32_process_image_read_rejects_non_finite() {
    let (broker, mut driver) = point_driver(
        "[[output_points]]\ntopic = \"line/out\"\nimage_offset = 0\ndata_type = \"f32\"\n",
        &["trust/io/in"],
    );
    assert_eq!(
        driver.write_outputs(&f32::NEG_INFINITY.to_le_bytes()),
        Err(RuntimeError::IoTransport(
            "mqtt F32 process-image value must be finite".into()
        ))
    );
    assert!(broker.record(|record| record.publishes.is_empty()));
}
