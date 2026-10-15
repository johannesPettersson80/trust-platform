//! Admission of untyped integer literals at the scan boundary.
//!
//! The attach-mode text grammar produces `LInt` for every integer. The
//! destination supplies the literal's type here (spec 11, "An admitted integer
//! is an untyped literal"), so the grammar itself stays unchanged.
use super::super::literal_admission::{declared_value, narrow};
use super::*;
use crate::io_address::IoSize;
use crate::vm::type_policy::normalize_vm_value_for_type;

impl EngineState<'_> {
    /// Give an untyped integer literal the type of its destination.
    ///
    /// A bound address uses its declared integer or bit-string type, checked
    /// against that declaration exactly as the scan's binding sample checks it.
    /// An unbound address takes the BYTE, WORD, DWORD or LWORD width of the
    /// address. Any other value is not a literal and passes through unchanged.
    pub(in crate::vm::engine) fn admit_cycle_image_value(
        &self,
        address: &IoAddress,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        let Value::LInt(literal) = value else {
            return Ok(value);
        };
        let declared = self
            .prepared
            .io_bindings
            .iter()
            .find(|binding| binding.address == *address)
            .and_then(|binding| binding.codec.wire_type.map(|wire| (binding, wire)));
        let Some((binding, wire_type)) = declared else {
            return width_value(address.size, literal);
        };
        if binding.codec.enum_type.is_some() {
            return Err(RuntimeError::TypeMismatch);
        }
        let typed = declared_value(wire_type, literal)?;
        let typed = match binding.type_id {
            Some(ty) => normalize_vm_value_for_type(&self.prepared.vm, ty, typed)?,
            None => typed,
        };
        binding.codec.encode(typed, address.size)
    }
}

/// The wire value of a literal written to an address with no declared type.
/// Bit and string windows take no integer literal.
fn width_value(size: IoSize, literal: i64) -> Result<Value, RuntimeError> {
    Ok(match size {
        IoSize::Byte => Value::Byte(narrow(literal)?),
        IoSize::Word => Value::Word(narrow(literal)?),
        IoSize::DWord => Value::DWord(narrow(literal)?),
        IoSize::LWord => Value::LWord(narrow(literal)?),
        IoSize::Bit | IoSize::Bytes(_) => return Err(RuntimeError::TypeMismatch),
    })
}
