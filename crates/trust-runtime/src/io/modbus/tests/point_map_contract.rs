fn modbus_contract_point(
    address: u16,
    data_type: Option<&str>,
    function: Option<&str>,
    image_offset: usize,
    image_bit: Option<u8>,
) -> ModbusPointToml {
    ModbusPointToml {
        image_offset,
        image_bit,
        address,
        function: function.map(str::to_string),
        data_type: data_type.map(str::to_string),
        scale: None,
        offset: None,
        byte_order: None,
        word_order: None,
    }
}

fn modbus_contract_config(text: &str) -> Result<super::super::ModbusTcpConfig, RuntimeError> {
    let value: toml::Value = toml::from_str(text).expect("parse Modbus contract TOML");
    super::super::ModbusTcpConfig::from_params(&value)
}

#[test]
fn modbus_point_contract_type_aliases_and_register_widths_are_exact() {
    for (alias, expected, registers) in [
        ("bool", ModbusPointType::Bool, 0),
        ("BOOLEAN", ModbusPointType::Bool, 0),
        ("coil", ModbusPointType::Bool, 0),
        ("u16", ModbusPointType::U16, 1),
        ("UINT16", ModbusPointType::U16, 1),
        ("word", ModbusPointType::U16, 1),
        ("i16", ModbusPointType::I16, 1),
        ("INT", ModbusPointType::I16, 1),
        ("u32", ModbusPointType::U32, 2),
        ("DWORD", ModbusPointType::U32, 2),
        ("i32", ModbusPointType::I32, 2),
        ("DINT", ModbusPointType::I32, 2),
        ("f32", ModbusPointType::F32, 2),
        ("REAL", ModbusPointType::F32, 2),
    ] {
        let parsed = ModbusPointType::parse(alias).expect("accepted type alias");
        assert_eq!(parsed, expected, "{alias}");
        assert_eq!(parsed.register_count(), registers, "{alias}");
    }
    assert!(ModbusPointType::parse("f64").is_err());
}

#[test]
fn modbus_point_contract_defaults_follow_selected_function_family() {
    assert_eq!(
        parse_point_type_or_default(None, ModbusInputFunction::Coils).expect("coil default"),
        ModbusPointType::Bool
    );
    assert_eq!(
        parse_point_type_or_default(None, ModbusInputFunction::InputRegisters)
            .expect("register default"),
        ModbusPointType::U16
    );
    assert_eq!(
        parse_output_type_or_default(None, ModbusOutputFunction::SingleCoil)
            .expect("coil output default"),
        ModbusPointType::Bool
    );
    assert_eq!(
        parse_output_type_or_default(None, ModbusOutputFunction::MultipleRegisters)
            .expect("register output default"),
        ModbusPointType::U16
    );
}

#[test]
fn modbus_point_contract_function_and_type_families_cannot_cross() {
    for (function, data_type) in [
        ("read_coils", "u16"),
        ("read_discrete_inputs", "f32"),
        ("read_holding_registers", "bool"),
        ("read_input_registers", "boolean"),
    ] {
        assert!(
            ModbusInputPoint::from_toml(
                modbus_contract_point(0, Some(data_type), Some(function), 0, Some(0)),
                ModbusInputFunction::InputRegisters,
            )
            .is_err(),
            "{function}/{data_type}"
        );
    }
    for (function, data_type) in [
        ("write_single_coil", "u16"),
        ("write_multiple_coils", "i16"),
        ("write_single_register", "bool"),
        ("write_multiple_registers", "boolean"),
    ] {
        assert!(
            ModbusOutputPoint::from_toml(
                modbus_contract_point(0, Some(data_type), Some(function), 0, Some(0)),
                ModbusOutputFunction::MultipleRegisters,
            )
            .is_err(),
            "{function}/{data_type}"
        );
    }
}

