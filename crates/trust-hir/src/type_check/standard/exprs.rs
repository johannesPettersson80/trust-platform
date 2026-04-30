use super::super::calls::NameResolveOutcome;
use super::super::*;

impl<'a> TypeChecker<'a> {
    pub(in crate::type_check) fn infer_paren_expr(&mut self, node: &SyntaxNode) -> TypeId {
        node.children()
            .next()
            .map(|child| self.expr().check_expression(&child))
            .unwrap_or_else(|| {
                self.legacy_type_from_outcome(self.unknown_type_outcome(node.text_range()))
            })
    }

    pub(in crate::type_check) fn infer_this_expr(&mut self, node: &SyntaxNode) -> TypeId {
        if let Some(ty) = self.this_type {
            return ty;
        }

        self.legacy_diagnostic_type(
            DiagnosticCode::CannotResolve,
            node.text_range(),
            "THIS is only valid inside function blocks or interfaces",
        )
    }

    pub(in crate::type_check) fn infer_super_expr(&mut self, node: &SyntaxNode) -> TypeId {
        if let Some(ty) = self.super_type {
            return ty;
        }

        self.legacy_diagnostic_type(
            DiagnosticCode::CannotResolve,
            node.text_range(),
            "SUPER is only valid when a base type is declared with EXTENDS",
        )
    }

    pub(in crate::type_check) fn infer_size_of_expr(&mut self, node: &SyntaxNode) -> TypeId {
        if let Some(type_ref) = node
            .children()
            .find(|child| child.kind() == SyntaxKind::TypeRef)
        {
            if let Some(type_id) = self.resolve_sizeof_type_ref(&type_ref) {
                self.validate_sizeof_target_type(type_id, node.text_range());
            } else {
                self.diagnostics.error(
                    DiagnosticCode::UndefinedType,
                    type_ref.text_range(),
                    "unable to resolve SIZEOF type operand",
                );
            }
            return TypeId::DINT;
        }

        if let Some(expr) = node
            .children()
            .find(|child| is_expression_kind(child.kind()))
        {
            match self.resolve_sizeof_expr_target_type(&expr) {
                Ok(type_id) => {
                    self.validate_sizeof_target_type(type_id, expr.text_range());
                }
                Err(SizeOfOperandError::UnknownName(name)) => {
                    self.diagnostics.error(
                        DiagnosticCode::CannotResolve,
                        expr.text_range(),
                        format!("SIZEOF operand '{name}' is neither a variable nor a type"),
                    );
                }
                Err(SizeOfOperandError::InvalidOperand) => {
                    self.diagnostics.error(
                        DiagnosticCode::InvalidOperation,
                        expr.text_range(),
                        "SIZEOF expects a type name or storage operand",
                    );
                }
                Err(SizeOfOperandError::SuppressedCascade) => {}
            }
        }

        TypeId::DINT
    }

    fn resolve_sizeof_expr_target_type(
        &mut self,
        node: &SyntaxNode,
    ) -> Result<TypeId, SizeOfOperandError> {
        if let Some(type_id) = self.resolve_sizeof_value_operand_type(node)? {
            return Ok(type_id);
        }
        if let Some(type_id) = self.resolve_sizeof_named_type_operand(node)? {
            return Ok(type_id);
        }
        if let Some(name) = self.sizeof_unknown_name(node) {
            return Err(SizeOfOperandError::UnknownName(name));
        }

        if is_expression_kind(node.kind()) {
            let _ = self.expr().check_expression(node);
        }
        Err(SizeOfOperandError::InvalidOperand)
    }

    fn resolve_sizeof_value_operand_type(
        &mut self,
        node: &SyntaxNode,
    ) -> Result<Option<TypeId>, SizeOfOperandError> {
        if node.kind() == SyntaxKind::ParenExpr {
            return node
                .children()
                .next()
                .map(|inner| self.resolve_sizeof_value_operand_type(&inner))
                .unwrap_or(Ok(None));
        }

        match node.kind() {
            SyntaxKind::NameRef => {
                let Some(name) = self.resolve_ref().get_name_from_ref(node) else {
                    return Ok(None);
                };
                let resolved = match self
                    .resolve()
                    .resolve_name_in_context_outcome(&name, node.text_range())
                {
                    NameResolveOutcome::Resolved(resolved) => resolved,
                    NameResolveOutcome::Ambiguous => {
                        return Err(SizeOfOperandError::SuppressedCascade);
                    }
                    NameResolveOutcome::NotFound => return Ok(None),
                };
                let Some(symbol) = self.symbols.get(resolved.id) else {
                    return Ok(None);
                };
                if symbol.is_type() || matches!(symbol.kind, SymbolKind::Namespace) {
                    return Ok(None);
                }
            }
            SyntaxKind::FieldExpr => {
                if let Some(symbol_id) = self.resolve_ref().resolve_namespace_qualified_symbol(node)
                {
                    let Some(symbol) = self.symbols.get(symbol_id) else {
                        return Ok(None);
                    };
                    if symbol.is_type() {
                        return Ok(None);
                    }
                }
                if !self.is_valid_lvalue(node) {
                    return Ok(None);
                }
            }
            SyntaxKind::ThisExpr | SyntaxKind::SuperExpr => {}
            _ if self.is_valid_lvalue(node) => {}
            _ => return Ok(None),
        };

        let type_id = self.expr().check_expression(node);

        Ok((type_id != TypeId::UNKNOWN).then_some(type_id))
    }

