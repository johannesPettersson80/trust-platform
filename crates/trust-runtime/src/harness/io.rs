use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::io::IoAddress;
use crate::memory::{InstanceId, VariableStorage};
use crate::program_model::FunctionBlockDef;
use crate::task::ProgramDef;
use crate::value::Value;
use trust_hir::types::TypeRegistry;
use trust_hir::{Type, TypeId};

use super::{CompileError, WildcardRequirement};

include!("io/types.rs");
include!("io/instance_bindings.rs");
include!("io/address_parse.rs");
include!("io/io_bindings.rs");
include!("io/direct_field_bindings.rs");
include!("io/sizing.rs");
