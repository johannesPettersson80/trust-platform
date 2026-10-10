//! Conservative logical preparation accounting, separate from measured heap peaks.
use super::PreparationLimits;
use crate::bytecode::*;
use crate::error::RuntimeError;
use crate::vm::module::{
    invalid_bytecode, VmNativeArgSpec, VmNativeSymbolSpec, VmParamMeta, VmPouEntry, VmRef,
};
use alloc::vec::Vec;
use smol_str::SmolStr;

/// Cumulative preparation demand; not allocator overhead, RSS or a timing bound.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PreparationUsage {
    /// Cumulative logical payload bytes charged during preparation.
    pub bytes: usize,
    /// Cumulative preparation work units, independent of elapsed wall time.
    pub work: usize,
}
pub(crate) struct PreparationBudget {
    limits: PreparationLimits,
    usage: PreparationUsage,
}
impl PreparationBudget {
    pub(crate) fn new(limits: PreparationLimits) -> Self {
        Self {
            limits,
            usage: PreparationUsage::default(),
        }
    }
    pub(crate) fn charge(&mut self, bytes: usize, work: usize) -> Result<(), RuntimeError> {
        let next = PreparationUsage {
            bytes: self
                .usage
                .bytes
                .checked_add(bytes)
                .ok_or(RuntimeError::Overflow)?,
            work: self
                .usage
                .work
                .checked_add(work)
                .ok_or(RuntimeError::Overflow)?,
        };
        if next.bytes > self.limits.max_preparation_bytes
            || next.work > self.limits.max_preparation_work
        {
            return Err(RuntimeError::PreparationLimit);
        }
        self.usage = next;
        Ok(())
    }
    pub(super) fn records<T>(&mut self, count: usize) -> Result<(), RuntimeError> {
        self.charge(
            count
                .checked_mul(core::mem::size_of::<T>())
                .ok_or(RuntimeError::Overflow)?,
            count,
        )
    }
    pub(super) fn map<K, V>(&mut self, count: usize) -> Result<(), RuntimeError> {
        // Logical payload plus four pointer-sized link/control words per entry.
        // Allocator buckets/node occupancy remain outside this accounting model.
        let width = core::mem::size_of::<K>()
            + core::mem::size_of::<V>()
            + 4 * core::mem::size_of::<usize>();
        self.charge(
            count.checked_mul(width).ok_or(RuntimeError::Overflow)?,
            count,
        )
    }
    pub(crate) fn metadata(
        &mut self,
        raw: &BytecodeModule,
        _encoded_bytes: usize,
    ) -> Result<(), RuntimeError> {
        let strings = match raw.section(SectionId::StringTable) {
            Some(SectionData::StringTable(v)) => v,
            _ => return Err(invalid_bytecode("missing STRING_TABLE")),
        };
        for section in &raw.sections {
            self.charge(0, 1)?;
            match &section.data {
                SectionData::StringTable(table) => {
                    self.records::<SmolStr>(table.entries.len())?;
                    self.records::<VmNativeSymbolSpec>(table.entries.len())?;
                    for text in &table.entries {
                        self.charge(
                            text.len().checked_mul(4).ok_or(RuntimeError::Overflow)?,
                            text.len().checked_mul(3).ok_or(RuntimeError::Overflow)?,
                        )?; // retained text, parsed names, uppercase scratch and normalized symbol
                        self.records::<VmNativeArgSpec>(
                            text.bytes().filter(|byte| *byte == b'|').count(),
                        )?;
                    }
                }
                SectionData::TypeTable(table) => {
                    self.records::<TypeEntry>(table.entries.len())?;
                    self.records::<u32>(table.offsets.len())?;
                    for entry in &table.entries {
                        match &entry.data {
                            TypeData::Array { dims, .. } => {
                                self.records::<(i64, i64)>(dims.len())?
                            }
                            TypeData::Struct { fields } | TypeData::Union { fields } => {
                                self.records::<Field>(fields.len())?
                            }
                            TypeData::Enum { variants, .. } => {
                                self.records::<EnumVariant>(variants.len())?
                            }
                            TypeData::Interface { methods } => {
                                self.records::<InterfaceMethod>(methods.len())?
                            }
                            _ => {}
                        }
                    }
                }
                SectionData::RefTable(table) => {
                    self.records::<VmRef>(table.entries.len())?;
                    for entry in &table.entries {
                        self.records::<crate::value::RefSegment>(entry.segments.len())?;
                        for segment in &entry.segments {
                            if let RefSegment::Index(indices) = segment {
                                self.records::<i64>(indices.len())?;
                            }
                        }
                    }
                }
                SectionData::PouIndex(table) => self.pou_metadata(raw, table, strings)?,
                SectionData::PouBodies(code) => self.records::<u8>(code.len())?,
                SectionData::VarMeta(table) => {
                    self.map::<u32, u32>(table.entries.len())?;
                    #[cfg(feature = "hir")]
                    {
                        self.map::<SmolStr, u32>(table.entries.len())?;
                        self.map::<u32, SmolStr>(table.entries.len())?;
                    }
                }
                #[cfg(feature = "hir")]
                SectionData::DebugMap(table) => self
                    .map::<(u32, u32), crate::vm::debug_map::VmSourceLocation>(
                        table.entries.len(),
                    )?,
                SectionData::StorageLayout(table) => {
                    self.records::<StorageDeclaration>(table.entries.len())?
                }
                SectionData::ConstructionRoots(table) => {
                    self.records::<ConstructionRoot>(table.entries.len())?
                }
                SectionData::Initializers(table) => {
                    self.records::<InitializerEntry>(table.entries.len())?;
                    self.records::<(u32, u32, Option<u32>, u32)>(
                        table
                            .entries
                            .iter()
                            .filter(|entry| entry.recipe_type_id.is_some())
                            .count(),
                    )?;
                }
                SectionData::AccessBindings(table) => {
                    self.records::<AccessBindingEntry>(table.entries.len())?
                }
                SectionData::ResourceMeta(table) => {
                    self.records::<ResourceEntry>(table.resources.len())?;
                    for resource in &table.resources {
                        self.records::<TaskEntry>(resource.tasks.len())?;
                        for task in &resource.tasks {
                            self.records::<u32>(task.program_name_idx.len())?;
                            self.records::<u32>(task.fb_ref_idx.len())?;
                        }
                    }
                }
                SectionData::IoMap(table) => self.io_metadata(raw, table, strings)?,
                // Constants use the charged decoder. Debug strings share SmolStr
                // storage; unknown sections and retain seeds are not retained here.
                _ => {}
            }
        }
        Ok(())
    }
    fn pou_metadata(
        &mut self,
        raw: &BytecodeModule,
        table: &PouIndex,
        strings: &StringTable,
    ) -> Result<(), RuntimeError> {
        let name_len = |id: u32| {
            strings
                .entries
                .get(id as usize)
                .map_or(0, |name| name.len())
        };
        for entry in &table.entries {
            self.map::<u32, VmPouEntry>(1)?;
            self.map::<u32, SmolStr>(1)?; // temporary identity/name index
            if entry.kind != PouKind::Method {
                // Function names remain a preparation-time native-resolution
                // index without HIR, even though only hosted adapters retain it.
                self.map::<SmolStr, u32>(1)?;
            }
            self.charge(name_len(entry.name_idx), name_len(entry.name_idx))?;
            self.map::<u32, Vec<VmParamMeta>>(1)?;
            self.records::<VmParamMeta>(entry.params.len())?;
            if entry.return_type_id.is_some() {
                self.map::<u32, ()>(1)?;
            }
            if entry.kind == PouKind::Method {
                self.map::<u32, u32>(1)?;
            }
            // infer_primary_instance_owner visits this actual body and
            // may retain one owner set entry for each reference opcode.
            let Some(SectionData::PouBodies(code)) = raw.section(SectionId::PouBodies) else {
                return Err(invalid_bytecode("missing POU_BODIES"));
            };
            let end = entry
                .code_offset
                .checked_add(entry.code_length)
                .ok_or(RuntimeError::Overflow)? as usize;
            let body = code
                .get(entry.code_offset as usize..end)
                .ok_or_else(|| invalid_bytecode("invalid POU body range"))?;
            let mut pc = 0usize;
            let mut references = 0usize;
            while pc < body.len() {
                self.charge(0, 1)?;
                let opcode = body[pc];
                let width = crate::vm::opcode_operand_len(opcode)
                    .ok_or_else(|| invalid_bytecode("invalid POU opcode"))?;
                if matches!(opcode, 0x20..=0x22) {
                    references += 1;
                }
                pc = pc
                    .checked_add(width + 1)
                    .filter(|next| *next <= body.len())
                    .ok_or_else(|| invalid_bytecode("invalid POU instruction extent"))?;
            }
            self.map::<u32, ()>(references)?;
            if let Some(class) = &entry.class_meta {
                if class.parent_pou_id.is_some() {
                    self.map::<u32, u32>(1)?;
                }
                self.map::<u32, Vec<u32>>(1)?;
                self.records::<u32>(class.interfaces.len())?;
                self.map::<u32, usize>(1)?;
                self.map::<SmolStr, u32>(class.methods.len())?;
                for method in &class.methods {
                    self.charge(name_len(method.name_idx), name_len(method.name_idx))?;
                }
            }
        }
        Ok(())
    }

