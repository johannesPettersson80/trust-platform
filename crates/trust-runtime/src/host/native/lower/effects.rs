//! What a part of a body can do besides compute (specification 44 §6.5, §6.6): whether evaluating
//! an operand can fault or write, and whether a loop's body can write its control variable. The
//! lowering leaves a check or a read out only where these say that nothing observable changes.

use trust_runtime_core::native::image::SYMBOL_IN_OUT;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::{call, Frame, Lowerer};
use crate::native::types::{Elem, Kind};

/// The operator of the unary or binary expression `node`: its first token that is not trivia.
pub(super) fn operator(node: &SyntaxNode) -> Option<SyntaxKind> {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
        .map(|token| token.kind())
}

impl Lowerer<'_, '_> {
    /// Whether the real `+`, `-` or `*` `node` lies inside a chain of them, through parentheses and
    /// signs, whose operands can neither fault nor write: its result then needs no check of its
    /// own, as a value that is not finite stays not finite through `+ - *` and the sign, and the
    /// chain's last operation checks it in the same statement, with nothing written or called in
    /// between (§6.5).
    pub(super) fn inside_pure_chain(&mut self, node: &SyntaxNode) -> bool {
        let mut current = node.clone();
        let mut root = None;
        while let Some(parent) = current.parent() {
            match parent.kind() {
                SyntaxKind::ParenExpr => {}
                SyntaxKind::UnaryExpr if self.real_sign(&parent) => {}
                SyntaxKind::BinaryExpr if self.real_chain_operation(&parent) => {
                    root = Some(parent.clone());
                }
                _ => break,
            }
            current = parent;
        }
        root.is_some_and(|root| self.pure_operand(&root))
    }

    /// Whether `node` is a real `+`, `-` or `*`.
    fn real_chain_operation(&mut self, node: &SyntaxNode) -> bool {
        node.kind() == SyntaxKind::BinaryExpr
            && matches!(
                operator(node),
                Some(SyntaxKind::Plus | SyntaxKind::Minus | SyntaxKind::Star)
            )
            && self.is_real(node)
    }

    /// Whether `node` is the sign of a real: `-x` or `+x`.
    fn real_sign(&mut self, node: &SyntaxNode) -> bool {
        matches!(operator(node), Some(SyntaxKind::Minus | SyntaxKind::Plus)) && self.is_real(node)
    }

    fn is_real(&mut self, node: &SyntaxNode) -> bool {
        self.type_of(node)
            .is_ok_and(|ty| self.info(ty).scalar().is_some_and(Elem::is_real))
    }

    /// Whether evaluating `node`, an operand in a chain of real `+ - *`, can neither fault nor
    /// write: a literal, a variable read where it lies, the sign of a real, or a real `+ - *` of
    /// such operands.
    fn pure_operand(&mut self, node: &SyntaxNode) -> bool {
        match node.kind() {
            SyntaxKind::Literal => true,
            SyntaxKind::ParenExpr => node
                .children()
                .next()
                .is_some_and(|inner| self.pure_operand(&inner)),
            SyntaxKind::UnaryExpr => {
                super::literal::signed_literal(node)
                    || (self.real_sign(node)
                        && node
                            .children()
                            .next()
                            .is_some_and(|operand| self.pure_operand(&operand)))
            }
            SyntaxKind::BinaryExpr => {
                self.real_chain_operation(node)
                    && node
                        .children()
                        .take(2)
                        .all(|operand| self.pure_operand(&operand))
            }
            SyntaxKind::NameRef | SyntaxKind::FieldExpr | SyntaxKind::IndexExpr => {
                self.pure_place(node)
            }
            _ => false,
        }
    }

    /// Whether `node` names a variable read where it lies: not a property, whose `GET` is a call,
    /// not through a dereference, and indexed by constants inside the bounds only.
    fn pure_place(&mut self, node: &SyntaxNode) -> bool {
        if self.property_name(node).is_some() {
            return false;
        }
        match node.kind() {
            SyntaxKind::ParenExpr => node
                .children()
                .next()
                .is_some_and(|inner| self.pure_place(&inner)),
            SyntaxKind::NameRef => self.direct_variable(&call::ident(node)),
            SyntaxKind::FieldExpr => {
                !self.namespace_qualified(node)
                    && node
                        .children()
                        .next()
                        .is_some_and(|base| self.pure_place(&base))
            }
            SyntaxKind::IndexExpr => {
                let mut children = node.children();
                let Some(base) = children.next() else {
                    return false;
                };
                let Ok(ty) = self.type_of(&base) else {
                    return false;
                };
                let Kind::Array { dims, .. } = self.info(ty).kind.clone() else {
                    return false;
                };
                let lint = self.model.types.elem(Elem::LInt);
                let indices: Vec<SyntaxNode> = children.collect();
                indices.len() == dims.len()
                    && indices.iter().zip(&dims).all(|(index, (lower, upper))| {
                        self.constant_index(index, lint)
                            .is_some_and(|constant| (*lower..=*upper).contains(&constant))
                    })
                    && self.pure_place(&base)
            }
            _ => false,
        }
    }

