fn mqtt_contract_point(
    topic: &str,
    data_type: &str,
    payload_format: Option<&str>,
    image_offset: usize,
    image_bit: Option<u8>,
) -> MqttPointToml {
    MqttPointToml {
        topic: topic.to_string(),
        image_offset,
        image_bit,
        data_type: Some(data_type.to_string()),
        payload_format: payload_format.map(str::to_string),
        metric_name: None,
        scale: None,
        offset: None,
    }
}

fn mqtt_contract_config(text: &str) -> Result<MqttIoDriver, RuntimeError> {
    let value: toml::Value = toml::from_str(text).expect("parse MQTT contract TOML");
    MqttIoDriver::from_params(&value)
}

#[test]
fn mqtt_point_contract_type_aliases_are_case_insensitive() {
    for (alias, expected) in [
        ("bool", MqttPointType::Bool),
        ("BOOLEAN", MqttPointType::Bool),
        ("u16", MqttPointType::U16),
        ("Uint16", MqttPointType::U16),
        ("WORD", MqttPointType::U16),
        ("i16", MqttPointType::I16),
        ("INT16", MqttPointType::I16),
        ("int", MqttPointType::I16),
        ("u32", MqttPointType::U32),
        ("UINT32", MqttPointType::U32),
        ("dword", MqttPointType::U32),
        ("i32", MqttPointType::I32),
        ("INT32", MqttPointType::I32),
        ("dint", MqttPointType::I32),
        ("f32", MqttPointType::F32),
        ("FLOAT", MqttPointType::F32),
        ("real", MqttPointType::F32),
    ] {
        assert_eq!(
            parse_data_type(Some(alias), "input_points").expect("accepted alias"),
            expected,
            "alias {alias}"
        );
    }
    assert!(parse_data_type(None, "input_points").is_err());
    assert!(parse_data_type(Some("f64"), "input_points").is_err());
}

#[test]
fn mqtt_point_contract_payload_format_aliases_are_case_insensitive() {
    for (alias, expected) in [
        (None, MqttPayloadFormat::Text),
        (Some("TEXT"), MqttPayloadFormat::Text),
        (Some("string"), MqttPayloadFormat::Text),
        (Some("Json"), MqttPayloadFormat::Json),
        (Some("binary-le"), MqttPayloadFormat::BinaryLe),
        (Some("little_endian"), MqttPayloadFormat::BinaryLe),
        (Some("LE"), MqttPayloadFormat::BinaryLe),
        (Some("binary_be"), MqttPayloadFormat::BinaryBe),
        (Some("big-endian"), MqttPayloadFormat::BinaryBe),
        (Some("be"), MqttPayloadFormat::BinaryBe),
    ] {
        assert_eq!(
            parse_payload_format(alias).expect("accepted payload alias"),
            expected
        );
    }
    assert!(parse_payload_format(Some("native")).is_err());
}

#[test]
fn mqtt_point_contract_topics_are_exact_noncontrol_names_not_filters() {
    assert_eq!(
        parse_point_topic("input_points", " line/temperature ")
            .expect("valid topic")
            .as_str(),
        "line/temperature"
    );
    for topic in ["", " \t ", "line/+", "line/#", "line/\nvalue", "line/\0value"] {
        assert!(
            parse_point_topic("input_points", topic).is_err(),
            "invalid exact topic was accepted: {topic:?}"
        );
    }
}

#[test]
fn mqtt_point_contract_metric_names_are_trimmed_and_control_free() {
    assert_eq!(
        parse_metric_name(" drive/speed ")
            .expect("valid metric name")
            .as_str(),
        "drive/speed"
    );
    for metric in ["", " \t ", "drive\nspeed", "drive\0speed"] {
        assert!(parse_metric_name(metric).is_err(), "{metric:?}");
    }
}

#[test]
fn mqtt_point_contract_bool_and_numeric_shapes_are_disjoint() {
    for bit in 0..=7 {
        validate_point_shape("input_points", MqttPointType::Bool, Some(bit))
            .expect("valid bool bit");
    }
    assert!(validate_point_shape("input_points", MqttPointType::Bool, None).is_err());
    assert!(validate_point_shape("input_points", MqttPointType::Bool, Some(8)).is_err());
    for data_type in [
        MqttPointType::U16,
        MqttPointType::I16,
        MqttPointType::U32,
        MqttPointType::I32,
        MqttPointType::F32,
    ] {
        validate_point_shape("output_points", data_type, None).expect("numeric point");
        assert!(validate_point_shape("output_points", data_type, Some(0)).is_err());
    }
}