    fn io_metadata(
        &mut self,
        raw: &BytecodeModule,
        table: &IoMap,
        strings: &StringTable,
    ) -> Result<(), RuntimeError> {
        self.records::<IoBinding>(table.bindings.len())?;
        self.records::<crate::io_image::PreparedIoBinding>(table.bindings.len())?;
        for binding in &table.bindings {
            let address = strings
                .entries
                .get(binding.address_str_idx as usize)
                .ok_or_else(|| invalid_bytecode("missing I/O address"))?;
            self.charge(0, address.len())?;
            self.records::<u32>(address.bytes().filter(|byte| *byte == b'.').count() + 1)?;
            if let Some(mut id) = binding.type_id {
                let Some(SectionData::TypeTable(types)) = raw.section(SectionId::TypeTable) else {
                    return Err(invalid_bytecode("missing TYPE_TABLE"));
                };
                for _ in 0..=BYTECODE_MAX_CONST_NESTING {
                    self.charge(0, 1)?;
                    let entry = types
                        .entries
                        .get(id as usize)
                        .ok_or_else(|| invalid_bytecode("invalid I/O type"))?;
                    match &entry.data {
                        TypeData::Alias { target_type_id } => id = *target_type_id,
                        TypeData::Subrange { base_type_id, .. } => id = *base_type_id,
                        TypeData::Enum {
                            base_type_id,
                            variants,
                        } => {
                            self.records::<(SmolStr, i64)>(variants.len())?;
                            id = *base_type_id;
                        }
                        _ => break,
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn remaining_bytes(&self) -> usize {
        self.limits.max_preparation_bytes - self.usage.bytes
    }
    pub(crate) fn remaining_work(&self) -> usize {
        self.limits.max_preparation_work - self.usage.work
    }
    pub(crate) fn usage(&self) -> PreparationUsage {
        self.usage
    }
}
