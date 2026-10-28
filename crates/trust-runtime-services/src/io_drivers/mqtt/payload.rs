//! The conversion between an MQTT payload and a process-image scalar of the `mqtt` driver's typed
//! points (specification 32 §7; specification 11, typed protocol point-map integration contract).
//! Every check runs before a byte is written, so a conversion that fails leaves its destination as
//! it was. The texts of a failed conversion are those of trust-runtime's driver.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ops::Range;

use serde_json::Value;

use super::points::{InputPoint, OutputPoint, PayloadFormat, PointType};

/// From 2^52 on every `f64` is a whole number.
const WHOLE_FROM: f64 = 4_503_599_627_370_496.0;

/// A point's value as the process image holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ImageValue {
    /// The bit of a bool point.
    Bit(bool),
    /// The little-endian bytes of a numeric point; the first `width` of the four count.
    Number {
        /// The bytes, least significant first.
        bytes: [u8; 4],
        /// 2 or 4.
        width: usize,
    },
}

impl ImageValue {
    /// The value of the two bytes `bytes`.
    fn word(bytes: [u8; 2]) -> Self {
        Self::Number {
            bytes: [bytes[0], bytes[1], 0, 0],
            width: 2,
        }
    }

    /// The value of the four bytes `bytes`.
    fn double(bytes: [u8; 4]) -> Self {
        Self::Number { bytes, width: 4 }
    }
}

/// The value of `payload` for the input point `point`: parsed by its format, scaled
/// (`engineering = raw × scale + offset`) and converted to its type (specification 32 §7).
pub(super) fn decode_input(point: &InputPoint, payload: &[u8]) -> Result<ImageValue, String> {
    if point.data_type == PointType::Bool {
        return parse_bool_payload(payload, point.payload_format).map(ImageValue::Bit);
    }
    let raw = parse_numeric_payload(payload, point.payload_format, point.data_type)?;
    let engineering = raw * point.scale + point.offset;
    Ok(match point.data_type {
        PointType::U16 => ImageValue::word(checked_u16(engineering)?.to_le_bytes()),
        PointType::I16 => ImageValue::word(checked_i16(engineering)?.to_le_bytes()),
        PointType::U32 => ImageValue::double(checked_u32(engineering)?.to_le_bytes()),
        PointType::I32 => ImageValue::double(checked_i32(engineering)?.to_le_bytes()),
        PointType::F32 => ImageValue::double(checked_f32(engineering)?.to_le_bytes()),
        PointType::Bool => {
            return Err("mqtt bool cannot be written as numeric image bytes".to_string());
        }
    })
}

/// Checks that the input point `point` lies within an input image of `len` bytes.
pub(super) fn check_input_fits(point: &InputPoint, len: usize) -> Result<(), String> {
    if point.data_type == PointType::Bool {
        bit_byte(point.image_offset, len, "input").map(|_| ())
    } else {
        byte_range(point.image_offset, width(point.data_type), len).map(|_| ())
    }
}

/// Writes `value` to the storage of `point` in `image`: a bit leaves the other bits of its byte as
/// they are, and a number takes exactly its width.
pub(super) fn write_input(
    image: &mut [u8],
    point: &InputPoint,
    value: ImageValue,
) -> Result<(), String> {
    match value {
        ImageValue::Bit(on) => {
            let byte = bit_byte(point.image_offset, image.len(), "input")?;
            let mask = bit_mask(point.image_bit)?;
            if let Some(slot) = image.get_mut(byte) {
                if on {
                    *slot |= mask;
                } else {
                    *slot &= !mask;
                }
            }
            Ok(())
        }
        ImageValue::Number { bytes, width } => {
            let range = byte_range(point.image_offset, width, image.len())?;
            let (Some(target), Some(source)) = (image.get_mut(range), bytes.get(..width)) else {
                return Err("mqtt point image range overflow".to_string());
            };
            target.copy_from_slice(source);
            Ok(())
        }
    }
}

/// The payload of the output point `point` from `image`: its value read, scaled back
/// (`raw = (engineering − offset) ÷ scale`) and written in its format (specification 32 §7).
pub(super) fn encode_output(point: &OutputPoint, image: &[u8]) -> Result<Vec<u8>, String> {
    if point.data_type == PointType::Bool {
        let value = read_bit(image, point.image_offset, point.image_bit)?;
        return Ok(encode_bool_payload(value, point.payload_format));
    }
    let raw = scaled_raw(point, image)?;
    encode_numeric_payload(raw, point.payload_format, point.data_type)
}

