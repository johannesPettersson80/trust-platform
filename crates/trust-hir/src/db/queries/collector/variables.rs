use super::*;
use crate::db::diagnostics::is_expression_kind;
use crate::db::queries::collector::const_utils::{scope_chain_for_node, ConstEvalError};
use crate::semantic::LEGACY_UNKNOWN_TYPE_ID;

impl SymbolCollector<'_> {
    pub(super) fn extract_var_decl_info(
        &mut self,
        node: &SyntaxNode,
    ) -> (Vec<(SmolStr, TextRange)>, TypeId, Option<SmolStr>) {
        let mut names = Vec::new();
        let mut type_id = TypeId::UNKNOWN;

        for child in node.children() {
            match child.kind() {
                SyntaxKind::Name => {
                    if let Some((name, range)) = name_from_node(&child) {
                        names.push((name, range));
                    }
                }
                SyntaxKind::TypeRef => {
                    type_id = self.resolve_type_from_ref(&child);
                }
                _ => {}
            }
        }

        let direct_address = var_decl_direct_address(node);

        (names, type_id, direct_address)
    }

    pub(super) fn collect_var_block(&mut self, node: &SyntaxNode) {
        let qualifier = var_qualifier_from_block(node);
        let is_constant = var_block_is_constant(node);
        let visibility = match qualifier {
            VarQualifier::Input | VarQualifier::Output => Visibility::Public,
            _ => self.visibility_for_var_block(node),
        };
        let use_global_scope = qualifier == VarQualifier::Global && self.in_configuration_scope();
        let previous_scope = self.table.current_scope();
        if use_global_scope {
            self.table.set_current_scope(ScopeId::GLOBAL);
        }
        for child in node.children() {
            if child.kind() == SyntaxKind::VarDecl {
                self.collect_var_decl(&child, qualifier, is_constant, visibility);
            }
        }
        if use_global_scope {
            self.table.set_current_scope(previous_scope);
        }
    }

    pub(super) fn collect_var_decl(
        &mut self,
        node: &SyntaxNode,
        qualifier: VarQualifier,
        is_constant: bool,
        visibility: Visibility,
    ) {
        let mut names = Vec::new();
        let mut type_ref = None;
        for child in node.children() {
            match child.kind() {
                SyntaxKind::Name => names.push(child),
                SyntaxKind::TypeRef => {
                    type_ref = Some(child);
                    break;
                }
                _ => {}
            }
        }

        let type_id = if let Some(type_ref) = type_ref.as_ref() {
            self.resolve_type_from_ref(type_ref)
        } else {
            TypeId::UNKNOWN
        };

        if let Some(expr) = node.children().find(|n| is_expression_kind(n.kind())) {
            self.check_string_initializer(type_id, &expr);
            self.check_aggregate_initializer_fields(type_id, &expr);
        }

        let direct_address = var_decl_direct_address(node);

        for name_node in names {
            if let Some((name, range)) = name_from_node(&name_node) {
                if is_constant
                    && matches!(
                        self.table
                            .type_by_id(self.table.resolve_alias_type(type_id)),
                        Some(Type::FunctionBlock { .. })
                    )
                {
                    self.diagnostics.error(
                        DiagnosticCode::InvalidOperation,
                        range,
                        "function block instances shall not be declared in CONSTANT sections",
                    );
                }

                let kind = match qualifier {
                    VarQualifier::Input => SymbolKind::Parameter {
                        direction: ParamDirection::In,
                    },
                    VarQualifier::Output => SymbolKind::Parameter {
                        direction: ParamDirection::Out,
                    },
                    VarQualifier::InOut => SymbolKind::Parameter {
                        direction: ParamDirection::InOut,
                    },
                    VarQualifier::Temp => SymbolKind::Variable { qualifier },
                    _ if is_constant => SymbolKind::Constant,
                    _ => SymbolKind::Variable { qualifier },
                };
                let mut symbol = Symbol::new(SymbolId::UNKNOWN, name, kind, type_id, range);
                symbol.is_constant = is_constant;
                symbol.direct_address = direct_address.clone();
                symbol.parent = self.current_parent();
                symbol.visibility = visibility;
                self.declare_symbol(symbol);
            }
        }
    }

    pub(super) fn check_string_initializer(&mut self, type_id: TypeId, expr: &SyntaxNode) {
        let Some(literal) = string_literal_info(expr) else {
            return;
        };
        let resolved = self.table.resolve_alias_type(type_id);
        match self.table.type_by_id(resolved) {
            Some(Type::String {
                max_len: Some(max_len),
            }) if !literal.is_wide && literal.len > *max_len => {
                let type_name = self
                    .table
                    .type_name(resolved)
                    .unwrap_or_else(|| "STRING".into());
                self.diagnostics.error(
                    DiagnosticCode::OutOfRange,
                    expr.text_range(),
                    format!(
                        "STRING literal length {} exceeds {} capacity",
                        literal.len, type_name
                    ),
                );
            }
            Some(Type::WString {
                max_len: Some(max_len),
            }) if literal.is_wide && literal.len > *max_len => {
                let type_name = self
                    .table
                    .type_name(resolved)
                    .unwrap_or_else(|| "WSTRING".into());
                self.diagnostics.error(
                    DiagnosticCode::OutOfRange,
                    expr.text_range(),
                    format!(
                        "WSTRING literal length {} exceeds {} capacity",
                        literal.len, type_name
                    ),
                );
            }
            _ => {}
        }
    }

    pub(super) fn check_aggregate_initializer_fields(
        &mut self,
        type_id: TypeId,
        expr: &SyntaxNode,
    ) {
        let resolved = self.table.resolve_alias_type(type_id);
        if expr.kind() == SyntaxKind::CallExpr {
            if matches!(self.table.type_by_id(resolved), Some(Type::Class { .. })) {
                self.diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    expr.text_range(),
                    "class types do not support aggregate initialization; use NEW T(...) for class instantiation",
                );
            }
            return;
        }
        if expr.kind() != SyntaxKind::InitializerList {
            return;
        }

        enum AggregateTarget {
            Members(Vec<(SmolStr, TypeId)>),
            FunctionBlock(TypeId),
        }

        let target = match self.table.type_by_id(resolved) {
            Some(Type::Struct { fields, .. }) => AggregateTarget::Members(
                fields
                    .iter()
                    .map(|field| (field.name.clone(), field.type_id))
                    .collect::<Vec<_>>(),
            ),
            Some(Type::Union { variants, .. }) => AggregateTarget::Members(
                variants
                    .iter()
                    .map(|variant| (variant.name.clone(), variant.type_id))
                    .collect::<Vec<_>>(),
            ),
            Some(Type::FunctionBlock { .. }) => AggregateTarget::FunctionBlock(resolved),
            Some(Type::Unknown) | None => return,
            _ => {
                self.diagnostics.error(
                    DiagnosticCode::TypeMismatch,
                    expr.text_range(),
                    "aggregate initializer requires a STRUCT, UNION, or function block type",
                );
                return;
            }
        };

        let mut seen = FxHashSet::default();
        let mut children = expr.children().peekable();
        while let Some(child) = children.next() {
            if child.kind() != SyntaxKind::Name {
                continue;
            }
            let Some((field_name, range)) = name_from_node(&child) else {
                continue;
            };
            let normalized = SmolStr::new(field_name.to_ascii_uppercase());
            let member_type = match &target {
                AggregateTarget::Members(members) => {
                    let member_type = members
                        .iter()
                        .find(|(name, _)| name.eq_ignore_ascii_case(field_name.as_str()))
                        .map(|(_, member_type)| *member_type);
                    if member_type.is_none() {
                        self.diagnostics.error(
                            DiagnosticCode::UndefinedField,
                            range,
                            format!("unknown aggregate field '{field_name}'"),
                        );
                    }
                    member_type
                }
                AggregateTarget::FunctionBlock(type_id) => {
                    self.function_block_initializer_member_type(*type_id, &field_name, range)
                }
            };
            if member_type.is_none() {
                continue;
            }
            if !seen.insert(normalized) {
                self.diagnostics.error(
                    DiagnosticCode::DuplicateField,
                    range,
                    format!("duplicate aggregate field '{field_name}'"),
                );
            }

            let Some(value_node) = children.find(|candidate| is_expression_kind(candidate.kind()))
            else {
                continue;
            };
            if let Some(member_type) = member_type {
                self.check_aggregate_initializer_fields(member_type, &value_node);
            }
        }
    }

    pub(super) fn check_required_default_expression(&mut self, type_id: TypeId, expr: &SyntaxNode) {
        if type_id == TypeId::UNKNOWN {
            return;
        }
        let resolved = self.table.resolve_alias_type(type_id);
        match expr.kind() {
            SyntaxKind::InitializerList => {
                self.check_required_aggregate_default(resolved, expr);
            }
            SyntaxKind::ArrayInitializer => {
                let Some(Type::Array { element, .. }) = self.table.type_by_id(resolved) else {
                    return;
                };
                let element = *element;
                for child in expr
                    .children()
                    .filter(|child| is_expression_kind(child.kind()))
                {
                    self.check_array_default_element(element, &child);
                }
            }
            SyntaxKind::CallExpr if self.is_array_repeat_expr(expr) => {
                let Some(Type::Array { element, .. }) = self.table.type_by_id(resolved) else {
                    self.check_required_scalar_default(resolved, expr);
                    return;
                };
                self.check_array_default_element(*element, expr);
            }
            _ if matches!(self.table.type_by_id(resolved), Some(Type::Array { .. })) => {
                self.diagnostics.error(
                    DiagnosticCode::TypeMismatch,
                    expr.text_range(),
                    "array default initializer requires an array initializer or repetition expression",
                );
            }
            _ => self.check_required_scalar_default(resolved, expr),
        }
    }

    fn check_required_aggregate_default(&mut self, type_id: TypeId, expr: &SyntaxNode) {
        let members = match self.table.type_by_id(type_id) {
            Some(Type::Struct { fields, .. }) => fields
                .iter()
                .map(|field| (field.name.clone(), field.type_id))
                .collect::<Vec<_>>(),
            Some(Type::Union { variants, .. }) => variants
                .iter()
                .map(|variant| (variant.name.clone(), variant.type_id))
                .collect::<Vec<_>>(),
            _ => return,
        };

        let mut children = expr.children().peekable();
        while let Some(child) = children.next() {
            if child.kind() != SyntaxKind::Name {
                continue;
            }
            let Some((field_name, _)) = name_from_node(&child) else {
                continue;
            };
            let Some((_, member_type)) = members
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(field_name.as_str()))
            else {
                continue;
            };
            let Some(value_node) = children.find(|candidate| is_expression_kind(candidate.kind()))
            else {
                continue;
            };
            self.check_required_default_expression(*member_type, &value_node);
        }
    }

    fn check_array_default_element(&mut self, element_type: TypeId, expr: &SyntaxNode) {
        if self.is_array_repeat_expr(expr) {
            for arg in expr
                .descendants()
                .filter(|node| node.kind() == SyntaxKind::Arg)
            {
                for value in arg
                    .children()
                    .filter(|node| is_expression_kind(node.kind()))
                {
                    self.check_required_default_expression(element_type, &value);
                }
            }
            return;
        }
        self.check_required_default_expression(element_type, expr);
    }

    fn is_array_repeat_expr(&self, expr: &SyntaxNode) -> bool {
        if expr.kind() != SyntaxKind::CallExpr {
            return false;
        }
        expr.children()
            .next()
            .is_some_and(|child| child.kind() == SyntaxKind::Literal)
    }

    fn check_required_scalar_default(&mut self, type_id: TypeId, expr: &SyntaxNode) {
        let Some(target) = self.table.type_by_id(type_id) else {
            return;
        };
        match target {
            Type::Bool if self.literal_is_integer(expr) => {
                self.diagnostics.error(
                    DiagnosticCode::TypeMismatch,
                    expr.text_range(),
                    "BOOL default initializer requires a Boolean value",
                );
            }
            Type::Reference { .. } if !self.literal_is_null(expr) => {
                self.diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    expr.text_range(),
                    "reference type/member defaults must be NULL",
                );
            }
            ty if ty.is_integer() => {
                let scopes = scope_chain_for_node(expr);
                let mut guard = FxHashSet::default();
                match self.try_eval_int_expr(expr, &scopes, &mut guard) {
                    Ok(value) => self.check_integer_default_range(type_id, value, expr),
                    Err(err) => self.report_default_const_eval_error(err, expr.text_range()),
                }
            }
            _ => {}
        }
    }

    fn literal_is_integer(&self, expr: &SyntaxNode) -> bool {
        expr.kind() == SyntaxKind::Literal
            && expr
                .descendants_with_tokens()
                .filter_map(|element| element.into_token())
                .any(|token| token.kind() == SyntaxKind::IntLiteral)
    }

    fn literal_is_null(&self, expr: &SyntaxNode) -> bool {
        expr.kind() == SyntaxKind::Literal
            && expr
                .descendants_with_tokens()
                .filter_map(|element| element.into_token())
                .any(|token| token.kind() == SyntaxKind::KwNull)
    }

    fn check_integer_default_range(&mut self, type_id: TypeId, value: i64, expr: &SyntaxNode) {
        let Some((lower, upper)) = self.integer_type_bounds(type_id) else {
            return;
        };
        if value < lower || value > upper {
            self.diagnostics.error(
                DiagnosticCode::OutOfRange,
                expr.text_range(),
                format!("integer default {value} is outside target type range {lower}..{upper}"),
            );
        }
    }

    fn integer_type_bounds(&self, type_id: TypeId) -> Option<(i64, i64)> {
        let resolved = self.table.resolve_alias_type(type_id);
        match self.table.type_by_id(resolved)? {
            Type::Subrange { lower, upper, .. } => Some((*lower, *upper)),
            Type::SInt => Some((i64::from(i8::MIN), i64::from(i8::MAX))),
            Type::Int => Some((i64::from(i16::MIN), i64::from(i16::MAX))),
            Type::DInt => Some((i64::from(i32::MIN), i64::from(i32::MAX))),
            Type::USInt => Some((0, i64::from(u8::MAX))),
            Type::UInt => Some((0, i64::from(u16::MAX))),
            Type::UDInt => Some((0, i64::from(u32::MAX))),
            Type::ULInt => Some((0, i64::MAX)),
            _ => None,
        }
    }

    fn report_default_const_eval_error(&mut self, err: ConstEvalError, range: TextRange) {
        match err {
            ConstEvalError::CyclicDependency(name) => self.diagnostics.error(
                DiagnosticCode::CyclicDependency,
                range,
                format!("cyclic constant/default reference involving '{name}'"),
            ),
            ConstEvalError::DivideByZero => self.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                range,
                "default constant expression divides by zero",
            ),
            ConstEvalError::IntegerOverflow => self.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                range,
                "default constant expression overflows",
            ),
            ConstEvalError::NegativeExponent => self.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                range,
                "integer exponent must be non-negative",
            ),
            ConstEvalError::UndefinedName(name) => self.diagnostics.error(
                DiagnosticCode::UndefinedVariable,
                range,
                format!("undefined constant '{name}'"),
            ),
            ConstEvalError::AmbiguousName(name) => self.diagnostics.error(
                DiagnosticCode::CannotResolve,
                range,
                format!("ambiguous enum value '{name}'"),
            ),
            ConstEvalError::NotConstant => self.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                range,
                "type/member default initializer must be a constant expression",
            ),
        }
    }

    fn function_block_initializer_member_type(
        &mut self,
        type_id: TypeId,
        field_name: &SmolStr,
        range: TextRange,
    ) -> Option<TypeId> {
        let Some(symbol_id) = self
            .table
            .resolve_member_symbol_in_type(type_id, field_name.as_str())
        else {
            self.diagnostics.error(
                DiagnosticCode::UndefinedField,
                range,
                format!("unknown aggregate field '{field_name}'"),
            );
            return None;
        };
        let symbol = self.table.get(symbol_id)?;
        let kind = symbol.kind.clone();
        let symbol_type_id = symbol.type_id;
        let visibility = symbol.visibility;
        match kind {
            SymbolKind::Parameter {
                direction: ParamDirection::In | ParamDirection::Out,
            } => Some(symbol_type_id),
            SymbolKind::Parameter {
                direction: ParamDirection::InOut,
            } => {
                self.diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    range,
                    "VAR_IN_OUT members cannot be initialized through aggregate syntax",
                );
                None
            }
            SymbolKind::Variable {
                qualifier: VarQualifier::Temp | VarQualifier::External | VarQualifier::InOut,
            } => {
                self.diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    range,
                    "temporary and external members cannot be initialized through aggregate syntax",
                );
                None
            }
            SymbolKind::Variable { .. } if visibility == Visibility::Public => Some(symbol_type_id),
            SymbolKind::Variable { .. } => {
                self.diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    range,
                    "private members cannot be initialized through aggregate syntax",
                );
                None
            }
            _ => {
                self.diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    range,
                    "member cannot be initialized through aggregate syntax",
                );
                None
            }
        }
    }

    pub(super) fn collect_var_access_block(&mut self, node: &SyntaxNode) {
        let use_global_scope = self.in_configuration_scope();
        let previous_scope = self.table.current_scope();
        if use_global_scope {
            self.table.set_current_scope(ScopeId::GLOBAL);
        }
        for access_decl in node
            .children()
            .filter(|n| n.kind() == SyntaxKind::AccessDecl)
        {
            let Some((name, range)) = access_decl
                .children()
                .find(|n| n.kind() == SyntaxKind::Name)
                .and_then(|n| name_from_node(&n))
            else {
                continue;
            };
            let type_id = access_decl
                .children()
                .find(|n| n.kind() == SyntaxKind::TypeRef)
                .map(|n| self.resolve_type_from_ref(&n))
                .unwrap_or(LEGACY_UNKNOWN_TYPE_ID);

            let mode = access_decl_mode(&access_decl);
            let kind = match mode {
                AccessMode::ReadOnly => SymbolKind::Constant,
                AccessMode::ReadWrite => SymbolKind::Variable {
                    qualifier: VarQualifier::Access,
                },
            };

            let mut symbol = Symbol::new(SymbolId::UNKNOWN, name, kind, type_id, range);
            symbol.parent = self.current_parent();
            self.declare_symbol(symbol);
        }
        if use_global_scope {
            self.table.set_current_scope(previous_scope);
        }
    }

    pub(super) fn collect_var_config_block(&mut self, node: &SyntaxNode) {
        if !self.in_configuration_scope() {
            self.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                node.text_range(),
                "VAR_CONFIG is only valid inside CONFIGURATION or RESOURCE declarations",
            );
        }

        for config_init in node
            .children()
            .filter(|n| n.kind() == SyntaxKind::ConfigInit)
        {
            if config_init
                .children()
                .all(|n| n.kind() != SyntaxKind::AccessPath)
            {
                self.diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    config_init.text_range(),
                    "VAR_CONFIG entry requires a target access path",
                );
            }

            if config_init
                .children()
                .all(|n| n.kind() != SyntaxKind::TypeRef)
            {
                self.diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    config_init.text_range(),
                    "VAR_CONFIG entry requires a declared type",
                );
            }
        }
    }

    fn in_configuration_scope(&self) -> bool {
        self.table
            .get_scope(self.table.current_scope())
            .is_some_and(|scope| {
                matches!(scope.kind, ScopeKind::Configuration | ScopeKind::Resource)
            })
    }
}
