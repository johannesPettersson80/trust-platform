//! Bytecode encoder (runtime to container).

#![allow(missing_docs)]

mod codegen;
mod consts;
mod debug;
mod io;
mod locals;
mod pou;
mod refs;
mod types;
mod util;

use std::collections::{HashMap, HashSet};

use smol_str::SmolStr;

use crate::memory::InstanceId;
use crate::value::ValueRef;
use trust_hir::TypeId;

use self::util::normalize_name;
use super::encode::compute_type_offsets_for_entries;
use super::{
    BytecodeError, BytecodeModule, BytecodeVersion, ConstEntry, ConstPool, DebugMap,
    InterfaceMethod, MethodEntry, RefEntry, RefTable, Section, SectionData, SectionId, StringTable,
    TypeEntry, TypeTable, SUPPORTED_MAJOR_VERSION, SUPPORTED_MINOR_VERSION,
};

/// Build a bytecode module from the current host runtime state.
pub fn build_module_from_runtime(
    runtime: &crate::Runtime,
) -> Result<BytecodeModule, BytecodeError> {
    BytecodeEncoder::new(runtime).build()
}

/// Build a bytecode module from runtime state and source text used for debug maps.
pub fn build_module_from_runtime_with_sources(
    runtime: &crate::Runtime,
    sources: &[&str],
) -> Result<BytecodeModule, BytecodeError> {
    BytecodeEncoder::with_sources(runtime, sources).build()
}

/// Build a bytecode module from runtime state, source text, and source paths used for debug maps.
pub fn build_module_from_runtime_with_sources_and_paths(
    runtime: &crate::Runtime,
    sources: &[&str],
    paths: &[&str],
) -> Result<BytecodeModule, BytecodeError> {
    BytecodeEncoder::with_sources_and_paths(runtime, sources, paths).build()
}

impl BytecodeModule {
    pub fn from_runtime(runtime: &crate::Runtime) -> Result<Self, BytecodeError> {
        build_module_from_runtime(runtime)
    }

    pub fn from_runtime_with_sources(
        runtime: &crate::Runtime,
        sources: &[&str],
    ) -> Result<Self, BytecodeError> {
        build_module_from_runtime_with_sources(runtime, sources)
    }

    pub fn from_runtime_with_sources_and_paths(
        runtime: &crate::Runtime,
        sources: &[&str],
        paths: &[&str],
    ) -> Result<Self, BytecodeError> {
        build_module_from_runtime_with_sources_and_paths(runtime, sources, paths)
    }
}

#[derive(Default)]
struct StringInterner {
    entries: Vec<SmolStr>,
    index: HashMap<SmolStr, u32>,
}

impl StringInterner {
    fn intern(&mut self, value: impl Into<SmolStr>) -> u32 {
        let value = value.into();
        if let Some(idx) = self.index.get(&value) {
            return *idx;
        }
        let idx = self.entries.len() as u32;
        self.entries.push(value.clone());
        self.index.insert(value, idx);
        idx
    }

