//! Bytecode validation.

#![allow(missing_docs)]

use std::collections::{HashMap, HashSet};

use super::reader::BytecodeReader;
use super::{
    BytecodeError, BytecodeModule, ConstEntry, ConstPool, DebugMap, IoMap, ParamEntry, PouEntry,
    PouIndex, PouKind, RefLocation, RefSegment, RefTable, ResourceMeta, RetainInit, SectionData,
    SectionId, StringTable, TypeData, TypeEntry, TypeKind, TypeTable, VarMeta,
    NATIVE_CALL_KIND_FUNCTION, NATIVE_CALL_KIND_FUNCTION_BLOCK,
};

mod resource_limits;

use resource_limits::{
    charge_decoded_instruction, validate_declared_resource_limits, validate_operand_stack_depth,
};

include!("validate/module_validate.rs");
include!("validate/tables_consts.rs");
include!("validate/pou_and_instr.rs");
include!("validate/reference_escape.rs");
include!("validate/owner_contract.rs");
include!("validate/stack_shape.rs");
include!("validate/const_compat.rs");
include!("validate/param_direction.rs");
include!("validate/call_target.rs");
include!("validate/resource_io.rs");
include!("validate/meta_debug.rs");
