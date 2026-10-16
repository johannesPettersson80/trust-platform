//! Decoded execution metadata shared by runtime compositions.
//!
//! Hosted metadata is exposed read-only. Source-free consumers receive only the
//! prepared application facade; raw metadata is never an admission token.

#[cfg(feature = "hir")]
use super::debug_map;
use super::validated_source::ValidatedSource;
use super::{const_pool, materialization_limits, opcode_operand_len, symbols, VmTrap};
use crate::bytecode::{
    PouKind, RefEntry, RefLocation, RefTable, SectionData, SectionId, StringTable, TypeData,
    TypeTable, VarMeta,
};
use crate::error::{PreparationDiagnostic, RuntimeError};
use crate::memory::IoArea;
use crate::value::{ref_indices_from_iter, RefPath, RefSegment as ValueRefSegment, Value};
#[cfg(not(feature = "std"))]
use alloc::collections::{BTreeMap as HashMap, BTreeMap as IdMap, BTreeSet as IdSet};
use alloc::vec::Vec;
use smol_str::SmolStr;
// POU ids are integer keys that the call path looks up on every invocation, so
// hosted builds hash them with Fx rather than SipHash.
#[cfg(feature = "std")]
use rustc_hash::{FxHashMap as IdMap, FxHashSet as IdSet};
#[cfg(feature = "std")]
use std::collections::HashMap;

#[cfg(not(feature = "std"))]
mod pou_map;
#[cfg(not(feature = "std"))]
pub use pou_map::PouMap;
/// Hosted metadata keeps a hash map, with the Fx hasher.
#[cfg(feature = "std")]
pub type PouMap<V> = IdMap<u32, V>;

fn pou_map<V>(count: usize) -> Result<PouMap<V>, RuntimeError> {
    #[cfg(not(feature = "std"))]
    {
        PouMap::with_capacity(count)
    }
    #[cfg(feature = "std")]
    {
        let _ = count;
        Ok(IdMap::default())
    }
}

fn insert_pou<V>(map: &mut PouMap<V>, id: u32, value: V) -> Result<Option<V>, RuntimeError> {
    #[cfg(not(feature = "std"))]
    {
        map.try_insert(id, value)
    }
    #[cfg(feature = "std")]
    {
        Ok(map.insert(id, value))
    }
}

/// Immutable execution metadata for a validated legacy hosted module.
/// Source-free applications are admitted through `PreparedModule` instead.
#[cfg_attr(
    feature = "hir",
    doc = r#"
```compile_fail,E0616
use trust_runtime_core::vm::hosted::module::VmModule;
fn overwrite_code(module: &mut VmModule) {
    module.code.clear();
}
```
"#
)]
#[derive(Debug, Clone)]
pub struct VmModule {
    pub(crate) version: crate::bytecode::BytecodeVersion,
    pub(crate) storage_abi: crate::value::StorageAbi,
    pub(crate) code: Vec<u8>,
    pub(crate) strings: Vec<SmolStr>,
    pub(crate) types: TypeTable,
    pub(crate) physical_layouts:
        Vec<Option<alloc::sync::Arc<crate::value::overlap::PhysicalLayout>>>,
    pub(crate) refs: Vec<VmRef>,
    pub(crate) consts: Vec<Value>,
    pub(crate) pou_by_id: PouMap<VmPouEntry>,
    pub(crate) program_ids: HashMap<SmolStr, u32>,
    #[cfg(feature = "hir")]
    pub(crate) function_ids: HashMap<SmolStr, u32>,
    pub(crate) function_block_ids: HashMap<SmolStr, u32>,
    pub(crate) class_ids: HashMap<SmolStr, u32>,
    pub(crate) parent_pou_ids: IdMap<u32, u32>,
    pub(crate) interface_type_ids_by_pou: PouMap<Vec<u32>>,
    pub(crate) native_symbol_specs: Vec<VmNativeSymbolSpec>,
    pub(crate) pou_params: PouMap<Vec<VmParamMeta>>,
    pub(crate) pou_has_return_slot: IdSet<u32>,
    pub(crate) method_table_by_owner: PouMap<HashMap<SmolStr, u32>>,
    pub(crate) ref_types: Vec<Option<u32>>,
    #[cfg(feature = "hir")]
    pub(crate) debug_map: debug_map::VmDebugMap,
    pub(crate) instruction_budget: usize,
}

