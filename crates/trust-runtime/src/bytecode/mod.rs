//! Bytecode container format and metadata.

#![allow(missing_docs)]

mod encoder;
mod format;

#[cfg(test)]
pub(crate) use encoder::build_legacy_fixture_module;
pub(crate) use encoder::build_legacy_with_type_projection;
#[cfg(test)]
pub(crate) use encoder::build_module_from_declarations;
pub(crate) use encoder::{
    authoring_value_type, build_legacy_module_from_builder, format_authoring_io_address,
};
pub use encoder::{
    build_module_from_runtime, build_module_from_runtime_with_sources,
    build_module_from_runtime_with_sources_and_paths,
};
pub(crate) use encoder::{
    build_module_with_type_projection, build_module_with_type_projection_for_version,
};
pub use format::*;