    fn resolve_sizeof_named_type_operand(
        &mut self,
        node: &SyntaxNode,
    ) -> Result<Option<TypeId>, SizeOfOperandError> {
        if node.kind() == SyntaxKind::ParenExpr {
            return node
                .children()
                .next()
                .map(|inner| self.resolve_sizeof_named_type_operand(&inner))
                .unwrap_or(Ok(None));
        }

        match node.kind() {
            SyntaxKind::NameRef => {
                let Some(name) = self.resolve_ref().get_name_from_ref(node) else {
                    return Ok(None);
                };
                match self
                    .resolve()
                    .resolve_name_in_context_outcome(&name, node.text_range())
                {
                    NameResolveOutcome::Resolved(resolved) => {
                        let Some(symbol) = self.symbols.get(resolved.id) else {
                            return Ok(None);
                        };
                        Ok(symbol.is_type().then_some(symbol.type_id))
                    }
                    NameResolveOutcome::Ambiguous => Err(SizeOfOperandError::SuppressedCascade),
                    NameResolveOutcome::NotFound => {
                        Ok(self.resolve_ref().resolve_type_by_name(name.as_str()))
                    }
                }
            }
            SyntaxKind::FieldExpr => {
                let Some(symbol_id) = self.resolve_ref().resolve_namespace_qualified_symbol(node)
                else {
                    return Ok(None);
                };
                let Some(symbol) = self.symbols.get(symbol_id) else {
                    return Ok(None);
                };
                Ok(symbol.is_type().then_some(symbol.type_id))
            }
            _ => Ok(None),
        }
    }

    fn sizeof_unknown_name(&self, node: &SyntaxNode) -> Option<String> {
        if node.kind() == SyntaxKind::ParenExpr {
            return node
                .children()
                .next()
                .and_then(|inner| self.sizeof_unknown_name(&inner));
        }

        (node.kind() == SyntaxKind::NameRef)
            .then(|| {
                self.resolve_ref()
                    .get_name_from_ref(node)
                    .map(|name| name.to_string())
            })
            .flatten()
    }

