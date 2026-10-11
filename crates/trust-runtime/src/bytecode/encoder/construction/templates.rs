//! Complete instance declaration templates, including private and native state.

use super::*;
use crate::bytecode::{StorageDeclaration, StorageOwner, StorageRole};
use crate::program_model::MethodDef;

impl BytecodeEncoder<'_> {
    pub(in crate::bytecode::encoder) fn collect_instance_templates(
        &mut self,
    ) -> Result<(), BytecodeError> {
        for program in self.programs().values() {
            let owner = self
                .pou_ids
                .program_id(&program.name)
                .ok_or_else(missing_owner)?;
            for variable in &program.vars {
                self.add_instance_variable(owner, owner, variable, &variable.name)?;
            }
            self.add_edge_states(owner, &program.name)?;
        }
        for fb in self.runtime.function_blocks().values() {
            if !self.emit_block_template(&fb.name) {
                continue;
            }
            let owner = self
                .pou_ids
                .function_block_id(&fb.name)
                .ok_or_else(missing_owner)?;
            if let Some(base) = &fb.base {
                let name = match base {
                    crate::program_model::FunctionBlockBase::FunctionBlock(name)
                    | crate::program_model::FunctionBlockBase::Class(name) => name,
                };
                let parent = self.pou_ids.class_like_id(name).ok_or_else(missing_owner)?;
                self.construction
                    .templates
                    .template_parents
                    .insert(owner, parent);
            }
            for parameter in &fb.params {
                self.add_instance_parameter(owner, parameter)?;
            }
            for variable in &fb.vars {
                self.add_instance_variable(owner, owner, variable, &variable.name)?;
            }
            self.collect_method_statics(owner, &fb.name, &fb.methods)?;
            self.add_edge_states(owner, &fb.name)?;
            if self.is_stdlib_fb(&fb.name) {
                let kind = crate::stdlib::fbs::builtin_kind(&fb.name).ok_or_else(missing_owner)?;
                for (name, ty) in crate::stdlib::fbs::builtin_state_layout(kind) {
                    self.add_native_state(owner, name, ty)?;
                }
            }
        }
        for class in self.runtime.classes().values() {
            let owner = self
                .pou_ids
                .class_id(&class.name)
                .ok_or_else(missing_owner)?;
            if let Some(base) = &class.base {
                let parent = self.pou_ids.class_id(base).ok_or_else(missing_owner)?;
                self.construction
                    .templates
                    .template_parents
                    .insert(owner, parent);
            }
            for variable in &class.vars {
                self.add_instance_variable(owner, owner, variable, &variable.name)?;
            }
            self.collect_method_statics(owner, &class.name, &class.methods)?;
        }
        Ok(())
    }

    fn collect_method_statics(
        &mut self,
        template: u32,
        owner: &SmolStr,
        methods: &[MethodDef],
    ) -> Result<(), BytecodeError> {
        for method in methods {
            let method_id = self
                .pou_ids
                .method_id(owner, &method.name)
                .ok_or_else(missing_owner)?;
            let storage_owner =
                crate::program_model::method_static_storage_owner(owner, &method.name);
            for variable in &method.static_locals {
                let name =
                    crate::program_model::static_storage_name(&storage_owner, &variable.name);
                self.add_instance_variable(template, method_id, variable, &name)?;
            }
        }
        Ok(())
    }

    fn add_edge_states(&mut self, template: u32, owner: &SmolStr) -> Result<(), BytecodeError> {
        for input in self.runtime.edge_inputs_for_pou(owner) {
            let declaration_id = self
                .construction
                .templates
                .template_fields
                .get(&template)
                .into_iter()
                .flatten()
                .copied()
                .find(|id| {
                    let declaration = &self.construction.layout.entries[*id as usize];
                    self.strings.entries[declaration.name_idx as usize]
                        .eq_ignore_ascii_case(&input.name)
                })
                .ok_or_else(|| {
                    BytecodeError::InvalidSection("edge input declaration missing".into())
                })?;
            let input_declaration = &mut self.construction.layout.entries[declaration_id as usize];
            input_declaration.flags |= 2;
            let retain = input_declaration.retain;
            let falling = input.qualifier == trust_hir::symbols::EdgeQualifier::Falling;
            let seed = self.const_index_for(&crate::value::Value::Bool(falling))?;
            let name = crate::program_model::edge_phase_storage_name(owner, &input.name);
            let name_idx = self.strings.intern(name);
            let slot = u32::try_from(self.construction.templates.template_fields[&template].len())
                .map_err(|_| BytecodeError::InvalidSection("edge phase slot overflow".into()))?;
            let id = self.add_declaration(
                StorageDeclaration {
                    owner: StorageOwner::Instance,
                    role: StorageRole::EdgePhase,
                    retain,
                    flags: if falling { 0x20 } else { 0x10 },
                    owner_pou_id: Some(template),
                    name_idx,
                    type_id: None,
                    slot,
                    ref_idx: None,
                    default_const_idx: Some(seed),
                    construction_nodes: 0,
                    related_declaration_idx: Some(declaration_id),
                    source_name_idx: None,
                },
                Some(TypeId::BOOL),
                None,
            )?;
            self.construction
                .templates
                .template_fields
                .entry(template)
                .or_default()
                .push(id);
        }
        Ok(())
    }

    fn add_native_state(
        &mut self,
        template: u32,
        name: &str,
        type_id: TypeId,
    ) -> Result<(), BytecodeError> {
        let slot = u32::try_from(
            self.construction
                .templates
                .template_fields
                .get(&template)
                .map_or(0, Vec::len),
        )
        .map_err(|_| BytecodeError::InvalidSection("native state slot overflow".into()))?;
        let name_idx = self.strings.intern(name);
        let id = self.add_declaration(
            StorageDeclaration {
                owner: StorageOwner::Instance,
                role: StorageRole::NativeState,
                retain: 0,
                flags: 0,
                owner_pou_id: Some(template),
                name_idx,
                type_id: None,
                slot,
                ref_idx: None,
                default_const_idx: None,
                construction_nodes: 0,
                related_declaration_idx: None,
                source_name_idx: None,
            },
            Some(type_id),
            None,
        )?;
        self.construction
            .templates
            .template_fields
            .entry(template)
            .or_default()
            .push(id);
        Ok(())
    }
}

fn missing_owner() -> BytecodeError {
    BytecodeError::InvalidSection("construction owner missing".into())
}
