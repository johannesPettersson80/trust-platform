//! The point maps `input_points` and `output_points` (specification 11 §6.5, "Typed protocol
//! point-map integration contract" and "Modbus declared-width floating-point finiteness"): each
//! point binds one coil, input or register value to one scalar of the process image, with linear
//! scaling, the byte and word order of its registers, and a conversion that rounds or narrows to
//! the declared type and refuses what does not fit, so that a failed point leaves its destination
//! unchanged.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ops::Range;

use trust_runtime_core::numeric::math::round;

use super::params::{
    InputFunction, OutputFunction, INPUT_WHAT, INPUT_WORDS, OUTPUT_WHAT, OUTPUT_WORDS,
};
use super::plan::ADDRESS_END;
use super::{word, Failure};
use crate::io_drivers::{ParamsError, Table};

/// The keys of a point.
const POINT_KEYS: [&str; 9] = [
    "address",
    "byte_order",
    "data_type",
    "function",
    "image_bit",
    "image_offset",
    "offset",
    "scale",
    "word_order",
];

/// The words that name a point's type, read as `word` reads them: the names of specification 11,
/// its IEC-style and explicit-width aliases, and the computer's `coil` and `float`.
const TYPE_WORDS: [(&str, PointType); 18] = [
    ("bool", PointType::Bool),
    ("boolean", PointType::Bool),
    ("coil", PointType::Bool),
    ("u16", PointType::U16),
    ("uint16", PointType::U16),
    ("word", PointType::U16),
    ("i16", PointType::I16),
    ("int16", PointType::I16),
    ("int", PointType::I16),
    ("u32", PointType::U32),
    ("uint32", PointType::U32),
    ("dword", PointType::U32),
    ("i32", PointType::I32),
    ("int32", PointType::I32),
    ("dint", PointType::I32),
    ("f32", PointType::F32),
    ("real", PointType::F32),
    ("float", PointType::F32),
];

/// The types a refusal lists.
const TYPE_WHAT: &str = "bool, u16, i16, u32, i32 or f32";

/// The words of a byte or word order, read as `word` reads them.
const ORDER_WORDS: [(&str, Endian); 6] = [
    ("big", Endian::Big),
    ("be", Endian::Big),
    ("big_endian", Endian::Big),
    ("little", Endian::Little),
    ("le", Endian::Little),
    ("little_endian", Endian::Little),
];

/// The orders a refusal lists.
const ORDER_WHAT: &str = "big or little";

/// The type of a point's value (specification 11 §6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointType {
    /// `bool`: one coil or discrete input, one bit of the image.
    Bool,
    /// `u16`: one register, two bytes of the image.
    U16,
    /// `i16`: one register, two bytes of the image.
    I16,
    /// `u32`: two registers, four bytes of the image.
    U32,
    /// `i32`: two registers, four bytes of the image.
    I32,
    /// `f32`: two registers, four bytes of the image, an IEC 60559 single.
    F32,
}

impl PointType {
    /// The registers a value takes on the wire: 0 for `bool`, which is a coil or an input, 1 for
    /// the 16-bit types and 2 for the 32-bit ones.
    #[must_use]
    pub const fn registers(self) -> u16 {
        match self {
            Self::Bool => 0,
            Self::U16 | Self::I16 => 1,
            Self::U32 | Self::I32 | Self::F32 => 2,
        }
    }

    /// The image bytes a value takes: the one byte whose bit a `bool` writes, two or four.
    #[must_use]
    pub const fn image_bytes(self) -> u32 {
        match self {
            Self::Bool => 1,
            Self::U16 | Self::I16 => 2,
            Self::U32 | Self::I32 | Self::F32 => 4,
        }
    }
}

/// The order of the two bytes of a register (`byte_order`) or of the two registers of a 32-bit
/// value (`word_order`); both are big-endian unless a point says otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    /// `big`: the high byte, or the high register, first.
    Big,
    /// `little`: the low byte, or the low register, first.
    Little,
}

