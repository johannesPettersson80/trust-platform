//! Allocate artifact identities from declaration graphs, without runtime Values.

use super::*;
use crate::bytecode::{
    ConstructionRoot, StorageOwner, StorageRole, BYTECODE_MAX_CONSTRUCTION_NODES,
    BYTECODE_MAX_CONSTRUCTION_RECORDS,
};
use crate::value::RefSegment;
use trust_hir::{types::ArrayDimensionExt, Type};

impl BytecodeEncoder<'_> {
    pub(in crate::bytecode::encoder) fn construct_persistent_roots(
        &mut self,
    ) -> Result<(), BytecodeError> {
        let globals: Vec<usize> = self
            .construction
            .layout
            .entries
            .iter()
            .enumerate()
            .filter(|(_, declaration)| {
                declaration.owner == StorageOwner::Global
                    && declaration.role != StorageRole::External
            })
            .map(|(id, _)| id)
            .collect();
        let mut remaining = BYTECODE_MAX_CONSTRUCTION_NODES as usize;
        for declaration in globals {
            let record = self.construction.layout.entries[declaration].clone();
            let name = self.strings.entries[record.name_idx as usize].clone();
            let binding = self
                .construction
                .bindings
                .global(&name)
                .cloned()
                .ok_or_else(|| invalid("persistent binding missing"))?;
            let root = self.push_construction_root(declaration as u32, Some(&binding), None, 0)?;
            if record.role == StorageRole::ProgramRoot {
                let instance = self.construct_instance(
                    record
                        .owner_pou_id
                        .ok_or_else(|| invalid("program template missing"))?,
                    root,
                    &mut remaining,
                    0,
                )?;
                self.construction
                    .bindings
                    .values_to_instances
                    .insert(binding, instance);
            } else {
                let ty = self.construction.declaration_types[declaration]
                    .ok_or_else(|| invalid("root type missing"))?;
                self.construct_nested_value(ty, &binding, root, &mut remaining, 0)?;
            }
        }
        Ok(())
    }

    fn push_construction_root(
        &mut self,
        declaration: u32,
        binding: Option<&ValueRef>,
        parent: Option<u32>,
        flags: u32,
    ) -> Result<u32, BytecodeError> {
        if self.construction.roots.entries.len() >= BYTECODE_MAX_CONSTRUCTION_RECORDS {
            return Err(invalid("construction root limit exceeded"));
        }
        let binding_ref_idx = binding.map(|value| self.ref_index_for(value)).transpose()?;
        let id = u32::try_from(self.construction.roots.entries.len())
            .map_err(|_| invalid("root identity overflow"))?;
        self.construction.roots.entries.push(ConstructionRoot {
            declaration_idx: declaration,
            binding_ref_idx,
            instance_owner_id: None,
            parent_root_idx: parent,
            template_pou_id: None,
            flags,
        });
        Ok(id)
    }

    fn construct_instance(
        &mut self,
        template: u32,
        root: u32,
        remaining: &mut usize,
        depth: u8,
    ) -> Result<InstanceId, BytecodeError> {
        consume_node(remaining, depth)?;
        let parent = if let Some(parent_template) = self
            .construction
            .templates
            .template_parents
            .get(&template)
            .copied()
        {
            let declaration = self.construction.roots.entries[root as usize].declaration_idx;
            let parent_root = self.push_construction_root(declaration, None, Some(root), 1)?;
            Some(self.construct_instance(parent_template, parent_root, remaining, depth + 1)?)
        } else {
            None
        };
        let instance = self.construction.bindings.allocate_instance(parent)?;
        let record = &mut self.construction.roots.entries[root as usize];
        record.instance_owner_id = Some(instance.0);
        record.template_pou_id = Some(template);
        let fields = self
            .construction
            .templates
            .template_fields
            .get(&template)
            .cloned()
            .unwrap_or_default();
        for field in fields {
            let declaration = self.construction.layout.entries[field as usize].clone();
            let name = self.strings.entries[declaration.name_idx as usize].clone();
            let ty = self.construction.declaration_types[field as usize]
                .ok_or_else(|| invalid("member type missing"))?;
            let binding = self
                .construction
                .bindings
                .allocate_field(instance, &name, ty)?;
            if binding.offset != declaration.slot as usize {
                return Err(invalid("instance layout slot mismatch"));
            }
            self.ref_index_for(&binding)?;
            if declaration.role == StorageRole::NativeState {
                consume_node(remaining, depth + 1)?;
                continue;
            }
            self.construct_member_value(ty, &binding, field, root, remaining, depth + 1)?;
        }
        Ok(instance)
    }

    fn construct_member_value(
        &mut self,
        ty: TypeId,
        binding: &ValueRef,
        declaration: u32,
        parent: u32,
        remaining: &mut usize,
        depth: u8,
    ) -> Result<(), BytecodeError> {
        let resolved = self.resolved_construction_type(ty)?;
        if matches!(resolved, Type::FunctionBlock { .. } | Type::Class { .. }) {
            let child = self.push_construction_root(declaration, Some(binding), Some(parent), 0)?;
            self.construct_nested_value(ty, binding, child, remaining, depth)?;
        } else {
            self.construct_aggregate_children(ty, binding, declaration, parent, remaining, depth)?;
        }
        Ok(())
    }

    fn construct_nested_value(
        &mut self,
        ty: TypeId,
        binding: &ValueRef,
        root: u32,
        remaining: &mut usize,
        depth: u8,
    ) -> Result<(), BytecodeError> {
        let resolved = self.resolved_construction_type(ty)?;
        match resolved {
            Type::FunctionBlock { ref name } | Type::Class { ref name } => {
                let template = self
                    .pou_ids
                    .class_like_id(name)
                    .ok_or_else(|| invalid("instance template missing"))?;
                let instance = self.construct_instance(template, root, remaining, depth)?;
                self.construction
                    .bindings
                    .values_to_instances
                    .insert(binding.clone(), instance);
            }
            _ => {
                let declaration = self.construction.roots.entries[root as usize].declaration_idx;
                self.construct_aggregate_children(
                    ty,
                    binding,
                    declaration,
                    root,
                    remaining,
                    depth,
                )?;
            }
        }
        Ok(())
    }

    fn construct_aggregate_children(
        &mut self,
        ty: TypeId,
        binding: &ValueRef,
        declaration: u32,
        root: u32,
        remaining: &mut usize,
        depth: u8,
    ) -> Result<(), BytecodeError> {
        consume_node(remaining, depth)?;
        match self.resolved_construction_type(ty)? {
            Type::Array {
                element,
                dimensions,
            } => {
                if dimensions.iter().any(ArrayDimensionExt::is_wildcard) {
                    return Ok(());
                }
                let count = dimensions.iter().try_fold(1usize, |total, (low, high)| {
                    let width = high
                        .checked_sub(*low)
                        .and_then(|value| value.checked_add(1))
                        .and_then(|value| usize::try_from(value).ok())
                        .ok_or_else(|| invalid("array construction extent overflow"))?;
                    total
                        .checked_mul(width)
                        .ok_or_else(|| invalid("array construction extent overflow"))
                })?;
                if count > *remaining {
                    return Err(invalid("construction node limit exceeded"));
                }
                for linear in 0..count {
                    let mut cursor = linear;
                    let mut indices = vec![0; dimensions.len()];
                    for (at, (low, high)) in dimensions.iter().enumerate().rev() {
                        let width = high
                            .checked_sub(*low)
                            .and_then(|value| value.checked_add(1))
                            .and_then(|value| usize::try_from(value).ok())
                            .filter(|value| *value != 0)
                            .ok_or_else(|| invalid("array extent overflow"))?;
                        indices[at] = low
                            .checked_add(
                                i64::try_from(cursor % width)
                                    .map_err(|_| invalid("array index overflow"))?,
                            )
                            .ok_or_else(|| invalid("array index overflow"))?;
                        cursor /= width;
                    }
                    let mut child = binding.clone();
                    child
                        .path
                        .push(RefSegment::Index(crate::value::ref_indices_from_iter(
                            indices,
                        )));
                    self.construct_member_value(
                        element,
                        &child,
                        declaration,
                        root,
                        remaining,
                        depth + 1,
                    )?;
                }
            }
            Type::Struct { fields, .. } => {
                for field in fields {
                    let mut child = binding.clone();
                    child.path.push(RefSegment::Field(field.name));
                    self.construct_member_value(
                        field.type_id,
                        &child,
                        declaration,
                        root,
                        remaining,
                        depth + 1,
                    )?;
                }
            }
            Type::Union { variants, .. } => {
                for field in variants {
                    let mut child = binding.clone();
                    child.path.push(RefSegment::Field(field.name));
                    self.construct_member_value(
                        field.type_id,
                        &child,
                        declaration,
                        root,
                        remaining,
                        depth + 1,
                    )?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn resolved_construction_type(&self, mut ty: TypeId) -> Result<Type, BytecodeError> {
        for _ in 0..64 {
            match self
                .runtime
                .registry()
                .get(ty)
                .ok_or_else(|| invalid("construction type missing"))?
            {
                Type::Alias { target, .. } => ty = *target,
                value => return Ok(value.clone()),
            }
        }
        Err(invalid("construction type recursion exceeded"))
    }
}

fn consume_node(remaining: &mut usize, depth: u8) -> Result<(), BytecodeError> {
    if depth > 64 {
        return Err(invalid("construction recursion exceeded"));
    }
    *remaining = remaining
        .checked_sub(1)
        .ok_or_else(|| invalid("construction node limit exceeded"))?;
    Ok(())
}
