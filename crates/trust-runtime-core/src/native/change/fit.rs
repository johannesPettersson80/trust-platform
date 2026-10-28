//! The elementary value an online change keeps when a variable's declaration changed (specification
//! 44 §12.2; specification 11 §6.7.1 and the owner's "keep what fits" of 20 October 2026), worked
//! out on the bytes of the two type tables' kinds.
//!
//! The new type holds the running value when the value, converted to the new type and back to the
//! old one, comes back unchanged, by the conversion the interpreter gives an initializer of the new
//! type (`vm::construction::values::scalar::coerce`). So an `INT` 5 becomes a `DINT` 5 and a `DINT`
//! 5 an `INT` 5, while a `DINT` 70,000 does not fit an `INT`; a bit string never converts to or
//! from an integer, nor an integer to or from a real, because the way back refuses it; a `REAL`
//! becomes an `LREAL` when it is finite, an `LREAL` a `REAL` when it is finite and exact; a `TIME`
//! and an `LTIME` convert both ways, a date only to its own kind, and a `CHAR` never to a `WCHAR`.
//! A text is kept when the new capacity holds it uncut, in bytes for a `STRING` and in code units
//! for a `WSTRING`, both ways between the two; a `CHAR` or a `WCHAR` and a text of that one
//! character convert where the way back gives the same character. A subrange holds a value of its
//! base within its bounds, and an enumeration keeps a value by its name, without regard to case.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use super::plan::{fetch, Spot};
use super::reserve;
use super::shape::Ty;
use crate::error::RuntimeError;
use crate::native::image::{type_kind as kind, TypeRecord};
use crate::native::memory::ProgramMemory;
use crate::native::values::{damaged, variant_of, width, Types};

/// An elementary type: a number of a kind from `BOOL` to `WCHAR`, an enumeration, a subrange or a
/// text.
#[derive(Clone, Copy)]
pub(super) enum Elementary {
    /// A number of this elementary kind.
    Number(u32),
    /// An enumeration, whose values its record's detail lists.
    Enumeration(TypeRecord),
    /// A subrange of an elementary kind.
    Subrange { base: u32, lower: i64, upper: i64 },
    /// A `STRING`, or a `WSTRING` when `wide`, of a capacity.
    Text { wide: bool, capacity: u32 },
}

/// The elementary type `ty` is, or `None` for another type.
pub(super) fn elementary(types: &Types<'_>, ty: &Ty) -> Result<Option<Elementary>, RuntimeError> {
    let record = ty.record;
    Ok(Some(match record.kind {
        kind::ENUM => Elementary::Enumeration(record),
        kind::SUBRANGE => {
            let (base, lower, upper) = types.subrange(&record)?;
            // A subrange's value is its base's, in the subrange's bytes.
            if width(base).is_none_or(|width| width as u64 != u64::from(record.size)) {
                return Err(damaged("type table"));
            }
            Elementary::Subrange { base, lower, upper }
        }
        kind::STRING => Elementary::Text {
            wide: false,
            capacity: record.count,
        },
        kind::WSTRING => Elementary::Text {
            wide: true,
            capacity: record.count,
        },
        other if width(other).is_some() => Elementary::Number(other),
        _ => return Ok(None),
    }))
}

/// A running elementary value, as its bytes hold it.
pub(super) enum Held<'i> {
    /// A number of an elementary kind, its bytes zero-extended.
    Number { kind: u32, bits: u64 },
    /// An enumeration's value by its name; `None` for a number its type does not declare.
    Variant(Option<&'i str>),
    /// A `STRING`'s bytes.
    Text(Vec<u8>),
    /// A `WSTRING`'s code units.
    Wide(Vec<u16>),
}