/// One encoded native-call argument descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmNativeArgSpec {
    /// Original parameter or symbol name.
    pub name: Option<SmolStr>,
    /// Whether this argument carries a writable target.
    pub is_target: bool,
}

/// Parsed native-call symbol or retained parse failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmNativeSymbolSpec {
    /// A successfully parsed native-call descriptor.
    Parsed {
        /// Original native target name.
        target_name: SmolStr,
        /// Canonical uppercase target name.
        normalized_target_name: SmolStr,
        /// Resolved bytecode function identity, when present.
        resolved_function_pou_id: Option<u32>,
        /// Recognized conversion descriptor, when present.
        conversion_spec: Option<crate::stdlib::conversions::ConversionSpec>,
        /// Ordered native argument descriptors.
        arg_specs: Vec<VmNativeArgSpec>,
    },
    /// Original diagnostic for a malformed encoded native symbol.
    ParseError(SmolStr),
}

#[cfg(any(feature = "hir", feature = "register-ir"))]
mod access;
#[cfg(feature = "hir")]
mod legacy;

impl VmModule {
    /// Immutable storage interpretation selected by the admitted artifact header.
    pub fn storage_abi(&self) -> crate::value::StorageAbi {
        self.storage_abi
    }
    pub(crate) fn physical_layout(
        &self,
        type_id: u32,
    ) -> Option<alloc::sync::Arc<crate::value::overlap::PhysicalLayout>> {
        let mut current = type_id;
        for _ in 0..=crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            let entry = self.types.entries.get(current as usize)?;
            if let TypeData::Alias { target_type_id } = entry.data {
                current = target_type_id;
            } else {
                return self.physical_layouts.get(current as usize)?.clone();
            }
        }
        None
    }

    /// Materialize validated STBC 1.x for the hosted adapter; 2.0/3.0 require preparation.
    #[cfg(feature = "hir")]
    pub fn from_validated(
        module: &crate::bytecode::ValidatedBytecode<'_>,
    ) -> Result<Self, RuntimeError> {
        if module.view().version.major != 1 {
            return Err(invalid_bytecode(SmolStr::new_static(
                "source-free construction and initializer execution requires the shared engine",
            )));
        }
        let mut source = *module;
        Self::materialize(&mut source, &mut |_, _| Ok(()))
    }

    pub(super) fn from_source_free(
        module: &mut impl ValidatedSource,
        budget: &mut super::prepared::PreparationBudget,
    ) -> Result<Self, RuntimeError> {
        if !module.view().version.is_source_free() {
            return Err(invalid_bytecode(SmolStr::new_static(
                "source-free execution requires STBC 2.0 or 3.0",
            )));
        }
        Self::materialize(module, &mut |bytes, work| budget.charge(bytes, work))
    }

    fn materialize(
        module: &mut impl ValidatedSource,
        charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
    ) -> Result<Self, RuntimeError> {
        let strings = match module.section(SectionId::StringTable) {
            Some(SectionData::StringTable(table)) => table,
            _ => {
                return Err(invalid_bytecode(SmolStr::new_static(
                    "missing STRING_TABLE",
                )))
            }
        };
        let types = match module.section(SectionId::TypeTable) {
            Some(SectionData::TypeTable(table)) => table,
            _ => return Err(invalid_bytecode(SmolStr::new_static("missing TYPE_TABLE"))),
        };
        let const_pool = match module.section(SectionId::ConstPool) {
            Some(SectionData::ConstPool(table)) => table,
            _ => return Err(invalid_bytecode(SmolStr::new_static("missing CONST_POOL"))),
        };
        let ref_table = match module.section(SectionId::RefTable) {
            Some(SectionData::RefTable(table)) => table,
            _ => return Err(invalid_bytecode(SmolStr::new_static("missing REF_TABLE"))),
        };
        let pou_index = match module.section(SectionId::PouIndex) {
            Some(SectionData::PouIndex(index)) => index,
            _ => return Err(invalid_bytecode(SmolStr::new_static("missing POU_INDEX"))),
        };
        let bodies = match module.section(SectionId::PouBodies) {
            Some(SectionData::PouBodies(code)) => code,
            _ => return Err(invalid_bytecode(SmolStr::new_static("missing POU_BODIES"))),
        };

        materialization_limits::validate_materialization_limits(ref_table, pou_index)?;
        let refs = decode_ref_table(ref_table, strings)?;
        let storage_abi = module.view().storage_abi().map_err(RuntimeError::from)?;
        let physical_layouts =
            crate::value::overlap::prepare_layouts(types, &strings.entries, storage_abi, charge)?;
        let consts = const_pool::decode_const_pool_entries_with_layouts(
            const_pool,
            types,
            strings,
            &physical_layouts,
            charge,
        )?;
        let mut native_symbol_specs = strings
            .entries
            .iter()
            .map(symbols::preparse_native_symbol_spec)
            .collect::<Vec<_>>();

        let var_meta = match module.section(SectionId::VarMeta) {
            Some(SectionData::VarMeta(meta)) => Some(meta),
            _ => None,
        };
        let ref_types = build_dense_ref_types(var_meta, refs.len())?;
        #[cfg(feature = "hir")]
        let debug_map = debug_map::VmDebugMap::from_sections(
            strings,
            var_meta,
            match module.section(SectionId::DebugStringTable) {
                Some(SectionData::DebugStringTable(table)) => Some(table),
                _ => None,
            },
            match module.section(SectionId::DebugMap) {
                Some(SectionData::DebugMap(map)) => Some(map),
                _ => None,
            },
        );

        let count = pou_index.entries.len();
        #[cfg(not(feature = "std"))]
        charge(0, count)?; // Count class payloads before exact vector reservations.
        #[cfg(not(feature = "std"))]
        let class_count = pou_index
            .entries
            .iter()
            .filter(|entry| entry.class_meta.is_some())
            .count();
        #[cfg(feature = "std")]
        let class_count = 0;
        let mut pou_by_id = pou_map(count)?;
        let mut program_ids = HashMap::default();
        let mut function_ids = HashMap::default();
        let mut function_block_ids = HashMap::default();
        let mut class_ids = HashMap::default();
        let mut parent_pou_ids = IdMap::default();
        let mut interface_type_ids_by_pou = pou_map(class_count)?;
        let mut pou_params = pou_map(count)?;
        let mut pou_has_return_slot = IdSet::default();
        let mut method_table_by_owner: PouMap<HashMap<SmolStr, u32>> = pou_map(class_count)?;

        // Admission has already checked POU identities and name indexes. Read
        // the canonical string table directly instead of rebuilding an id/name
        // map used only during this loop.
        for entry in &pou_index.entries {
            let name = strings
                .entries
                .get(entry.name_idx as usize)
                .cloned()
                .ok_or_else(|| {
                    PreparationDiagnostic::InvalidIndex {
                        kind: "POU name string",
                        index: entry.name_idx,
                    }
                    .into_runtime_error()
                })?;
            let code_start = entry.code_offset as usize;
            let code_end = code_start
                .checked_add(entry.code_length as usize)
                .ok_or_else(|| invalid_bytecode(SmolStr::new_static("POU code range overflow")))?;
            if code_end > bodies.len() {
                return Err(PreparationDiagnostic::PouCodeRange(name.clone()).into_runtime_error());
            }
            let mut vm_entry = VmPouEntry {
                kind: entry.kind,
                name: SmolStr::new(name.clone()),
                code_start,
                code_end,
                local_ref_start: entry.local_ref_start,
                local_ref_count: entry.local_ref_count,
                primary_instance_owner: None,
            };
            vm_entry.primary_instance_owner =
                infer_primary_instance_owner(&vm_entry, bodies, &refs);
            insert_pou(&mut pou_by_id, entry.id, vm_entry)?;

            if entry.return_type_id.is_some() {
                pou_has_return_slot.insert(entry.id);
            }
            let mut params = Vec::with_capacity(entry.params.len());
            for param in &entry.params {
                let param_name = strings
                    .entries
                    .get(param.name_idx as usize)
                    .cloned()
                    .ok_or_else(|| {
                        PreparationDiagnostic::InvalidIndex {
                            kind: "param name string",
                            index: param.name_idx,
                        }
                        .into_runtime_error()
                    })?;
                params.push(VmParamMeta {
                    name: param_name,
                    type_id: param.type_id,
                    direction: param.direction,
                    default_const_idx: param.default_const_idx,
                });
            }
            insert_pou(&mut pou_params, entry.id, params)?;

            let key = SmolStr::new(name.to_ascii_uppercase());
            if matches!(entry.kind, PouKind::Program) {
                if program_ids.insert(key.clone(), entry.id).is_some() {
                    return Err(PreparationDiagnostic::DuplicateName {
                        kind: "PROGRAM",
                        name: key.clone(),
                    }
                    .into_runtime_error());
                }
            } else if matches!(entry.kind, PouKind::FunctionBlock) {
                if function_block_ids.insert(key.clone(), entry.id).is_some() {
                    return Err(PreparationDiagnostic::DuplicateName {
                        kind: "FUNCTION_BLOCK",
                        name: key.clone(),
                    }
                    .into_runtime_error());
                }
            } else if matches!(entry.kind, PouKind::Function) {
                if function_ids.insert(key.clone(), entry.id).is_some() {
                    return Err(PreparationDiagnostic::DuplicateName {
                        kind: "FUNCTION",
                        name: key.clone(),
                    }
                    .into_runtime_error());
                }
            } else if matches!(entry.kind, PouKind::Class)
                && class_ids.insert(key.clone(), entry.id).is_some()
            {
                return Err(PreparationDiagnostic::DuplicateName {
                    kind: "CLASS",
                    name: key.clone(),
                }
                .into_runtime_error());
            }

            if let Some(class_meta) = &entry.class_meta {
                let owner = entry.id;
                if let Some(parent) = class_meta.parent_pou_id {
                    parent_pou_ids.insert(owner, parent);
                }
                insert_pou(
                    &mut interface_type_ids_by_pou,
                    owner,
                    class_meta
                        .interfaces
                        .iter()
                        .map(|interface| interface.interface_type_id)
                        .collect(),
                )?;
                let mut table = HashMap::default();
                for method in &class_meta.methods {
                    let method_name = strings
                        .entries
                        .get(method.name_idx as usize)
                        .cloned()
                        .ok_or_else(|| {
                            PreparationDiagnostic::InvalidIndex {
                                kind: "method name string",
                                index: method.name_idx,
                            }
                            .into_runtime_error()
                        })?;
                    let method_key = SmolStr::new(method_name.to_ascii_uppercase());
                    if table.insert(method_key.clone(), method.pou_id).is_some() {
                        return Err(PreparationDiagnostic::DuplicateMethod {
                            name: method_key.clone(),
                            owner,
                        }
                        .into_runtime_error());
                    }
                }
                insert_pou(&mut method_table_by_owner, owner, table)?;
            }
        }
        symbols::resolve_native_symbol_specs(&mut native_symbol_specs, &function_ids);

        Ok(Self {
            version: module.view().version,
            storage_abi,
            code: module
                .take_bodies()
                .ok_or_else(|| invalid_bytecode(SmolStr::new_static("missing POU_BODIES")))?,
            strings: module
                .take_strings()
                .ok_or_else(|| invalid_bytecode(SmolStr::new_static("missing STRING_TABLE")))?
                .entries,
            types: module
                .take_types()
                .ok_or_else(|| invalid_bytecode(SmolStr::new_static("missing TYPE_TABLE")))?,
            physical_layouts,
            refs,
            consts,
            pou_by_id,
            program_ids,
            #[cfg(feature = "hir")]
            function_ids,
            function_block_ids,
            class_ids,
            parent_pou_ids,
            interface_type_ids_by_pou,
            native_symbol_specs,
            pou_params,
            pou_has_return_slot,
            method_table_by_owner,
            ref_types,
            #[cfg(feature = "hir")]
            debug_map,
            instruction_budget: super::VM_MAX_EXECUTED_INSTRUCTIONS,
        })
    }

    /// Look up a POU by artifact identity.
    pub fn pou(&self, id: u32) -> Option<&VmPouEntry> {
        self.pou_by_id.get(&id)
    }

    /// Return the original POU name.
    pub fn pou_name(&self, id: u32) -> Option<&str> {
        self.pou(id).map(|entry| entry.name.as_str())
    }

    /// Return ordered parameter metadata for a POU.
    pub fn pou_params(&self, id: u32) -> Option<&[VmParamMeta]> {
        self.pou_params.get(&id).map(Vec::as_slice)
    }

    /// Whether a POU reserves a local return slot.
    pub fn pou_has_return_slot(&self, id: u32) -> bool {
        self.pou_has_return_slot.contains(&id)
    }

    /// Return the declared type of a reference when metadata supplies it.
    pub fn ref_type(&self, ref_idx: u32) -> Option<u32> {
        usize::try_from(ref_idx)
            .ok()
            .and_then(|index| self.ref_types.get(index).copied().flatten())
    }

    /// Resolve a canonical uppercase method name within its owner.
    pub fn resolve_method_pou_id_uppercase(
        &self,
        owner_pou_id: u32,
        method_name_upper: &str,
    ) -> Option<u32> {
        self.method_table_by_owner
            .get(&owner_pou_id)
            .and_then(|table| table.get(method_name_upper))
            .copied()
    }

    /// Return a parsed native-call descriptor or its preserved parse error.
    pub fn native_symbol_spec(&self, symbol_idx: u32) -> Result<&VmNativeSymbolSpec, VmTrap> {
        let entry = self
            .native_symbol_specs
            .get(symbol_idx as usize)
            .ok_or(VmTrap::InvalidNativeSymbolIndex(symbol_idx))?;
        match entry {
            VmNativeSymbolSpec::Parsed { .. } => Ok(entry),
            VmNativeSymbolSpec::ParseError(message) => {
                Err(VmTrap::InvalidNativeCall(message.clone()))
            }
        }
    }
}

