//! The computer's reading of the `modbus-tcp` parameters in `io.toml` (specification 11 §6.5, the
//! Modbus/TCP driver and the "Typed protocol point-map integration contract"). The TOML table
//! reaches the shared rules as the JSON text that a chip reads (specification 34 §5.1.1, section
//! 8), so a computer takes and refuses what a chip does, and a refusal is the shared rules'
//! `<key>: <reason>` after the driver's name.
//!
//! These are the computer's point-map contract tests, moved onto the shared rules. Their expected
//! texts are the shared rules' words, which replace the computer's own (`io.params...`) because
//! the driver's logic is now one implementation on every runtime (specification 11 §6.5, "The
//! protocol is one implementation on every runtime"; specification 34 ARCH-11). The conversions
//! on the wire are proven by the shared module's tests and, through a server, by
//! `tests/modbus_driver.rs`.

use trust_runtime_services::io_drivers::modbus::{
    Endian, InputFunction, ModbusTcpParams, OutputFunction, PointType,
};

use super::ModbusTcpDriver;
use crate::error::RuntimeError;

/// The parameters of the `io.toml` table `text`, as the computer reads them.
fn read(text: &str) -> Result<ModbusTcpParams, RuntimeError> {
    let value: toml::Value = toml::from_str(text).expect("a TOML table");
    ModbusTcpDriver::read_params(&value)
}

/// The refusal of `key` for `reason`, after the driver's name.
fn refused(key: &str, reason: &str) -> Result<ModbusTcpParams, RuntimeError> {
    Err(RuntimeError::InvalidConfig(
        format!("modbus-tcp: {key}: {reason}").into(),
    ))
}

/// The table of the server `127.0.0.1:502` with one input point, whose keys are `fields`.
fn input_point(fields: &str) -> Result<ModbusTcpParams, RuntimeError> {
    read(&format!(
        "address = \"127.0.0.1:502\"\n[[input_points]]\n{fields}\n"
    ))
}

/// The table of the server `127.0.0.1:502` with one output point, whose keys are `fields`.
fn output_point(fields: &str) -> Result<ModbusTcpParams, RuntimeError> {
    read(&format!(
        "address = \"127.0.0.1:502\"\n[[output_points]]\n{fields}\n"
    ))
}

/// "`boolean`, `word`, `int`, `dword`, `dint`, and `real` and explicit-width aliases select the same
/// type case-insensitively", with one register for a 16-bit type and two for a 32-bit one; `f64`
/// is no type. Catches an alias that the computer reads otherwise than a chip, and a 32-bit value
/// read as one register.
#[test]
fn point_types_and_their_aliases_select_their_widths() {
    for (alias, data_type, registers) in [
        ("bool", PointType::Bool, 0),
        ("BOOLEAN", PointType::Bool, 0),
        ("coil", PointType::Bool, 0),
        ("u16", PointType::U16, 1),
        ("UINT16", PointType::U16, 1),
        ("word", PointType::U16, 1),
        ("i16", PointType::I16, 1),
        ("INT", PointType::I16, 1),
        ("u32", PointType::U32, 2),
        ("DWORD", PointType::U32, 2),
        ("i32", PointType::I32, 2),
        ("DINT", PointType::I32, 2),
        ("f32", PointType::F32, 2),
        ("REAL", PointType::F32, 2),
    ] {
        let function = if data_type == PointType::Bool {
            "read_coils"
        } else {
            "read_holding_registers"
        };
        let params = input_point(&format!(
            "image_offset = 0\naddress = 0\nfunction = \"{function}\"\ndata_type = \"{alias}\""
        ))
        .unwrap_or_else(|error| panic!("{alias}: {error}"));
        let point = &params.input_points[0];
        assert_eq!(point.data_type, data_type, "{alias}");
        assert_eq!(point.data_type.registers(), registers, "{alias}");
    }
    assert_eq!(
        input_point("image_offset = 0\naddress = 0\ndata_type = \"f64\""),
        refused(
            "input_points[0].data_type",
            r#"must be bool, u16, i16, u32, i32 or f32, not "f64""#
        )
    );
}

/// "Omitted type defaults to Boolean for bit functions and U16 for register functions", and a point
/// without a function takes the driver's, FC04 for the inputs and FC16 for the outputs by default.
/// Catches a register point read as a coil, and a point that ignores the driver's function.
#[test]
fn a_point_takes_the_defaults_of_its_function() {
    let inputs = |fields: &str| {
        let params = input_point(fields).expect("a valid point");
        let point = &params.input_points[0];
        (point.function, point.data_type)
    };
    assert_eq!(
        inputs("image_offset = 0\naddress = 0\nfunction = \"read_coils\""),
        (InputFunction::Coils, PointType::Bool)
    );
    assert_eq!(
        inputs("image_offset = 0\naddress = 0"),
        (InputFunction::InputRegisters, PointType::U16)
    );
    let outputs = |fields: &str| {
        let params = output_point(fields).expect("a valid point");
        let point = &params.output_points[0];
        (point.function, point.data_type)
    };
    assert_eq!(
        outputs("image_offset = 0\naddress = 0\nfunction = \"write_single_coil\""),
        (OutputFunction::SingleCoil, PointType::Bool)
    );
    assert_eq!(
        outputs("image_offset = 0\naddress = 0"),
        (OutputFunction::MultipleRegisters, PointType::U16)
    );
}

