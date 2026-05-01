#[test]
fn register_lowering_cache_hits_after_first_execution() {
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
    runtime.set_vm_register_lowering_cache_enabled(true);
    runtime.reset_vm_register_lowering_cache();
    runtime.storage_mut().set_global("g0", Value::DInt(1));

    let first = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("first execution");
    let second = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("second execution");
    assert_eq!(first, RegisterExecutionOutcome::Executed);
    assert_eq!(second, RegisterExecutionOutcome::Executed);

    let snapshot = runtime.vm_register_lowering_cache_snapshot();
    assert!(snapshot.enabled);
    assert_eq!(snapshot.cached_entries, 1);
    assert_eq!(snapshot.misses, 1);
    assert_eq!(snapshot.hits, 1);
    assert_eq!(snapshot.build_errors, 0);
}

#[test]
fn register_lowering_cache_caches_lowering_errors() {
    let mut code = Vec::new();
    code.push(0x02);
    emit_i32(&mut code, 4096);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, Vec::new(), 0);

    let mut runtime = Runtime::new();
    runtime.set_vm_register_lowering_cache_enabled(true);
    runtime.reset_vm_register_lowering_cache();

    let first = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("first fallback");
    let second = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("second fallback");
    assert_eq!(first, RegisterExecutionOutcome::FallbackToStack);
    assert_eq!(second, RegisterExecutionOutcome::FallbackToStack);

    let snapshot = runtime.vm_register_lowering_cache_snapshot();
    assert!(snapshot.enabled);
    assert_eq!(snapshot.cached_entries, 1);
    assert_eq!(snapshot.misses, 1);
    assert_eq!(snapshot.hits, 1);
    assert_eq!(snapshot.build_errors, 1);
}

#[test]
fn register_executor_tier1_specialized_executor_keeps_startup_path_cold_until_hot_threshold() {
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
    runtime.storage_mut().set_global("g0", Value::DInt(41));
    runtime.set_vm_tier1_specialized_executor_enabled(true);
    runtime.reset_vm_tier1_specialized_executor();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    let snapshot = runtime.vm_tier1_specialized_executor_snapshot();
    assert_eq!(snapshot.compile_attempts, 0);
    assert_eq!(snapshot.block_executions, 0);
}

#[test]
fn tier1_compiler_accepts_load_ref_addr_dynamic_block() {
    let mut code = Vec::new();
    code.push(0x22);
    emit_u32(&mut code, 0);
    code.push(0x32);
    code.push(0x21);
    emit_u32(&mut code, 1);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, Vec::new(), 2);

    let lowered = lower_pou_to_register_ir(&module, pou_id).expect("lower register ir");
    verify_register_program(&lowered).expect("verify register ir");
    let block = lowered
        .blocks
        .iter()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, RegisterInstr::LoadRefAddr { .. }))
                && block
                    .instructions
                    .iter()
                    .any(|instruction| matches!(instruction, RegisterInstr::LoadDynamic { .. }))
        })
        .expect("load-ref-addr block");
    let key = super::tier1_block_key(&module, pou_id, block);
    assert!(
        super::compile_tier1_block(&module, block, key).is_ok(),
        "expected tier-1 compiler to accept LoadRefAddr block: {:?}",
        block.instructions
    );
}

#[test]
fn register_executor_tier1_specialized_executor_executes_load_ref_addr_block() {
    let mut code = Vec::new();
    code.push(0x22);
    emit_u32(&mut code, 0);
    code.push(0x32);
    code.push(0x21);
    emit_u32(&mut code, 1);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, Vec::new(), 2);

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::DInt(41));
    runtime.storage_mut().set_global("g1", Value::DInt(0));
    runtime.set_vm_tier1_specialized_executor_enabled(true);
    runtime.reset_vm_tier1_specialized_executor();
    runtime.vm_tier1_specialized_executor.hot_block_threshold = 1;

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    assert_eq!(runtime.storage().get_global("g1"), Some(&Value::DInt(41)));

    let snapshot = runtime.vm_tier1_specialized_executor_snapshot();
    assert!(snapshot.compile_successes >= 1, "snapshot={snapshot:?}");
    assert!(snapshot.block_executions >= 1, "snapshot={snapshot:?}");
    assert_eq!(snapshot.compile_failures, 0, "snapshot={snapshot:?}");
}

