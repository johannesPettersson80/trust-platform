//! Bytecode decoding.

#![allow(missing_docs)]

use smol_str::SmolStr;

use super::reader::BytecodeReader;
use super::util::align4;
use super::{
    BytecodeError, BytecodeModule, BytecodeVersion, ConstEntry, ConstPool, DebugEntry, DebugMap,
    EnumVariant, Field, InterfaceImpl, InterfaceMethod, IoBinding, IoMap, MethodEntry,
    PouClassMeta, PouEntry, PouIndex, PouKind, RefEntry, RefLocation, RefSegment, RefTable,
    ResourceEntry, ResourceMeta, RetainInit, RetainInitEntry, Section, SectionData, SectionEntry,
    SectionId, StringTable, TypeData, TypeEntry, TypeKind, TypeTable, VarMeta, VarMetaEntry,
    HEADER_FLAG_CRC32, HEADER_SIZE, MAGIC, SECTION_ENTRY_SIZE, SUPPORTED_MAJOR_VERSION,
};

fn read_bounded_count(
    reader: &mut BytecodeReader<'_>,
    minimum_entry_bytes: usize,
    context: &str,
) -> Result<usize, BytecodeError> {
    debug_assert!(minimum_entry_bytes > 0);
    let count = reader.read_u32()? as usize;
    let required = count.checked_mul(minimum_entry_bytes).ok_or_else(|| {
        BytecodeError::InvalidSection(format!("{context} count exceeds section bounds").into())
    })?;
    if required > reader.remaining() {
        return Err(BytecodeError::InvalidSection(
            format!("{context} count exceeds section bounds").into(),
        ));
    }
    Ok(count)
}

include!("decode/module_decode.rs");
include!("decode/section_decode.rs");
include!("decode/string_type_decode.rs");
include!("decode/section_validate.rs");
