//! Variable handling split by request type and helpers.
//! - list: variables listing
//! - set: setVariable handling
//! - expression: setExpression handling
//! - write: the runtime requests of a write, a force and a release
//! - eval: evaluate
//! - compiled: debug expressions compiled for a runtime that runs a compiled program

mod compiled;
mod eval;
mod expression;
mod list;
mod set;
mod write;
#[cfg(test)]
mod write_tests;

pub(in crate::adapter) use compiled::ExpressionTypes;
