mod configuration;
mod context;
mod expression;
mod globals;
mod nondeterminism;
mod oop;
mod shared_globals;
mod type_check;
mod unreachable;
mod unused;
mod using;

#[cfg(test)]
mod tests;

pub(super) use complexity::check_cyclomatic_complexity;
pub(super) use configuration::check_configuration_semantics;
pub(super) use context::{expression_context, is_pou_kind};
pub(super) use expression::{expression_by_id, is_expression_kind, ExpressionIndex};
pub(super) use globals::{
    check_global_external_links_with_project, resolve_pending_types_with_table,
};
pub(super) use nondeterminism::check_nondeterminism;
pub(super) use oop::{
    check_abstract_instantiations, check_class_semantics, check_extends_implements_semantics,
    check_interface_conformance, check_property_accessors,
};
pub(super) use shared_globals::check_shared_global_task_hazards;
pub(super) use type_check::type_check_file;
pub(super) use unreachable::check_unreachable_statements;
pub(super) use unused::{add_unused_symbol_warnings, collect_used_symbols};
pub(super) use using::check_using_directives;
mod complexity;
