//! The values of a compiled program's variables, read and written through the image's type table
//! (specification 44 §5.8).
//!
//! A value is read as the interpreter holds it: an enumeration by its declared type and value names,
//! a structure by its declared type and member names, an array with its bounds, a text as text. A
//! value is written with the interpreter's conversions (specification 11 §6.7): a number widened
//! within its family, a `REAL` to an `LREAL`, a text cut to its capacity without splitting a
//! character, an enumeration of the same type and value, an array of the same bounds element by
//! element, a structure member by member, where a member the value lacks takes its starting value.
//! The checkpoint of the retained variables (§11) and the tools that read and write a variable by
//! its symbol share this one access.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

use smol_str::SmolStr;

use super::image::{
    type_kind as kind, Image, MemberDetail, SectionId, Table, TypeRecord, MEMBER_BYTES,
};
use crate::bytecode::BYTECODE_MAX_TYPE_NESTING;
use crate::collections::OrderedMap;
use crate::error::RuntimeError;
use crate::value::{
    string_capacity_cut, ArrayValue, DateTimeValue, DateValue, Duration, EnumValue, LDateTimeValue,
    LDateValue, LTimeOfDayValue, StructValue, TimeOfDayValue, Value,
};

/// The bytes of one of a compiled program's areas, where the program holds them.
pub(crate) trait AreaBytes {
    /// The area's length in bytes.
    fn len(&self) -> usize;

    /// Copies the bytes from `offset` into `out`; `false` when they are not all in the area.
    fn read(&self, offset: usize, out: &mut [u8]) -> bool;
}

impl AreaBytes for [u8] {
    fn len(&self) -> usize {
        <[u8]>::len(self)
    }

    fn read(&self, offset: usize, out: &mut [u8]) -> bool {
        match offset
            .checked_add(out.len())
            .and_then(|end| self.get(offset..end))
        {
            Some(bytes) => {
                out.copy_from_slice(bytes);
                true
            }
            None => false,
        }
    }
}

/// The failure of a table of the image that does not read as the build wrote it.
pub(crate) fn damaged(what: &str) -> RuntimeError {
    RuntimeError::InvalidBundle(SmolStr::new(format!(
        "the program image's {what} does not read"
    )))
}

/// The type table with its detail list (specification 44 §5.8).
pub(crate) struct Types<'i> {
    image: Image<'i>,
    records: Table<'i, TypeRecord>,
    section: &'i [u8],
}

impl<'i> Types<'i> {
    pub(crate) fn of(image: &Image<'i>) -> Result<Self, RuntimeError> {
        Ok(Self {
            image: *image,
            records: image
                .table::<TypeRecord>()
                .map_err(|_| damaged("type table"))?,
            section: image.section_bytes(SectionId::Types),
        })
    }

    pub(crate) fn get(&self, index: u32) -> Result<TypeRecord, RuntimeError> {
        self.records.get(index).ok_or_else(|| damaged("type table"))
    }

    /// The number of types in the table.
    pub(crate) fn len(&self) -> u32 {
        self.records.len()
    }

