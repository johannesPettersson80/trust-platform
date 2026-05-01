//! Bytecode container format and metadata.

#![allow(missing_docs)]

mod decode;
mod encode;
mod encoder;
mod format;
mod metadata;
mod reader;
mod util;
mod validate;

pub use encoder::{
    build_module_from_runtime, build_module_from_runtime_with_sources,
    build_module_from_runtime_with_sources_and_paths,
};
pub use format::*;
