//! A value text admitted as a value of its destination's declared type (specification 11, "Live
//! Values"; IEC 61131-3 Ed.3 §6.3.2 to §6.3.5 and §6.4.2): the IEC literal the text spells
//! ([`super::literal`]) taken as a value of the type the program's type table declares, so that a
//! chip and trust-runtime take the same text the same way (specification 34). A literal of another
//! type, outside the type's range or longer than its capacity is refused with a plain reason, and
//! nothing is cut to fit; a real number is read once, as the floating-point number nearest to its
//! decimal text.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::datetime::NANOS_PER_DAY;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::{IoAddress, IoSize};
use trust_runtime_core::memory::IoArea;
use trust_runtime_core::native::image::{
    type_kind as kind, Image, IoBindRecord, SymbolRecord, TypeRecord, BIND_OFFSET_SHIFT,
    BIND_TIME_MILLIS,
};
use trust_runtime_core::native::memory::{
    enum_value, enum_values, type_name, type_shape, write_place, Place, ProgramMemory, TypeShape,
};
use trust_runtime_core::native::reflect::leaf_type;
use trust_runtime_core::value::{
    string_capacity_cut, DateTimeValue, DateValue, Duration, LDateTimeValue, LDateValue,
    LTimeOfDayValue, TimeOfDayValue, Value,
};

use super::literal::{parse_literal, Literal};

/// The milliseconds of a day: a `DATE`, a `TIME_OF_DAY` and a `DATE_AND_TIME` of a compiled
/// program count milliseconds (specification 44 §6.8).
const MILLIS_PER_DAY: i64 = 86_400_000;
/// The nanoseconds of a millisecond.
const NANOS_PER_MILLI: i64 = 1_000_000;

/// `text`, an IEC 61131-3 literal, as a value of the variable of type `type_index` of `image`
/// (specification 11, "Live Values").
///
/// # Errors
///
/// `unsupported value '<text>'` for a text that is no literal, and a plain reason for a literal the
/// type does not take: of another type, outside the type's range, a text longer than the type holds
/// or a name that is no value of the enumeration, and for a type that holds no single value.
pub fn typed_value(image: &Image<'_>, type_index: u32, text: &str) -> Result<Value, String> {
    let literal = parse_literal(text)?;
    let target = Target::of(image, type_index, text.trim())?;
    let value = target.admit(&literal)?;
    target.bytes(&value)?;
    Ok(value)
}

/// `text` as the value of the process-image `address` of `image` when a variable is bound there
/// (specification 11, "Live Values"; specification 44 §5.6): the literal admitted as a value of the
/// bound variable's declared type, in the image's form of that variable, its bytes or, for a
/// `TIME` bound in whole milliseconds, those milliseconds. `None` for an address no variable is
/// bound to, whose text keeps the untyped grammar ([`super::parse_value`]).
#[must_use]
pub fn address_value(
    image: &Image<'_>,
    address: &IoAddress,
    text: &str,
) -> Option<Result<Value, String>> {
    let (binding, symbol) = bound(image, address)?;
    let width = binding.width();
    let leaf = leaf_type(
        image,
        symbol.type_index,
        binding.flags >> BIND_OFFSET_SHIFT,
        width,
    )
    .ok()
    .flatten()?;
    Some(typed_value(image, leaf, text).and_then(|value| {
        if binding.flags & 0xFF == BIND_TIME_MILLIS {
            return whole_millis(&value, text.trim());
        }
        if binding.size == 0 {
            return Ok(value);
        }
        let bytes = Target::of(image, leaf, text.trim())?.bytes(&value)?;
        bit_string(&bytes, width)
            .ok_or_else(|| format!("'{}' does not fit its address", text.trim()))
    }))
}

