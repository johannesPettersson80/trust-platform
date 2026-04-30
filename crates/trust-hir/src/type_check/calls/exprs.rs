use super::super::*;
use super::*;
use crate::semantic::SemanticOutcome;

impl<'a, 'b> CallChecker<'a, 'b> {
    pub(in crate::type_check) fn infer_index_expr(&mut self, node: &SyntaxNode) -> TypeId {
        let outcome = self.infer_index_expr_outcome(node);
        self.checker.legacy_type_from_outcome(outcome)
    }

    fn infer_index_expr_outcome(&mut self, node: &SyntaxNode) -> SemanticOutcome<TypeId> {
        let children: Vec<_> = node.children().collect();
        if children.is_empty() {
            return self.checker.unknown_type_outcome(node.text_range());
        }

        let base_type = self.checker.expr().check_expression(&children[0]);
        let resolved_base = self.checker.resolve_alias_type(base_type);
        let index_count = children.len().saturating_sub(1);
        let mut index_exprs = Vec::new();

        // Check index expression types (must be integers).
        for idx_expr in children.iter().skip(1) {
            let idx_type_raw = self.checker.expr().check_expression(idx_expr);
            let idx_type = self.checker.resolve_alias_type(idx_type_raw);
            index_exprs.push((idx_expr.clone(), idx_type_raw, idx_type));
            if let Some(ty) = self.checker.symbols.type_by_id(idx_type) {
                if !ty.is_integer() {
                    self.checker.diagnostics.error(
                        DiagnosticCode::InvalidArrayIndex,
                        idx_expr.text_range(),
                        "array index must be an integer type",
                    );
                }
            }
        }

        if resolved_base == TypeId::UNKNOWN {
            return self
                .checker
                .suppressed_type_outcome(DiagnosticCode::CannotResolve, children[0].text_range());
        }

        if let Some(base_type) = self.checker.symbols.type_by_id(resolved_base) {
            match base_type {
                Type::Array {
                    element,
                    dimensions,
                } => {
                    let element = *element;
                    let dimensions = dimensions.clone();
                    if index_count != dimensions.len() {
                        return self.checker.diagnostic_type_outcome(
                            DiagnosticCode::InvalidArrayIndex,
                            node.text_range(),
                            format!(
                                "expected {} index value(s), found {}",
                                dimensions.len(),
                                index_count
                            ),
                        );
                    }
                    for ((expr, _, idx_type), (lower, upper)) in
                        index_exprs.iter().zip(dimensions.iter())
                    {
                        self.check_array_index_bounds(expr, *idx_type, *lower, *upper);
                    }
                    return SemanticOutcome::Resolved(element);
                }
                Type::String { max_len } => {
                    if index_count != 1 {
                        return self.checker.diagnostic_type_outcome(
                            DiagnosticCode::InvalidArrayIndex,
                            node.text_range(),
                            format!("expected 1 index value, found {index_count}"),
                        );
                    }
                    if let Some(max_len) = max_len {
                        self.check_array_index_bounds(
                            &index_exprs[0].0,
                            index_exprs[0].2,
                            1,
                            i64::from(*max_len),
                        );
                    }
                    return SemanticOutcome::Resolved(TypeId::CHAR);
                }
                Type::WString { max_len } => {
                    if index_count != 1 {
                        return self.checker.diagnostic_type_outcome(
                            DiagnosticCode::InvalidArrayIndex,
                            node.text_range(),
                            format!("expected 1 index value, found {index_count}"),
                        );
                    }
                    if let Some(max_len) = max_len {
                        self.check_array_index_bounds(
                            &index_exprs[0].0,
                            index_exprs[0].2,
                            1,
                            i64::from(*max_len),
                        );
                    }
                    return SemanticOutcome::Resolved(TypeId::WCHAR);
                }
                _ => {}
            }
        }

        self.checker.diagnostic_type_outcome(
            DiagnosticCode::TypeMismatch,
            node.text_range(),
            "indexing requires an array, STRING, or WSTRING type",
        )
    }

    pub(in crate::type_check) fn infer_field_expr(&mut self, node: &SyntaxNode) -> TypeId {
        let outcome = self.infer_field_expr_outcome(node);
        self.checker.legacy_type_from_outcome(outcome)
    }

