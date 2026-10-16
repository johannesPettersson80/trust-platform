//! Untyped integer literals take the declared type of their destination (spec 11, "An admitted
//! integer is an untyped literal"). The I/O admission and the queued variable writes and forces
//! share this conversion, so both apply the same range and width rules at the scan boundary.
use super::*;
use crate::io_image::IoValueCodec;
use crate::stdlib::conversions::ConversionType;
use crate::value::ValueRefView;

impl EngineState<'_> {
    /// Give an untyped integer literal the integer or bit-string type `declared`. The type is
    /// resolved by the codec that binds I/O addresses (`IoValueCodec::from_type`), so aliases and
    /// subranges share one table. A value that is not a literal passes through unchanged. So does
    /// a literal whose destination is no plain integer or bit-string type (enums, aggregates and
    /// unresolved types): the destination's own checks then accept or reject it as before.
    pub(super) fn admit_literal(
        &self,
        declared: Option<u32>,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        let Value::LInt(literal) = value else {
            return Ok(value);
        };
        let codec =
            IoValueCodec::from_type(declared, &self.prepared.vm.types, &self.prepared.vm.strings)
                .ok();
        match codec {
            Some(IoValueCodec {
                wire_type: Some(wire),
                enum_type: None,
                ..
            }) => declared_value(wire, literal),
            _ => Ok(Value::LInt(literal)),
        }
    }
}

impl CycleAccess<'_, '_> {
    /// Apply one queued or forced write to a global at a scan boundary. An untyped integer
    /// literal takes the declared type of the global first; a value the global cannot take is
    /// rejected alone, as the write_rejected path reports it.
    pub fn write_admitted_global(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        let declared = self
            .engine
            .prepared
            .global(name)
            .and_then(|(_, declaration)| declaration.type_id);
        let value = self.engine.admit_literal(declared, value)?;
        self.write_named_path(name, &[], value)
    }

    /// Apply one queued or forced write to a resolved retained or instance reference at a scan
    /// boundary, with the same literal admission as `write_admitted_global`.
    pub fn write_admitted_reference(
        &mut self,
        reference: ValueRefView<'_>,
        value: Value,
    ) -> Result<(), RuntimeError> {
        let declared = match value {
            Value::LInt(_) => self.engine.reference_type(reference)?,
            _ => None,
        };
        let value = self.engine.admit_literal(declared, value)?;
        self.write_reference(reference, value)
    }
}

/// The value of a literal in a declared integer or bit-string type. A literal outside that
/// type's range is an overflow; a type that takes no integer literal (BOOL, REAL, TIME, STRING,
/// CHAR and the rest) is a type mismatch.
pub(super) fn declared_value(
    wire_type: ConversionType,
    literal: i64,
) -> Result<Value, RuntimeError> {
    Ok(match wire_type {
        ConversionType::SInt => Value::SInt(narrow(literal)?),
        ConversionType::Int => Value::Int(narrow(literal)?),
        ConversionType::DInt => Value::DInt(narrow(literal)?),
        ConversionType::LInt => Value::LInt(literal),
        ConversionType::USInt => Value::USInt(narrow(literal)?),
        ConversionType::UInt => Value::UInt(narrow(literal)?),
        ConversionType::UDInt => Value::UDInt(narrow(literal)?),
        ConversionType::ULInt => Value::ULInt(narrow(literal)?),
        ConversionType::Byte => Value::Byte(narrow(literal)?),
        ConversionType::Word => Value::Word(narrow(literal)?),
        ConversionType::DWord => Value::DWord(narrow(literal)?),
        ConversionType::LWord => Value::LWord(narrow(literal)?),
        _ => return Err(RuntimeError::TypeMismatch),
    })
}

/// Narrow a literal to its destination's range; a literal outside it overflows.
pub(super) fn narrow<T: TryFrom<i64>>(literal: i64) -> Result<T, RuntimeError> {
    T::try_from(literal).map_err(|_| RuntimeError::Overflow)
}
