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
fn tier1_dint_binary_guard_returns_exact_arithmetic_results() {
    let cases = [
        (BinaryOp::Add, 7, 5, Value::DInt(12)),
        (BinaryOp::Sub, 7, 5, Value::DInt(2)),
        (BinaryOp::Mul, 7, 5, Value::DInt(35)),
        (BinaryOp::Div, 8, 2, Value::DInt(4)),
        (BinaryOp::Mod, 8, 3, Value::DInt(2)),
    ];

    for (op, left, right, expected) in cases {
        assert_eq!(
            super::apply_dint_binary_guard_borrowed(
                op,
                &Value::DInt(left),
                &Value::DInt(right),
            )
            .expect("guard result"),
            Some(expected),
            "unexpected guard result for {op:?}",
        );
    }

    let div_zero = super::apply_dint_binary_guard_borrowed(
        BinaryOp::Div,
        &Value::DInt(8),
        &Value::DInt(0),
    )
    .expect_err("division by zero should fail");
    assert!(matches!(div_zero, RuntimeError::DivisionByZero));
}

#[test]
fn tier1_dint_binary_guard_returns_exact_comparison_results() {
    let cases = [
        (BinaryOp::Eq, 7, 7, Value::Bool(true)),
        (BinaryOp::Ne, 7, 5, Value::Bool(true)),
        (BinaryOp::Lt, 5, 7, Value::Bool(true)),
        (BinaryOp::Lt, 7, 7, Value::Bool(false)),
        (BinaryOp::Le, 7, 7, Value::Bool(true)),
        (BinaryOp::Gt, 7, 5, Value::Bool(true)),
        (BinaryOp::Gt, 7, 7, Value::Bool(false)),
        (BinaryOp::Ge, 7, 7, Value::Bool(true)),
    ];

    for (op, left, right, expected) in cases {
        assert_eq!(
            super::apply_dint_binary_guard_borrowed(
                op,
                &Value::DInt(left),
                &Value::DInt(right),
            )
            .expect("guard result"),
            Some(expected),
            "unexpected guard result for {op:?}",
        );
    }
}

#[test]
fn tier1_dint_binary_guard_declines_unsupported_inputs() {
    assert_eq!(
        super::apply_dint_binary_guard_borrowed(
            BinaryOp::Add,
            &Value::Bool(true),
            &Value::DInt(1),
        )
        .expect("guard result"),
        None
    );
    assert_eq!(
        super::apply_dint_binary_guard_borrowed(
            BinaryOp::And,
            &Value::DInt(1),
            &Value::DInt(1),
        )
        .expect("guard result"),
        None
    );
}

#[test]
fn tier1_compiler_accepts_all_fused_binary_register_forms() {
    let instructions = [
        RegisterInstr::BinaryRefToRef {
            op: BinaryOp::Add,
            left_ref_idx: 0,
            right_ref_idx: 1,
            dest_ref_idx: 2,
        },
        RegisterInstr::BinaryRefConstToRef {
            op: BinaryOp::Sub,
            left_ref_idx: 0,
            const_idx: 0,
            dest_ref_idx: 2,
        },
        RegisterInstr::BinaryConstRefToRef {
            op: BinaryOp::Mul,
            const_idx: 0,
            right_ref_idx: 1,
            dest_ref_idx: 2,
        },
    ];

    for instruction in instructions {
        compile_single_tier1_instruction(instruction).expect("fused binary should compile");
    }
}

#[test]
fn tier1_compiler_accepts_cmp_ref_const_jump_only_for_comparisons() {
    compile_single_tier1_instruction(RegisterInstr::CmpRefConstJumpIf {
        op: BinaryOp::Lt,
        ref_idx: 0,
        const_idx: 0,
        jump_if_true: true,
        target: BlockTarget::Exit,
    })
    .expect("comparison branch should compile");

    let err =
        compile_single_tier1_instruction(RegisterInstr::CmpRefConstJumpIf {
            op: BinaryOp::Add,
            ref_idx: 0,
            const_idx: 0,
            jump_if_true: true,
            target: BlockTarget::Exit,
        })
        .expect_err("non-comparison branch should be rejected");
    assert!(
        err.contains("unsupported_cmp_op:add"),
        "unexpected compile error: {err}",
    );
}

