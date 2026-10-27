//! Checked conversion between typed numeric process-image bindings and model values.

use crate::error::RuntimeError;
use crate::io::IoAddress;
use crate::stdlib::conversions::ConversionType;
use crate::value::Value;
use crate::CompiledRuntime;

/// A bound process image a plant reads and encodes against.
pub(super) trait BoundImage {
    /// The elementary type of the variable bound at `address`, as the program's image declares
    /// it; `None` for an address no variable binds and for an enumeration.
    fn binding_type(&self, address: &IoAddress) -> Option<ConversionType>;
    fn read(&self, address: &IoAddress) -> Result<Value, RuntimeError>;
}

impl BoundImage for CompiledRuntime {
    fn binding_type(&self, address: &IoAddress) -> Option<ConversionType> {
        self.io_bindings()
            .iter()
            .find(|binding| binding.address == *address)
            .filter(|binding| binding.codec.enum_type.is_none())
            .and_then(|binding| binding.codec.wire_type)
    }

    fn read(&self, address: &IoAddress) -> Result<Value, RuntimeError> {
        self.read_io(address)
    }
}

pub(super) fn read_numeric(io: &impl BoundImage, address: &IoAddress) -> Result<f64, RuntimeError> {
    let raw = io.read(address)?;
    let value = match (io.binding_type(address), raw) {
        (Some(ConversionType::Bool), Value::Bool(value)) => f64::from(u8::from(value)),
        (Some(ConversionType::SInt), Value::Byte(value)) => f64::from(value as i8),
        (Some(ConversionType::Int), Value::Word(value)) => f64::from(value as i16),
        (Some(ConversionType::DInt), Value::DWord(value)) => f64::from(value as i32),
        (Some(ConversionType::LInt), Value::LWord(value)) => exact_i64(value as i64)?,
        (Some(ConversionType::USInt), Value::Byte(value))
        | (Some(ConversionType::Byte), Value::Byte(value)) => f64::from(value),
        (Some(ConversionType::UInt), Value::Word(value))
        | (Some(ConversionType::Word), Value::Word(value)) => f64::from(value),
        (Some(ConversionType::UDInt), Value::DWord(value))
        | (Some(ConversionType::DWord), Value::DWord(value)) => f64::from(value),
        (Some(ConversionType::ULInt), Value::LWord(value))
        | (Some(ConversionType::LWord), Value::LWord(value)) => exact_u64(value)?,
        (Some(ConversionType::Real), Value::DWord(bits)) => f64::from(f32::from_bits(bits)),
        (Some(ConversionType::LReal), Value::LWord(bits)) => f64::from_bits(bits),
        _ => return Err(RuntimeError::TypeMismatch),
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err(RuntimeError::Overflow)
    }
}

