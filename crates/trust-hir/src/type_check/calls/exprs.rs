use super::super::*;
use super::*;

impl<'a, 'b> CallChecker<'a, 'b> {
    pub(in crate::type_check) fn infer_index_expr(&mut self, node: &SyntaxNode) -> TypeId {
        let children: Vec<_> = node.children().collect();
        if children.is_empty() {
            return TypeId::UNKNOWN;
        }

        let base_type = self.checker.expr().check_expression(&children[0]);
        let resolved_base = self.checker.resolve_alias_type(base_type);
        let index_count = children.len().saturating_sub(1);
        let mut index_exprs = Vec::new();

        // Check index expression types (must be integers)
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

        if let Some(base_type) = self.checker.symbols.type_by_id(resolved_base) {
            match base_type {
                Type::Array {
                    element,
                    dimensions,
                } => {
                    let element = *element;
                    let dimensions = dimensions.clone();
                    if index_count != dimensions.len() {
                        self.checker.diagnostics.error(
                            DiagnosticCode::InvalidArrayIndex,
                            node.text_range(),
                            format!(
                                "expected {} index value(s), found {}",
                                dimensions.len(),
                                index_count
                            ),
                        );
                        return TypeId::UNKNOWN;
                    }
                    for ((expr, _, idx_type), (lower, upper)) in
                        index_exprs.iter().zip(dimensions.iter())
                    {
                        self.check_array_index_bounds(expr, *idx_type, *lower, *upper);
                    }
                    return element;
                }
                Type::String { max_len } => {
                    if index_count != 1 {
                        self.checker.diagnostics.error(
                            DiagnosticCode::InvalidArrayIndex,
                            node.text_range(),
                            format!("expected 1 index value, found {index_count}"),
                        );
                        return TypeId::UNKNOWN;
                    }
                    if let Some(max_len) = max_len {
                        self.check_array_index_bounds(
                            &index_exprs[0].0,
                            index_exprs[0].2,
                            1,
                            i64::from(*max_len),
                        );
                    }
                    return TypeId::CHAR;
                }
                Type::WString { max_len } => {
                    if index_count != 1 {
                        self.checker.diagnostics.error(
                            DiagnosticCode::InvalidArrayIndex,
                            node.text_range(),
                            format!("expected 1 index value, found {index_count}"),
                        );
                        return TypeId::UNKNOWN;
                    }
                    if let Some(max_len) = max_len {
                        self.check_array_index_bounds(
                            &index_exprs[0].0,
                            index_exprs[0].2,
                            1,
                            i64::from(*max_len),
                        );
                    }
                    return TypeId::WCHAR;
                }
                _ => {}
            }
        }

        self.checker.diagnostics.error(
            DiagnosticCode::TypeMismatch,
            node.text_range(),
            "indexing requires an array, STRING, or WSTRING type",
        );
        TypeId::UNKNOWN
    }