    pub(crate) fn string(&self, offset: u32) -> Result<&'i str, RuntimeError> {
        self.image
            .string(offset)
            .ok_or_else(|| damaged("string pool"))
    }

    /// `bytes` bytes of the detail of `ty`.
    pub(crate) fn detail(&self, ty: &TypeRecord, bytes: usize) -> Result<&'i [u8], RuntimeError> {
        let start = usize::try_from(ty.detail).map_err(|_| RuntimeError::Overflow)?;
        start
            .checked_add(bytes)
            .and_then(|end| self.section.get(start..end))
            .ok_or_else(|| damaged("type table"))
    }

    /// `count` records of `width` bytes in the detail of `ty`.
    fn records_of(
        &self,
        ty: &TypeRecord,
        prefix: usize,
        width: usize,
    ) -> Result<&'i [u8], RuntimeError> {
        let count = usize::try_from(ty.count).map_err(|_| RuntimeError::Overflow)?;
        let bytes = count
            .checked_mul(width)
            .and_then(|bytes| bytes.checked_add(prefix))
            .ok_or(RuntimeError::Overflow)?;
        self.detail(ty, bytes).map(|detail| &detail[prefix..])
    }

    /// The members of a structure, in their declared order.
    pub(crate) fn members(
        &self,
        ty: &TypeRecord,
    ) -> Result<impl Iterator<Item = MemberDetail> + 'i, RuntimeError> {
        Ok(self
            .records_of(ty, 0, MEMBER_BYTES)?
            .as_chunks::<MEMBER_BYTES>()
            .0
            .iter()
            .map(|member| MemberDetail::read(member)))
    }

    /// The values of an enumeration: each name's string offset and its number.
    pub(crate) fn variants(
        &self,
        ty: &TypeRecord,
    ) -> Result<impl Iterator<Item = (u32, i64)> + 'i, RuntimeError> {
        Ok(self
            .records_of(ty, 0, 16)?
            .as_chunks::<16>()
            .0
            .iter()
            .map(|value| (u32_at(value, 0), i64_at(value, 8))))
    }

    /// A subrange's elementary base kind and its bounds.
    pub(crate) fn subrange(&self, ty: &TypeRecord) -> Result<(u32, i64, i64), RuntimeError> {
        let detail = self.detail(ty, 24)?;
        Ok((u32_at(detail, 0), i64_at(detail, 8), i64_at(detail, 16)))
    }

    /// An array's element type and the bounds of its dimensions.
    pub(crate) fn array(
        &self,
        ty: &TypeRecord,
    ) -> Result<(u32, impl ExactSizeIterator<Item = (i64, i64)> + Clone + 'i), RuntimeError> {
        let element = u32_at(self.detail(ty, 8)?, 0);
        let dimensions = self
            .records_of(ty, 8, 16)?
            .as_chunks::<16>()
            .0
            .iter()
            .map(|bounds| (i64_at(bounds, 0), i64_at(bounds, 8)));
        Ok((element, dimensions))
    }

    /// The elements of an array of `dimensions` and the bytes between two of them.
    pub(crate) fn layout(
        &self,
        element: u32,
        dimensions: impl Iterator<Item = (i64, i64)>,
    ) -> Result<(usize, usize), RuntimeError> {
        let mut count = 1usize;
        for (lower, upper) in dimensions {
            let length = upper
                .checked_sub(lower)
                .and_then(|span| span.checked_add(1))
                .and_then(|length| usize::try_from(length).ok())
                .ok_or_else(|| damaged("type table"))?;
            count = count.checked_mul(length).ok_or(RuntimeError::Overflow)?;
        }
        let stride =
            usize::try_from(self.get(element)?.size).map_err(|_| RuntimeError::Overflow)?;
        Ok((count, stride))
    }
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    let mut word = [0u8; 4];
    word.copy_from_slice(&bytes[at..at + 4]);
    u32::from_le_bytes(word)
}

fn i64_at(bytes: &[u8], at: usize) -> i64 {
    let mut word = [0u8; 8];
    word.copy_from_slice(&bytes[at..at + 8]);
    i64::from_le_bytes(word)
}

/// The bytes of a value of the elementary kind `kind`; `None` for a kind that is not elementary.
pub(crate) fn width(kind: u32) -> Option<usize> {
    Some(match kind {
        kind::BOOL | kind::SINT | kind::USINT | kind::BYTE | kind::CHAR => 1,
        kind::INT | kind::UINT | kind::WORD | kind::WCHAR => 2,
        kind::DINT | kind::UDINT | kind::DWORD | kind::REAL => 4,
        kind::LINT
        | kind::ULINT
        | kind::LWORD
        | kind::LREAL
        | kind::TIME
        | kind::LTIME
        | kind::DATE
        | kind::LDATE
        | kind::TOD
        | kind::LTOD
        | kind::DT
        | kind::LDT => 8,
        _ => return None,
    })
}

/// Reads the bytes at `offset` of `area` into `out`.
pub(crate) fn read<A: AreaBytes + ?Sized>(
    area: &A,
    offset: usize,
    out: &mut [u8],
) -> Result<(), RuntimeError> {
    if area.read(offset, out) {
        Ok(())
    } else {
        Err(damaged("retain map"))
    }
}

