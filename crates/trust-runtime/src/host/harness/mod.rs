//! Test harness for driving runtime cycles.

#![allow(missing_docs)]

mod api;
mod artifact;
mod build;
mod coerce;
mod compiler;
mod config;
#[allow(clippy::module_inception)]
mod harness;
pub(crate) mod initializer;
mod io;
mod lower;
mod parse;
mod protocol;
mod types;
mod util;

pub use crate::boundary::{BoundaryEntry, BoundaryError};
pub use api::{
    bytecode_bytes_from_source, bytecode_bytes_from_source_with_path, bytecode_bytes_from_sources,
    bytecode_bytes_from_sources_with_paths, bytecode_module_from_source,
    bytecode_module_from_source_with_path, bytecode_module_from_sources,
    bytecode_module_from_sources_with_paths, CompileSession,
};
pub use artifact::configured_resource;
pub(crate) use build::LoweredApplication;
#[cfg(test)]
pub(crate) use build::{lower_application, lower_application_with_abi};
pub use coerce::{coerce_initializer_value_to_type, coerce_value_to_type};
pub(crate) use config::{program_access_target_policy, AccessTargetPolicy};
pub use harness::TestHarness;
pub(crate) use io::{
    bind_value_ref_to_address, collect_direct_field_bindings, parse_field_address, FieldAddress,
    InstanceBinding,
};
pub use parse::{
    parse_debug_expression, parse_debug_expression_with_abi, parse_debug_lvalue,
    parse_debug_lvalue_with_abi,
};
pub use protocol::{
    decode_json_value, encode_json_value, HarnessAutomation, HarnessAutomationError,
    HarnessLoadSummary, HarnessRunUntilSummary, HarnessStateSummary, HarnessValueSnapshot,
    HarnessWatchSnapshot,
};
pub use types::{CompileError, CycleResult, SourceFile};

pub(crate) use compiler::{
    class_type_name, function_block_type_name, AccessDecl, AccessPart, AccessPath, ConfigInit,
    GlobalInit, ProgramInstanceConfig, ResolvedAccess, WildcardRequirement,
};
use compiler::{
    collect_edge_declarations, contains_textual_action, interface_type_name, lower_classes,
    lower_configuration, lower_function_blocks, lower_functions, lower_interfaces, lower_programs,
    lower_type_decls, lower_type_ref, predeclare_classes, predeclare_function_blocks,
    predeclare_interfaces, predeclare_project_types, resolve_program_type_name,
    resolve_project_global_constants, resolve_type_name, validate_project_aliases, LoweringContext,
};
use lower::lower_expr;
