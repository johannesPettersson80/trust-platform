//! Bytecode container format types.

pub use trust_runtime_core::bytecode::{
    BytecodeError, BytecodeMetadata, BytecodeModuleView, BytecodeVersion, ConstEntry, ConstPool,
    DebugEntry, DebugMap, EnumVariant, Field, InterfaceImpl, InterfaceMethod, IoBinding, IoMap,
    MethodEntry, ParamEntry, PouClassMeta, PouEntry, PouIndex, PouKind, ProcessImageConfig,
    RefEntry, RefLocation, RefSegment, RefTable, RejectionReason, ResourceEntry, ResourceMeta,
    ResourceMetadata, RetainInit, RetainInitEntry, Section, SectionData, SectionEntry, SectionId,
    StringTable, TaskEntry, TypeData, TypeEntry, TypeKind, TypeTable, ValidatedBytecode,
    ValidationLimits, ValidationStats, VarMeta, VarMetaEntry, BYTECODE_MAX_CONST_NESTING,
    BYTECODE_MAX_CONTAINER_BYTES, BYTECODE_MAX_INSTRUCTIONS, BYTECODE_MAX_LOCALS_PER_POU,
    BYTECODE_MAX_NATIVE_ARGUMENTS, BYTECODE_MAX_PARAMETERS_PER_POU, BYTECODE_MAX_REFERENCES,
    NATIVE_CALL_KIND_FUNCTION, NATIVE_CALL_KIND_FUNCTION_BLOCK, NATIVE_CALL_KIND_METHOD,
    NATIVE_CALL_KIND_STDLIB, SUPPORTED_MAJOR_VERSION, SUPPORTED_MINOR_VERSION,
};

mod module;
pub use module::BytecodeModule;
