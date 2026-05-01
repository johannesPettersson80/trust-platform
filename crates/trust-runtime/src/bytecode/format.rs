//! Bytecode container format types.

#![allow(missing_docs)]

use smol_str::SmolStr;
pub use trust_runtime_core::bytecode::{
    BytecodeError, BytecodeMetadata, BytecodeVersion, ProcessImageConfig, ResourceMetadata,
    SUPPORTED_MAJOR_VERSION, SUPPORTED_MINOR_VERSION,
};

include!("format/header.rs");
include!("format/types.rs");
include!("format/refs_consts.rs");
include!("format/pou.rs");
include!("format/resource_io_debug.rs");
include!("format/module.rs");
