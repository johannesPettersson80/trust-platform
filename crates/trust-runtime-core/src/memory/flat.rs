//! The bytes of one leaf of a flat block (specification 34 MEM-12): every variable at its declared
//! size and alignment, read and written as the type of its layout says, with no tag in the block.
//!
//! A [`Cell`](crate::memory::cells::Cell) is the decoded value of one scalar, enumeration, instance
//! or reference leaf, as an operand is: the block never holds one.
//! [`decode`](crate::memory::flat::decode) reads a leaf's bytes into a cell and
//! [`encode`](crate::memory::flat::encode) writes a cell into them;
//! [`read_text`](crate::memory::flat::read_text) and
//! [`write_text`](crate::memory::flat::write_text) do the same for a text leaf, whose characters lie inline after their length. Every multi-byte value is little-endian on
//! every target, so a block holds the same bytes on a chip and on a computer.

use crate::error::RuntimeError;
use crate::memory::cells::Cell;
use crate::memory::layout::LayoutRegion;
use crate::memory::{Generation, InstanceId};
use crate::value::string_capacity_cut;

/// How the bytes of one leaf are read and written: the declared type of the leaf, as its layout
/// gives it (MEM-12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeafKind {
    /// A primitive of specification 12 §6.2, by its id. A `STRING` or `WSTRING` leaf is read with
    /// [`read_text`] and written with [`write_text`] instead.
    Primitive(u16),
    /// An enumeration of TYPE_TABLE type `type_idx`, stored as its base primitive `base`.
    Enumeration {
        /// The enumeration's TYPE_TABLE type.
        type_idx: u32,
        /// The primitive id of its base type.
        base: u16,
    },
    /// A function-block, class or interface instance leaf: the identity, then a byte that says
    /// whether the leaf names one (MEM-12's 8 bytes).
    Instance,
    /// A reference leaf: its first byte says whether it holds a reference (MEM-12's 16 bytes).
    Reference,
}

/// The kind byte of a native counter slot (primitive `0x0100`) that no call has bound yet.
const UNBOUND: u8 = 0;

/// The first byte of a reference leaf (MEM-12): no reference, a reference its region's table holds,
/// or a flat reference, whose block, offset and generation follow it in the leaf (C1-11d).
const NULL_REFERENCE: u8 = 0;
const TABLE_REFERENCE: u8 = 1;
const FLAT_REFERENCE: u8 = 2;

/// Read the leaf of kind `kind` from its bytes `bytes`, which start at the leaf's offset.
///
/// # Errors
///
/// [`RuntimeError::InvalidExecutionState`] when `bytes` is shorter than the leaf, and
/// [`RuntimeError::TypeMismatch`] for a primitive the format does not define.
pub fn decode(kind: LeafKind, bytes: &[u8]) -> Result<Cell, RuntimeError> {
    match kind {
        LeafKind::Primitive(prim_id) => decode_primitive(prim_id, bytes),
        LeafKind::Enumeration { type_idx, base } => {
            let value = match decode_primitive(base, bytes)? {
                Cell::SInt(v) => i64::from(v),
                Cell::Int(v) => i64::from(v),
                Cell::DInt(v) => i64::from(v),
                Cell::LInt(v) => v,
                Cell::USInt(v) | Cell::Byte(v) => i64::from(v),
                Cell::UInt(v) | Cell::Word(v) => i64::from(v),
                Cell::UDInt(v) | Cell::DWord(v) => i64::from(v),
                Cell::ULInt(v) | Cell::LWord(v) => {
                    i64::try_from(v).map_err(|_| RuntimeError::TypeMismatch)?
                }
                _ => return Err(RuntimeError::TypeMismatch),
            };
            Ok(Cell::Enum { type_idx, value })
        }
        LeafKind::Instance => {
            let id = u32::from_le_bytes(take::<4>(bytes, 0)?);
            Ok(if take::<1>(bytes, 4)?[0] == 0 {
                Cell::Null
            } else {
                Cell::Instance(InstanceId(id))
            })
        }
        LeafKind::Reference => Ok(match take::<1>(bytes, 0)?[0] {
            NULL_REFERENCE => Cell::NullRef,
            FLAT_REFERENCE => Cell::Flat {
                region: match take::<1>(bytes, 1)?[0] {
                    0 => LayoutRegion::Persistent,
                    1 => LayoutRegion::Activation,
                    _ => return Err(RuntimeError::InvalidExecutionState),
                },
                elements: u16::from_le_bytes(take::<2>(bytes, 2)?),
                offset: u32::from_le_bytes(take::<4>(bytes, 4)?),
                generation: Generation(u32::from_le_bytes(take::<4>(bytes, 8)?)),
            },
            _ => Cell::Ref(crate::memory::cells::RefSlot(0)),
        }),
    }
}

