//! Helpers and dispatcher instructions share fuel and physical-deadline cadence.
use super::*;
use crate::error::ExecutionLimitKind;
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
    let mut state_data = EngineState::construct_data(&prepared, 0, &probe).unwrap();
    let state = EngineState::borrow(&mut state_data, &prepared, &probe);
    state.data.resources.work_budget.reset(64);
    probe.calls.set(0);
    state.begin_execution(ExecutionEntry::Nested, 9999).unwrap();
    assert_eq!(
        state.data.resources.work_budget.remaining(),
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
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    assert!(state.charge_execution_work(1).is_err());
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
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
    let mut state_data =
        EngineState::construct_data(&prepared, 0, &super::services::LOGICAL_ONLY).unwrap();
    let mut state = EngineState::borrow(&mut state_data, &prepared, &super::services::LOGICAL_ONLY);
    let Some(Value::Instance(instance)) = state.data.storage.get_global("Plant") else {
        panic!("fixture root")
    };
    let instance = *instance;
    let pou = *state
        .data
        .construction
        .instance_templates
        .get(&instance)
        .unwrap();
    state.data.resources.work_budget.reset(0);
    let error = state.execute_pou(pou, Some(instance)).unwrap_err();
    assert!(matches!(
        error,
        RuntimeError::ExecutionLimit {
            kind: crate::error::ExecutionLimitKind::Work,
            used: 0,
            limit: 0,
            ..
        }
    ));
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    assert!(
        state.data.storage.execution_activation_count() == 0,
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
    let mut state_data = EngineState::construct_data(&prepared, 0, &probe).unwrap();
    let mut state = EngineState::borrow(&mut state_data, &prepared, &probe);
    let owner = state.data.storage.reserve_execution_frame().unwrap();
    state.data.storage.activate_execution_frame(owner).unwrap();
    let pou = *prepared.vm.function_block_ids.get("COUNTER").unwrap();
    state
        .data
        .storage
        .set_execution_frame_owner(owner, pou, None)
        .unwrap();
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
    state
        .data
        .resources
        .work_budget
        .reset(prepared.limits.max_work);
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
    // Retirement is bookkeeping (TIME-08): it neither samples the clock nor consumes
    // the allowance, so cleanup completes without re-observing the latched expiry.
    assert!(!state.data.resources.work_budget.charge(31).unwrap());
    let remaining = state.data.resources.work_budget.remaining();
    assert_eq!(state.retire_frame(&frame), Ok(()));
    assert_eq!(state.data.resources.work_budget.remaining(), remaining);
    assert_eq!(state.data.storage.execution_frame_count(), 0);
    assert!(state.data.storage.get_instance(owned).is_none());
    assert_eq!(state.data.storage.execution_activation_count(), 0);
    assert!(state.data.storage.execution_frame_owner(owner).is_none());
    assert!(!state.data.lifetimes.instance_lifetimes.contains_key(&owned));
    assert_eq!(
        probe.calls.get(),
        1,
        "terminal cleanup must not rediscover expiry"
    );

    state.data.resources.work_budget.reset(64);
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
    assert_eq!(state.data.resources.work_budget.remaining(), 32);
}

#[test]
fn construction_allowance_is_separate_from_the_scan_allowance() {
    let artifact = include_bytes!(
        "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
    );
    // Construction is one entry under max_construction_work (TIME-09), so a scan-sized
    // limit does not refuse it.
    let scan_limited = crate::vm::PreparationLimits {
        max_work: 10,
        ..Default::default()
    };
    let prepared = PreparedModule::from_bytes(artifact, scan_limited).unwrap();
    assert!(prepared.instantiate(0).is_ok());

    // The same program under an equal construction allowance refuses at its first
    // work charge beyond the limit.
    let construction_limited = crate::vm::PreparationLimits {
        max_construction_work: 10,
        ..Default::default()
    };
    let prepared = PreparedModule::from_bytes(artifact, construction_limited).unwrap();
    assert!(matches!(
        prepared.instantiate(0),
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            limit: 10,
            ..
        })
    ));
}