/// The running program's value of the elementary type `ty` at `at`, read with the running
/// program's type table `types`.
pub(super) fn read<'i>(
    memory: &dyn ProgramMemory,
    at: Spot,
    ty: &Elementary,
    types: &Types<'i>,
) -> Result<Held<'i>, RuntimeError> {
    Ok(match *ty {
        Elementary::Number(kind) | Elementary::Subrange { base: kind, .. } => {
            let width = width(kind).ok_or_else(|| damaged("type table"))?;
            Held::Number {
                kind,
                bits: bits(memory, at, width)?,
            }
        }
        Elementary::Enumeration(record) => {
            let width = record.size as usize;
            let bits = bits(memory, at, width)?;
            let variant = variant_of(types, &record, bits, width)?;
            Held::Variant(variant.map(|(name, _)| types.string(name)).transpose()?)
        }
        Elementary::Text { wide, capacity } => {
            let mut length = [0u8; 2];
            fetch(memory, at, &mut length)?;
            // A length above the capacity reads as the capacity, as the program reads it
            // (specification 44 §6.2).
            let length = u32::from(u16::from_le_bytes(length)).min(capacity) as usize;
            let size = if wide { 2 * length } else { length };
            let mut bytes = Vec::new();
            reserve(&mut bytes, size)?;
            bytes.resize(size, 0);
            fetch(memory, at.at(2)?, &mut bytes)?;
            if wide {
                let mut units = Vec::new();
                reserve(&mut units, length)?;
                units.extend(
                    bytes
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|unit| u16::from_le_bytes(*unit)),
                );
                Held::Wide(units)
            } else {
                Held::Text(bytes)
            }
        }
    })
}

/// The `width` bytes at `at`, at most eight, zero-extended.
fn bits(memory: &dyn ProgramMemory, at: Spot, width: usize) -> Result<u64, RuntimeError> {
    let mut bytes = [0u8; 8];
    let part = bytes
        .get_mut(..width)
        .ok_or_else(|| damaged("type table"))?;
    fetch(memory, at, part)?;
    Ok(u64::from_le_bytes(bytes))
}

/// Appends to `out` the bytes of the value of the new program's elementary type `ty` that holds
/// `held`, reading `ty` with the new program's type table `types`, and returns whether `ty` holds
/// it; `out` is as it was when it does not.
pub(super) fn fit(
    held: &Held<'_>,
    ty: &Elementary,
    types: &Types<'_>,
    out: &mut Vec<u8>,
) -> Result<bool, RuntimeError> {
    Ok(match *ty {
        Elementary::Number(kind) => {
            let bits = match held {
                Held::Number { kind: old, bits } => convert(*old, *bits, kind),
                Held::Text(text) => match text.as_slice() {
                    [byte] if kind == kind::CHAR && byte.is_ascii() => Some(u64::from(*byte)),
                    _ => None,
                },
                Held::Wide(units) => match units.as_slice() {
                    [unit] if kind == kind::CHAR && *unit < 0x80 => Some(u64::from(*unit)),
                    [unit] if kind == kind::WCHAR && !surrogate(*unit) => Some(u64::from(*unit)),
                    _ => None,
                },
                Held::Variant(_) => None,
            };
            bits.is_some_and(|bits| push_number(out, kind, bits))
        }
        Elementary::Subrange { base, lower, upper } => {
            let Held::Number { kind: old, bits } = held else {
                return Ok(false);
            };
            let Some(bits) = convert(*old, *bits, base) else {
                return Ok(false);
            };
            // A subrange holds an integer within its bounds.
            let within = match Number::of(base, bits) {
                Some(Number::Integer(number)) => {
                    (i128::from(lower)..=i128::from(upper)).contains(&number)
                }
                _ => false,
            };
            within && push_number(out, base, bits)
        }
        Elementary::Enumeration(record) => {
            let Held::Variant(Some(name)) = held else {
                return Ok(false);
            };
            let mut number = None;
            for (variant, value) in types.variants(&record)? {
                if types.string(variant)?.eq_ignore_ascii_case(name) {
                    number = Some(value);
                    break;
                }
            }
            let Some(number) = number else {
                return Ok(false);
            };
            let bytes = number.to_le_bytes();
            match bytes.get(..record.size as usize) {
                Some(bytes) => {
                    out.extend_from_slice(bytes);
                    true
                }
                None => false,
            }
        }
        Elementary::Text {
            wide: false,
            capacity,
        } => match held {
            Held::Text(text) => push_text(out, text, capacity),
            Held::Wide(units) => {
                let mut text = String::new();
                for character in char::decode_utf16(units.iter().copied()) {
                    let Ok(character) = character else {
                        return Ok(false);
                    };
                    text.push(character);
                }
                push_text(out, text.as_bytes(), capacity)
            }
            Held::Number {
                kind: kind::CHAR,
                bits,
            } => u8::try_from(*bits)
                .ok()
                .filter(u8::is_ascii)
                .is_some_and(|byte| push_text(out, &[byte], capacity)),
            _ => false,
        },
        Elementary::Text {
            wide: true,
            capacity,
        } => {
            let units: Cow<'_, [u16]> = match held {
                Held::Wide(units) => Cow::Borrowed(units.as_slice()),
                Held::Text(text) => match core::str::from_utf8(text) {
                    Ok(text) => Cow::Owned(text.encode_utf16().collect()),
                    Err(_) => return Ok(false),
                },
                Held::Number {
                    kind: kind::CHAR,
                    bits,
                } if *bits < 0x80 => Cow::Owned(vec![u16::from(*bits as u8)]),
                Held::Number {
                    kind: kind::WCHAR,
                    bits,
                } => match u16::try_from(*bits) {
                    Ok(unit) if !surrogate(unit) => Cow::Owned(vec![unit]),
                    _ => return Ok(false),
                },
                _ => return Ok(false),
            };
            push_wide(out, &units, capacity)
        }
    })
}

