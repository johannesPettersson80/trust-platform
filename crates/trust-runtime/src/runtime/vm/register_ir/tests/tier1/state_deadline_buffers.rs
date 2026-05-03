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

    let saved = [
        (ENABLED, std::env::var_os(ENABLED)),
        (THRESHOLD, std::env::var_os(THRESHOLD)),
        (CACHE_CAP, std::env::var_os(CACHE_CAP)),
    ];
    std::env::set_var(ENABLED, "false");
    std::env::set_var(THRESHOLD, "7");
    std::env::set_var(CACHE_CAP, "9");

    let snapshot = super::RegisterTier1SpecializedExecutorState::from_env().snapshot();
    restore_env_vars(saved);

    assert!(!snapshot.enabled);
    assert_eq!(snapshot.hot_block_threshold, 7);
    assert_eq!(snapshot.cache_capacity, 9);
}

#[test]
fn register_executor_tier1_env_parsers_accept_tokens_and_defaults() {
    let bool_key = "TRUST_TEST_TIER1_ENV_BOOL";
    std::env::remove_var(bool_key);
    assert!(parse_tier1_env_bool(bool_key, true));
    assert!(!parse_tier1_env_bool(bool_key, false));

    for value in ["1", "true", "YES", " on "] {
        std::env::set_var(bool_key, value);
        assert!(
            parse_tier1_env_bool(bool_key, false),
            "expected true for {value:?}"
        );
    }
    for value in ["0", "false", "NO", " off "] {
        std::env::set_var(bool_key, value);
        assert!(
            !parse_tier1_env_bool(bool_key, true),
            "expected false for {value:?}"
        );
    }
    std::env::set_var(bool_key, "maybe");
    assert!(parse_tier1_env_bool(bool_key, true));
    assert!(!parse_tier1_env_bool(bool_key, false));
    std::env::remove_var(bool_key);

    let usize_key = "TRUST_TEST_TIER1_ENV_USIZE";
    std::env::remove_var(usize_key);
    assert_eq!(parse_tier1_env_usize(usize_key, 128), 128);
    std::env::set_var(usize_key, "9");
    assert_eq!(parse_tier1_env_usize(usize_key, 128), 9);
    std::env::set_var(usize_key, "bad");
    assert_eq!(parse_tier1_env_usize(usize_key, 128), 128);
    std::env::remove_var(usize_key);
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
    assert!(after.compile_failure_reasons.is_empty(), "snapshot={after:?}");
}

fn restore_env_vars<const N: usize>(saved: [(&'static str, Option<std::ffi::OsString>); N]) {
    for (key, value) in saved {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }
}

#[test]
fn register_deadline_stride_checks_first_and_stride_boundaries() {
    assert!(super::should_check_register_deadline(0));
    assert!(!super::should_check_register_deadline(1));
    assert!(super::should_check_register_deadline(
        super::REGISTER_DEADLINE_CHECK_STRIDE
    ));
    assert!(super::should_check_register_deadline(
        super::REGISTER_DEADLINE_CHECK_STRIDE * 2
    ));
}

#[test]
fn register_execution_buffers_reuse_clears_frames_and_register_files() {
    super::VM_REGISTER_FRAME_STACK_POOL.with(|pool| pool.borrow_mut().clear());
    super::VM_REGISTER_FILE_POOL.with(|pool| pool.borrow_mut().clear());
    super::VM_REGISTER_READ_COUNTS_POOL.with(|pool| pool.borrow_mut().clear());

    {
        let mut buffers = super::RegisterExecutionBuffers::acquire(3);
        let (frames, registers, remaining_reads, _) = buffers.buffers_mut();
        frames
            .push(super::super::frames::VmFrame {
                pou_id: 1,
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

    let mut buffers = super::RegisterExecutionBuffers::acquire(3);
    let (frames, registers, remaining_reads, _) = buffers.buffers_mut();
    assert!(frames.is_empty());
    assert!(registers.iter().all(|value| matches!(value, Value::Null)));
    assert!(remaining_reads.iter().all(|count| *count == 0));
}

// ── P2 register-executor corpus diagnostic tests ──