#[test]
fn scan_allowance_boundary_is_exact_at_the_measured_unit_count() {
    // TIME-08: each top-level invocation has its own `max_work`, so the least allowance under
    // which ten scans complete is the largest invocation's work N over those scans. The search
    // measures N rather than assuming an instruction count; ten scans make sure the tasks run.
    let artifact = include_bytes!(
        "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
    );
    let first_scan = |limit: usize| {
        let limits = crate::vm::PreparationLimits {
            max_work: limit,
            ..Default::default()
        };
        let prepared = PreparedModule::from_bytes(artifact, limits).unwrap();
        let mut state = prepared.instantiate(0).unwrap();
        for scan in 1..=10_i64 {
            state.execute_cycle(crate::value::Duration::from_millis(scan * 10))?;
        }
        Ok::<(), RuntimeError>(())
    };
    let mut low: usize = 1;
    let mut high: usize = crate::vm::PreparationLimits::default().max_work;
    assert_eq!(
        first_scan(high),
        Ok(()),
        "the default scan allowance must run ten scans"
    );
    while low < high {
        let middle = low + (high - low) / 2;
        if first_scan(middle).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let units = low;
    assert!(units > 1, "the scans must consume work");
    assert_eq!(first_scan(units), Ok(()), "exactly N units must complete");

    // The invocation of work N crosses N - 1 only with its final charge, so the refusal
    // reports used + requested == N against the limit N - 1 (vm/budget.rs:64-71).
    match first_scan(units - 1) {
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used,
            requested,
            limit,
        }) => {
            assert_eq!(limit, units - 1);
            assert_eq!(
                used + requested,
                units,
                "used {used}, requested {requested}"
            );
            assert!(used <= limit);
        }
        other => panic!("expected the exact work refusal at N - 1, got {other:?}"),
    }
}

#[test]
fn construction_allowance_boundary_is_exact_at_the_measured_unit_count() {
    let artifact = include_bytes!(
        "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
    );
    let admits = |limit: usize| {
        let limits = crate::vm::PreparationLimits {
            max_construction_work: limit,
            ..Default::default()
        };
        let prepared = PreparedModule::from_bytes(artifact, limits).unwrap();
        prepared.instantiate(0).is_ok()
    };
    // Admission is monotone in the construction allowance (TIME-09), so the least admitting
    // limit is the artifact's construction work N. The search measures N rather than
    // assuming a count.
    let mut low: usize = 1;
    let mut high: usize = crate::vm::PreparationLimits::default().max_construction_work;
    assert!(
        admits(high),
        "the default construction allowance must admit the artifact"
    );
    while low < high {
        let middle = low + (high - low) / 2;
        if admits(middle) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let units = low;
    assert!(units > 1, "constructing the artifact must consume work");
    assert!(
        admits(units),
        "exactly N construction units must instantiate"
    );

    // The first refused charge would take the total to exactly N: used + requested == N
    // (vm/budget.rs:64-71 refuses the whole charge and reports the units already used).
    let limits = crate::vm::PreparationLimits {
        max_construction_work: units - 1,
        ..Default::default()
    };
    let prepared = PreparedModule::from_bytes(artifact, limits).unwrap();
    let outcome = prepared.instantiate(0);
    match outcome {
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used,
            requested,
            limit,
        }) => {
            assert_eq!(limit, units - 1);
            assert_eq!(used + requested, units);
        }
        other => panic!(
            "N-1 construction units must refuse with ExecutionLimit(Work), got {:?}",
            other.err()
        ),
    }
}

#[test]
fn empty_work_preserves_exhaustion_deadline_stride_and_latch() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let clock = ClockProbe {
        calls: Cell::new(0),
        expired: Cell::new(false),
    };
    let mut data = EngineState::construct_data(&prepared, 0, &clock).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, &clock);
    let budget = &state.data.resources.work_budget;
    budget.reset(0);
    clock.calls.set(0);
    state.charge_work_units(0).unwrap();
    assert_eq!(budget.remaining(), 0);
    assert_eq!(clock.calls.get(), 0);
    assert!(matches!(
        state.charge_work_units(1),
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 1,
            limit: 0,
        })
    ));

    budget.reset(96);
    state.charge_work_units(31).unwrap();
    clock.expired.set(true);
    for _ in 0..64 {
        state.charge_work_units(0).unwrap();
    }
    assert_eq!(budget.remaining(), 65);
    assert_eq!(clock.calls.get(), 0);
    let deadline = crate::vm::VmTrap::DeadlineExceeded.into_runtime_error();
    assert_eq!(state.charge_work_units(1), Err(deadline.clone()));
    assert_eq!(budget.remaining(), 64);
    assert_eq!(clock.calls.get(), 1);

    // A no-op neither observes nor clears an already latched deadline.
    clock.expired.set(false);
    state.charge_work_units(0).unwrap();
    assert_eq!(budget.remaining(), 64);
    assert_eq!(state.charge_work_units(32), Err(deadline));
    assert_eq!(clock.calls.get(), 1);
}
