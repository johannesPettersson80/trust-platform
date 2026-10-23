//! Source-free std composition: one portable owner and explicit host services.
use std::{
    cell::Cell,
    sync::Arc,
    time::{Instant, SystemTime},
};
use trust_runtime_core::{
    error::RuntimeError,
    io_address::IoAddress,
    memory::VariableStorage,
    retain::{RestartMode, RetainSnapshot},
    task::TaskState,
    value::{DateTimeProfile, DateTimeValue, Duration, RefSegment, Value, ValueRef},
    vm::{ExecutionServices, OwnedRuntimeState, PreparationLimits, PreparedModule},
};

struct HostServices {
    deadline: Cell<Option<Instant>>,
    #[cfg(feature = "register-ir")]
    register_configuration: trust_runtime_core::vm::register::RegisterExecutionConfiguration,
}
impl Default for HostServices {
    fn default() -> Self {
        Self {
            deadline: Cell::new(None),
            #[cfg(feature = "register-ir")]
            register_configuration: crate::register_environment::capture(),
        }
    }
}
impl ExecutionServices for HostServices {
    #[cfg(feature = "register-ir")]
    fn register_configuration(
        &self,
    ) -> trust_runtime_core::vm::register::RegisterExecutionConfiguration {
        self.register_configuration
    }
    fn deadline_exceeded(&self) -> bool {
        self.deadline
            .get()
            .is_some_and(|deadline| Instant::now() >= deadline)
    }
    fn has_wall_clock(&self) -> bool {
        true
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        crate::host_clock::current_dt(SystemTime::now())
    }
}

/// Compiler-free hosted controller over the canonical admitted execution owner.
///
/// Select `--no-default-features --features runtime-only`. Optional `register-ir`
/// selects prepared optimizations; it never removes an IEC function. Full
/// controller/development services and construction APIs use the `full` feature.
///
/// A saved application runs without a compiler session. This example loads the
/// portability GPIO fixture, writes a global through the checked runtime-only API,
/// runs one cycle, and reads the program result:
///
/// ```
/// use trust_runtime::{
///     runtime_core::vm::PreparationLimits,
///     value::{Duration, Value},
///     Runtime,
/// };
///
/// const GPIO: &[u8] = include_bytes!("../../tests/fixtures/portability/f401/gpio.stbc");
/// let mut runtime = Runtime::from_bytes(GPIO, PreparationLimits::default())?;
/// runtime.write_global("probe_depth", Value::Int(3))?;
/// runtime.advance_time(Duration::from_millis(10));
/// runtime.execute_cycle()?;
/// assert_eq!(runtime.storage().get_global("probe_depth"), Some(&Value::Int(3)));
/// let Some(Value::Instance(plant)) = runtime.storage().get_global("Plant") else {
///     panic!("admitted program root")
/// };
/// assert_eq!(
///     runtime.storage().get_instance_var(*plant, "depth_result"),
///     Some(&Value::Int(3)),
/// );
/// assert!(!runtime.faulted());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// ```compile_fail,E0599
/// fn bypass(runtime: &mut trust_runtime::Runtime) {
///     let _ = runtime.storage_mut();
/// }
/// ```
/// ```compile_fail,E0599
/// fn bypass(runtime: &mut trust_runtime::Runtime) {
///     let _ = runtime.registry_mut();
/// }
/// ```
/// ```compile_fail,E0599
/// fn bypass(runtime: &mut trust_runtime::Runtime) {
///     let _ = runtime.io_mut();
/// }
/// ```
///
/// Installed borrows end before the next exclusive operation:
///
/// ```compile_fail,E0502
/// use trust_runtime::Runtime;
/// fn held(runtime: &mut Runtime) {
///     let inputs = runtime.inputs();
///     let _ = runtime.execute_cycle();
///     let _ = inputs.len();
/// }
/// ```
///
/// ```
/// use trust_runtime::Runtime;
/// fn sequenced(runtime: &mut Runtime) {
///     let _ = runtime.inputs().len();
///     let _ = runtime.execute_cycle();
/// }
/// ```
///
/// Retained checkpoint capture needs the exclusive owner borrow (spec 34 §4.3.1):
///
/// ```compile_fail,E0596
/// fn shared_capture(runtime: &trust_runtime::Runtime) {
///     let _ = runtime.retain_snapshot();
/// }
/// ```
///
/// ```
/// fn exclusive_capture(runtime: &mut trust_runtime::Runtime) {
///     let _ = runtime.retain_snapshot();
/// }
/// ```
///
/// Engineering aliases are written through the checked `write_access`; no raw access map
/// is exposed:
///
/// ```compile_fail,E0599
/// fn bypass(runtime: &mut trust_runtime::Runtime) {
///     let _ = runtime.access_map_mut();
/// }
/// ```
///
/// ```
/// fn checked_alias(runtime: &mut trust_runtime::Runtime) {
///     let _ = runtime.write_access("alias", trust_runtime::value::Value::Int(1));
/// }
/// ```
///
/// The portable core's construction module is private to that core:
///
/// ```compile_fail,E0603
/// use trust_runtime::runtime_core::vm::construction;
/// ```
///
/// ```
/// use trust_runtime::runtime_core::vm::PreparedModule;
/// fn names(_: &PreparedModule) {}
/// ```
pub struct Runtime {
    state: OwnedRuntimeState<HostServices>,
}

