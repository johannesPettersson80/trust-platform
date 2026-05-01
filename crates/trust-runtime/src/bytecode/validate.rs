//! Bytecode validation.

#![allow(missing_docs)]

use std::collections::HashSet;

use super::reader::BytecodeReader;
use super::{
    BytecodeError, BytecodeModule, ConstEntry, ConstPool, DebugMap, IoMap, PouEntry, PouIndex,
    PouKind, RefLocation, RefSegment, RefTable, ResourceMeta, RetainInit, SectionData, SectionId,
    StringTable, TypeData, TypeEntry, TypeKind, TypeTable, VarMeta,
};

include!("validate/module_validate.rs");
include!("validate/tables_consts.rs");
include!("validate/pou_and_instr.rs");
include!("validate/resource_io.rs");
include!("validate/meta_debug.rs");