/// Build declared reference types, rejecting duplicate reference metadata.
#[cfg(feature = "hir")]
pub fn build_ref_type_map(var_meta: Option<&VarMeta>) -> Result<HashMap<u32, u32>, RuntimeError> {
    let Some(var_meta) = var_meta else {
        return Ok(HashMap::default());
    };
    let mut ref_types = HashMap::default();
    for entry in &var_meta.entries {
        if ref_types.insert(entry.ref_idx, entry.type_id).is_some() {
            return Err(invalid_bytecode(SmolStr::new_static(
                "duplicate VAR_META ref index",
            )));
        }
    }
    Ok(ref_types)
}

/// Build the runtime reference-type table directly by validated REF_TABLE ordinal.
fn build_dense_ref_types(
    var_meta: Option<&VarMeta>,
    reference_count: usize,
) -> Result<Vec<Option<u32>>, RuntimeError> {
    let mut ref_types = Vec::new();
    ref_types
        .try_reserve_exact(reference_count)
        .map_err(|_| RuntimeError::PreparationLimit)?;
    ref_types.resize(reference_count, None);

    if let Some(var_meta) = var_meta {
        for entry in &var_meta.entries {
            let index = usize::try_from(entry.ref_idx).map_err(|_| {
                invalid_bytecode(SmolStr::new_static("VAR_META ref index out of range"))
            })?;
            let slot = ref_types.get_mut(index).ok_or_else(|| {
                invalid_bytecode(SmolStr::new_static("VAR_META ref index out of range"))
            })?;
            if slot.replace(entry.type_id).is_some() {
                return Err(invalid_bytecode(SmolStr::new_static(
                    "duplicate VAR_META ref index",
                )));
            }
        }
    }
    Ok(ref_types)
}