/// The bit of the bool output point at `offset` and `bit` in `image`.
pub(super) fn read_bit(image: &[u8], offset: u32, bit: Option<u8>) -> Result<bool, String> {
    let byte = bit_byte(offset, image.len(), "output")?;
    let mask = bit_mask(bit)?;
    Ok(image.get(byte).is_some_and(|value| value & mask != 0))
}

/// The raw value of the numeric output point `point` in `image`: its engineering value less its
/// offset, divided by its scale.
pub(super) fn scaled_raw(point: &OutputPoint, image: &[u8]) -> Result<f64, String> {
    let engineering = read_numeric(image, point.image_offset, point.data_type)?;
    Ok((engineering - point.offset) / point.scale)
}

/// The value of type `data_type` at `offset` in `image`, little-endian; an F32 must be finite.
fn read_numeric(image: &[u8], offset: u32, data_type: PointType) -> Result<f64, String> {
    let range = byte_range(offset, width(data_type), image.len())?;
    let bytes = image.get(range).unwrap_or_default();
    match data_type {
        PointType::U16 => Ok(f64::from(u16::from_le_bytes(exact::<2>(bytes, data_type)?))),
        PointType::I16 => Ok(f64::from(i16::from_le_bytes(exact::<2>(bytes, data_type)?))),
        PointType::U32 => Ok(f64::from(u32::from_le_bytes(exact::<4>(bytes, data_type)?))),
        PointType::I32 => Ok(f64::from(i32::from_le_bytes(exact::<4>(bytes, data_type)?))),
        PointType::F32 => {
            let value = f32::from_le_bytes(exact::<4>(bytes, data_type)?);
            if value.is_finite() {
                Ok(f64::from(value))
            } else {
                Err("mqtt F32 process-image value must be finite".to_string())
            }
        }
        PointType::Bool => Err("mqtt bool cannot be read as numeric image bytes".to_string()),
    }
}

/// The bytes of a value of type `data_type` in the image: 1 for a bool's byte, 2 or 4.
fn width(data_type: PointType) -> usize {
    usize::try_from(data_type.image_bytes()).unwrap_or(usize::MAX)
}

/// The byte `offset` of an image of `len` bytes, which a bool point uses.
fn bit_byte(offset: u32, len: usize, image: &str) -> Result<usize, String> {
    usize::try_from(offset)
        .ok()
        .filter(|byte| *byte < len)
        .ok_or_else(|| format!("mqtt point image_offset {offset} is outside {image} image"))
}

/// The mask of the bit `bit`, 0 to 7.
fn bit_mask(bit: Option<u8>) -> Result<u8, String> {
    bit.and_then(|bit| 1_u8.checked_shl(u32::from(bit)))
        .ok_or_else(|| "mqtt bool point image_bit must be between 0 and 7".to_string())
}

/// The `width` bytes from `offset` in an image of `len` bytes, checked for overflow and bounds.
fn byte_range(offset: u32, width: usize, len: usize) -> Result<Range<usize>, String> {
    let start =
        usize::try_from(offset).map_err(|_| "mqtt point image range overflow".to_string())?;
    let end = start
        .checked_add(width)
        .ok_or_else(|| "mqtt point image range overflow".to_string())?;
    if end > len {
        return Err(format!(
            "mqtt point image range {start}..{end} exceeds process image size {len}"
        ));
    }
    Ok(start..end)
}

/// A bool payload in `format`: text `true`, `1` or `on` and `false`, `0` or `off` in any case and
/// with surrounding whitespace; in JSON also a Boolean or a number, true when not 0; one byte in
/// binary, true when not 0.
fn parse_bool_payload(payload: &[u8], format: PayloadFormat) -> Result<bool, String> {
    match format {
        PayloadFormat::Text => parse_bool_str(utf8(payload)?.trim()),
        PayloadFormat::Json => match json(payload)? {
            Value::Bool(value) => Ok(value),
            Value::Number(number) => Ok(number.as_f64().is_some_and(|value| value != 0.0)),
            Value::String(text) => parse_bool_str(&text),
            _ => Err("mqtt bool JSON payload must be bool, number, or string".to_string()),
        },
        PayloadFormat::BinaryLe | PayloadFormat::BinaryBe => match payload {
            [value] => Ok(*value != 0),
            _ => Err("mqtt bool binary payload must contain exactly one byte".to_string()),
        },
    }
}

/// The Boolean that the word `text` names, trimmed and in any case.
fn parse_bool_str(text: &str) -> Result<bool, String> {
    match text.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "on" => Ok(true),
        "false" | "0" | "off" => Ok(false),
        _ => Err(format!(
            "mqtt bool payload '{text}' is not true/false or 1/0"
        )),
    }
}

