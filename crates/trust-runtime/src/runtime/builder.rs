//! The declarations the front of the build lowers a project to (specification 44 §3).

use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_hir::{types::TypeRegistry, Type};

use crate::{
    debug::SourceLocation,
    memory::VariableStorage,
    program_model::{
        ClassDef, EdgeInput, FunctionBlockDef, FunctionDef, InitializerCatalog, InterfaceDef,
    },
    stdlib::StandardLibrary,
    task::{ProgramDef, TaskConfig},
    value::DateTimeProfile,
};

/// The declarations of a project while the front of the build lowers them (specification 44 §3): its
/// types, POUs, globals, program instances and tasks, with the starting values the build computes.
///
/// This object has no cycle, execution, watchdog or debugger-control API.
///
/// ```compile_fail,E0599
/// let mut declarations = trust_runtime::RuntimeBuilder::new();
/// declarations.execute_cycle().unwrap();
/// ```
pub struct RuntimeBuilder {
    pub(super) resource_name: SmolStr,
    pub(super) profile: DateTimeProfile,
    pub(super) registry: TypeRegistry,
    pub(super) initializer_catalog: InitializerCatalog,
    pub(super) stdlib: StandardLibrary,
    pub(super) functions: IndexMap<SmolStr, FunctionDef>,
    pub(super) function_blocks: IndexMap<SmolStr, FunctionBlockDef>,
    pub(super) classes: IndexMap<SmolStr, ClassDef>,
    pub(super) interfaces: IndexMap<SmolStr, InterfaceDef>,
    pub(super) edge_inputs: IndexMap<SmolStr, Vec<EdgeInput>>,
    pub(super) programs: IndexMap<SmolStr, ProgramDef>,
    /// The declared globals, each with its declared type.
    pub(super) globals: IndexMap<SmolStr, trust_hir::TypeId>,
    /// Globals declared `VAR_GLOBAL CONSTANT`, which nothing outside the program may write.
    constant_globals: indexmap::IndexSet<SmolStr>,
    /// The program type of each configured program instance, by the instance's name in upper case.
    program_types: IndexMap<SmolStr, SmolStr>,
    pub(super) tasks: Vec<TaskConfig>,
    pub(super) storage: VariableStorage,
    /// The `VAR_ACCESS` declarations, by the names of the program's symbols.
    pub(super) access: Vec<crate::harness::NamedAccess>,
    pub(super) io: crate::io::IoInterface,
    pub(super) statement_index: IndexMap<u32, Vec<SourceLocation>>,
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        let mut builder = Self {
            resource_name: "RESOURCE".into(),
            profile: DateTimeProfile::default(),
            registry: TypeRegistry::new(),
            initializer_catalog: InitializerCatalog::default(),
            stdlib: StandardLibrary::new(),
            functions: IndexMap::new(),
            function_blocks: IndexMap::new(),
            classes: IndexMap::new(),
            interfaces: IndexMap::new(),
            edge_inputs: IndexMap::new(),
            programs: IndexMap::new(),
            globals: IndexMap::new(),
            constant_globals: indexmap::IndexSet::new(),
            program_types: IndexMap::new(),
            tasks: Vec::new(),
            storage: VariableStorage::default(),
            access: Vec::new(),
            io: crate::io::IoInterface::new(),
            statement_index: IndexMap::new(),
        };
        for fb in crate::stdlib::fbs::standard_function_blocks() {
            if builder.registry.lookup(fb.name.as_str()).is_none() {
                builder.registry.register(
                    fb.name.clone(),
                    Type::FunctionBlock {
                        name: fb.name.clone(),
                    },
                );
            }
            builder.register_function_block(fb);
        }
        builder
    }
}

impl RuntimeBuilder {
    /// Create an empty authoring context with the complete standard library.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the rules of the type conversion functions before installation, so that
    /// initializers and every scan use them (specification 07, vendor profiles).
    pub fn set_conversion_profile(
        &mut self,
        profile: crate::stdlib::conversions::ConversionProfile,
    ) {
        // The declarations' starting values follow the rules through the metadata's standard
        // library; the program image takes them from `native::build_with_conversions`
        // (specification 07 §2.10).
        self.stdlib = std::mem::take(&mut self.stdlib).with_conversion_profile(profile);
    }