    /// Whether the variable `name` is read where it lies, as `name_place` finds it: a variable of
    /// the frame, of the instance or a global, not a function block's `VAR_IN_OUT`, which is read
    /// through its stored reference (§6.4).
    fn direct_variable(&self, name: &str) -> bool {
        if self
            .locals
            .iter()
            .any(|local| local.name.eq_ignore_ascii_case(name))
        {
            return true;
        }
        match self.frame {
            Frame::Program(index) => {
                if self.model.program_types[index].member(name).is_some() {
                    return true;
                }
            }
            Frame::FunctionBlock(ty) | Frame::Method(ty) => {
                if let Some(member) = self
                    .info(ty)
                    .members()
                    .iter()
                    .find(|member| member.name.eq_ignore_ascii_case(name))
                {
                    return member.flags & SYMBOL_IN_OUT == 0;
                }
            }
            Frame::Function => {}
        }
        self.model.global(name).is_some()
    }

    /// Whether a release image may hold the control variable `name` of the `FOR` loop `node` in a
    /// Rust local (§6.6): a variable of the POU's own frame or instance, which no other POU names,
    /// in a loop whose body calls nothing, dereferences nothing, reaches no property and no
    /// `VAR_IN_OUT`, and has no `FOR` over the same variable; trust-hir refuses an assignment to it
    /// in the body.
    pub(super) fn holds_control(&self, node: &SyntaxNode, name: &str) -> bool {
        self.own_variable(name)
            && node
                .children()
                .filter(|child| super::super::front::is_statement(child.kind()))
                .all(|statement| {
                    statement
                        .descendants()
                        .all(|inner| !self.may_write_control(&inner, name))
                })
    }

    /// Whether `name` is a variable of the frame or of the instance that is not a `VAR_IN_OUT`.
    fn own_variable(&self, name: &str) -> bool {
        if let Some(local) = self
            .locals
            .iter()
            .rev()
            .find(|local| local.name.eq_ignore_ascii_case(name))
        {
            return !local.is_ref;
        }
        match self.frame {
            Frame::Program(index) => self.model.program_types[index].member(name).is_some(),
            Frame::FunctionBlock(ty) | Frame::Method(ty) => self
                .info(ty)
                .members()
                .iter()
                .find(|member| member.name.eq_ignore_ascii_case(name))
                .is_some_and(|member| member.flags & SYMBOL_IN_OUT == 0),
            Frame::Function => false,
        }
    }

    /// Whether `node`, a part of a loop's body, can write the loop's control variable `name`
    /// other than by the loop: a call, a dereference, a property, a `VAR_IN_OUT`, which can name
    /// any variable, or a `FOR` over `name`.
    fn may_write_control(&self, node: &SyntaxNode, name: &str) -> bool {
        match node.kind() {
            SyntaxKind::CallExpr | SyntaxKind::DerefExpr => true,
            SyntaxKind::ForStmt => node
                .children()
                .find(|child| child.kind() == SyntaxKind::Name)
                .is_some_and(|control| call::ident(&control).eq_ignore_ascii_case(name)),
            SyntaxKind::FieldExpr => self.property_name(node).is_some(),
            SyntaxKind::NameRef => {
                self.property_name(node).is_some() || self.through_reference(&call::ident(node))
            }
            _ => false,
        }
    }

    /// Whether the variable `name` is reached through a reference: a `VAR_IN_OUT` of the frame or
    /// of the function block.
    fn through_reference(&self, name: &str) -> bool {
        if let Some(local) = self
            .locals
            .iter()
            .rev()
            .find(|local| local.name.eq_ignore_ascii_case(name))
        {
            return local.is_ref;
        }
        match self.frame {
            Frame::FunctionBlock(ty) | Frame::Method(ty) => self
                .info(ty)
                .members()
                .iter()
                .find(|member| member.name.eq_ignore_ascii_case(name))
                .is_some_and(|member| member.flags & SYMBOL_IN_OUT != 0),
            Frame::Program(_) | Frame::Function => false,
        }
    }
}
