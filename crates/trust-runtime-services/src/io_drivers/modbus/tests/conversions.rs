//! The conversions of the point maps (specification 11 §6.5, "Typed protocol point-map integration
//! contract" and "Modbus declared-width floating-point finiteness"): the byte and word order, the
//! scaling, the rounding and the range of the integers, the finiteness of `f32`, and a point's
//! place in the image.

use alloc::format;
use core::slice;

use crate::io_drivers::modbus::point_map::{
    decode_input, encode_output, fits, write_input, Scalar,
};
use crate::io_drivers::modbus::{
    Endian, Failure, InputFunction, InputPoint, ModbusTcpParams, OutputFunction, OutputPoint,
    PointType,
};
use crate::io_drivers::ParamsError;

/// Big-endian bytes and words, the default.
const BIG: (Endian, Endian) = (Endian::Big, Endian::Big);

/// The input point of `data_type` with `scale`, `offset` and the byte and word `orders`, at image
/// byte 0 and holding register 0.
fn input(data_type: PointType, scale: f64, offset: f64, orders: (Endian, Endian)) -> InputPoint {
    InputPoint {
        image_offset: 0,
        image_bit: 0,
        address: 0,
        function: InputFunction::HoldingRegisters,
        data_type,
        scale,
        offset,
        byte_order: orders.0,
        word_order: orders.1,
    }
}

/// The output point of `data_type` with `scale`, `offset` and the byte and word `orders`, at image
/// byte 0 and holding register 0.
fn output(data_type: PointType, scale: f64, offset: f64, orders: (Endian, Endian)) -> OutputPoint {
    OutputPoint {
        image_offset: 0,
        image_bit: 0,
        address: 0,
        function: OutputFunction::MultipleRegisters,
        data_type,
        scale,
        offset,
        byte_order: orders.0,
        word_order: orders.1,
    }
}

/// A number's bytes, as a point's value holds them.
fn bytes(given: &[u8]) -> Scalar {
    let mut buffer = [0; 4];
    buffer[..given.len()].copy_from_slice(given);
    Scalar::Bytes {
        bytes: buffer,
        len: given.len(),
    }
}

/// The `u32` `0x1122_3344` in each order ("byte order applies inside every 16-bit register and word
/// order applies between the two registers of a 32-bit value"): big and big is 11 22 33 44, little
/// bytes swap each register's two bytes to 22 11 44 33, little words swap the two registers to
/// 33 44 11 22, and both make 44 33 22 11. In the image the value is little-endian, 44 33 22 11,
/// whatever the order on the wire. A register value is exactly its registers' bytes ("A truncated
/// or oversized buffer is rejected rather than partially decoded"). Catches an order applied to
/// the image, the two orders mixed up, and a value decoded from too few or too many bytes.
#[test]
fn register_bytes_follow_the_byte_and_word_order() {
    let image = [0x44, 0x33, 0x22, 0x11];
    for (orders, wire) in [
        ((Endian::Big, Endian::Big), [0x11, 0x22, 0x33, 0x44]),
        ((Endian::Little, Endian::Big), [0x22, 0x11, 0x44, 0x33]),
        ((Endian::Big, Endian::Little), [0x33, 0x44, 0x11, 0x22]),
        ((Endian::Little, Endian::Little), [0x44, 0x33, 0x22, 0x11]),
    ] {
        let point = input(PointType::U32, 1.0, 0.0, orders);
        assert_eq!(decode_input(&point, &wire), Ok(bytes(&image)), "{orders:?}");
        let point = output(PointType::U32, 1.0, 0.0, orders);
        assert_eq!(
            encode_output(&point, &image),
            Ok(bytes(&wire)),
            "{orders:?}"
        );
    }
    let point = input(PointType::U16, 1.0, 0.0, BIG);
    assert_eq!(
        decode_input(&point, &[0x12, 0x34]),
        Ok(bytes(&[0x34, 0x12]))
    );
    for wire in [&[0x12][..], &[0x12, 0x34, 0x56, 0x78][..]] {
        let len = wire.len();
        assert_eq!(
            decode_input(&point, wire),
            Err(Failure::data(format!(
                "modbus mapped register response must contain exactly 2 byte(s), got {len}"
            )))
        );
    }
}

