type InstanceInitContext<'a> = (
    &'a mut VariableStorage,
    &'a TypeRegistry,
    &'a IndexMap<SmolStr, ClassDef>,
    &'a IndexMap<SmolStr, FunctionBlockDef>,
    &'a IndexMap<SmolStr, FunctionDef>,
    &'a StandardLibrary,
);

impl Runtime {
    /// Mutable access to variable storage (temporary API).
    pub fn storage_mut(&mut self) -> &mut VariableStorage {
        &mut self.storage
    }

    /// Access variable storage.
    #[must_use]
    pub fn storage(&self) -> &VariableStorage {
        &self.storage
    }

    #[must_use]
    /// Access the type registry.
    pub fn registry(&self) -> &TypeRegistry {
        &self.registry
    }

    /// Mutable access to the type registry.
    pub fn registry_mut(&mut self) -> &mut TypeRegistry {
        &mut self.registry
    }

    pub(crate) fn initializer_catalog(&self) -> &crate::program_model::InitializerCatalog {
        &self.initializer_catalog
    }

    pub(crate) fn registry_and_initializer_catalog_mut(
        &mut self,
    ) -> (&mut TypeRegistry, &mut crate::program_model::InitializerCatalog) {
        (&mut self.registry, &mut self.initializer_catalog)
    }

    /// Access the registered functions.
    #[must_use]
    pub fn functions(&self) -> &IndexMap<SmolStr, FunctionDef> {
        &self.functions
    }

    /// Access the registered function blocks.
    #[must_use]
    pub fn function_blocks(&self) -> &IndexMap<SmolStr, FunctionBlockDef> {
        &self.function_blocks
    }

    /// Access the registered classes.
    #[must_use]
    pub fn classes(&self) -> &IndexMap<SmolStr, ClassDef> {
        &self.classes
    }

    /// Access the registered interfaces.
    #[must_use]
    pub fn interfaces(&self) -> &IndexMap<SmolStr, InterfaceDef> {
        &self.interfaces
    }

    /// Access the registered programs.
    #[must_use]
    pub fn programs(&self) -> &IndexMap<SmolStr, ProgramDef> {
        &self.programs
    }

    pub(crate) fn globals(&self) -> &IndexMap<SmolStr, GlobalVarMeta> {
        &self.globals
    }

    /// Access the standard library.
    #[must_use]
    pub fn stdlib(&self) -> &StandardLibrary {
        &self.stdlib
    }