/// The number of `width` bytes at `offset`, zero-extended.
pub(crate) fn read_unsigned<A: AreaBytes + ?Sized>(
    area: &A,
    offset: usize,
    width: usize,
) -> Result<u64, RuntimeError> {
    let mut bytes = [0u8; 8];
    read(area, offset, &mut bytes[..width.min(8)])?;
    Ok(u64::from_le_bytes(bytes))
}

/// `unsigned`, a number of `width` bytes, sign-extended.
fn sign_extend(unsigned: u64, width: usize) -> i64 {
    let shift = 64 - 8 * u32::try_from(width.clamp(1, 8)).unwrap_or(8);
    (i64::from_le_bytes(unsigned.to_le_bytes()) << shift) >> shift
}

/// The declared value of the enumeration `ty` that the `width` bytes `unsigned` hold: its name's
/// string offset and its number, or `None` when the type declares no such value. The base is signed
/// or unsigned; one reading is a declared value.
///
/// # Errors
///
/// [`RuntimeError::InvalidBundle`] for a type whose detail does not read.
pub(crate) fn variant_of(
    types: &Types<'_>,
    ty: &TypeRecord,
    unsigned: u64,
    width: usize,
) -> Result<Option<(u32, i64)>, RuntimeError> {
    let signed = sign_extend(unsigned, width);
    let zero_extended = i64::try_from(unsigned).unwrap_or(signed);
    Ok(types
        .variants(ty)?
        .find(|(_, number)| *number == signed || *number == zero_extended))
}

/// The length of the text at `offset`, at most `capacity`.
pub(crate) fn text_length<A: AreaBytes + ?Sized>(
    area: &A,
    offset: usize,
    capacity: u32,
) -> Result<usize, RuntimeError> {
    let length = read_unsigned(area, offset, 2)?;
    if length > u64::from(capacity) {
        return Err(RuntimeError::StringCapacityExceeded);
    }
    usize::try_from(length).map_err(|_| RuntimeError::Overflow)
}

/// The length in code units of the `WSTRING` at `offset`, at most `capacity`, whose units all lie
/// in `area`.
pub(crate) fn wide_length<A: AreaBytes + ?Sized>(
    area: &A,
    offset: usize,
    capacity: u32,
) -> Result<usize, RuntimeError> {
    let length = text_length(area, offset, capacity)?;
    let end = length
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(offset))
        .and_then(|end| end.checked_add(2))
        .ok_or(RuntimeError::Overflow)?;
    if end > area.len() {
        return Err(damaged("retain map"));
    }
    Ok(length)
}

/// The UTF-16 code units of the `WSTRING` of `length` units at `offset`, which [`wide_length`]
/// found in the area.
pub(crate) fn wide_units<A: AreaBytes + ?Sized>(
    area: &A,
    offset: usize,
    length: usize,
) -> impl Iterator<Item = u16> + '_ {
    (0..length).map(move |unit| {
        let mut bytes = [0u8; 2];
        // `wide_length` found every unit in the area.
        let _ = area.read(offset + 2 + 2 * unit, &mut bytes);
        u16::from_le_bytes(bytes)
    })
}

/// The family of an integer or bit-string kind, for the widening a write allows.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    Signed,
    Unsigned,
    Bits,
}

/// The family and the width of an integer or bit-string kind.
fn integer_kind(kind: u32) -> Option<(Family, usize)> {
    Some(match kind {
        kind::SINT => (Family::Signed, 1),
        kind::INT => (Family::Signed, 2),
        kind::DINT => (Family::Signed, 4),
        kind::LINT => (Family::Signed, 8),
        kind::USINT => (Family::Unsigned, 1),
        kind::UINT => (Family::Unsigned, 2),
        kind::UDINT => (Family::Unsigned, 4),
        kind::ULINT => (Family::Unsigned, 8),
        kind::BYTE => (Family::Bits, 1),
        kind::WORD => (Family::Bits, 2),
        kind::DWORD => (Family::Bits, 4),
        kind::LWORD => (Family::Bits, 8),
        _ => return None,
    })
}