    fn into_table(self) -> StringTable {
        StringTable {
            entries: self.entries,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MethodKey {
    owner: SmolStr,
    name: SmolStr,
}

impl MethodKey {
    fn new(owner: &SmolStr, name: &SmolStr) -> Self {
        Self {
            owner: normalize_name(owner),
            name: normalize_name(name),
        }
    }
}

struct PouIdMap {
    next_id: u32,
    programs: HashMap<SmolStr, u32>,
    function_blocks: HashMap<SmolStr, u32>,
    functions: HashMap<SmolStr, u32>,
    classes: HashMap<SmolStr, u32>,
    methods: HashMap<MethodKey, u32>,
}

impl PouIdMap {
    fn build(runtime: &crate::Runtime) -> Self {
        let mut map = Self {
            next_id: 0,
            programs: HashMap::new(),
            function_blocks: HashMap::new(),
            functions: HashMap::new(),
            classes: HashMap::new(),
            methods: HashMap::new(),
        };

        for name in runtime.programs().keys() {
            let key = normalize_name(name);
            let id = map.alloc();
            map.programs.insert(key, id);
        }
        for name in runtime.function_blocks().keys() {
            let key = normalize_name(name);
            let id = map.alloc();
            map.function_blocks.insert(key, id);
        }
        for name in runtime.functions().keys() {
            let key = normalize_name(name);
            let id = map.alloc();
            map.functions.insert(key, id);
        }
        for name in runtime.classes().keys() {
            let key = normalize_name(name);
            let id = map.alloc();
            map.classes.insert(key, id);
        }
        for (owner, fb) in runtime.function_blocks().iter() {
            for method in &fb.methods {
                let key = MethodKey::new(owner, &method.name);
                let id = map.alloc();
                map.methods.insert(key, id);
            }
        }
        for (owner, class) in runtime.classes().iter() {
            for method in &class.methods {
                let key = MethodKey::new(owner, &method.name);
                let id = map.alloc();
                map.methods.insert(key, id);
            }
        }

        map
    }

    fn alloc(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    fn program_id(&self, name: &SmolStr) -> Option<u32> {
        let key = normalize_name(name);
        self.programs.get(&key).copied()
    }

    fn function_block_id(&self, name: &SmolStr) -> Option<u32> {
        let key = normalize_name(name);
        self.function_blocks.get(&key).copied()
    }

    fn function_id(&self, name: &SmolStr) -> Option<u32> {
        let key = normalize_name(name);
        self.functions.get(&key).copied()
    }

    fn class_id(&self, name: &SmolStr) -> Option<u32> {
        let key = normalize_name(name);
        self.classes.get(&key).copied()
    }

    fn class_like_id(&self, name: &SmolStr) -> Option<u32> {
        self.function_block_id(name).or_else(|| self.class_id(name))
    }

    fn method_id(&self, owner: &SmolStr, name: &SmolStr) -> Option<u32> {
        let key = MethodKey::new(owner, name);
        self.methods.get(&key).copied()
    }
}

struct BytecodeEncoder<'a> {
    runtime: &'a crate::Runtime,
    sources: Option<&'a [&'a str]>,
    paths: Option<&'a [&'a str]>,
    file_path_indices: HashMap<u32, u32>,
    strings: StringInterner,
    debug_strings: StringInterner,
    types: Vec<TypeEntry>,
    type_map: HashMap<TypeId, u32>,
    const_pool: Vec<ConstEntry>,
    ref_entries: Vec<RefEntry>,
    ref_map: HashMap<ValueRef, u32>,
    next_local_frame_id: u32,
    pou_ids: PouIdMap,
    stdlib_fbs: HashSet<SmolStr>,
    method_tables: HashMap<SmolStr, Vec<MethodEntry>>,
    method_stack: Vec<SmolStr>,
    interface_tables: HashMap<SmolStr, Vec<InterfaceMethod>>,
    interface_stack: Vec<SmolStr>,
}

#[derive(Clone, Default)]
struct CodegenContext {
    instance_id: Option<InstanceId>,
    return_name: Option<SmolStr>,
    using: Vec<SmolStr>,
    locals: HashMap<SmolStr, ValueRef>,
    static_refs: HashMap<SmolStr, ValueRef>,
    self_fields: HashMap<SmolStr, SmolStr>,
    for_temp_pairs: Vec<(SmolStr, SmolStr)>,
    next_for_temp: usize,
    loop_stack: Vec<LoopPatchState>,
}

#[derive(Clone, Default)]
struct LoopPatchState {
    continue_jumps: Vec<usize>,
    exit_jumps: Vec<usize>,
}

impl CodegenContext {
    fn new(
        instance_id: Option<InstanceId>,
        return_name: Option<SmolStr>,
        using: Vec<SmolStr>,
        mut locals: HashMap<SmolStr, ValueRef>,
        static_refs: HashMap<SmolStr, ValueRef>,
        self_fields: HashMap<SmolStr, SmolStr>,
        for_temp_pairs: Vec<(SmolStr, SmolStr)>,
    ) -> Self {
        if let Some(return_name) = return_name.as_ref() {
            if let Some((_, leaf)) = return_name.rsplit_once('.') {
                let qualified_key = normalize_name(return_name);
                if let Some(reference) = locals.get(&qualified_key).cloned() {
                    locals
                        .entry(normalize_name(&SmolStr::new(leaf)))
                        .or_insert(reference);
                }
            }
        }
        Self {
            instance_id,
            return_name,
            using,
            locals,
            static_refs,
            self_fields,
            for_temp_pairs,
            next_for_temp: 0,
            loop_stack: Vec::new(),
        }
    }

    fn local_ref(&self, name: &SmolStr) -> Option<&ValueRef> {
        let key = normalize_name(name);
        self.locals.get(&key)
    }

    fn static_ref(&self, name: &SmolStr) -> Option<&ValueRef> {
        let key = normalize_name(name);
        self.static_refs.get(&key)
    }

    fn self_field_name(&self, name: &SmolStr) -> Option<&SmolStr> {
        let key = normalize_name(name);
        self.self_fields.get(&key)
    }

    fn next_for_temp_pair(&mut self) -> Option<(SmolStr, SmolStr)> {
        let pair = self.for_temp_pairs.get(self.next_for_temp).cloned();
        if pair.is_some() {
            self.next_for_temp += 1;
        }
        pair
    }

    fn push_loop(&mut self) {
        self.loop_stack.push(LoopPatchState::default());
    }

    fn pop_loop(&mut self) -> Option<LoopPatchState> {
        self.loop_stack.pop()
    }

    fn record_continue_jump(&mut self, jump: usize) -> bool {
        let Some(loop_state) = self.loop_stack.last_mut() else {
            return false;
        };
        loop_state.continue_jumps.push(jump);
        true
    }

