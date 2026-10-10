//! Decoded execution metadata shared by runtime compositions.
//!
//! Hosted metadata is exposed read-only. Source-free consumers receive only the
//! prepared application facade; raw metadata is never an admission token.

#[cfg(feature = "hir")]
use super::debug_map;
use super::{const_pool, materialization_limits, opcode_operand_len, symbols, VmTrap};
use crate::bytecode::{
    PouKind, RefEntry, RefLocation, RefTable, SectionData, SectionId, StringTable, TypeTable,
    VarMeta,
};
use crate::error::{PreparationDiagnostic, RuntimeError};
use crate::memory::IoArea;
use crate::value::{ref_indices_from_iter, RefPath, RefSegment as ValueRefSegment, Value};
#[cfg(not(feature = "std"))]
use alloc::collections::{BTreeMap as HashMap, BTreeSet as HashSet};
use alloc::vec::Vec;
use smol_str::SmolStr;
#[cfg(feature = "std")]
use std::collections::{HashMap, HashSet};

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
    pub(crate) code: Vec<u8>,
    pub(crate) strings: Vec<SmolStr>,
    pub(crate) types: TypeTable,
    pub(crate) refs: Vec<VmRef>,
    pub(crate) consts: Vec<Value>,
    pub(crate) pou_by_id: HashMap<u32, VmPouEntry>,
    pub(crate) program_ids: HashMap<SmolStr, u32>,
    #[cfg(feature = "hir")]
    pub(crate) function_ids: HashMap<SmolStr, u32>,
    pub(crate) function_block_ids: HashMap<SmolStr, u32>,
    pub(crate) class_ids: HashMap<SmolStr, u32>,
    pub(crate) parent_pou_ids: HashMap<u32, u32>,
    pub(crate) interface_type_ids_by_pou: HashMap<u32, Vec<u32>>,
    pub(crate) native_symbol_specs: Vec<VmNativeSymbolSpec>,
    pub(crate) pou_params: HashMap<u32, Vec<VmParamMeta>>,
    pub(crate) pou_has_return_slot: HashSet<u32>,
    pub(crate) method_table_by_owner: HashMap<u32, HashMap<SmolStr, u32>>,
    pub(crate) ref_types: HashMap<u32, u32>,
    #[cfg(feature = "hir")]
    pub(crate) debug_map: debug_map::VmDebugMap,
    pub(crate) instruction_budget: usize,
}

/// One encoded native-call argument descriptor.
#[derive(Debug, Clone)]
pub struct VmNativeArgSpec {
    /// Original parameter or symbol name.
    pub name: Option<SmolStr>,
    /// Whether this argument carries a writable target.
    pub is_target: bool,
}

/// Parsed native-call symbol or retained parse failure.
#[derive(Debug, Clone)]
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

#[cfg(feature = "hir")]
mod access;
#[cfg(feature = "hir")]
mod legacy;

impl VmModule {
    /// Materialize validated STBC 1.x for the hosted adapter; STBC 2.0 requires preparation.
    #[cfg(feature = "hir")]
    pub fn from_validated(
        module: &crate::bytecode::ValidatedBytecode<'_>,
    ) -> Result<Self, RuntimeError> {
        if module.view().version.major != 1 {
            return Err(invalid_bytecode(SmolStr::new_static(
                "STBC 2.0 construction and initializer execution requires the shared engine",
            )));
        }
        Self::materialize(module, &mut |_, _| Ok(()))
    }

    pub(crate) fn from_source_free(
        module: &crate::bytecode::ValidatedBytecode<'_>,
        budget: &mut super::prepared::PreparationBudget,
    ) -> Result<Self, RuntimeError> {
        if module.view().version.major != 2 {
            return Err(invalid_bytecode(SmolStr::new_static(
                "source-free execution requires STBC 2.0",
            )));
        }
        Self::materialize(module, &mut |bytes, work| budget.charge(bytes, work))
    }

    fn materialize(
        module: &crate::bytecode::ValidatedBytecode<'_>,
        charge: &mut impl FnMut(usize, usize) -> Result<(), RuntimeError>,
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
        let consts =
            const_pool::decode_const_pool_entries_charged(const_pool, types, strings, charge)?;
        let mut native_symbol_specs = strings
            .entries
            .iter()
            .map(symbols::preparse_native_symbol_spec)
            .collect::<Vec<_>>();

        let var_meta = match module.section(SectionId::VarMeta) {
            Some(SectionData::VarMeta(meta)) => Some(meta),
            _ => None,
        };
        let ref_types = build_ref_type_map(var_meta)?;
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

        let mut pou_by_id = HashMap::new();
        let mut program_ids = HashMap::new();
        let mut function_ids = HashMap::new();
        let mut function_block_ids = HashMap::new();
        let mut class_ids = HashMap::new();
        let mut parent_pou_ids = HashMap::new();
        let mut interface_type_ids_by_pou = HashMap::new();
        let mut pou_params = HashMap::new();
        let mut pou_has_return_slot = HashSet::new();
        let mut method_table_by_owner: HashMap<u32, HashMap<SmolStr, u32>> = HashMap::new();

        let mut pou_name_by_id: HashMap<u32, SmolStr> = HashMap::new();
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
            if pou_name_by_id.insert(entry.id, name).is_some() {
                return Err(PreparationDiagnostic::DuplicatePou(entry.id).into_runtime_error());
            }
        }

        for entry in &pou_index.entries {
            let name = pou_name_by_id.get(&entry.id).cloned().ok_or_else(|| {
                PreparationDiagnostic::MissingDecodedPou(entry.id).into_runtime_error()
            })?;
            let code_start = entry.code_offset as usize;
            let code_end = code_start
                .checked_add(entry.code_length as usize)
                .ok_or_else(|| invalid_bytecode(SmolStr::new_static("POU code range overflow")))?;
            if code_end > bodies.len() {
                return Err(PreparationDiagnostic::PouCodeRange(name.clone()).into_runtime_error());
            }
            let mut vm_entry = VmPouEntry {
                name: SmolStr::new(name.clone()),
                code_start,
                code_end,
                local_ref_start: entry.local_ref_start,
                local_ref_count: entry.local_ref_count,
                primary_instance_owner: None,
            };
            vm_entry.primary_instance_owner =
                infer_primary_instance_owner(&vm_entry, bodies, &refs);
            pou_by_id.insert(entry.id, vm_entry);

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
            pou_params.insert(entry.id, params);

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
                interface_type_ids_by_pou.insert(
                    owner,
                    class_meta
                        .interfaces
                        .iter()
                        .map(|interface| interface.interface_type_id)
                        .collect(),
                );
                let table = method_table_by_owner.entry(owner).or_default();
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
            }
        }
        symbols::resolve_native_symbol_specs(&mut native_symbol_specs, &function_ids);

        Ok(Self {
            version: module.view().version,
            code: bodies.clone(),
            strings: strings.entries.clone(),
            types: types.clone(),
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
        self.ref_types.get(&ref_idx).copied()
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
pub fn build_ref_type_map(var_meta: Option<&VarMeta>) -> Result<HashMap<u32, u32>, RuntimeError> {
    let Some(var_meta) = var_meta else {
        return Ok(HashMap::new());
    };
    let mut ref_types = HashMap::new();
    for entry in &var_meta.entries {
        if ref_types.insert(entry.ref_idx, entry.type_id).is_some() {
            return Err(invalid_bytecode(SmolStr::new_static(
                "duplicate VAR_META ref index",
            )));
        }
    }
    Ok(ref_types)
}

/// Code range and frame shape of one POU.
#[derive(Debug, Clone)]
pub struct VmPouEntry {
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
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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
