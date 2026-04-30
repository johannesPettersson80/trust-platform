//! Expression type inference for IEC 61131-3 Structured Text.
//!
//! This module provides type checking and inference for expressions and statements.

use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;
use text_size::TextRange;

use crate::diagnostics::{DiagnosticBuilder, DiagnosticCode};
use crate::semantic::{SemanticOutcome, LEGACY_UNKNOWN_TYPE_ID};
use crate::symbols::{
    ParamDirection, ScopeId, ScopeKind, SymbolId, SymbolKind, SymbolTable, UsingResolution,
    Visibility,
};
use crate::types::{Type, TypeId};
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

mod calls;
mod compatibility;
mod const_eval;
mod expr;
mod helpers;
mod literals;
mod ops;
mod standard;
mod stmt;
mod symbol_resolve;
mod validation;

pub(crate) use literals::string_literal_info;
pub use ops::{BinaryOp, UnaryOp};

fn non_value_role(kind: &SymbolKind) -> Option<&'static str> {
    match kind {
        SymbolKind::Type | SymbolKind::Class | SymbolKind::Interface => Some("type"),
        SymbolKind::FunctionBlock => Some("function block type"),
        SymbolKind::Function { .. } => Some("function"),
        SymbolKind::Method { .. } => Some("method"),
        SymbolKind::Namespace => Some("namespace"),
        SymbolKind::Program => Some("program"),
        SymbolKind::Configuration => Some("configuration"),
        SymbolKind::Resource => Some("resource"),
        SymbolKind::Task => Some("task"),
        SymbolKind::ProgramInstance => Some("program instance"),
        SymbolKind::Variable { .. }
        | SymbolKind::Constant
        | SymbolKind::EnumValue { .. }
        | SymbolKind::Parameter { .. }
        | SymbolKind::Property { .. } => None,
    }
}

/// Type checker for expressions and statements.
pub struct TypeChecker<'a> {
    symbols: &'a mut SymbolTable,
    diagnostics: &'a mut DiagnosticBuilder,
    current_scope: ScopeId,
    /// The expected return type of the current function (None for procedures/programs).
    current_function_return: Option<TypeId>,
    current_pou_symbol: Option<SymbolId>,
    saw_return_value: bool,
    return_value_definitely_assigned: bool,
    this_type: Option<TypeId>,
    super_type: Option<TypeId>,
    loop_stack: Vec<LoopContext>,
    label_scopes: Vec<LabelScope>,
    expression_types: FxHashMap<(u32, u32), TypeId>,
}

pub(crate) struct ExprChecker<'a, 'b> {
    checker: &'b mut TypeChecker<'a>,
}

pub(crate) struct StmtChecker<'a, 'b> {
    checker: &'b mut TypeChecker<'a>,
}

pub(crate) struct CallChecker<'a, 'b> {
    checker: &'b mut TypeChecker<'a>,
}

pub(crate) struct StandardChecker<'a, 'b> {
    checker: &'b mut TypeChecker<'a>,
}

pub(crate) struct ResolveChecker<'a, 'b> {
    checker: &'b mut TypeChecker<'a>,
}

pub(crate) struct ResolveCheckerRef<'a, 'b> {
    checker: &'b TypeChecker<'a>,
}

impl<'a> TypeChecker<'a> {
    pub(crate) fn expr(&mut self) -> ExprChecker<'a, '_> {
        ExprChecker { checker: self }
    }

    pub(crate) fn stmt(&mut self) -> StmtChecker<'a, '_> {
        StmtChecker { checker: self }
    }

    pub(crate) fn calls(&mut self) -> CallChecker<'a, '_> {
        CallChecker { checker: self }
    }

    pub(crate) fn standard(&mut self) -> StandardChecker<'a, '_> {
        StandardChecker { checker: self }
    }

    pub(crate) fn resolve(&mut self) -> ResolveChecker<'a, '_> {
        ResolveChecker { checker: self }
    }

    pub(crate) fn resolve_ref(&self) -> ResolveCheckerRef<'a, '_> {
        ResolveCheckerRef { checker: self }
    }

    /// Infers the type of an expression.
    pub fn check_expression(&mut self, node: &SyntaxNode) -> TypeId {
        self.expr().check_expression(node)
    }

