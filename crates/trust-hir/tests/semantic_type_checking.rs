mod common;

#[path = "semantic_type_checking/array_wildcard_compatibility.rs"]
mod array_wildcard_compatibility;
#[path = "semantic_type_checking/assignments_and_var_access.rs"]
mod assignments_and_var_access;
#[path = "semantic_type_checking/basics_and_warnings.rs"]
mod basics_and_warnings;
#[path = "semantic_type_checking/bounded_value_semantics.rs"]
mod bounded_value_semantics;
#[path = "semantic_type_checking/call_binding_contract_acceptance.rs"]
mod call_binding_contract_acceptance;
#[path = "semantic_type_checking/call_binding_contract_rejection.rs"]
mod call_binding_contract_rejection;
#[path = "semantic_type_checking/control_flow_and_calls.rs"]
mod control_flow_and_calls;
#[path = "semantic_type_checking/control_flow_contract_acceptance.rs"]
mod control_flow_contract_acceptance;
#[path = "semantic_type_checking/control_flow_contract_rejection.rs"]
mod control_flow_contract_rejection;
#[path = "semantic_type_checking/edge_declaration_contract.rs"]
mod edge_declaration_contract;
#[path = "semantic_type_checking/enum_unqualified_in_expressions.rs"]
mod enum_unqualified_in_expressions;
#[path = "semantic_type_checking/expression_operator_contract_acceptance.rs"]
mod expression_operator_contract_acceptance;
#[path = "semantic_type_checking/expression_operator_contract_rejection.rs"]
mod expression_operator_contract_rejection;
#[path = "semantic_type_checking/generic_type_contract.rs"]
mod generic_type_contract;
#[path = "semantic_type_checking/hir_mutation_hardening.rs"]
mod hir_mutation_hardening;
#[path = "semantic_type_checking/member_access_acceptance.rs"]
mod member_access_acceptance;
#[path = "semantic_type_checking/member_access_declaration_rejection.rs"]
mod member_access_declaration_rejection;
#[path = "semantic_type_checking/member_access_use_rejection.rs"]
mod member_access_use_rejection;
#[path = "semantic_type_checking/parameter_constant_qualifier.rs"]
mod parameter_constant_qualifier;
#[path = "semantic_type_checking/partial_access_acceptance.rs"]
mod partial_access_acceptance;
#[path = "semantic_type_checking/partial_access_rejection.rs"]
mod partial_access_rejection;
#[path = "semantic_type_checking/phase18_behavior_closure.rs"]
mod phase18_behavior_closure;
#[path = "semantic_type_checking/pointer_param_write_through.rs"]
mod pointer_param_write_through;
#[path = "semantic_type_checking/reference_contract_acceptance.rs"]
mod reference_contract_acceptance;
#[path = "semantic_type_checking/reference_contract_rejection.rs"]
mod reference_contract_rejection;
#[path = "semantic_type_checking/sizeof_semantics.rs"]
mod sizeof_semantics;
#[path = "semantic_type_checking/string_index_contract.rs"]
mod string_index_contract;
#[path = "semantic_type_checking/struct_initializers.rs"]
mod struct_initializers;
#[path = "semantic_type_checking/types_and_references.rs"]
mod types_and_references;
#[path = "semantic_type_checking/user_type_contract_acceptance.rs"]
mod user_type_contract_acceptance;
#[path = "semantic_type_checking/user_type_contract_rejection.rs"]
mod user_type_contract_rejection;
#[path = "semantic_type_checking/variable_initializer_constant_expression.rs"]
mod variable_initializer_constant_expression;
#[path = "semantic_type_checking/variable_section_qualifier_acceptance.rs"]
mod variable_section_qualifier_acceptance;
#[path = "semantic_type_checking/variable_section_qualifier_rejection.rs"]
mod variable_section_qualifier_rejection;
#[path = "semantic_type_checking/wrong_kind_resolution.rs"]
mod wrong_kind_resolution;
