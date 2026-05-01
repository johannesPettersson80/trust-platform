#[test]
fn register_executor_runs_supported_program() {
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

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    assert_eq!(runtime.storage().get_global("g0"), Some(&Value::DInt(42)));
}

#[test]
fn register_executor_profile_records_hot_blocks_for_supported_program() {
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
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);

    let profile = runtime.vm_register_profile_snapshot();
    assert!(profile.enabled);
    assert_eq!(profile.register_programs_executed, 1);
    assert_eq!(profile.register_program_fallbacks, 0);
    assert!(
        profile
            .hot_blocks
            .iter()
            .any(|block| block.pou_id == pou_id && block.hits >= 1),
        "expected at least one hot block for executed POU",
    );
}

#[test]
fn register_executor_profile_records_dynamic_ref_and_instance_lookup_counters() {
    let mut code = Vec::new();
    code.push(0x22);
    emit_u32(&mut code, 0);
    code.push(0x30);
    emit_u32(&mut code, 0);
    code.push(0x32);
    code.push(0x21);
    emit_u32(&mut code, 1);
    code.push(0x22);
    emit_u32(&mut code, 0);
    code.push(0x30);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x33);
    code.push(0x23);
    code.push(0x30);
    emit_u32(&mut code, 1);
    code.push(0x32);
    code.push(0x21);
    emit_u32(&mut code, 2);
    code.push(0x23);
    code.push(0x30);
    emit_u32(&mut code, 1);
    code.push(0x10);
    emit_u32(&mut code, 1);
    code.push(0x33);
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
    let refs = vec![
        VmRef::Global {
            offset: 0,
            path: RefPath::new(),
        },
        VmRef::Global {
            offset: 1,
            path: RefPath::new(),
        },
        VmRef::Global {
            offset: 2,
            path: RefPath::new(),
        },
    ];
    let module = VmModule {
        code,
        strings: vec![SmolStr::new("VALUE"), SmolStr::new("ACC")],
        types: TypeTable::default(),
        refs,
        consts: vec![Value::DInt(11), Value::DInt(13)],
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
    runtime.storage_mut().set_global(
        "g0",
        Value::Struct(std::sync::Arc::new(StructValue::from_untyped_parts(
            SmolStr::new("CELL_T"),
            IndexMap::from([(SmolStr::new("VALUE"), Value::DInt(7))]),
        ))),
    );
    runtime.storage_mut().set_global("g1", Value::DInt(0));
    runtime.storage_mut().set_global("g2", Value::DInt(0));
    let instance_id = runtime.storage_mut().create_instance("COUNTER");
    assert!(runtime
        .storage_mut()
        .set_instance_var(instance_id, "ACC", Value::DInt(9)));
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let outcome =
        try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, Some(instance_id))
            .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    assert_eq!(runtime.storage().get_global("g1"), Some(&Value::DInt(7)));
    assert_eq!(runtime.storage().get_global("g2"), Some(&Value::DInt(9)));
    assert_eq!(
        runtime.storage().get_global("g0"),
        Some(&Value::Struct(std::sync::Arc::new(
            StructValue::from_untyped_parts(
                SmolStr::new("CELL_T"),
                IndexMap::from([(SmolStr::new("VALUE"), Value::DInt(11))]),
            )
        )))
    );
    assert_eq!(
        runtime.storage().get_instance_var(instance_id, "ACC"),
        Some(&Value::DInt(13))
    );

    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(profile.register_program_fallbacks, 0);
    assert_eq!(profile.ref_ops.load_ref, 0);
    assert_eq!(profile.ref_ops.store_ref, 2);
    assert_eq!(profile.ref_ops.load_ref_addr, 2);
    assert_eq!(profile.ref_ops.ref_field, 4);
    assert_eq!(profile.ref_ops.ref_index, 0);
    assert_eq!(profile.ref_ops.load_dynamic, 2);
    assert_eq!(profile.ref_ops.store_dynamic, 2);
    assert_eq!(profile.ref_ops.instance_field_lookups, 2);
    assert_eq!(profile.value_ops.read_value_clones, 0);
}

