use super::super::dispatch::execute_pou_stack_with_locals;

fn vmpar_add_store_module() -> (VmModule, u32) {
    let mut code = Vec::new();
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x40);
    code.push(0x21);
    emit_u32(&mut code, 0);
    code.push(0x06);
    manual_vm_module(code, vec![Value::DInt(1)], 1)
}

fn vmpar_seed_runtime() -> Runtime {
    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::DInt(41));
    runtime
}

fn assert_deadline_exceeded(error: RuntimeError) {
    assert!(
        matches!(error, RuntimeError::ExecutionTimeout),
        "expected ExecutionTimeout, got {error:?}",
    );
}

#[test]
fn vmpar_stack_deadline_traps_before_forward_workload_commits() {
    let (module, pou_id) = vmpar_add_store_module();
    let mut runtime = vmpar_seed_runtime();
    runtime.set_execution_deadline(Some(Instant::now() - Duration::from_secs(1)));

    let error =
        execute_pou_stack_with_locals(&mut runtime, &module, pou_id, None, None, false, 0, None)
            .expect_err("expired deadline must trap stack VM before store");

    assert_deadline_exceeded(error);
    assert_eq!(runtime.storage().get_global("g0"), Some(&Value::DInt(41)));
}

#[test]
fn vmpar_register_deadline_traps_without_fallback_or_commit() {
    let (module, pou_id) = vmpar_add_store_module();
    let mut runtime = vmpar_seed_runtime();
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();
    runtime.set_execution_deadline(Some(Instant::now() - Duration::from_secs(1)));

    let error = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect_err("expired deadline must trap register IR before store");

    assert_deadline_exceeded(error);
    assert_eq!(runtime.storage().get_global("g0"), Some(&Value::DInt(41)));
    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(profile.register_programs_executed, 0, "profile={profile:?}");
    assert_eq!(profile.register_program_fallbacks, 0, "profile={profile:?}");
}

#[test]
fn vmpar_tier1_deadline_traps_in_compiled_block_without_commit() {
    let (module, pou_id) = vmpar_add_store_module();
    let mut runtime = vmpar_seed_runtime();
    runtime.set_vm_tier1_specialized_executor_enabled(true);
    runtime.reset_vm_tier1_specialized_executor();
    runtime.vm_tier1_specialized_executor.hot_block_threshold = 1;

    let warmup = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("warmup execution");
    assert_eq!(warmup, RegisterExecutionOutcome::Executed);
    assert_eq!(runtime.storage().get_global("g0"), Some(&Value::DInt(42)));
    let before_deadline = runtime.vm_tier1_specialized_executor_snapshot();
    assert!(
        before_deadline.compile_successes >= 1,
        "snapshot={before_deadline:?}",
    );
    assert!(
        before_deadline.block_executions >= 1,
        "snapshot={before_deadline:?}",
    );

    runtime.set_execution_deadline(Some(Instant::now() - Duration::from_secs(1)));
    let error = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect_err("expired deadline must trap tier1 before store");

    assert_deadline_exceeded(error);
    assert_eq!(runtime.storage().get_global("g0"), Some(&Value::DInt(42)));
    let after_deadline = runtime.vm_tier1_specialized_executor_snapshot();
    assert_eq!(
        after_deadline.block_executions, before_deadline.block_executions,
        "deadline trap must not be counted as a completed tier1 block: before={before_deadline:?} after={after_deadline:?}",
    );
    assert_eq!(
        after_deadline.deopt_count, before_deadline.deopt_count,
        "deadline trap must not be hidden as tier1 deopt: before={before_deadline:?} after={after_deadline:?}",
    );
}

#[test]
fn vmpar_stack_register_and_tier1_paths_produce_expected_forward_value() {
    let (module, pou_id) = vmpar_add_store_module();

    let mut stack_runtime = vmpar_seed_runtime();
    execute_pou_stack_with_locals(
        &mut stack_runtime,
        &module,
        pou_id,
        None,
        None,
        false,
        0,
        None,
    )
    .expect("stack VM execution");
    assert_eq!(
        stack_runtime.storage().get_global("g0"),
        Some(&Value::DInt(42))
    );

    let mut register_runtime = vmpar_seed_runtime();
    register_runtime.set_vm_register_profile_enabled(true);
    register_runtime.reset_vm_register_profile();
    let register_outcome =
        try_execute_pou_with_register_ir(&mut register_runtime, &module, pou_id, None)
            .expect("register IR execution");
    assert_eq!(register_outcome, RegisterExecutionOutcome::Executed);
    assert_eq!(
        register_runtime.storage().get_global("g0"),
        Some(&Value::DInt(42))
    );
    let register_profile = register_runtime.vm_register_profile_snapshot();
    assert_eq!(
        register_profile.register_program_fallbacks, 0,
        "profile={register_profile:?}",
    );

    let mut tier1_runtime = vmpar_seed_runtime();
    tier1_runtime.set_vm_tier1_specialized_executor_enabled(true);
    tier1_runtime.reset_vm_tier1_specialized_executor();
    tier1_runtime
        .vm_tier1_specialized_executor
        .hot_block_threshold = 1;
    let tier1_outcome = try_execute_pou_with_register_ir(&mut tier1_runtime, &module, pou_id, None)
        .expect("tier1-backed register execution");
    assert_eq!(tier1_outcome, RegisterExecutionOutcome::Executed);
    assert_eq!(
        tier1_runtime.storage().get_global("g0"),
        Some(&Value::DInt(42))
    );
    let tier1_profile = tier1_runtime.vm_tier1_specialized_executor_snapshot();
    assert!(
        tier1_profile.compile_successes >= 1,
        "snapshot={tier1_profile:?}",
    );
    assert!(
        tier1_profile.block_executions >= 1,
        "snapshot={tier1_profile:?}",
    );
    assert_eq!(
        tier1_profile.compile_failures, 0,
        "snapshot={tier1_profile:?}"
    );
    assert_eq!(tier1_profile.deopt_count, 0, "snapshot={tier1_profile:?}");
}
