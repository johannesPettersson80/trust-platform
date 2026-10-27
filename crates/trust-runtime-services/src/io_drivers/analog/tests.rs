use alloc::format;
use alloc::string::ToString;
use alloc::vec;

use super::*;

/// The concrete, flat address of `area` and `size` at the byte offset `byte`: specification 11
/// §6.5 reads a word address such as `%IW2` as the word at byte offset 2 of its area.
fn flat(area: IoArea, size: IoSize, byte: u32) -> IoAddress {
    IoAddress {
        area,
        size,
        byte,
        bit: 0,
        path: vec![byte],
        wildcard: false,
    }
}

/// The refusal of `key` for `reason`.
fn refused(key: &str, reason: &str) -> Result<AnalogParams, ParamsError> {
    Err(ParamsError {
        key: key.into(),
        reason: reason.into(),
    })
}

/// The parameters of one input on `GPIO2` at `address`, the JSON text of a value.
fn input_at(address: &str) -> Result<AnalogParams, ParamsError> {
    AnalogParams::from_json(&format!(
        r#"{{"inputs":[{{"address":{address},"line":"GPIO2"}}]}}"#
    ))
}

/// A table with an input on a chip's pin and one on an IIO channel, and an output with and one
/// without `initial`, reads into exactly those entries in their order, with `initial` 0 where it is
/// left out (specification 11 §6.5). `%IW0` and `%QW0` share an offset in two areas and are both
/// taken. Catches an entry dropped or reordered, a line or address read from the wrong key, a
/// default other than 0, and an input address checked against the output list.
#[test]
fn a_full_table_reads_into_its_entries() {
    let params = AnalogParams::from_json(concat!(
        r#"{"inputs":[{"address":"%IW0","line":"GPIO2"},"#,
        r#"{"address":"%IW2","line":"iio:device0/in_voltage1"}],"#,
        r#""outputs":[{"address":"%QW0","initial":-1500,"line":"DAC1"},"#,
        r#"{"address":"%QW4","line":"iio:device0/out_voltage0"}]}"#
    ));
    assert_eq!(
        params,
        Ok(AnalogParams {
            inputs: vec![
                AnalogInput {
                    address: flat(IoArea::Input, IoSize::Word, 0),
                    line: "GPIO2".into(),
                },
                AnalogInput {
                    address: flat(IoArea::Input, IoSize::Word, 2),
                    line: "iio:device0/in_voltage1".into(),
                },
            ],
            outputs: vec![
                AnalogOutput {
                    address: flat(IoArea::Output, IoSize::Word, 0),
                    line: "DAC1".into(),
                    initial: -1_500,
                },
                AnalogOutput {
                    address: flat(IoArea::Output, IoSize::Word, 4),
                    line: "iio:device0/out_voltage0".into(),
                    initial: 0,
                },
            ],
        })
    );
}

/// An empty table has no entries (an absent list is empty), and `initial` takes both ends of an
/// `INT`, −32,768 and 32,767. Catches an absent list refused instead of read as empty, and the
/// range of `initial` cut short at either end.
#[test]
fn an_absent_list_is_empty_and_initial_takes_a_whole_int() {
    assert_eq!(
        AnalogParams::from_json("{}"),
        Ok(AnalogParams {
            inputs: vec![],
            outputs: vec![],
        })
    );
    let ends = AnalogParams::from_json(concat!(
        r#"{"outputs":[{"address":"%QW0","initial":-32768,"line":"DAC0"},"#,
        r#"{"address":"%QW2","initial":32767,"line":"DAC1"}]}"#
    ))
    .expect("both ends of an INT are taken");
    let initial: Vec<i16> = ends.outputs.iter().map(|output| output.initial).collect();
    assert_eq!(initial, vec![-32_768, 32_767]);
}