/// Code range and frame shape of one POU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmPouEntry {
    /// Closed POU category retained from the admitted descriptor.
    pub kind: crate::bytecode::PouKind,
    /// Original parameter or symbol name.
    pub name: SmolStr,
    /// Inclusive byte offset of the POU body.
    pub code_start: usize,
    /// Exclusive byte offset of the POU body.
    pub code_end: usize,
    /// First reference-table index owned by this frame.
    pub local_ref_start: u32,
    /// Number of local slots in the frame.
    pub local_ref_count: u32,
    /// Artifact instance owner used to bind the active receiver.
    pub primary_instance_owner: Option<u32>,
}

/// Declared parameter binding metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmParamMeta {
    /// Original parameter or symbol name.
    pub name: SmolStr,
    /// Declared TYPE_TABLE identity.
    pub type_id: u32,
    /// Encoded parameter direction: input, output or in-out.
    pub direction: u8,
    /// Optional CONST_POOL entry for a legacy parameter default.
    pub default_const_idx: Option<u32>,
}

/// Decoded reference descriptor; runtime contexts resolve artifact owner identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmRef {
    /// Active initializer staging result.
    InitializerResult {
        /// Initializer whose active staging result is addressed.
        initializer_id: u32,
        /// Ordered aggregate selections below the root.
        path: RefPath,
    },
    /// Resource-global storage.
    Global {
        /// Slot index within the addressed storage domain.
        offset: usize,
        /// Ordered aggregate selections below the root.
        path: RefPath,
    },
    /// Frame-local storage.
    Local {
        /// Artifact frame owner, resolved against the active call frame.
        owner_frame_id: u32,
        /// Slot index within the addressed storage domain.
        offset: usize,
        /// Ordered aggregate selections below the root.
        path: RefPath,
    },
    /// Instance member storage.
    Instance {
        /// Artifact instance owner, resolved through the execution context.
        owner_instance_id: u32,
        /// Slot index within the addressed storage domain.
        offset: usize,
        /// Ordered aggregate selections below the root.
        path: RefPath,
    },
    /// Raw retain-domain reference; source-free admission may reject this profile.
    Retain {
        /// Slot index within the addressed storage domain.
        offset: usize,
        /// Ordered aggregate selections below the root.
        path: RefPath,
    },
    /// Raw process-image reference; distinct from ordinary storage with I/O bindings.
    Io {
        /// Addressed process-image area.
        area: IoArea,
        /// Slot index within the addressed storage domain.
        offset: usize,
        /// Ordered aggregate selections below the root.
        path: RefPath,
    },
}

