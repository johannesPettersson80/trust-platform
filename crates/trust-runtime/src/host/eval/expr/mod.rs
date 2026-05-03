//! Expression evaluation.

#![allow(missing_docs)]

#[cfg(test)]
mod access;
#[cfg(test)]
mod ast;
#[cfg(test)]
mod call;
#[cfg(test)]
mod eval;
#[cfg(test)]
mod lvalue;

pub use crate::program_model::expr::{Expr, LValue, SizeOfTarget};
#[cfg(test)]
pub(crate) use eval::eval_expr;
#[cfg(test)]
pub(crate) use lvalue::{read_lvalue, write_lvalue};

#[cfg(test)]
pub(crate) use call::read_arg_value;
