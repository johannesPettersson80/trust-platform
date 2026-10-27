//! The conversion between payloads and process-image scalars (specification 32 §7; specification
//! 11, typed protocol point-map integration contract), with values worked out by hand.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::io_drivers::mqtt::payload::{
    check_input_fits, decode_input, encode_output, write_input,
};
use crate::io_drivers::mqtt::{InputPoint, OutputPoint, PayloadFormat, PointType};

/// An input point of `data_type` and `payload_format` at byte 0, bit 2 for a bool, unscaled.
fn input(data_type: PointType, payload_format: PayloadFormat) -> InputPoint {
    InputPoint {
        topic: "t".into(),
        image_offset: 0,
        image_bit: (data_type == PointType::Bool).then_some(2),
        data_type,
        payload_format,
        scale: 1.0,
        offset: 0.0,
    }
}

/// An output point of `data_type` and `payload_format` at byte 0, bit 1 for a bool, unscaled.
fn output(data_type: PointType, payload_format: PayloadFormat) -> OutputPoint {
    OutputPoint {
        topic: "t".into(),
        metric_name: "t".into(),
        image_offset: 0,
        image_bit: (data_type == PointType::Bool).then_some(1),
        data_type,
        payload_format,
        scale: 1.0,
        offset: 0.0,
    }
}

/// The image `image` after `payload` for `point`, as a read writes it: converted, checked
/// against the image, then written; or the conversion's error, with the image as it was.
fn read(point: &InputPoint, payload: &[u8], image: &[u8]) -> Result<Vec<u8>, String> {
    let mut image = image.to_vec();
    let value = decode_input(point, payload)?;
    check_input_fits(point, image.len())?;
    write_input(&mut image, point, value)?;
    Ok(image)
}

/// The text of a conversion that fails.
fn failure(text: &str) -> Result<Vec<u8>, String> {
    Err(text.into())
}

/// A bool payload is `true`, `1` or `on` and `false`, `0` or `off` as text in any case and with
/// whitespace; in JSON also a Boolean or a number, true when not 0; one byte in binary. Writing it
/// sets or clears bit 2 and leaves the other bits of the byte (specification 11: writes preserve
/// every other bit). Catches a whole byte written for one bit, a JSON number other than 0 read as
/// false, and a binary payload of two bytes taken.
#[test]
fn a_bool_payload_sets_or_clears_its_bit_and_only_its_bit() {
    let text = input(PointType::Bool, PayloadFormat::Text);
    for payload in [&b"true"[..], b" TRUE ", b"1", b"on"] {
        assert_eq!(read(&text, payload, &[0b1111_1011]), Ok(vec![0xFF]));
    }
    for payload in [&b"false"[..], b" Off", b"0"] {
        assert_eq!(read(&text, payload, &[0xFF]), Ok(vec![0b1111_1011]));
    }
    assert_eq!(
        read(&text, b"maybe", &[0]),
        failure("mqtt bool payload 'maybe' is not true/false or 1/0")
    );
    let json = input(PointType::Bool, PayloadFormat::Json);
    for payload in [&b"true"[..], b"1", b"2.5", br#""on""#] {
        assert_eq!(read(&json, payload, &[0]), Ok(vec![0b0000_0100]));
    }
    for payload in [&b"false"[..], b"0", br#""off""#] {
        assert_eq!(read(&json, payload, &[0xFF]), Ok(vec![0b1111_1011]));
    }
    for payload in [&b"{}"[..], b"[1]", b"null"] {
        assert_eq!(
            read(&json, payload, &[0]),
            failure("mqtt bool JSON payload must be bool, number, or string")
        );
    }
    let binary = input(PointType::Bool, PayloadFormat::BinaryLe);
    assert_eq!(read(&binary, &[0x80], &[0]), Ok(vec![0b0000_0100]));
    assert_eq!(read(&binary, &[0x00], &[0xFF]), Ok(vec![0b1111_1011]));
    let empty: &[u8] = &[];
    for payload in [empty, &[1, 0]] {
        assert_eq!(
            read(&binary, payload, &[0]),
            failure("mqtt bool binary payload must contain exactly one byte")
        );
    }
}