fn compile_single_tier1_instruction(instruction: RegisterInstr) -> Result<(), String> {
    let (module, pou_id) = manual_vm_module(Vec::new(), vec![Value::DInt(1)], 3);
    let block = RegisterBlock {
        id: 0,
        start_pc: 0,
        end_pc: 0,
        entry_stack_depth: 0,
        instructions: vec![instruction],
    };
    let key = super::tier1_block_key(&module, pou_id, &block);
    super::compile_tier1_block(&module, &block, key).map(|_| ())
}

#[test]
fn tier1_executor_rejects_null_reference_ref_field() {
    let (module, pou_id) = manual_vm_module(Vec::new(), Vec::new(), 0);
    let (program, source_block) = tier1_two_block_program(pou_id, 2);
    let mut runtime = Runtime::new();
    let mut registers = vec![Value::Reference(None), Value::Null];

    let err = execute_single_compiled_tier1_instruction(
        &module,
        &program,
        &source_block,
        Tier1CompiledInstr::RefField {
            base: RegisterId(0),
            field: SmolStr::new("FIELD"),
            dest: RegisterId(1),
        },
        &mut runtime,
        &mut registers,
    )
    .expect_err("null ref field should fail");

    assert!(matches!(err, RuntimeError::NullReference));
}

#[test]
fn tier1_executor_cmp_ref_const_jump_takes_matching_branch() {
    let (module, pou_id) = manual_vm_module(Vec::new(), vec![Value::DInt(5)], 1);
    let (program, source_block) = tier1_two_block_program(pou_id, 0);
    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::DInt(3));
    let mut registers = Vec::new();

    let outcome = execute_single_compiled_tier1_instruction(
        &module,
        &program,
        &source_block,
        Tier1CompiledInstr::CmpRefConstJumpIfDIntGuard {
            op: BinaryOp::Lt,
            ref_idx: 0,
            const_idx: 0,
            jump_if_true: true,
            target: BlockTarget::Block(1),
        },
        &mut runtime,
        &mut registers,
    )
    .expect("comparison branch should execute");

    assert_eq!(
        outcome,
        RegisterBlockExecutionOutcome::Continue(Some(BlockTarget::Block(1)))
    );
}

#[test]
fn tier1_executor_jump_if_takes_matching_branch() {
    let (module, pou_id) = manual_vm_module(Vec::new(), Vec::new(), 0);
    let (program, source_block) = tier1_two_block_program(pou_id, 1);
    let mut runtime = Runtime::new();
    let mut registers = vec![Value::Bool(true)];

    let outcome = execute_single_compiled_tier1_instruction(
        &module,
        &program,
        &source_block,
        Tier1CompiledInstr::JumpIf {
            cond: RegisterId(0),
            jump_if_true: true,
            target: BlockTarget::Block(1),
        },
        &mut runtime,
        &mut registers,
    )
    .expect("jump-if should execute");

    assert_eq!(
        outcome,
        RegisterBlockExecutionOutcome::Continue(Some(BlockTarget::Block(1)))
    );
}

fn tier1_two_block_program(pou_id: u32, max_registers: u32) -> (RegisterProgram, RegisterBlock) {
    let source_block = RegisterBlock {
        id: 0,
        start_pc: 0,
        end_pc: 1,
        entry_stack_depth: 0,
        instructions: Vec::new(),
    };
    let target_block = RegisterBlock {
        id: 1,
        start_pc: 1,
        end_pc: 1,
        entry_stack_depth: 0,
        instructions: Vec::new(),
    };
    let program = RegisterProgram {
        pou_id,
        entry_block: 0,
        max_registers,
        blocks: vec![source_block.clone(), target_block],
    };
    (program, source_block)
}

fn execute_single_compiled_tier1_instruction(
    module: &VmModule,
    program: &RegisterProgram,
    source_block: &RegisterBlock,
    instruction: Tier1CompiledInstr,
    runtime: &mut Runtime,
    registers: &mut [Value],
) -> Result<RegisterBlockExecutionOutcome, RuntimeError> {
    let key = super::tier1_block_key(module, program.pou_id, source_block);
    let compiled = Tier1CompiledBlock {
        key,
        instructions: vec![instruction],
    };
    let mut frames = super::FrameStack::default();
    let mut native_call_stack = super::OperandStack::default();
    let mut budget = 16;
    let Tier1BlockExecutionOutcome::Executed(outcome) = execute_tier1_compiled_block(
        runtime,
        module,
        program,
        source_block,
        &mut frames,
        registers,
        &mut native_call_stack,
        &compiled,
        &mut budget,
        0,
    )?;
    Ok(outcome)
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