#[test]
fn modbus_point_contract_single_register_rejects_two_register_scalars() {
    for data_type in ["u32", "i32", "f32"] {
        assert!(
            ModbusOutputPoint::from_toml(
                modbus_contract_point(
                    0,
                    Some(data_type),
                    Some("write_single_register"),
                    0,
                    None,
                ),
                ModbusOutputFunction::MultipleRegisters,
            )
            .is_err(),
            "{data_type}"
        );
    }
    for data_type in ["u16", "i16"] {
        ModbusOutputPoint::from_toml(
            modbus_contract_point(
                0,
                Some(data_type),
                Some("write_single_register"),
                0,
                None,
            ),
            ModbusOutputFunction::MultipleRegisters,
        )
        .expect("one-register scalar");
    }
}

#[test]
fn modbus_point_contract_bit_scale_and_offset_validation_is_bounded() {
    for bit in 0..=7 {
        assert_eq!(parse_image_bit(Some(bit)).expect("valid bit"), bit);
    }
    assert_eq!(parse_image_bit(None).expect("default bit"), 0);
    assert!(parse_image_bit(Some(8)).is_err());

    assert_eq!(parse_scale(None).expect("default scale"), 1.0);
    assert_eq!(parse_offset(None).expect("default offset"), 0.0);
    for scale in [0.0, -0.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(parse_scale(Some(scale)).is_err());
    }
    for offset in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(parse_offset(Some(offset)).is_err());
    }
}

#[test]
fn modbus_point_contract_byte_and_word_order_aliases_are_case_insensitive() {
    for alias in [None, Some("big"), Some("BE"), Some("big-endian")] {
        assert_eq!(
            parse_byte_order(alias).expect("big byte order"),
            ModbusByteOrder::Big
        );
        assert_eq!(
            parse_word_order(alias).expect("big word order"),
            ModbusWordOrder::Big
        );
    }
    for alias in [Some("little"), Some("LE"), Some("little_endian")] {
        assert_eq!(
            parse_byte_order(alias).expect("little byte order"),
            ModbusByteOrder::Little
        );
        assert_eq!(
            parse_word_order(alias).expect("little word order"),
            ModbusWordOrder::Little
        );
    }
    assert!(parse_byte_order(Some("native")).is_err());
    assert!(parse_word_order(Some("native")).is_err());
}

#[test]
fn modbus_point_contract_u32_endian_cross_product_round_trips() {
    let expected_by_order = [
        (
            ModbusByteOrder::Big,
            ModbusWordOrder::Big,
            vec![0x11, 0x22, 0x33, 0x44],
        ),
        (
            ModbusByteOrder::Little,
            ModbusWordOrder::Big,
            vec![0x22, 0x11, 0x44, 0x33],
        ),
        (
            ModbusByteOrder::Big,
            ModbusWordOrder::Little,
            vec![0x33, 0x44, 0x11, 0x22],
        ),
        (
            ModbusByteOrder::Little,
            ModbusWordOrder::Little,
            vec![0x44, 0x33, 0x22, 0x11],
        ),
    ];
    for (byte_order, word_order, expected) in expected_by_order {
        let encoded = encode_modbus_numeric(
            ModbusPointType::U32,
            0x1122_3344_u32 as f64,
            byte_order,
            word_order,
        )
        .expect("encode ordered U32");
        assert_eq!(encoded, expected);
        assert_eq!(
            decode_modbus_numeric(ModbusPointType::U32, &encoded, byte_order, word_order)
                .expect("decode ordered U32"),
            0x1122_3344_u32 as f64
        );
    }
}

#[test]
fn modbus_point_contract_register_decode_requires_exact_declared_width() {
    assert_eq!(
        decode_modbus_numeric(
            ModbusPointType::U16,
            &[0x12, 0x34],
            ModbusByteOrder::Big,
            ModbusWordOrder::Big,
        )
        .expect("exact U16"),
        0x1234 as f64
    );
    for wire in [
        &[0x12][..],
        &[0x12, 0x34, 0x56][..],
        &[0x12, 0x34, 0x56, 0x78][..],
    ] {
        assert!(
            decode_modbus_numeric(
                ModbusPointType::U16,
                wire,
                ModbusByteOrder::Big,
                ModbusWordOrder::Big,
            )
            .is_err(),
            "non-exact U16 wire accepted: {wire:?}"
        );
    }
}

