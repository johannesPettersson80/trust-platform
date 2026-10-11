//! Declaration templates retain initializer expressions for later bytecode lowering.

use super::*;
use crate::bytecode::{
    StorageDeclaration, StorageOwner, StorageRole, BYTECODE_MAX_CONSTRUCTION_RECORDS,
};
use crate::program_model::{Param, VarDef};
use trust_hir::symbols::ParamDirection;

impl BytecodeEncoder<'_> {
    pub(in crate::bytecode::encoder) fn add_declaration(
        &mut self,
        mut declaration: StorageDeclaration,
        type_id: Option<TypeId>,
        expression: Option<Expr>,
    ) -> Result<u32, BytecodeError> {
        if self.construction.layout.entries.len() >= BYTECODE_MAX_CONSTRUCTION_RECORDS {
            return Err(BytecodeError::InvalidSection(
                "construction declaration limit exceeded".into(),
            ));
        }
        if expression.is_some()
            && type_id.is_some_and(|ty| {
                crate::harness::class_type_name(ty, self.runtime.registry()).is_some()
            })
        {
            return Err(BytecodeError::InvalidSection(
                "class instances cannot have initializers".into(),
            ));
        }
        declaration.type_id = type_id.map(|ty| self.type_index(ty)).transpose()?;
        let id = u32::try_from(self.construction.layout.entries.len())
            .map_err(|_| BytecodeError::InvalidSection("declaration identity overflow".into()))?;
        let needs_plan = declaration.role != StorageRole::NativeState;
        self.construction.layout.entries.push(declaration);
        self.construction.declaration_types.push(type_id);
        if let Some(type_id) = type_id.filter(|_| needs_plan) {
            self.construction.plans.push(DeclarationPlan {
                declaration: id,
                type_id,
                expression,
            });
        }
        Ok(id)
    }

    pub(in crate::bytecode::encoder) fn add_instance_variable(
        &mut self,
        template: u32,
        owner: u32,
        variable: &VarDef,
        stored_name: &SmolStr,
    ) -> Result<Option<u32>, BytecodeError> {
        if variable.external {
            return Ok(None);
        }
        let slot = self
            .construction
            .templates
            .template_fields
            .get(&template)
            .map_or(0, Vec::len);
        let slot = u32::try_from(slot)
            .map_err(|_| BytecodeError::InvalidSection("instance slot overflow".into()))?;
        let name_idx = self.strings.intern(stored_name.clone());
        let declaration = StorageDeclaration {
            owner: StorageOwner::Instance,
            role: if owner != template {
                StorageRole::Static
            } else {
                StorageRole::Variable
            },
            retain: retention(variable.retain),
            flags: u8::from(variable.constant) | if variable.in_out { 8 } else { 0 },
            owner_pou_id: Some(owner),
            name_idx,
            type_id: None,
            slot,
            ref_idx: None,
            default_const_idx: None,
            construction_nodes: 0,
            related_declaration_idx: None,
            source_name_idx: (owner != template)
                .then(|| self.strings.intern(variable.name.clone())),
        };
        let id = self.add_declaration(
            declaration,
            Some(variable.type_id),
            variable.initializer.clone(),
        )?;
        self.construction
            .templates
            .template_fields
            .entry(template)
            .or_default()
            .push(id);
        Ok(Some(id))
    }

    pub(in crate::bytecode::encoder) fn add_instance_parameter(
        &mut self,
        template: u32,
        parameter: &Param,
    ) -> Result<u32, BytecodeError> {
        let slot = self
            .construction
            .templates
            .template_fields
            .get(&template)
            .map_or(0, Vec::len);
        let slot = u32::try_from(slot)
            .map_err(|_| BytecodeError::InvalidSection("instance slot overflow".into()))?;
        let name_idx = self.strings.intern(parameter.name.clone());
        let flags = match parameter.direction {
            ParamDirection::In => 2,
            ParamDirection::Out => 4,
            ParamDirection::InOut => 8,
        };
        let expression = parameter_initializer(parameter);
        let declaration = StorageDeclaration {
            owner: StorageOwner::Instance,
            role: StorageRole::Parameter,
            retain: 0,
            flags,
            owner_pou_id: Some(template),
            name_idx,
            type_id: None,
            slot,
            ref_idx: None,
            default_const_idx: None,
            construction_nodes: 0,
            related_declaration_idx: None,
            source_name_idx: None,
        };
        let id = self.add_declaration(declaration, Some(parameter.type_id), expression)?;
        self.construction
            .templates
            .template_fields
            .entry(template)
            .or_default()
            .push(id);
        Ok(id)
    }
}

pub(super) fn retention(policy: crate::RetainPolicy) -> u8 {
    match policy {
        crate::RetainPolicy::Unspecified => 0,
        crate::RetainPolicy::Retain => 1,
        crate::RetainPolicy::NonRetain => 2,
        crate::RetainPolicy::Persistent => 3,
    }
}

pub(super) fn parameter_initializer(parameter: &Param) -> Option<Expr> {
    if (parameter.direction == ParamDirection::In && parameter.name.eq_ignore_ascii_case("EN"))
        || (parameter.direction == ParamDirection::Out
            && parameter.name.eq_ignore_ascii_case("ENO"))
    {
        Some(Expr::Literal(crate::value::Value::Bool(true)))
    } else {
        parameter.default.clone()
    }
}
