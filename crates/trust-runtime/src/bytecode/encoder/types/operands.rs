//! Discover expression-only type dependencies before selecting instance templates.

use super::*;
use crate::program_model::{ArgValue, Expr, LValue, SizeOfTarget, Stmt};

impl BytecodeEncoder<'_> {
    pub(super) fn collect_expression_types(&mut self, expr: &Expr) -> Result<(), BytecodeError> {
        if self.authoring.is_none() {
            return Ok(());
        }
        match expr {
            Expr::SizeOf(SizeOfTarget::Type(ty)) => {
                self.type_index(*ty)?;
            }
            Expr::ArrayInitializer(values) => {
                for value in values {
                    self.collect_expression_types(value)?;
                }
            }
            Expr::StructInitializer(fields) => {
                for (_, value) in fields {
                    self.collect_expression_types(value)?;
                }
            }
            Expr::Call { target, args } => {
                self.collect_expression_types(target)?;
                for arg in args {
                    match &arg.value {
                        ArgValue::Expr(expr) => self.collect_expression_types(expr)?,
                        ArgValue::Target(target) => self.collect_target_types(target)?,
                    }
                }
            }
            Expr::Unary { expr, .. } | Expr::Deref(expr) => self.collect_expression_types(expr)?,
            Expr::Binary { left, right, .. } => {
                self.collect_expression_types(left)?;
                self.collect_expression_types(right)?;
            }
            Expr::Index { target, indices } => {
                self.collect_expression_types(target)?;
                for index in indices {
                    self.collect_expression_types(index)?;
                }
            }
            Expr::Field { target, .. } => self.collect_expression_types(target)?,
            Expr::Ref(target) => self.collect_target_types(target)?,
            Expr::Literal(_) | Expr::This | Expr::Super | Expr::Name(_) => {}
        }
        Ok(())
    }

    fn collect_target_types(&mut self, target: &LValue) -> Result<(), BytecodeError> {
        match target {
            LValue::Index { target, indices } => {
                self.collect_target_types(target)?;
                for index in indices {
                    self.collect_expression_types(index)?;
                }
            }
            LValue::Field { target, .. } => self.collect_target_types(target)?,
            LValue::Deref(expr) => self.collect_expression_types(expr)?,
            LValue::Name(_) => {}
        }
        Ok(())
    }

    pub(super) fn collect_body_types(&mut self, statements: &[Stmt]) -> Result<(), BytecodeError> {
        if self.authoring.is_none() {
            return Ok(());
        }
        for statement in statements {
            match statement {
                Stmt::Assign { target, value, .. } | Stmt::AssignAttempt { target, value, .. } => {
                    if let Stmt::AssignAttempt { target_type, .. } = statement {
                        self.type_index(*target_type)?;
                    }
                    self.collect_target_types(target)?;
                    self.collect_expression_types(value)?;
                }
                Stmt::Expr { expr, .. }
                | Stmt::Return {
                    expr: Some(expr), ..
                } => self.collect_expression_types(expr)?,
                Stmt::If {
                    condition,
                    then_block,
                    else_if,
                    else_block,
                    ..
                } => {
                    self.collect_expression_types(condition)?;
                    self.collect_body_types(then_block)?;
                    for (condition, body) in else_if {
                        self.collect_expression_types(condition)?;
                        self.collect_body_types(body)?;
                    }
                    self.collect_body_types(else_block)?;
                }
                Stmt::Case {
                    selector,
                    branches,
                    else_block,
                    ..
                } => {
                    self.collect_expression_types(selector)?;
                    for (_, body) in branches {
                        self.collect_body_types(body)?;
                    }
                    self.collect_body_types(else_block)?;
                }
                Stmt::For {
                    start,
                    end,
                    step,
                    body,
                    ..
                } => {
                    for expr in [start, end, step] {
                        self.collect_expression_types(expr)?;
                    }
                    self.collect_body_types(body)?;
                }
                Stmt::While {
                    condition, body, ..
                } => {
                    self.collect_expression_types(condition)?;
                    self.collect_body_types(body)?;
                }
                Stmt::Repeat { body, until, .. } => {
                    self.collect_body_types(body)?;
                    self.collect_expression_types(until)?;
                }
                Stmt::Label {
                    stmt: Some(stmt), ..
                } => self.collect_body_types(std::slice::from_ref(stmt.as_ref()))?,
                Stmt::Return { expr: None, .. }
                | Stmt::Label { stmt: None, .. }
                | Stmt::Jmp { .. }
                | Stmt::Exit { .. }
                | Stmt::Continue { .. } => {}
            }
        }
        Ok(())
    }
}
