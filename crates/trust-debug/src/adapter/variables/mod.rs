//! Variable handling split by request type and helpers.
//! - list: variables listing
//! - set: setVariable handling
//! - expression: setExpression handling
//! - eval: evaluate
//! - compiled: debug expressions compiled for a runtime that runs a compiled program

mod compiled;
mod eval;
mod expression;
mod list;
mod set;

pub(in crate::adapter) use compiled::ExpressionTypes;
