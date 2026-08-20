use super::*;
use crate::db::diagnostics::is_expression_kind;
use crate::db::queries::collector::const_utils::{
    qualified_const_parts, scope_chain_for_node, ConstEvalError,
};
use crate::symbols::{EnumValueResolution, UsingResolution};
use crate::type_check::{is_standard_function_name, TypeChecker};

impl SymbolCollector<'_> {
    pub(super) fn check_variable_initializer_constant_expressions(&mut self, root: &SyntaxNode) {
        for block in root
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::VarBlock)
        {
            let is_constant_block = var_block_is_constant(&block);
            for declaration in block
                .children()
                .filter(|node| node.kind() == SyntaxKind::VarDecl)
            {
                let Some(name_node) = declaration
                    .children()
                    .find(|node| node.kind() == SyntaxKind::Name)
                else {
                    continue;
                };
                let Some((name, range)) = name_from_node(&name_node) else {
                    continue;
                };
                let Some(symbol_id) = self.table.lookup_by_name_range(name.as_str(), range) else {
                    continue;
                };
                let Some(symbol) = self.table.get(symbol_id) else {
                    continue;
                };
                let type_id = symbol.type_id;
                let scope = symbol
                    .parent
                    .and_then(|parent| self.table.scope_for_owner(parent))
                    .unwrap_or(ScopeId::GLOBAL);
                let Some(expr) = declaration
                    .children()
                    .find(|node| is_expression_kind(node.kind()))
                else {
                    continue;
                };
                self.check_variable_initializer_constant_expression(
                    type_id,
                    &expr,
                    scope,
                    is_constant_block,
                );
            }
        }
    }

    fn check_variable_initializer_constant_expression(
        &mut self,
        type_id: TypeId,
        expr: &SyntaxNode,
        scope: ScopeId,
        is_constant_block: bool,
    ) {
        let resolved = self.table.resolve_alias_type(type_id);
        let Some(target) = self.table.type_by_id(resolved) else {
            return;
        };
        if matches!(target, Type::Reference { .. }) && is_unqualified_ref_call(expr) {
            self.check_reference_variable_initializer(type_id, expr, scope);
            return;
        }
        if !target.is_integer() {
            if let Some(violation) = self.initializer_const_violation(expr, scope) {
                let message = match violation {
                    InitializerConstViolation::Mutable(name) => format!(
                        "variable initializer must be a literal or constant expression: mutable dependency '{name}'"
                    ),
                    InitializerConstViolation::NonConstant => {
                        "variable initializer must be a literal or constant expression".to_string()
                    }
                    InitializerConstViolation::Undefined(name) => {
                        self.diagnostics.error(
                            DiagnosticCode::UndefinedVariable,
                            expr.text_range(),
                            format!("undefined identifier '{name}'"),
                        );
                        return;
                    }
                    InitializerConstViolation::UndefinedCallable(name) => {
                        self.diagnostics.error(
                            DiagnosticCode::UndefinedFunction,
                            expr.text_range(),
                            format!("undefined function '{name}'"),
                        );
                        return;
                    }
                    InitializerConstViolation::Ambiguous(name) => {
                        self.diagnostics.error(
                            DiagnosticCode::CannotResolve,
                            expr.text_range(),
                            format!("ambiguous initializer reference '{name}'"),
                        );
                        return;
                    }
                };
                self.diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    expr.text_range(),
                    message,
                );
            }
            return;
        }

        if is_constant_block {
            return;
        }

        let scopes = scope_chain_for_node(expr);
        let mut guard = FxHashSet::default();
        match self.try_eval_int_expr(expr, &scopes, &mut guard) {
            Ok(_) => {}
            Err(ConstEvalError::UndefinedName(name))
                if self.table.resolve(name.as_str(), scope).is_some() =>
            {
                self.diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    expr.text_range(),
                    format!(
                        "variable initializer must be a literal or constant expression: mutable dependency '{name}'"
                    ),
                );
            }
            Err(ConstEvalError::NotConstant) => {
                self.diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    expr.text_range(),
                    "variable initializer must be a literal or constant expression",
                );
            }
            Err(err) => self.report_default_const_eval_error(err, expr.text_range()),
        }
    }

    fn check_reference_variable_initializer(
        &mut self,
        type_id: TypeId,
        expr: &SyntaxNode,
        scope: ScopeId,
    ) {
        // IEC 61131-3 Ed.3 section 6.4.4.10.2 permits a reference
        // declaration to initialize from REF(existing storage). Reuse the
        // canonical expression checker so REF lvalue, visibility, lifetime,
        // and target-type rules remain owned by one semantic boundary.
        let current_pou = self.table.get_scope(scope).and_then(|data| data.owner);
        let (actual_type, compatible) = {
            let mut checker = TypeChecker::new(&mut self.table, &mut self.diagnostics, scope);
            checker.set_current_pou(current_pou);
            let actual_type = checker.check_expression(expr);
            let compatible = checker.is_assignable(type_id, actual_type);
            (actual_type, compatible)
        };
        if actual_type != TypeId::UNKNOWN && !compatible {
            self.diagnostics.error(
                DiagnosticCode::IncompatibleAssignment,
                expr.text_range(),
                "reference initializer type is incompatible with the declared reference type",
            );
        }
    }

    fn initializer_const_violation(
        &self,
        expr: &SyntaxNode,
        scope: ScopeId,
    ) -> Option<InitializerConstViolation> {
        match expr.kind() {
            SyntaxKind::Literal | SyntaxKind::SizeOfExpr => None,
            SyntaxKind::NameRef | SyntaxKind::FieldExpr => {
                self.initializer_reference_violation(expr, scope, true)
            }
            SyntaxKind::CallExpr => self.initializer_call_violation(expr, scope),
            SyntaxKind::DerefExpr
            | SyntaxKind::AddrExpr
            | SyntaxKind::ThisExpr
            | SyntaxKind::SuperExpr => Some(InitializerConstViolation::NonConstant),
            _ => {
                let children = expr
                    .children()
                    .filter(|child| is_expression_kind(child.kind()))
                    .collect::<Vec<_>>();
                if children.is_empty() {
                    return Some(InitializerConstViolation::NonConstant);
                }
                children
                    .iter()
                    .find_map(|child| self.initializer_const_violation(child, scope))
            }
        }
    }

    fn initializer_call_violation(
        &self,
        expr: &SyntaxNode,
        scope: ScopeId,
    ) -> Option<InitializerConstViolation> {
        let mut children = expr.children();
        let callee = children.next()?;
        let constructor_or_repetition = match callee.kind() {
            SyntaxKind::Literal
            | SyntaxKind::UnaryExpr
            | SyntaxKind::BinaryExpr
            | SyntaxKind::ParenExpr => self.initializer_const_violation(&callee, scope),
            SyntaxKind::NameRef | SyntaxKind::FieldExpr => {
                if self.initializer_reference_is_type(&callee, scope) {
                    None
                } else if qualified_const_parts(&callee).is_some_and(|parts| {
                    parts.len() == 1 && is_standard_function_name(parts[0].as_str())
                }) {
                    Some(InitializerConstViolation::NonConstant)
                } else {
                    match self.initializer_reference_violation(&callee, scope, true) {
                        Some(InitializerConstViolation::Undefined(name)) => {
                            Some(InitializerConstViolation::UndefinedCallable(name))
                        }
                        violation => violation,
                    }
                }
            }
            _ => Some(InitializerConstViolation::NonConstant),
        };
        constructor_or_repetition.or_else(|| {
            expr.children()
                .find(|child| child.kind() == SyntaxKind::ArgList)
                .into_iter()
                .flat_map(|arguments| arguments.children())
                .filter(|child| child.kind() == SyntaxKind::Arg)
                .flat_map(|argument| argument.children().collect::<Vec<_>>())
                .filter(|child| is_expression_kind(child.kind()))
                .find_map(|child| self.initializer_const_violation(&child, scope))
        })
    }

    fn initializer_reference_is_type(&self, node: &SyntaxNode, scope: ScopeId) -> bool {
        let Some(parts) = qualified_const_parts(node) else {
            return false;
        };
        if parts.len() == 1
            && self
                .table
                .lookup_registered_type_name(parts[0].as_str())
                .is_some()
        {
            return true;
        }
        match self.resolve_initializer_reference(&parts, scope) {
            InitializerReferenceResolution::Found(symbol_id) => {
                self.table.get(symbol_id).is_some_and(|symbol| {
                    matches!(symbol.kind, SymbolKind::Type | SymbolKind::FunctionBlock)
                })
            }
            InitializerReferenceResolution::NotFound
            | InitializerReferenceResolution::Ambiguous => false,
        }
    }

    fn initializer_reference_violation(
        &self,
        node: &SyntaxNode,
        scope: ScopeId,
        report_resolution_failure: bool,
    ) -> Option<InitializerConstViolation> {
        let parts = qualified_const_parts(node)?;
        let name = parts.last()?.clone();
        let symbol_id = match self.resolve_initializer_reference(&parts, scope) {
            InitializerReferenceResolution::Found(symbol_id) => symbol_id,
            InitializerReferenceResolution::Ambiguous => {
                return report_resolution_failure
                    .then_some(InitializerConstViolation::Ambiguous(name));
            }
            InitializerReferenceResolution::NotFound => {
                if parts.len() > 1 {
                    if let InitializerReferenceResolution::Found(base_id) =
                        self.resolve_initializer_reference(&parts[..1], scope)
                    {
                        if self.table.get(base_id).is_some_and(|symbol| {
                            matches!(
                                symbol.kind,
                                SymbolKind::Variable { .. }
                                    | SymbolKind::Parameter { .. }
                                    | SymbolKind::Property { .. }
                            )
                        }) {
                            return Some(InitializerConstViolation::Mutable(parts[0].clone()));
                        }
                    }
                }
                return report_resolution_failure
                    .then_some(InitializerConstViolation::Undefined(name));
            }
        };
        let Some(symbol) = self.table.get(symbol_id) else {
            if node.kind() == SyntaxKind::FieldExpr {
                return node
                    .children()
                    .filter(|child| is_expression_kind(child.kind()))
                    .find_map(|child| self.initializer_const_violation(&child, scope));
            }
            return None;
        };
        if parts.len() == 1
            && matches!(symbol.kind, SymbolKind::EnumValue { .. })
            && matches!(
                self.table.resolve_enum_value_by_name(name.as_str()),
                EnumValueResolution::Ambiguous
            )
        {
            return Some(InitializerConstViolation::Ambiguous(name));
        }
        if symbol.is_constant
            || matches!(
                symbol.kind,
                SymbolKind::Constant | SymbolKind::EnumValue { .. }
            )
        {
            None
        } else if matches!(
            symbol.kind,
            SymbolKind::Variable { .. }
                | SymbolKind::Parameter { .. }
                | SymbolKind::Property { .. }
        ) {
            Some(InitializerConstViolation::Mutable(name))
        } else {
            Some(InitializerConstViolation::NonConstant)
        }
    }

    fn resolve_initializer_reference(
        &self,
        parts: &[SmolStr],
        scope: ScopeId,
    ) -> InitializerReferenceResolution {
        if parts.len() > 1 {
            return self.table.resolve_qualified(parts).map_or(
                InitializerReferenceResolution::NotFound,
                InitializerReferenceResolution::Found,
            );
        }
        let Some(name) = parts.first() else {
            return InitializerReferenceResolution::NotFound;
        };
        let mut scope_id = Some(scope);
        while let Some(current) = scope_id {
            let Some(scope_data) = self.table.get_scope(current) else {
                break;
            };
            if let Some(symbol_id) = self.table.lookup_in_scope(current, name.as_str()) {
                return InitializerReferenceResolution::Found(symbol_id);
            }
            if matches!(scope_data.kind, ScopeKind::Class | ScopeKind::FunctionBlock) {
                if let Some(symbol_id) = scope_data.owner.and_then(|owner| {
                    self.table
                        .resolve_member_symbol_in_hierarchy(owner, name.as_str())
                }) {
                    return InitializerReferenceResolution::Found(symbol_id);
                }
            }
            match self.table.resolve_using_in_scope(scope_data, name.as_str()) {
                UsingResolution::Single(symbol_id) => {
                    return InitializerReferenceResolution::Found(symbol_id);
                }
                UsingResolution::Ambiguous => return InitializerReferenceResolution::Ambiguous,
                UsingResolution::None => scope_id = scope_data.parent,
            }
        }
        InitializerReferenceResolution::NotFound
    }
}

enum InitializerConstViolation {
    Mutable(SmolStr),
    NonConstant,
    Undefined(SmolStr),
    UndefinedCallable(SmolStr),
    Ambiguous(SmolStr),
}

enum InitializerReferenceResolution {
    Found(SymbolId),
    NotFound,
    Ambiguous,
}

fn is_unqualified_ref_call(expr: &SyntaxNode) -> bool {
    if expr.kind() != SyntaxKind::CallExpr {
        return false;
    }
    expr.children().next().is_some_and(|callee| {
        callee.kind() == SyntaxKind::NameRef
            && callee
                .descendants_with_tokens()
                .filter_map(|element| element.into_token())
                .any(|token| token.kind() == SyntaxKind::KwRef)
    })
}