    /// Register a global declared `VAR_GLOBAL CONSTANT`.
    pub(crate) fn register_constant_global(&mut self, name: SmolStr) {
        self.constant_globals.insert(name);
    }

    /// Record the program type of a program instance a configuration names.
    pub(crate) fn set_program_type(&mut self, instance: &str, program_type: SmolStr) {
        self.program_types
            .insert(SmolStr::new(instance.to_ascii_uppercase()), program_type);
    }

    /// Inspect the date/time interpretation used while lowering declarations.
    #[must_use]
    pub fn profile(&self) -> DateTimeProfile {
        self.profile
    }

    /// Inspect authoring types.
    #[must_use]
    pub fn registry(&self) -> &TypeRegistry {
        &self.registry
    }

    /// Define types before producing an executable artifact.
    pub fn registry_mut(&mut self) -> &mut TypeRegistry {
        &mut self.registry
    }

    /// Inspect construction-only storage used by the legacy producer.
    #[must_use]
    pub fn storage(&self) -> &VariableStorage {
        &self.storage
    }

    /// Edit construction-only values before producing and admitting an artifact.
    /// This borrow belongs to the non-executable builder and cannot address an
    /// installed controller. Installation must validate the declared layout and
    /// initial values; a raw edit is not an admission or execution capability.
    pub fn storage_mut(&mut self) -> &mut VariableStorage {
        &mut self.storage
    }

    /// The `VAR_ACCESS` declarations, by the names of the program's symbols.
    pub(crate) fn access_paths(&self) -> &[crate::harness::NamedAccess] {
        &self.access
    }

    /// Records the `VAR_ACCESS` declaration `access`.
    pub(crate) fn register_access(&mut self, access: crate::harness::NamedAccess) {
        self.access.push(access);
    }

    /// Inspect construction-time process-image bindings.
    #[must_use]
    pub fn io(&self) -> &crate::io::IoInterface {
        &self.io
    }

    /// Define process-image bindings before installation.
    pub fn io_mut(&mut self) -> &mut crate::io::IoInterface {
        &mut self.io
    }

    /// Inspect the authoring library used to resolve native calls.
    #[must_use]
    pub fn stdlib(&self) -> &StandardLibrary {
        &self.stdlib
    }

    /// Inspect registered program declarations.
    #[must_use]
    pub fn programs(&self) -> &IndexMap<SmolStr, ProgramDef> {
        &self.programs
    }

    /// Inspect registered function declarations.
    #[must_use]
    pub fn functions(&self) -> &IndexMap<SmolStr, FunctionDef> {
        &self.functions
    }

    /// Register a function declaration without executing its initializers.
    pub fn register_function(&mut self, declaration: FunctionDef) {
        self.functions
            .insert(declaration.name.to_ascii_uppercase().into(), declaration);
    }

    /// Register a function-block template without constructing an instance.
    pub fn register_function_block(&mut self, declaration: FunctionBlockDef) {
        self.function_blocks
            .insert(declaration.name.to_ascii_uppercase().into(), declaration);
    }

    /// Register a class template without constructing an instance.
    pub fn register_class(&mut self, declaration: ClassDef) {
        self.classes
            .insert(declaration.name.to_ascii_uppercase().into(), declaration);
    }

    /// Register an interface declaration.
    pub fn register_interface(&mut self, declaration: InterfaceDef) {
        self.interfaces
            .insert(declaration.name.to_ascii_uppercase().into(), declaration);
    }

    /// Register a program template without constructing its instance or running
    /// initializers. The producer/admission step checks its body and declarations.
    pub fn register_program(&mut self, declaration: ProgramDef) {
        self.programs.insert(declaration.name.clone(), declaration);
    }

    /// Declare a task before installation. Readiness state is owned by the
    /// installed shared engine, not by this authoring object.
    pub fn register_task(&mut self, task: TaskConfig) {
        self.tasks.push(task);
    }

    /// Inspect declared task configuration without mutable live readiness state.
    #[must_use]
    pub fn tasks(&self) -> &[TaskConfig] {
        &self.tasks
    }

    pub(crate) fn registry_and_initializer_catalog_mut(
        &mut self,
    ) -> (&mut TypeRegistry, &mut InitializerCatalog) {
        (&mut self.registry, &mut self.initializer_catalog)
    }