#[test]
fn register_executor_profile_records_function_block_call_counters() {
    let source = r#"
            FUNCTION_BLOCK Counter
            VAR_INPUT
                inc : BOOL;
            END_VAR
            VAR_OUTPUT
                value : INT;
            END_VAR

            IF inc THEN
                value := value + INT#1;
            END_IF;
            END_FUNCTION_BLOCK

            PROGRAM Main
            VAR
                fb : Counter;
                out_count : INT := INT#0;
            END_VAR
            fb(inc := TRUE, value => out_count);
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
    harness.runtime_mut().set_vm_register_profile_enabled(true);
    harness.runtime_mut().reset_vm_register_profile();

    let result = harness.cycle();
    assert!(
        result.errors.is_empty(),
        "cycle errors: {:?}",
        result.errors
    );
    assert_eq!(harness.get_output("out_count"), Some(Value::Int(1)));

    let profile = harness.runtime().vm_register_profile_snapshot();
    assert_eq!(profile.register_program_fallbacks, 0);
    assert_eq!(profile.call_ops.function_block_call_entries, 1);
    assert_eq!(profile.call_ops.parameter_bindings, 2);
    assert_eq!(profile.call_ops.output_copy_backs, 1);
    assert!(profile.call_ops.frame_pushes >= 2);
    assert!(profile.call_ops.frame_pops >= 2);
    assert_eq!(profile.value_ops.binding_expr_clones, 0);
    assert_eq!(profile.value_ops.output_value_clones, 0);
}

#[test]
fn register_executor_profile_avoids_clone_counters_for_struct_inout_function_block() {
    let source = r#"
            TYPE AXIS_REF :
            STRUCT
                AxisId : UDINT;
                InternalIndex : UINT;
            END_STRUCT
            END_TYPE

            FUNCTION_BLOCK TouchAxis
            VAR_IN_OUT
                Axis : AXIS_REF;
            END_VAR
            VAR_OUTPUT
                Done : BOOL;
            END_VAR

            Axis.InternalIndex := Axis.InternalIndex + UINT#1;
            Done := TRUE;
            END_FUNCTION_BLOCK

            PROGRAM Main
            VAR
                Axis : AXIS_REF;
                Fb : TouchAxis;
            END_VAR

            Axis.AxisId := UDINT#1;
            Axis.InternalIndex := UINT#1;
            Fb(Axis := Axis);
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
    harness.runtime_mut().set_vm_register_profile_enabled(true);
    harness.runtime_mut().reset_vm_register_profile();

    let result = harness.cycle();
    assert!(
        result.errors.is_empty(),
        "cycle errors: {:?}",
        result.errors
    );

    let profile = harness.runtime().vm_register_profile_snapshot();
    assert_eq!(profile.register_program_fallbacks, 0);
    assert!(profile.call_ops.function_block_call_entries >= 1);
    assert!(profile.call_ops.parameter_bindings >= 1);
    assert!(profile.call_ops.output_copy_backs >= 1);
    assert_eq!(
        profile.value_ops.read_value_clones, 0,
        "profile: {:?}",
        profile.value_ops
    );
    assert_eq!(
        profile.value_ops.output_value_clones, 0,
        "profile: {:?}",
        profile.value_ops
    );
}

#[test]
fn read_register_with_counts_records_clone_then_move_reads() {
    let mut profile = RegisterProfileState::default();
    profile.set_enabled(true);
    let mut registers = vec![Value::DInt(7)];
    let mut remaining = vec![2_u32];

    let first = read_register_with_counts(
        &mut profile,
        registers.as_mut_slice(),
        remaining.as_mut_slice(),
        RegisterId(0),
    )
    .expect("first read");
    let second = read_register_with_counts(
        &mut profile,
        registers.as_mut_slice(),
        remaining.as_mut_slice(),
        RegisterId(0),
    )
    .expect("second read");

    assert_eq!(first, Value::DInt(7));
    assert_eq!(second, Value::DInt(7));
    assert_eq!(registers[0], Value::Null);
    let snapshot = profile.snapshot();
    assert_eq!(snapshot.value_ops.register_read_clones, 1);
    assert_eq!(snapshot.value_ops.register_read_moves, 1);
}

#[test]
fn register_executor_falls_back_when_lowering_contains_unsupported_opcode() {
    let mut code = Vec::new();
    code.push(0x07);
    emit_u32(&mut code, 0);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, Vec::new(), 0);
    let mut runtime = Runtime::new();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("fallback decision");
    assert_eq!(outcome, RegisterExecutionOutcome::FallbackToStack);
}