/// The binding of `address` in `image` and the bound variable's symbol (specification 44 §5.6).
fn bound(image: &Image<'_>, address: &IoAddress) -> Option<(IoBindRecord, SymbolRecord)> {
    let area = match address.area {
        IoArea::Input => 1,
        IoArea::Output => 2,
        IoArea::Memory => 3,
    };
    let size = match address.size {
        IoSize::Bit => 0,
        IoSize::Byte => 1,
        IoSize::Word => 2,
        IoSize::DWord => 3,
        IoSize::LWord => 4,
        IoSize::Bytes(_) => return None,
    };
    let hierarchical = address.path.len() > 1;
    let binding = image.table::<IoBindRecord>().ok()?.iter().find(|binding| {
        binding.area == area
            && binding.size == size
            && if hierarchical {
                image
                    .binding_address(binding)
                    .is_some_and(|bound| bound.path == address.path && bound.bit == address.bit)
            } else {
                !binding.is_hierarchical()
                    && binding.byte == address.byte
                    && (size != 0 || binding.bit == address.bit)
            }
    })?;
    let symbol = image.table::<SymbolRecord>().ok()?.get(binding.symbol)?;
    Some((binding, symbol))
}

/// A duration as the whole milliseconds of the image's double word that a `TIME` is bound to.
fn whole_millis(value: &Value, text: &str) -> Result<Value, String> {
    let (Value::Time(span) | Value::LTime(span)) = value else {
        return Err(format!("'{text}' is no duration"));
    };
    let nanos = span.as_nanos();
    if nanos % NANOS_PER_MILLI != 0 {
        return Err(format!(
            "'{text}' is finer than the millisecond its address holds"
        ));
    }
    u32::try_from(nanos / NANOS_PER_MILLI)
        .map(Value::DWord)
        .map_err(|_| format!("'{text}' is outside what its address holds"))
}

/// The bit string of `width` bytes that the little-endian `bytes` are.
fn bit_string(bytes: &[u8], width: u32) -> Option<Value> {
    let word = |count: usize| {
        let mut wide = [0u8; 8];
        wide[..count].copy_from_slice(bytes.get(..count)?);
        Some(u64::from_le_bytes(wide))
    };
    Some(match width {
        1 => Value::Byte(u8::try_from(word(1)?).ok()?),
        2 => Value::Word(u16::try_from(word(2)?).ok()?),
        4 => Value::DWord(u32::try_from(word(4)?).ok()?),
        8 => Value::LWord(word(8)?),
        _ => return None,
    })
}

/// Whether the integer `value` is held exactly by a binary floating-point number of `mantissa`
/// bits whose largest exponent is `max_exponent`: its odd part fits the mantissa.
fn exact_in_float(value: i128, mantissa: u32, max_exponent: u32) -> bool {
    let mut odd = value.unsigned_abs();
    if odd == 0 {
        return true;
    }
    let shift = odd.trailing_zeros();
    odd >>= shift;
    let bits = odd.bit_width();
    bits <= mantissa && shift + bits <= max_exponent
}

/// The type a literal is admitted to: its index, shape and declared name, with the request's text
/// for the reasons of a refusal.
struct Target<'i, 't> {
    image: Image<'i>,
    type_index: u32,
    shape: TypeShape,
    name: SmolStr,
    text: &'t str,
}

impl<'i, 't> Target<'i, 't> {
    fn of(image: &Image<'i>, type_index: u32, text: &'t str) -> Result<Self, String> {
        let shape = type_shape(image, type_index)
            .map_err(|_| String::from("the program image's type table does not read"))?;
        let declared = type_name(image, type_index);
        // A text is named with its capacity, `STRING[20]`, unless an alias names it.
        let name = match &shape {
            TypeShape::Text { wide, capacity }
                if declared.is_empty()
                    || declared.eq_ignore_ascii_case("STRING")
                    || declared.eq_ignore_ascii_case("WSTRING") =>
            {
                SmolStr::new(format!(
                    "{}[{capacity}]",
                    if *wide { "WSTRING" } else { "STRING" }
                ))
            }
            TypeShape::Elementary(kind) | TypeShape::Subrange(kind) if declared.is_empty() => {
                SmolStr::new(kind_name(*kind))
            }
            _ => declared,
        };
        Ok(Self {
            image: *image,
            type_index,
            shape,
            name,
            text,
        })
    }

