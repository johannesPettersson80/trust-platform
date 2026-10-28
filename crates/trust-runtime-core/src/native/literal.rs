//! An integer that Live Values writes into a compiled program is an untyped literal (specification
//! 11, "An admitted integer is an untyped literal"; specification 44 §13.3), as the interpreter's
//! writes take it: it takes the declared integer or bit-string type of the variable it is written
//! to, through aliases and subranges, and a literal outside that type's range overflows. At a
//! process-image address it takes the type of the variable bound there, or, where none is, the bit
//! string of the address's width, and the image holds its bits. Any other value passes through,
//! and the destination accepts or refuses it.

use super::image::{type_kind as kind, Image, IoBindRecord, SymbolRecord};
use super::values::{damaged, Types};
use crate::error::RuntimeError;
use crate::io_address::{IoAddress, IoSize};
use crate::memory::IoArea;
use crate::native::values::MAX_TYPE_NESTING;
use crate::value::Value;

/// `value` typed for a variable of type `type_index` of `image`: `Some` for an integer literal that
/// the variable's integer or bit-string type takes, `None` for a value that passes through.
///
/// # Errors
///
/// [`RuntimeError::Overflow`] for a literal outside the variable's type, and the error of a type
/// table that does not read.
pub(crate) fn for_variable(
    image: &Image<'_>,
    type_index: u32,
    value: &Value,
) -> Result<Option<Value>, RuntimeError> {
    let Value::LInt(literal) = *value else {
        return Ok(None);
    };
    match integer_kind(&Types::of(image)?, type_index)? {
        Some(kind) => typed(kind, literal).map(Some),
        None => Ok(None),
    }
}

/// `value` typed for the process-image `address` of `image`: an integer literal takes the type of
/// the variable bound at the address, or the bit string of its width, and becomes that bit string's
/// value with the literal's bits. Any other value, and a literal at a bit or text address, passes
/// through.
///
/// # Errors
///
/// [`RuntimeError::Overflow`] for a literal outside the type, and the error of a table that does
/// not read.
pub(crate) fn for_address(
    image: &Image<'_>,
    address: &IoAddress,
    value: Value,
) -> Result<Value, RuntimeError> {
    let Value::LInt(literal) = value else {
        return Ok(value);
    };
    let (string, bytes) = match address.size {
        IoSize::Byte => (kind::BYTE, 1),
        IoSize::Word => (kind::WORD, 2),
        IoSize::DWord => (kind::DWORD, 4),
        IoSize::LWord => (kind::LWORD, 8),
        IoSize::Bit | IoSize::Bytes(_) => return Ok(value),
    };
    let declared = match bound_type(image, address)? {
        Some(type_index) => integer_kind(&Types::of(image)?, type_index)?
            .filter(|declared| super::values::width(*declared) == Some(bytes)),
        None => None,
    };
    let typed = typed(declared.unwrap_or(string), literal)?;
    let bits = match typed {
        Value::SInt(v) => i128::from(v),
        Value::Int(v) => i128::from(v),
        Value::DInt(v) => i128::from(v),
        Value::LInt(v) => i128::from(v),
        Value::USInt(v) | Value::Byte(v) => i128::from(v),
        Value::UInt(v) | Value::Word(v) => i128::from(v),
        Value::UDInt(v) | Value::DWord(v) => i128::from(v),
        Value::ULInt(v) | Value::LWord(v) => i128::from(v),
        _ => return Err(RuntimeError::TypeMismatch),
    };
    // The image holds the literal's bits in the address's width, two's complement for a signed one.
    let mask = (1i128 << (bytes * 8)) - 1;
    let raw = bits & mask;
    Ok(match bytes {
        1 => Value::Byte(u8::try_from(raw).map_err(|_| RuntimeError::Overflow)?),
        2 => Value::Word(u16::try_from(raw).map_err(|_| RuntimeError::Overflow)?),
        4 => Value::DWord(u32::try_from(raw).map_err(|_| RuntimeError::Overflow)?),
        _ => Value::LWord(u64::try_from(raw).map_err(|_| RuntimeError::Overflow)?),
    })
}

