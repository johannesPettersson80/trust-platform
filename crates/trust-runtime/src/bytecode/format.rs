//! Bytecode container format types.

#![allow(missing_docs)]

pub use trust_runtime_core::bytecode::{
    BytecodeError, BytecodeMetadata, BytecodeVersion, ConstEntry, ConstPool, DebugEntry, DebugMap,
    EnumVariant, Field, InterfaceImpl, InterfaceMethod, IoBinding, IoMap, MethodEntry, ParamEntry,
    PouClassMeta, PouEntry, PouIndex, PouKind, ProcessImageConfig, RefEntry, RefLocation,
    RefSegment, RefTable, ResourceEntry, ResourceMeta, ResourceMetadata, RetainInit,
    RetainInitEntry, Section, SectionData, SectionEntry, SectionId, StringTable, TaskEntry,
    TypeData, TypeEntry, TypeKind, TypeTable, VarMeta, VarMetaEntry, NATIVE_CALL_KIND_FUNCTION,
    NATIVE_CALL_KIND_FUNCTION_BLOCK, NATIVE_CALL_KIND_METHOD, NATIVE_CALL_KIND_STDLIB,
    SUPPORTED_MAJOR_VERSION, SUPPORTED_MINOR_VERSION,
};
pub(crate) use trust_runtime_core::bytecode::{
    HEADER_FLAG_CRC32, HEADER_SIZE, MAGIC, SECTION_ENTRY_SIZE,
};

include!("format/module.rs");