/// "FC01/FC02 points are Boolean and FC03/FC04 points are numeric; FC05/FC15 outputs are Boolean
/// and FC06/FC16 outputs are numeric", and "FC06 accepts only one-register U16/I16 values". Catches
/// a register read as a coil, and an FC06 that would send half of a 32-bit value.
#[test]
fn a_point_type_outside_its_function_family_is_refused() {
    let coil = r#"coil/discrete input mappings require data_type = "bool", not "#;
    let numeric = "register mappings require a numeric data_type, not ";
    for (function, data_type, rule) in [
        ("read_coils", "u16", coil),
        ("read_discrete_inputs", "f32", coil),
        ("read_holding_registers", "bool", numeric),
        ("read_input_registers", "boolean", numeric),
    ] {
        assert_eq!(
            input_point(&format!(
                "image_offset = 0\naddress = 0\nfunction = \"{function}\"\ndata_type = \"{data_type}\""
            )),
            refused(
                "input_points[0].data_type",
                &format!("{rule}\"{data_type}\"")
            ),
            "{function}/{data_type}"
        );
    }
    let coil = r#"coil mappings require data_type = "bool", not "#;
    let single = "write_single_register supports only 16-bit data types, not ";
    for (function, data_type, rule) in [
        ("write_single_coil", "u16", coil),
        ("write_multiple_coils", "i16", coil),
        ("write_single_register", "bool", numeric),
        ("write_multiple_registers", "boolean", numeric),
        ("write_single_register", "u32", single),
        ("write_single_register", "i32", single),
        ("write_single_register", "f32", single),
    ] {
        assert_eq!(
            output_point(&format!(
                "image_offset = 0\naddress = 0\nfunction = \"{function}\"\ndata_type = \"{data_type}\""
            )),
            refused(
                "output_points[0].data_type",
                &format!("{rule}\"{data_type}\"")
            ),
            "{function}/{data_type}"
        );
    }
    for data_type in ["u16", "i16"] {
        output_point(&format!(
            "image_offset = 0\naddress = 0\nfunction = \"write_single_register\"\ndata_type = \"{data_type}\""
        ))
        .unwrap_or_else(|error| panic!("{data_type}: {error}"));
    }
}

/// A bit index is `0..=7`, 0 by default; "Scale must be finite and non-zero and offset must be
/// finite", 1 and 0 by default. TOML writes a float that JSON cannot, `nan` and `inf`, which the
/// computer refuses with its key before the shared rules read the table (specification 34 §5.1.1,
/// section 8: such a float "has no JSON form"). Catches a ninth bit of a byte, a scale of zero that
/// cannot be inverted, and a NaN or an infinity taken as a scale or an offset.
#[test]
fn bits_scales_and_offsets_stay_in_their_ranges() {
    for bit in 0..=7 {
        let params = input_point(&format!(
            "image_offset = 0\nimage_bit = {bit}\naddress = 0\nfunction = \"read_coils\""
        ))
        .expect("a bit from 0 to 7");
        assert_eq!(params.input_points[0].image_bit, bit);
    }
    assert_eq!(
        input_point("image_offset = 0\nimage_bit = 8\naddress = 0\nfunction = \"read_coils\""),
        refused(
            "input_points[0].image_bit",
            "must be an integer from 0 to 7, not 8"
        )
    );
    let params = input_point("image_offset = 0\naddress = 0").expect("a valid point");
    let point = &params.input_points[0];
    assert_eq!((point.image_bit, point.scale, point.offset), (0, 1.0, 0.0));
    for given in ["0.0", "-0.0"] {
        assert_eq!(
            input_point(&format!("image_offset = 0\naddress = 0\nscale = {given}")),
            refused(
                "input_points[0].scale",
                &format!("must be a finite, non-zero number, not {given}")
            )
        );
    }
    for (given, shown) in [("nan", "NaN"), ("inf", "inf"), ("-inf", "-inf")] {
        for key in ["scale", "offset"] {
            assert_eq!(
                input_point(&format!("image_offset = 0\naddress = 0\n{key} = {given}")),
                refused(
                    &format!("input_points[0].{key}"),
                    &format!("{shown} is not a finite number")
                ),
                "{key} = {given}"
            );
        }
    }
}

