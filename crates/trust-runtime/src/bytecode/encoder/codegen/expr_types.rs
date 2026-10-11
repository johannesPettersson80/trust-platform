//! Receiver-aware source types for defaults; no live Values are evaluated.

use super::*;
use crate::program_model::{Expr, FunctionBlockBase, LValue};
use trust_hir::{Type, TypeId};

impl BytecodeEncoder<'_> {
    pub(super) fn source_disabled_call_result_type(
        &mut self,
        ctx: &CodegenContext,
        target: &NativeCallTarget<'_>,
    ) -> Result<Option<TypeId>, BytecodeError> {
        if !matches!(target.kind, NativeTargetKind::Method) {
            return self.disabled_call_result_type(target);
        }
        let receiver = match &target.receiver {
            NativeReceiver::Expression(expression) => self.expression_type(ctx, expression, 0)?,
            NativeReceiver::SelfValue => ctx
                .owner_name
                .as_ref()
                .and_then(|name| self.runtime.registry().lookup(name)),
            NativeReceiver::None => None,
        }
        .ok_or_else(|| invalid("disabled method receiver type is unresolved"))?;
        self.method_return_type(receiver, &target.name)
    }

    fn expression_type(
        &mut self,
        ctx: &CodegenContext,
        expr: &Expr,
        depth: u8,
    ) -> Result<Option<TypeId>, BytecodeError> {
        if depth > 64 {
            return Err(invalid("receiver expression type recursion exceeded"));
        }
        match expr {
            Expr::Name(name) => {
                if let Some(reference) = ctx.local_ref(name).or_else(|| ctx.static_ref(name)) {
                    return Ok(self.construction.bindings.types.get(reference).copied());
                }
                if let Some(field) = ctx.self_field_name(name) {
                    if let Some(owner) = &ctx.owner_name {
                        let mut template = self
                            .pou_ids
                            .program_id(owner)
                            .or_else(|| self.pou_ids.class_like_id(owner));
                        for _ in 0..64 {
                            let Some(id) = template else {
                                break;
                            };
                            if let Some(declaration) = self
                                .construction
                                .templates
                                .template_fields
                                .get(&id)
                                .into_iter()
                                .flatten()
                                .find(|id| {
                                    let declaration =
                                        &self.construction.layout.entries[**id as usize];
                                    self.strings.entries[declaration.name_idx as usize]
                                        .eq_ignore_ascii_case(field)
                                })
                            {
                                return Ok(
                                    self.construction.declaration_types[*declaration as usize]
                                );
                            }
                            template = self
                                .construction
                                .templates
                                .template_parents
                                .get(&id)
                                .copied();
                        }
                        if let Some(ty) = self.runtime.registry().lookup(owner) {
                            return self.field_type(ty, field).map(Some);
                        }
                    }
                }
                let reference = self.resolve_name_ref(ctx, name)?;
                Ok(reference.and_then(|reference| {
                    self.construction.bindings.types.get(&reference).copied()
                }))
            }
            Expr::This => Ok(ctx
                .owner_name
                .as_ref()
                .and_then(|name| self.runtime.registry().lookup(name))),
            Expr::Super => {
                let ty = ctx
                    .owner_name
                    .as_ref()
                    .and_then(|name| self.runtime.registry().lookup(name))
                    .ok_or_else(|| invalid("SUPER owner missing"))?;
                self.parent_type(ty)
            }
            Expr::Field { target, field } => {
                let ty = self
                    .expression_type(ctx, target, depth + 1)?
                    .ok_or_else(|| invalid("field receiver type missing"))?;
                self.field_type(ty, field).map(Some)
            }
            Expr::Index { target, .. } => {
                let ty = self
                    .expression_type(ctx, target, depth + 1)?
                    .ok_or_else(|| invalid("array receiver type missing"))?;
                match self.resolved_type(ty)? {
                    Type::Array { element, .. } => Ok(Some(*element)),
                    _ => Err(invalid("indexed receiver is not an array")),
                }
            }
            Expr::Deref(value) => {
                if let Expr::Ref(target) = value.as_ref() {
                    return self.lvalue_type(ctx, target, depth + 1);
                }
                let ty = self
                    .expression_type(ctx, value, depth + 1)?
                    .ok_or_else(|| invalid("reference receiver type missing"))?;
                match self.resolved_type(ty)? {
                    Type::Reference { target } => Ok(Some(*target)),
                    _ => Err(invalid("receiver dereference is not a reference")),
                }
            }
            Expr::Call { target, .. } => {
                let call = self.resolve_native_call_target(ctx, target)?;
                match call.kind {
                    NativeTargetKind::Function => self.disabled_call_result_type(&call),
                    NativeTargetKind::Method => {
                        let ty = match &call.receiver {
                            NativeReceiver::Expression(value) => {
                                self.expression_type(ctx, value, depth + 1)?
                            }
                            NativeReceiver::SelfValue => ctx
                                .owner_name
                                .as_ref()
                                .and_then(|name| self.runtime.registry().lookup(name)),
                            NativeReceiver::None => None,
                        }
                        .ok_or_else(|| invalid("method result receiver type missing"))?;
                        self.method_return_type(ty, &call.name)
                    }
                    _ => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }

    fn lvalue_type(
        &mut self,
        ctx: &CodegenContext,
        value: &LValue,
        depth: u8,
    ) -> Result<Option<TypeId>, BytecodeError> {
        if depth > 64 {
            return Err(invalid("receiver lvalue recursion exceeded"));
        }
        match value {
            LValue::Name(name) => self.expression_type(ctx, &Expr::Name(name.clone()), depth + 1),
            LValue::Field { target, field } => {
                let ty = self
                    .lvalue_type(ctx, target, depth + 1)?
                    .ok_or_else(|| invalid("lvalue field type missing"))?;
                self.field_type(ty, field).map(Some)
            }
            LValue::Index { target, .. } => {
                let ty = self
                    .lvalue_type(ctx, target, depth + 1)?
                    .ok_or_else(|| invalid("lvalue array type missing"))?;
                match self.resolved_type(ty)? {
                    Type::Array { element, .. } => Ok(Some(*element)),
                    _ => Err(invalid("lvalue is not an array")),
                }
            }
            LValue::Deref(value) => {
                self.expression_type(ctx, &Expr::Deref(value.clone()), depth + 1)
            }
        }
    }

    fn resolved_type(&self, mut ty: TypeId) -> Result<&Type, BytecodeError> {
        for _ in 0..64 {
            match self
                .runtime
                .registry()
                .get(ty)
                .ok_or_else(|| invalid("receiver type missing"))?
            {
                Type::Alias { target, .. } => ty = *target,
                value => return Ok(value),
            }
        }
        Err(invalid("receiver type recursion exceeded"))
    }

    fn parent_type(&self, ty: TypeId) -> Result<Option<TypeId>, BytecodeError> {
        let name = match self.resolved_type(ty)? {
            Type::FunctionBlock { name } => self
                .runtime
                .function_blocks()
                .get(&super::super::util::normalize_name(name))
                .and_then(|owner| owner.base.as_ref())
                .map(|base| match base {
                    FunctionBlockBase::FunctionBlock(name) | FunctionBlockBase::Class(name) => name,
                }),
            Type::Class { name } => self
                .runtime
                .classes()
                .get(&super::super::util::normalize_name(name))
                .and_then(|owner| owner.base.as_ref()),
            Type::Interface { name } => self
                .runtime
                .interfaces()
                .get(&super::super::util::normalize_name(name))
                .and_then(|owner| owner.base.as_ref()),
            _ => None,
        };
        name.map(|name| {
            self.runtime
                .registry()
                .lookup(name)
                .ok_or_else(|| invalid("parent receiver type missing"))
        })
        .transpose()
    }

    fn method_return_type(
        &self,
        mut ty: TypeId,
        method: &str,
    ) -> Result<Option<TypeId>, BytecodeError> {
        for _ in 0..64 {
            let methods = match self.resolved_type(ty)? {
                Type::FunctionBlock { name } => self
                    .runtime
                    .function_blocks()
                    .get(&super::super::util::normalize_name(name))
                    .map(|owner| &owner.methods),
                Type::Class { name } => self
                    .runtime
                    .classes()
                    .get(&super::super::util::normalize_name(name))
                    .map(|owner| &owner.methods),
                Type::Interface { name } => self
                    .runtime
                    .interfaces()
                    .get(&super::super::util::normalize_name(name))
                    .map(|owner| &owner.methods),
                _ => None,
            }
            .ok_or_else(|| invalid("method receiver declaration missing"))?;
            if let Some(method) = methods
                .iter()
                .find(|candidate| candidate.name.eq_ignore_ascii_case(method))
            {
                return Ok(method.return_type);
            }
            ty = self
                .parent_type(ty)?
                .ok_or_else(|| invalid("method declaration missing"))?;
        }
        Err(invalid("method inheritance recursion exceeded"))
    }

    fn field_type(&self, mut ty: TypeId, field: &str) -> Result<TypeId, BytecodeError> {
        for _ in 0..64 {
            let found = match self.resolved_type(ty)? {
                Type::Struct { fields, .. } => fields
                    .iter()
                    .find(|item| item.name.eq_ignore_ascii_case(field))
                    .map(|item| item.type_id),
                Type::Union { variants, .. } => variants
                    .iter()
                    .find(|item| item.name.eq_ignore_ascii_case(field))
                    .map(|item| item.type_id),
                Type::FunctionBlock { name } => self
                    .runtime
                    .function_blocks()
                    .get(&super::super::util::normalize_name(name))
                    .and_then(|owner| {
                        owner
                            .params
                            .iter()
                            .find(|item| item.name.eq_ignore_ascii_case(field))
                            .map(|item| item.type_id)
                            .or_else(|| {
                                owner
                                    .vars
                                    .iter()
                                    .find(|item| item.name.eq_ignore_ascii_case(field))
                                    .map(|item| item.type_id)
                            })
                    }),
                Type::Class { name } => self
                    .runtime
                    .classes()
                    .get(&super::super::util::normalize_name(name))
                    .and_then(|owner| {
                        owner
                            .vars
                            .iter()
                            .find(|item| item.name.eq_ignore_ascii_case(field))
                            .map(|item| item.type_id)
                    }),
                _ => None,
            };
            if let Some(ty) = found {
                return Ok(ty);
            }
            ty = self
                .parent_type(ty)?
                .ok_or_else(|| invalid("receiver field type missing"))?;
        }
        Err(invalid("field inheritance recursion exceeded"))
    }
}

fn invalid(message: &str) -> BytecodeError {
    BytecodeError::InvalidSection(message.into())
}
