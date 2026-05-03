#[test]
fn tier1_compiler_accepts_call_native_function_blocks() {
    let source = r#"
            VAR_GLOBAL
                g_value : DINT;
            END_VAR

            FUNCTION_BLOCK Counter
            VAR_OUTPUT
                Value : DINT;
            END_VAR

            Value := Value + DINT#1;
            END_FUNCTION_BLOCK

            PROGRAM Main
            VAR
                fb : Counter;
            END_VAR

            fb();
            g_value := fb.Value;
            END_PROGRAM
        "#;

    let bytecode = bytecode_module_from_source(source).expect("compile bytecode");
    let vm_module = VmModule::from_bytecode(&bytecode).expect("decode vm module");
    let main_pou_id = vm_module
        .program_ids
        .get(&SmolStr::new("MAIN"))
        .copied()
        .expect("main pou id");
    let lowered = lower_pou_to_register_ir(&vm_module, main_pou_id).expect("lower register ir");
    verify_register_program(&lowered).expect("verify register ir");
    let block = lowered
        .blocks
        .iter()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, RegisterInstr::CallNative { .. }))
        })
        .expect("call-native block");
    let key = super::tier1_block_key(&vm_module, main_pou_id, block);
    assert!(
        super::compile_tier1_block(&vm_module, block, key).is_ok(),
        "expected tier-1 compiler to accept CallNative block: {:?}",
        block.instructions
    );
}

#[test]
fn register_executor_tier1_specialized_executor_executes_function_call_block() {
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

    let mut harness = TestHarness::from_source(source).expect("create harness");
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("set backend");
    harness
        .runtime_mut()
        .restart(RestartMode::Cold)
        .expect("restart");
    harness
        .runtime_mut()
        .vm_tier1_specialized_executor
        .set_enabled(true);
    harness
        .runtime_mut()
        .vm_tier1_specialized_executor
        .hot_block_threshold = 1;
    harness.runtime_mut().reset_vm_tier1_specialized_executor();
    harness
        .runtime_mut()
        .vm_tier1_specialized_executor
        .hot_block_threshold = 1;

    let result = harness.cycle();
    assert!(
        result.errors.is_empty(),
        "cycle errors: {:?}",
        result.errors
    );

    assert_eq!(harness.get_output("g_value"), Some(Value::DInt(42)));
    let snapshot = harness.runtime().vm_tier1_specialized_executor_snapshot();
    assert!(snapshot.compile_successes >= 1, "snapshot={snapshot:?}");
    assert!(snapshot.block_executions >= 1, "snapshot={snapshot:?}");
    assert_eq!(snapshot.compile_failures, 0, "snapshot={snapshot:?}");
}

#[test]
fn register_executor_tier1_specialized_executor_executes_function_block_call_block() {
    let source = r#"
            VAR_GLOBAL
                g_value : DINT;
            END_VAR

            FUNCTION_BLOCK Counter
            VAR_OUTPUT
                Value : DINT;
            END_VAR

            Value := Value + DINT#1;
            END_FUNCTION_BLOCK

            PROGRAM Main
            VAR
                fb : Counter;
            END_VAR

            fb();
            g_value := fb.Value;
            END_PROGRAM
        "#;

    let mut harness = TestHarness::from_source(source).expect("create harness");
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("set backend");
    harness
        .runtime_mut()
        .restart(RestartMode::Cold)
        .expect("restart");
    harness
        .runtime_mut()
        .vm_tier1_specialized_executor
        .set_enabled(true);
    harness
        .runtime_mut()
        .vm_tier1_specialized_executor
        .hot_block_threshold = 1;
    harness.runtime_mut().reset_vm_tier1_specialized_executor();
    harness
        .runtime_mut()
        .vm_tier1_specialized_executor
        .hot_block_threshold = 1;

    for cycle in 0..3 {
        let result = harness.cycle();
        assert!(
            result.errors.is_empty(),
            "cycle {} errors: {:?}",
            cycle + 1,
            result.errors
        );
    }

    assert_eq!(harness.get_output("g_value"), Some(Value::DInt(3)));
    let snapshot = harness.runtime().vm_tier1_specialized_executor_snapshot();
    assert!(snapshot.compile_successes >= 1, "snapshot={snapshot:?}");
    assert!(snapshot.block_executions >= 1, "snapshot={snapshot:?}");
    assert_eq!(snapshot.compile_failures, 0, "snapshot={snapshot:?}");
}