    fn resolve_sizeof_type_ref(&mut self, node: &SyntaxNode) -> Option<TypeId> {
        if let Some(array_node) = node
            .children()
            .find(|child| child.kind() == SyntaxKind::ArrayType)
        {
            let element_ref = array_node
                .children()
                .find(|child| child.kind() == SyntaxKind::TypeRef)?;
            let element = self.resolve_sizeof_type_ref(&element_ref)?;
            let mut dimensions = Vec::new();
            for subrange in array_node
                .children()
                .filter(|child| child.kind() == SyntaxKind::Subrange)
            {
                dimensions.push(self.resolve_sizeof_subrange(&subrange)?);
            }
            return Some(self.symbols.register_array_type(element, dimensions));
        }

        if let Some(pointer_node) = node
            .children()
            .find(|child| child.kind() == SyntaxKind::PointerType)
        {
            let inner = pointer_node
                .children()
                .find(|child| child.kind() == SyntaxKind::TypeRef)?;
            let target = self.resolve_sizeof_type_ref(&inner)?;
            return Some(self.symbols.register_pointer_type(target));
        }

        if let Some(reference_node) = node
            .children()
            .find(|child| child.kind() == SyntaxKind::ReferenceType)
        {
            let inner = reference_node
                .children()
                .find(|child| child.kind() == SyntaxKind::TypeRef)?;
            let target = self.resolve_sizeof_type_ref(&inner)?;
            return Some(self.symbols.register_reference_type(target));
        }

        if let Some(string_node) = node
            .children()
            .find(|child| child.kind() == SyntaxKind::StringType)
        {
            let is_wide = string_node
                .children_with_tokens()
                .filter_map(|element| element.into_token())
                .any(|token| token.kind() == SyntaxKind::KwWString);
            if let Some(len_expr) = string_node
                .children()
                .find(|child| is_expression_kind(child.kind()))
            {
                let len = self.require_const_int_expr(
                    &len_expr,
                    "STRING/WSTRING length must be a constant integer expression",
                )?;
                let len = u32::try_from(len).ok()?;
                return Some(self.register_sized_string_type(is_wide, len));
            }
            return Some(if is_wide {
                TypeId::WSTRING
            } else {
                TypeId::STRING
            });
        }

        let mut base = None;
        if let Some(name_node) = node
            .children()
            .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName))
        {
            let name = name_node.text().to_string();
            base = self.resolve_ref().resolve_type_by_name(name.trim());
        }
        if base.is_none() {
            for token in node
                .children_with_tokens()
                .filter_map(|element| element.into_token())
            {
                if let Some(type_id) = TypeId::from_builtin_name(token.text()) {
                    base = Some(type_id);
                    break;
                }
            }
        }
        let mut type_id = base?;

        if let Some(subrange) = node
            .children()
            .find(|child| child.kind() == SyntaxKind::Subrange)
        {
            let (lower, upper) = self.resolve_sizeof_subrange(&subrange)?;
            type_id = self.symbols.register_subrange_type(type_id, lower, upper);
        }

        Some(type_id)
    }

    fn resolve_sizeof_subrange(&mut self, node: &SyntaxNode) -> Option<(i64, i64)> {
        let exprs: Vec<_> = node
            .children()
            .filter(|child| is_expression_kind(child.kind()))
            .collect();
        match exprs.as_slice() {
            [] => None,
            [expr] if expr.text().to_string().trim() == "*" => Some((0, i64::MAX)),
            [expr] => {
                let value = self.require_const_int_expr(
                    expr,
                    "subrange bound must be a constant integer expression",
                )?;
                Some((value, value))
            }
            [lower, upper]
                if lower.text().to_string().trim() == "*"
                    || upper.text().to_string().trim() == "*" =>
            {
                Some((0, i64::MAX))
            }
            [lower, upper] => Some((
                self.require_const_int_expr(
                    lower,
                    "subrange lower bound must be a constant integer expression",
                )?,
                self.require_const_int_expr(
                    upper,
                    "subrange upper bound must be a constant integer expression",
                )?,
            )),
            _ => None,
        }
    }

    fn register_sized_string_type(&mut self, is_wide: bool, max_len: u32) -> TypeId {
        let name = if is_wide {
            format!("WSTRING[{max_len}]")
        } else {
            format!("STRING[{max_len}]")
        };
        let ty = if is_wide {
            Type::WString {
                max_len: Some(max_len),
            }
        } else {
            Type::String {
                max_len: Some(max_len),
            }
        };
        self.symbols.register_type(name, ty)
    }

    fn validate_sizeof_target_type(&mut self, type_id: TypeId, range: TextRange) {
        if let Err(message) = self.ensure_sizeof_target_type_supported(type_id) {
            self.diagnostics
                .error(DiagnosticCode::InvalidOperation, range, message);
        }
    }

    fn ensure_sizeof_target_type_supported(&self, type_id: TypeId) -> Result<(), String> {
        let mut stack = Vec::new();
        self.ensure_sizeof_target_type_supported_inner(type_id, &mut stack)
    }

    fn ensure_sizeof_target_type_supported_inner(
        &self,
        type_id: TypeId,
        stack: &mut Vec<TypeId>,
    ) -> Result<(), String> {
        if stack.contains(&type_id) {
            return Ok(());
        }
        stack.push(type_id);

        let ty = self
            .symbols
            .type_by_id(type_id)
            .ok_or_else(|| "SIZEOF operand type is unknown".to_string())?;
        let result = match ty {
            Type::Alias { target, .. } => {
                self.ensure_sizeof_target_type_supported_inner(*target, stack)
            }
            Type::Subrange { base, .. } => {
                self.ensure_sizeof_target_type_supported_inner(*base, stack)
            }
            Type::Enum { base, .. } => self.ensure_sizeof_target_type_supported_inner(*base, stack),
            Type::Array {
                element,
                dimensions,
            } => {
                if dimensions
                    .iter()
                    .any(|(lower, upper)| *lower == 0 && *upper == i64::MAX)
                {
                    Err("SIZEOF does not support open ARRAY[*] storage size".to_string())
                } else {
                    self.ensure_sizeof_target_type_supported_inner(*element, stack)
                }
            }
            Type::Struct { fields, .. } => {
                for field in fields {
                    self.ensure_sizeof_target_type_supported_inner(field.type_id, stack)?;
                }
                Ok(())
            }
            Type::Union { variants, .. } => {
                for variant in variants {
                    self.ensure_sizeof_target_type_supported_inner(variant.type_id, stack)?;
                }
                Ok(())
            }
            Type::String { max_len: None } | Type::WString { max_len: None } => Err(
                "SIZEOF requires STRING/WSTRING operands with an explicit declared length"
                    .to_string(),
            ),
            Type::FunctionBlock { .. } | Type::Class { .. } | Type::Interface { .. } => Err(
                "SIZEOF does not support function block, class, or interface instance storage size"
                    .to_string(),
            ),
            _ => Ok(()),
        };

        let _ = stack.pop();
        result
    }
}

enum SizeOfOperandError {
    UnknownName(String),
    InvalidOperand,
    SuppressedCascade,
}