#[test]
fn mqtt_point_contract_scale_offset_are_finite_nonzero_and_numeric_only() {
    assert_eq!(
        parse_scale_offset("input_points", MqttPointType::I16, None, None)
            .expect("defaults"),
        (1.0, 0.0)
    );
    assert_eq!(
        parse_scale_offset(
            "input_points",
            MqttPointType::F32,
            Some(-0.5),
            Some(10.0)
        )
        .expect("finite scaling"),
        (-0.5, 10.0)
    );
    for scale in [0.0, -0.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            parse_scale_offset("input_points", MqttPointType::I16, Some(scale), None).is_err()
        );
    }
    for offset in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            parse_scale_offset("input_points", MqttPointType::I16, None, Some(offset)).is_err()
        );
    }
    assert!(
        parse_scale_offset("input_points", MqttPointType::Bool, Some(2.0), None).is_err()
    );
    assert!(
        parse_scale_offset("input_points", MqttPointType::Bool, None, Some(1.0)).is_err()
    );
}

#[test]
fn mqtt_point_contract_bool_payload_formats_are_scalar_and_exact() {
    for text in [b"true".as_slice(), b" TRUE ", b"1", b"on"] {
        assert!(parse_bool_payload(text, MqttPayloadFormat::Text).expect("true text"));
    }
    for text in [b"false".as_slice(), b" FALSE ", b"0", b"off"] {
        assert!(!parse_bool_payload(text, MqttPayloadFormat::Text).expect("false text"));
    }
    for json in [b"true".as_slice(), b"1", br#""on""#] {
        assert!(parse_bool_payload(json, MqttPayloadFormat::Json).expect("true JSON"));
    }
    for json in [b"false".as_slice(), b"0", br#""off""#] {
        assert!(!parse_bool_payload(json, MqttPayloadFormat::Json).expect("false JSON"));
    }
    assert!(parse_bool_payload(&[1], MqttPayloadFormat::BinaryLe).expect("binary true"));
    for payload in [&[][..], &[0, 1][..], &[1, 0][..]] {
        assert!(
            parse_bool_payload(payload, MqttPayloadFormat::BinaryLe).is_err(),
            "non-scalar binary bool accepted: {payload:?}"
        );
    }
    for malformed in [b"maybe".as_slice(), b"{}".as_slice(), b"[]".as_slice()] {
        assert!(parse_bool_payload(malformed, MqttPayloadFormat::Json).is_err());
    }
}

