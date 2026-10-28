//! The point maps' parameters (specification 11 §6.5, "Typed protocol point-map integration
//! contract"): types and their aliases, defaults, scaling, orders, and the addresses and image bits
//! that the points of one list may not share.

use alloc::format;
use alloc::vec::Vec;

use crate::io_drivers::modbus::{
    Endian, InputFunction, InputPoint, ModbusTcpParams, OutputFunction, PointType,
};
use crate::io_drivers::ParamsError;

/// The refusal of `key` for `reason`.
fn refused(key: &str, reason: &str) -> Result<ModbusTcpParams, ParamsError> {
    Err(ParamsError {
        key: key.into(),
        reason: reason.into(),
    })
}

/// The parameters of a server with the one input point `fields`, the members of a JSON table.
fn input_point(fields: &str) -> Result<ModbusTcpParams, ParamsError> {
    ModbusTcpParams::from_json(&format!(
        r#"{{"address":"10.0.0.5:502","input_points":[{{{fields}}}]}}"#
    ))
}

/// The parameters of a server with the one output point `fields`.
fn output_point(fields: &str) -> Result<ModbusTcpParams, ParamsError> {
    ModbusTcpParams::from_json(&format!(
        r#"{{"address":"10.0.0.5:502","output_points":[{{{fields}}}]}}"#
    ))
}

/// Each type and its aliases, in any case ("`boolean`, `word`, `int`, `dword`, `dint`, and `real`
/// and explicit-width aliases select the same type case-insensitively"), and the computer's `coil`
/// and `float`, with the registers and the image bytes the type takes; `f64` is no type. Catches an
/// alias that one runtime refuses, and a 32-bit type read as one register.
#[test]
fn point_types_and_their_aliases_select_their_widths() {
    for (word, data_type, registers, image) in [
        ("bool", PointType::Bool, 0, 1),
        ("BOOLEAN", PointType::Bool, 0, 1),
        ("coil", PointType::Bool, 0, 1),
        ("u16", PointType::U16, 1, 2),
        ("UINT16", PointType::U16, 1, 2),
        ("Word", PointType::U16, 1, 2),
        ("i16", PointType::I16, 1, 2),
        ("int16", PointType::I16, 1, 2),
        ("INT", PointType::I16, 1, 2),
        ("u32", PointType::U32, 2, 4),
        ("uint32", PointType::U32, 2, 4),
        ("dword", PointType::U32, 2, 4),
        ("i32", PointType::I32, 2, 4),
        ("Int32", PointType::I32, 2, 4),
        ("DINT", PointType::I32, 2, 4),
        ("f32", PointType::F32, 2, 4),
        ("REAL", PointType::F32, 2, 4),
        ("float", PointType::F32, 2, 4),
    ] {
        let function = if data_type == PointType::Bool {
            "read_coils"
        } else {
            "read_holding_registers"
        };
        let params = input_point(&format!(
            r#""image_offset":0,"address":0,"function":"{function}","data_type":"{word}""#
        ))
        .expect("a type of specification 11");
        let point = &params.input_points[0];
        assert_eq!(point.data_type, data_type, "{word}");
        assert_eq!(point.data_type.registers(), registers, "{word}");
        assert_eq!(point.data_type.image_bytes(), image, "{word}");
    }
    assert_eq!(
        input_point(r#""image_offset":0,"address":0,"data_type":"f64""#),
        refused(
            "input_points[0].data_type",
            r#"must be bool, u16, i16, u32, i32 or f32, not "f64""#
        )
    );
}

/// A point without a type is a `bool` for a coil or input function and a `u16` for a register
/// function ("omitted type defaults to Boolean for bit functions and U16 for register functions"),
/// and without a function it takes the driver's; its bit is 0, scale 1, offset 0, orders big.
/// Catches a register point read as a coil and a point that ignores the driver's function.
#[test]
fn a_point_takes_the_defaults_of_its_function() {
    let registers = input_point(r#""image_offset":4,"address":9"#).expect("a valid point");
    assert_eq!(
        registers.input_points,
        [InputPoint {
            image_offset: 4,
            image_bit: 0,
            address: 9,
            function: InputFunction::InputRegisters,
            data_type: PointType::U16,
            scale: 1.0,
            offset: 0.0,
            byte_order: Endian::Big,
            word_order: Endian::Big,
        }]
    );
    let defaults = |text: &str| -> Vec<(InputFunction, PointType)> {
        let params = ModbusTcpParams::from_json(text).expect("a valid point");
        let points = params.input_points.iter();
        points
            .map(|point| (point.function, point.data_type))
            .collect()
    };
    assert_eq!(
        defaults(concat!(
            r#"{"address":"10.0.0.5:502","input_function":"read_coils","#,
            r#""input_points":[{"image_offset":0,"address":0}]}"#
        )),
        [(InputFunction::Coils, PointType::Bool)]
    );
    let coils = ModbusTcpParams::from_json(concat!(
        r#"{"address":"10.0.0.5:502","output_function":"coil","#,
        r#""output_points":[{"image_offset":0,"address":0}]}"#
    ))
    .expect("a valid point");
    let point = &coils.output_points[0];
    assert_eq!(
        (point.function, point.data_type),
        (OutputFunction::SingleCoil, PointType::Bool)
    );
    let registers = output_point(r#""image_offset":0,"address":0"#).expect("a valid point");
    let point = &registers.output_points[0];
    assert_eq!(
        (point.function, point.data_type),
        (OutputFunction::MultipleRegisters, PointType::U16)
    );
}

/// A type that does not fit its function is refused, with the computer's words: FC01 and FC02
/// points are `bool`, FC03 and FC04 points are numbers, FC05 and FC15 outputs are `bool`, FC06 and
/// FC16 outputs are numbers, and FC06 takes one register only ("FC06 accepts only one-register
/// U16/I16 values"). Catches a register read as a coil, and an FC06 that would send half of a
/// 32-bit value.
#[test]
fn a_type_that_does_not_fit_its_function_is_refused() {
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
                r#""image_offset":0,"address":0,"function":"{function}","data_type":"{data_type}""#
            )),
            refused(
                "input_points[0].data_type",
                &format!(r#"{rule}"{data_type}""#)
            )
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
                r#""image_offset":0,"address":0,"function":"{function}","data_type":"{data_type}""#
            )),
            refused(
                "output_points[0].data_type",
                &format!(r#"{rule}"{data_type}""#)
            )
        );
    }
    for data_type in ["u16", "i16"] {
        assert!(output_point(&format!(
            r#""image_offset":0,"address":0,"function":"write_single_register","data_type":"{data_type}""#
        ))
        .is_ok());
    }
}

