//! Variable handling split by request type and helpers.
//! - state: stVarState snapshots
//! - write: stVarWrite request
//! - list: variables listing + handles
//! - set: setVariable handling
//! - expression: setExpression handling
//! - symbolic_force: force/release of named targets for setExpression
//! - eval: evaluate + snapshot evaluation
//! - compiled: debug expressions compiled for a runtime that runs a compiled program
//! - format: value formatting + type mapping

mod compiled;
mod eval;
mod expression;
mod format;
mod list;
pub(in crate::adapter) mod retained;
mod set;
mod state;
mod symbolic_force;
mod write;

pub(in crate::adapter) use compiled::ExpressionTypes;
pub(in crate::adapter) use format::{format_value, type_id_for_value, value_type_name};
