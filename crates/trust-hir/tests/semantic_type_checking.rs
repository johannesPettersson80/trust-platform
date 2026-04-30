mod common;

#[path = "semantic_type_checking/array_wildcard_compatibility.rs"]
mod array_wildcard_compatibility;
#[path = "semantic_type_checking/assignments_and_var_access.rs"]
mod assignments_and_var_access;
#[path = "semantic_type_checking/basics_and_warnings.rs"]
mod basics_and_warnings;
#[path = "semantic_type_checking/control_flow_and_calls.rs"]
mod control_flow_and_calls;
#[path = "semantic_type_checking/enum_unqualified_in_expressions.rs"]
mod enum_unqualified_in_expressions;
#[path = "semantic_type_checking/hir_mutation_hardening.rs"]
mod hir_mutation_hardening;
#[path = "semantic_type_checking/parameter_constant_qualifier.rs"]
mod parameter_constant_qualifier;
#[path = "semantic_type_checking/pointer_param_write_through.rs"]
mod pointer_param_write_through;
#[path = "semantic_type_checking/sizeof_semantics.rs"]
mod sizeof_semantics;
#[path = "semantic_type_checking/struct_initializers.rs"]
mod struct_initializers;
#[path = "semantic_type_checking/types_and_references.rs"]
mod types_and_references;
#[path = "semantic_type_checking/wrong_kind_resolution.rs"]
mod wrong_kind_resolution;
