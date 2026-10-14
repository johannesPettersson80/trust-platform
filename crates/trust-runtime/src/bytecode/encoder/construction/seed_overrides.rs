//! Embedding-only value projection onto an initializer's privately staged result.
//! Existing typed reference paths preserve provenance, including constant members.
use super::*;
use crate::value::RefSegment;
use trust_hir::Type;

impl BytecodeEncoder<'_> {
    pub(super) fn emit_seed_override(
        &mut self,
        context: &CodegenContext,
        ty: TypeId,
        expression: &Expr,
        reference: ValueRef,
        code: &mut Vec<u8>,
        depth: usize,
    ) -> Result<(), BytecodeError> {
        if depth >= crate::bytecode::BYTECODE_MAX_CONST_NESTING as usize {
            return Err(invalid("builder seed nesting limit exceeded"));
        }
        let kind = self
            .runtime
            .registry()
            .get(ty)
            .cloned()
            .ok_or_else(|| invalid("builder seed type missing"))?;
        match (kind, expression) {
            (
                Type::Alias { target, .. },
                Expr::StructInitializer(_) | Expr::ArrayInitializer(_),
            ) => {
                return self.emit_seed_override(
                    context,
                    target,
                    expression,
                    reference,
                    code,
                    depth + 1,
                )
            }
            (
                Type::FunctionBlock { name } | Type::Class { name },
                Expr::StructInitializer(values),
            ) => {
                let mut seen = std::collections::HashSet::new();
                for (field, expression) in values {
                    if !seen.insert(normalize_name(field)) {
                        return Err(invalid("duplicate builder seed member"));
                    }
                    let (name, ty) = self.seed_member(&name, field)?;
                    let mut target = reference.clone();
                    target.path.push(RefSegment::Field(name));
                    self.emit_seed_override(context, ty, expression, target, code, depth + 1)?;
                }
                return Ok(());
            }
            (Type::Struct { fields, .. }, Expr::StructInitializer(values)) => {
                let mut seen = std::collections::HashSet::new();
                for (name, expression) in values {
                    if !seen.insert(normalize_name(name)) {
                        return Err(invalid("duplicate builder struct field"));
                    }
                    let field = fields
                        .iter()
                        .find(|field| field.name.eq_ignore_ascii_case(name))
                        .ok_or_else(|| invalid("builder struct field missing"))?;
                    let mut target = reference.clone();
                    target.path.push(RefSegment::Field(field.name.clone()));
                    self.emit_seed_override(
                        context,
                        field.type_id,
                        expression,
                        target,
                        code,
                        depth + 1,
                    )?;
                }
                return Ok(());
            }
            (Type::Union { variants, .. }, Expr::StructInitializer(values)) => {
                let mut seen = std::collections::HashSet::new();
                for (name, expression) in values {
                    if !seen.insert(normalize_name(name)) {
                        return Err(invalid("duplicate builder union field"));
                    }
                    let field = variants
                        .iter()
                        .find(|field| field.name.eq_ignore_ascii_case(name))
                        .ok_or_else(|| invalid("builder union field missing"))?;
                    let mut target = reference.clone();
                    target.path.push(RefSegment::Field(field.name.clone()));
                    self.emit_seed_override(
                        context,
                        field.type_id,
                        expression,
                        target,
                        code,
                        depth + 1,
                    )?;
                }
                return Ok(());
            }
            (
                Type::Array {
                    element,
                    dimensions,
                },
                Expr::ArrayInitializer(values),
            ) => {
                let expected = dimensions
                    .iter()
                    .try_fold(1usize, |count, (lower, upper)| {
                        let length = upper
                            .checked_sub(*lower)
                            .and_then(|n| n.checked_add(1))
                            .and_then(|n| usize::try_from(n).ok())
                            .ok_or_else(|| invalid("invalid seed array bound"))?;
                        count
                            .checked_mul(length)
                            .ok_or_else(|| invalid("seed array size overflow"))
                    })?;
                if expected != values.len() {
                    return Err(invalid("seed array length differs"));
                }
                for (offset, expression) in values.iter().enumerate() {
                    let mut rest = offset;
                    let mut indices = vec![0; dimensions.len()];
                    for (at, (lower, upper)) in dimensions.iter().enumerate().rev() {
                        let length = usize::try_from(i128::from(*upper) - i128::from(*lower) + 1)
                            .map_err(|_| invalid("seed dimension overflow"))?;
                        if length == 0 {
                            return Err(invalid("empty seed array dimension"));
                        }
                        indices[at] = lower
                            .checked_add(
                                i64::try_from(rest % length)
                                    .map_err(|_| invalid("seed index overflow"))?,
                            )
                            .ok_or_else(|| invalid("seed index overflow"))?;
                        rest /= length;
                    }
                    let mut target = reference.clone();
                    target
                        .path
                        .push(RefSegment::Index(crate::value::ref_indices_from_iter(
                            indices,
                        )));
                    self.emit_seed_override(context, element, expression, target, code, depth + 1)?;
                }
                return Ok(());
            }
            _ => {}
        }
        self.construction
            .bindings
            .types
            .insert(reference.clone(), ty);
        if !self.emit_expr(context, expression, code)? {
            return Err(invalid("builder seed expression cannot be lowered"));
        }
        code.push(crate::bytecode::opcodes::APPLY_INIT_VALUE);
        code.extend_from_slice(&self.type_index(ty)?.to_le_bytes());
        self.emit_store_ref(&reference, code)
    }
}