    /// Checks a statement for type errors.
    pub fn check_statement(&mut self, node: &SyntaxNode) {
        self.stmt().check_statement(node);
    }

    /// Emits missing return diagnostics after statement checks.
    pub fn finish_return_checks(&mut self, node: &SyntaxNode) {
        self.stmt().finish_return_checks(node);
    }

    pub(crate) fn take_expression_types(&mut self) -> FxHashMap<(u32, u32), TypeId> {
        std::mem::take(&mut self.expression_types)
    }

    fn record_expression_type(&mut self, node: &SyntaxNode, type_id: TypeId) -> TypeId {
        let range = node.text_range();
        self.expression_types
            .insert((u32::from(range.start()), u32::from(range.end())), type_id);
        type_id
    }

    fn legacy_type_from_outcome(&self, outcome: SemanticOutcome<TypeId>) -> TypeId {
        match outcome {
            SemanticOutcome::Resolved(type_id) => type_id,
            SemanticOutcome::Unknown { .. }
            | SemanticOutcome::Ambiguous { .. }
            | SemanticOutcome::WrongKind { .. }
            | SemanticOutcome::SuppressedCascade { .. }
            | SemanticOutcome::InvariantViolation { .. } => LEGACY_UNKNOWN_TYPE_ID,
        }
    }

    fn unknown_type_outcome(&self, range: TextRange) -> SemanticOutcome<TypeId> {
        SemanticOutcome::Unknown {
            name: None,
            range: Some(range),
        }
    }

    fn suppressed_type_outcome(
        &self,
        primary: DiagnosticCode,
        range: TextRange,
    ) -> SemanticOutcome<TypeId> {
        SemanticOutcome::SuppressedCascade {
            primary,
            range: Some(range),
        }
    }

    fn diagnostic_type_outcome(
        &mut self,
        code: DiagnosticCode,
        range: TextRange,
        message: impl Into<String>,
    ) -> SemanticOutcome<TypeId> {
        self.diagnostics.error(code, range, message);
        self.suppressed_type_outcome(code, range)
    }

    fn legacy_suppressed_type(&self, primary: DiagnosticCode, range: TextRange) -> TypeId {
        self.legacy_type_from_outcome(self.suppressed_type_outcome(primary, range))
    }

    fn legacy_diagnostic_type(
        &mut self,
        code: DiagnosticCode,
        range: TextRange,
        message: impl Into<String>,
    ) -> TypeId {
        let outcome = self.diagnostic_type_outcome(code, range, message);
        self.legacy_type_from_outcome(outcome)
    }
}

#[derive(Debug, Clone)]
struct LoopContext {
    restricted: FxHashSet<SymbolId>,
}

#[derive(Debug, Clone)]
struct LabelScope {
    labels: FxHashSet<SmolStr>,
    pending_jumps: Vec<(SmolStr, SmolStr, TextRange)>,
}

#[derive(Debug, Default)]
struct CaseLabelTracker {
    ints: FxHashMap<i64, TextRange>,
    ranges: Vec<(i64, i64)>,
}

impl CaseLabelTracker {
    fn covers(&self, value: i64) -> bool {
        if self.ints.contains_key(&value) {
            return true;
        }
        self.ranges
            .iter()
            .any(|(lower, upper)| value >= *lower && value <= *upper)
    }
}

fn is_expression_kind(kind: SyntaxKind) -> bool {
    kind.is_initializer_expression_node()
}

fn is_statement_kind(kind: SyntaxKind) -> bool {
    kind.is_statement_node()
}

fn first_expression_child(node: &SyntaxNode) -> Option<SyntaxNode> {
    node.children()
        .find(|child| is_expression_kind(child.kind()))
}

fn last_expression_child(node: &SyntaxNode) -> Option<SyntaxNode> {
    node.children()
        .filter(|child| is_expression_kind(child.kind()))
        .last()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary_op_from_node() {
        // Basic test that BinaryOp enum is defined correctly
        assert!(BinaryOp::Add.is_arithmetic());
        assert!(BinaryOp::Eq.is_comparison());
        assert!(BinaryOp::And.is_logical());
    }
}