/// The family, the width and the number of an integer or bit-string value.
fn integer_value(value: &Value) -> Option<(Family, usize, i128)> {
    Some(match *value {
        Value::SInt(v) => (Family::Signed, 1, i128::from(v)),
        Value::Int(v) => (Family::Signed, 2, i128::from(v)),
        Value::DInt(v) => (Family::Signed, 4, i128::from(v)),
        Value::LInt(v) => (Family::Signed, 8, i128::from(v)),
        Value::USInt(v) => (Family::Unsigned, 1, i128::from(v)),
        Value::UInt(v) => (Family::Unsigned, 2, i128::from(v)),
        Value::UDInt(v) => (Family::Unsigned, 4, i128::from(v)),
        Value::ULInt(v) => (Family::Unsigned, 8, i128::from(v)),
        Value::Byte(v) => (Family::Bits, 1, i128::from(v)),
        Value::Word(v) => (Family::Bits, 2, i128::from(v)),
        Value::DWord(v) => (Family::Bits, 4, i128::from(v)),
        Value::LWord(v) => (Family::Bits, 8, i128::from(v)),
        _ => return None,
    })
}

/// Values written into a copy of an area's bytes, `staged`; a structure member a written value
/// lacks takes its bytes from `starting`, the same place in the bytes the variable starts from.
pub(crate) struct Writer<'a, 'i> {
    pub(crate) types: &'a Types<'i>,
    pub(crate) staged: &'a mut [u8],
    pub(crate) starting: &'a [u8],
}

impl Writer<'_, '_> {
    /// The `length` staged bytes at `offset`.
    fn slot(&mut self, offset: usize, length: usize) -> Result<&mut [u8], RuntimeError> {
        offset
            .checked_add(length)
            .and_then(|end| self.staged.get_mut(offset..end))
            .ok_or_else(|| damaged("symbol table"))
    }

    /// Writes `bytes` at `offset`.
    fn put(&mut self, offset: usize, bytes: &[u8]) -> Result<(), RuntimeError> {
        self.slot(offset, bytes.len())?.copy_from_slice(bytes);
        Ok(())
    }