/// A numeric text or JSON payload is one complete finite number, with surrounding whitespace; a
/// JSON payload is a number or a numeric string (specification 32 §7). 4660 is 0x1234, stored
/// little-endian as 0x34 0x12; −12.5 as an F32 is `0xC148_0000`. Catches trailing text taken, a
/// non-finite value taken, and a big-endian image.
#[test]
fn a_numeric_text_or_json_payload_is_one_complete_finite_number() {
    let text = input(PointType::U16, PayloadFormat::Text);
    assert_eq!(read(&text, b" 4660 ", &[0; 2]), Ok(vec![0x34, 0x12]));
    let real = input(PointType::F32, PayloadFormat::Text);
    assert_eq!(
        read(&real, b" -12.5 ", &[0; 4]),
        Ok(vec![0x00, 0x00, 0x48, 0xC1])
    );
    for (payload, text_of_failure) in [
        (
            &b"1 trailing"[..],
            "mqtt numeric payload: invalid float literal",
        ),
        (
            b"",
            "mqtt numeric payload: cannot parse float from empty string",
        ),
        (b"NaN", "mqtt numeric payload must be finite"),
        (b"inf", "mqtt numeric payload must be finite"),
        (
            &[0xFF],
            "mqtt text payload is not UTF-8: invalid utf-8 sequence of 1 bytes from index 0",
        ),
    ] {
        assert_eq!(read(&real, payload, &[0; 4]), failure(text_of_failure));
    }
    let json = input(PointType::U16, PayloadFormat::Json);
    for payload in [&b"42"[..], br#""42""#, br#"" 42 ""#] {
        assert_eq!(read(&json, payload, &[0; 2]), Ok(vec![42, 0]));
    }
    for (payload, text_of_failure) in [
        (
            &br#""x""#[..],
            "mqtt JSON numeric string: invalid float literal",
        ),
        (
            b"true",
            "mqtt numeric JSON payload must be number or string",
        ),
        (br#""NaN""#, "mqtt numeric payload must be finite"),
    ] {
        assert_eq!(read(&json, payload, &[0; 2]), failure(text_of_failure));
    }
}

/// A binary payload holds exactly the bytes of its type in its byte order; the image holds the
/// value little-endian (specification 32 §7). 0x1234 big-endian is 0x12 0x34 and stored 0x34
/// 0x12; 12.5 as an F32 is `0x4148_0000`. Catches the payload's byte order used for the image, a
/// truncated or padded payload taken, and a non-finite F32 taken.
#[test]
fn a_binary_payload_holds_exactly_the_bytes_of_its_type() {
    let big_word = input(PointType::U16, PayloadFormat::BinaryBe);
    assert_eq!(
        read(&big_word, &[0x12, 0x34], &[0; 2]),
        Ok(vec![0x34, 0x12])
    );
    let little_word = input(PointType::U16, PayloadFormat::BinaryLe);
    assert_eq!(
        read(&little_word, &[0x34, 0x12], &[0; 2]),
        Ok(vec![0x34, 0x12])
    );
    let signed = input(PointType::I16, PayloadFormat::BinaryLe);
    assert_eq!(read(&signed, &[0x2E, 0xFB], &[0; 2]), Ok(vec![0x2E, 0xFB]));
    let big_double = input(PointType::U32, PayloadFormat::BinaryBe);
    assert_eq!(
        read(&big_double, &[0x12, 0x34, 0x56, 0x78], &[0; 4]),
        Ok(vec![0x78, 0x56, 0x34, 0x12])
    );
    let big_real = input(PointType::F32, PayloadFormat::BinaryBe);
    assert_eq!(
        read(&big_real, &[0x41, 0x48, 0x00, 0x00], &[0; 4]),
        Ok(vec![0x00, 0x00, 0x48, 0x41])
    );
    assert_eq!(
        read(&big_real, &[0x7F, 0xC0, 0x00, 0x00], &[0; 4]),
        failure("mqtt numeric payload must be finite")
    );
    assert_eq!(
        read(&big_word, &[1, 2, 3], &[0; 2]),
        failure("mqtt binary payload for U16 must be exactly 2 byte(s), got 3")
    );
    assert_eq!(
        read(&big_double, &[1, 2], &[0; 4]),
        failure("mqtt binary payload for U32 must be exactly 4 byte(s), got 2")
    );
}

/// Inbound scaling is `raw × scale + offset`: 100 × 0.5 − 10 = 40. An integer rounds half away
/// from zero, 1.5 to 2, 2.5 to 3 and −1.5 to −2, and is then checked against its range; a value
/// out of range or an F32 beyond the largest `f32` is refused and the image stays as it was
/// (specification 32 §7). Catches rounding half to even, truncation, a saturated value written,
/// and a failed conversion that writes part of the image.
#[test]
fn scaling_rounds_half_away_from_zero_and_checks_the_range() {
    let scaled = InputPoint {
        scale: 0.5,
        offset: -10.0,
        ..input(PointType::I16, PayloadFormat::Text)
    };
    assert_eq!(read(&scaled, b"100", &[0; 2]), Ok(vec![40, 0]));
    let word = input(PointType::U16, PayloadFormat::Text);
    let signed = input(PointType::I16, PayloadFormat::Text);
    for (point, payload, image) in [
        (&word, &b"1.5"[..], [2, 0]),
        (&word, b"2.5", [3, 0]),
        (&word, b"0.49", [0, 0]),
        (&word, b"65535.49", [0xFF, 0xFF]),
        (&signed, b"-1.5", [0xFE, 0xFF]),
        (&signed, b"-2.5", [0xFD, 0xFF]),
    ] {
        assert_eq!(read(point, payload, &[0xA5; 2]), Ok(image.to_vec()));
    }
    let double = input(PointType::U32, PayloadFormat::Text);
    let long = input(PointType::I32, PayloadFormat::Text);
    let real = input(PointType::F32, PayloadFormat::Text);
    let thousandfold = InputPoint {
        scale: 1000.0,
        ..input(PointType::U16, PayloadFormat::Text)
    };
    assert_eq!(read(&double, b"4294967295", &[0; 4]), Ok(vec![0xFF; 4]));
    assert_eq!(
        read(&long, b"-2147483648", &[0; 4]),
        Ok(vec![0x00, 0x00, 0x00, 0x80])
    );
    for (point, payload, text_of_failure) in [
        (
            &word,
            &b"65535.5"[..],
            "mqtt u16 value 65536 is out of range",
        ),
        (&word, b"-0.5", "mqtt u16 value -1 is out of range"),
        (&signed, b"32767.5", "mqtt i16 value 32768 is out of range"),
        (
            &signed,
            b"-32768.5",
            "mqtt i16 value -32769 is out of range",
        ),
        (
            &double,
            b"4294967296",
            "mqtt u32 value 4294967296 is out of range",
        ),
        (&thousandfold, b"70", "mqtt u16 value 70000 is out of range"),
    ] {
        assert_eq!(read(point, payload, &[0xA5; 4]), failure(text_of_failure));
    }
    assert_eq!(
        read(&real, b"1e39", &[0xA5; 4]),
        failure("mqtt f32 value 1000000000000000000000000000000000000000 is out of range")
    );
}

/// A point that does not lie within the image is refused before a byte is written: a 16-bit value
/// at byte 3 of a 4-byte image needs bytes 3 and 4, and a bool at byte 4 of it has no byte. Catches
/// a write past the image's end and a value cut to fit.
#[test]
fn a_point_beyond_the_image_is_refused() {
    let word = InputPoint {
        image_offset: 3,
        ..input(PointType::U16, PayloadFormat::Text)
    };
    assert_eq!(
        read(&word, b"1", &[0; 4]),
        failure("mqtt point image range 3..5 exceeds process image size 4")
    );
    let bit = InputPoint {
        image_offset: 4,
        ..input(PointType::Bool, PayloadFormat::Text)
    };
    assert_eq!(
        read(&bit, b"true", &[0; 4]),
        failure("mqtt point image_offset 4 is outside input image")
    );
}

/// Outbound scaling is `(engineering − offset) ÷ scale`: (60 − 10) ÷ 0.5 = 100. A number is
/// written in its format: text and JSON in decimal, binary in its byte order; an F32 as text in
/// its shortest form, 0.1, and as JSON widened to an `f64`, 0.10000000149011612, as the computer
/// writes them; a bool as `true` or `false`, or one byte (specification 32 §7). Catches the
/// inverse scaling applied backwards, the wrong byte order, and a value read past the image.
#[test]
fn an_output_is_scaled_back_and_written_in_its_format() {
    let speed = OutputPoint {
        scale: 0.5,
        offset: 10.0,
        ..output(PointType::U16, PayloadFormat::Json)
    };
    assert_eq!(encode_output(&speed, &[60, 0]), Ok(b"100".to_vec()));
    let count = output(PointType::I32, PayloadFormat::BinaryBe);
    assert_eq!(
        encode_output(&count, &[0xD6, 0xFF, 0xFF, 0xFF]),
        Ok(vec![0xFF, 0xFF, 0xFF, 0xD6])
    );
    let text = output(PointType::F32, PayloadFormat::Text);
    let json = output(PointType::F32, PayloadFormat::Json);
    let tenth = [0xCD, 0xCC, 0xCC, 0x3D];
    let hundred = [0x00, 0x00, 0xC8, 0x42];
    assert_eq!(encode_output(&text, &tenth), Ok(b"0.1".to_vec()));
    assert_eq!(
        encode_output(&json, &tenth),
        Ok(b"0.10000000149011612".to_vec())
    );
    assert_eq!(encode_output(&text, &hundred), Ok(b"100".to_vec()));
    assert_eq!(encode_output(&json, &hundred), Ok(b"100.0".to_vec()));
    let flag = output(PointType::Bool, PayloadFormat::Text);
    assert_eq!(encode_output(&flag, &[0b10]), Ok(b"true".to_vec()));
    assert_eq!(encode_output(&flag, &[0b01]), Ok(b"false".to_vec()));
    let flag_byte = output(PointType::Bool, PayloadFormat::BinaryLe);
    assert_eq!(encode_output(&flag_byte, &[0b10]), Ok(vec![1]));
    let far = OutputPoint {
        image_offset: 4,
        ..output(PointType::U16, PayloadFormat::BinaryLe)
    };
    assert_eq!(
        encode_output(&far, &[1]),
        Err(String::from(
            "mqtt point image range 4..6 exceeds process image size 1"
        ))
    );
    assert_eq!(
        encode_output(&flag, &[]),
        Err(String::from(
            "mqtt point image_offset 0 is outside output image"
        ))
    );
    assert_eq!(
        encode_output(&text, &[0x00, 0x00, 0xC0, 0x7F]),
        Err(String::from("mqtt F32 process-image value must be finite"))
    );
    let halved = OutputPoint {
        scale: 0.5,
        ..output(PointType::U16, PayloadFormat::Text)
    };
    assert_eq!(
        encode_output(&halved, &[0xFF, 0xFF]),
        Err(String::from("mqtt u16 value 131070 is out of range"))
    );
}
