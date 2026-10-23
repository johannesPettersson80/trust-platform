#[test]
fn register_executor_tier1_state_defaults_disabled() {
    let state = super::RegisterTier1SpecializedExecutorState::default();

    assert!(!state.enabled());
    assert!(!state.snapshot().enabled);
}

#[test]
fn register_executor_tier1_state_from_env_reads_threshold_and_cache() {
    const ENABLED: &str = "TRUST_VM_TIER1_SPECIALIZED_EXECUTOR";
    const THRESHOLD: &str = "TRUST_VM_TIER1_SPECIALIZED_EXECUTOR_HOT_THRESHOLD";
    const CACHE_CAP: &str = "TRUST_VM_TIER1_SPECIALIZED_EXECUTOR_CACHE_CAP";

    let values = [(ENABLED, "false"), (THRESHOLD, "7"), (CACHE_CAP, "9")];
    let snapshot = super::RegisterTier1SpecializedExecutorState::from_environment(|name| {
        values
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
    })
    .snapshot();

    assert!(!snapshot.enabled);
    assert_eq!(snapshot.hot_block_threshold, 7);
    assert_eq!(snapshot.cache_capacity, 9);
}

#[test]
fn register_executor_tier1_env_parsers_accept_tokens_and_defaults() {
    assert!(parse_bool(None, true));
    assert!(!parse_bool(None, false));
    for value in ["1", "true", "YES", " on "] {
        assert!(
            parse_bool(Some(value), false),
            "expected true for {value:?}"
        );
    }
    for value in ["0", "false", "NO", " off "] {
        assert!(
            !parse_bool(Some(value), true),
            "expected false for {value:?}"
        );
    }
    assert!(parse_bool(Some("maybe"), true));
    assert!(!parse_bool(Some("maybe"), false));
    assert_eq!(parse_number(None, 128_usize), 128);
    assert_eq!(parse_number(Some("9"), 128_usize), 9);
    assert_eq!(parse_number(Some("bad"), 128_usize), 128);
}

#[test]
fn register_executor_tier1_environment_defaults_and_capacity_clamp() {
    let state = super::RegisterTier1SpecializedExecutorState::from_environment(|_| None).snapshot();
    assert!(!state.enabled);
    assert_eq!(state.hot_block_threshold, 64);
    assert_eq!(state.cache_capacity, 128);
    let state = super::RegisterTier1SpecializedExecutorState::from_environment(|name| {
        Some(
            if name.ends_with("CACHE_CAP") {
                "0"
            } else {
                "invalid"
            }
            .to_owned(),
        )
    })
    .snapshot();
    assert!(!state.enabled);
    assert_eq!(state.hot_block_threshold, 64);
    assert_eq!(state.cache_capacity, 1);
}

#[test]
fn register_executor_tier1_state_reset_clears_cache_and_counters() {
    let mut code = Vec::new();
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x40);
    code.push(0x21);
    emit_u32(&mut code, 0);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(1)], 1);

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::DInt(1));
    runtime.vm_tier1_specialized_executor.set_enabled(true);
    runtime.vm_tier1_specialized_executor.hot_block_threshold = 1;

    let mut runtime = LegacyRegisterFixtureContext::new(&mut runtime, &module)
        .expect("prepare legacy register fixture");
    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);

    let before = runtime.vm_tier1_specialized_executor_snapshot();
    assert!(before.cached_blocks >= 1, "snapshot={before:?}");
    assert!(before.compile_attempts >= 1, "snapshot={before:?}");
    assert!(before.compile_successes >= 1, "snapshot={before:?}");
    assert!(before.block_executions >= 1, "snapshot={before:?}");

    runtime.reset_vm_tier1_specialized_executor();

    let after = runtime.vm_tier1_specialized_executor_snapshot();
    assert!(after.enabled);
    assert_eq!(after.cached_blocks, 0, "snapshot={after:?}");
    assert_eq!(after.compile_attempts, 0, "snapshot={after:?}");
    assert_eq!(after.compile_successes, 0, "snapshot={after:?}");
    assert_eq!(after.compile_failures, 0, "snapshot={after:?}");
    assert_eq!(after.cache_evictions, 0, "snapshot={after:?}");
    assert_eq!(after.block_executions, 0, "snapshot={after:?}");
    assert!(
        after.compile_failure_reasons.is_empty(),
        "snapshot={after:?}"
    );
}

#[test]
fn register_deadline_stride_carries_remaining_work_between_blocks() {
    let budget = trust_runtime_core::vm::hosted::budget::ExecutionBudget::new(64);
    assert!(!budget.charge(17).unwrap());
    assert!(!budget.charge(14).unwrap());
    assert!(budget.charge(1).unwrap());
    assert_eq!(budget.remaining(), 32);
}

#[test]
fn register_execution_buffers_reuse_clears_frames_and_register_files() {
    let mut buffers =
        super::RegisterExecutionBuffers::reserve(3, 0).expect("reserve owned scratch");
    {
        let (frames, registers, remaining_reads, _) = buffers.buffers_mut();
        frames
            .push(super::super::frames::VmFrame {
                parameter_values_present: Vec::new(),
                activation: None,
                pou_id: Some(1),
                return_pc: 2,
                code_start: 3,
                code_end: 4,
                local_ref_start: 0,
                local_ref_count: 1,
                locals: vec![Value::DInt(9)],
                runtime_instance: None,
                instance_owner: None,
            })
            .expect("push pooled frame");
        registers[0] = Value::DInt(7);
        remaining_reads[0] = 11;
    }

    buffers.select(3).expect("reuse admitted scratch");
    let (frames, registers, remaining_reads, _) = buffers.buffers_mut();
    assert!(frames.is_empty());
    assert!(registers.iter().all(|value| matches!(value, Value::Null)));
    assert!(remaining_reads.iter().all(|count| *count == 0));
}

// ── P2 register-executor corpus diagnostic tests ──
