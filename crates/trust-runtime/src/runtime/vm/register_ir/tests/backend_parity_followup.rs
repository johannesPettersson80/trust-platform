use super::super::dispatch::execute_pou_stack_with_locals;
use std::sync::Arc;

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

fn assert_instruction_budget_exceeded(error: RuntimeError) {
    assert!(
        matches!(error, RuntimeError::ExecutionTimeout),
        "expected instruction-budget ExecutionTimeout, got {error:?}",
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

#[test]
fn vmpar_instruction_budget_faults_at_the_same_original_instruction_boundary() {
    for (budget, expected_value) in [(3, 41), (4, 42)] {
        let (mut stack_module, pou_id) = vmpar_add_store_module();
        stack_module.instruction_budget = budget;
        let mut stack_runtime = vmpar_seed_runtime();
        let stack_error = execute_pou_stack_with_locals(
            &mut stack_runtime,
            &stack_module,
            pou_id,
            None,
            None,
            false,
            0,
            None,
        )
        .expect_err("stack path must exhaust the fixed test budget");
        assert_instruction_budget_exceeded(stack_error);
        assert_eq!(
            stack_runtime.storage().get_global("g0"),
            Some(&Value::DInt(expected_value))
        );

        let (mut register_module, pou_id) = vmpar_add_store_module();
        register_module.instruction_budget = budget;
        let mut register_runtime = vmpar_seed_runtime();
        let register_error =
            try_execute_pou_with_register_ir(&mut register_runtime, &register_module, pou_id, None)
                .expect_err("register path must exhaust the fixed test budget");
        assert_instruction_budget_exceeded(register_error);
        assert_eq!(
            register_runtime.storage().get_global("g0"),
            Some(&Value::DInt(expected_value))
        );

        let (mut tier1_module, pou_id) = vmpar_add_store_module();
        let mut tier1_runtime = vmpar_seed_runtime();
        tier1_runtime.set_vm_tier1_specialized_executor_enabled(true);
        tier1_runtime.reset_vm_tier1_specialized_executor();
        tier1_runtime
            .vm_tier1_specialized_executor
            .hot_block_threshold = 1;
        try_execute_pou_with_register_ir(&mut tier1_runtime, &tier1_module, pou_id, None)
            .expect("tier1 warmup must compile the block");
        tier1_runtime
            .storage_mut()
            .set_global("g0", Value::DInt(41));
        tier1_module.instruction_budget = budget;
        let tier1_error =
            try_execute_pou_with_register_ir(&mut tier1_runtime, &tier1_module, pou_id, None)
                .expect_err("tier1 path must exhaust the fixed test budget");
        assert_instruction_budget_exceeded(tier1_error);
        assert_eq!(
            tier1_runtime.storage().get_global("g0"),
            Some(&Value::DInt(expected_value))
        );
    }
}

#[test]
fn vmpar_nested_function_call_shares_the_top_level_instruction_budget() {
    let source = r#"
        VAR_GLOBAL
            g_value : DINT;
        END_VAR

        FUNCTION AddOne : DINT
        VAR_INPUT
            Input : DINT;
        END_VAR
        AddOne := Input + DINT#1;
        END_FUNCTION

        PROGRAM Main
        g_value := AddOne(Input := DINT#41);
        END_PROGRAM
    "#;
    let mut harness = TestHarness::from_source(source).expect("create nested-budget harness");
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("select VM backend");
    harness.runtime_mut().set_execution_deadline(None);

    let module = Arc::make_mut(
        harness
            .runtime_mut()
            .vm_module
            .as_mut()
            .expect("harness must load a VM module"),
    );
    let main_id = *module
        .program_ids
        .get(&SmolStr::new("MAIN"))
        .expect("main POU id");
    let add_one_id = *module
        .function_ids
        .get(&SmolStr::new("ADDONE"))
        .expect("AddOne POU id");
    let main_count = lower_pou_to_register_ir(module, main_id)
        .expect("lower Main")
        .blocks
        .iter()
        .map(|block| block.bytecode_instruction_count)
        .sum::<usize>();
    let callee_count = lower_pou_to_register_ir(module, add_one_id)
        .expect("lower AddOne")
        .blocks
        .iter()
        .map(|block| block.bytecode_instruction_count)
        .sum::<usize>();
    assert!(main_count > 0 && callee_count > 0);
    module.instruction_budget = main_count.max(callee_count);

    let cycle = harness.cycle();
    assert!(
        cycle
            .errors
            .iter()
            .any(|error| matches!(error, RuntimeError::ExecutionTimeout)),
        "nested call must consume the caller's remaining budget: main={main_count}, callee={callee_count}, errors={:?}",
        cycle.errors
    );
}