    /// `literal` as a value of the target's type, before the type table's own check.
    fn admit(&self, literal: &Literal) -> Result<Value, String> {
        if let Literal::Typed { type_name, literal } = literal {
            return self.typed(type_name, literal);
        }
        match &self.shape {
            TypeShape::Elementary(kind) | TypeShape::Subrange(kind) => {
                self.elementary(*kind, literal, false)
            }
            TypeShape::Text { wide, capacity } => self.text(*wide, *capacity, literal, false),
            TypeShape::Enumeration { name, .. } => self.enumeration(name, literal),
            TypeShape::Array { .. } | TypeShape::Other => Err(self.no_single_value()),
        }
    }

    /// A typed literal `prefix#literal`: of an elementary type, which the target's type takes when
    /// it is that type or a wider one of the same kind (the type table's check decides), or of the
    /// target's enumeration.
    fn typed(&self, prefix: &str, literal: &Literal) -> Result<Value, String> {
        let upper = prefix.to_ascii_uppercase();
        if upper == "STRING" || upper == "WSTRING" {
            let wide = upper == "WSTRING";
            return match &self.shape {
                // `WSTRING#'OK'` is a double-byte string too (Table 6, feature 4c).
                TypeShape::Text {
                    wide: target,
                    capacity,
                } if *target == wide => self.text(wide, *capacity, literal, wide),
                _ => Err(self.mismatch()),
            };
        }
        if let Some(kind) = keyword_kind(&upper) {
            return self.elementary(kind, literal, true);
        }
        match &self.shape {
            TypeShape::Enumeration { name, .. } if name.eq_ignore_ascii_case(prefix) => {
                self.enumeration(name, literal)
            }
            _ => Err(self.mismatch()),
        }
    }

    /// `literal` as a value of the elementary type `kind`; a `typed` literal names its type before
    /// `#`, which lets a character take its code (`CHAR#16#41`, Table 5 feature 9) and a `WCHAR` a
    /// single-quoted character (Table 6, feature 4d).
    fn elementary(&self, kind: u32, literal: &Literal, typed: bool) -> Result<Value, String> {
        match (kind, literal) {
            (kind::BOOL, Literal::Bool(flag)) => Ok(Value::Bool(*flag)),
            // Table 5 feature 7: the integers 0 and 1 are Boolean literals too.
            (
                kind::BOOL,
                Literal::Integer {
                    value: number @ (0 | 1),
                    based: false,
                },
            ) => Ok(Value::Bool(*number == 1)),
            (kind::REAL | kind::LREAL, Literal::Real(text)) => self.real(kind, text),
            (
                kind::REAL | kind::LREAL,
                Literal::Integer {
                    value,
                    based: false,
                },
            ) => self.integral_real(kind, *value),
            (kind::TIME, Literal::Duration { long: false, nanos }) => {
                Ok(Value::Time(Duration::from_nanos(*nanos)))
            }
            (kind::LTIME, Literal::Duration { nanos, .. }) => {
                Ok(Value::LTime(Duration::from_nanos(*nanos)))
            }
            (kind::DATE, Literal::Date { long: false, days }) => {
                Ok(Value::Date(DateValue::new(self.date_millis(*days, 0)?)))
            }
            (
                kind::DT,
                Literal::DateAndTime {
                    long: false,
                    days,
                    nanos,
                },
            ) => Ok(Value::Dt(DateTimeValue::new(
                self.date_millis(*days, *nanos)?,
            ))),
            (kind::TOD, Literal::TimeOfDay { long: false, nanos }) => {
                Ok(Value::Tod(TimeOfDayValue::new(self.millis(*nanos)?)))
            }
            (kind::LDATE, Literal::Date { days, .. }) => {
                Ok(Value::LDate(LDateValue::new(self.date_nanos(*days, 0)?)))
            }
            (kind::LDT, Literal::DateAndTime { days, nanos, .. }) => Ok(Value::Ldt(
                LDateTimeValue::new(self.date_nanos(*days, *nanos)?),
            )),
            (kind::LTOD, Literal::TimeOfDay { nanos, .. }) => {
                Ok(Value::LTod(LTimeOfDayValue::new(*nanos)))
            }
            (kind::CHAR | kind::WCHAR, Literal::Text { wide, text }) => {
                let own_quotes = *wide == (kind == kind::WCHAR);
                if !(own_quotes || (typed && kind == kind::WCHAR)) {
                    return Err(self.mismatch());
                }
                self.character(kind, text)
            }
            (kind::CHAR | kind::WCHAR, Literal::Integer { value, .. }) if typed => {
                self.code(kind, *value)
            }
            (_, Literal::Integer { value, .. }) => self.integer(kind, *value),
            _ => Err(self.mismatch()),
        }
    }