/// The computer's scaled points, worked by hand: holding registers E8 03 00 00 with little bytes and
/// words are `0x0000_03E8` = 1000, and `1000 × 0.5 + 1 = 501`, which is F5 01 00 00 in the image;
/// back, `(501 − 1) / 0.5 = 1000`. A `u16` of 60 with scale 0.5, offset 10 and little bytes is
/// `(60 − 10) / 0.5 = 100` = 0x0064, sent 64 00; an `f32` 1.5 (`0x3FC0_0000`) scaled by 2 is 3.0
/// (`0x4040_0000`). Catches scaling applied the wrong way round, and an offset added before the
/// scale.
#[test]
fn scaled_values_reach_the_image_little_endian() {
    let little = (Endian::Little, Endian::Little);
    let point = input(PointType::U32, 0.5, 1.0, little);
    assert_eq!(
        decode_input(&point, &[0xE8, 0x03, 0x00, 0x00]),
        Ok(bytes(&[0xF5, 0x01, 0x00, 0x00]))
    );
    let point = output(PointType::U32, 0.5, 1.0, little);
    assert_eq!(
        encode_output(&point, &501_u32.to_le_bytes()),
        Ok(bytes(&[0xE8, 0x03, 0x00, 0x00]))
    );
    let point = output(PointType::U16, 0.5, 10.0, (Endian::Little, Endian::Big));
    assert_eq!(
        encode_output(&point, &60_u16.to_le_bytes()),
        Ok(bytes(&[0x64, 0x00]))
    );
    let point = input(PointType::F32, 2.0, 0.0, BIG);
    assert_eq!(
        decode_input(&point, &[0x3F, 0xC0, 0x00, 0x00]),
        Ok(bytes(&[0x00, 0x00, 0x40, 0x40]))
    );
}

/// "Integer conversion rounds to the nearest integer with half-way values away from zero, then
/// checks the declared range": 3 × 0.5 = 1.5 is 2, 5 × 0.5 = 2.5 is 3 (not the 2 of ties to even),
/// −3 × 0.5 = −1.5 is −2 (FE FF in the image); going out, 65535 + 0.4 is 65535, −1 − 0.5 is −2
/// (FF FE on the wire), while −0.5 rounds to −1 and 65535.5 to 65536, both outside a `u16`, and
/// 4294967295 + 1 is outside a `u32`. Catches rounding to even or toward zero, and a value cut or
/// wrapped into its type.
#[test]
fn integers_round_half_away_from_zero_within_their_range() {
    let half = input(PointType::U16, 0.5, 0.0, BIG);
    assert_eq!(decode_input(&half, &[0x00, 0x03]), Ok(bytes(&[0x02, 0x00])));
    assert_eq!(decode_input(&half, &[0x00, 0x05]), Ok(bytes(&[0x03, 0x00])));
    let signed = input(PointType::I16, 0.5, 0.0, BIG);
    assert_eq!(
        decode_input(&signed, &[0xFF, 0xFD]),
        Ok(bytes(&[0xFE, 0xFF]))
    );
    let below = input(PointType::U16, 1.0, -0.5, BIG);
    assert_eq!(
        decode_input(&below, &[0x00, 0x00]),
        Err(Failure::data(
            "modbus mapped numeric value -0.5 outside 0..=65535"
        ))
    );
    let above = output(PointType::U16, 1.0, -0.5, BIG);
    assert_eq!(
        encode_output(&above, &[0xFF, 0xFF]),
        Err(Failure::data(
            "modbus mapped numeric value 65535.5 outside 0..=65535"
        ))
    );
    let inside = output(PointType::U16, 1.0, -0.4, BIG);
    assert_eq!(
        encode_output(&inside, &[0xFF, 0xFF]),
        Ok(bytes(&[0xFF, 0xFF]))
    );
    let signed = output(PointType::I16, 1.0, 0.5, BIG);
    assert_eq!(
        encode_output(&signed, &[0xFF, 0xFF]),
        Ok(bytes(&[0xFF, 0xFE]))
    );
    let wide = output(PointType::U32, 1.0, -1.0, BIG);
    assert_eq!(
        encode_output(&wide, &[0xFF, 0xFF, 0xFF, 0xFF]),
        Err(Failure::data(
            "modbus mapped numeric value 4294967296 outside 0..=4294967295"
        ))
    );
}

