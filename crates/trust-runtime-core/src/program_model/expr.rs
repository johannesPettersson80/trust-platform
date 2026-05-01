use alloc::{boxed::Box, format, vec::Vec};

use smol_str::SmolStr;
use trust_hir::TypeId;

use crate::value::Value;

use super::ops::{BinaryOp, UnaryOp};

/// Expression node.
#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Value),
    ArrayInitializer(Vec<Expr>),
    StructInitializer(Vec<(SmolStr, Expr)>),
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
    Type(TypeId),
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

/// Call argument value.
#[derive(Debug, Clone)]
pub enum ArgValue {
    Expr(Expr),
    Target(LValue),
}

/// Named call argument.
#[derive(Debug, Clone)]
pub struct CallArg {
    pub name: Option<SmolStr>,
    pub value: ArgValue,
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

#[cfg(test)]
mod tests {
    use super::LValue;
    use smol_str::SmolStr;

    #[test]
    fn lvalue_root_and_qualified_name_contracts_hold() {
        let lvalue = LValue::Field {
            target: Box::new(LValue::Field {
                target: Box::new(LValue::Name(SmolStr::new("fb"))),
                field: SmolStr::new("nested"),
            }),
            field: SmolStr::new("field"),
        };

        assert_eq!(lvalue.root_name().map(SmolStr::as_str), Some("fb"));
        assert_eq!(lvalue.qualified_name().as_deref(), Some("fb.nested.field"));
        assert!(!lvalue.contains_index());
    }
}