/// A numeric payload in `format`: one complete finite number as text, with surrounding
/// whitespace; a JSON number or numeric string; or exactly the bytes of `data_type`.
fn parse_numeric_payload(
    payload: &[u8],
    format: PayloadFormat,
    data_type: PointType,
) -> Result<f64, String> {
    let value = match format {
        PayloadFormat::Text => utf8(payload)?
            .trim()
            .parse::<f64>()
            .map_err(|error| format!("mqtt numeric payload: {error}"))?,
        PayloadFormat::Json => match json(payload)? {
            Value::Number(number) => number
                .as_f64()
                .ok_or_else(|| "mqtt JSON number out of range".to_string())?,
            Value::String(text) => text
                .trim()
                .parse::<f64>()
                .map_err(|error| format!("mqtt JSON numeric string: {error}"))?,
            _ => return Err("mqtt numeric JSON payload must be number or string".to_string()),
        },
        PayloadFormat::BinaryLe | PayloadFormat::BinaryBe => {
            parse_binary_numeric(payload, format, data_type)?
        }
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err("mqtt numeric payload must be finite".to_string())
    }
}

/// Exactly the bytes of a value of `data_type` in the byte order of `format`.
fn parse_binary_numeric(
    payload: &[u8],
    format: PayloadFormat,
    data_type: PointType,
) -> Result<f64, String> {
    let little = format == PayloadFormat::BinaryLe;
    match data_type {
        PointType::U16 => {
            let bytes = exact::<2>(payload, data_type)?;
            Ok(f64::from(if little {
                u16::from_le_bytes(bytes)
            } else {
                u16::from_be_bytes(bytes)
            }))
        }
        PointType::I16 => {
            let bytes = exact::<2>(payload, data_type)?;
            Ok(f64::from(if little {
                i16::from_le_bytes(bytes)
            } else {
                i16::from_be_bytes(bytes)
            }))
        }
        PointType::U32 => {
            let bytes = exact::<4>(payload, data_type)?;
            Ok(f64::from(if little {
                u32::from_le_bytes(bytes)
            } else {
                u32::from_be_bytes(bytes)
            }))
        }
        PointType::I32 => {
            let bytes = exact::<4>(payload, data_type)?;
            Ok(f64::from(if little {
                i32::from_le_bytes(bytes)
            } else {
                i32::from_be_bytes(bytes)
            }))
        }
        PointType::F32 => {
            let bytes = exact::<4>(payload, data_type)?;
            Ok(f64::from(if little {
                f32::from_le_bytes(bytes)
            } else {
                f32::from_be_bytes(bytes)
            }))
        }
        PointType::Bool => Err("mqtt bool binary payload cannot be parsed as numeric".to_string()),
    }
}

/// `payload` as exactly `N` bytes for a value of `data_type`.
fn exact<const N: usize>(payload: &[u8], data_type: PointType) -> Result<[u8; N], String> {
    <[u8; N]>::try_from(payload).map_err(|_| {
        format!(
            "mqtt binary payload for {data_type:?} must be exactly {N} byte(s), got {}",
            payload.len()
        )
    })
}

/// The payload of the Boolean `value` in `format`: `true` or `false` as text and JSON, one byte
/// of 1 or 0 in binary.
fn encode_bool_payload(value: bool, format: PayloadFormat) -> Vec<u8> {
    match format {
        PayloadFormat::Text | PayloadFormat::Json => {
            if value {
                b"true".to_vec()
            } else {
                b"false".to_vec()
            }
        }
        PayloadFormat::BinaryLe | PayloadFormat::BinaryBe => alloc::vec![u8::from(value)],
    }
}