    pub(crate) fn instance_init_context(&mut self) -> InstanceInitContext<'_> {
        (
            &mut self.storage,
            &self.registry,
            &self.classes,
            &self.function_blocks,
            &self.functions,
            &self.stdlib,
        )
    }

    /// Register a function definition by name.
    pub fn register_function(&mut self, function: FunctionDef) {
        let key = function.name.to_ascii_uppercase();
        self.functions.insert(key.into(), function);
        self.vm_local_init_plan_cache.invalidate_all();
    }

    /// Register a function block definition by name.
    pub fn register_function_block(&mut self, function_block: FunctionBlockDef) {
        let key = function_block.name.to_ascii_uppercase();
        self.function_blocks.insert(key.into(), function_block);
        self.vm_local_init_plan_cache.invalidate_all();
    }

    /// Register a class definition by name.
    pub fn register_class(&mut self, class_def: ClassDef) {
        let key = class_def.name.to_ascii_uppercase();
        self.classes.insert(key.into(), class_def);
        self.vm_local_init_plan_cache.invalidate_all();
    }

    /// Register an interface definition by name.
    pub fn register_interface(&mut self, interface_def: InterfaceDef) {
        let key = interface_def.name.to_ascii_uppercase();
        self.interfaces.insert(key.into(), interface_def);
    }

    fn register_builtin_function_blocks(&mut self) {
        for fb in stdlib::fbs::standard_function_blocks() {
            if self.registry.lookup(fb.name.as_ref()).is_none() {
                let name = fb.name.clone();
                self.registry
                    .register(name.clone(), Type::FunctionBlock { name });
            }
            self.register_function_block(fb);
        }
    }

    /// Gets the current simulation time.
    #[must_use]
    pub fn current_time(&self) -> Duration {
        self.current_time
    }

    /// Returns the active execution backend mode.
    #[must_use]
    pub fn execution_backend(&self) -> crate::execution_backend::ExecutionBackend {
        self.execution_backend
    }

    /// Select execution backend mode.
    pub fn set_execution_backend(
        &mut self,
        backend: crate::execution_backend::ExecutionBackend,
    ) -> Result<(), error::RuntimeError> {
        self.execution_backend = backend;
        self.metrics.set_execution_backend(backend);
        Ok(())
    }

    /// Enable or disable register-VM profiling counters.
    pub fn set_vm_register_profile_enabled(&mut self, enabled: bool) {
        self.vm_register_profile.set_enabled(enabled);
    }

    /// Enable or disable register-IR lowering cache reuse.
    pub fn set_vm_register_lowering_cache_enabled(&mut self, enabled: bool) {
        self.vm_register_lowering_cache.set_enabled(enabled);
    }

    /// Clear register-IR lowering cache entries and counters.
    pub fn reset_vm_register_lowering_cache(&mut self) {
        self.vm_register_lowering_cache.reset();
    }

    /// Snapshot register-IR lowering cache counters.
    #[must_use]
    pub fn vm_register_lowering_cache_snapshot(
        &self,
    ) -> crate::execution_backend::VmRegisterLoweringCacheSnapshot {
        self.vm_register_lowering_cache.snapshot()
    }

    /// Clear register-VM profiling counters.
    pub fn reset_vm_register_profile(&mut self) {
        self.vm_register_profile.reset();
    }

    /// Snapshot register-VM profiling counters.
    #[must_use]
    pub fn vm_register_profile_snapshot(&self) -> crate::execution_backend::VmRegisterProfileSnapshot {
        self.vm_register_profile.snapshot()
    }

    /// Resolve a VM POU id to its decoded name when a bytecode module is loaded.
    #[must_use]
    pub fn vm_pou_name(&self, pou_id: u32) -> Option<String> {
        self.vm_module
            .as_ref()
            .and_then(|module| module.pou_name(pou_id))
            .map(ToOwned::to_owned)
    }

    /// Enable or disable the experimental tier-1 specialized register-executor path.
    pub fn set_vm_tier1_specialized_executor_enabled(&mut self, enabled: bool) {
        self.vm_tier1_specialized_executor.set_enabled(enabled);
    }

    /// Clear tier-1 specialized register-executor cache and counters.
    pub fn reset_vm_tier1_specialized_executor(&mut self) {
        self.vm_tier1_specialized_executor.reset();
    }

    /// Snapshot tier-1 specialized register-executor cache/counter state.
    #[must_use]
    pub fn vm_tier1_specialized_executor_snapshot(
        &self,
    ) -> crate::execution_backend::VmTier1SpecializedExecutorSnapshot {
        self.vm_tier1_specialized_executor.snapshot()
    }

    /// Access the I/O interface.
    pub fn io(&self) -> &IoInterface {
        self.io.interface()
    }

    /// Mutable access to the I/O interface.
    pub fn io_mut(&mut self) -> &mut IoInterface {
        self.io.interface_mut()
    }

    /// Register an I/O driver invoked at cycle boundaries.
    pub fn add_io_driver(&mut self, name: impl Into<SmolStr>, driver: Box<dyn IoDriver>) {
        self.io.add_driver(name, driver);
    }

    /// Clear all registered I/O drivers.
    pub fn clear_io_drivers(&mut self) {
        self.io.clear_drivers();
    }

    /// Set the sink for I/O driver health snapshots.
    pub fn set_io_health_sink(
        &mut self,
        sink: Option<std::sync::Arc<std::sync::Mutex<Vec<IoDriverStatus>>>>,
    ) {
        self.io.set_health_sink(sink);
    }

    pub(super) fn update_io_health(&self) {
        self.io.update_health();
    }

    /// Access the current cycle counter.
    #[must_use]
    pub fn cycle_counter(&self) -> u64 {
        self.cycle_counter
    }

    /// Returns the VAR_ACCESS binding map.
    #[must_use]
    pub fn access_map(&self) -> &AccessMap {
        &self.access
    }

    /// Returns a mutable VAR_ACCESS binding map.
    pub fn access_map_mut(&mut self) -> &mut AccessMap {
        &mut self.access
    }

    /// Resolve USING directives for the given frame id.
    #[must_use]
    pub fn using_for_frame(&self, frame_id: FrameId) -> Option<Vec<SmolStr>> {
        let frame = self
            .storage
            .frames()
            .iter()
            .find(|frame| frame.id == frame_id)?;
        resolve_using_for_frame(
            frame,
            &self.storage,
            &self.functions,
            &self.function_blocks,
            &self.classes,
            &self.programs,
        )
        .map(|using| using.to_vec())
    }

    /// Reads a VAR_ACCESS binding by name.
    #[must_use]
    pub fn read_access(&self, name: &str) -> Option<Value> {
        let binding = self.access.get(name)?;
        let value = self.storage.read_by_ref(binding.reference.clone())?.clone();
        if let Some(partial) = binding.partial {
            crate::value::read_partial_access(&value, partial).ok()
        } else {
            Some(value)
        }
    }

    /// Writes a VAR_ACCESS binding by name.
    pub fn write_access(&mut self, name: &str, value: Value) -> Result<(), error::RuntimeError> {
        let Some(binding) = self.access.get(name) else {
            return Err(error::RuntimeError::UndefinedVariable(name.into()));
        };
        if let Some(partial) = binding.partial {
            let current = self
                .storage
                .read_by_ref(binding.reference.clone())
                .cloned()
                .ok_or(error::RuntimeError::NullReference)?;
            let updated = crate::value::write_partial_access(current, partial, value).map_err(
                |err| match err {
                    crate::value::PartialAccessError::IndexOutOfBounds {
                        index,
                        lower,
                        upper,
                    } => error::RuntimeError::IndexOutOfBounds {
                        index,
                        lower,
                        upper,
                    },
                    crate::value::PartialAccessError::TypeMismatch => {
                        error::RuntimeError::TypeMismatch
                    }
                },
            )?;
            if self
                .storage
                .write_by_ref(binding.reference.clone(), updated)
            {
                Ok(())
            } else {
                Err(error::RuntimeError::NullReference)
            }
        } else if self.storage.write_by_ref(binding.reference.clone(), value) {
            Ok(())
        } else {
            Err(error::RuntimeError::NullReference)
        }
    }

}
