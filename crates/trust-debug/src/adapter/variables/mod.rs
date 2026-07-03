//! Variable handling split by request type and helpers.
//! - state: stVarState snapshots
//! - write: stVarWrite request
//! - list: variables listing + handles
//! - set: setVariable handling
//! - expression: setExpression handling
//! - eval: evaluate + snapshot evaluation
//! - format: value formatting + type mapping

mod eval;
mod expression;
mod format;
mod list;
mod set;
mod state;
mod write;

pub(in crate::adapter) use format::{format_value, type_id_for_value, value_type_name};