/// One point of a map: a coil, an input or a register value at `address`, and its scalar in the
/// image at `image_offset`.
#[derive(Debug, Clone, PartialEq)]
pub struct Point<F> {
    /// `image_offset`: the image byte where the value starts; required.
    pub image_offset: u32,
    /// `image_bit`: the bit of a `bool` point's byte, 0 to 7; default 0.
    pub image_bit: u8,
    /// `address`: the coil, the input or the first register; required.
    pub address: u16,
    /// `function`; default the driver's `input_function` or `output_function`.
    pub function: F,
    /// `data_type`; default `bool` for coils and inputs and `u16` for registers.
    pub data_type: PointType,
    /// `scale`: finite and non-zero; default 1. Inbound `engineering = raw × scale + offset`,
    /// outbound `raw = (engineering − offset) / scale`.
    pub scale: f64,
    /// `offset`: finite; default 0.
    pub offset: f64,
    /// `byte_order`; default big.
    pub byte_order: Endian,
    /// `word_order`; default big.
    pub word_order: Endian,
}

/// A point of `input_points`.
pub type InputPoint = Point<InputFunction>;

/// A point of `output_points`.
pub type OutputPoint = Point<OutputFunction>;

impl<F> Point<F> {
    /// The image bytes the point takes.
    fn image_bytes(&self) -> Range<u64> {
        let start = u64::from(self.image_offset);
        let end = start + u64::from(self.data_type.image_bytes());
        start..end
    }

    /// The image bits the point takes: one for a `bool`, all of its bytes' for a number.
    fn image_bits(&self) -> Range<u64> {
        let bytes = self.image_bytes();
        if self.data_type == PointType::Bool {
            let bit = bytes.start * 8 + u64::from(self.image_bit);
            let end = bit + 1;
            bit..end
        } else {
            bytes.start * 8..bytes.end * 8
        }
    }

    /// The addresses the point takes in its table: one for a coil, an input or a 16-bit value,
    /// two for a 32-bit value.
    fn addresses(&self) -> Range<u32> {
        let first = u32::from(self.address);
        let end = first + u32::from(self.data_type.registers().max(1));
        first..end
    }
}

/// A Modbus data table (V1.1b3 §4.3), whose addresses two points of one list may not share.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DataTable {
    /// The coils, which FC01 reads and FC05 and FC15 write.
    Coils,
    /// The discrete inputs, which FC02 reads.
    DiscreteInputs,
    /// The holding registers, which FC03 reads and FC06 and FC16 write.
    HoldingRegisters,
    /// The input registers, which FC04 reads.
    InputRegisters,
}

impl DataTable {
    /// One entry of the table, or several, in words.
    const fn noun(self, several: bool) -> &'static str {
        match (self, several) {
            (Self::Coils, false) => "coil",
            (Self::Coils, true) => "coils",
            (Self::DiscreteInputs, false) => "discrete input",
            (Self::DiscreteInputs, true) => "discrete inputs",
            (Self::HoldingRegisters, false) => "holding register",
            (Self::HoldingRegisters, true) => "holding registers",
            (Self::InputRegisters, false) => "input register",
            (Self::InputRegisters, true) => "input registers",
        }
    }
}

/// What tells the points of `input_points` and `output_points` apart: their functions, their key
/// and their image.
pub(super) trait Direction: Copy + PartialEq + 'static {
    /// The key of the list, `input_points` or `output_points`.
    const POINTS: &'static str;
    /// The image's prefix, `%I` or `%Q`.
    const PREFIX: &'static str;
    /// The image's name, `input` or `output`.
    const IMAGE: &'static str;
    /// The words that name the functions.
    const WORDS: &'static [(&'static str, Self)];
    /// The functions a refusal lists.
    const WHAT: &'static str;

    /// The table that the function reads or writes.
    fn table(self) -> DataTable;

    /// Whether the function reads or writes bits.
    fn bits(self) -> bool;

    /// Why a point of the function cannot have the type `data_type`, in the computer's words.
    fn refusal(self, data_type: PointType) -> Option<&'static str>;
}