impl Runtime {
    /// Decode, validate, prepare and construct a source-free application.
    pub fn from_bytes(bytes: &[u8], limits: PreparationLimits) -> Result<Self, RuntimeError> {
        Self::from_prepared(Arc::new(PreparedModule::from_bytes(bytes, limits)?), 0)
    }
    /// Construct one admitted resource with explicit host clock/deadline services.
    pub fn from_prepared(
        prepared: Arc<PreparedModule>,
        resource: usize,
    ) -> Result<Self, RuntimeError> {
        let state = prepared.instantiate_owned_with_services(resource, HostServices::default())?;
        Ok(Self { state })
    }
    /// Inspect immutable executable metadata without compiler types.
    pub fn prepared(&self) -> &PreparedModule {
        self.state.prepared()
    }
    /// Borrow authoritative values, without a mutable storage escape.
    pub fn storage(&self) -> &VariableStorage {
        self.state.storage()
    }
    /// Inspect the shared date/time representation.
    pub fn profile(&self) -> DateTimeProfile {
        *self.state.profile()
    }
    /// Read logical time independently of the host wall clock.
    pub fn current_time(&self) -> Duration {
        self.state.current_time()
    }
    /// Set logical time without changing task deadlines or state.
    pub fn set_current_time(&mut self, time: Duration) {
        self.state.set_current_time(time);
    }
    /// Advance logical time with the same saturation as the full host adapter.
    pub fn advance_time(&mut self, time: Duration) {
        self.set_current_time(Duration::from_nanos(
            self.current_time()
                .as_nanos()
                .saturating_add(time.as_nanos()),
        ));
    }
    /// Execute the shared scheduler at the current logical sample.
    pub fn execute_cycle(&mut self) -> Result<(), RuntimeError> {
        self.state.execute_cycle(self.state.current_time())
    }
    /// Borrow selected external adapters through the same narrow checked cycle port.
    /// This selects no built-in driver/protocol dependency and exposes no raw state.
    pub fn execute_cycle_with_hooks(
        &mut self,
        hooks: &mut dyn trust_runtime_core::vm::CycleHooks,
    ) -> Result<(), RuntimeError> {
        self.state
            .execute_cycle_with_hooks(self.state.current_time(), hooks)
    }
    /// Execute one installed program through the same engine and common allowance.
    pub fn execute_program(&mut self, name: &str) -> Result<(), RuntimeError> {
        self.state.execute_program(name)
    }
    /// Construct and execute a temporary admitted function-block template.
    pub fn execute_function_block_by_name(&mut self, name: &str) -> Result<(), RuntimeError> {
        self.state.execute_function_block_by_name(name)
    }
    /// Execute a currently live function-block destination through common policy.
    pub fn execute_function_block(&mut self, reference: &ValueRef) -> Result<(), RuntimeError> {
        self.state.execute_function_block(reference)
    }
    /// Reconstruct an isolated restart candidate before replacing mutable state.
    pub fn restart(&mut self, mode: RestartMode) -> Result<(), RuntimeError> {
        self.state.restart(mode)
    }
    /// Capture admitted retained destinations through common checked policy.
    pub fn retain_snapshot(&mut self) -> Result<RetainSnapshot, RuntimeError> {
        self.state.retain_snapshot()
    }
    /// Apply a typed retained checkpoint atomically through the shared owner.
    pub fn apply_retain_snapshot(
        &mut self,
        snapshot: &RetainSnapshot,
    ) -> Result<Vec<trust_runtime_core::vm::RetainApplyEvent>, RuntimeError> {
        self.state.apply_retain_snapshot(snapshot)
    }
    /// Inspect nominal task timing and overrun history.
    pub fn task_states(&self) -> &[TaskState] {
        self.state.task_states()
    }
    /// Inspect the existing common fault latch.
    pub fn last_fault(&self) -> Option<&RuntimeError> {
        self.state.fault()
    }
    /// Whether shared execution has latched a fault.
    pub fn faulted(&self) -> bool {
        self.state.fault().is_some()
    }
    /// Clear the latch without changing values, code or readiness.
    pub fn clear_fault(&mut self) {
        self.state.clear_fault();
    }
    /// Set an absolute physical deadline independently of PLC logical time.
    pub fn set_execution_deadline(&mut self, deadline: Option<Instant>) {
        self.state.services().deadline.set(deadline);
    }
    /// Inspect the currently selected physical deadline.
    pub fn execution_deadline(&self) -> Option<Instant> {
        self.state.services().deadline.get()
    }
    /// Read a currently bound value through common type/lifetime policy.
    pub fn read_reference(&mut self, reference: &ValueRef) -> Result<Value, RuntimeError> {
        self.state.read_reference(reference)
    }
    /// Commit a checked value update without executing or reinitializing code.
    pub fn write_reference(
        &mut self,
        reference: &ValueRef,
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.state.write_reference(reference.as_view(), value)
    }
    /// Write an admitted mutable global through the shared owner.
    pub fn write_global(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        self.state.write_global(name, value)
    }
    /// Write an installed named/member path with admitted types and bounds.
    pub fn write_named_path(
        &mut self,
        name: &str,
        path: &[RefSegment],
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.state.write_named_path(name, path, value)
    }
    /// Read a declared VAR_ACCESS alias through common checked policy.
    pub fn read_access(&mut self, name: &str) -> Result<Value, RuntimeError> {
        self.state.read_access(name)
    }
    /// Write an admitted engineering alias without a raw storage borrow.
    pub fn write_access(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        self.state.write_access(name, value)
    }
    /// Resolve an admitted alias to its current checked live destination.
    pub fn resolve_access_reference(&mut self, name: &str) -> Result<ValueRef, RuntimeError> {
        self.state.resolve_access_reference(name)
    }
    /// Borrow the authoritative flat input image without live resize/mutation.
    pub fn inputs(&self) -> &[u8] {
        self.state.inputs()
    }
    /// Borrow the flat published output image.
    pub fn outputs(&self) -> &[u8] {
        self.state.outputs()
    }
    /// Read one selected process-image value through common bounds and accounting.
    pub fn read_io(&mut self, address: &IoAddress) -> Result<Value, RuntimeError> {
        self.state.read_image(address)
    }
    /// Borrow an admitted hierarchical image value without cloning an entire image.
    pub fn read_hierarchical_image_ref(&self, address: &IoAddress) -> Option<&Value> {
        self.state.read_hierarchical_image_ref(address)
    }
    /// Commit one checked process-image value between owner entries.
    pub fn write_io(&mut self, address: &IoAddress, value: Value) -> Result<(), RuntimeError> {
        self.state.write_image(address, value)
    }

