//! Installed hosted controller: one shared execution owner plus host adapters.

use super::{
    ads_subsystem::AdsSubsystem, hosted_services::HostedExecutionServices,
    io_subsystem::IoSubsystem, metrics_subsystem::MetricsSubsystem,
    opcua_client_subsystem::OpcUaClientSubsystem, openot_telemetry::OpenOtTelemetrySubsystem,
    watchdog_subsystem::WatchdogSubsystem, RuntimeMetadata,
};
use crate::{
    debug::DebugControl,
    error::RuntimeError,
    execution_backend::ExecutionBackend,
    io::{InstalledIoView, IoAddress, IoDriver, IoDriverStatus, IoSafeState},
    memory::{AccessMap, VariableStorage},
    retain::RetainManager,
    task::TaskConfig,
    value::{DateTimeProfile, Duration, PartialAccess, RefSegment, Value, ValueRef},
    watchdog::{FaultPolicy, WatchdogPolicy},
};
use smol_str::SmolStr;
use std::{
    sync::Arc,
    time::{Duration as PhysicalDuration, Instant},
};
use trust_runtime_core::vm::{OwnedRuntimeState, PreparationLimits, PreparedModule};

/// An admitted application and its hosted service composition.
///
/// Construction belongs to [`super::RuntimeBuilder`]. This type has neither a
/// mutable PLC storage/type-registry getter nor an authoring execution context.
/// Installed values change only through the shared engine's checked operations.
///
/// ```compile_fail,E0599
/// fn bypass(runtime: &mut trust_runtime::Runtime) {
///     let _ = runtime.storage_mut();
/// }
/// ```
pub struct Runtime {
    pub(super) state: OwnedRuntimeState<HostedExecutionServices>,
    pub(super) metadata: Arc<RuntimeMetadata>,
    pub(super) artifact: Option<Arc<crate::bytecode::BytecodeModule>>,
    // Original legacy transport is exportable only until a committed configuration change.
    pub(super) original_artifact: Option<Arc<crate::bytecode::BytecodeModule>>,
    pub(super) resource_name: SmolStr,
    pub(super) tasks: Vec<TaskConfig>,
    pub(super) io: IoSubsystem,
    pub(super) ads: AdsSubsystem,
    pub(super) opcua_client: OpcUaClientSubsystem,
    pub(super) retain: RetainManager,
    pub(super) metrics: MetricsSubsystem,
    pub(super) watchdog: WatchdogSubsystem,
    pub(super) fault_policy: FaultPolicy,
    pub(super) openot_telemetry: OpenOtTelemetrySubsystem,
    pub(super) cycle_counter: u64,
    pub(super) output_commit_deadline: Option<Instant>,
    pub(super) output_commit_pause_baseline: PhysicalDuration,
}

impl std::fmt::Debug for Runtime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Runtime")
            .field("resource_name", &self.resource_name)
            .field("profile", self.state.profile())
            .field("storage", self.state.storage())
            .field("cycle_counter", &self.cycle_counter)
            .field("fault", &self.state.fault())
            .finish_non_exhaustive()
    }
}

impl Runtime {
    /// Decode and admit a supported source-free STBC application before constructing
    /// any executable state. Legacy 1.1 input requires its authoring context.
    pub fn from_bytes(bytes: &[u8], limits: PreparationLimits) -> Result<Self, RuntimeError> {
        let (prepared, raw) = PreparedModule::from_bytes_with_artifact(bytes, limits)?;
        let mut runtime = Self::from_prepared_with_metadata(Arc::new(prepared), None, 0)?;
        runtime.artifact = Some(Arc::new(raw.into()));
        Ok(runtime)
    }

    /// Keep the validated original contract for export after contextual legacy admission.
    /// The installed module retains the explicit legacy-ABI flag and shared execution plans.
    pub(crate) fn retain_legacy_import_artifact(
        &mut self,
        source: &crate::bytecode::BytecodeModule,
    ) -> Result<(), RuntimeError> {
        if source.version != crate::bytecode::BytecodeVersion::LEGACY
            || self.prepared().storage_abi() != crate::value::StorageAbi::LegacyTemporal32
        {
            return Err(RuntimeError::InvalidConfig(
                "legacy artifact identity does not match installation".into(),
            ));
        }
        self.original_artifact = Some(Arc::new(source.clone()));
        Ok(())
    }

