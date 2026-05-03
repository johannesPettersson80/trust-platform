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