/// Create a stable VM decode failure with diagnostic detail.
#[cold]
pub fn invalid_bytecode(message: impl Into<SmolStr>) -> RuntimeError {
    invalid_bytecode_detail(message.into())
}

#[cold]
#[inline(never)]
fn invalid_bytecode_detail(message: SmolStr) -> RuntimeError {
    RuntimeError::bytecode(crate::error::StableErrorCode::VmBytecodeDecode, message)
}

/// Decode all reference descriptors against the string table.
pub fn decode_ref_table(
    ref_table: &RefTable,
    strings: &StringTable,
) -> Result<Vec<VmRef>, RuntimeError> {
    let mut refs = Vec::with_capacity(ref_table.entries.len());
    for entry in &ref_table.entries {
        refs.push(decode_vm_ref(entry, strings)?);
    }
    Ok(refs)
}

/// Decode one descriptor and its aggregate selections.
pub fn decode_vm_ref(entry: &RefEntry, strings: &StringTable) -> Result<VmRef, RuntimeError> {
    let mut path = RefPath::with_capacity(entry.segments.len());
    for segment in &entry.segments {
        match segment {
            crate::bytecode::RefSegment::Index(indices) => {
                path.push(ValueRefSegment::Index(ref_indices_from_iter(
                    indices.iter().copied(),
                )));
            }
            crate::bytecode::RefSegment::Field { name_idx } => {
                let name = strings
                    .entries
                    .get(*name_idx as usize)
                    .cloned()
                    .ok_or_else(|| {
                        PreparationDiagnostic::InvalidIndex {
                            kind: "ref field string",
                            index: *name_idx,
                        }
                        .into_runtime_error()
                    })?;
                path.push(ValueRefSegment::Field(name));
            }
        }
    }

    let offset = entry.offset as usize;
    match entry.location {
        RefLocation::InitializerResult => Ok(VmRef::InitializerResult {
            initializer_id: entry.owner_id,
            path,
        }),
        RefLocation::Global => Ok(VmRef::Global { offset, path }),
        RefLocation::Local => Ok(VmRef::Local {
            owner_frame_id: entry.owner_id,
            offset,
            path,
        }),
        RefLocation::Instance => Ok(VmRef::Instance {
            owner_instance_id: entry.owner_id,
            offset,
            path,
        }),
        RefLocation::Retain => Ok(VmRef::Retain { offset, path }),
        RefLocation::Io => {
            let area = match entry.owner_id {
                0 => IoArea::Input,
                1 => IoArea::Output,
                2 => IoArea::Memory,
                other => {
                    return Err(PreparationDiagnostic::InvalidIoArea(other).into_runtime_error());
                }
            };
            Ok(VmRef::Io { area, offset, path })
        }
    }
}