/// Scaling is finite and non-zero, an offset finite, and a `bool` point takes neither ("Boolean
/// points do not accept scaling"); a tiny scale is not zero. The other keys of a point are checked
/// as the root's. Catches scaling ignored on a coil, a zero scale that cannot be inverted, a scale
/// such as 1e-20 refused although it is not zero, and a ninth bit of a byte.
#[test]
fn a_point_key_that_breaks_its_rule_is_refused() {
    let coil = r#""image_offset":0,"address":0,"function":"read_coils","#;
    let reason = "is not taken by a bool point: boolean points do not accept scaling";
    assert_eq!(
        input_point(&format!(r#"{coil}"scale":1"#)),
        refused("input_points[0].scale", reason)
    );
    assert_eq!(
        input_point(&format!(r#"{coil}"offset":0"#)),
        refused("input_points[0].offset", reason)
    );
    let register = r#""image_offset":0,"address":0,"#;
    for given in ["0", "-0.0", r#""2""#] {
        assert_eq!(
            input_point(&format!("{register}\"scale\":{given}")),
            refused(
                "input_points[0].scale",
                &format!("must be a finite, non-zero number, not {given}")
            )
        );
    }
    assert_eq!(
        input_point(&format!(r#"{register}"offset":"x""#)),
        refused(
            "input_points[0].offset",
            r#"must be a finite number, not "x""#
        )
    );
    let tiny = input_point(&format!(r#"{register}"scale":1e-20"#)).expect("1e-20 is not zero");
    assert!(tiny.input_points[0].scale > 0.0);
    assert_eq!(
        input_point(&format!(r#"{coil}"image_bit":8"#)),
        refused(
            "input_points[0].image_bit",
            "must be an integer from 0 to 7, not 8"
        )
    );
    assert_eq!(
        input_point(r#""address":0"#),
        refused(
            "input_points[0].image_offset",
            "is required, an integer from 0 to 4294967295"
        )
    );
    assert_eq!(
        input_point(r#""image_offset":0,"address":65536"#),
        refused(
            "input_points[0].address",
            "must be an integer from 0 to 65535, not 65536"
        )
    );
    assert_eq!(
        input_point(&format!(r#"{register}"length":2"#)),
        refused(
            "input_points[0].length",
            "unknown key; a point takes address, byte_order, data_type, function, image_bit, \
             image_offset, offset, scale and word_order"
        )
    );
    for (word, order) in [
        ("BE", Endian::Big),
        ("big-endian", Endian::Big),
        ("le", Endian::Little),
        ("Little_Endian", Endian::Little),
    ] {
        let params = input_point(&format!(
            r#"{register}"byte_order":"{word}","word_order":"{word}""#
        ))
        .expect("an order of specification 11");
        let point = &params.input_points[0];
        assert_eq!(
            (point.byte_order, point.word_order),
            (order, order),
            "{word}"
        );
    }
    assert_eq!(
        input_point(&format!(r#"{register}"word_order":"native""#)),
        refused(
            "input_points[0].word_order",
            r#"must be big or little, not "native""#
        )
    );
}

/// Two points of one list may not take one address of one table: a `u32` at holding register 10
/// takes 10 and 11, so a `u16` at 11 is refused, and FC05 and FC15 both write the coils, so they may
/// not share coil 7 ("register ranges account for the type's complete one- or two-register width
/// and the selected function/table"); holding and input register 10 are different tables. A
/// 32-bit value at 65535 runs past the last address, a `u16` or a coil there does not. Catches an
/// overlap compared per function instead of per table, a 32-bit value counted as one register, and
/// the last address refused.
#[test]
fn points_of_one_list_do_not_share_an_address_of_one_table() {
    let rule = "duplicate or overlapping Modbus address ranges are refused";
    assert_eq!(
        input_point(concat!(
            r#""image_offset":0,"address":10,"function":"holding","data_type":"u32"},"#,
            r#"{"image_offset":4,"address":11,"function":"holding""#
        )),
        refused(
            "input_points[1].address",
            &format!("holding register 11 overlaps holding registers 10 and 11 of input_points[0]; {rule}")
        )
    );
    assert_eq!(
        output_point(concat!(
            r#""image_offset":0,"address":7,"function":"write_single_coil"},"#,
            r#"{"image_offset":0,"image_bit":1,"address":7,"function":"write_multiple_coils""#
        )),
        refused(
            "output_points[1].address",
            &format!("coil 7 overlaps coil 7 of output_points[0]; {rule}")
        )
    );
    assert!(input_point(concat!(
        r#""image_offset":0,"address":10,"function":"holding"},"#,
        r#"{"image_offset":2,"address":10,"function":"input""#
    ))
    .is_ok());
    assert_eq!(
        input_point(r#""image_offset":0,"address":65535,"function":"holding","data_type":"f32""#),
        refused(
            "input_points[0].address",
            "2 holding registers from 65535 run past address 65535"
        )
    );
    assert!(input_point(r#""image_offset":0,"address":65535,"data_type":"u16""#).is_ok());
    assert!(output_point(r#""image_offset":0,"address":65535,"function":"coil""#).is_ok());
}

/// Two points of one list may not take one bit of the image: a `u32` at byte 0 takes bytes 0 to 3,
/// so a `u16` at byte 2 is refused; two `bool` points share a byte when their bits differ ("Boolean
/// bits in the same byte are distinct storage only when their bit indices differ"); an input and an
/// output may take the same offset, as `%I` and `%Q` are separate images. Catches two points that
/// write one byte over each other, and bits of one byte refused as an overlap.
#[test]
fn points_of_one_list_do_not_share_a_bit_of_the_image() {
    let rule = "overlapping process-image ranges are refused";
    assert_eq!(
        input_point(concat!(
            r#""image_offset":0,"address":0,"data_type":"u32"},"#,
            r#"{"image_offset":2,"address":5"#
        )),
        refused(
            "input_points[1].image_offset",
            &format!("%IW2 overlaps %ID0 of input_points[0]; {rule}")
        )
    );
    let coils = concat!(
        r#""image_offset":0,"image_bit":2,"address":0,"function":"coils"},"#,
        r#"{"image_offset":0,"image_bit":3,"address":1,"function":"coils""#
    );
    assert!(input_point(coils).is_ok());
    assert_eq!(
        output_point(concat!(
            r#""image_offset":3,"image_bit":2,"address":0,"function":"coils"},"#,
            r#"{"image_offset":3,"image_bit":2,"address":1,"function":"coils""#
        )),
        refused(
            "output_points[1].image_offset",
            &format!("%QX3.2 overlaps %QX3.2 of output_points[0]; {rule}")
        )
    );
    let both = ModbusTcpParams::from_json(concat!(
        r#"{"address":"10.0.0.5:502","input_points":[{"image_offset":0,"address":0}],"#,
        r#""output_points":[{"image_offset":0,"address":0}]}"#
    ));
    assert!(both.is_ok());
}
