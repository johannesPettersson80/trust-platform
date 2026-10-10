//! Helpers and dispatcher instructions share fuel and physical-deadline cadence.
use super::*;
use crate::vm::{budget::ExecutionEntry, context::ExecutionContext};
use core::cell::Cell;

struct ClockProbe {
    calls: Cell<usize>,
    expired: Cell<bool>,
}
impl ExecutionServices for ClockProbe {
    fn deadline_exceeded(&self) -> bool {
        self.calls.set(self.calls.get() + 1);
        self.expired.get()
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<crate::value::DateTimeValue, RuntimeError> {
        Err(RuntimeError::TypeMismatch)
    }
}

#[test]
fn instruction_and_helper_work_share_one_allowance_and_deadline_stride() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!(
            "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let probe = ClockProbe {
        calls: Cell::new(0),
        expired: Cell::new(false),
    };
    let state = EngineState::new(&prepared, 0, &probe).unwrap();
    state.resources.work_budget.reset(64);
    probe.calls.set(0);
    state.begin_execution(ExecutionEntry::Nested, 9999).unwrap();
    assert_eq!(
        state.resources.work_budget.remaining(),
        64,
        "nested entry cannot replenish fuel"
    );
    for _ in 0..31 {
        state.charge_work_units(1).unwrap();
    }
    assert_eq!(
        probe.calls.get(),
        1,
        "entry only, not one clock read per helper charge"
    );
    state.charge_execution_work(1).unwrap();
    assert_eq!(
        probe.calls.get(),
        2,
        "instruction crosses the same helper stride"
    );
    state.charge_work_units(32).unwrap();
    assert_eq!(probe.calls.get(), 3);
    assert_eq!(state.resources.work_budget.remaining(), 0);
    assert!(state.charge_execution_work(1).is_err());
    assert_eq!(state.resources.work_budget.remaining(), 0);
    probe.expired.set(true);
    assert_eq!(
        state.check_execution_deadline(),
        Err(crate::vm::VmTrap::DeadlineExceeded.into_runtime_error()),
        "completion must sample even after no additional work"
    );
}

#[test]
fn a_nested_dispatch_cannot_restart_an_exhausted_outer_allowance() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!(
            "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut state = EngineState::new(&prepared, 0, &super::services::LOGICAL_ONLY).unwrap();
    let Some(Value::Instance(instance)) = state.storage.get_global("Plant") else {
        panic!("fixture root")
    };
    let instance = *instance;
    let pou = state.construction.instance_templates[&instance];
    state.resources.work_budget.reset(0);
    let error = state.execute_pou(pou, Some(instance)).unwrap_err();
    assert_eq!(
        error,
        crate::vm::VmTrap::BudgetExceeded.into_runtime_error()
    );
    assert_eq!(state.resources.work_budget.remaining(), 0);
    assert!(
        state.lifetimes.live_activations.is_empty(),
        "failed entry retires its activation"
    );
}
