//! Harness lowering and configuration compilation.

#![allow(missing_docs)]

mod action_profile;
mod config;
mod dependency_prelude;
mod edge;
mod model;
mod pou;
mod types;
mod vars;

#[cfg(test)]
#[path = "action_profile_contract_tests.rs"]
mod action_profile_contract_tests;

#[cfg(test)]
#[path = "generic_type_profile_contract_tests.rs"]
mod generic_type_profile_contract_tests;

pub(super) use action_profile::contains_textual_action;
pub(super) use config::{
    lower_configuration, lower_root_global_var_blocks, resolve_program_type_name,
};
pub(super) use dependency_prelude::{
    predeclare_project_types, resolve_pou_local_constants, resolve_project_global_constants,
    validate_project_aliases,
};
pub(super) use edge::collect_edge_declarations;
pub(super) use model::{
    AccessDecl, AccessPart, AccessPath, ConfigInit, GlobalInit, LoweringContext, LoweringInputs,
    ProgramInstanceConfig, ResolvedAccess, WildcardRequirement,
};
pub(super) use pou::{
    lower_classes, lower_function_blocks, lower_functions, lower_interfaces, lower_programs,
};
pub(super) use types::{
    class_type_name, function_block_type_name, interface_type_name, lower_type_decls,
    lower_type_ref, predeclare_classes, predeclare_function_blocks, predeclare_interfaces,
    resolve_named_type, resolve_type_name,
};

#[cfg(test)]
#[path = "pou_function_contract_tests.rs"]
mod pou_function_contract_tests;

#[cfg(test)]
#[path = "pou_call_binding_runtime_contract_tests.rs"]
mod pou_call_binding_runtime_contract_tests;

#[cfg(test)]
#[path = "pou_initializer_dependency_contract_tests.rs"]
mod pou_initializer_dependency_contract_tests;

#[cfg(test)]
#[path = "pou_initializer_lifetime_contract_tests.rs"]
mod pou_initializer_lifetime_contract_tests;

#[cfg(test)]
#[path = "pou_member_access_contract_tests.rs"]
mod pou_member_access_contract_tests;

#[cfg(test)]
#[path = "pou_object_contract_tests.rs"]
mod pou_object_contract_tests;

#[cfg(test)]
#[path = "pou_variable_section_acceptance_contract_tests.rs"]
mod pou_variable_section_acceptance_contract_tests;

#[cfg(test)]
#[path = "pou_variable_section_rejection_contract_tests.rs"]
mod pou_variable_section_rejection_contract_tests;

#[cfg(test)]
#[path = "pou_variable_qualifier_projection_contract_tests.rs"]
mod pou_variable_qualifier_projection_contract_tests;

#[cfg(test)]
#[path = "project_assembly_contract_tests.rs"]
mod project_assembly_contract_tests;

#[cfg(test)]
#[path = "constant_initializer_contract_tests.rs"]
mod constant_initializer_contract_tests;

#[cfg(test)]
#[path = "default_initializer_dependency_contract_tests.rs"]
mod default_initializer_dependency_contract_tests;

#[cfg(test)]
#[path = "derived_type_constant_contract_tests.rs"]
mod derived_type_constant_contract_tests;

#[cfg(test)]
#[path = "type_contract_tests.rs"]
mod type_contract_tests;

#[cfg(test)]
#[path = "variable_contract_tests.rs"]
mod variable_contract_tests;