#[test]
fn modbus_point_contract_process_image_numeric_is_little_endian() {
    let mut image = [0xA5; 8];
    write_image_numeric(&mut image, 2, ModbusPointType::I32, -123_456.0)
        .expect("write process image");
    assert_eq!(&image[2..6], &(-123_456_i32).to_le_bytes());
    assert_eq!(
        read_image_numeric(&image, 2, ModbusPointType::I32).expect("read process image"),
        -123_456.0
    );
    assert_eq!(&image[..2], &[0xA5; 2]);
    assert_eq!(&image[6..], &[0xA5; 2]);
}

#[test]
fn modbus_point_contract_boolean_write_preserves_neighbor_bits() {
    let mut image = [0b1010_0101];
    write_image_bool(&mut image, 0, 1, true).expect("set bit");
    assert_eq!(image, [0b1010_0111]);
    write_image_bool(&mut image, 0, 2, false).expect("clear bit");
    assert_eq!(image, [0b1010_0011]);
    assert!(read_image_bool(&image, 0, 1).expect("read set bit"));
    assert!(!read_image_bool(&image, 0, 2).expect("read clear bit"));
}

#[test]
fn modbus_point_contract_integer_rounding_is_ties_away_and_range_checked() {
    assert_eq!(
        encode_modbus_numeric(
            ModbusPointType::I16,
            1.5,
            ModbusByteOrder::Big,
            ModbusWordOrder::Big,
        )
        .expect("positive tie"),
        2_i16.to_be_bytes()
    );
    assert_eq!(
        encode_modbus_numeric(
            ModbusPointType::I16,
            -1.5,
            ModbusByteOrder::Big,
            ModbusWordOrder::Big,
        )
        .expect("negative tie"),
        (-2_i16).to_be_bytes()
    );
    for value in [-0.5, u16::MAX as f64 + 0.5] {
        assert!(
            encode_modbus_numeric(
                ModbusPointType::U16,
                value,
                ModbusByteOrder::Big,
                ModbusWordOrder::Big,
            )
            .is_err()
        );
    }
}

#[test]
fn modbus_point_contract_image_range_failure_is_atomic() {
    let mut image = [0xA5; 3];
    let before = image;
    assert!(
        write_image_numeric(&mut image, 1, ModbusPointType::U32, 42.0).is_err()
    );
    assert_eq!(image, before);
    assert!(read_image_numeric(&image, usize::MAX, ModbusPointType::U16).is_err());
    assert!(write_image_bool(&mut image, usize::MAX, 0, true).is_err());
    assert_eq!(image, before);
}

#[test]
fn modbus_point_contract_duplicate_protocol_and_image_ranges_are_rejected() {
    for invalid in [
        r#"
address = "127.0.0.1:502"
[[input_points]]
address = 10
image_offset = 0
data_type = "u16"
[[input_points]]
address = 10
image_offset = 2
data_type = "u16"
"#,
        r#"
address = "127.0.0.1:502"
[[output_points]]
address = 10
image_offset = 0
data_type = "u32"
[[output_points]]
address = 12
image_offset = 2
data_type = "u16"
"#,
        r#"
address = "127.0.0.1:502"
[[input_points]]
address = 10
image_offset = 0
image_bit = 2
function = "read_coils"
data_type = "bool"
[[input_points]]
address = 11
image_offset = 0
image_bit = 2
function = "read_coils"
data_type = "bool"
"#,
    ] {
        match modbus_contract_config(invalid) {
            Ok(_) => panic!("ambiguous Modbus point map was accepted"),
            Err(error) => assert!(
                error.to_string().contains("duplicate")
                    || error.to_string().contains("overlap"),
                "unexpected point-map diagnostic: {error}"
            ),
        }
    }
}