impl Direction for InputFunction {
    const POINTS: &'static str = "input_points";
    const PREFIX: &'static str = "%I";
    const IMAGE: &'static str = "input";
    const WORDS: &'static [(&'static str, Self)] = &INPUT_WORDS;
    const WHAT: &'static str = INPUT_WHAT;

    fn table(self) -> DataTable {
        match self {
            Self::Coils => DataTable::Coils,
            Self::DiscreteInputs => DataTable::DiscreteInputs,
            Self::HoldingRegisters => DataTable::HoldingRegisters,
            Self::InputRegisters => DataTable::InputRegisters,
        }
    }

    fn bits(self) -> bool {
        self.reads_bits()
    }

    fn refusal(self, data_type: PointType) -> Option<&'static str> {
        let boolean = data_type == PointType::Bool;
        if self.reads_bits() && !boolean {
            Some("coil/discrete input mappings require data_type = \"bool\"")
        } else if !self.reads_bits() && boolean {
            Some("register mappings require a numeric data_type")
        } else {
            None
        }
    }
}

impl Direction for OutputFunction {
    const POINTS: &'static str = "output_points";
    const PREFIX: &'static str = "%Q";
    const IMAGE: &'static str = "output";
    const WORDS: &'static [(&'static str, Self)] = &OUTPUT_WORDS;
    const WHAT: &'static str = OUTPUT_WHAT;

    fn table(self) -> DataTable {
        if self.writes_bits() {
            DataTable::Coils
        } else {
            DataTable::HoldingRegisters
        }
    }

    fn bits(self) -> bool {
        self.writes_bits()
    }

    fn refusal(self, data_type: PointType) -> Option<&'static str> {
        let boolean = data_type == PointType::Bool;
        if self.writes_bits() && !boolean {
            Some("coil mappings require data_type = \"bool\"")
        } else if !self.writes_bits() && boolean {
            Some("register mappings require a numeric data_type")
        } else if self == Self::SingleRegister && data_type.registers() != 1 {
            Some("write_single_register supports only 16-bit data types")
        } else {
            None
        }
    }
}

/// The points of the list of `F`, in order, each with `default` as its function when it names
/// none; no two of them share an address of one table or a bit of the image.
pub(super) fn points<F: Direction>(
    root: &Table<'_>,
    default: F,
) -> Result<Vec<Point<F>>, ParamsError> {
    let mut points: Vec<Point<F>> = Vec::new();
    let mut keys: Vec<String> = Vec::new();
    for entry in root.list(F::POINTS)? {
        entry.only(&POINT_KEYS, "a point")?;
        let point = point(&entry, default)?;
        claim(&entry, &point, &points, &keys)?;
        keys.push(entry.path.clone());
        points.push(point);
    }
    Ok(points)
}

/// One point, with `default` as its function when it names none.
fn point<F: Direction>(entry: &Table<'_>, default: F) -> Result<Point<F>, ParamsError> {
    let image_offset = entry.required_integer("image_offset", 0, u32::MAX)?;
    let image_bit = entry.integer("image_bit", 0, 7_u8)?.unwrap_or(0);
    let address = entry.required_integer("address", 0, u16::MAX)?;
    let function = word(entry, "function", F::WORDS, F::WHAT)?.unwrap_or(default);
    let fallback = if function.bits() {
        PointType::Bool
    } else {
        PointType::U16
    };
    let data_type = word(entry, "data_type", &TYPE_WORDS, TYPE_WHAT)?.unwrap_or(fallback);
    if let Some(rule) = function.refusal(data_type) {
        let given = entry
            .get("data_type")
            .map_or_else(|| String::from("the default"), ToString::to_string);
        return Err(entry.refuse("data_type", format!("{rule}, not {given}")));
    }
    let (scale, offset) = if data_type == PointType::Bool {
        no_scaling(entry)?;
        (1.0, 0.0)
    } else {
        (
            number(entry, "scale", 1.0, true)?,
            number(entry, "offset", 0.0, false)?,
        )
    };
    let byte_order = word(entry, "byte_order", &ORDER_WORDS, ORDER_WHAT)?.unwrap_or(Endian::Big);
    let word_order = word(entry, "word_order", &ORDER_WORDS, ORDER_WHAT)?.unwrap_or(Endian::Big);
    Ok(Point {
        image_offset,
        image_bit,
        address,
        function,
        data_type,
        scale,
        offset,
        byte_order,
        word_order,
    })
}