#[test]
fn register_executor_profile_records_ref_op_counters_for_load_ref_store_ref_program() {
    let mut code = Vec::new();
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x21);
    emit_u32(&mut code, 1);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, Vec::new(), 2);

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::DInt(41));
    runtime.storage_mut().set_global("g1", Value::DInt(0));
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    assert_eq!(runtime.storage().get_global("g1"), Some(&Value::DInt(41)));

    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(profile.register_program_fallbacks, 0);
    assert_eq!(profile.ref_ops.load_ref, 1);
    assert_eq!(profile.ref_ops.store_ref, 1);
    assert_eq!(profile.ref_ops.load_ref_addr, 0);
    assert_eq!(profile.ref_ops.ref_field, 0);
    assert_eq!(profile.ref_ops.ref_index, 0);
    assert_eq!(profile.ref_ops.load_dynamic, 0);
    assert_eq!(profile.ref_ops.store_dynamic, 0);
    assert_eq!(profile.ref_ops.instance_field_lookups, 0);
    assert_eq!(profile.value_ops.read_value_clones, 0);
    assert_eq!(profile.value_ops.register_read_moves, 1);
    assert_eq!(profile.value_ops.register_read_clones, 0);
}

#[test]
fn register_executor_profile_avoids_clone_counter_for_scalar_load_const() {
    let mut code = Vec::new();
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x21);
    emit_u32(&mut code, 0);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(41)], 1);

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::DInt(0));
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    assert_eq!(runtime.storage().get_global("g0"), Some(&Value::DInt(41)));

    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(profile.register_program_fallbacks, 0);
    assert_eq!(profile.value_ops.const_load_clones, 0);
}

#[test]
fn register_executor_profile_avoids_clone_counters_for_borrowed_ref_const_binary_guard() {
    let mut code = Vec::new();
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x40);
    code.push(0x21);
    emit_u32(&mut code, 1);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(1)], 2);

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::DInt(41));
    runtime.storage_mut().set_global("g1", Value::DInt(0));
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    assert_eq!(runtime.storage().get_global("g1"), Some(&Value::DInt(42)));

    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(profile.register_program_fallbacks, 0);
    assert_eq!(profile.value_ops.read_value_clones, 0);
    assert_eq!(profile.value_ops.const_load_clones, 0);
}

#[test]
fn register_executor_profile_avoids_clone_counters_for_borrowed_ref_ref_non_dint_binary() {
    let mut code = Vec::new();
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x20);
    emit_u32(&mut code, 1);
    code.push(0x47);
    code.push(0x21);
    emit_u32(&mut code, 2);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, Vec::new(), 3);

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::Bool(false));
    runtime.storage_mut().set_global("g1", Value::Bool(true));
    runtime.storage_mut().set_global("g2", Value::Bool(false));
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    assert_eq!(runtime.storage().get_global("g2"), Some(&Value::Bool(true)));

    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(profile.register_program_fallbacks, 0);
    assert_eq!(profile.value_ops.read_value_clones, 0);
}

#[test]
fn register_executor_profile_avoids_clone_counters_for_borrowed_ref_const_non_dint_binary() {
    let mut code = Vec::new();
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x47);
    code.push(0x21);
    emit_u32(&mut code, 1);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, vec![Value::Bool(true)], 2);

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::Bool(false));
    runtime.storage_mut().set_global("g1", Value::Bool(false));
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    assert_eq!(runtime.storage().get_global("g1"), Some(&Value::Bool(true)));

    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(profile.register_program_fallbacks, 0);
    assert_eq!(profile.value_ops.read_value_clones, 0);
    assert_eq!(profile.value_ops.const_load_clones, 0);
}

#[test]
fn register_executor_profile_records_fallback_reason() {
    let mut code = Vec::new();
    code.push(0x07);
    emit_u32(&mut code, 0);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, Vec::new(), 0);
    let mut runtime = Runtime::new();
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("fallback decision");
    assert_eq!(outcome, RegisterExecutionOutcome::FallbackToStack);

    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(profile.register_programs_executed, 0);
    assert_eq!(profile.register_program_fallbacks, 1);
    assert!(
        profile
            .fallback_reasons
            .iter()
            .any(|entry| entry.reason.starts_with("unsupported_opcode") && entry.count == 1),
        "expected unsupported opcode fallback reason in profile snapshot",
    );
}
