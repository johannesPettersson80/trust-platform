use smol_str::SmolStr;

use crate::value::Value;

use super::ops::{BinaryOp, UnaryOp};
use super::CallArg;

/// Expression node.
#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Value),
    This,
    Super,
    SizeOf(SizeOfTarget),
    Name(SmolStr),
    Call {
        target: Box<Expr>,
        args: Vec<CallArg>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Index {
        target: Box<Expr>,
        indices: Vec<Expr>,
    },
    Field {
        target: Box<Expr>,
        field: SmolStr,
    },
    Ref(LValue),
    Deref(Box<Expr>),
}

/// SIZEOF target.
#[derive(Debug, Clone)]
pub enum SizeOfTarget {
    Type(trust_hir::TypeId),
    Expr(Box<Expr>),
}

/// Assignment target.
#[derive(Debug, Clone)]
pub enum LValue {
    Name(SmolStr),
    Index { name: SmolStr, indices: Vec<Expr> },
    Field { name: SmolStr, field: SmolStr },
    Deref(Box<Expr>),
}

impl LValue {
    #[must_use]
    pub fn name(&self) -> &SmolStr {
        match self {
            LValue::Name(name) => name,
            LValue::Index { name, .. } => name,
            LValue::Field { name, .. } => name,
            LValue::Deref(_) => {
                static PLACEHOLDER: SmolStr = SmolStr::new_static("<deref>");
                &PLACEHOLDER
            }
        }
    }
}