/// Inspect a POU body for its instance owner used by hosted frame binding.
pub fn infer_primary_instance_owner(
    entry: &VmPouEntry,
    code: &[u8],
    refs: &[VmRef],
) -> Option<u32> {
    let mut owner = None;
    let mut conflict = false;
    let mut pc = entry.code_start;
    while pc < entry.code_end {
        let opcode = *code.get(pc)?;
        pc += 1;
        let operand_len = opcode_operand_len(opcode)?;
        if pc + operand_len > entry.code_end {
            return None;
        }
        if matches!(opcode, 0x20..=0x22) && operand_len == 4 {
            let bytes = [code[pc], code[pc + 1], code[pc + 2], code[pc + 3]];
            let ref_idx = u32::from_le_bytes(bytes);
            if let Some(VmRef::Instance {
                owner_instance_id, ..
            }) = refs.get(ref_idx as usize)
            {
                match owner {
                    None => owner = Some(*owner_instance_id),
                    Some(previous) if previous != *owner_instance_id => conflict = true,
                    Some(_) => (),
                }
            }
        }
        pc += operand_len;
    }

    if conflict {
        None
    } else {
        owner
    }
}

#[cfg(test)]
mod owner_inference_tests {
    use super::*;
    use alloc::vec;

    fn infer(code: &[u8], refs: &[VmRef]) -> Option<u32> {
        let entry = VmPouEntry {
            kind: crate::bytecode::PouKind::Function,
            name: "Main".into(),
            code_start: 0,
            code_end: code.len(),
            local_ref_start: 0,
            local_ref_count: 0,
            primary_instance_owner: None,
        };
        infer_primary_instance_owner(&entry, code, refs)
    }