/// Whether `unit` is half of a UTF-16 surrogate pair, which is no character of its own.
fn surrogate(unit: u16) -> bool {
    (0xD800..=0xDFFF).contains(&unit)
}

/// Appends the `bits` of a number of `kind`.
fn push_number(out: &mut Vec<u8>, kind: u32, bits: u64) -> bool {
    let bytes = bits.to_le_bytes();
    match width(kind).and_then(|width| bytes.get(..width)) {
        Some(bytes) => {
            out.extend_from_slice(bytes);
            true
        }
        None => false,
    }
}

/// Appends a `STRING` of `capacity` bytes that holds `text` uncut: its length, its bytes and zeros
/// for the rest; `false` when the capacity does not hold it.
fn push_text(out: &mut Vec<u8>, text: &[u8], capacity: u32) -> bool {
    let capacity = capacity as usize;
    let Ok(length) = u16::try_from(text.len()) else {
        return false;
    };
    if text.len() > capacity {
        return false;
    }
    out.extend_from_slice(&length.to_le_bytes());
    out.extend_from_slice(text);
    out.resize(out.len() + (capacity - text.len()), 0);
    true
}

/// Appends a `WSTRING` of `capacity` code units that holds `units` uncut: its length, its units
/// and zeros for the rest; `false` when the capacity does not hold it.
fn push_wide(out: &mut Vec<u8>, units: &[u16], capacity: u32) -> bool {
    let capacity = capacity as usize;
    let Ok(length) = u16::try_from(units.len()) else {
        return false;
    };
    if units.len() > capacity {
        return false;
    }
    out.extend_from_slice(&length.to_le_bytes());
    for unit in units {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    out.resize(out.len() + 2 * (capacity - units.len()), 0);
    true
}

/// The bits of the number `bits` of kind `old` converted to kind `new`, when the conversion and its
/// way back give the number unchanged: the interpreter's coercion of an initializer, both ways.
pub(super) fn convert(old: u32, bits: u64, new: u32) -> Option<u64> {
    let value = Number::of(old, bits)?;
    let converted = value.coerce(new)?;
    let back = Number::of(new, converted)?.coerce(old)?;
    (Number::of(old, back)? == value).then_some(converted)
}

/// A number as the coercion sees it.
#[derive(Clone, Copy, PartialEq)]
enum Number {
    Bool(bool),
    /// A signed or unsigned integer.
    Integer(i128),
    /// A bit string.
    Bits(u64),
    Real(f32),
    LReal(f64),
    /// A `TIME` or an `LTIME`, in nanoseconds.
    Duration(i64),
    /// A date or a time of day of one kind.
    Date {
        kind: u32,
        ticks: i64,
    },
    Char(u8),
    WChar(u16),
}

impl Number {
    /// The number of kind `kind` whose bytes, zero-extended, are `bits`.
    fn of(kind: u32, bits: u64) -> Option<Self> {
        let width = width(kind)?;
        let signed = sign_extend(bits, width);
        Some(match kind {
            kind::BOOL => Self::Bool(bits & 0xFF != 0),
            kind::SINT | kind::INT | kind::DINT | kind::LINT => Self::Integer(i128::from(signed)),
            kind::USINT | kind::UINT | kind::UDINT | kind::ULINT => Self::Integer(i128::from(bits)),
            kind::BYTE | kind::WORD | kind::DWORD | kind::LWORD => Self::Bits(bits),
            kind::REAL => Self::Real(f32::from_bits(u32::try_from(bits).ok()?)),
            kind::LREAL => Self::LReal(f64::from_bits(bits)),
            kind::TIME | kind::LTIME => Self::Duration(signed),
            kind::DATE | kind::LDATE | kind::TOD | kind::LTOD | kind::DT | kind::LDT => {
                Self::Date {
                    kind,
                    ticks: signed,
                }
            }
            kind::CHAR => Self::Char(u8::try_from(bits).ok()?),
            kind::WCHAR => Self::WChar(u16::try_from(bits).ok()?),
            _ => return None,
        })
    }

    /// The bits of this number coerced to kind `kind`, as the interpreter coerces an initializer
    /// of that kind, or `None` when the coercion refuses it.
    fn coerce(self, kind: u32) -> Option<u64> {
        let width = width(kind)?;
        match kind {
            kind::BOOL => match self {
                Self::Bool(flag) => Some(u64::from(flag)),
                _ => None,
            },
            kind::BYTE | kind::WORD | kind::DWORD | kind::LWORD => match self {
                Self::Bits(bits) => unsigned(i128::from(bits), width),
                Self::Integer(number) => unsigned(number, width),
                _ => None,
            },
            kind::SINT | kind::INT | kind::DINT | kind::LINT => match self {
                Self::Integer(number) => signed(number, width),
                _ => None,
            },
            kind::USINT | kind::UINT | kind::UDINT | kind::ULINT => match self {
                Self::Integer(number) => unsigned(number, width),
                _ => None,
            },
            kind::REAL | kind::LREAL => {
                let number = match self {
                    Self::Real(number) => f64::from(number),
                    Self::LReal(number) => number,
                    Self::Integer(number) => number as f64,
                    _ => return None,
                };
                if !number.is_finite() {
                    return None;
                }
                if kind == kind::LREAL {
                    return Some(number.to_bits());
                }
                let narrow = number as f32;
                narrow.is_finite().then(|| u64::from(narrow.to_bits()))
            }
            kind::TIME | kind::LTIME => match self {
                Self::Duration(nanoseconds) => Some(nanoseconds as u64),
                _ => None,
            },
            kind::CHAR => match self {
                Self::Char(character) => Some(u64::from(character)),
                _ => None,
            },
            kind::WCHAR => match self {
                Self::WChar(character) => Some(u64::from(character)),
                _ => None,
            },
            _ => match self {
                Self::Date { kind: held, ticks } if held == kind => Some(ticks as u64),
                _ => None,
            },
        }
    }
}

/// `bits`, a number of `width` bytes, sign-extended.
fn sign_extend(bits: u64, width: usize) -> i64 {
    let shift = 64 - 8 * u32::try_from(width.clamp(1, 8)).unwrap_or(8);
    (bits.cast_signed() << shift) >> shift
}

/// The bits of `number` as a signed integer of `width` bytes, when it fits.
fn signed(number: i128, width: usize) -> Option<u64> {
    let half = 1i128 << (8 * width - 1);
    (-half..half)
        .contains(&number)
        .then(|| number as u64 & mask(width))
}

/// The bits of `number` as an unsigned integer of `width` bytes, when it fits.
fn unsigned(number: i128, width: usize) -> Option<u64> {
    (0..1i128 << (8 * width))
        .contains(&number)
        .then_some(number as u64)
}

/// The bits of a number of `width` bytes.
fn mask(width: usize) -> u64 {
    u64::MAX >> (64 - 8 * width.clamp(1, 8))
}