    /// The edge-qualified inputs of each program and function block, by its qualified name.
    pub(crate) fn edge_inputs(&self) -> &IndexMap<SmolStr, Vec<EdgeInput>> {
        &self.edge_inputs
    }

    pub(crate) fn register_edge_inputs(
        &mut self,
        owner: impl Into<SmolStr>,
        inputs: Vec<EdgeInput>,
    ) {
        self.edge_inputs.insert(owner.into(), inputs);
    }

    /// Attach statement locations to non-executable development declarations.
    pub fn register_statement_locations(&mut self, file_id: u32, locations: Vec<SourceLocation>) {
        self.statement_index.insert(file_id, locations);
    }

    /// Inspect function-block declarations before installation.
    #[must_use]
    pub fn function_blocks(&self) -> &IndexMap<SmolStr, FunctionBlockDef> {
        &self.function_blocks
    }

    /// Inspect class declarations before installation.
    #[must_use]
    pub fn classes(&self) -> &IndexMap<SmolStr, ClassDef> {
        &self.classes
    }

    /// Inspect interface declarations before installation.
    #[must_use]
    pub fn interfaces(&self) -> &IndexMap<SmolStr, InterfaceDef> {
        &self.interfaces
    }

    pub(crate) fn initializer_catalog(&self) -> &InitializerCatalog {
        &self.initializer_catalog
    }

    /// Select the resource identity before producing an artifact.
    pub fn set_resource_name(&mut self, name: impl Into<SmolStr>) {
        self.resource_name = name.into();
    }

    /// Inspect the declared resource identity.
    #[must_use]
    pub fn resource_name(&self) -> &SmolStr {
        &self.resource_name
    }

    /// The declared globals, each with its declared type, in declaration order.
    pub(crate) fn global_types(&self) -> IndexMap<SmolStr, trust_hir::TypeId> {
        self.globals.clone()
    }

    /// Records the declared type of the global `name`.
    pub(crate) fn register_global_type(&mut self, name: SmolStr, type_id: trust_hir::TypeId) {
        self.globals.insert(name, type_id);
    }

    /// Creates the starting state of the program instance `program` in the declarations' storage
    /// and records the instance.
    pub(crate) fn instantiate_program(
        &mut self,
        program: ProgramDef,
    ) -> Result<(), crate::error::RuntimeError> {
        let instance = crate::instance::create_program_instance(
            &mut self.storage,
            &self.registry,
            &self.profile,
            &self.classes,
            &self.function_blocks,
            &self.functions,
            &self.stdlib,
            &self.initializer_catalog,
            &program,
        )?;
        self.storage.set_global(
            program.name.clone(),
            crate::value::Value::Instance(instance),
        );
        self.programs.insert(program.name.clone(), program);
        Ok(())
    }

