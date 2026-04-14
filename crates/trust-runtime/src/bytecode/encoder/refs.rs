use smol_str::SmolStr;

use crate::memory::{FrameId, InstanceId, IoArea, MemoryLocation};
use crate::value::{RefSegment as ValueRefSegment, Value, ValueRef};

use crate::bytecode::{RefEntry, RefLocation, RefSegment};

use super::util::to_u32;
use super::{BytecodeEncoder, BytecodeError, CodegenContext};

impl<'a> BytecodeEncoder<'a> {
    pub(super) fn resolve_lvalue_ref(
        &self,
        ctx: &CodegenContext,
        target: &crate::program_model::LValue,
    ) -> Result<Option<ValueRef>, BytecodeError> {
        use crate::program_model::LValue;
        let mut reference = match target {
            LValue::Name(name) => return self.resolve_name_ref(ctx, name),
            LValue::Field { name, field } => {
                let qualified = SmolStr::new(format!("{name}.{field}"));
                if let Some(reference) = self.resolve_name_ref(ctx, &qualified)? {
                    return Ok(Some(reference));
                }
                match self.resolve_name_ref(ctx, name)? {
                    Some(reference) => reference,
                    None => return Ok(None),
                }
            }
            LValue::Index { name, .. } => match self.resolve_name_ref(ctx, name)? {
                Some(reference) => reference,
                None => return Ok(None),
            },
            LValue::Deref(_) => return Ok(None),
        };
        match target {
            LValue::Field { field, .. } => {
                reference
                    .path
                    .push(crate::value::RefSegment::Field(field.clone()));
            }
            LValue::Index { indices, .. } => {
                let mut resolved = Vec::with_capacity(indices.len());
                for expr in indices {
                    let value = match expr {
                        crate::program_model::Expr::Literal(value) => value,
                        _ => return Ok(None),
                    };
                    let index = match value {
                        Value::SInt(v) => i64::from(*v),
                        Value::Int(v) => i64::from(*v),
                        Value::DInt(v) => i64::from(*v),
                        Value::LInt(v) => *v,
                        Value::USInt(v) => i64::from(*v),
                        Value::UInt(v) => i64::from(*v),
                        Value::UDInt(v) => i64::from(*v),
                        Value::ULInt(v) => match i64::try_from(*v) {
                            Ok(value) => value,
                            Err(_) => return Ok(None),
                        },
                        Value::Byte(v) => i64::from(*v),
                        Value::Word(v) => i64::from(*v),
                        Value::DWord(v) => i64::from(*v),
                        Value::LWord(v) => match i64::try_from(*v) {
                            Ok(value) => value,
                            Err(_) => return Ok(None),
                        },
                        _ => return Ok(None),
                    };
                    resolved.push(index);
                }
                reference
                    .path
                    .push(crate::value::RefSegment::Index(resolved));
            }
            _ => {}
        }
        Ok(Some(reference))
    }

    pub(super) fn resolve_name_ref(
        &self,
        ctx: &CodegenContext,
        name: &SmolStr,
    ) -> Result<Option<ValueRef>, BytecodeError> {
        if let Some(reference) = ctx.local_ref(name) {
            return Ok(Some(reference.clone()));
        }
        if let Some(reference) = ctx.static_ref(name) {
            return Ok(Some(reference.clone()));
        }
        if let Some(instance_id) = ctx.instance_id {
            if let Some(reference) = self
                .runtime
                .storage()
                .ref_for_instance_recursive(instance_id, name.as_ref())
            {
                return Ok(Some(reference));
            }
        }
        if let Some(binding) = self.runtime.access_map().get(name.as_ref()) {
            if binding.partial.is_none() {
                return Ok(Some(binding.reference.clone()));
            }
        }
        Ok(self.runtime.storage().ref_for_global(name.as_ref()))
    }

    pub(super) fn ref_index_for(&mut self, value_ref: &ValueRef) -> Result<u32, BytecodeError> {
        if let Some(idx) = self.ref_map.get(value_ref) {
            return Ok(*idx);
        }
        let (location, owner_id) = match value_ref.location {
            MemoryLocation::Global => (RefLocation::Global, 0),
            MemoryLocation::Local(FrameId(id)) => (RefLocation::Local, id),
            MemoryLocation::Instance(InstanceId(id)) => (RefLocation::Instance, id),
            MemoryLocation::Retain => (RefLocation::Retain, 0),
            MemoryLocation::Io(area) => {
                let owner = match area {
                    IoArea::Input => 0,
                    IoArea::Output => 1,
                    IoArea::Memory => 2,
                };
                (RefLocation::Io, owner)
            }
        };
        let offset = to_u32(value_ref.offset, "ref offset")?;
        let mut segments = Vec::new();
        for segment in &value_ref.path {
            match segment {
                ValueRefSegment::Index(indices) => {
                    segments.push(RefSegment::Index(indices.clone()));
                }
                ValueRefSegment::Field(name) => {
                    let name_idx = self.strings.intern(name.clone());
                    segments.push(RefSegment::Field { name_idx });
                }
            }
        }
        let entry = RefEntry {
            location,
            owner_id,
            offset,
            segments,
        };
        let idx = self.ref_entries.len() as u32;
        self.ref_entries.push(entry);
        self.ref_map.insert(value_ref.clone(), idx);
        Ok(idx)
    }
}
