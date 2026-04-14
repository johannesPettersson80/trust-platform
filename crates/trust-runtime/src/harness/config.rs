use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::instance::{create_class_instance, create_fb_instance};
use crate::task::ProgramDef;
use crate::value::{default_value_for_type_id, Value};
use crate::Runtime;

use super::io::{
    bind_value_ref_to_address, collect_direct_field_bindings, collect_instance_bindings,
    collect_program_instance_bindings,
};
use super::{
    AccessDecl, AccessPart, AccessPath, CompileError, ConfigInit, GlobalInit,
    ProgramInstanceConfig, ResolvedAccess, WildcardRequirement,
};

include!("config/access_paths.rs");
include!("config/program_tasks.rs");
include!("config/globals.rs");
include!("config/config_inits.rs");
include!("config/bindings.rs");
