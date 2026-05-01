//! Bytecode container format records shared by host and core bytecode paths.

#![allow(missing_docs)]

use alloc::vec::Vec;

use smol_str::SmolStr;

include!("format/header.rs");
include!("format/types.rs");
include!("format/refs_consts.rs");
include!("format/pou.rs");
include!("format/resource_io_debug.rs");