    fn infer_field_expr_outcome(&mut self, node: &SyntaxNode) -> SemanticOutcome<TypeId> {
        let children: Vec<_> = node.children().collect();
        if children.len() < 2 {
            return self.checker.unknown_type_outcome(node.text_range());
        }

        if let Some(symbol_id) = self
            .checker
            .resolve_ref()
            .resolve_namespace_qualified_symbol(node)
        {
            if let Some(symbol) = self.checker.symbols.get(symbol_id) {
                if let Some(role) = non_value_role(&symbol.kind) {
                    let name = symbol.name.clone();
                    return self.checker.diagnostic_type_outcome(
                        DiagnosticCode::InvalidOperation,
                        node.text_range(),
                        format!("{role} '{name}' cannot be used as a value"),
                    );
                }
                return SemanticOutcome::Resolved(symbol.type_id);
            }
        }

        let base = &children[0];
        let member = &children[1];
        let base_type = self.checker.expr().check_expression(base);
        let field_name = self.checker.resolve_ref().get_name_from_ref(member);

        if field_name.is_none() {
            if let Some(outcome) = self.infer_partial_bit_access_outcome(base_type, member) {
                return outcome;
            }
            return self.checker.unknown_type_outcome(member.text_range());
        }

        let field_name = field_name.unwrap();
        let resolved_base = self.checker.resolve_alias_type(base_type);

        if base_type == TypeId::UNKNOWN || resolved_base == TypeId::UNKNOWN {
            return self
                .checker
                .suppressed_type_outcome(DiagnosticCode::CannotResolve, base.text_range());
        }

        if let Some(ty) = self.checker.symbols.type_by_id(resolved_base) {
            match ty {
                Type::Struct { .. } | Type::Union { .. } => {
                    if let Some(field_type) = self
                        .checker
                        .resolve_ref()
                        .resolve_member_in_type(base_type, &field_name)
                    {
                        return SemanticOutcome::Resolved(field_type);
                    }
                    return self.checker.diagnostic_type_outcome(
                        DiagnosticCode::CannotResolve,
                        member.text_range(),
                        format!("no field '{}' on struct", field_name),
                    );
                }
                Type::FunctionBlock { .. } | Type::Class { .. } | Type::Interface { .. } => {
                    if let Some(resolved) = self.checker.resolve().resolve_member_symbol_in_type(
                        base_type,
                        &field_name,
                        member.text_range(),
                    ) {
                        let Some(symbol) = self.checker.symbols.get(resolved.id) else {
                            return self.checker.unknown_type_outcome(member.text_range());
                        };
                        if resolved.accessible {
                            if let SymbolKind::Property { has_get, .. } = symbol.kind {
                                if !has_get {
                                    self.checker.diagnostics.error(
                                        DiagnosticCode::InvalidOperation,
                                        member.text_range(),
                                        format!("property '{}' has no getter", symbol.name),
                                    );
                                }
                            }
                        }
                        return SemanticOutcome::Resolved(symbol.type_id);
                    }
                    return self.checker.diagnostic_type_outcome(
                        DiagnosticCode::CannotResolve,
                        member.text_range(),
                        format!("no member '{}' on type", field_name),
                    );
                }
                _ => {
                    return self.checker.diagnostic_type_outcome(
                        DiagnosticCode::TypeMismatch,
                        node.text_range(),
                        "field access requires struct, function block, or class type",
                    );
                }
            }
        }

        SemanticOutcome::InvariantViolation {
            message: SmolStr::new(format!("type id {:?} has no registry entry", resolved_base)),
            range: Some(base.text_range()),
        }
    }

    fn infer_partial_bit_access_outcome(
        &mut self,
        base_type: TypeId,
        member: &SyntaxNode,
    ) -> Option<SemanticOutcome<TypeId>> {
        let access = parse_partial_access(member.text().to_string().trim())?;
        let resolved = self.checker.resolve_alias_type(base_type);
        if base_type == TypeId::UNKNOWN || resolved == TypeId::UNKNOWN {
            return Some(
                self.checker
                    .suppressed_type_outcome(DiagnosticCode::CannotResolve, member.text_range()),
            );
        }

        let Some(ty) = self.checker.symbols.type_by_id(resolved) else {
            return Some(SemanticOutcome::InvariantViolation {
                message: SmolStr::new(format!("type id {:?} has no registry entry", resolved)),
                range: Some(member.text_range()),
            });
        };

        let (result, max_index) = match (ty, access) {
            (Type::Byte, PartialAccess::Bit(_)) => (TypeId::BOOL, 7u8),
            (Type::Word, PartialAccess::Bit(_)) => (TypeId::BOOL, 15u8),
            (Type::DWord, PartialAccess::Bit(_)) => (TypeId::BOOL, 31u8),
            (Type::LWord, PartialAccess::Bit(_)) => (TypeId::BOOL, 63u8),
            (Type::Word, PartialAccess::Byte(_)) => (TypeId::BYTE, 1u8),
            (Type::DWord, PartialAccess::Byte(_)) => (TypeId::BYTE, 3u8),
            (Type::LWord, PartialAccess::Byte(_)) => (TypeId::BYTE, 7u8),
            (Type::DWord, PartialAccess::Word(_)) => (TypeId::WORD, 1u8),
            (Type::LWord, PartialAccess::Word(_)) => (TypeId::WORD, 3u8),
            (Type::LWord, PartialAccess::DWord(_)) => (TypeId::DWORD, 1u8),
            _ => return None,
        };
        let index = access.index();
        if index > max_index {
            return Some(self.checker.diagnostic_type_outcome(
                DiagnosticCode::OutOfRange,
                member.text_range(),
                "partial access index out of range",
            ));
        }
        Some(SemanticOutcome::Resolved(result))
    }

