//! Resolve configured paths from declarations and reserved instance identities.

use super::*;
use crate::harness::AccessPart;
use crate::value::{PartialAccess, RefSegment};
use trust_hir::{types::TypeRegistry, Type};

#[derive(Clone)]
pub(in crate::bytecode::encoder) struct ResolvedBinding {
    pub(in crate::bytecode::encoder) reference: ValueRef,
    pub(in crate::bytecode::encoder) type_id: Option<TypeId>,
    pub(in crate::bytecode::encoder) partial: Option<PartialAccess>,
}

impl StructuralBindings {
    pub(super) fn resolve_parts(
        &self,
        parts: &[AccessPart],
        program_instances: &[ValueRef],
        registry: &TypeRegistry,
    ) -> Result<ResolvedBinding, BytecodeError> {
        let mut parts = parts;
        // Qualifiers are recognized in order; unknown names are never discarded.
        for qualifier in [&self.configuration_name, &self.resource_name] {
            if let (Some(qualifier), Some(AccessPart::Name(name))) = (qualifier, parts.first()) {
                if qualifier.eq_ignore_ascii_case(name) {
                    parts = &parts[1..];
                }
            }
        }
        {
            let Some(AccessPart::Name(name)) = parts.first() else {
                return Err(invalid("configured path must begin with a storage name"));
            };
            let reference = if let Some(reference) = self.global(name) {
                Some(reference.clone())
            } else {
                let mut found = None;
                for root in program_instances {
                    let Some(instance) = self.values_to_instances.get(root) else {
                        continue;
                    };
                    if let Some(reference) = self.instance(*instance, name) {
                        if found.is_some() {
                            found = None;
                            break;
                        }
                        found = Some(reference.clone());
                    }
                }
                found
            };
            let reference = reference.ok_or_else(|| invalid("unknown configured path prefix"))?;
            let mut resolved = ResolvedBinding {
                type_id: self.types.get(&reference).copied(),
                reference,
                partial: None,
            };
            for part in &parts[1..] {
                if resolved.partial.is_some() {
                    return Err(invalid("partial access must end a configured path"));
                }
                match part {
                    AccessPart::Name(name) => self.select_field(&mut resolved, name, registry)?,
                    AccessPart::Index(indices) => {
                        let Type::Array {
                            element,
                            dimensions,
                        } = resolve_type(registry, resolved.type_id)?
                        else {
                            return Err(invalid("index target is not an array"));
                        };
                        if indices.len() != dimensions.len()
                            || indices
                                .iter()
                                .zip(dimensions)
                                .any(|(index, (low, high))| index < low || index > high)
                        {
                            return Err(invalid("configured array index out of bounds"));
                        }
                        resolved.reference.path.push(RefSegment::Index(
                            crate::value::ref_indices_from_iter(indices.iter().copied()),
                        ));
                        resolved.type_id = Some(*element);
                    }
                    AccessPart::Partial(partial) => {
                        let bits = match resolve_type(registry, resolved.type_id)? {
                            Type::Byte => 8,
                            Type::Word => 16,
                            Type::DWord => 32,
                            Type::LWord => 64,
                            _ => return Err(invalid("partial access requires a bit string")),
                        };
                        let (selection, index) =
                            crate::bytecode::PartialAccessSpec::from_access(*partial);
                        if index
                            .checked_add(1)
                            .and_then(|value| value.checked_mul(selection.width))
                            .is_none_or(|end| end > bits)
                        {
                            return Err(invalid("configured partial access out of bounds"));
                        }
                        resolved.partial = Some(*partial);
                    }
                }
            }
            Ok(resolved)
        }
    }

    fn select_field(
        &self,
        selected: &mut ResolvedBinding,
        name: &SmolStr,
        registry: &TypeRegistry,
    ) -> Result<(), BytecodeError> {
        if let Some(instance) = self.values_to_instances.get(&selected.reference) {
            let reference = self
                .instance(*instance, name)
                .ok_or_else(|| invalid("unknown instance field"))?;
            selected.reference = reference.clone();
            selected.type_id = self.types.get(reference).copied();
            return Ok(());
        }
        let (name, ty) = match resolve_type(registry, selected.type_id)? {
            Type::Struct { fields, .. } => fields
                .iter()
                .find(|field| field.name.eq_ignore_ascii_case(name))
                .map(|field| (field.name.clone(), field.type_id)),
            Type::Union { variants, .. } => variants
                .iter()
                .find(|field| field.name.eq_ignore_ascii_case(name))
                .map(|field| (field.name.clone(), field.type_id)),
            _ => None,
        }
        .ok_or_else(|| invalid("unknown structured field"))?;
        selected.reference.path.push(RefSegment::Field(name));
        selected.type_id = Some(ty);
        Ok(())
    }
}

fn resolve_type(registry: &TypeRegistry, ty: Option<TypeId>) -> Result<&Type, BytecodeError> {
    let mut ty = ty.ok_or_else(|| invalid("configured target has no value type"))?;
    for _ in 0..64 {
        match registry
            .get(ty)
            .ok_or_else(|| invalid("unknown configured target type"))?
        {
            Type::Alias { target, .. } => ty = *target,
            value => return Ok(value),
        }
    }
    Err(invalid("configured target type recursion exceeded"))
}

impl BytecodeEncoder<'_> {
    pub(super) fn source_program_roots(&self) -> Vec<ValueRef> {
        self.construction
            .layout
            .entries
            .iter()
            .filter(|entry| entry.role == crate::bytecode::StorageRole::ProgramRoot)
            .filter_map(|entry| {
                self.construction
                    .bindings
                    .global(&self.strings.entries[entry.name_idx as usize])
                    .cloned()
            })
            .collect()
    }

    pub(super) fn declaration_for_binding(
        &self,
        reference: &ValueRef,
    ) -> Result<u32, BytecodeError> {
        use crate::memory::MemoryLocation;
        let found = match reference.location {
            MemoryLocation::Instance(instance) => {
                let template = self
                    .construction
                    .roots
                    .entries
                    .iter()
                    .find(|root| root.instance_owner_id == Some(instance.0))
                    .and_then(|root| root.template_pou_id)
                    .ok_or_else(|| invalid("instance template binding missing"))?;
                self.construction
                    .templates
                    .template_fields
                    .get(&template)
                    .into_iter()
                    .flatten()
                    .copied()
                    .find(|id| {
                        self.construction.layout.entries[*id as usize].slot as usize
                            == reference.offset
                    })
            }
            MemoryLocation::Global | MemoryLocation::Retain => self
                .construction
                .layout
                .entries
                .iter()
                .enumerate()
                .find(|(_, declaration)| {
                    declaration.owner == crate::bytecode::StorageOwner::Global
                        && declaration.slot as usize == reference.offset
                })
                .map(|(id, _)| id as u32),
            _ => None,
        };
        found.ok_or_else(|| invalid("configured target declaration missing"))
    }
}