    #[test]
    fn scalar_owner_tracking_matches_unique_set_semantics() {
        let refs = [
            VmRef::Instance {
                owner_instance_id: 0,
                offset: 0,
                path: RefPath::new(),
            },
            VmRef::Instance {
                owner_instance_id: 0,
                offset: 1,
                path: RefPath::new(),
            },
            VmRef::Instance {
                owner_instance_id: u32::MAX,
                offset: 0,
                path: RefPath::new(),
            },
            VmRef::Global {
                offset: 0,
                path: RefPath::new(),
            },
        ];
        for (indices, expected) in [
            (&[][..], None),
            (&[0][..], Some(0)),
            (&[2][..], Some(u32::MAX)),
            (&[0, 1, 0][..], Some(0)),
            (&[0, 2, 0][..], None),
            (&[3, 2, 3][..], Some(u32::MAX)),
            (&[3][..], None),
            (&[99][..], None),
        ] {
            let mut code = Vec::new();
            for (position, index) in indices.iter().enumerate() {
                code.push([0x20, 0x21, 0x22][position % 3]);
                code.extend_from_slice(&u32::to_le_bytes(*index));
            }
            assert_eq!(infer(&code, &refs), expected);
        }
    }

    #[test]
    fn null_literals_do_not_hide_owner_but_unknown_and_truncated_operands_do() {
        let refs = [VmRef::Instance {
            owner_instance_id: 42,
            offset: 0,
            path: RefPath::new(),
        }];
        let mut code = vec![0x25, 0x20, 0, 0, 0, 0, 0x25];
        assert_eq!(infer(&code, &refs), Some(42));
        code.push(0xff);
        assert_eq!(infer(&code, &refs), None);
        for opcode in [0x20, 0x21, 0x22] {
            let code = [opcode, 0, 0, 0, 0];
            for length in 1..5 {
                assert_eq!(infer(&code[..length], &refs), None);
            }
        }
    }
}

