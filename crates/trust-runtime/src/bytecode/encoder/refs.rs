use smol_str::SmolStr;

use crate::memory::{FrameId, InstanceId, IoArea, MemoryLocation};
use crate::value::{ref_indices_from_iter, RefSegment as ValueRefSegment, Value, ValueRef};

use crate::bytecode::{RefEntry, RefLocation, RefSegment};

use super::util::normalize_name;
use super::util::to_u32;
use super::{BytecodeEncoder, BytecodeError, CodegenContext};

impl<'a> BytecodeEncoder<'a> {
    pub(super) fn resolve_lvalue_ref(
        &self,
        ctx: &CodegenContext,
        target: &crate::program_model::LValue,
    ) -> Result<Option<ValueRef>, BytecodeError> {
        use crate::program_model::LValue;
        if target.contains_index() {
            return Ok(None);
        }
        if let Some(qualified) = target.qualified_name() {
            if let Some(reference) = self.resolve_name_ref(ctx, &qualified)? {
                return Ok(Some(reference));
            }
        }
        match target {
            LValue::Name(name) => self.resolve_name_ref(ctx, name),
            LValue::Index { target, indices } => {
                let Some(mut reference) = self.resolve_lvalue_ref(ctx, target)? else {
                    return Ok(None);
                };
                let Some(resolved) = literal_indices(indices)? else {
                    return Ok(None);
                };
                reference
                    .path
                    .push(crate::value::RefSegment::Index(ref_indices_from_iter(
                        resolved,
                    )));
                Ok(Some(reference))
            }
            LValue::Field { target, field } => {
                let Some(mut reference) = self.resolve_lvalue_ref(ctx, target)? else {
                    return Ok(None);
                };
                reference
                    .path
                    .push(crate::value::RefSegment::Field(field.clone()));
                Ok(Some(reference))
            }
            LValue::Deref(_) => Ok(None),
        }
    }

    pub(super) fn resolve_name_ref(
        &self,
        ctx: &CodegenContext,
        name: &SmolStr,
    ) -> Result<Option<ValueRef>, BytecodeError> {
        let key = normalize_name(name);
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
                .or_else(|| {
                    (key != *name).then(|| {
                        self.runtime
                            .storage()
                            .ref_for_instance_recursive(instance_id, key.as_ref())
                    })?
                })
            {
                return Ok(Some(reference));
            }
        }
        if let Some(binding) = self
            .runtime
            .access_map()
            .get(name.as_ref())
            .or_else(|| (key != *name).then(|| self.runtime.access_map().get(key.as_ref()))?)
        {
            if binding.partial.is_none() {
                return Ok(Some(binding.reference.clone()));
            }
        }
        Ok(self
            .runtime
            .storage()
            .ref_for_global(name.as_ref())
            .or_else(|| {
                (key != *name).then(|| self.runtime.storage().ref_for_global(key.as_ref()))?
            }))
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
                    segments.push(RefSegment::Index(indices.to_vec()));
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

fn literal_indices(
    indices: &[crate::program_model::Expr],
) -> Result<Option<Vec<i64>>, BytecodeError> {
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
            Value::ULInt(v) => i64::try_from(*v)
                .map_err(|_| BytecodeError::InvalidSection("index literal overflow".into()))?,
            Value::Byte(v) => i64::from(*v),
            Value::Word(v) => i64::from(*v),
            Value::DWord(v) => i64::from(*v),
            Value::LWord(v) => i64::try_from(*v)
                .map_err(|_| BytecodeError::InvalidSection("index literal overflow".into()))?,
            _ => return Ok(None),
        };
        resolved.push(index);
    }
    Ok(Some(resolved))
}