    pub(in crate::type_check) fn infer_deref_expr(&mut self, node: &SyntaxNode) -> TypeId {
        let outcome = self.infer_deref_expr_outcome(node);
        self.checker.legacy_type_from_outcome(outcome)
    }

    fn infer_deref_expr_outcome(&mut self, node: &SyntaxNode) -> SemanticOutcome<TypeId> {
        let operand = match node.children().next() {
            Some(child) => self.checker.expr().check_expression(&child),
            None => return self.checker.unknown_type_outcome(node.text_range()),
        };

        let operand = self.checker.resolve_alias_type(operand);
        if operand == TypeId::UNKNOWN {
            return self
                .checker
                .suppressed_type_outcome(DiagnosticCode::CannotResolve, node.text_range());
        }
        if let Some(Type::Pointer { target } | Type::Reference { target }) =
            self.checker.symbols.type_by_id(operand)
        {
            return SemanticOutcome::Resolved(*target);
        }

        self.checker.diagnostic_type_outcome(
            DiagnosticCode::TypeMismatch,
            node.text_range(),
            "dereference requires pointer type",
        )
    }

    pub(in crate::type_check) fn infer_addr_expr(&mut self, node: &SyntaxNode) -> TypeId {
        let outcome = self.infer_addr_expr_outcome(node);
        self.checker.legacy_type_from_outcome(outcome)
    }

    fn infer_addr_expr_outcome(&mut self, node: &SyntaxNode) -> SemanticOutcome<TypeId> {
        let operand = match node.children().next() {
            Some(child) => child,
            None => return self.checker.unknown_type_outcome(node.text_range()),
        };

        if !self.checker.is_valid_lvalue(&operand) {
            return self.checker.diagnostic_type_outcome(
                DiagnosticCode::InvalidOperation,
                operand.text_range(),
                "ADR expects an assignable operand",
            );
        }
        if self.checker.is_constant_target(&operand) {
            return self.checker.diagnostic_type_outcome(
                DiagnosticCode::InvalidOperation,
                operand.text_range(),
                "ADR cannot take the address of a constant",
            );
        }

        let operand = self.checker.expr().check_expression(&operand);

        // Pointer model: pointers are typed and non-arithmetic. ADR produces a
        // `POINTER TO <operand_type>`. Dereference (`^`) is a valid lvalue and
        // may be assigned to regardless of the parameter direction of the
        // pointer variable itself; only the pointer slot is direction-bound.
        // `ARRAY[*]` is the only bound-agnostic form; implicit bound widening
        // between concrete arrays is intentionally not supported.
        if operand == TypeId::UNKNOWN {
            return self
                .checker
                .suppressed_type_outcome(DiagnosticCode::CannotResolve, node.text_range());
        }

        SemanticOutcome::Resolved(self.checker.symbols.register_pointer_type(operand))
    }

    fn check_array_index_bounds(
        &mut self,
        expr: &SyntaxNode,
        idx_type: TypeId,
        lower: i64,
        upper: i64,
    ) {
        if let Some(value_int) = self.checker.eval_const_int_expr_or_report(expr) {
            if value_int < lower || value_int > upper {
                self.checker.diagnostics.error(
                    DiagnosticCode::OutOfRange,
                    expr.text_range(),
                    format!(
                        "array index {} outside bounds {}..{}",
                        value_int, lower, upper
                    ),
                );
                return;
            }
        }

        if let Some((_, idx_lower, idx_upper)) = self.checker.subrange_bounds(idx_type) {
            if idx_lower < lower || idx_upper > upper {
                self.checker.diagnostics.error(
                    DiagnosticCode::OutOfRange,
                    expr.text_range(),
                    format!(
                        "array index subrange {}..{} outside bounds {}..{}",
                        idx_lower, idx_upper, lower, upper
                    ),
                );
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum PartialAccess {
    Bit(u8),
    Byte(u8),
    Word(u8),
    DWord(u8),
}

impl PartialAccess {
    fn index(self) -> u8 {
        match self {
            Self::Bit(idx) | Self::Byte(idx) | Self::Word(idx) | Self::DWord(idx) => idx,
        }
    }
}

fn parse_partial_access(text: &str) -> Option<PartialAccess> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(stripped) = trimmed.strip_prefix('%') {
        let mut chars = stripped.chars();
        let prefix = chars.next()?;
        let digits: String = chars.collect();
        let index = parse_access_index(&digits)?;
        return match prefix.to_ascii_uppercase() {
            'X' => Some(PartialAccess::Bit(index)),
            'B' => Some(PartialAccess::Byte(index)),
            'W' => Some(PartialAccess::Word(index)),
            'D' => Some(PartialAccess::DWord(index)),
            _ => None,
        };
    }
    if trimmed.chars().all(|c| c.is_ascii_digit() || c == '_') {
        let index = parse_access_index(trimmed)?;
        return Some(PartialAccess::Bit(index));
    }
    None
}

fn parse_access_index(text: &str) -> Option<u8> {
    let cleaned: String = text.chars().filter(|c| *c != '_').collect();
    let value: u64 = cleaned.parse().ok()?;
    u8::try_from(value).ok()
}