#[cfg(test)]
mod dense_ref_type_tests {
    use super::build_dense_ref_types;
    use crate::bytecode::{VarMeta, VarMetaEntry};
    use crate::error::StableErrorCode;
    use alloc::vec;

    fn entry(ref_idx: u32, type_id: u32) -> VarMetaEntry {
        VarMetaEntry {
            name_idx: 0,
            type_id,
            ref_idx,
            retain: 0,
            init_const_idx: None,
        }
    }

    #[test]
    fn dense_ref_types_preserve_ordinals_and_absent_metadata() {
        let metadata = VarMeta {
            entries: vec![entry(2, 7), entry(0, 3)],
        };
        assert_eq!(
            build_dense_ref_types(Some(&metadata), 4).unwrap(),
            vec![Some(3), None, Some(7), None]
        );
        assert_eq!(build_dense_ref_types(None, 3).unwrap(), vec![None; 3]);
    }

    #[test]
    fn dense_ref_types_reject_duplicate_and_out_of_range_indices() {
        for metadata in [
            VarMeta {
                entries: vec![entry(1, 2), entry(1, 3)],
            },
            VarMeta {
                entries: vec![entry(2, 2)],
            },
        ] {
            let error = build_dense_ref_types(Some(&metadata), 2).unwrap_err();
            assert_eq!(error.stable_code(), StableErrorCode::VmBytecodeDecode);
        }
    }
}

#[cfg(test)]
mod invalid_bytecode_tests {
    use super::invalid_bytecode;
    use crate::error::StableErrorCode;
    use alloc::string::ToString;
    use smol_str::SmolStr;

    #[test]
    fn shared_decode_error_constructor_preserves_owned_and_borrowed_messages() {
        let detail = "POU code range overflow";
        let borrowed = invalid_bytecode(detail);
        assert_eq!(borrowed.stable_code(), StableErrorCode::VmBytecodeDecode);
        assert_eq!(
            borrowed.to_string(),
            "invalid bytecode 'POU code range overflow'"
        );
        assert_eq!(invalid_bytecode(detail.to_string()), borrowed);
        assert_eq!(invalid_bytecode(SmolStr::new_static(detail)), borrowed);
    }
}