    pub(crate) fn from_module_with_metadata(
        module: crate::bytecode::BytecodeModule,
        metadata: Option<Arc<RuntimeMetadata>>,
        resource_index: usize,
    ) -> Result<Self, RuntimeError> {
        let bytes = module.encode().map_err(RuntimeError::from)?;
        let (prepared, raw) =
            PreparedModule::from_bytes_with_artifact(&bytes, PreparationLimits::default())?;
        let mut runtime =
            Self::from_prepared_with_metadata(Arc::new(prepared), metadata, resource_index)?;
        runtime.artifact = Some(Arc::new(raw.into()));
        Ok(runtime)
    }

    pub(crate) fn from_prepared_with_metadata(
        prepared: Arc<PreparedModule>,
        metadata: Option<Arc<RuntimeMetadata>>,
        resource_index: usize,
    ) -> Result<Self, RuntimeError> {
        let services = HostedExecutionServices::new(Arc::clone(&prepared), None, None);
        let state = prepared.instantiate_owned_with_services(resource_index, services)?;
        Self::from_owned_with_metadata(state, metadata, None, resource_index)
    }

    /// Assemble host adapters around an already constructed candidate. It must
    /// not execute initializers a second time merely to bind host metadata.
    pub(super) fn from_owned_with_metadata(
        mut state: OwnedRuntimeState<HostedExecutionServices>,
        metadata: Option<Arc<RuntimeMetadata>>,
        artifact: Option<Arc<crate::bytecode::BytecodeModule>>,
        resource_index: usize,
    ) -> Result<Self, RuntimeError> {
        let prepared = state.shared_prepared();
        let (name, tasks, metadata, io) =
            Self::prepare_host_schema(&prepared, &mut state, metadata, resource_index)?;
        state.services_mut().set_metadata(Arc::clone(&metadata));
        Ok(Self {
            state,
            metadata,
            artifact,
            original_artifact: None,
            resource_name: name,
            tasks,
            io,
            ads: AdsSubsystem::new(),
            opcua_client: OpcUaClientSubsystem::new(),
            retain: RetainManager::default(),
            metrics: MetricsSubsystem::new(),
            watchdog: WatchdogSubsystem::new(),
            fault_policy: FaultPolicy::Halt,
            openot_telemetry: OpenOtTelemetrySubsystem::default(),
            cycle_counter: 0,
            output_commit_deadline: None,
            output_commit_pause_baseline: PhysicalDuration::ZERO,
        })
    }

    /// Bind immutable host presentation against an existing owner without
    /// constructing or copying PLC values. Resource-only updates use this seam.
    pub(super) fn prepare_host_schema(
        prepared: &PreparedModule,
        state: &mut OwnedRuntimeState<HostedExecutionServices>,
        metadata: Option<Arc<RuntimeMetadata>>,
        resource_index: usize,
    ) -> Result<(SmolStr, Vec<TaskConfig>, Arc<RuntimeMetadata>, IoSubsystem), RuntimeError> {
        let resource = prepared
            .resources()
            .get(resource_index)
            .ok_or_else(|| RuntimeError::InvalidConfig("resource index out of bounds".into()))?;
        let name = prepared
            .strings()
            .get(resource.name_idx as usize)
            .cloned()
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let mut metadata = match metadata {
            Some(metadata) => (*metadata).clone(),
            None => RuntimeMetadata::from_prepared(prepared, resource_index)?,
        };
        metadata.bind_known_artifact_types(prepared)?;
        metadata.bind_retained_declarations(prepared)?;
        metadata.bind_global_declarations(prepared)?;
        // All external aliases and task FB destinations bind to live root ids,
        // never to a guessed artifact instance sequence.
        let mut access = AccessMap::default();
        for binding in prepared.access_bindings() {
            let name = prepared
                .strings()
                .get(binding.name_idx as usize)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            let partial = PartialAccess::from_wire(binding.partial_kind, binding.partial_index)
                .map_err(RuntimeError::from)?;
            let reference = state.resolve_access_reference(name)?;
            access.bind_with_mode(name.clone(), reference, partial, binding.is_writable());
        }
        metadata.access = access;
        let mut tasks = Vec::with_capacity(resource.tasks.len());
        for task in &resource.tasks {
            let string = |id: u32| {
                prepared
                    .strings()
                    .get(id as usize)
                    .cloned()
                    .ok_or(RuntimeError::InvalidExecutionState)
            };
            let mut fb_instances = Vec::with_capacity(task.fb_ref_idx.len());
            for &reference in &task.fb_ref_idx {
                fb_instances.push(state.resolve_persistent_reference(reference)?);
            }
            tasks.push(TaskConfig {
                name: string(task.name_idx)?,
                interval: Duration::from_nanos(task.interval_nanos),
                single: task.single_name_idx.map(string).transpose()?,
                priority: task.priority,
                programs: task
                    .program_name_idx
                    .iter()
                    .map(|id| string(*id))
                    .collect::<Result<_, _>>()?,
                fb_instances,
            });
        }
        metadata.storage_abi = prepared.storage_abi();
        metadata.tasks = tasks.clone();
        metadata.task_thread_ids.clear();
        let mut next_thread = 1u32;
        for task in &tasks {
            if !metadata.task_thread_ids.contains_key(&task.name) {
                metadata
                    .task_thread_ids
                    .insert(task.name.clone(), next_thread);
                next_thread = next_thread.checked_add(1).ok_or(RuntimeError::Overflow)?;
            }
        }
        metadata.background_thread_id = metadata.has_background_programs().then_some(next_thread);
        let metadata = Arc::new(metadata);
        let mut io = IoSubsystem::new();
        for (index, binding) in prepared.io_bindings().iter().enumerate() {
            let reference = state.resolve_persistent_reference(binding.ref_idx)?;
            let display_name = match metadata.io_display_names.get(index).cloned().flatten() {
                Some(name) => Some(name),
                None => Some(prepared.persistent_reference_name(binding.ref_idx)?),
            };
            let value_type = binding.type_id.and_then(|id| metadata.artifact_type(id));
            io.interface_mut().bind_prepared_ref(
                reference,
                binding.address.clone(),
                binding.codec.clone(),
                value_type,
                display_name,
            );
        }
        io.freeze_bindings();
        Ok((name, tasks, metadata, io))
    }