#[test]
fn tier1_compiler_accepts_load_super_dynamic_block() {
    let mut code = Vec::new();
    code.push(0x24);
    code.push(0x30);
    emit_u32(&mut code, 0);
    code.push(0x32);
    code.push(0x21);
    emit_u32(&mut code, 0);
    code.push(0x06);

    let pou_id = 1_u32;
    let mut pou_by_id = HashMap::new();
    pou_by_id.insert(
        pou_id,
        VmPouEntry {
            name: SmolStr::new("MAIN"),
            code_start: 0,
            code_end: code.len(),
            local_ref_start: 0,
            local_ref_count: 0,
            primary_instance_owner: None,
        },
    );
    let mut program_ids = HashMap::new();
    program_ids.insert(SmolStr::new("MAIN"), pou_id);
    let module = VmModule {
        code,
        strings: vec![SmolStr::new("COUNT")],
        types: TypeTable::default(),
        refs: vec![VmRef::Global {
            offset: 0,
            path: RefPath::new(),
        }],
        consts: Vec::new(),
        pou_by_id,
        program_ids,
        function_ids: HashMap::new(),
        function_block_ids: HashMap::new(),
        class_ids: HashMap::new(),
        native_symbol_specs: Vec::new(),
        pou_params: HashMap::new(),
        pou_has_return_slot: HashSet::new(),
        method_table_by_owner: HashMap::new(),
        debug_map: super::super::debug_map::VmDebugMap::default(),
        instruction_budget: super::super::DEFAULT_INSTRUCTION_BUDGET,
    };

    let lowered = lower_pou_to_register_ir(&module, pou_id).expect("lower register ir");
    verify_register_program(&lowered).expect("verify register ir");
    let block = lowered
        .blocks
        .iter()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, RegisterInstr::LoadSuper { .. }))
                && block
                    .instructions
                    .iter()
                    .any(|instruction| matches!(instruction, RegisterInstr::LoadDynamic { .. }))
        })
        .expect("load-super block");
    let key = super::tier1_block_key(&module, pou_id, block);
    assert!(
        super::compile_tier1_block(&module, block, key).is_ok(),
        "expected tier-1 compiler to accept LoadSuper block: {:?}",
        block.instructions
    );
}

#[test]
fn register_executor_tier1_specialized_executor_executes_load_super_block() {
    let mut code = Vec::new();
    code.push(0x24);
    code.push(0x30);
    emit_u32(&mut code, 0);
    code.push(0x32);
    code.push(0x21);
    emit_u32(&mut code, 0);
    code.push(0x06);

    let pou_id = 1_u32;
    let mut pou_by_id = HashMap::new();
    pou_by_id.insert(
        pou_id,
        VmPouEntry {
            name: SmolStr::new("MAIN"),
            code_start: 0,
            code_end: code.len(),
            local_ref_start: 0,
            local_ref_count: 0,
            primary_instance_owner: None,
        },
    );
    let mut program_ids = HashMap::new();
    program_ids.insert(SmolStr::new("MAIN"), pou_id);
    let module = VmModule {
        code,
        strings: vec![SmolStr::new("COUNT")],
        types: TypeTable::default(),
        refs: vec![VmRef::Global {
            offset: 0,
            path: RefPath::new(),
        }],
        consts: Vec::new(),
        pou_by_id,
        program_ids,
        function_ids: HashMap::new(),
        function_block_ids: HashMap::new(),
        class_ids: HashMap::new(),
        native_symbol_specs: Vec::new(),
        pou_params: HashMap::new(),
        pou_has_return_slot: HashSet::new(),
        method_table_by_owner: HashMap::new(),
        debug_map: super::super::debug_map::VmDebugMap::default(),
        instruction_budget: super::super::DEFAULT_INSTRUCTION_BUDGET,
    };

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::DInt(0));
    let base = runtime.storage_mut().create_instance("BASE");
    let derived = runtime.storage_mut().create_instance("DERIVED");
    runtime
        .storage_mut()
        .get_instance_mut(derived)
        .expect("derived instance")
        .parent = Some(base);
    assert!(runtime
        .storage_mut()
        .set_instance_var(base, "COUNT", Value::DInt(10)));
    runtime.set_vm_tier1_specialized_executor_enabled(true);
    runtime.reset_vm_tier1_specialized_executor();
    runtime.vm_tier1_specialized_executor.hot_block_threshold = 1;
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, Some(derived))
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    assert_eq!(runtime.storage().get_global("g0"), Some(&Value::DInt(10)));

    let tier1 = runtime.vm_tier1_specialized_executor_snapshot();
    assert!(tier1.compile_successes >= 1, "snapshot={tier1:?}");
    assert!(tier1.block_executions >= 1, "snapshot={tier1:?}");
    assert_eq!(tier1.compile_failures, 0, "snapshot={tier1:?}");
    assert_eq!(tier1.deopt_count, 0, "snapshot={tier1:?}");

    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(profile.register_program_fallbacks, 0, "profile={profile:?}");
    assert_eq!(profile.ref_ops.load_dynamic, 1, "profile={profile:?}");
    assert_eq!(
        profile.ref_ops.instance_field_lookups, 1,
        "profile={profile:?}"
    );
}

#[test]
fn register_executor_tier1_specialized_executor_executes_bool_or_without_deopt() {
    let mut code = Vec::new();
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x47);
    code.push(0x21);
    emit_u32(&mut code, 0);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, vec![Value::Bool(true)], 1);

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::Bool(false));
    runtime.set_vm_tier1_specialized_executor_enabled(true);
    runtime.reset_vm_tier1_specialized_executor();
    runtime.vm_tier1_specialized_executor.hot_block_threshold = 1;
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    assert_eq!(runtime.storage().get_global("g0"), Some(&Value::Bool(true)));

    let snapshot = runtime.vm_tier1_specialized_executor_snapshot();
    assert!(snapshot.compile_successes >= 1, "snapshot={snapshot:?}");
    assert!(snapshot.block_executions >= 1, "snapshot={snapshot:?}");
    assert_eq!(snapshot.deopt_count, 0, "snapshot={snapshot:?}");
    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(
        profile.value_ops.read_value_clones, 0,
        "profile={profile:?}"
    );
    assert_eq!(
        profile.value_ops.const_load_clones, 0,
        "profile={profile:?}"
    );
}

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
