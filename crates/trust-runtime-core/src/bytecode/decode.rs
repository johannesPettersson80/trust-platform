//! Bytecode decoding.

use alloc::vec::Vec;

use smol_str::SmolStr;

use super::align4;
use super::BytecodeReader;
use super::{
    BytecodeError, BytecodeModule, BytecodeVersion, ConstEntry, ConstPool, DebugEntry, DebugMap,
    EnumVariant, Field, InterfaceImpl, InterfaceMethod, IoBinding, IoMap, MethodEntry,
    PouClassMeta, PouEntry, PouIndex, PouKind, RefEntry, RefLocation, RefSegment, RefTable,
    RejectionReason, ResourceEntry, ResourceMeta, RetainInit, RetainInitEntry, Section,
    SectionData, SectionDiagnostic, SectionEntry, SectionId, StringTable, TypeData, TypeEntry,
    TypeKind, TypeTable, VarMeta, VarMetaEntry, BYTECODE_MAX_CONTAINER_BYTES,
    BYTECODE_MAX_LOCALS_PER_POU, BYTECODE_MAX_PARAMETERS_PER_POU, BYTECODE_MAX_REFERENCES,
    HEADER_FLAG_CRC32, HEADER_SIZE, MAGIC, SECTION_ENTRY_SIZE,
};

fn read_bounded_count(
    reader: &mut BytecodeReader<'_>,
    minimum_entry_bytes: usize,
    context: &'static str,
) -> Result<usize, BytecodeError> {
    debug_assert!(minimum_entry_bytes > 0);
    let count = reader.read_u32()? as usize;
    let required = count.checked_mul(minimum_entry_bytes).ok_or_else(|| {
        BytecodeError::section_diagnostic(super::SectionDiagnostic::CountBounds(context))
    })?;
    if required > reader.remaining() {
        return Err(BytecodeError::section_diagnostic(
            super::SectionDiagnostic::CountBounds(context),
        ));
    }
    Ok(count)
}

fn read_bounded_count_with_limit(
    reader: &mut BytecodeReader<'_>,
    minimum_entry_bytes: usize,
    maximum: usize,
    context: &'static str,
) -> Result<usize, BytecodeError> {
    debug_assert!(minimum_entry_bytes > 0);
    let count = reader.read_u32()? as usize;
    if count > maximum {
        return Err(BytecodeError::section_diagnostic(
            super::SectionDiagnostic::CountLimit(context),
        ));
    }
    let required = count.checked_mul(minimum_entry_bytes).ok_or_else(|| {
        BytecodeError::section_diagnostic(super::SectionDiagnostic::CountBounds(context))
    })?;
    if required > reader.remaining() {
        return Err(BytecodeError::section_diagnostic(
            super::SectionDiagnostic::CountBounds(context),
        ));
    }
    Ok(count)
}

mod budget;
use budget::DecodeBudget;
pub use budget::DecodeStats;
mod module_decode;
mod section_decode;
use section_decode::*;
mod string_type_decode;
use string_type_decode::*;
mod section_validate;
use section_validate::*;

mod construction;
