mod expr;
mod stmt;

pub(super) use expr::{
    const_duration_from_node, const_int_from_node, lower_expr, lower_lvalue, parse_subrange,
    resolve_initializer_enum_variant,
};
pub(super) use stmt::lower_stmt_list;

#[cfg(test)]
#[path = "expression_contract_tests.rs"]
mod expression_contract_tests;

#[cfg(test)]
#[path = "expression_operator_runtime_contract_tests.rs"]
mod expression_operator_runtime_contract_tests;

#[cfg(test)]
#[path = "partial_access_runtime_contract_tests.rs"]
mod partial_access_runtime_contract_tests;

#[cfg(test)]
#[path = "reference_runtime_contract_tests.rs"]
mod reference_runtime_contract_tests;

#[cfg(test)]
#[path = "runtime_clock_source_contract_tests.rs"]
mod runtime_clock_source_contract_tests;

#[cfg(test)]
#[path = "statement_contract_tests.rs"]
mod statement_contract_tests;

#[cfg(test)]
#[path = "statement_control_flow_contract_tests.rs"]
mod statement_control_flow_contract_tests;

#[cfg(test)]
#[path = "string_index_runtime_contract_tests.rs"]
mod string_index_runtime_contract_tests;
