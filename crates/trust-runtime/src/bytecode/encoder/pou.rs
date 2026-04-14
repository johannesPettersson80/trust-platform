use std::collections::{HashMap, HashSet};

use smol_str::SmolStr;

use crate::bytecode::{
    DebugEntry, InterfaceMethod, MethodEntry, ParamEntry, PouClassMeta, PouEntry, PouIndex, PouKind,
};
use crate::program_model::{ClassDef, FunctionBlockDef, FunctionDef, MethodDef, Param};
use crate::value::Value;
use trust_hir::symbols::ParamDirection;

use super::util::{normalize_name, to_u32};
use super::{BytecodeEncoder, BytecodeError, CodegenContext, LocalScope};

include!("pou/build.rs");
include!("pou/entries.rs");
include!("pou/class_meta.rs");
include!("pou/class_like.rs");