    /// Writes `value` as a value of type `index` at `offset`, converted by the interpreter's
    /// migration rules (specification 11 §6.7).
    ///
    /// # Errors
    ///
    /// [`RuntimeError::TypeMismatch`] for a value the type refuses,
    /// [`RuntimeError::SubrangeViolation`] for a number outside its subrange and
    /// [`RuntimeError::RetainMigration`] for a structure with a physical backing. The staged bytes
    /// may hold part of the value then.
    pub(crate) fn write(
        &mut self,
        index: u32,
        offset: usize,
        value: &Value,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        if depth > BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        let ty = self.types.get(index)?;
        match ty.kind {
            kind::ENUM => {
                let Value::Enum(value) = value else {
                    return Err(RuntimeError::TypeMismatch);
                };
                if !value
                    .type_name()
                    .eq_ignore_ascii_case(self.types.string(ty.name)?)
                {
                    return Err(RuntimeError::TypeMismatch);
                }
                let mut declared = None;
                for (name, number) in self.types.variants(&ty)? {
                    if number == value.numeric_value()
                        && self
                            .types
                            .string(name)?
                            .eq_ignore_ascii_case(value.variant_name())
                    {
                        declared = Some(number);
                        break;
                    }
                }
                let number = declared.ok_or(RuntimeError::TypeMismatch)?;
                let width = usize::try_from(ty.size).map_err(|_| RuntimeError::Overflow)?;
                self.put(offset, &number.to_le_bytes()[..width.min(8)])
            }
            kind::SUBRANGE => {
                let (base, lower, upper) = self.types.subrange(&ty)?;
                self.write_scalar(base, offset, value)?;
                let (_, _, number) = integer_value(value)
                    .filter(|(family, _, _)| *family != Family::Bits)
                    .ok_or(RuntimeError::TypeMismatch)?;
                if number < i128::from(lower) || number > i128::from(upper) {
                    return Err(RuntimeError::SubrangeViolation {
                        value: number,
                        lower: i128::from(lower),
                        upper: i128::from(upper),
                    });
                }
                Ok(())
            }
            kind::STRING => {
                let Value::String(text) = value else {
                    return Err(RuntimeError::TypeMismatch);
                };
                let capacity = usize::try_from(ty.count).map_err(|_| RuntimeError::Overflow)?;
                let end = string_capacity_cut(text, false, capacity).unwrap_or(text.len());
                let kept = &text.as_bytes()[..end];
                self.write_text(offset, capacity, kept.len(), kept)
            }
            kind::WSTRING => {
                let Value::WString(text) = value else {
                    return Err(RuntimeError::TypeMismatch);
                };
                let capacity = usize::try_from(ty.count).map_err(|_| RuntimeError::Overflow)?;
                let end = string_capacity_cut(text, true, capacity).unwrap_or(text.len());
                let mut units = Vec::with_capacity(capacity.saturating_mul(2));
                for unit in text[..end].encode_utf16() {
                    units.extend_from_slice(&unit.to_le_bytes());
                }
                self.write_text(offset, capacity * 2, units.len() / 2, &units)
            }
            kind::ARRAY => {
                let Value::Array(array) = value else {
                    return Err(RuntimeError::TypeMismatch);
                };
                let (element, dimensions) = self.types.array(&ty)?;
                if !array.dimensions().iter().copied().eq(dimensions.clone()) {
                    return Err(RuntimeError::TypeMismatch);
                }
                let (count, stride) = self.types.layout(element, dimensions)?;
                if array.elements().len() != count {
                    return Err(RuntimeError::TypeMismatch);
                }
                for (position, element_value) in array.elements().iter().enumerate() {
                    let at = position
                        .checked_mul(stride)
                        .and_then(|from| from.checked_add(offset))
                        .ok_or(RuntimeError::Overflow)?;
                    self.write(element, at, element_value, depth + 1)?;
                }
                Ok(())
            }
            kind::STRUCT => {
                let Value::Struct(structure) = value else {
                    return Err(RuntimeError::TypeMismatch);
                };
                if structure.physical_layout().is_some() {
                    return Err(RuntimeError::RetainMigration(
                        "retained physical backing requires a compatible physical destination"
                            .into(),
                    ));
                }
                for member in self.types.members(&ty)? {
                    let name = self.types.string(member.name)?;
                    let at = offset
                        .checked_add(
                            usize::try_from(member.offset).map_err(|_| RuntimeError::Overflow)?,
                        )
                        .ok_or(RuntimeError::Overflow)?;
                    let field = structure
                        .fields()
                        .iter()
                        .find(|(field, _)| field.eq_ignore_ascii_case(name));
                    match field {
                        Some((_, field)) => self.write(member.type_index, at, field, depth + 1)?,
                        // A member the saved value lacks takes its starting value: its declared
                        // initial value, or its type's.
                        None => {
                            let size = usize::try_from(self.types.get(member.type_index)?.size)
                                .map_err(|_| RuntimeError::Overflow)?;
                            let source = self.starting;
                            let starting = at
                                .checked_add(size)
                                .and_then(|end| source.get(at..end))
                                .ok_or_else(|| damaged("starting memory"))?;
                            self.put(at, starting)?;
                        }
                    }
                }
                Ok(())
            }
            other => self.write_scalar(other, offset, value),
        }
    }

    /// Writes a text of `length` units, `bytes` long, after its length, and clears the rest of
    /// its `room` bytes.
    fn write_text(
        &mut self,
        offset: usize,
        room: usize,
        length: usize,
        bytes: &[u8],
    ) -> Result<(), RuntimeError> {
        let length = u16::try_from(length).map_err(|_| RuntimeError::Overflow)?;
        self.put(offset, &length.to_le_bytes())?;
        let text = self.slot(offset + 2, room)?;
        let (written, rest) = text.split_at_mut(bytes.len().min(room));
        written.copy_from_slice(&bytes[..written.len()]);
        rest.fill(0);
        Ok(())
    }

