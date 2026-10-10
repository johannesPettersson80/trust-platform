//! Public state surface excludes internal storage and dispatcher context traits.
use super::{EngineState, ExecutionServices, PreparedModule};
use crate::{
    error::RuntimeError,
    memory::VariableStorage,
    retain::RestartMode,
    task::TaskState,
    value::{Duration, Value},
};

/// Single-owner execution of an immutable admitted application.
///
/// Runtime storage cannot be obtained mutably through the dispatcher's adapter
/// traits. Engineering writes use the typed methods on this state instead.
///
#[cfg_attr(
    feature = "hir",
    doc = r#"
```compile_fail,E0277
use trust_runtime_core::vm::{RuntimeState, hosted::context::ReferenceContext};
fn bypass_admission(state: &mut RuntimeState<'_>) {
    let _ = ReferenceContext::storage_mut(state);
}
```
"#
)]
pub struct RuntimeState<'a> {
    state: EngineState<'a>,
}

impl PreparedModule {
    /// Construct a fresh resource using only the admitted artifact and shared engine.
    pub fn instantiate(&self, resource_index: usize) -> Result<RuntimeState<'_>, RuntimeError> {
        self.instantiate_with_services(resource_index, &super::services::LOGICAL_ONLY)
    }

    /// Construct with explicit physical-deadline, UTC and retain platform services.
    pub fn instantiate_with_services<'a>(
        &'a self,
        resource_index: usize,
        services: &'a dyn ExecutionServices,
    ) -> Result<RuntimeState<'a>, RuntimeError> {
        Ok(RuntimeState {
            state: EngineState::new(self, resource_index, services)?,
        })
    }
}

impl RuntimeState<'_> {
    fn engineering_operation<T>(
        &mut self,
        action: impl FnOnce(&mut EngineState<'_>) -> Result<T, RuntimeError>,
    ) -> Result<T, RuntimeError> {
        self.state
            .resources
            .work_budget
            .reset(self.state.prepared.limits.max_work);
        self.state.resources.construction_bytes.set(0);
        self.state.resources.constructed_values = 0;
        self.state.check_entry_deadline()?;
        // Each operation checks completion after preparation and before its
        // final publication; no fallible clock sample follows a committed write.
        action(&mut self.state)
    }

    /// Inspect variables without exposing an untyped mutation path.
    pub fn storage(&self) -> &VariableStorage {
        self.state.storage()
    }
    /// Sample platform inputs before the next cooperative cycle.
    pub fn inputs_mut(&mut self) -> &mut [u8] {
        self.state.inputs_mut()
    }
    /// Borrow the last successfully published output image.
    pub fn outputs(&self) -> &[u8] {
        self.state.outputs()
    }
    /// Sample a declared hierarchical input through its admitted value codec.
    pub fn set_hierarchical_input(
        &mut self,
        address: &crate::io_address::IoAddress,
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.engineering_operation(|state| state.set_hierarchical_input(address, value))
    }
    /// Observe the last published value of an admitted hierarchical output binding.
    pub fn read_hierarchical_output(
        &self,
        address: &crate::io_address::IoAddress,
    ) -> Option<&Value> {
        self.state.read_hierarchical_output(address)
    }
    /// Inspect the latched execution fault, if any.
    pub fn fault(&self) -> Option<&RuntimeError> {
        self.state.fault()
    }
    /// Inspect nominal deadlines and missed-period counters.
    pub fn task_states(&self) -> &[TaskState] {
        self.state.task_states()
    }
    /// Execute one resource cycle using an explicitly sampled logical time.
    pub fn execute_cycle(&mut self, now: Duration) -> Result<(), RuntimeError> {
        self.state.execute_cycle(now)
    }
    /// Reconstruct at the current logical time, publishing only a complete candidate.
    pub fn restart(&mut self, mode: RestartMode) -> Result<(), RuntimeError> {
        self.state.restart(mode)
    }
    /// Read a declared access alias, including its partial selection.
    pub fn read_access(&mut self, name: &str) -> Result<Value, RuntimeError> {
        self.engineering_operation(|state| {
            let value = state.read_access(name)?;
            state.check_entry_deadline()?;
            Ok(value)
        })
    }
    /// Write a writable access alias using admitted type and lifetime checks.
    pub fn write_access(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        self.engineering_operation(|state| state.write_access(name, value))
    }
    /// Write a mutable resource-global variable between cycles.
    pub fn write_global(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        self.engineering_operation(|state| state.write_global(name, value))
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;

    #[test]
    fn engineering_entry_does_not_inherit_exhausted_scan_fuel() {
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut state = prepared.instantiate(0).unwrap();
        state.state.resources.work_budget.reset(0);
        state
            .engineering_operation(|engine| engine.charge_work_units(1))
            .unwrap();
        assert_eq!(
            state.state.resources.work_budget.remaining(),
            prepared.limits.max_work - 1
        );
        state.state.resources.work_budget.reset(0);
        state
            .engineering_operation(|engine| engine.charge_work_units(2))
            .unwrap();
        assert_eq!(
            state.state.resources.work_budget.remaining(),
            prepared.limits.max_work - 2
        );
    }
}