/// Write `cell` into the leaf of kind `kind` whose bytes `bytes` start at the leaf's offset.
///
/// # Errors
///
/// [`RuntimeError::TypeMismatch`] when `cell` is not a value of the leaf's type, and
/// [`RuntimeError::InvalidExecutionState`] when `bytes` is shorter than the leaf. Nothing is written
/// when either is refused.
pub fn encode(kind: LeafKind, cell: Cell, bytes: &mut [u8]) -> Result<(), RuntimeError> {
    match (kind, cell) {
        (LeafKind::Primitive(prim_id), cell) => encode_primitive(prim_id, cell, bytes),
        (LeafKind::Enumeration { base, .. }, Cell::Enum { value, .. }) => {
            encode_integer(base, value, bytes)
        }
        (LeafKind::Instance, Cell::Instance(InstanceId(id))) => {
            put(bytes, 0, &id.to_le_bytes())?;
            put(bytes, 4, &[1])
        }
        (LeafKind::Instance, Cell::Null) => put(bytes, 0, &[0; 5]),
        (LeafKind::Reference, Cell::Ref(_)) => put(bytes, 0, &[TABLE_REFERENCE]),
        (LeafKind::Reference, Cell::NullRef) => put(bytes, 0, &[NULL_REFERENCE]),
        (
            LeafKind::Reference,
            Cell::Flat {
                region,
                elements,
                offset,
                generation,
            },
        ) => {
            let mut leaf = [0u8; 16];
            leaf[0] = FLAT_REFERENCE;
            leaf[1] = match region {
                LayoutRegion::Persistent => 0,
                LayoutRegion::Activation => 1,
            };
            leaf[2..4].copy_from_slice(&elements.to_le_bytes());
            leaf[4..8].copy_from_slice(&offset.to_le_bytes());
            leaf[8..12].copy_from_slice(&generation.0.to_le_bytes());
            put(bytes, 0, &leaf)
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

/// The text of a text leaf whose bytes start at its length field of `length_bytes` bytes, as the
/// UTF-8 of a `STRING` or the UTF-16 code units of a `WSTRING` (`wide`).
///
/// # Errors
///
/// [`RuntimeError::InvalidExecutionState`] when the length is longer than the leaf, or a `STRING`'s
/// bytes are not UTF-8.
pub fn read_text<'a>(
    bytes: &'a [u8],
    length_bytes: u32,
    wide: bool,
) -> Result<TextBytes<'a>, RuntimeError> {
    let field = usize::try_from(length_bytes).map_err(|_| RuntimeError::Overflow)?;
    let length = if field == 4 {
        usize::try_from(u32::from_le_bytes(take::<4>(bytes, 0)?))
            .map_err(|_| RuntimeError::Overflow)?
    } else {
        usize::from(u16::from_le_bytes(take::<2>(bytes, 0)?))
    };
    let units = if wide { 2 } else { 1 };
    let used = length
        .checked_mul(units)
        .ok_or(RuntimeError::InvalidExecutionState)?;
    let text = bytes
        .get(field..)
        .and_then(|rest| rest.get(..used))
        .ok_or(RuntimeError::InvalidExecutionState)?;
    if wide {
        Ok(TextBytes::Wide(text))
    } else {
        core::str::from_utf8(text)
            .map(TextBytes::Narrow)
            .map_err(|_| RuntimeError::InvalidExecutionState)
    }
}

/// The characters of a text leaf in place: the UTF-8 of a `STRING`, or the little-endian UTF-16 code
/// units of a `WSTRING`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextBytes<'a> {
    /// A `STRING`'s UTF-8.
    Narrow(&'a str),
    /// A `WSTRING`'s code units, two little-endian bytes each.
    Wide(&'a [u8]),
}

impl TextBytes<'_> {
    /// The code units of a `WSTRING`, in order; none for a `STRING`.
    pub fn units(self) -> impl Iterator<Item = u16> + Clone {
        let wide = match self {
            TextBytes::Wide(bytes) => bytes,
            TextBytes::Narrow(_) => &[][..],
        };
        wide.as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
    }
}

/// Write `text` into a text leaf whose bytes start at its length field of `length_bytes` bytes and
/// hold `capacity_bytes` bytes of characters, cut exactly where assignment normalization cuts it
/// (specification 02 §6): at the last whole character that fits. The capacity after the text is
/// zeroed, so a leaf's bytes depend only on its text.
///
/// # Errors
///
/// [`RuntimeError::InvalidExecutionState`] when `bytes` is shorter than the leaf.
pub fn write_text(
    bytes: &mut [u8],
    length_bytes: u32,
    capacity_bytes: u32,
    text: &str,
    wide: bool,
) -> Result<(), RuntimeError> {
    let field = usize::try_from(length_bytes).map_err(|_| RuntimeError::Overflow)?;
    let capacity = usize::try_from(capacity_bytes).map_err(|_| RuntimeError::Overflow)?;
    let units = if wide { capacity / 2 } else { capacity };
    let kept = match string_capacity_cut(text, wide, units) {
        Some(end) => text.get(..end).ok_or(RuntimeError::InvalidExecutionState)?,
        None => text,
    };
    let area = bytes
        .get_mut(field..)
        .and_then(|rest| rest.get_mut(..capacity))
        .ok_or(RuntimeError::InvalidExecutionState)?;
    let length = if wide {
        let mut written = 0usize;
        for unit in kept.encode_utf16() {
            let at = written * 2;
            area.get_mut(at..at + 2)
                .ok_or(RuntimeError::InvalidExecutionState)?
                .copy_from_slice(&unit.to_le_bytes());
            written += 1;
        }
        area.get_mut(written * 2..)
            .ok_or(RuntimeError::InvalidExecutionState)?
            .fill(0);
        written
    } else {
        area.get_mut(..kept.len())
            .ok_or(RuntimeError::InvalidExecutionState)?
            .copy_from_slice(kept.as_bytes());
        area.get_mut(kept.len()..)
            .ok_or(RuntimeError::InvalidExecutionState)?
            .fill(0);
        kept.len()
    };
    if field == 4 {
        let length = u32::try_from(length).map_err(|_| RuntimeError::Overflow)?;
        put(bytes, 0, &length.to_le_bytes())
    } else {
        let length = u16::try_from(length).map_err(|_| RuntimeError::Overflow)?;
        put(bytes, 0, &length.to_le_bytes())
    }
}

/// Whether the native counter slot (primitive `0x0100`) whose bytes are `bytes` is bound.
pub fn native_counter_bound(bytes: &[u8]) -> bool {
    bytes.get(8).is_some_and(|kind| *kind != UNBOUND)
}

/// Read one primitive from its bytes.
fn decode_primitive(prim_id: u16, bytes: &[u8]) -> Result<Cell, RuntimeError> {
    Ok(match prim_id {
        1 => Cell::Bool(take::<1>(bytes, 0)?[0] != 0),
        2 => Cell::Byte(take::<1>(bytes, 0)?[0]),
        3 => Cell::Word(u16::from_le_bytes(take(bytes, 0)?)),
        4 => Cell::DWord(u32::from_le_bytes(take(bytes, 0)?)),
        5 => Cell::LWord(u64::from_le_bytes(take(bytes, 0)?)),
        6 => Cell::SInt(i8::from_le_bytes(take(bytes, 0)?)),
        7 => Cell::Int(i16::from_le_bytes(take(bytes, 0)?)),
        8 => Cell::DInt(i32::from_le_bytes(take(bytes, 0)?)),
        9 => Cell::LInt(i64::from_le_bytes(take(bytes, 0)?)),
        10 => Cell::USInt(take::<1>(bytes, 0)?[0]),
        11 => Cell::UInt(u16::from_le_bytes(take(bytes, 0)?)),
        12 => Cell::UDInt(u32::from_le_bytes(take(bytes, 0)?)),
        13 => Cell::ULInt(u64::from_le_bytes(take(bytes, 0)?)),
        14 => Cell::Real(f32::from_le_bytes(take(bytes, 0)?)),
        15 => Cell::LReal(f64::from_le_bytes(take(bytes, 0)?)),
        16..=23 => {
            let ticks = i64::from_le_bytes(take(bytes, 0)?);
            match prim_id {
                16 => Cell::Time(ticks),
                17 => Cell::LTime(ticks),
                18 => Cell::Date(ticks),
                19 => Cell::LDate(ticks),
                20 => Cell::Tod(ticks),
                21 => Cell::LTod(ticks),
                22 => Cell::Dt(ticks),
                _ => Cell::Ldt(ticks),
            }
        }
        26 => Cell::Char(take::<1>(bytes, 0)?[0]),
        27 => Cell::WChar(u16::from_le_bytes(take(bytes, 0)?)),
        0x0100 => {
            if native_counter_bound(bytes) {
                decode_native_counter(bytes)?
            } else {
                Cell::Null
            }
        }
        _ => return Err(RuntimeError::TypeMismatch),
    })
}

/// Read a bound native counter slot: its value, of the integer kind its kind byte names.
fn decode_native_counter(bytes: &[u8]) -> Result<Cell, RuntimeError> {
    let kind = u16::from(take::<1>(bytes, 8)?[0]);
    decode_primitive(kind, bytes)
}

/// Write one primitive into its bytes.
fn encode_primitive(prim_id: u16, cell: Cell, bytes: &mut [u8]) -> Result<(), RuntimeError> {
    match (prim_id, cell) {
        (1, Cell::Bool(v)) => put(bytes, 0, &[u8::from(v)]),
        (2, Cell::Byte(v)) | (10, Cell::USInt(v)) | (26, Cell::Char(v)) => put(bytes, 0, &[v]),
        (3, Cell::Word(v)) | (11, Cell::UInt(v)) | (27, Cell::WChar(v)) => {
            put(bytes, 0, &v.to_le_bytes())
        }
        (4, Cell::DWord(v)) | (12, Cell::UDInt(v)) => put(bytes, 0, &v.to_le_bytes()),
        (5, Cell::LWord(v)) | (13, Cell::ULInt(v)) => put(bytes, 0, &v.to_le_bytes()),
        (6, Cell::SInt(v)) => put(bytes, 0, &v.to_le_bytes()),
        (7, Cell::Int(v)) => put(bytes, 0, &v.to_le_bytes()),
        (8, Cell::DInt(v)) => put(bytes, 0, &v.to_le_bytes()),
        (9, Cell::LInt(v)) => put(bytes, 0, &v.to_le_bytes()),
        (14, Cell::Real(v)) => put(bytes, 0, &v.to_le_bytes()),
        (15, Cell::LReal(v)) => put(bytes, 0, &v.to_le_bytes()),
        (16, Cell::Time(v))
        | (17, Cell::LTime(v))
        | (18, Cell::Date(v))
        | (19, Cell::LDate(v))
        | (20, Cell::Tod(v))
        | (21, Cell::LTod(v))
        | (22, Cell::Dt(v))
        | (23, Cell::Ldt(v)) => put(bytes, 0, &v.to_le_bytes()),
        (0x0100, Cell::Null) => put(bytes, 0, &[0; 9]),
        (0x0100, cell) => {
            let kind = integer_kind(cell).ok_or(RuntimeError::TypeMismatch)?;
            encode_primitive(u16::from(kind), cell, bytes)?;
            put(bytes, 8, &[kind])
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

/// The primitive id of an integer cell, which a native counter slot keeps as its kind byte.
fn integer_kind(cell: Cell) -> Option<u8> {
    Some(match cell {
        Cell::SInt(_) => 6,
        Cell::Int(_) => 7,
        Cell::DInt(_) => 8,
        Cell::LInt(_) => 9,
        Cell::USInt(_) => 10,
        Cell::UInt(_) => 11,
        Cell::UDInt(_) => 12,
        Cell::ULInt(_) => 13,
        Cell::Byte(_) => 2,
        Cell::Word(_) => 3,
        Cell::DWord(_) => 4,
        Cell::LWord(_) => 5,
        _ => return None,
    })
}

/// Write the enumeration value `value` as its base integer primitive `base`.
fn encode_integer(base: u16, value: i64, bytes: &mut [u8]) -> Result<(), RuntimeError> {
    let refuse = |_| RuntimeError::TypeMismatch;
    let cell = match base {
        6 => Cell::SInt(i8::try_from(value).map_err(refuse)?),
        7 => Cell::Int(i16::try_from(value).map_err(refuse)?),
        8 => Cell::DInt(i32::try_from(value).map_err(refuse)?),
        9 => Cell::LInt(value),
        10 => Cell::USInt(u8::try_from(value).map_err(refuse)?),
        11 => Cell::UInt(u16::try_from(value).map_err(refuse)?),
        12 => Cell::UDInt(u32::try_from(value).map_err(refuse)?),
        13 => Cell::ULInt(u64::try_from(value).map_err(refuse)?),
        2 => Cell::Byte(u8::try_from(value).map_err(refuse)?),
        3 => Cell::Word(u16::try_from(value).map_err(refuse)?),
        4 => Cell::DWord(u32::try_from(value).map_err(refuse)?),
        5 => Cell::LWord(u64::try_from(value).map_err(refuse)?),
        _ => return Err(RuntimeError::TypeMismatch),
    };
    encode_primitive(base, cell, bytes)
}

/// The `N` bytes of `bytes` from `at`.
fn take<const N: usize>(bytes: &[u8], at: usize) -> Result<[u8; N], RuntimeError> {
    bytes
        .get(
            at..at
                .checked_add(N)
                .ok_or(RuntimeError::InvalidExecutionState)?,
        )
        .and_then(|slice| <[u8; N]>::try_from(slice).ok())
        .ok_or(RuntimeError::InvalidExecutionState)
}

/// Write `value` into `bytes` from `at`.
fn put(bytes: &mut [u8], at: usize, value: &[u8]) -> Result<(), RuntimeError> {
    bytes
        .get_mut(
            at..at
                .checked_add(value.len())
                .ok_or(RuntimeError::InvalidExecutionState)?,
        )
        .ok_or(RuntimeError::InvalidExecutionState)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests;