    /// Writes the elementary `value` as kind `kind` at `offset`: a number widened within its family,
    /// a `REAL` as an `LREAL`, every other value only as its own kind (specification 11 §6.7).
    fn write_scalar(
        &mut self,
        kind: u32,
        offset: usize,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        if let Some((family, width)) = integer_kind(kind) {
            let (source, source_width, number) =
                integer_value(value).ok_or(RuntimeError::TypeMismatch)?;
            if source != family || source_width > width {
                return Err(RuntimeError::TypeMismatch);
            }
            // The number fits the wider kind, so its low bytes are its value.
            let bytes = match family {
                Family::Signed => i64::try_from(number)
                    .map_err(|_| RuntimeError::TypeMismatch)?
                    .to_le_bytes(),
                Family::Unsigned | Family::Bits => u64::try_from(number)
                    .map_err(|_| RuntimeError::TypeMismatch)?
                    .to_le_bytes(),
            };
            return self.put(offset, &bytes[..width]);
        }
        let bytes: ([u8; 8], usize) = match (kind, value) {
            (kind::BOOL, Value::Bool(flag)) => ([u8::from(*flag), 0, 0, 0, 0, 0, 0, 0], 1),
            (kind::REAL, Value::Real(number)) => (widen(&number.to_le_bytes()), 4),
            (kind::LREAL, Value::Real(number)) => (f64::from(*number).to_le_bytes(), 8),
            (kind::LREAL, Value::LReal(number)) => (number.to_le_bytes(), 8),
            (kind::TIME, Value::Time(span)) | (kind::LTIME, Value::LTime(span)) => {
                (span.as_nanos().to_le_bytes(), 8)
            }
            (kind::DATE, Value::Date(date)) => (date.ticks().to_le_bytes(), 8),
            (kind::LDATE, Value::LDate(date)) => (date.nanos().to_le_bytes(), 8),
            (kind::TOD, Value::Tod(time)) => (time.ticks().to_le_bytes(), 8),
            (kind::LTOD, Value::LTod(time)) => (time.nanos().to_le_bytes(), 8),
            (kind::DT, Value::Dt(stamp)) => (stamp.ticks().to_le_bytes(), 8),
            (kind::LDT, Value::Ldt(stamp)) => (stamp.nanos().to_le_bytes(), 8),
            (kind::CHAR, Value::Char(character)) => ([*character, 0, 0, 0, 0, 0, 0, 0], 1),
            (kind::WCHAR, Value::WChar(character)) => (widen(&character.to_le_bytes()), 2),
            _ => return Err(RuntimeError::TypeMismatch),
        };
        self.put(offset, &bytes.0[..bytes.1])
    }
}

/// `bytes` at the start of eight.
fn widen(bytes: &[u8]) -> [u8; 8] {
    let mut wide = [0u8; 8];
    wide[..bytes.len()].copy_from_slice(bytes);
    wide
}

/// The value of type `index` at `offset` of `area`, as the interpreter holds it: an enumeration by
/// its declared names, a structure by its declared type and member names.
///
/// # Errors
///
/// [`RuntimeError::TypeMismatch`] for bytes that are no value of the type (an undeclared
/// enumeration value, a text that is not UTF-8 or UTF-16), and [`RuntimeError::InvalidBundle`] for
/// a type table that does not read or a value outside the area.
pub(crate) fn read_value<A: AreaBytes + ?Sized>(
    types: &Types<'_>,
    area: &A,
    offset: usize,
    index: u32,
) -> Result<Value, RuntimeError> {
    decode(types, area, offset, index, 0)
}