    pub(crate) fn metadata_for_configuration(
        &self,
        program_templates: IndexMap<SmolStr, ProgramDef>,
        programs: IndexMap<SmolStr, ProgramDef>,
        tasks: Vec<TaskConfig>,
    ) -> Result<super::RuntimeMetadata, crate::error::RuntimeError> {
        let mut next_thread = 1u32;
        let mut task_thread_ids = IndexMap::new();
        for task in &tasks {
            if !task_thread_ids.contains_key(&task.name) {
                task_thread_ids.insert(task.name.clone(), next_thread);
                next_thread = next_thread
                    .checked_add(1)
                    .ok_or(crate::error::RuntimeError::Overflow)?;
            }
        }
        let background = programs.keys().any(|name| {
            !tasks.iter().any(|task| {
                task.programs
                    .iter()
                    .any(|bound| bound.eq_ignore_ascii_case(name))
            })
        });
        Ok(super::RuntimeMetadata {
            profile: self.profile,
            registry: self.registry.clone(),
            stdlib: self.stdlib.clone(),
            functions: self.functions.clone(),
            function_blocks: self.function_blocks.clone(),
            classes: self.classes.clone(),
            interfaces: self.interfaces.clone(),
            programs,
            constant_globals: self.constant_globals.clone(),
            program_types: self.program_types.clone(),
            program_templates,
            tasks,
            task_thread_ids,
            background_thread_id: background.then_some(next_thread),
            statement_index: self.statement_index.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_and_task_registration_remain_non_executable_declarations() {
        let mut builder = RuntimeBuilder::new();
        builder.register_program(ProgramDef {
            name: "Main".into(),
            vars: vec![crate::program_model::VarDef {
                name: "value".into(),
                type_id: trust_hir::TypeId::INT,
                // Resolving this expression is an installation responsibility.
                // Merely registering the template cannot execute it.
                initializer: Some(crate::program_model::Expr::Name("missing".into())),
                retain: crate::RetainPolicy::Unspecified,
                static_storage: false,
                external: false,
                in_out: false,
                constant: false,
                address: None,
            }],
            temps: Vec::new(),
            using: Vec::new(),
            body: Vec::new(),
            methods: Vec::new(),
        });
        builder.register_task(TaskConfig {
            name: "Scan".into(),
            interval: crate::value::Duration::from_millis(25),
            single: None,
            priority: 0,
            programs: vec!["Main".into()],
            fb_instances: Vec::new(),
        });
        assert!(builder.storage().globals().is_empty());
        assert!(builder.storage().instances().is_empty());
        assert_eq!(builder.programs()["Main"].vars.len(), 1);
        assert_eq!(builder.tasks().len(), 1);
        assert_eq!(
            builder.tasks()[0].interval,
            crate::value::Duration::from_millis(25)
        );
        builder
            .storage_mut()
            .set_global("construction_seed", crate::value::Value::Int(7));
        assert_eq!(
            builder.storage().get_global("construction_seed"),
            Some(&crate::value::Value::Int(7))
        );
    }

    #[test]
    fn configured_template_using_does_not_create_a_background_program() {
        let mut builder = RuntimeBuilder::new();
        let template = ProgramDef {
            name: "Gpio".into(),
            vars: Vec::new(),
            temps: Vec::new(),
            using: vec!["PlantLibrary".into()],
            body: Vec::new(),
            methods: Vec::new(),
        };
        builder.register_program(template.clone());
        let mut instance = template.clone();
        instance.name = "Plant".into();
        let tasks = vec![TaskConfig {
            name: "Scan".into(),
            interval: crate::value::Duration::from_millis(10),
            priority: 0,
            single: None,
            programs: vec!["Plant".into()],
            fb_instances: Vec::new(),
        }];
        let metadata = builder
            .metadata_for_configuration(
                IndexMap::from([("GPIO".into(), template)]),
                IndexMap::from([("Plant".into(), instance)]),
                tasks,
            )
            .unwrap();
        assert_eq!(metadata.programs().len(), 1);
        assert!(metadata.programs().contains_key("Plant"));
        assert!(!metadata.has_background_programs());
        assert_eq!(metadata.background_thread_id(), None);
        assert_eq!(metadata.task_thread_id(&"Scan".into()), Some(1));
        let mut detached = VariableStorage::default();
        let frame = detached.push_frame("gPiO");
        assert_eq!(
            metadata.using_for_frame(&detached, frame),
            Some(vec!["PlantLibrary".into()])
        );
        detached.pop_frame();
        let frame = detached.push_frame("pLaNt");
        assert_eq!(
            metadata.using_for_frame(&detached, frame),
            Some(vec!["PlantLibrary".into()])
        );
    }

    #[test]
    fn builder_standard_declarations_preserve_legacy_type_identities_and_order() {
        let builder = RuntimeBuilder::new();
        let standard = crate::stdlib::fbs::standard_function_blocks();
        let expected_names = standard
            .iter()
            .map(|fb| fb.name.to_ascii_uppercase())
            .collect::<Vec<_>>();
        assert_eq!(
            builder
                .function_blocks
                .keys()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            expected_names
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        );
        for declaration in standard {
            assert!(builder.registry.lookup(&declaration.name).is_some());
            assert_eq!(
                builder.function_blocks[declaration.name.to_ascii_uppercase().as_str()].name,
                declaration.name
            );
        }
        assert!(builder.storage.globals().is_empty());
        assert!(builder.programs.is_empty());
        assert!(builder.tasks.is_empty());
        assert_eq!(builder.profile.epoch, DateTimeProfile::default().epoch);
        assert_eq!(
            builder.profile.resolution,
            DateTimeProfile::default().resolution
        );
    }
}