/// "Byte order applies inside every 16-bit register and word order applies between the two
/// registers of a 32-bit value; both default to big-endian and accept the documented `big`/`be`
/// and `little`/`le` aliases", in any case and with `-` as `_`. Catches an alias refused, and an
/// unknown order taken as big.
#[test]
fn byte_and_word_orders_take_their_aliases() {
    for (alias, order) in [
        ("big", Endian::Big),
        ("BE", Endian::Big),
        ("big-endian", Endian::Big),
        ("little", Endian::Little),
        ("LE", Endian::Little),
        ("little_endian", Endian::Little),
    ] {
        let params = input_point(&format!(
            "image_offset = 0\naddress = 0\nbyte_order = \"{alias}\"\nword_order = \"{alias}\""
        ))
        .unwrap_or_else(|error| panic!("{alias}: {error}"));
        let point = &params.input_points[0];
        assert_eq!(
            (point.byte_order, point.word_order),
            (order, order),
            "{alias}"
        );
    }
    for key in ["byte_order", "word_order"] {
        assert_eq!(
            input_point(&format!(
                "image_offset = 0\naddress = 0\n{key} = \"native\""
            )),
            refused(
                &format!("input_points[0].{key}"),
                r#"must be big or little, not "native""#
            )
        );
    }
}

/// "Within one input or output map, two point definitions must not claim the same protocol
/// identity or overlapping process-image storage": two `u16` at input register 10; a `u16` at
/// image byte 2 inside the `u32` of bytes 0 to 3; two coils in bit 2 of byte 0; and, as the
/// ranges "account for the ... function/table", FC05 and FC15 at coil 7, one table. The computer
/// compared functions, so it took both; the shared rules compare tables. Catches two points that
/// write one register or one bit over each other.
#[test]
fn points_that_overlap_are_refused() {
    let address_rule = "duplicate or overlapping Modbus address ranges are refused";
    let image_rule = "overlapping process-image ranges are refused";
    assert_eq!(
        read(concat!(
            "address = \"127.0.0.1:502\"\n",
            "[[input_points]]\naddress = 10\nimage_offset = 0\ndata_type = \"u16\"\n",
            "[[input_points]]\naddress = 10\nimage_offset = 2\ndata_type = \"u16\"\n",
        )),
        refused(
            "input_points[1].address",
            &format!(
                "input register 10 overlaps input register 10 of input_points[0]; {address_rule}"
            )
        )
    );
    assert_eq!(
        read(concat!(
            "address = \"127.0.0.1:502\"\n",
            "[[output_points]]\naddress = 10\nimage_offset = 0\ndata_type = \"u32\"\n",
            "[[output_points]]\naddress = 12\nimage_offset = 2\ndata_type = \"u16\"\n",
        )),
        refused(
            "output_points[1].image_offset",
            &format!("%QW2 overlaps %QD0 of output_points[0]; {image_rule}")
        )
    );
    assert_eq!(
        read(concat!(
            "address = \"127.0.0.1:502\"\n",
            "[[input_points]]\naddress = 10\nimage_offset = 0\nimage_bit = 2\nfunction = \"read_coils\"\n",
            "[[input_points]]\naddress = 11\nimage_offset = 0\nimage_bit = 2\nfunction = \"read_coils\"\n",
        )),
        refused(
            "input_points[1].image_offset",
            &format!("%IX0.2 overlaps %IX0.2 of input_points[0]; {image_rule}")
        )
    );
    assert_eq!(
        read(concat!(
            "address = \"127.0.0.1:502\"\n",
            "[[output_points]]\naddress = 7\nimage_offset = 0\nfunction = \"write_single_coil\"\n",
            "[[output_points]]\naddress = 7\nimage_offset = 0\nimage_bit = 1\nfunction = \"write_multiple_coils\"\n",
        )),
        refused(
            "output_points[1].address",
            &format!("coil 7 overlaps coil 7 of output_points[0]; {address_rule}")
        )
    );
}

/// The root keys by the shared rules, where the computer read them otherwise before: an unknown
/// key of the table or of a point is refused (the computer ignored it), the port and the timeout
/// may not be 0 (the computer took both and then failed every connection), an IPv4 address keeps
/// the shared rules' refusal, and an IPv6 address in brackets is taken, as the computer's socket
/// address took it. Catches a misspelt key ignored, and IPv6 lost on the computer.
#[test]
fn root_keys_follow_the_shared_rules() {
    assert_eq!(
        read("address = \"127.0.0.1:502\"\npoll_ms = 10\n"),
        refused(
            "poll_ms",
            "unknown key; the modbus-tcp driver takes address, input_function, input_points, \
             input_start, on_error, output_function, output_points, output_start, timeout_ms and \
             unit_id"
        )
    );
    assert_eq!(
        input_point("image_offset = 0\naddress = 0\nlength = 2"),
        refused(
            "input_points[0].length",
            "unknown key; a point takes address, byte_order, data_type, function, image_bit, \
             image_offset, offset, scale and word_order"
        )
    );
    let ipv4 = r#"an IPv4 address with a port from 1 to 65535, such as "192.168.1.10:502""#;
    for given in ["127.0.0.1:0", "plc.local:502"] {
        assert_eq!(
            read(&format!("address = \"{given}\"\n")),
            refused("address", &format!("must be {ipv4}, not \"{given}\""))
        );
    }
    assert_eq!(
        read("address = \"127.0.0.1:502\"\ntimeout_ms = 0\n"),
        refused(
            "timeout_ms",
            "must be an integer from 1 to 4294967295, not 0"
        )
    );
    let ipv6 = read("address = \"[::1]:1502\"\n").expect("an IPv6 address in brackets");
    assert_eq!(ipv6.address.to_string(), "[::1]:1502");
}