fn decode<A: AreaBytes + ?Sized>(
    types: &Types<'_>,
    area: &A,
    offset: usize,
    index: u32,
    depth: usize,
) -> Result<Value, RuntimeError> {
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    let ty = types.get(index)?;
    match ty.kind {
        kind::ENUM => {
            let width = usize::try_from(ty.size).map_err(|_| RuntimeError::Overflow)?;
            let unsigned = read_unsigned(area, offset, width)?;
            let (name, number) =
                variant_of(types, &ty, unsigned, width)?.ok_or(RuntimeError::TypeMismatch)?;
            Ok(Value::Enum(Box::new(EnumValue::from_canonical_parts(
                SmolStr::new(types.string(ty.name)?),
                SmolStr::new(types.string(name)?),
                number,
            ))))
        }
        kind::SUBRANGE => decode_scalar(area, offset, types.subrange(&ty)?.0),
        kind::STRING => {
            let length = text_length(area, offset, ty.count)?;
            let mut bytes = vec![0u8; length];
            read(area, offset + 2, &mut bytes)?;
            let text = String::from_utf8(bytes).map_err(|_| RuntimeError::TypeMismatch)?;
            Ok(Value::String(SmolStr::new(text)))
        }
        kind::WSTRING => {
            let length = wide_length(area, offset, ty.count)?;
            let units: Vec<u16> = wide_units(area, offset, length).collect();
            let text = String::from_utf16(&units).map_err(|_| RuntimeError::TypeMismatch)?;
            Ok(Value::WString(text))
        }
        kind::ARRAY => {
            let (element, dimensions) = types.array(&ty)?;
            let (count, stride) = types.layout(element, dimensions.clone())?;
            let mut elements = Vec::with_capacity(count);
            for position in 0..count {
                let at = position
                    .checked_mul(stride)
                    .and_then(|from| from.checked_add(offset))
                    .ok_or(RuntimeError::Overflow)?;
                elements.push(decode(types, area, at, element, depth + 1)?);
            }
            Ok(Value::Array(Box::new(ArrayValue::from_canonical_parts(
                elements,
                dimensions.collect(),
            ))))
        }
        kind::STRUCT => {
            let mut fields = OrderedMap::default();
            for member in types.members(&ty)? {
                let at = offset
                    .checked_add(
                        usize::try_from(member.offset).map_err(|_| RuntimeError::Overflow)?,
                    )
                    .ok_or(RuntimeError::Overflow)?;
                fields.insert(
                    SmolStr::new(types.string(member.name)?),
                    decode(types, area, at, member.type_index, depth + 1)?,
                );
            }
            Ok(Value::Struct(Arc::new(StructValue::from_canonical_parts(
                SmolStr::new(types.string(ty.name)?),
                fields,
            ))))
        }
        other => decode_scalar(area, offset, other),
    }
}

/// The elementary value of kind `kind` at `offset` of `area`.
fn decode_scalar<A: AreaBytes + ?Sized>(
    area: &A,
    offset: usize,
    kind: u32,
) -> Result<Value, RuntimeError> {
    let width = width(kind).ok_or(RuntimeError::TypeMismatch)?;
    let unsigned = read_unsigned(area, offset, width)?;
    let signed = sign_extend(unsigned, width);
    let narrow = |_| RuntimeError::TypeMismatch;
    Ok(match kind {
        kind::BOOL => Value::Bool(unsigned != 0),
        kind::SINT => Value::SInt(i8::try_from(signed).map_err(narrow)?),
        kind::INT => Value::Int(i16::try_from(signed).map_err(narrow)?),
        kind::DINT => Value::DInt(i32::try_from(signed).map_err(narrow)?),
        kind::LINT => Value::LInt(signed),
        kind::USINT => Value::USInt(u8::try_from(unsigned).map_err(narrow)?),
        kind::UINT => Value::UInt(u16::try_from(unsigned).map_err(narrow)?),
        kind::UDINT => Value::UDInt(u32::try_from(unsigned).map_err(narrow)?),
        kind::ULINT => Value::ULInt(unsigned),
        kind::BYTE => Value::Byte(u8::try_from(unsigned).map_err(narrow)?),
        kind::WORD => Value::Word(u16::try_from(unsigned).map_err(narrow)?),
        kind::DWORD => Value::DWord(u32::try_from(unsigned).map_err(narrow)?),
        kind::LWORD => Value::LWord(unsigned),
        kind::REAL => Value::Real(f32::from_bits(u32::try_from(unsigned).map_err(narrow)?)),
        kind::LREAL => Value::LReal(f64::from_bits(unsigned)),
        kind::TIME => Value::Time(Duration::from_nanos(signed)),
        kind::LTIME => Value::LTime(Duration::from_nanos(signed)),
        kind::DATE => Value::Date(DateValue::new(signed)),
        kind::LDATE => Value::LDate(LDateValue::new(signed)),
        kind::TOD => Value::Tod(TimeOfDayValue::new(signed)),
        kind::LTOD => Value::LTod(LTimeOfDayValue::new(signed)),
        kind::DT => Value::Dt(DateTimeValue::new(signed)),
        kind::LDT => Value::Ldt(LDateTimeValue::new(signed)),
        kind::CHAR => Value::Char(u8::try_from(unsigned).map_err(narrow)?),
        kind::WCHAR => Value::WChar(u16::try_from(unsigned).map_err(narrow)?),
        _ => return Err(RuntimeError::TypeMismatch),
    })
}