/// An `f32` must be finite on the wire, in the image and "after scaling or inverse scaling is
/// narrowed from the internal `f64` calculation to the declared `f32` width": a quiet NaN
/// (`0x7FC0_0000`) and an infinity (`0x7F80_0000`) on the wire are refused; `f32::MAX` (`0x7F7F_FFFF`)
/// scaled by 2 is finite as an `f64` and infinite as an `f32`; a `u32` scaled by 1e300 is beyond
/// an `f64`; an infinity in the image is refused; and 1 divided by a scale of 1e-320 is beyond an
/// `f64`. Catches a NaN or an infinity that becomes a `REAL` or a register payload.
#[test]
fn f32_values_stay_finite_after_narrowing() {
    let wire = "modbus mapped F32 wire value must be finite";
    let point = input(PointType::F32, 1.0, 0.0, BIG);
    assert_eq!(
        decode_input(&point, &[0x7F, 0xC0, 0x00, 0x00]),
        Err(Failure::data(wire))
    );
    assert_eq!(
        decode_input(&point, &[0x7F, 0x80, 0x00, 0x00]),
        Err(Failure::data(wire))
    );
    let doubled = input(PointType::F32, 2.0, 0.0, BIG);
    assert_eq!(
        decode_input(&doubled, &[0x7F, 0x7F, 0xFF, 0xFF]),
        Err(Failure::data(
            "modbus mapped F32 process-image value must remain finite after narrowing"
        ))
    );
    let huge = input(PointType::U32, 1e300, 0.0, BIG);
    assert_eq!(
        decode_input(&huge, &[0xFF, 0xFF, 0xFF, 0xFF]),
        Err(Failure::data(
            "modbus mapped process-image value must be finite"
        ))
    );
    let point = output(PointType::F32, 1.0, 0.0, BIG);
    assert_eq!(
        encode_output(&point, &f32::INFINITY.to_le_bytes()),
        Err(Failure::data(
            "modbus mapped F32 process-image value must be finite"
        ))
    );
    let halved = output(PointType::F32, 0.5, 0.0, BIG);
    assert_eq!(
        encode_output(&halved, &f32::MAX.to_le_bytes()),
        Err(Failure::data(
            "modbus mapped F32 wire value must remain finite after narrowing"
        ))
    );
    let tiny = output(PointType::U16, 1e-320, 0.0, BIG);
    assert_eq!(
        encode_output(&tiny, &1_u16.to_le_bytes()),
        Err(Failure::data("modbus mapped numeric value must be finite"))
    );
}

/// "A Boolean occupies one selected bit; its bit index is `0..=7`, and writes preserve every other
/// bit in the byte": setting bit 1 of `1010_0101` gives `1010_0111`, then clearing bit 2 gives
/// `1010_0011`; an output point reads its own bit only. Catches a whole byte written for one coil,
/// and a bit counted from the high end.
#[test]
fn a_bool_point_writes_and_reads_only_its_bit() {
    let coil = |image_bit: u8| InputPoint {
        image_bit,
        function: InputFunction::Coils,
        ..input(PointType::Bool, 1.0, 0.0, BIG)
    };
    let mut image = [0b1010_0101];
    write_input(&coil(1), Scalar::Bit(true), &mut image);
    assert_eq!(image, [0b1010_0111]);
    write_input(&coil(2), Scalar::Bit(false), &mut image);
    assert_eq!(image, [0b1010_0011]);
    let out = |image_bit: u8| OutputPoint {
        image_bit,
        function: OutputFunction::SingleCoil,
        ..output(PointType::Bool, 1.0, 0.0, BIG)
    };
    assert_eq!(
        encode_output(&out(1), &[0b0000_0010]),
        Ok(Scalar::Bit(true))
    );
    assert_eq!(
        encode_output(&out(2), &[0b0000_0010]),
        Ok(Scalar::Bit(false))
    );
}

/// "Every offset-plus-width calculation is checked for arithmetic overflow and image bounds before
/// mutation": a `u32` at byte 1 takes bytes 1 to 4, past an image of 3 bytes, a `bool` at byte 3
/// is past it too, and a `u16` at byte 1 fits. At configuration the refusal names the point.
/// Catches a value written past the image's end, which would panic or write a neighbour's bytes,
/// and a point that fits refused.
#[test]
fn a_point_outside_the_image_is_refused_before_any_byte_changes() {
    let wide = InputPoint {
        image_offset: 1,
        ..input(PointType::U32, 1.0, 0.0, BIG)
    };
    let range = "modbus point image range 1..5 out of 3";
    assert_eq!(fits(slice::from_ref(&wide), 3), Err(Failure::data(range)));
    let coil = InputPoint {
        image_offset: 3,
        function: InputFunction::Coils,
        ..input(PointType::Bool, 1.0, 0.0, BIG)
    };
    assert_eq!(
        fits(&[coil], 3),
        Err(Failure::data("modbus point image_offset 3 out of range"))
    );
    let narrow = InputPoint {
        image_offset: 1,
        ..input(PointType::U16, 1.0, 0.0, BIG)
    };
    assert_eq!(fits(&[narrow], 3), Ok(()));
    let out = OutputPoint {
        image_offset: 1,
        ..output(PointType::U32, 1.0, 0.0, BIG)
    };
    assert_eq!(encode_output(&out, &[0; 3]), Err(Failure::data(range)));
    let params = ModbusTcpParams::from_json(
        r#"{"address":"10.0.0.5:502","input_points":[{"image_offset":1,"address":0,"data_type":"u32"}]}"#,
    )
    .expect("a valid point");
    assert_eq!(
        params.check_images(3, 0),
        Err(ParamsError {
            key: "input_points[0].image_offset".into(),
            reason: "%ID1 is past the end of the 3-byte input image".into(),
        })
    );
    assert_eq!(params.check_images(5, 0), Ok(()));
}