/// The payload of the raw value `value` of `data_type` in `format`, converted as
/// [`decode_input`] converts: an integer rounded half away from zero and range-checked, an F32
/// finite after narrowing. Text writes an integer in decimal and an F32 in its shortest form;
/// JSON writes an F32 widened to an `f64`.
fn encode_numeric_payload(
    value: f64,
    format: PayloadFormat,
    data_type: PointType,
) -> Result<Vec<u8>, String> {
    let little = format == PayloadFormat::BinaryLe;
    let bytes = match data_type {
        PointType::U16 => {
            let value = checked_u16(value)?;
            match format {
                PayloadFormat::Text | PayloadFormat::Json => value.to_string().into_bytes(),
                _ if little => value.to_le_bytes().to_vec(),
                _ => value.to_be_bytes().to_vec(),
            }
        }
        PointType::I16 => {
            let value = checked_i16(value)?;
            match format {
                PayloadFormat::Text | PayloadFormat::Json => value.to_string().into_bytes(),
                _ if little => value.to_le_bytes().to_vec(),
                _ => value.to_be_bytes().to_vec(),
            }
        }
        PointType::U32 => {
            let value = checked_u32(value)?;
            match format {
                PayloadFormat::Text | PayloadFormat::Json => value.to_string().into_bytes(),
                _ if little => value.to_le_bytes().to_vec(),
                _ => value.to_be_bytes().to_vec(),
            }
        }
        PointType::I32 => {
            let value = checked_i32(value)?;
            match format {
                PayloadFormat::Text | PayloadFormat::Json => value.to_string().into_bytes(),
                _ if little => value.to_le_bytes().to_vec(),
                _ => value.to_be_bytes().to_vec(),
            }
        }
        PointType::F32 => {
            let value = checked_f32(value)?;
            match format {
                PayloadFormat::Text => value.to_string().into_bytes(),
                PayloadFormat::Json => serde_json::Number::from_f64(f64::from(value))
                    .ok_or_else(|| "mqtt f32 JSON value is not finite".to_string())?
                    .to_string()
                    .into_bytes(),
                _ if little => value.to_le_bytes().to_vec(),
                _ => value.to_be_bytes().to_vec(),
            }
        }
        PointType::Bool => return Err("mqtt bool cannot be encoded as numeric".to_string()),
    };
    Ok(bytes)
}

/// `payload` as UTF-8 text.
fn utf8(payload: &[u8]) -> Result<&str, String> {
    core::str::from_utf8(payload)
        .map_err(|error| format!("mqtt text payload is not UTF-8: {error}"))
}

/// `payload` as one JSON value.
fn json(payload: &[u8]) -> Result<Value, String> {
    serde_json::from_slice(payload).map_err(|error| format!("mqtt JSON payload: {error}"))
}

/// `value` as a `u16`.
pub(super) fn checked_u16(value: f64) -> Result<u16, String> {
    checked_integer(value, 0.0, f64::from(u16::MAX), "u16")
}

/// `value` as an `i16`.
pub(super) fn checked_i16(value: f64) -> Result<i16, String> {
    checked_integer(value, f64::from(i16::MIN), f64::from(i16::MAX), "i16")
}

/// `value` as a `u32`.
pub(super) fn checked_u32(value: f64) -> Result<u32, String> {
    checked_integer(value, 0.0, f64::from(u32::MAX), "u32")
}

/// `value` as an `i32`.
pub(super) fn checked_i32(value: f64) -> Result<i32, String> {
    checked_integer(value, f64::from(i32::MIN), f64::from(i32::MAX), "i32")
}

/// `value` rounded to the nearest whole number, a half away from zero, then checked against `min`
/// and `max` (specification 32 §7): `mqtt <name> value is not finite` for a value that is not,
/// and `mqtt <name> value <rounded> is out of range` beyond the range.
#[allow(clippy::cast_possible_truncation)] // a whole number from `min` to `max`, which an i64 holds
fn checked_integer<T: TryFrom<i64>>(
    value: f64,
    min: f64,
    max: f64,
    name: &str,
) -> Result<T, String> {
    if !value.is_finite() {
        return Err(format!("mqtt {name} value is not finite"));
    }
    let rounded = round_half_away(value);
    if rounded < min || rounded > max {
        return Err(format!("mqtt {name} value {rounded} is out of range"));
    }
    T::try_from(rounded as i64).map_err(|_| format!("mqtt {name} value {rounded} is out of range"))
}

/// `value` as an `f32`, which must stay finite: refused with `mqtt f32 value <value> is out of
/// range` when it is not finite or lies beyond the largest `f32`.
#[allow(clippy::cast_possible_truncation)] // within the f32 range, so the nearest f32 is finite
pub(super) fn checked_f32(value: f64) -> Result<f32, String> {
    if !value.is_finite() || value < f64::from(f32::MIN) || value > f64::from(f32::MAX) {
        return Err(format!("mqtt f32 value {value} is out of range"));
    }
    Ok(value as f32)
}

/// `value` rounded to the nearest whole number, a half away from zero, as `f64::round` rounds; by
/// hand, because a chip has no `f64::round` without the standard library.
// The casts are exact below 2^52, see the body.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn round_half_away(value: f64) -> f64 {
    // From 2^52 on, and for a value that is not a number, there is nothing to round. Below it the
    // cast toward zero, the cast back and the subtraction are exact.
    if !(-WHOLE_FROM..=WHOLE_FROM).contains(&value) {
        return value;
    }
    let whole = value as i64;
    let fraction = value - whole as f64;
    let rounded = if fraction >= 0.5 {
        whole + 1
    } else if fraction <= -0.5 {
        whole - 1
    } else {
        whole
    };
    rounded as f64
}
