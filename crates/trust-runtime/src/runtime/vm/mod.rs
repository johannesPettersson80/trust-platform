use std::collections::{HashMap, HashSet};

use smol_str::SmolStr;

use crate::bytecode::{
    BytecodeModule, PouKind, RefEntry, RefLocation, RefTable, SectionData, SectionId, StringTable,
    TypeTable, VarMeta,
};
use crate::error::RuntimeError;
use crate::memory::IoArea;
use crate::task::ProgramDef;
use crate::value::{
    ref_indices_from_iter, RefPath, RefSegment as ValueRefSegment, Value, ValueRef,
};

mod budget;
mod call;
mod const_pool;
mod debug_map;
mod dispatch;
mod dispatch_ops;
mod dispatch_refs;
mod dispatch_sizeof;
mod edge;
mod errors;
mod frames;
mod limits;
mod local_init;
mod reference_attempt;
mod register_ir;
mod stack;
mod type_policy;

// VM module ownership notes (Phase B):
// - dispatch: instruction pointer loop + opcode routing + debug-hook emission.
// - dispatch_ops: arithmetic/logic execution helpers + operand/jump decoding.
// - dispatch_refs: ref/deref chain execution and storage bridge helpers.
// - dispatch_sizeof: TYPE_TABLE driven SIZEOF evaluation helpers.
// - const_pool: VM CONST_POOL decode + primitive literal materialization.
// - stack: operand stack invariants and overflow/underflow enforcement.
// - frames/call: call-stack and frame lifecycle.
// - errors: VM trap taxonomy and stable RuntimeError mapping.
// - debug_map: symbol/source lookup tables for external name/debug APIs.
// - register_ir: Phase A scaffold for stack-bytecode -> register-IR lowering + verifier.

use self::errors::VmTrap;
use super::core::Runtime;

pub(super) use local_init::VmLocalInitPlanCacheState;
pub(super) use register_ir::{
    RegisterLoweringCacheState, RegisterProfileState, RegisterTier1SpecializedExecutorState,
};
pub(super) use trust_runtime_core::vm::{materialize_borrowed_value, opcode_operand_len};

pub(super) const DEFAULT_INSTRUCTION_BUDGET: usize =
    trust_runtime_core::vm::VM_MAX_EXECUTED_INSTRUCTIONS;

pub(super) fn execute_program(
    runtime: &mut Runtime,
    program: &ProgramDef,
) -> Result<(), RuntimeError> {
    dispatch::execute_program(runtime, program)
}

pub(super) fn execute_program_by_name(
    runtime: &mut Runtime,
    program_name: &SmolStr,
) -> Result<(), RuntimeError> {
    dispatch::execute_program_by_name(runtime, program_name)
}

pub(super) fn execute_function_block_ref(
    runtime: &mut Runtime,
    reference: &ValueRef,
) -> Result<(), RuntimeError> {
    dispatch::execute_function_block_ref(runtime, reference)
}

#[derive(Debug, Clone)]
pub(super) struct VmModule {
    pub(super) code: Vec<u8>,
    pub(super) strings: Vec<SmolStr>,
    pub(super) types: TypeTable,
    pub(super) refs: Vec<VmRef>,
    pub(super) consts: Vec<Value>,
    pub(super) pou_by_id: HashMap<u32, VmPouEntry>,
    pub(super) program_ids: HashMap<SmolStr, u32>,
    pub(super) function_ids: HashMap<SmolStr, u32>,
    pub(super) function_block_ids: HashMap<SmolStr, u32>,
    pub(super) class_ids: HashMap<SmolStr, u32>,
    pub(super) parent_pou_ids: HashMap<u32, u32>,
    pub(super) interface_type_ids_by_pou: HashMap<u32, Vec<u32>>,
    native_symbol_specs: Vec<VmNativeSymbolSpec>,
    pou_params: HashMap<u32, Vec<VmParamMeta>>,
    pou_has_return_slot: HashSet<u32>,
    method_table_by_owner: HashMap<u32, HashMap<SmolStr, u32>>,
    ref_types: HashMap<u32, u32>,
    debug_map: debug_map::VmDebugMap,
    pub(super) instruction_budget: usize,
}

#[derive(Debug, Clone)]
pub(super) struct VmNativeArgSpec {
    pub(super) name: Option<SmolStr>,
    pub(super) is_target: bool,
}