/// Refuses `scale` and `offset` on a `bool` point (specification 11 §6.5: boolean points do not
/// accept scaling).
fn no_scaling(entry: &Table<'_>) -> Result<(), ParamsError> {
    for key in ["scale", "offset"] {
        if entry.get(key).is_some() {
            return Err(entry.refuse(
                key,
                "is not taken by a bool point: boolean points do not accept scaling",
            ));
        }
    }
    Ok(())
}

/// The finite number `key` gives, also non-zero when `non_zero`, or `default` when the key is
/// absent (specification 11 §6.5: scale finite and non-zero, offset finite).
fn number(entry: &Table<'_>, key: &str, default: f64, non_zero: bool) -> Result<f64, ParamsError> {
    let Some(value) = entry.get(key) else {
        return Ok(default);
    };
    let what = if non_zero {
        "a finite, non-zero number"
    } else {
        "a finite number"
    };
    value
        .as_f64()
        .filter(|given| given.is_finite() && (!non_zero || *given != 0.0))
        .ok_or_else(|| entry.refuse(key, format!("must be {what}, not {value}")))
}

/// Refuses `point` when its addresses run past 65535, or when it shares an address of its table
/// or a bit of the image with one of `earlier`, the list's points before it, whose keys are
/// `keys`.
fn claim<F: Direction>(
    entry: &Table<'_>,
    point: &Point<F>,
    earlier: &[Point<F>],
    keys: &[String],
) -> Result<(), ParamsError> {
    let ours = point.addresses();
    if ours.end > ADDRESS_END {
        return Err(entry.refuse(
            "address",
            format!(
                "2 {} from {} run past address 65535",
                point.function.table().noun(true),
                point.address
            ),
        ));
    }
    let our_bits = point.image_bits();
    for (other, key) in earlier.iter().zip(keys) {
        let theirs = other.addresses();
        if other.function.table() == point.function.table()
            && theirs.start < ours.end
            && ours.start < theirs.end
        {
            let verb = if point.data_type.registers() == 2 {
                "overlap"
            } else {
                "overlaps"
            };
            return Err(entry.refuse(
                "address",
                format!(
                    "{} {verb} {} of {key}; duplicate or overlapping Modbus address ranges are \
                     refused",
                    addresses_text(point),
                    addresses_text(other)
                ),
            ));
        }
        let their_bits = other.image_bits();
        if their_bits.start < our_bits.end && our_bits.start < their_bits.end {
            return Err(entry.refuse(
                "image_offset",
                format!(
                    "{} overlaps {} of {key}; overlapping process-image ranges are refused",
                    image_text(point),
                    image_text(other)
                ),
            ));
        }
    }
    Ok(())
}

/// The addresses of `point` in words, such as `holding registers 10 and 11` or `coil 7`.
fn addresses_text<F: Direction>(point: &Point<F>) -> String {
    let table = point.function.table();
    if point.data_type.registers() == 2 {
        format!(
            "{} {} and {}",
            table.noun(true),
            point.address,
            u32::from(point.address) + 1
        )
    } else {
        format!("{} {}", table.noun(false), point.address)
    }
}

