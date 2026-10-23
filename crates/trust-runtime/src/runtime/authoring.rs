//! Borrowed producer metadata; neither consumer gains mutable runtime access.

use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_hir::types::TypeRegistry;

use crate::{
    io::IoInterface,
    memory::{AccessMap, VariableStorage},
    program_model::{
        ClassDef, EdgeInput, FunctionBlockDef, FunctionDef, InitializerCatalog, InterfaceDef,
    },
    stdlib::StandardLibrary,
    task::{ProgramDef, TaskConfig},
    value::DateTimeProfile,
};

use super::{types::GlobalVarMeta, RuntimeBuilder};

/// Read-only inputs shared by the legacy and declaration-based producers.
#[derive(Clone, Copy)]
pub(crate) struct BytecodeAuthoringView<'a> {
    storage_abi: crate::value::StorageAbi,
    resource_name: &'a SmolStr,
    profile: DateTimeProfile,
    registry: &'a TypeRegistry,
    initializer_catalog: &'a InitializerCatalog,
    stdlib: &'a StandardLibrary,
    functions: &'a IndexMap<SmolStr, FunctionDef>,
    function_blocks: &'a IndexMap<SmolStr, FunctionBlockDef>,
    classes: &'a IndexMap<SmolStr, ClassDef>,
    interfaces: &'a IndexMap<SmolStr, InterfaceDef>,
    edge_inputs: &'a IndexMap<SmolStr, Vec<EdgeInput>>,
    programs: &'a IndexMap<SmolStr, ProgramDef>,
    globals: &'a IndexMap<SmolStr, GlobalVarMeta>,
    tasks: &'a [TaskConfig],
    storage: &'a VariableStorage,
    access: &'a AccessMap,
    io: &'a IoInterface,
}

impl<'a> From<&'a RuntimeBuilder> for BytecodeAuthoringView<'a> {
    fn from(builder: &'a RuntimeBuilder) -> Self {
        Self {
            storage_abi: builder.storage_abi,
            resource_name: &builder.resource_name,
            profile: builder.profile,
            registry: &builder.registry,
            initializer_catalog: &builder.initializer_catalog,
            stdlib: &builder.stdlib,
            functions: &builder.functions,
            function_blocks: &builder.function_blocks,
            classes: &builder.classes,
            interfaces: &builder.interfaces,
            edge_inputs: &builder.edge_inputs,
            programs: &builder.programs,
            globals: &builder.globals,
            tasks: &builder.tasks,
            storage: &builder.storage,
            access: &builder.access,
            io: &builder.io,
        }
    }
}

// Private historical dispatcher fixtures preserve the old corpus. They cannot
// enter the installed controller or a production authoring/execution path.
#[cfg(test)]
impl<'a> From<&'a super::core::Runtime> for BytecodeAuthoringView<'a> {
    fn from(runtime: &'a super::core::Runtime) -> Self {
        Self {
            storage_abi: crate::value::StorageAbi::LegacyTemporal32,
            resource_name: &runtime.resource_name,
            profile: runtime.profile,
            registry: &runtime.registry,
            initializer_catalog: &runtime.initializer_catalog,
            stdlib: &runtime.stdlib,
            functions: &runtime.functions,
            function_blocks: &runtime.function_blocks,
            classes: &runtime.classes,
            interfaces: &runtime.interfaces,
            edge_inputs: &runtime.edge_inputs,
            programs: &runtime.programs,
            globals: &runtime.globals,
            tasks: &runtime.tasks,
            storage: &runtime.storage,
            access: &runtime.access,
            io: runtime.io.interface(),
        }
    }
}

impl<'a> BytecodeAuthoringView<'a> {
    pub(crate) fn storage_abi(&self) -> crate::value::StorageAbi {
        self.storage_abi
    }
    pub(crate) fn resource_name(&self) -> &'a SmolStr {
        self.resource_name
    }
    pub(crate) fn profile(&self) -> DateTimeProfile {
        self.profile
    }
    pub(crate) fn registry(&self) -> &'a TypeRegistry {
        self.registry
    }
    pub(crate) fn initializer_catalog(&self) -> &'a InitializerCatalog {
        self.initializer_catalog
    }
    pub(crate) fn stdlib(&self) -> &'a StandardLibrary {
        self.stdlib
    }
    pub(crate) fn functions(&self) -> &'a IndexMap<SmolStr, FunctionDef> {
        self.functions
    }
    pub(crate) fn function_blocks(&self) -> &'a IndexMap<SmolStr, FunctionBlockDef> {
        self.function_blocks
    }
    pub(crate) fn classes(&self) -> &'a IndexMap<SmolStr, ClassDef> {
        self.classes
    }
    pub(crate) fn interfaces(&self) -> &'a IndexMap<SmolStr, InterfaceDef> {
        self.interfaces
    }
    pub(crate) fn programs(&self) -> &'a IndexMap<SmolStr, ProgramDef> {
        self.programs
    }
    pub(crate) fn globals(&self) -> &'a IndexMap<SmolStr, GlobalVarMeta> {
        self.globals
    }
    pub(crate) fn tasks(&self) -> &'a [TaskConfig] {
        self.tasks
    }
    pub(crate) fn storage(&self) -> &'a VariableStorage {
        self.storage
    }
    pub(crate) fn access_map(&self) -> &'a AccessMap {
        self.access
    }
    pub(crate) fn io(&self) -> &'a IoInterface {
        self.io
    }
    pub(crate) fn edge_inputs_for_pou(&self, owner: &str) -> &'a [EdgeInput] {
        self.edge_inputs.get(owner).map_or(&[], Vec::as_slice)
    }
}