    pub(in crate::type_check) fn infer_field_expr(&mut self, node: &SyntaxNode) -> TypeId {
        let children: Vec<_> = node.children().collect();
        if children.len() < 2 {
            return TypeId::UNKNOWN;
        }

        if let Some(symbol_id) = self
            .checker
            .resolve_ref()
            .resolve_namespace_qualified_symbol(node)
        {
            if let Some(symbol) = self.checker.symbols.get(symbol_id) {
                return symbol.type_id;
            }
        }

        let base = &children[0];
        let member = &children[1];
        let base_type = self.checker.expr().check_expression(base);
        let field_name = self.checker.resolve_ref().get_name_from_ref(member);

        if field_name.is_none() {
            if let Some(ty) = self.infer_partial_bit_access(base_type, member) {
                return ty;
            }
            return TypeId::UNKNOWN;
        }

        let field_name = field_name.unwrap();

        if let Some(ty) = self
            .checker
            .symbols
            .type_by_id(self.checker.resolve_alias_type(base_type))
        {
            match ty {
                Type::Struct { .. } | Type::Union { .. } => {
                    if let Some(field_type) = self
                        .checker
                        .resolve_ref()
                        .resolve_member_in_type(base_type, &field_name)
                    {
                        return field_type;
                    }
                    self.checker.diagnostics.error(
                        DiagnosticCode::CannotResolve,
                        member.text_range(),
                        format!("no field '{}' on struct", field_name),
                    );
                    return TypeId::UNKNOWN;
                }
                Type::FunctionBlock { .. } | Type::Class { .. } | Type::Interface { .. } => {
                    if let Some(resolved) = self.checker.resolve().resolve_member_symbol_in_type(
                        base_type,
                        &field_name,
                        member.text_range(),
                    ) {
                        let Some(symbol) = self.checker.symbols.get(resolved.id) else {
                            return TypeId::UNKNOWN;
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
                        return symbol.type_id;
                    }
                    self.checker.diagnostics.error(
                        DiagnosticCode::CannotResolve,
                        member.text_range(),
                        format!("no member '{}' on type", field_name),
                    );
                    return TypeId::UNKNOWN;
                }
                _ => {
                    self.checker.diagnostics.error(
                        DiagnosticCode::TypeMismatch,
                        node.text_range(),
                        "field access requires struct, function block, or class type",
                    );
                    return TypeId::UNKNOWN;
                }
            }
        }

        TypeId::UNKNOWN
    }

    fn infer_partial_bit_access(
        &mut self,
        base_type: TypeId,
        member: &SyntaxNode,
    ) -> Option<TypeId> {
        let access = parse_partial_access(member.text().to_string().trim())?;
        let resolved = self.checker.resolve_alias_type(base_type);
        let ty = self.checker.symbols.type_by_id(resolved)?;
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
            self.checker.diagnostics.error(
                DiagnosticCode::OutOfRange,
                member.text_range(),
                "partial access index out of range",
            );
            return Some(TypeId::UNKNOWN);
        }
        Some(result)
    }

    pub(in crate::type_check) fn infer_deref_expr(&mut self, node: &SyntaxNode) -> TypeId {
        let operand = match node.children().next() {
            Some(child) => self.checker.expr().check_expression(&child),
            None => return TypeId::UNKNOWN,
        };

        let operand = self.checker.resolve_alias_type(operand);
        if let Some(Type::Pointer { target } | Type::Reference { target }) =
            self.checker.symbols.type_by_id(operand)
        {
            return *target;
        }

        self.checker.diagnostics.error(
            DiagnosticCode::TypeMismatch,
            node.text_range(),
            "dereference requires pointer type",
        );
        TypeId::UNKNOWN
    }

    pub(in crate::type_check) fn infer_addr_expr(&mut self, node: &SyntaxNode) -> TypeId {
        let operand = match node.children().next() {
            Some(child) => child,
            None => return TypeId::UNKNOWN,
        };

        if !self.checker.is_valid_lvalue(&operand) {
            self.checker.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                operand.text_range(),
                "ADR expects an assignable operand",
            );
            return TypeId::UNKNOWN;
        }
        if self.checker.is_constant_target(&operand) {
            self.checker.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                operand.text_range(),
                "ADR cannot take the address of a constant",
            );
            return TypeId::UNKNOWN;
        }

        let operand = self.checker.expr().check_expression(&operand);

        // Pointer model: pointers are typed and non-arithmetic. ADR produces a
        // `POINTER TO <operand_type>`. Dereference (`^`) is a valid lvalue and
        // may be assigned to regardless of the parameter direction of the
        // pointer variable itself; only the pointer slot is direction-bound.
        // `ARRAY[*]` is the only bound-agnostic form; implicit bound widening
        // between concrete arrays is intentionally not supported.
        if operand == TypeId::UNKNOWN {
            return TypeId::UNKNOWN;
        }

        self.checker.symbols.register_pointer_type(operand)
    }

    fn check_array_index_bounds(
        &mut self,
        expr: &SyntaxNode,
        idx_type: TypeId,
        lower: i64,
        upper: i64,
    ) {
        if let Some(value_int) = self.checker.eval_const_int_expr(expr) {
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