    /// Select existing prepared register telemetry without RUN allocations.
    #[cfg(feature = "register-ir")]
    pub fn set_vm_register_profile_enabled(&mut self, enabled: bool) {
        self.state.set_register_profile_enabled(enabled);
    }
    /// Clear execution observations while retaining prepared code and selections.
    #[cfg(feature = "register-ir")]
    pub fn reset_vm_register_profile(&mut self) {
        self.state.reset_register_profile();
    }
    /// Observe actual register execution and explicit stack fallback reasons.
    #[cfg(feature = "register-ir")]
    pub fn vm_register_profile_snapshot(
        &self,
    ) -> trust_runtime_core::vm::register::VmRegisterProfileSnapshot {
        self.state.register_profile_snapshot()
    }
    /// Select compatibility lowering observations; RUN never rebuilds code.
    #[cfg(feature = "register-ir")]
    pub fn set_vm_register_lowering_cache_enabled(&mut self, enabled: bool) {
        self.state.set_register_lowering_enabled(enabled);
    }
    /// Reset observations without discarding installed immutable plans.
    #[cfg(feature = "register-ir")]
    pub fn reset_vm_register_lowering_cache(&mut self) {
        self.state.reset_register_lowering_cache();
    }
    /// Read preparation counts separately from execution/cache observations.
    #[cfg(feature = "register-ir")]
    pub fn vm_register_lowering_cache_snapshot(
        &self,
    ) -> trust_runtime_core::vm::register::VmRegisterLoweringCacheSnapshot {
        self.state.register_lowering_snapshot()
    }
    /// Select prepared specialization without RUN compilation.
    #[cfg(feature = "register-ir")]
    pub fn set_vm_tier1_specialized_executor_enabled(&mut self, enabled: bool) {
        self.state.set_tier1_enabled(enabled);
    }
    /// Clear selection/hotness, retaining prepared code and reserved records.
    #[cfg(feature = "register-ir")]
    pub fn reset_vm_tier1_specialized_executor(&mut self) {
        self.state.reset_tier1();
    }
    /// Observe specialization preparation and execution separately.
    #[cfg(feature = "register-ir")]
    pub fn vm_tier1_specialized_executor_snapshot(
        &self,
    ) -> trust_runtime_core::vm::register::VmTier1SpecializedExecutorSnapshot {
        self.state.tier1_snapshot()
    }
}