/// The table's shape is checked with the GPIO driver's fail-closed rules (specification 11 §6.5):
/// the root is a table, a list an array of tables, and an unknown key at the root or in an entry is
/// refused, before the keys it may stand for. Catches a typing mistake such as `input` or `adress`
/// ignored, which would leave a channel silently unread, and a refusal that names a missing
/// `address` where the user wrote `adress`.
#[test]
fn a_table_of_another_shape_is_refused() {
    assert_eq!(
        AnalogParams::from_json("[]"),
        refused("params", "must be a table, not []")
    );
    let broken = AnalogParams::from_json("{").expect_err("not JSON");
    assert_eq!(broken.key, "params");
    assert!(broken.reason.starts_with("is not JSON: "), "{broken}");
    assert_eq!(
        AnalogParams::from_json(r#"{"input":[]}"#),
        refused(
            "input",
            "unknown key; the analog driver takes inputs and outputs"
        )
    );
    assert_eq!(
        AnalogParams::from_json(r#"{"inputs":{}}"#),
        refused("inputs", "must be an array of tables, not {}")
    );
    assert_eq!(
        AnalogParams::from_json(r#"{"inputs":["%IW0"]}"#),
        refused("inputs[0]", r#"must be a table, not "%IW0""#)
    );
    assert_eq!(
        AnalogParams::from_json(r#"{"inputs":[{"adress":"%IW0","line":"GPIO2"}]}"#),
        refused(
            "inputs[0].adress",
            "unknown key; an analog input takes address and line"
        )
    );
    assert_eq!(
        AnalogParams::from_json(
            r#"{"outputs":[{"address":"%QW0","frequency_hz":50,"line":"DAC0"}]}"#
        ),
        refused(
            "outputs[0].frequency_hz",
            "unknown key; an analog output takes address, initial and line"
        )
    );
    // The text of a refusal is `<key>: <reason>`.
    let missing = ParamsError {
        key: "inputs[0].line".into(),
        reason: "is required, a non-empty string naming the line".into(),
    };
    assert_eq!(
        missing.to_string(),
        "inputs[0].line: is required, a non-empty string naming the line"
    );
}

/// An input's address is a concrete, flat `%IW` word, and an output's a `%QW` word; any other
/// address is refused with the value given (specification 11 §6.5: every address is concrete and
/// of the size the driver names; §6.4: an overflowing address calculation is rejected). Catches a
/// wildcard read as `%IW0`, a hierarchical address read as its first part, a bit, byte or double
/// word read as a word, and an input written to the output or marker image.
#[test]
fn an_address_other_than_a_concrete_word_of_its_area_is_refused() {
    let input = |given: &str| {
        refused(
            "inputs[0].address",
            &format!(r#"must be a concrete, flat %IW word such as "%IW0", not {given}"#),
        )
    };
    assert_eq!(
        AnalogParams::from_json(r#"{"inputs":[{"line":"GPIO2"}]}"#),
        refused(
            "inputs[0].address",
            r#"is required, a concrete, flat %IW word such as "%IW0""#
        )
    );
    assert_eq!(input_at("5"), input("5"));
    assert_eq!(input_at(r#""IW0""#), input(r#""IW0""#));
    assert_eq!(input_at(r#""%I*""#), input(r#""%I*""#));
    assert_eq!(input_at(r#""%IW1.2""#), input(r#""%IW1.2""#));
    assert_eq!(input_at(r#""%IX0.0""#), input(r#""%IX0.0""#));
    assert_eq!(input_at(r#""%IB0""#), input(r#""%IB0""#));
    assert_eq!(input_at(r#""%ID0""#), input(r#""%ID0""#));
    assert_eq!(input_at(r#""%QW0""#), input(r#""%QW0""#));
    assert_eq!(input_at(r#""%MW0""#), input(r#""%MW0""#));
    // The word at byte 4,294,967,295 would end past the last byte offset a u32 holds.
    assert_eq!(input_at(r#""%IW4294967295""#), input(r#""%IW4294967295""#));
    assert_eq!(
        AnalogParams::from_json(r#"{"outputs":[{"address":"%IW0","line":"DAC0"}]}"#),
        refused(
            "outputs[0].address",
            r#"must be a concrete, flat %QW word such as "%QW0", not "%IW0""#
        )
    );
}

/// `line` is a non-empty string, used once across the driver's inputs and outputs, and a
/// process-image address once in its list (specification 11 §6.5). `%IW00` is the same address as
/// `%IW0`: the word at byte 0. Catches two entries driving or reading one pin, and two inputs
/// writing one word, where the later would hide the earlier.
#[test]
fn a_line_is_used_once_and_an_address_once_in_its_list() {
    assert_eq!(
        AnalogParams::from_json(r#"{"inputs":[{"address":"%IW0"}]}"#),
        refused(
            "inputs[0].line",
            "is required, a non-empty string naming the line"
        )
    );
    assert_eq!(
        AnalogParams::from_json(r#"{"inputs":[{"address":"%IW0","line":""}]}"#),
        refused(
            "inputs[0].line",
            r#"must be a non-empty string naming the line, not """#
        )
    );
    assert_eq!(
        AnalogParams::from_json(r#"{"inputs":[{"address":"%IW0","line":2}]}"#),
        refused(
            "inputs[0].line",
            "must be a non-empty string naming the line, not 2"
        )
    );
    assert_eq!(
        AnalogParams::from_json(concat!(
            r#"{"inputs":[{"address":"%IW0","line":"GPIO2"},"#,
            r#"{"address":"%IW2","line":"GPIO2"}]}"#
        )),
        refused(
            "inputs[1].line",
            r#""GPIO2" is used by inputs[0].line already; a line is used once"#
        )
    );
    assert_eq!(
        AnalogParams::from_json(concat!(
            r#"{"inputs":[{"address":"%IW0","line":"GPIO2"}],"#,
            r#""outputs":[{"address":"%QW0","line":"GPIO2"}]}"#
        )),
        refused(
            "outputs[0].line",
            r#""GPIO2" is used by inputs[0].line already; a line is used once"#
        )
    );
    assert_eq!(
        AnalogParams::from_json(concat!(
            r#"{"inputs":[{"address":"%IW0","line":"GPIO2"},"#,
            r#"{"address":"%IW00","line":"GPIO3"}]}"#
        )),
        refused(
            "inputs[1].address",
            r#""%IW0" overlaps the bytes of inputs[0].address ("%IW0"); the entries of a list use bytes of their own"#
        )
    );
    assert_eq!(
        AnalogParams::from_json(concat!(
            r#"{"outputs":[{"address":"%QW2","line":"DAC0"},"#,
            r#"{"address":"%QW2","line":"DAC1"}]}"#
        )),
        refused(
            "outputs[1].address",
            r#""%QW2" overlaps the bytes of outputs[0].address ("%QW2"); the entries of a list use bytes of their own"#
        )
    );
}

/// An output's `initial` is an integer that an `INT` holds, −32,768 to 32,767 (specification 11
/// §6.5: the millivolts as an `INT`). Catches a value that would wrap when stored, and a number
/// written as a fraction or a string taken as some other value.
#[test]
fn an_initial_value_an_int_cannot_hold_is_refused() {
    let initial = |given: &str| {
        AnalogParams::from_json(&format!(
            r#"{{"outputs":[{{"address":"%QW0","initial":{given},"line":"DAC0"}}]}}"#
        ))
    };
    let reason = |given: &str| {
        refused(
            "outputs[0].initial",
            &format!("must be an integer from -32768 to 32767, not {given}"),
        )
    };
    assert_eq!(initial("-32769"), reason("-32769"));
    assert_eq!(initial("32768"), reason("32768"));
    assert_eq!(initial("1.5"), reason("1.5"));
    assert_eq!(initial(r#""0""#), reason(r#""0""#));
}

/// An input's value is its millivolts as an `INT`, limited to −32,768 and 32,767 (specification
/// 11 §6.5: a value outside what the hardware converts is limited to its range). Catches a value
/// that wraps, so that 32,768 mV would read as −32,768 mV.
#[test]
fn an_input_value_is_limited_to_an_int() {
    // Within the range the value is the millivolts themselves.
    assert_eq!(input_value(1_650), 1_650);
    assert_eq!(input_value(-1_650), -1_650);
    // At and past the top, 32,767.
    assert_eq!(input_value(32_767), 32_767);
    assert_eq!(input_value(32_768), 32_767);
    assert_eq!(input_value(i32::MAX), 32_767);
    // At and past the bottom, −32,768.
    assert_eq!(input_value(-32_768), -32_768);
    assert_eq!(input_value(-32_769), -32_768);
    assert_eq!(input_value(i32::MIN), -32_768);
}

/// A Linux IIO input reads `(raw + offset) × scale` millivolts, rounded to the nearest millivolt
/// (specification 11 §6.5), a half away from zero. Catches a reading truncated instead of rounded,
/// which reads 1,650 mV as 1,649, the offset added after the scale, and a reading out of range
/// that wraps.
#[test]
fn an_iio_reading_is_rounded_to_the_nearest_millivolt() {
    // A 12-bit converter over 3.3 V: 3,300 / 4,096 mV per count. 1,000 × 3,300 / 4,096 =
    // 805.664 0625 mV, nearest 806; 4,095 counts are 3,299.194 mV, nearest 3,299.
    assert_eq!(iio_millivolts(1_000, 0.0, 3_300.0 / 4_096.0), 806);
    assert_eq!(iio_millivolts(4_095, 0.0, 3_300.0 / 4_096.0), 3_299);
    // The same scale as the kernel writes it, 0.805664062: 2,048 counts are 1,649.999 998 976 mV,
    // nearest 1,650.
    assert_eq!(iio_millivolts(2_048, 0.0, 0.805_664_062), 1_650);
    // The offset is added before the scale: (10 + 2) × 0.5 = 6, where 10 × 0.5 + 2 = 7.
    assert_eq!(iio_millivolts(10, 2.0, 0.5), 6);
    // A half rounds away from zero: (100 − 0.5) × 1 = 99.5 is 100; 1 × 2.5 is 3 and −1 × 2.5 is −3.
    assert_eq!(iio_millivolts(100, -0.5, 1.0), 100);
    assert_eq!(iio_millivolts(1, 0.0, 2.5), 3);
    assert_eq!(iio_millivolts(-1, 0.0, 2.5), -3);
    // Less than a half rounds toward zero: ±249 × 0.01 = ±2.49 is ±2.
    assert_eq!(iio_millivolts(249, 0.0, 0.01), 2);
    assert_eq!(iio_millivolts(-249, 0.0, 0.01), -2);
    // Beyond an i32 the reading is limited to it, and a scale that is not a number reads 0.
    assert_eq!(iio_millivolts(i64::MAX, 0.0, 1.0), i32::MAX);
    assert_eq!(iio_millivolts(i64::MIN, 0.0, 1.0), i32::MIN);
    assert_eq!(iio_millivolts(1_000, 0.0, f64::NAN), 0);
}

/// Specification 11 §6.5: the entries of a list use bytes of their own. `%IW0` takes bytes 0 and 1,
/// so `%IW1`, which takes bytes 1 and 2, overlaps it and is refused, and `%IW2` does not and is
/// taken. Catches two inputs that write the same byte of the image, each over the other.
#[test]
fn a_word_that_overlaps_another_of_its_list_is_refused() {
    assert_eq!(
        AnalogParams::from_json(concat!(
            r#"{"inputs":[{"address":"%IW0","line":"GPIO2"},"#,
            r#"{"address":"%IW1","line":"GPIO3"}]}"#
        )),
        refused(
            "inputs[1].address",
            r#""%IW1" overlaps the bytes of inputs[0].address ("%IW0"); the entries of a list use bytes of their own"#
        )
    );
    assert!(AnalogParams::from_json(concat!(
        r#"{"inputs":[{"address":"%IW0","line":"GPIO2"},"#,
        r#"{"address":"%IW2","line":"GPIO3"}]}"#
    ))
    .is_ok());
}