/// The image scalar of `point` as an IEC address, such as `%IX0.3`, `%IW2` or `%QD4`.
fn image_text<F: Direction>(point: &Point<F>) -> String {
    let prefix = F::PREFIX;
    let offset = point.image_offset;
    match point.data_type.image_bytes() {
        2 => format!("{prefix}W{offset}"),
        4 => format!("{prefix}D{offset}"),
        _ => format!("{prefix}X{offset}.{}", point.image_bit),
    }
}

/// The first point of `points` whose bytes are not all in an image of `len` bytes.
fn outside<F>(points: &[Point<F>], len: usize) -> Option<(usize, &Point<F>)> {
    let len = u64::try_from(len).unwrap_or(u64::MAX);
    points
        .iter()
        .enumerate()
        .find(|(_, point)| point.image_bytes().end > len)
}

/// Refuses the first point of `points` that is not in its image of `len` bytes, at configuration.
pub(super) fn check_image<F: Direction>(
    points: &[Point<F>],
    len: usize,
) -> Result<(), ParamsError> {
    outside(points, len).map_or(Ok(()), |(index, point)| {
        Err(ParamsError::new(
            format!("{}[{index}].image_offset", F::POINTS),
            format!(
                "{} is past the end of the {len}-byte {} image",
                image_text(point),
                F::IMAGE
            ),
        ))
    })
}

/// Fails when a point of `points` is not in an image of `len` bytes, with the computer's text,
/// before any byte of the image changes.
pub(super) fn fits<F>(points: &[Point<F>], len: usize) -> Result<(), Failure> {
    outside(points, len).map_or(Ok(()), |(_, point)| {
        let bytes = point.image_bytes();
        Err(Failure::data(if point.data_type == PointType::Bool {
            format!(
                "modbus point image_offset {} out of range",
                point.image_offset
            )
        } else {
            format!(
                "modbus point image range {}..{} out of {len}",
                bytes.start, bytes.end
            )
        }))
    })
}

/// A value of a point, as the image or the wire holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Scalar {
    /// A `bool` point's value: its bit of the image, or its coil or input.
    Bit(bool),
    /// A number's bytes: little-endian in the image, in the point's byte and word order on the
    /// wire.
    Bytes {
        /// The bytes, of which the first `len` count.
        bytes: [u8; 4],
        /// 2 or 4.
        len: usize,
    },
}

/// The image value of an input register point, from the register bytes of its response: put into
/// big-endian order, scaled, and rounded or narrowed to the point's type.
///
/// # Errors
///
/// A wire value of another length than the type's registers, an `f32` that is not finite on the
/// wire or after narrowing, and an integer outside its type after rounding, with the computer's
/// texts.
pub(super) fn decode_input(point: &InputPoint, wire: &[u8]) -> Result<Scalar, Failure> {
    let raw = wire_number(point, wire)?;
    image_scalar(point.data_type, raw * point.scale + point.offset)
}

/// The wire value of an output point, from its bytes or bit of the output image: scaled back, and
/// rounded or narrowed to the point's type in its byte and word order.
///
/// # Errors
///
/// A point outside the image, an `f32` in the image that is not finite or that is not finite on
/// the wire after narrowing, and an integer outside its type after rounding, with the computer's
/// texts. Nothing reaches the wire then.
pub(super) fn encode_output(point: &OutputPoint, image: &[u8]) -> Result<Scalar, Failure> {
    if point.data_type == PointType::Bool {
        let byte = usize::try_from(point.image_offset)
            .ok()
            .and_then(|start| image.get(start))
            .ok_or_else(|| {
                Failure::data(format!(
                    "modbus point image_offset {} out of range",
                    point.image_offset
                ))
            })?;
        return Ok(Scalar::Bit((byte & (1_u8 << point.image_bit)) != 0));
    }
    let engineering = image_number(point, image)?;
    wire_scalar(point, (engineering - point.offset) / point.scale)
}