#[test]
fn mqtt_point_contract_numeric_text_and_json_require_one_finite_scalar() {
    assert_eq!(
        parse_numeric_payload(b" -12.5 ", MqttPayloadFormat::Text, MqttPointType::F32)
            .expect("text scalar"),
        -12.5
    );
    assert_eq!(
        parse_numeric_payload(br#""42""#, MqttPayloadFormat::Json, MqttPointType::I16)
            .expect("JSON numeric string"),
        42.0
    );
    assert_eq!(
        parse_numeric_payload(b"42", MqttPayloadFormat::Json, MqttPointType::U16)
            .expect("JSON number"),
        42.0
    );
    for payload in [
        b"1 trailing".as_slice(),
        b"NaN".as_slice(),
        b"Infinity".as_slice(),
        b"{}".as_slice(),
        b"[]".as_slice(),
        b"true".as_slice(),
    ] {
        assert!(
            parse_numeric_payload(payload, MqttPayloadFormat::Json, MqttPointType::F32)
                .is_err()
        );
    }
}

#[test]
fn mqtt_point_contract_binary_numeric_width_and_endianness_are_exact() {
    let cases = [
        (MqttPointType::U16, 0x1234_u16.to_le_bytes().to_vec(), 4660.0),
        (
            MqttPointType::I16,
            (-1234_i16).to_le_bytes().to_vec(),
            -1234.0,
        ),
        (
            MqttPointType::U32,
            0x1234_5678_u32.to_le_bytes().to_vec(),
            305_419_896.0,
        ),
        (
            MqttPointType::I32,
            (-123_456_i32).to_le_bytes().to_vec(),
            -123_456.0,
        ),
        (
            MqttPointType::F32,
            12.5_f32.to_le_bytes().to_vec(),
            12.5,
        ),
    ];
    for (data_type, little, expected) in cases {
        assert_eq!(
            parse_numeric_payload(&little, MqttPayloadFormat::BinaryLe, data_type)
                .expect("little endian"),
            expected
        );
        let mut big = little.clone();
        big.reverse();
        assert_eq!(
            parse_numeric_payload(&big, MqttPayloadFormat::BinaryBe, data_type)
                .expect("big endian"),
            expected
        );
        let mut extra = little.clone();
        extra.push(0);
        assert!(
            parse_numeric_payload(&extra, MqttPayloadFormat::BinaryLe, data_type).is_err()
        );
        assert!(
            parse_numeric_payload(&little[..little.len() - 1], MqttPayloadFormat::BinaryLe, data_type)
                .is_err()
        );
    }
}

#[test]
fn mqtt_point_contract_decode_scaling_and_image_encoding_are_atomic() {
    let mut point = mqtt_contract_point("line/in", "i16", Some("text"), 1, None);
    point.scale = Some(0.5);
    point.offset = Some(-10.0);
    let point = MqttInputPoint::from_toml(point).expect("mapped input");
    let mut image = [0xA5; 4];
    decode_mqtt_point(&point, b"100", &mut image).expect("decode mapped input");
    assert_eq!(image[0], 0xA5);
    assert_eq!(i16::from_le_bytes([image[1], image[2]]), 40);
    assert_eq!(image[3], 0xA5);

    let before = image;
    assert!(decode_mqtt_point(&point, b"999999", &mut image).is_err());
    assert_eq!(image, before);
}

#[test]
fn mqtt_point_contract_output_inverse_scaling_preserves_declared_format() {
    let mut point = mqtt_contract_point("line/out", "i16", Some("json"), 0, None);
    point.scale = Some(0.5);
    point.offset = Some(-10.0);
    point.metric_name = Some("line/speed".to_string());
    let point = MqttOutputPoint::from_toml(point).expect("mapped output");
    let image = 40_i16.to_le_bytes();
    assert_eq!(
        encode_mqtt_point(&point, &image).expect("encode mapped output"),
        b"100"
    );
    assert_eq!(point.metric_name.as_str(), "line/speed");
}

#[test]
fn mqtt_point_contract_integer_rounding_is_ties_away_and_range_checked() {
    assert_eq!(checked_i16(1.49).expect("round down"), 1);
    assert_eq!(checked_i16(1.5).expect("positive tie"), 2);
    assert_eq!(checked_i16(-1.5).expect("negative tie"), -2);
    assert_eq!(checked_u16(65_535.49).expect("rounded max"), u16::MAX);
    assert!(checked_u16(-0.5).is_err());
    assert!(checked_u16(65_535.5).is_err());
    assert!(checked_i16(i16::MIN as f64 - 0.5).is_err());
    assert!(checked_i16(i16::MAX as f64 + 0.5).is_err());
}

#[test]
fn mqtt_point_contract_image_ranges_fail_without_partial_mutation() {
    let mut image = [0xA5; 3];
    let before = image;
    assert!(
        write_image_numeric(&mut image, 1, MqttPointType::U32, 1.0).is_err()
    );
    assert_eq!(image, before);
    assert!(read_image_numeric(&image, usize::MAX, MqttPointType::U16).is_err());
    assert!(
        write_image_bool(&mut image, usize::MAX, Some(0), true).is_err()
    );
    assert_eq!(image, before);
}

#[test]
fn mqtt_point_contract_duplicate_topics_and_overlapping_image_ranges_are_rejected() {
    for invalid in [
        r#"
broker = "127.0.0.1:1883"
[[input_points]]
topic = "line/value"
image_offset = 0
data_type = "u16"
[[input_points]]
topic = "line/value"
image_offset = 2
data_type = "u16"
"#,
        r#"
broker = "127.0.0.1:1883"
[[output_points]]
topic = "line/first"
image_offset = 0
data_type = "u32"
[[output_points]]
topic = "line/second"
image_offset = 2
data_type = "u16"
"#,
        r#"
broker = "127.0.0.1:1883"
[[input_points]]
topic = "line/first"
image_offset = 0
image_bit = 3
data_type = "bool"
[[input_points]]
topic = "line/second"
image_offset = 0
image_bit = 3
data_type = "bool"
"#,
    ] {
        match mqtt_contract_config(invalid) {
            Ok(_) => panic!("ambiguous MQTT point map was accepted"),
            Err(error) => assert!(
                error.to_string().contains("duplicate")
                    || error.to_string().contains("overlap"),
                "unexpected point-map diagnostic: {error}"
            ),
        }
    }
}
