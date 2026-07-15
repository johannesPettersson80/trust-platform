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
    ) -> (
        &mut TypeRegistry,
        &mut crate::program_model::InitializerCatalog,
    ) {
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
    pub fn vm_register_profile_snapshot(
        &self,
    ) -> crate::execution_backend::VmRegisterProfileSnapshot {
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

    /// Register and start one ADS client connection.
    pub fn start_ads_connection<T>(
        &mut self,
        connection: &crate::ads::AdsConnectionConfig,
        transport: T,
        worker_tick_interval: std::time::Duration,
    ) -> Result<(), error::RuntimeError>
    where
        T: crate::ads::AdsTransport + Send + 'static,
    {
        let bindings = crate::ads::resolve_declared_bindings(self, connection).map_err(|err| {
            error::RuntimeError::InvalidConfig(
                format!("ADS connection '{}': {err}", connection.route.name).into(),
            )
        })?;
        let (bridge, worker) = crate::ads::AdsConnectionBridge::with_transport(transport, bindings)
            .map_err(|err| {
                error::RuntimeError::IoTransport(
                    format!("ADS connection '{}': {err}", connection.route.name).into(),
                )
            })?;
        let worker = worker.spawn(worker_tick_interval).map_err(|err| {
            error::RuntimeError::IoTransport(
                format!("ADS connection '{}': {err}", connection.route.name).into(),
            )
        })?;
        self.ads
            .add_connection(connection.route.clone(), bridge, worker);
        Ok(())
    }

    /// Stop all active ADS client workers.
    pub fn shutdown_ads(&mut self) -> Result<(), error::RuntimeError> {
        self.ads.shutdown()
    }

    /// Number of configured ADS client connections.
    #[must_use]
    pub fn ads_connection_count(&self) -> usize {
        self.ads.connection_count()
    }

    /// Record the ADS config hash loaded by the runtime bundle.
    pub fn set_ads_deployed_config_hash(&mut self, hash: Option<String>) {
        self.ads.set_deployed_ads_config_hash(hash);
    }

    /// Current ADS client status projection.
    #[must_use]
    pub fn ads_status_report(&self) -> crate::ads::diagnostics::AdsStatusReport {
        self.ads.status_report()
    }

    /// Imported ADS globals projected into the existing Live Values surface.
    #[must_use]
    pub fn ads_live_values(&self) -> Vec<crate::ads::AdsLiveValue> {
        let mut values = self.ads.live_values(&self.storage);
        let forced_globals = self
            .debug
            .as_ref()
            .map(|debug| {
                debug
                    .forced_snapshot()
                    .vars
                    .into_iter()
                    .filter_map(|forced| match forced.target {
                        crate::debug::ForcedVarTarget::Global(name) => Some(name),
                        _ => None,
                    })
                    .collect::<std::collections::BTreeSet<_>>()
            })
            .unwrap_or_default();
        for value in &mut values {
            value.forced = forced_globals.contains(value.point_name.as_str());
        }
        values
    }

    /// Queue a write for a writable imported ADS global, if one owns this name.
    pub fn queue_ads_live_write(&mut self, point_name: &str, value: Value) -> bool {
        self.ads.queue_live_write(point_name, value)
    }

    /// Whether an imported ADS global exists and permits writes to the PLC.
    #[must_use]
    pub fn ads_live_value_writable(&self, point_name: &str) -> Option<bool> {
        self.ads.live_value_writable(point_name)
    }

    /// Configure OPC UA client connections for this runtime.
    pub fn configure_opcua_client(
        &mut self,
        config: &crate::opcua::OpcUaClientConfig,
    ) -> Result<(), error::RuntimeError> {
        let mut subsystem = super::opcua_client_subsystem::OpcUaClientSubsystem::new();
        subsystem.configure(self, config)?;
        self.opcua_client = subsystem;
        Ok(())
    }

    /// Register and start one OPC UA client connection.
    pub fn start_opcua_client_connection<T>(
        &mut self,
        connection: &crate::opcua::OpcUaClientConnectionConfig,
        transport: T,
        worker_tick_interval: std::time::Duration,
    ) -> Result<(), error::RuntimeError>
    where
        T: crate::opcua::OpcUaClientTransport + Send + 'static,
    {
        let bindings = crate::opcua::resolve_opcua_client_bindings(self, connection)?;
        let (bridge, worker) = crate::opcua::OpcUaClientBridge::with_transport(
            connection.clone(),
            transport,
            bindings,
        )
        .map_err(|err| {
            error::RuntimeError::IoTransport(
                format!("OPC UA client connection '{}': {err}", connection.name).into(),
            )
        })?;
        let worker = worker.spawn(worker_tick_interval).map_err(|err| {
            error::RuntimeError::IoTransport(
                format!("OPC UA client connection '{}': {err}", connection.name).into(),
            )
        })?;
        self.opcua_client
            .add_connection(connection.clone(), bridge, Some(worker));
        Ok(())
    }

    /// Clear and stop active OPC UA client workers.
    pub fn reset_opcua_client_connections(&mut self) -> Result<(), error::RuntimeError> {
        self.opcua_client.shutdown()?;
        self.opcua_client = super::opcua_client_subsystem::OpcUaClientSubsystem::new();
        Ok(())
    }

    /// Record the OPC UA client config hash loaded by the runtime bundle.
    pub fn set_opcua_client_deployed_config_hash(&mut self, hash: Option<String>) {
        self.opcua_client.set_deployed_config_hash(hash);
    }

    /// Number of configured OPC UA client connections.
    #[must_use]
    pub fn opcua_client_connection_count(&self) -> usize {
        self.opcua_client.connection_count()
    }

    /// Current OPC UA client status projection.
    #[must_use]
    pub fn opcua_client_status_report(&self) -> crate::opcua::OpcUaClientStatusReport {
        self.opcua_client.status_report()
    }

    /// Snapshot for a live ADS device that overlaps a doctor target.
    #[must_use]
    pub fn active_ads_device_snapshot(
        &self,
        target: &crate::ads::diagnostics::TargetIdentity,
        local: Option<&crate::ads::diagnostics::LocalIdentity>,
    ) -> Option<crate::ads::onboarding::ActiveAdsDeviceSnapshot> {
        self.ads.active_device_snapshot(target, local)
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