    /// The integer `value` as a value of the integer or bit-string type `kind`.
    fn integer(&self, kind: u32, value: i128) -> Result<Value, String> {
        let range = |low: i128, high: i128| {
            format!(
                "'{}' is outside the range of {} ({low}..{high})",
                self.text,
                kind_name(kind)
            )
        };
        macro_rules! within {
            ($variant:ident, $ty:ty) => {
                <$ty>::try_from(value)
                    .map(Value::$variant)
                    .map_err(|_| range(i128::from(<$ty>::MIN), i128::from(<$ty>::MAX)))
            };
        }
        match kind {
            kind::SINT => within!(SInt, i8),
            kind::INT => within!(Int, i16),
            kind::DINT => within!(DInt, i32),
            kind::LINT => within!(LInt, i64),
            kind::USINT => within!(USInt, u8),
            kind::UINT => within!(UInt, u16),
            kind::UDINT => within!(UDInt, u32),
            kind::ULINT => within!(ULInt, u64),
            kind::BYTE => within!(Byte, u8),
            kind::WORD => within!(Word, u16),
            kind::DWORD => within!(DWord, u32),
            kind::LWORD => within!(LWord, u64),
            _ => Err(self.mismatch()),
        }
    }

    /// The real number of the decimal `text` as a `REAL` or `LREAL`, read once from the text.
    fn real(&self, kind: u32, text: &str) -> Result<Value, String> {
        let value = if kind == kind::REAL {
            text.parse::<f32>()
                .ok()
                .filter(|number| number.is_finite())
                .map(Value::Real)
        } else {
            text.parse::<f64>()
                .ok()
                .filter(|number| number.is_finite())
                .map(Value::LReal)
        };
        value.ok_or_else(|| self.out_of_range())
    }

    /// The integer `value` as a `REAL` or `LREAL` that holds it exactly; an integer the type would
    /// round is refused.
    fn integral_real(&self, kind: u32, value: i128) -> Result<Value, String> {
        let (mantissa, max_exponent) = if kind == kind::REAL {
            (f32::MANTISSA_DIGITS, 128)
        } else {
            (f64::MANTISSA_DIGITS, 1024)
        };
        if !exact_in_float(value, mantissa, max_exponent) {
            return Err(format!(
                "'{}' cannot be held exactly by {}",
                self.text,
                kind_name(kind)
            ));
        }
        // Held exactly, the decimal text reads as that number.
        self.real(kind, &format!("{value}"))
    }

    /// The whole milliseconds of the time of day `nanos`; a time finer than a millisecond is
    /// refused, because a `DATE`, a `TIME_OF_DAY` and a `DATE_AND_TIME` hold milliseconds.
    fn millis(&self, nanos: i64) -> Result<i64, String> {
        if nanos % NANOS_PER_MILLI == 0 {
            Ok(nanos / NANOS_PER_MILLI)
        } else {
            Err(format!(
                "'{}' is finer than the millisecond {} holds",
                self.text, self.name
            ))
        }
    }

    /// The milliseconds since 1970-01-01 of the day `days` at the time of day `nanos`.
    fn date_millis(&self, days: i64, nanos: i64) -> Result<i64, String> {
        let millis = self.millis(nanos)?;
        days.checked_mul(MILLIS_PER_DAY)
            .and_then(|ticks| ticks.checked_add(millis))
            .ok_or_else(|| self.out_of_range())
    }