/// Writes `value`, the value of `point`, to its bytes of `image`, which `fits` checked: a `bool`
/// sets or clears its bit and keeps the other bits of the byte.
pub(super) fn write_input(point: &InputPoint, value: Scalar, image: &mut [u8]) {
    let Ok(start) = usize::try_from(point.image_offset) else {
        return;
    };
    match value {
        Scalar::Bit(on) => {
            if let Some(byte) = image.get_mut(start) {
                let mask = 1_u8 << point.image_bit;
                if on {
                    *byte |= mask;
                } else {
                    *byte &= !mask;
                }
            }
        }
        Scalar::Bytes { bytes, len } => {
            let target = start
                .checked_add(len)
                .and_then(|end| image.get_mut(start..end));
            if let (Some(target), Some(source)) = (target, bytes.get(..len)) {
                target.copy_from_slice(source);
            }
        }
    }
}

/// The raw value of a register point's wire bytes, which must be exactly its registers' bytes.
fn wire_number(point: &InputPoint, wire: &[u8]) -> Result<f64, Failure> {
    if point.data_type == PointType::Bool {
        return Err(Failure::data(
            "modbus bool point cannot be decoded as register numeric",
        ));
    }
    let len = usize::from(point.data_type.registers()) * 2;
    let mut bytes = [0_u8; 4];
    match bytes.get_mut(..len) {
        Some(target) if wire.len() == len => target.copy_from_slice(wire),
        _ => {
            return Err(Failure::data(format!(
                "modbus mapped register response must contain exactly {len} byte(s), got {}",
                wire.len()
            )));
        }
    }
    reorder(&mut bytes, len, point.byte_order, point.word_order);
    let [high, low, ..] = bytes;
    Ok(match point.data_type {
        PointType::U16 => f64::from(u16::from_be_bytes([high, low])),
        PointType::I16 => f64::from(i16::from_be_bytes([high, low])),
        PointType::U32 => f64::from(u32::from_be_bytes(bytes)),
        PointType::I32 => f64::from(i32::from_be_bytes(bytes)),
        PointType::F32 | PointType::Bool => {
            let value = f32::from_bits(u32::from_be_bytes(bytes));
            if !value.is_finite() {
                return Err(Failure::data("modbus mapped F32 wire value must be finite"));
            }
            f64::from(value)
        }
    })
}

/// The number in the image bytes of an output point: little-endian.
fn image_number(point: &OutputPoint, image: &[u8]) -> Result<f64, Failure> {
    let range = point.image_bytes();
    let len = image.len();
    let source = usize::try_from(range.start)
        .ok()
        .zip(usize::try_from(range.end).ok())
        .and_then(|(start, end)| image.get(start..end))
        .ok_or_else(|| {
            Failure::data(format!(
                "modbus point image range {}..{} out of {len}",
                range.start, range.end
            ))
        })?;
    let mut bytes = [0_u8; 4];
    if let Some(target) = bytes.get_mut(..source.len()) {
        target.copy_from_slice(source);
    }
    let [low, high, ..] = bytes;
    Ok(match point.data_type {
        PointType::U16 => f64::from(u16::from_le_bytes([low, high])),
        PointType::I16 => f64::from(i16::from_le_bytes([low, high])),
        PointType::U32 => f64::from(u32::from_le_bytes(bytes)),
        PointType::I32 => f64::from(i32::from_le_bytes(bytes)),
        PointType::F32 | PointType::Bool => {
            let value = f32::from_bits(u32::from_le_bytes(bytes));
            if !value.is_finite() {
                return Err(Failure::data(
                    "modbus mapped F32 process-image value must be finite",
                ));
            }
            f64::from(value)
        }
    })
}

/// The image bytes of `value`, an engineering value, for a point of `data_type`: an integer rounded
/// a half away from zero and in its type's range, an `f32` finite after narrowing; little-endian.
fn image_scalar(data_type: PointType, value: f64) -> Result<Scalar, Failure> {
    if !value.is_finite() {
        return Err(Failure::data(
            "modbus mapped process-image value must be finite",
        ));
    }
    if let Some((min, max, len)) = integer_range(data_type) {
        let all = whole(value, min, max)?.to_le_bytes();
        let mut bytes = [0_u8; 4];
        if let (Some(target), Some(source)) = (bytes.get_mut(..len), all.get(..len)) {
            target.copy_from_slice(source);
        }
        return Ok(Scalar::Bytes { bytes, len });
    }
    let narrowed = narrow(value, "modbus mapped F32 process-image value")?;
    Ok(Scalar::Bytes {
        bytes: narrowed.to_bits().to_le_bytes(),
        len: 4,
    })
}