#[test]
fn register_executor_tier1_specialized_executor_records_compile_failure_reason_for_unsupported_instruction(
) {
    let mut code = Vec::new();
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x61);
    code.push(0x12);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(7)], 0);

    let mut runtime = Runtime::new();
    runtime.set_vm_tier1_specialized_executor_enabled(true);
    runtime.reset_vm_tier1_specialized_executor();
    runtime.vm_tier1_specialized_executor.hot_block_threshold = 1;

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);

    let snapshot = runtime.vm_tier1_specialized_executor_snapshot();
    assert_eq!(snapshot.compile_attempts, 1);
    assert_eq!(snapshot.compile_failures, 1);
    assert!(
            snapshot.compile_failure_reasons.iter().any(|entry| {
                entry.reason == "unsupported_instr:size_of_value" && entry.count >= 1
            }),
            "expected SizeOfValue compile failure reason in tier-1 specialized executor snapshot, got {snapshot:?}",
        );
}

#[test]
fn register_executor_tier1_specialized_executor_executes_non_dint_binary_without_deopt() {
    let mut code = Vec::new();
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x40);
    code.push(0x21);
    emit_u32(&mut code, 0);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, vec![Value::Int(1)], 1);

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::Int(0));
    runtime.set_vm_tier1_specialized_executor_enabled(true);
    runtime.reset_vm_tier1_specialized_executor();

    for _ in 0..80 {
        let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
            .expect("execute register program");
        assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    }

    assert_eq!(runtime.storage().get_global("g0"), Some(&Value::Int(80)));
    let snapshot = runtime.vm_tier1_specialized_executor_snapshot();
    assert!(snapshot.compile_attempts >= 1);
    assert!(snapshot.compile_successes >= 1);
    assert!(snapshot.block_executions >= 1);
    assert_eq!(snapshot.deopt_count, 0, "snapshot={snapshot:?}");
    assert!(snapshot.deopt_reasons.is_empty(), "snapshot={snapshot:?}");
}

#[test]
fn register_executor_tier1_specialized_executor_cache_capacity_evicts_old_blocks() {
    let mut code_a = Vec::new();
    code_a.push(0x20);
    emit_u32(&mut code_a, 0);
    code_a.push(0x10);
    emit_u32(&mut code_a, 0);
    code_a.push(0x40);
    code_a.push(0x21);
    emit_u32(&mut code_a, 0);
    code_a.push(0x06);
    let (module_a, pou_a) = manual_vm_module(code_a, vec![Value::DInt(1)], 1);

    let mut code_b = Vec::new();
    code_b.push(0x20);
    emit_u32(&mut code_b, 0);
    code_b.push(0x10);
    emit_u32(&mut code_b, 0);
    code_b.push(0x41);
    code_b.push(0x21);
    emit_u32(&mut code_b, 0);
    code_b.push(0x06);
    let (module_b, pou_b) = manual_vm_module(code_b, vec![Value::DInt(1)], 1);

    let mut runtime = Runtime::new();
    runtime.vm_tier1_specialized_executor.set_enabled(true);
    runtime.vm_tier1_specialized_executor.hot_block_threshold = 1;
    runtime.vm_tier1_specialized_executor.cache_capacity = 1;
    runtime.reset_vm_tier1_specialized_executor();
    runtime.vm_tier1_specialized_executor.hot_block_threshold = 1;
    runtime.vm_tier1_specialized_executor.cache_capacity = 1;

    runtime.storage_mut().set_global("g0", Value::DInt(10));
    try_execute_pou_with_register_ir(&mut runtime, &module_a, pou_a, None)
        .expect("execute module a");
    runtime.storage_mut().set_global("g0", Value::DInt(10));
    try_execute_pou_with_register_ir(&mut runtime, &module_b, pou_b, None)
        .expect("execute module b");

    let snapshot = runtime.vm_tier1_specialized_executor_snapshot();
    assert_eq!(snapshot.cached_blocks, 1);
    assert!(
        snapshot.cache_evictions >= 1,
        "expected at least one cache eviction with cap=1",
    );
}

#[test]
fn register_executor_tier1_specialized_executor_cache_hits_reuse_compiled_block_arc() {
    let key = super::tier1::Tier1BlockKey {
        module_ptr: 1,
        pou_id: 2,
        block_id: 3,
        start_pc: 4,
    };
    let compiled = std::sync::Arc::new(super::tier1::Tier1CompiledBlock {
        key,
        instructions: vec![super::tier1::Tier1CompiledInstr::Return],
    });
    let mut state = super::RegisterTier1SpecializedExecutorState::default();

    state.insert_compiled_block(std::sync::Arc::clone(&compiled));
    let fetched = state.compiled_block(&key).cloned().expect("compiled block");

    assert!(std::sync::Arc::ptr_eq(&compiled, &fetched));
}