    /// The nanoseconds since 1970-01-01 of the day `days` at the time of day `nanos`.
    fn date_nanos(&self, days: i64, nanos: i64) -> Result<i64, String> {
        days.checked_mul(NANOS_PER_DAY)
            .and_then(|total| total.checked_add(nanos))
            .ok_or_else(|| self.out_of_range())
    }

    /// The one character of `text` as a `CHAR` or a `WCHAR`.
    fn character(&self, kind: u32, text: &str) -> Result<Value, String> {
        let mut characters = text.chars();
        let (Some(character), None) = (characters.next(), characters.next()) else {
            return Err(self.mismatch());
        };
        self.code(kind, i128::from(u32::from(character)))
    }

    /// The character code `code` as a `CHAR`, up to 16#FF, or a `WCHAR`, up to 16#FFFF.
    fn code(&self, kind: u32, code: i128) -> Result<Value, String> {
        if kind == kind::CHAR {
            u8::try_from(code)
                .map(Value::Char)
                .map_err(|_| self.out_of_range())
        } else {
            u16::try_from(code)
                .map(Value::WChar)
                .map_err(|_| self.out_of_range())
        }
    }

    /// `literal` as a `STRING`, or a `WSTRING` when `wide`, of `capacity`: a text in the quotes of
    /// its type, single for a `STRING` and double for a `WSTRING`, or in either when
    /// `either_quotes` (`WSTRING#'OK'`, Table 6 feature 4c); a text longer than the type holds is
    /// refused.
    fn text(
        &self,
        wide: bool,
        capacity: u32,
        literal: &Literal,
        either_quotes: bool,
    ) -> Result<Value, String> {
        let Literal::Text { wide: double, text } = literal else {
            return Err(self.mismatch());
        };
        if *double != wide && !either_quotes {
            return Err(self.mismatch());
        }
        let capacity = usize::try_from(capacity).unwrap_or(usize::MAX);
        if string_capacity_cut(text, wide, capacity).is_some() {
            return Err(format!(
                "'{}' is longer than {} holds",
                self.text, self.name
            ));
        }
        Ok(if wide {
            Value::WString(text.clone())
        } else {
            Value::String(SmolStr::new(text))
        })
    }

    /// `literal`, a name, as a value of the enumeration `enumeration` (IEC 61131-3 §6.4.4.2).
    fn enumeration(&self, enumeration: &str, literal: &Literal) -> Result<Value, String> {
        let Literal::Name(name) = literal else {
            return Err(self.mismatch());
        };
        enum_value(&self.image, enumeration, name).ok_or_else(|| {
            format!(
                "'{name}' is not a value of {enumeration} ({})",
                self.enumeration_values(enumeration).join(", ")
            )
        })
    }

    /// The names of the values of the enumeration `enumeration`, in their declared order.
    fn enumeration_values(&self, enumeration: &str) -> Vec<SmolStr> {
        let Ok(types) = self.image.table::<TypeRecord>() else {
            return Vec::new();
        };
        (0..types.len())
            .find(|index| {
                types.get(*index).is_some_and(|record| {
                    record.kind == kind::ENUM
                        && self
                            .image
                            .string(record.name)
                            .is_some_and(|name| name.eq_ignore_ascii_case(enumeration))
                })
            })
            .map(|index| enum_values(&self.image, index))
            .unwrap_or_default()
    }

    /// The bytes of `value` as a variable of the target's type holds them, written with the type
    /// table's own conversions on a variable that stands apart from the program: a subrange's
    /// bounds and an enumeration's declared values are checked here, as the write at the boundary
    /// checks them.
    fn bytes(&self, value: &Value) -> Result<Vec<u8>, String> {
        let size = self
            .image
            .table::<TypeRecord>()
            .ok()
            .and_then(|types| types.get(self.type_index))
            .and_then(|record| usize::try_from(record.size).ok())
            .ok_or_else(|| String::from("the program image's type table does not read"))?;
        let mut scratch = Scratch {
            image: self.image,
            bytes: vec![0; size],
        };
        let place = Place::Area { area: 1, offset: 0 };
        write_place(&mut scratch, place, self.type_index, value).map_err(|error| match error {
            RuntimeError::SubrangeViolation { lower, upper, .. } => format!(
                "'{}' is outside the range of {} ({lower}..{upper})",
                self.text, self.name
            ),
            RuntimeError::TypeMismatch => self.mismatch(),
            RuntimeError::Overflow => self.out_of_range(),
            RuntimeError::NonFiniteValue => format!("'{}' is not a finite number", self.text),
            other => format!("'{}': {other}", self.text),
        })?;
        Ok(scratch.bytes)
    }