#[derive(Debug, Clone)]
pub(super) enum VmNativeSymbolSpec {
    Parsed {
        target_name: SmolStr,
        normalized_target_name: SmolStr,
        resolved_function_pou_id: Option<u32>,
        conversion_spec: Option<crate::stdlib::conversions::ConversionSpec>,
        arg_specs: Vec<VmNativeArgSpec>,
    },
    ParseError(SmolStr),
}

impl VmModule {
    pub(super) fn from_bytecode(module: &BytecodeModule) -> Result<Self, RuntimeError> {
        let strings = match module.section(SectionId::StringTable) {
            Some(SectionData::StringTable(table)) => table,
            _ => return Err(invalid_bytecode("missing STRING_TABLE")),
        };
        let types = match module.section(SectionId::TypeTable) {
            Some(SectionData::TypeTable(table)) => table,
            _ => return Err(invalid_bytecode("missing TYPE_TABLE")),
        };
        let const_pool = match module.section(SectionId::ConstPool) {
            Some(SectionData::ConstPool(table)) => table,
            _ => return Err(invalid_bytecode("missing CONST_POOL")),
        };
        let ref_table = match module.section(SectionId::RefTable) {
            Some(SectionData::RefTable(table)) => table,
            _ => return Err(invalid_bytecode("missing REF_TABLE")),
        };
        let pou_index = match module.section(SectionId::PouIndex) {
            Some(SectionData::PouIndex(index)) => index,
            _ => return Err(invalid_bytecode("missing POU_INDEX")),
        };
        let bodies = match module.section(SectionId::PouBodies) {
            Some(SectionData::PouBodies(code)) => code,
            _ => return Err(invalid_bytecode("missing POU_BODIES")),
        };

        limits::validate_materialization_limits(ref_table, pou_index)?;
        let refs = decode_ref_table(ref_table, strings)?;
        let consts = const_pool::decode_const_pool_entries(const_pool, types, strings)?;
        let mut native_symbol_specs = strings
            .entries
            .iter()
            .map(call::preparse_native_symbol_spec)
            .collect::<Vec<_>>();

        let var_meta = match module.section(SectionId::VarMeta) {
            Some(SectionData::VarMeta(meta)) => Some(meta),
            _ => None,
        };
        let ref_types = build_ref_type_map(var_meta)?;
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
                    invalid_bytecode(format!("invalid POU name string index {}", entry.name_idx))
                })?;
            if pou_name_by_id.insert(entry.id, name).is_some() {
                return Err(invalid_bytecode(format!("duplicate POU id {}", entry.id)));
            }
        }

        for entry in &pou_index.entries {
            let name = pou_name_by_id.get(&entry.id).cloned().ok_or_else(|| {
                invalid_bytecode(format!("missing decoded POU name for id {}", entry.id))
            })?;
            let code_start = entry.code_offset as usize;
            let code_end = code_start + entry.code_length as usize;
            if code_end > bodies.len() {
                return Err(invalid_bytecode(format!(
                    "POU '{}' code range out of bounds",
                    name
                )));
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
                        invalid_bytecode(format!(
                            "invalid param name string index {}",
                            param.name_idx
                        ))
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
                    return Err(invalid_bytecode(format!("duplicate PROGRAM name '{key}'")));
                }
            } else if matches!(entry.kind, PouKind::FunctionBlock) {
                if function_block_ids.insert(key.clone(), entry.id).is_some() {
                    return Err(invalid_bytecode(format!(
                        "duplicate FUNCTION_BLOCK name '{key}'"
                    )));
                }
            } else if matches!(entry.kind, PouKind::Function) {
                if function_ids.insert(key.clone(), entry.id).is_some() {
                    return Err(invalid_bytecode(format!("duplicate FUNCTION name '{key}'")));
                }
            } else if matches!(entry.kind, PouKind::Class)
                && class_ids.insert(key.clone(), entry.id).is_some()
            {
                return Err(invalid_bytecode(format!("duplicate CLASS name '{key}'")));
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
                            invalid_bytecode(format!(
                                "invalid method name string index {}",
                                method.name_idx
                            ))
                        })?;
                    let method_key = SmolStr::new(method_name.to_ascii_uppercase());
                    if table.insert(method_key.clone(), method.pou_id).is_some() {
                        return Err(invalid_bytecode(format!(
                            "duplicate METHOD name '{method_key}' for owner POU {owner}"
                        )));
                    }
                }
            }
        }
        call::resolve_native_symbol_specs(&mut native_symbol_specs, &function_ids);

        Ok(Self {
            code: bodies.clone(),
            strings: strings.entries.clone(),
            types: types.clone(),
            refs,
            consts,
            pou_by_id,
            program_ids,
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
            debug_map,
            instruction_budget: DEFAULT_INSTRUCTION_BUDGET,
        })
    }

    pub(super) fn pou(&self, id: u32) -> Option<&VmPouEntry> {
        self.pou_by_id.get(&id)
    }

    pub(super) fn pou_name(&self, id: u32) -> Option<&str> {
        self.pou(id).map(|entry| entry.name.as_str())
    }

    pub(super) fn pou_params(&self, id: u32) -> Option<&[VmParamMeta]> {
        self.pou_params.get(&id).map(Vec::as_slice)
    }

    pub(super) fn pou_has_return_slot(&self, id: u32) -> bool {
        self.pou_has_return_slot.contains(&id)
    }

    pub(super) fn ref_type(&self, ref_idx: u32) -> Option<u32> {
        self.ref_types.get(&ref_idx).copied()
    }

    pub(super) fn resolve_method_pou_id_uppercase(
        &self,
        owner_pou_id: u32,
        method_name_upper: &str,
    ) -> Option<u32> {
        self.method_table_by_owner
            .get(&owner_pou_id)
            .and_then(|table| table.get(method_name_upper))
            .copied()
    }

    fn native_symbol_spec(&self, symbol_idx: u32) -> Result<&VmNativeSymbolSpec, VmTrap> {
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

fn build_ref_type_map(var_meta: Option<&VarMeta>) -> Result<HashMap<u32, u32>, RuntimeError> {
    let Some(var_meta) = var_meta else {
        return Ok(HashMap::new());
    };
    let mut ref_types = HashMap::new();
    for entry in &var_meta.entries {
        if ref_types.insert(entry.ref_idx, entry.type_id).is_some() {
            return Err(invalid_bytecode("duplicate VAR_META ref index"));
        }
    }
    Ok(ref_types)
}

#[derive(Debug, Clone)]
pub(super) struct VmPouEntry {
    pub(super) name: SmolStr,
    pub(super) code_start: usize,
    pub(super) code_end: usize,
    pub(super) local_ref_start: u32,
    pub(super) local_ref_count: u32,
    pub(super) primary_instance_owner: Option<u32>,
}

#[derive(Debug, Clone)]
pub(super) struct VmParamMeta {
    pub(super) name: SmolStr,
    pub(super) type_id: u32,
    pub(super) direction: u8,
    pub(super) default_const_idx: Option<u32>,
}

#[derive(Debug, Clone)]
pub(super) enum VmRef {
    Global {
        offset: usize,
        path: RefPath,
    },
    Local {
        owner_frame_id: u32,
        offset: usize,
        path: RefPath,
    },
    Instance {
        owner_instance_id: u32,
        offset: usize,
        path: RefPath,
    },
    Retain {
        offset: usize,
        path: RefPath,
    },
    Io {
        area: IoArea,
        offset: usize,
        path: RefPath,
    },
}

pub(super) fn invalid_bytecode(message: impl Into<SmolStr>) -> RuntimeError {
    RuntimeError::bytecode(crate::error::StableErrorCode::VmBytecodeDecode, message)
}

fn decode_ref_table(
    ref_table: &RefTable,
    strings: &StringTable,
) -> Result<Vec<VmRef>, RuntimeError> {
    let mut refs = Vec::with_capacity(ref_table.entries.len());
    for entry in &ref_table.entries {
        refs.push(decode_vm_ref(entry, strings)?);
    }
    Ok(refs)
}

fn decode_vm_ref(entry: &RefEntry, strings: &StringTable) -> Result<VmRef, RuntimeError> {
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
                        invalid_bytecode(format!("invalid ref field string index {name_idx}"))
                    })?;
                path.push(ValueRefSegment::Field(name));
            }
        }
    }

    let offset = entry.offset as usize;
    match entry.location {
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
                    return Err(invalid_bytecode(format!(
                        "invalid VM IO owner area {other}"
                    )));
                }
            };
            Ok(VmRef::Io { area, offset, path })
        }
    }
}

