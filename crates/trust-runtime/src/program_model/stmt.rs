use smol_str::SmolStr;

use crate::debug::SourceLocation;
use crate::value::Value;

use super::expr::{Expr, LValue};

/// Statement execution result.
#[derive(Debug, Clone, PartialEq)]
pub enum StmtResult {
    Continue,
    Return(Option<Value>),
    Exit,
    LoopContinue,
    Jump(SmolStr),
}

/// CASE label.
#[derive(Debug, Clone)]
pub enum CaseLabel {
    Single(Value),
    Range(i64, i64),
}

/// Statement node.
#[derive(Debug, Clone)]
pub enum Stmt {
    Assign {
        target: LValue,
        value: Expr,
        location: Option<SourceLocation>,
    },
    AssignAttempt {
        target: LValue,
        value: Expr,
        location: Option<SourceLocation>,
    },
    Expr {
        expr: Expr,
        location: Option<SourceLocation>,
    },
    If {
        condition: Expr,
        then_block: Vec<Stmt>,
        else_if: Vec<(Expr, Vec<Stmt>)>,
        else_block: Vec<Stmt>,
        location: Option<SourceLocation>,
    },
    Case {
        selector: Expr,
        branches: Vec<(Vec<CaseLabel>, Vec<Stmt>)>,
        else_block: Vec<Stmt>,
        location: Option<SourceLocation>,
    },
    For {
        control: SmolStr,
        start: Expr,
        end: Expr,
        step: Expr,
        body: Vec<Stmt>,
        location: Option<SourceLocation>,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
        location: Option<SourceLocation>,
    },
    Repeat {
        body: Vec<Stmt>,
        until: Expr,
        location: Option<SourceLocation>,
    },
    Label {
        name: SmolStr,
        stmt: Option<Box<Stmt>>,
        location: Option<SourceLocation>,
    },
    Jmp {
        target: SmolStr,
        location: Option<SourceLocation>,
    },
    Return {
        expr: Option<Expr>,
        location: Option<SourceLocation>,
    },
    Exit {
        location: Option<SourceLocation>,
    },
    Continue {
        location: Option<SourceLocation>,
    },
}

impl Stmt {
    #[must_use]
    pub fn location(&self) -> Option<&SourceLocation> {
        match self {
            Stmt::Assign { location, .. }
            | Stmt::AssignAttempt { location, .. }
            | Stmt::Expr { location, .. }
            | Stmt::If { location, .. }
            | Stmt::Case { location, .. }
            | Stmt::For { location, .. }
            | Stmt::While { location, .. }
            | Stmt::Repeat { location, .. }
            | Stmt::Label { location, .. }
            | Stmt::Jmp { location, .. }
            | Stmt::Return { location, .. }
            | Stmt::Exit { location, .. }
            | Stmt::Continue { location, .. } => location.as_ref(),
        }
    }
}