pub(super) fn encode_numeric(
    io: &impl BoundImage,
    address: &IoAddress,
    value: f64,
) -> Result<Value, RuntimeError> {
    if !value.is_finite() {
        return Err(RuntimeError::Overflow);
    }
    match io.binding_type(address) {
        Some(ConversionType::Bool) if value == 0.0 || value == 1.0 => Ok(Value::Bool(value == 1.0)),
        Some(ConversionType::SInt) => signed(value, i8::MIN as f64, i8::MAX as f64)
            .map(|value| Value::Byte((value as i8) as u8)),
        Some(ConversionType::Int) => signed(value, i16::MIN as f64, i16::MAX as f64)
            .map(|value| Value::Word((value as i16) as u16)),
        Some(ConversionType::DInt) => signed(value, i32::MIN as f64, i32::MAX as f64)
            .map(|value| Value::DWord((value as i32) as u32)),
        Some(ConversionType::LInt) => signed(value, -((1_u64 << 63) as f64), (1_u64 << 63) as f64)
            .and_then(|value| {
                if value < (1_u64 << 63) as f64 {
                    Ok(Value::LWord((value as i64) as u64))
                } else {
                    Err(RuntimeError::Overflow)
                }
            }),
        Some(ConversionType::USInt | ConversionType::Byte) => {
            unsigned(value, 256.0).map(|value| Value::Byte(value as u8))
        }
        Some(ConversionType::UInt | ConversionType::Word) => {
            unsigned(value, 65_536.0).map(|value| Value::Word(value as u16))
        }
        Some(ConversionType::UDInt | ConversionType::DWord) => {
            unsigned(value, 4_294_967_296.0).map(|value| Value::DWord(value as u32))
        }
        Some(ConversionType::ULInt | ConversionType::LWord) => {
            unsigned(value, 18_446_744_073_709_551_616.0).map(|value| Value::LWord(value as u64))
        }
        Some(ConversionType::Real) => {
            let narrowed = value as f32;
            if narrowed.is_finite() {
                Ok(Value::DWord(narrowed.to_bits()))
            } else {
                Err(RuntimeError::Overflow)
            }
        }
        Some(ConversionType::LReal) => Ok(Value::LWord(value.to_bits())),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

/// A program variable's numeric value (Specification 46 §13.1).
pub(super) fn decode_storage(value: &Value) -> Result<f64, RuntimeError> {
    let number = match value {
        Value::Real(value) => f64::from(*value),
        Value::LReal(value) => *value,
        Value::SInt(value) => f64::from(*value),
        Value::Int(value) => f64::from(*value),
        Value::DInt(value) => f64::from(*value),
        Value::LInt(value) => exact_i64(*value)?,
        Value::USInt(value) => f64::from(*value),
        Value::UInt(value) => f64::from(*value),
        Value::UDInt(value) => f64::from(*value),
        Value::ULInt(value) => exact_u64(*value)?,
        _ => return Err(RuntimeError::TypeMismatch),
    };
    if number.is_finite() {
        Ok(number)
    } else {
        Err(RuntimeError::Overflow)
    }
}

/// `value` in the type of the program variable currently holding `current`, by the checked conversion rules
/// (Specification 46 §13.2).
pub(super) fn encode_storage(current: &Value, value: f64) -> Result<Value, RuntimeError> {
    if !value.is_finite() {
        return Err(RuntimeError::Overflow);
    }
    Ok(match current {
        Value::Real(_) => {
            let narrowed = value as f32;
            if !narrowed.is_finite() {
                return Err(RuntimeError::Overflow);
            }
            Value::Real(narrowed)
        }
        Value::LReal(_) => Value::LReal(value),
        Value::SInt(_) => Value::SInt(signed(value, i8::MIN as f64, i8::MAX as f64)? as i8),
        Value::Int(_) => Value::Int(signed(value, i16::MIN as f64, i16::MAX as f64)? as i16),
        Value::DInt(_) => Value::DInt(signed(value, i32::MIN as f64, i32::MAX as f64)? as i32),
        Value::LInt(_) => {
            let rounded = signed(value, -((1_u64 << 63) as f64), (1_u64 << 63) as f64)?;
            if rounded >= (1_u64 << 63) as f64 {
                return Err(RuntimeError::Overflow);
            }
            Value::LInt(rounded as i64)
        }
        Value::USInt(_) => Value::USInt(unsigned(value, 256.0)? as u8),
        Value::UInt(_) => Value::UInt(unsigned(value, 65_536.0)? as u16),
        Value::UDInt(_) => Value::UDInt(unsigned(value, 4_294_967_296.0)? as u32),
        Value::ULInt(_) => Value::ULInt(unsigned(value, 18_446_744_073_709_551_616.0)? as u64),
        _ => return Err(RuntimeError::TypeMismatch),
    })
}

fn signed(value: f64, minimum: f64, maximum: f64) -> Result<f64, RuntimeError> {
    let rounded = value.round_ties_even();
    if rounded < minimum || rounded > maximum {
        return Err(RuntimeError::Overflow);
    }
    Ok(rounded)
}

fn unsigned(value: f64, exclusive_maximum: f64) -> Result<f64, RuntimeError> {
    let rounded = value.round_ties_even();
    if rounded < 0.0 || rounded >= exclusive_maximum {
        return Err(RuntimeError::Overflow);
    }
    Ok(rounded)
}

fn exact_i64(value: i64) -> Result<f64, RuntimeError> {
    let converted = value as f64;
    if converted as i128 == i128::from(value) {
        Ok(converted)
    } else {
        Err(RuntimeError::Overflow)
    }
}

fn exact_u64(value: u64) -> Result<f64, RuntimeError> {
    let converted = value as f64;
    if converted as u128 == u128::from(value) {
        Ok(converted)
    } else {
        Err(RuntimeError::Overflow)
    }
}

#[cfg(test)]
mod tests {
    use trust_hir::TypeId;

    use super::*;

    #[test]
    fn bool_numeric_io_roundtrips_only_zero_and_one() {
        let address = IoAddress::parse("%IX0.0").unwrap();
        let output = IoAddress::parse("%QX0.0").unwrap();
        let runtime = crate::harness::runtime_with_bindings(&[
            (address.clone(), TypeId::BOOL),
            (output.clone(), TypeId::BOOL),
        ])
        .expect("compile the BOOL bindings");
        assert_eq!(read_numeric(&runtime, &address).unwrap(), 0.0);
        assert_eq!(
            encode_numeric(&runtime, &output, 1.0).unwrap(),
            Value::Bool(true)
        );
        assert!(matches!(
            encode_numeric(&runtime, &output, 0.5),
            Err(RuntimeError::TypeMismatch)
        ));
    }
}
