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
    let pou = *state
        .construction
        .instance_templates
        .get(&instance)
        .unwrap();
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

#[test]
fn expired_operation_retires_owned_state_without_resampling_and_reset_rearms_it() {
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
    let mut state = EngineState::new(&prepared, 0, &probe).unwrap();
    let owner = state.storage.reserve_execution_frame().unwrap();
    state.lifetimes.live_activations.push(owner);
    let pou = *prepared.vm.function_block_ids.get("COUNTER").unwrap();
    state.lifetimes.activation_pous.insert(owner, pou).unwrap();
    let owned = state.reserve_instance(pou, Some(owner)).unwrap();
    let frame = VmFrame {
        activation: Some(owner),
        pou_id: Some(pou),
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 0,
        locals: Vec::new(),
        parameter_values_present: Vec::new(),
        runtime_instance: None,
        instance_owner: None,
    };
    state.resources.work_budget.reset(prepared.limits.max_work);
    probe.calls.set(0);
    probe.expired.set(true);
    let deadline = crate::vm::VmTrap::DeadlineExceeded.into_runtime_error();
    assert_eq!(state.charge_work_units(32), Err(deadline.clone()));
    assert_eq!(probe.calls.get(), 1);
    // Even if the platform is subsequently disarmed, this operation is terminal.
    probe.expired.set(false);
    assert_eq!(state.check_entry_deadline(), Err(deadline.clone()));
    assert_eq!(state.check_execution_deadline(), Err(deadline.clone()));
    assert!(
        state.deadline_exceeded(),
        "native-call checks share the same latch"
    );
    assert_eq!(
        state.begin_execution(ExecutionEntry::Nested, usize::MAX),
        Err(deadline.clone())
    );
    assert_eq!(state.charge_execution_work(32), Err(deadline.clone()));
    assert_eq!(state.charge_work_units(32), Err(deadline.clone()));
    assert_eq!(probe.calls.get(), 1);
    // Arrange a stride crossing during the real retirement path, not a mock cleanup.
    assert!(!state.resources.work_budget.charge(31).unwrap());
    let remaining = state.resources.work_budget.remaining();
    assert_eq!(state.retire_frame(&frame), Err(deadline));
    assert!(state.resources.work_budget.remaining() < remaining);
    assert_eq!(state.storage.execution_frame_count(), 0);
    assert!(state.storage.get_instance(owned).is_none());
    assert!(state.lifetimes.live_activations.is_empty());
    assert!(state.lifetimes.activation_pous.get(&owner).is_none());
    assert!(!state.lifetimes.instance_lifetimes.contains_key(&owned));
    assert_eq!(
        probe.calls.get(),
        1,
        "terminal cleanup must not rediscover expiry"
    );

    state.resources.work_budget.reset(64);
    state
        .begin_execution(ExecutionEntry::Nested, usize::MAX)
        .unwrap();
    assert_eq!(
        probe.calls.get(),
        2,
        "new operation samples the platform again"
    );
    state.charge_execution_work(32).unwrap();
    assert_eq!(probe.calls.get(), 3);
    assert_eq!(state.resources.work_budget.remaining(), 32);
}