    fn record_exit_jump(&mut self, jump: usize) -> bool {
        let Some(loop_state) = self.loop_stack.last_mut() else {
            return false;
        };
        loop_state.exit_jumps.push(jump);
        true
    }
}

struct LocalScope {
    locals: HashMap<SmolStr, ValueRef>,
    local_ref_start: u32,
    local_ref_count: u32,
    for_temp_pairs: Vec<(SmolStr, SmolStr)>,
}

#[derive(Clone)]
enum AccessKind {
    Static(ValueRef),
    SelfField(SmolStr),
}

impl<'a> BytecodeEncoder<'a> {
    fn new(runtime: &'a crate::Runtime) -> Self {
        let stdlib_fbs: HashSet<SmolStr> = crate::stdlib::fbs::standard_function_blocks()
            .into_iter()
            .map(|fb| normalize_name(&fb.name))
            .collect();
        Self {
            runtime,
            sources: None,
            paths: None,
            file_path_indices: HashMap::new(),
            strings: StringInterner::default(),
            debug_strings: StringInterner::default(),
            types: Vec::new(),
            type_map: HashMap::new(),
            const_pool: Vec::new(),
            ref_entries: Vec::new(),
            ref_map: HashMap::new(),
            next_local_frame_id: 0,
            pou_ids: PouIdMap::build(runtime),
            stdlib_fbs,
            method_tables: HashMap::new(),
            method_stack: Vec::new(),
            interface_tables: HashMap::new(),
            interface_stack: Vec::new(),
        }
    }

    fn with_sources(runtime: &'a crate::Runtime, sources: &'a [&'a str]) -> Self {
        let mut encoder = Self::new(runtime);
        encoder.sources = Some(sources);
        encoder
    }

    fn with_sources_and_paths(
        runtime: &'a crate::Runtime,
        sources: &'a [&'a str],
        paths: &'a [&'a str],
    ) -> Self {
        let mut encoder = Self::with_sources(runtime, sources);
        encoder.paths = Some(paths);
        encoder
    }

    fn build(mut self) -> Result<BytecodeModule, BytecodeError> {
        self.collect_decl_types()?;
        let (pou_index, pou_bodies, debug_entries) = self.build_pou_index_and_bodies()?;
        let resource_meta = self.build_resource_meta()?;
        let io_map = self.build_io_map()?;
        let var_meta = self.build_var_meta()?;
        let retain_init = self.build_retain_init(&var_meta)?;
        let type_offsets = compute_type_offsets_for_entries(&self.types);
        let type_table = TypeTable {
            offsets: type_offsets,
            entries: self.types,
        };
        let const_pool = ConstPool {
            entries: self.const_pool,
        };
        let ref_table = RefTable {
            entries: self.ref_entries,
        };
        let string_table = self.strings.into_table();
        let debug_string_table = self.debug_strings.into_table();

        let mut sections = vec![
            Section {
                id: SectionId::StringTable.as_raw(),
                flags: 0,
                data: SectionData::StringTable(string_table),
            },
            Section {
                id: SectionId::TypeTable.as_raw(),
                flags: 0,
                data: SectionData::TypeTable(type_table),
            },
            Section {
                id: SectionId::ConstPool.as_raw(),
                flags: 0,
                data: SectionData::ConstPool(const_pool),
            },
            Section {
                id: SectionId::RefTable.as_raw(),
                flags: 0,
                data: SectionData::RefTable(ref_table),
            },
            Section {
                id: SectionId::PouIndex.as_raw(),
                flags: 0,
                data: SectionData::PouIndex(pou_index),
            },
            Section {
                id: SectionId::PouBodies.as_raw(),
                flags: 0,
                data: SectionData::PouBodies(pou_bodies),
            },
            Section {
                id: SectionId::ResourceMeta.as_raw(),
                flags: 0,
                data: SectionData::ResourceMeta(resource_meta),
            },
            Section {
                id: SectionId::IoMap.as_raw(),
                flags: 0,
                data: SectionData::IoMap(io_map),
            },
        ];
        if !var_meta.entries.is_empty() {
            sections.push(Section {
                id: SectionId::VarMeta.as_raw(),
                flags: 0,
                data: SectionData::VarMeta(var_meta),
            });
        }
        if !retain_init.entries.is_empty() {
            sections.push(Section {
                id: SectionId::RetainInit.as_raw(),
                flags: 0,
                data: SectionData::RetainInit(retain_init),
            });
        }
        if !debug_entries.is_empty() {
            sections.push(Section {
                id: SectionId::DebugStringTable.as_raw(),
                flags: 0,
                data: SectionData::DebugStringTable(debug_string_table),
            });
            sections.push(Section {
                id: SectionId::DebugMap.as_raw(),
                flags: 0,
                data: SectionData::DebugMap(DebugMap {
                    entries: debug_entries,
                }),
            });
        }

        let mut module = BytecodeModule::new(BytecodeVersion::new(
            SUPPORTED_MAJOR_VERSION,
            SUPPORTED_MINOR_VERSION,
        ));
        module.sections = sections;
        module.validate()?;
        Ok(module)
    }
}