    fn mismatch(&self) -> String {
        format!("'{}' is not a value of type {}", self.text, self.name)
    }

    fn out_of_range(&self) -> String {
        format!("'{}' is outside the range of {}", self.text, self.name)
    }

    fn no_single_value(&self) -> String {
        format!(
            "{} holds no single value: write one of its members or elements",
            self.name
        )
    }
}

/// The elementary type a typed literal's prefix names (Table 5 feature 9, Table 6 feature 3).
fn keyword_kind(upper: &str) -> Option<u32> {
    Some(match upper {
        "BOOL" => kind::BOOL,
        "SINT" => kind::SINT,
        "INT" => kind::INT,
        "DINT" => kind::DINT,
        "LINT" => kind::LINT,
        "USINT" => kind::USINT,
        "UINT" => kind::UINT,
        "UDINT" => kind::UDINT,
        "ULINT" => kind::ULINT,
        "REAL" => kind::REAL,
        "LREAL" => kind::LREAL,
        "BYTE" => kind::BYTE,
        "WORD" => kind::WORD,
        "DWORD" => kind::DWORD,
        "LWORD" => kind::LWORD,
        "CHAR" => kind::CHAR,
        "WCHAR" => kind::WCHAR,
        _ => return None,
    })
}

/// The keyword of the elementary type `kind`.
fn kind_name(kind: u32) -> &'static str {
    match kind {
        kind::BOOL => "BOOL",
        kind::SINT => "SINT",
        kind::INT => "INT",
        kind::DINT => "DINT",
        kind::LINT => "LINT",
        kind::USINT => "USINT",
        kind::UINT => "UINT",
        kind::UDINT => "UDINT",
        kind::ULINT => "ULINT",
        kind::REAL => "REAL",
        kind::LREAL => "LREAL",
        kind::BYTE => "BYTE",
        kind::WORD => "WORD",
        kind::DWORD => "DWORD",
        kind::LWORD => "LWORD",
        kind::TIME => "TIME",
        kind::LTIME => "LTIME",
        kind::DATE => "DATE",
        kind::LDATE => "LDATE",
        kind::TOD => "TIME_OF_DAY",
        kind::LTOD => "LTIME_OF_DAY",
        kind::DT => "DATE_AND_TIME",
        kind::LDT => "LDATE_AND_TIME",
        kind::CHAR => "CHAR",
        kind::WCHAR => "WCHAR",
        _ => "its type",
    }
}

/// The bytes of one variable, apart from the program, on which a value is written to check it and
/// to read its bytes.
struct Scratch<'i> {
    image: Image<'i>,
    bytes: Vec<u8>,
}

impl ProgramMemory for Scratch<'_> {
    fn image(&self) -> Image<'_> {
        self.image
    }

    fn read_area(&self, _id: u8, offset: usize, out: &mut [u8]) -> bool {
        match offset
            .checked_add(out.len())
            .and_then(|end| self.bytes.get(offset..end))
        {
            Some(bytes) => {
                out.copy_from_slice(bytes);
                true
            }
            None => false,
        }
    }

    fn write_area(&mut self, _id: u8, offset: usize, data: &[u8]) -> bool {
        match offset
            .checked_add(data.len())
            .and_then(|end| self.bytes.get_mut(offset..end))
        {
            Some(bytes) => {
                bytes.copy_from_slice(data);
                true
            }
            None => false,
        }
    }

    fn write_image(&mut self, _address: &IoAddress, _value: Value) -> Result<(), RuntimeError> {
        Err(RuntimeError::TypeMismatch)
    }
}

#[cfg(test)]
mod tests;