fn infer_primary_instance_owner(entry: &VmPouEntry, code: &[u8], refs: &[VmRef]) -> Option<u32> {
    let mut owners = HashSet::new();
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
                owners.insert(*owner_instance_id);
            }
        }
        pc += operand_len;
    }

    if owners.len() == 1 {
        owners.iter().copied().next()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::bytecode::{
        BytecodeVersion, ConstPool, MethodEntry, PouClassMeta, PouEntry, PouIndex,
        RefSegment as BytecodeRefSegment, Section, TypeTable, VarMetaEntry,
        SUPPORTED_MAJOR_VERSION, SUPPORTED_MINOR_VERSION,
    };
    use crate::error::StableErrorCode;
    use trust_hir::TypeId;

    #[test]
    fn vm_module_materialization_requires_every_execution_section() {
        let complete = bytecode_module(vec!["Main".into()], vec![pou(1, 0, PouKind::Program)]);

        for required in [
            SectionId::StringTable,
            SectionId::TypeTable,
            SectionId::ConstPool,
            SectionId::RefTable,
            SectionId::PouIndex,
            SectionId::PouBodies,
        ] {
            let mut missing = complete.clone();
            missing
                .sections
                .retain(|section| section.id != required.as_raw());
            let error = VmModule::from_bytecode(&missing)
                .expect_err("missing required execution section must reject");
            assert_eq!(error.stable_code(), StableErrorCode::VmBytecodeDecode);
        }
    }

    #[test]
    fn vm_module_materialization_preserves_case_insensitive_pou_metadata() {
        let mut class = pou(4, 3, PouKind::Class);
        class.class_meta = Some(PouClassMeta {
            parent_pou_id: None,
            interfaces: Vec::new(),
            methods: vec![MethodEntry {
                name_idx: 4,
                pou_id: 5,
                vtable_slot: 0,
                access: 0,
                flags: 0,
            }],
        });
        let mut function = pou(2, 1, PouKind::Function);
        function.return_type_id = Some(TypeId::DINT.0);
        function.params = vec![crate::bytecode::ParamEntry {
            name_idx: 5,
            type_id: TypeId::INT.0,
            direction: 2,
            default_const_idx: Some(7),
        }];
        let module = bytecode_module(
            vec![
                "MainProgram".into(),
                "Compute".into(),
                "MotorFb".into(),
                "MotorClass".into(),
                "Start".into(),
                "Input".into(),
            ],
            vec![
                pou(1, 0, PouKind::Program),
                function,
                pou(3, 2, PouKind::FunctionBlock),
                class,
                pou(5, 4, PouKind::Method),
            ],
        );

        let materialized = VmModule::from_bytecode(&module).unwrap();

        assert_eq!(materialized.program_ids.get("MAINPROGRAM"), Some(&1));
        assert_eq!(materialized.function_ids.get("COMPUTE"), Some(&2));
        assert_eq!(materialized.function_block_ids.get("MOTORFB"), Some(&3));
        assert_eq!(materialized.class_ids.get("MOTORCLASS"), Some(&4));
        assert_eq!(materialized.pou_name(1), Some("MainProgram"));
        assert!(materialized.pou_has_return_slot(2));
        let params = materialized.pou_params(2).expect("function parameters");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "Input");
        assert_eq!(params[0].type_id, TypeId::INT.0);
        assert_eq!(params[0].direction, 2);
        assert_eq!(params[0].default_const_idx, Some(7));
        assert_eq!(
            materialized.resolve_method_pou_id_uppercase(4, "START"),
            Some(5)
        );
        assert_eq!(
            materialized.resolve_method_pou_id_uppercase(3, "START"),
            None
        );
    }

    #[test]
    fn vm_module_materialization_rejects_duplicate_pou_ids_and_kind_names() {
        let duplicate_id = bytecode_module(
            vec!["First".into(), "Second".into()],
            vec![pou(1, 0, PouKind::Program), pou(1, 1, PouKind::Function)],
        );
        let error = VmModule::from_bytecode(&duplicate_id)
            .expect_err("one POU id cannot identify two declarations");
        assert_eq!(error.stable_code(), StableErrorCode::VmBytecodeDecode);
        assert!(error.to_string().contains("duplicate POU id"));

        let duplicate_name = bytecode_module(
            vec!["Main".into(), "mAiN".into()],
            vec![pou(1, 0, PouKind::Program), pou(2, 1, PouKind::Program)],
        );
        let error = VmModule::from_bytecode(&duplicate_name)
            .expect_err("one case-insensitive kind name cannot identify two POUs");
        assert_eq!(error.stable_code(), StableErrorCode::VmBytecodeDecode);
        assert!(error.to_string().contains("duplicate PROGRAM name"));
    }

    #[test]
    fn vm_module_materialization_rejects_duplicate_owner_local_method_names() {
        let mut class = pou(1, 0, PouKind::Class);
        class.class_meta = Some(PouClassMeta {
            parent_pou_id: None,
            interfaces: Vec::new(),
            methods: vec![
                MethodEntry {
                    name_idx: 1,
                    pou_id: 2,
                    vtable_slot: 0,
                    access: 0,
                    flags: 0,
                },
                MethodEntry {
                    name_idx: 2,
                    pou_id: 3,
                    vtable_slot: 1,
                    access: 0,
                    flags: 0,
                },
            ],
        });
        let module = bytecode_module(
            vec!["Motor".into(), "Start".into(), "sTaRt".into()],
            vec![
                class,
                pou(2, 1, PouKind::Method),
                pou(3, 2, PouKind::Method),
            ],
        );

        let error = VmModule::from_bytecode(&module)
            .expect_err("one owner cannot expose duplicate case-insensitive method names");
        assert_eq!(error.stable_code(), StableErrorCode::VmBytecodeDecode);
        assert!(error.to_string().contains("duplicate METHOD name"));
    }

    #[test]
    fn vm_ref_type_map_is_optional_and_rejects_duplicate_references() {
        assert!(build_ref_type_map(None).unwrap().is_empty());

        let metadata = VarMeta {
            entries: vec![var_meta(2, 5), var_meta(9, 16)],
        };
        let map = build_ref_type_map(Some(&metadata)).unwrap();
        assert_eq!(map.get(&2), Some(&5));
        assert_eq!(map.get(&9), Some(&16));

        let duplicate = VarMeta {
            entries: vec![var_meta(2, 5), var_meta(2, 16)],
        };
        let error = build_ref_type_map(Some(&duplicate))
            .expect_err("duplicate metadata for one reference must reject");
        assert_eq!(error.stable_code(), StableErrorCode::VmBytecodeDecode);
        assert!(error.to_string().contains("duplicate VAR_META ref index"));
    }

    #[test]
    fn vm_ref_decoder_preserves_locations_owners_offsets_and_paths() {
        let strings = StringTable {
            entries: vec!["Child".into()],
        };
        let global = decode_vm_ref(
            &reference(
                RefLocation::Global,
                99,
                4,
                vec![
                    BytecodeRefSegment::Index(vec![-3, 7]),
                    BytecodeRefSegment::Field { name_idx: 0 },
                ],
            ),
            &strings,
        )
        .unwrap();
        assert!(matches!(
            global,
            VmRef::Global { offset: 4, ref path }
                if matches!(
                    path.as_slice(),
                    [
                        ValueRefSegment::Index(indices),
                        ValueRefSegment::Field(field)
                    ] if indices.as_slice() == [-3, 7] && field == "Child"
                )
        ));

        assert!(matches!(
            decode_vm_ref(&reference(RefLocation::Local, 21, 5, Vec::new()), &strings).unwrap(),
            VmRef::Local {
                owner_frame_id: 21,
                offset: 5,
                ..
            }
        ));
        assert!(matches!(
            decode_vm_ref(
                &reference(RefLocation::Instance, 34, 6, Vec::new()),
                &strings
            )
            .unwrap(),
            VmRef::Instance {
                owner_instance_id: 34,
                offset: 6,
                ..
            }
        ));
        assert!(matches!(
            decode_vm_ref(&reference(RefLocation::Retain, 55, 7, Vec::new()), &strings).unwrap(),
            VmRef::Retain { offset: 7, .. }
        ));

        for (owner, area) in [(0, IoArea::Input), (1, IoArea::Output), (2, IoArea::Memory)] {
            assert!(matches!(
                decode_vm_ref(
                    &reference(RefLocation::Io, owner, 8, Vec::new()),
                    &strings
                )
                .unwrap(),
                VmRef::Io {
                    area: actual,
                    offset: 8,
                    ..
                } if actual == area
            ));
        }
    }

    #[test]
    fn vm_ref_decoder_rejects_invalid_io_owner_and_field_string() {
        let strings = StringTable::default();
        for entry in [
            reference(RefLocation::Io, 3, 0, Vec::new()),
            reference(
                RefLocation::Global,
                0,
                0,
                vec![BytecodeRefSegment::Field { name_idx: 7 }],
            ),
        ] {
            let error = decode_vm_ref(&entry, &strings).expect_err("invalid reference must reject");
            assert_eq!(error.stable_code(), StableErrorCode::VmBytecodeDecode);
        }
    }

    #[test]
    fn infer_primary_instance_owner_scans_partial_access_operands() {
        let mut code = vec![0x22];
        code.extend_from_slice(&0_u32.to_le_bytes());
        code.push(0x62);
        code.extend_from_slice(&0_u32.to_le_bytes());

        let entry = VmPouEntry {
            name: SmolStr::new("Main"),
            code_start: 0,
            code_end: code.len(),
            local_ref_start: 0,
            local_ref_count: 0,
            primary_instance_owner: None,
        };
        let refs = vec![VmRef::Instance {
            owner_instance_id: 42,
            offset: 0,
            path: RefPath::new(),
        }];

        assert_eq!(infer_primary_instance_owner(&entry, &code, &refs), Some(42));
    }

    #[test]
    fn infer_primary_instance_owner_returns_none_for_ambiguous_owners() {
        let mut code = vec![0x20];
        code.extend_from_slice(&0_u32.to_le_bytes());
        code.push(0x20);
        code.extend_from_slice(&1_u32.to_le_bytes());

        let entry = VmPouEntry {
            name: SmolStr::new("Main"),
            code_start: 0,
            code_end: code.len(),
            local_ref_start: 0,
            local_ref_count: 0,
            primary_instance_owner: None,
        };
        let refs = vec![
            VmRef::Instance {
                owner_instance_id: 42,
                offset: 0,
                path: RefPath::new(),
            },
            VmRef::Instance {
                owner_instance_id: 77,
                offset: 0,
                path: RefPath::new(),
            },
        ];

        assert_eq!(infer_primary_instance_owner(&entry, &code, &refs), None);
    }

    #[test]
    fn infer_primary_instance_owner_rejects_absent_unknown_and_truncated_code() {
        let refs = vec![VmRef::Instance {
            owner_instance_id: 42,
            offset: 0,
            path: RefPath::new(),
        }];

        for code in [vec![0x00], vec![0xff], vec![0x20, 0x00, 0x00]] {
            let entry = pou_entry(code.len());
            assert_eq!(infer_primary_instance_owner(&entry, &code, &refs), None);
        }
    }

    fn var_meta(ref_idx: u32, type_id: u32) -> VarMetaEntry {
        VarMetaEntry {
            name_idx: 0,
            type_id,
            ref_idx,
            retain: 0,
            init_const_idx: None,
        }
    }

    fn reference(
        location: RefLocation,
        owner_id: u32,
        offset: u32,
        segments: Vec<BytecodeRefSegment>,
    ) -> RefEntry {
        RefEntry {
            location,
            owner_id,
            offset,
            segments,
        }
    }

    fn pou_entry(code_end: usize) -> VmPouEntry {
        VmPouEntry {
            name: "Main".into(),
            code_start: 0,
            code_end,
            local_ref_start: 0,
            local_ref_count: 0,
            primary_instance_owner: None,
        }
    }

    fn bytecode_module(strings: Vec<SmolStr>, pous: Vec<PouEntry>) -> BytecodeModule {
        let mut module = BytecodeModule::new(BytecodeVersion::new(
            SUPPORTED_MAJOR_VERSION,
            SUPPORTED_MINOR_VERSION,
        ));
        module.sections = vec![
            section(
                SectionId::StringTable,
                SectionData::StringTable(StringTable { entries: strings }),
            ),
            section(
                SectionId::TypeTable,
                SectionData::TypeTable(TypeTable::default()),
            ),
            section(
                SectionId::ConstPool,
                SectionData::ConstPool(ConstPool::default()),
            ),
            section(
                SectionId::RefTable,
                SectionData::RefTable(RefTable::default()),
            ),
            section(
                SectionId::PouIndex,
                SectionData::PouIndex(PouIndex { entries: pous }),
            ),
            section(SectionId::PouBodies, SectionData::PouBodies(Vec::new())),
        ];
        module
    }

    fn section(id: SectionId, data: SectionData) -> Section {
        Section {
            id: id.as_raw(),
            flags: 0,
            data,
        }
    }

    fn pou(id: u32, name_idx: u32, kind: PouKind) -> PouEntry {
        PouEntry {
            id,
            name_idx,
            kind,
            code_offset: 0,
            code_length: 0,
            local_ref_start: 0,
            local_ref_count: 0,
            return_type_id: None,
            owner_pou_id: None,
            params: Vec::new(),
            class_meta: None,
        }
    }
}