/// The wire bytes of `value`, a raw value, for `point`: an integer rounded a half away from zero
/// and in its type's range, an `f32` finite after narrowing; big-endian, then put into the point's
/// byte and word order.
fn wire_scalar(point: &OutputPoint, value: f64) -> Result<Scalar, Failure> {
    if !value.is_finite() {
        return Err(Failure::data("modbus mapped numeric value must be finite"));
    }
    let (mut bytes, len) = if let Some((min, max, len)) = integer_range(point.data_type) {
        let all = whole(value, min, max)?.to_be_bytes();
        let mut bytes = [0_u8; 4];
        let start = all.len() - len;
        if let (Some(target), Some(source)) = (bytes.get_mut(..len), all.get(start..)) {
            target.copy_from_slice(source);
        }
        (bytes, len)
    } else {
        let narrowed = narrow(value, "modbus mapped F32 wire value")?;
        (narrowed.to_bits().to_be_bytes(), 4)
    };
    reorder(&mut bytes, len, point.byte_order, point.word_order);
    Ok(Scalar::Bytes { bytes, len })
}

/// The least and the greatest value of an integer type, and its bytes; `None` for `bool` and
/// `f32`.
fn integer_range(data_type: PointType) -> Option<(f64, f64, usize)> {
    match data_type {
        PointType::U16 => Some((f64::from(u16::MIN), f64::from(u16::MAX), 2)),
        PointType::I16 => Some((f64::from(i16::MIN), f64::from(i16::MAX), 2)),
        PointType::U32 => Some((f64::from(u32::MIN), f64::from(u32::MAX), 4)),
        PointType::I32 => Some((f64::from(i32::MIN), f64::from(i32::MAX), 4)),
        PointType::Bool | PointType::F32 => None,
    }
}

/// `value` rounded to the nearest integer, a half away from zero (specification 11 §6.5), which
/// must lie from `min` to `max`.
#[allow(clippy::cast_possible_truncation)] // a whole number within an i32 or a u32 converts exactly
fn whole(value: f64, min: f64, max: f64) -> Result<i64, Failure> {
    let rounded = round(value);
    if !(min..=max).contains(&rounded) {
        return Err(Failure::data(format!(
            "modbus mapped numeric value {value} outside {min}..={max}"
        )));
    }
    Ok(rounded as i64)
}

/// `value` narrowed to the declared `f32` width, which must stay finite (specification 11,
/// "Modbus declared-width floating-point finiteness"); `label` names the value in the refusal.
#[allow(clippy::cast_possible_truncation)] // the narrowing is the point; its result is checked
fn narrow(value: f64, label: &str) -> Result<f32, Failure> {
    let narrowed = value as f32;
    if narrowed.is_finite() {
        Ok(narrowed)
    } else {
        Err(Failure::data(format!(
            "{label} must remain finite after narrowing"
        )))
    }
}

/// Puts the first `len` bytes of `bytes`, register bytes, from the point's byte and word order into
/// big-endian order, or back, as the two swaps undo themselves (specification 11 §6.5: the byte
/// order applies inside every register, the word order between the two registers of a 32-bit
/// value).
fn reorder(bytes: &mut [u8; 4], len: usize, byte_order: Endian, word_order: Endian) {
    if byte_order == Endian::Little {
        bytes.swap(0, 1);
        bytes.swap(2, 3);
    }
    if word_order == Endian::Little && len == 4 {
        bytes.swap(0, 2);
        bytes.swap(1, 3);
    }
}
