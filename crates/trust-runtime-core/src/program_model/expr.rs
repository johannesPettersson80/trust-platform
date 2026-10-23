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
    /// Resolve parser-owned index and reference expressions into values.
    /// The returned symbolic destination carries no expression-only type IDs;
    /// the execution owner must still check its binding and lifetime at commit.
    pub fn resolve_expressions<E>(
        self,
        eval: &mut impl FnMut(&Expr) -> Result<Value, E>,
    ) -> Result<Self, E> {
        match self {
            Self::Name(_) => Ok(self),
            Self::Field { target, field } => Ok(Self::Field {
                target: Box::new(target.resolve_expressions(eval)?),
                field,
            }),
            Self::Index { target, indices } => Ok(Self::Index {
                target: Box::new(target.resolve_expressions(eval)?),
                indices: indices
                    .iter()
                    .map(|expr| eval(expr).map(Expr::Literal))
                    .collect::<Result<Vec<_>, E>>()?,
            }),
            Self::Deref(expr) => Ok(Self::Deref(Box::new(Expr::Literal(eval(&expr)?)))),
        }
    }

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
    use super::{Expr, LValue};
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

    #[test]
    fn lvalue_path_queries_cover_index_field_and_deref_boundaries() {
        let name = LValue::Name(SmolStr::new("root"));
        assert_eq!(name.root_name().map(SmolStr::as_str), Some("root"));
        assert_eq!(name.qualified_name().as_deref(), Some("root"));
        assert!(!name.contains_index());

        let field_after_index = LValue::Field {
            target: Box::new(LValue::Index {
                target: Box::new(LValue::Field {
                    target: Box::new(LValue::Name(SmolStr::new("root"))),
                    field: SmolStr::new("items"),
                }),
                indices: vec![Expr::Name(SmolStr::new("i"))],
            }),
            field: SmolStr::new("value"),
        };
        assert_eq!(
            field_after_index.root_name().map(SmolStr::as_str),
            Some("root")
        );
        assert_eq!(field_after_index.qualified_name(), None);
        assert!(field_after_index.contains_index());

        let field_after_deref = LValue::Field {
            target: Box::new(LValue::Deref(Box::new(Expr::Name(SmolStr::new("pointer"))))),
            field: SmolStr::new("value"),
        };
        assert_eq!(field_after_deref.root_name(), None);
        assert_eq!(field_after_deref.qualified_name(), None);
        assert!(!field_after_deref.contains_index());
    }
}
