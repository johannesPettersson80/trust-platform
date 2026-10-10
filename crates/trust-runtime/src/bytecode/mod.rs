//! Bytecode container format and metadata.

#![allow(missing_docs)]

mod encoder;
mod format;

pub use encoder::{
    build_module_from_runtime, build_module_from_runtime_with_sources,
    build_module_from_runtime_with_sources_and_paths,
};
pub use format::*;
