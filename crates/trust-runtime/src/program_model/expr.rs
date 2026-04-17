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
}

/// Assignment target.
#[derive(Debug, Clone)]
pub enum LValue {
    Name(SmolStr),
    Index {
        target: Box<LValue>,
        indices: Vec<Expr>,
    },
    Field {
        target: Box<LValue>,
        field: SmolStr,
    },
    Deref(Box<Expr>),
}

impl LValue {
    #[must_use]
    pub fn root_name(&self) -> Option<&SmolStr> {
        match self {
            LValue::Name(name) => Some(name),
            LValue::Index { target, .. } | LValue::Field { target, .. } => target.root_name(),
            LValue::Deref(_) => None,
        }
    }

    #[must_use]
    pub fn qualified_name(&self) -> Option<SmolStr> {
        match self {
            LValue::Name(name) => Some(name.clone()),
            LValue::Field { target, field } => {
                let prefix = target.qualified_name()?;
                Some(SmolStr::new(format!("{prefix}.{field}")))
            }
            LValue::Index { .. } | LValue::Deref(_) => None,
        }
    }

    #[must_use]
    pub fn contains_index(&self) -> bool {
        match self {
            LValue::Name(_) => false,
            LValue::Index { .. } => true,
            LValue::Field { target, .. } => target.contains_index(),
            LValue::Deref(_) => false,
        }
    }
}