    /// Inspect the admitted bytecode/type/resource contract.
    pub fn prepared(&self) -> &PreparedModule {
        self.state.prepared()
    }
    pub(crate) fn deployment_artifact(
        &self,
    ) -> Result<&crate::bytecode::BytecodeModule, crate::bytecode::BytecodeError> {
        self.original_artifact
            .as_deref()
            .or(self.artifact.as_deref())
            .ok_or_else(|| {
                crate::bytecode::BytecodeError::InvalidSection(
                    "installation has no retained deployment artifact".into(),
                )
            })
    }
    /// Return the admitted construction archive used to reconfigure this
    /// installation. A preserved legacy export archive is never an admission
    /// source after contextual import.
    pub(crate) fn construction_artifact(
        &self,
    ) -> Result<&crate::bytecode::BytecodeModule, crate::bytecode::BytecodeError> {
        self.artifact.as_deref().ok_or_else(|| {
            crate::bytecode::BytecodeError::InvalidSection(
                "installation has no admitted construction artifact".into(),
            )
        })
    }
    /// Inspect authoritative PLC values without an untyped mutation escape.
    pub fn storage(&self) -> &VariableStorage {
        self.state.storage()
    }
    /// Inspect a detached immutable tooling metadata snapshot.
    pub fn metadata_snapshot(&self) -> RuntimeMetadata {
        (*self.metadata).clone()
    }
    pub(crate) fn tooling_metadata(&self) -> &RuntimeMetadata {
        &self.metadata
    }
    /// Inspect the tooling type registry; RUN uses the admitted TYPE_TABLE.
    pub fn registry(&self) -> &trust_hir::types::TypeRegistry {
        self.metadata.registry()
    }
    /// Inspect the complete hosted standard-library presentation.
    pub fn stdlib(&self) -> &crate::stdlib::StandardLibrary {
        self.metadata.stdlib()
    }
    /// Read the current date/time interpretation.
    pub fn profile(&self) -> DateTimeProfile {
        *self.state.profile()
    }
    /// Read logical time independently of physical deadlines.
    pub fn current_time(&self) -> Duration {
        self.state.current_time()
    }
    /// Set simulator logical time without resetting nominal task deadlines.
    pub fn set_current_time(&mut self, now: Duration) {
        self.state.set_current_time(now);
    }
    /// Advance the simulator clock with the existing saturating arithmetic.
    pub fn advance_time(&mut self, delta: Duration) {
        self.set_current_time(Duration::from_nanos(
            self.current_time()
                .as_nanos()
                .saturating_add(delta.as_nanos()),
        ));
    }
    pub(crate) fn reset_task_timing(&mut self, now: Duration) {
        self.state.reset_task_timing(now);
    }
    #[cfg(test)]
    pub(crate) fn resource_name(&self) -> &SmolStr {
        &self.resource_name
    }
    /// Inspect immutable configured tasks; readiness belongs to the shared owner.
    pub fn tasks(&self) -> &[TaskConfig] {
        &self.tasks
    }
    /// Read dropped activation accounting by task name.
    pub fn task_overrun_count(&self, name: &str) -> Option<u64> {
        self.tasks
            .iter()
            .position(|task| task.name.eq_ignore_ascii_case(name))
            .and_then(|index| self.state.task_states().get(index))
            .map(|state| state.overrun_count)
    }
    /// Inspect background-program configuration without altering the scheduler.
    pub fn has_background_programs(&self) -> bool {
        self.metadata.has_background_programs()
    }
    /// Inspect configured program source/tooling definitions.
    pub fn programs(&self) -> &indexmap::IndexMap<SmolStr, crate::task::ProgramDef> {
        self.metadata.programs()
    }
    /// Inspect immutable function source/tooling definitions.
    pub fn functions(&self) -> &indexmap::IndexMap<SmolStr, crate::program_model::FunctionDef> {
        self.metadata.functions()
    }
    /// Inspect immutable block source/tooling definitions.
    pub fn function_blocks(
        &self,
    ) -> &indexmap::IndexMap<SmolStr, crate::program_model::FunctionBlockDef> {
        self.metadata.function_blocks()
    }
    /// Inspect immutable class source/tooling definitions.
    pub fn classes(&self) -> &indexmap::IndexMap<SmolStr, crate::program_model::ClassDef> {
        self.metadata.classes()
    }
    /// Inspect immutable interface source/tooling definitions.
    pub fn interfaces(&self) -> &indexmap::IndexMap<SmolStr, crate::program_model::InterfaceDef> {
        self.metadata.interfaces()
    }
    /// Inspect current checked engineering aliases.
    pub fn access_map(&self) -> &AccessMap {
        self.metadata.access_map()
    }
    pub(crate) fn globals(
        &self,
    ) -> &indexmap::IndexMap<SmolStr, super::metadata::GlobalDeclarationInfo> {
        &self.metadata.global_declarations
    }
    /// Commit a checked global value update between owner entries.
    pub fn write_global(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        self.state.write_global(name, value)
    }
    /// Check a prospective shared-resource write without changing live storage.
    pub(crate) fn validate_global_write(
        &mut self,
        name: &str,
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.state.validate_global_write(name, value)
    }
    /// Commit a checked program/global aggregate path update.
    pub fn write_named_path(
        &mut self,
        name: &str,
        path: &[RefSegment],
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.state.write_named_path(name, path, value)
    }
    /// Apply a current typed destination with lifetime checks.
    pub fn write_reference(
        &mut self,
        reference: &ValueRef,
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.state.write_reference(reference.as_view(), value)
    }
    /// Resolve an existing instance member and commit through the common checked
    /// reference policy. Missing/retired instances cannot add undeclared fields.
    pub fn write_instance(
        &mut self,
        instance: crate::memory::InstanceId,
        name: &str,
        value: Value,
    ) -> Result<(), RuntimeError> {
        let reference = self
            .storage()
            .ref_for_instance_recursive(instance, name)
            .ok_or(RuntimeError::ReferenceLifetime)?;
        self.write_reference(&reference, value)
    }
    /// Inspect a checked bound value through common reference policy.
    pub fn read_reference(&mut self, reference: &ValueRef) -> Result<Value, RuntimeError> {
        self.state.read_reference(reference)
    }
    /// Read an admitted alias, preserving the hosted optional lookup surface.
    pub fn read_access(&self, name: &str) -> Option<Value> {
        let binding = self.metadata.access_map().get(name)?;
        let value = self.storage().materialize_by_ref_ref(&binding.reference)?;
        match binding.partial {
            Some(partial) => crate::value::read_partial_access(&value, partial).ok(),
            None => Some(value),
        }
    }
    /// Write an admitted alias through the shared mutability/type/lifetime checks.
    pub fn write_access(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        match self.state.write_access(name, value) {
            Err(RuntimeError::InvalidAlias)
                if self
                    .metadata
                    .access_map()
                    .get(name)
                    .is_some_and(|binding| !binding.writable) =>
            {
                Err(RuntimeError::ReadOnlyAlias(name.into()))
            }
            result => result,
        }
    }
    /// Borrow images owned by the shared engine, with immutable binding metadata.
    pub fn io(&self) -> InstalledIoView<'_> {
        self.io.installed_view(&self.state)
    }
    /// Label existing admitted bindings for display. This operation changes
    /// provenance text only, not their destinations, codecs or live images.
    pub fn annotate_io_binding_sources(&mut self, drivers: &[crate::config::IoDriverConfig]) {
        crate::io::annotate_io_binding_sources(self.io.interface_mut(), drivers);
    }
    /// Commit a checked image-window write without changing bindings or capacity.
    pub fn write_io(&mut self, address: &IoAddress, value: Value) -> Result<(), RuntimeError> {
        self.state.write_image(address, value)
    }
    /// Sample physical drivers directly into the authoritative input image.
    pub fn sample_inputs(&mut self) -> Result<(), RuntimeError> {
        self.io.sample_inputs(&mut self.state)
    }
    /// Register a host driver without modifying admitted process-image bindings.
    pub fn add_io_driver(&mut self, name: impl Into<SmolStr>, driver: Box<dyn IoDriver>) {
        self.io.add_driver(name, driver);
    }
    /// Stop using currently registered host drivers.
    pub fn clear_io_drivers(&mut self) {
        self.io.clear_drivers();
    }
    /// Select the existing driver health reporting sink.
    pub fn set_io_health_sink(&mut self, sink: Option<Arc<std::sync::Mutex<Vec<IoDriverStatus>>>>) {
        self.io.set_health_sink(sink);
    }
    /// Replace host safe-output policy; application bindings remain immutable.
    pub fn set_io_safe_state(&mut self, policy: IoSafeState) {
        self.io.set_safe_state(policy);
    }
    /// Connect the existing debug control to detached observations and queued writes.
    pub fn set_debug_control(&mut self, control: DebugControl) {
        self.state.services_mut().set_debug_control(Some(control));
    }
    /// Enable debugging in the existing hosted composition.
    pub fn enable_debug(&mut self) -> DebugControl {
        let control = DebugControl::new();
        self.set_debug_control(control.clone());
        control
    }
    /// Get the existing control handle without acquiring execution storage.
    pub fn debug_control(&self) -> Option<DebugControl> {
        self.state.services().debug_control().cloned()
    }
    /// Arm a physical execution deadline, including the existing pause accounting.
    pub fn set_execution_deadline(&mut self, deadline: Option<Instant>) {
        self.state.services().set_execution_deadline(deadline);
    }
    /// Read the armed physical execution deadline.
    pub fn execution_deadline(&self) -> Option<Instant> {
        self.state.services().execution_deadline()
    }
    pub(crate) fn effective_execution_deadline(&self) -> Option<Instant> {
        self.state.services().effective_execution_deadline()
    }
    pub(crate) fn debug_watchdog_pause_elapsed(&self) -> PhysicalDuration {
        self.debug_control()
            .as_ref()
            .map(DebugControl::watchdog_pause_elapsed)
            .unwrap_or_default()
    }
    /// Select host supervision policy without changing PLC semantics.
    pub fn set_watchdog_policy(&mut self, policy: WatchdogPolicy) {
        self.watchdog.set_policy(policy);
    }
    /// Inspect the selected supervision policy.
    pub fn watchdog_policy(&self) -> WatchdogPolicy {
        self.watchdog.policy()
    }
    /// Select host handling of a fault latched by the shared engine.
    pub fn set_fault_policy(&mut self, policy: FaultPolicy) {
        self.fault_policy = policy;
    }
    /// Inspect host fault handling policy.
    pub fn fault_policy(&self) -> FaultPolicy {
        self.fault_policy
    }
    /// Inspect the single shared engine fault latch.
    pub fn faulted(&self) -> bool {
        self.state.fault().is_some()
    }
    /// Inspect the owning execution fault, independent of safe-output delivery errors.
    pub fn last_fault(&self) -> Option<&RuntimeError> {
        self.state.fault()
    }
    /// The canonical bytecode backend remains the only execution backend.
    pub fn execution_backend(&self) -> ExecutionBackend {
        ExecutionBackend::BytecodeVm
    }
    /// Preserve backend-selection protocol without introducing another dispatcher.
    pub fn set_execution_backend(&mut self, backend: ExecutionBackend) -> Result<(), RuntimeError> {
        self.metrics.set_execution_backend(backend);
        Ok(())
    }
}