/// The integer or bit-string kind of type `index`, through aliases, a subrange by its base; `None`
/// for any other type.
fn integer_kind(types: &Types<'_>, index: u32) -> Result<Option<u32>, RuntimeError> {
    let mut index = index;
    for _ in 0..=MAX_TYPE_NESTING {
        let record = types.get(index)?;
        let declared = match record.kind {
            kind::ALIAS => {
                index = types
                    .detail(&record, 4)?
                    .first_chunk::<4>()
                    .map(|word| u32::from_le_bytes(*word))
                    .ok_or_else(|| damaged("type table"))?;
                continue;
            }
            kind::SUBRANGE => types.subrange(&record)?.0,
            other => other,
        };
        return Ok(range(declared).map(|_| declared));
    }
    Err(damaged("type table"))
}

/// The values of an integer or bit-string kind; `None` for any other kind.
fn range(declared: u32) -> Option<(i128, i128)> {
    Some(match declared {
        kind::SINT => (i128::from(i8::MIN), i128::from(i8::MAX)),
        kind::INT => (i128::from(i16::MIN), i128::from(i16::MAX)),
        kind::DINT => (i128::from(i32::MIN), i128::from(i32::MAX)),
        kind::LINT => (i128::from(i64::MIN), i128::from(i64::MAX)),
        kind::USINT | kind::BYTE => (0, i128::from(u8::MAX)),
        kind::UINT | kind::WORD => (0, i128::from(u16::MAX)),
        kind::UDINT | kind::DWORD => (0, i128::from(u32::MAX)),
        kind::ULINT | kind::LWORD => (0, i128::from(u64::MAX)),
        _ => return None,
    })
}

/// The literal as a value of the integer or bit-string kind `declared`.
fn typed(declared: u32, literal: i64) -> Result<Value, RuntimeError> {
    let (low, high) = range(declared).ok_or(RuntimeError::TypeMismatch)?;
    let number = i128::from(literal);
    if number < low || number > high {
        return Err(RuntimeError::Overflow);
    }
    let overflow = |_| RuntimeError::Overflow;
    Ok(match declared {
        kind::SINT => Value::SInt(i8::try_from(literal).map_err(overflow)?),
        kind::INT => Value::Int(i16::try_from(literal).map_err(overflow)?),
        kind::DINT => Value::DInt(i32::try_from(literal).map_err(overflow)?),
        kind::LINT => Value::LInt(literal),
        kind::USINT => Value::USInt(u8::try_from(literal).map_err(overflow)?),
        kind::UINT => Value::UInt(u16::try_from(literal).map_err(overflow)?),
        kind::UDINT => Value::UDInt(u32::try_from(literal).map_err(overflow)?),
        kind::ULINT => Value::ULInt(u64::try_from(literal).map_err(overflow)?),
        kind::BYTE => Value::Byte(u8::try_from(literal).map_err(overflow)?),
        kind::WORD => Value::Word(u16::try_from(literal).map_err(overflow)?),
        kind::DWORD => Value::DWord(u32::try_from(literal).map_err(overflow)?),
        _ => Value::LWord(u64::try_from(literal).map_err(overflow)?),
    })
}

/// The type of the variable bound at `address`, by the image's bindings (specification 44 §5.6).
fn bound_type(image: &Image<'_>, address: &IoAddress) -> Result<Option<u32>, RuntimeError> {
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
        IoSize::Bytes(_) => return Ok(None),
    };
    let bindings = image
        .table::<IoBindRecord>()
        .map_err(|_| damaged("I/O binding table"))?;
    let hierarchical = address.path.len() > 1;
    let Some(binding) = bindings.iter().find(|binding| {
        binding.area == area
            && binding.size == size
            && if hierarchical {
                // A hierarchical address is the one its binding's text spells (§5.6).
                image
                    .binding_address(binding)
                    .is_some_and(|bound| bound.path == address.path && bound.bit == address.bit)
            } else {
                !binding.is_hierarchical()
                    && binding.byte == address.byte
                    && (size != 0 || binding.bit == address.bit)
            }
    }) else {
        return Ok(None);
    };
    let symbols = image
        .table::<SymbolRecord>()
        .map_err(|_| damaged("symbol table"))?;
    Ok(symbols.get(binding.symbol).map(|symbol| symbol.type_index))
}

#[cfg(test)]
mod tests;
