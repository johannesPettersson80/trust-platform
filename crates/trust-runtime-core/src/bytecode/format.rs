//! Bytecode container format records shared by host and core bytecode paths.

use alloc::vec::Vec;

use smol_str::SmolStr;

mod header;
pub use header::*;
mod types;
pub use types::*;
mod refs_consts;
pub use refs_consts::*;
mod pou;
pub use pou::*;
mod resource_io_debug;
pub use resource_io_debug::*;
