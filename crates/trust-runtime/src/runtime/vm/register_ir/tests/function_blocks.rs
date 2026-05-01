#[test]
fn register_ir_lowering_handles_function_block_self_fields_without_fallback() {
    let source = r#"
            FUNCTION_BLOCK Counter
            VAR_INPUT
                Enable : BOOL;
            END_VAR
            VAR_OUTPUT
                Value : DINT;
            END_VAR

            IF Enable THEN
                Value := Value + DINT#1;
            END_IF;
            END_FUNCTION_BLOCK

            PROGRAM Main
            VAR
                fb : Counter;
            END_VAR
            fb(Enable := TRUE);
            END_PROGRAM
        "#;

    let bytecode = bytecode_module_from_source(source).expect("compile bytecode");
    let vm_module = VmModule::from_bytecode(&bytecode).expect("decode vm module");
    let fb_pou_id = vm_module
        .function_block_ids
        .get(&SmolStr::new("COUNTER"))
        .copied()
        .expect("counter pou id");
    let lowered = lower_pou_to_register_ir(&vm_module, fb_pou_id).expect("lower register ir");
    verify_register_program(&lowered).expect("verify register ir");
    assert_no_fallback(&lowered);
}

#[test]
fn tier1_compiler_accepts_function_block_self_field_dynamic_ops() {
    let source = r#"
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
            END_PROGRAM
        "#;

    let bytecode = bytecode_module_from_source(source).expect("compile bytecode");
    let vm_module = VmModule::from_bytecode(&bytecode).expect("decode vm module");
    let fb_pou_id = vm_module
        .function_block_ids
        .get(&SmolStr::new("COUNTER"))
        .copied()
        .expect("counter pou id");
    let lowered = lower_pou_to_register_ir(&vm_module, fb_pou_id).expect("lower register ir");
    verify_register_program(&lowered).expect("verify register ir");
    let block = lowered
        .blocks
        .iter()
        .find(|block| {
            block.instructions.iter().any(|instruction| {
                matches!(instruction, RegisterInstr::LoadSelfFieldDynamic { .. })
            }) && block.instructions.iter().any(|instruction| {
                matches!(instruction, RegisterInstr::StoreSelfFieldDynamic { .. })
            })
        })
        .expect("function block fused self-field dynamic block");
    let key = super::tier1_block_key(&vm_module, fb_pou_id, block);
    assert!(
        super::compile_tier1_block(&vm_module, block, key).is_ok(),
        "expected tier-1 compiler to accept self-field dynamic block: {:?}",
        block.instructions
    );
}

#[test]
fn register_ir_lowering_fuses_self_field_dynamic_load_store() {
    let source = r#"
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
            END_PROGRAM
        "#;

    let bytecode = bytecode_module_from_source(source).expect("compile bytecode");
    let vm_module = VmModule::from_bytecode(&bytecode).expect("decode vm module");
    let fb_pou_id = vm_module
        .function_block_ids
        .get(&SmolStr::new("COUNTER"))
        .copied()
        .expect("counter pou id");
    let lowered = lower_pou_to_register_ir(&vm_module, fb_pou_id).expect("lower register ir");
    verify_register_program(&lowered).expect("verify register ir");
    assert_no_fallback(&lowered);

    let has_unfused_self_field_dynamic = lowered.blocks.iter().any(|block| {
        block.instructions.windows(3).any(|window| {
            let [RegisterInstr::LoadSelf { dest: self_reg }, RegisterInstr::RefField {
                base,
                dest: field_reg,
                ..
            }, third] = window
            else {
                return false;
            };
            base == self_reg
                && matches!(
                    third,
                    RegisterInstr::LoadDynamic { reference, .. }
                    | RegisterInstr::StoreDynamic {
                        reference,
                        ..
                    } if reference == field_reg
                )
        })
    });

    assert!(
        !has_unfused_self_field_dynamic,
        "SELF.field dynamic access should lower to a fused register instruction: {lowered:#?}"
    );
}

#[test]
fn tier1_compiler_accepts_function_block_index_dynamic_ops() {
    let source = r#"
            FUNCTION_BLOCK CounterArray
            VAR_OUTPUT
                Data : ARRAY[1..2] OF DINT;
            END_VAR

            Data[1] := Data[1] + DINT#1;
            END_FUNCTION_BLOCK

            PROGRAM Main
            VAR
                fb : CounterArray;
            END_VAR

            fb();
            END_PROGRAM
        "#;

    let bytecode = bytecode_module_from_source(source).expect("compile bytecode");
    let vm_module = VmModule::from_bytecode(&bytecode).expect("decode vm module");
    let fb_pou_id = vm_module
        .function_block_ids
        .get(&SmolStr::new("COUNTERARRAY"))
        .copied()
        .expect("counterarray pou id");
    let lowered = lower_pou_to_register_ir(&vm_module, fb_pou_id).expect("lower register ir");
    verify_register_program(&lowered).expect("verify register ir");
    let block = lowered
        .blocks
        .iter()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, RegisterInstr::RefIndex { .. }))
                && block
                    .instructions
                    .iter()
                    .any(|instruction| matches!(instruction, RegisterInstr::LoadDynamic { .. }))
                && block
                    .instructions
                    .iter()
                    .any(|instruction| matches!(instruction, RegisterInstr::StoreDynamic { .. }))
        })
        .expect("function block ref-index/dynamic block");
    let key = super::tier1_block_key(&vm_module, fb_pou_id, block);
    assert!(
        super::compile_tier1_block(&vm_module, block, key).is_ok(),
        "expected tier-1 compiler to accept index dynamic block: {:?}",
        block.instructions
    );
}

#[test]
fn register_executor_tier1_specialized_executor_executes_array_ref_blocks() {
    let source = r#"
            VAR_GLOBAL
                g_value : DINT;
            END_VAR

            FUNCTION_BLOCK CounterArray
            VAR_OUTPUT
                Data : ARRAY[1..2] OF DINT;
            END_VAR

            Data[1] := Data[1] + DINT#1;
            END_FUNCTION_BLOCK

            PROGRAM Main
            VAR
                fb : CounterArray;
            END_VAR

            fb();
            g_value := fb.Data[1];
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
    assert!(
        snapshot.compile_successes >= 1,
        "expected at least one compiled tier-1 block, snapshot={snapshot:?}"
    );
    assert!(
        snapshot.block_executions >= 1,
        "expected at least one executed compiled tier-1 block, snapshot={snapshot:?}"
    );
}

#[test]
fn register_executor_runs_program_with_complex_local_fields_without_fallback() {
    let pou_id = 1_u32;
    let code = vec![
        0x10, 0, 0, 0, 0, // LOAD_CONST 0
        0x21, 0, 0, 0, 0, // STORE_REF local path
        0x20, 0, 0, 0, 0, // LOAD_REF local path
        0x21, 1, 0, 0, 0,    // STORE_REF global
        0x06, // RETURN
    ];
    let mut pou_by_id = HashMap::new();
    pou_by_id.insert(
        pou_id,
        VmPouEntry {
            name: SmolStr::new("MAIN"),
            code_start: 0,
            code_end: code.len(),
            local_ref_start: 0,
            local_ref_count: 1,
            primary_instance_owner: None,
        },
    );
    let mut program_ids = HashMap::new();
    program_ids.insert(SmolStr::new("MAIN"), pou_id);
    let refs = vec![
        VmRef::Local {
            owner_frame_id: 0,
            offset: 0,
            path: [
                RefSegment::Field(SmolStr::new("INNER")),
                RefSegment::Field(SmolStr::new("VALUE")),
            ]
            .into_iter()
            .collect(),
        },
        VmRef::Global {
            offset: 0,
            path: RefPath::new(),
        },
    ];
    let module = VmModule {
        code,
        strings: Vec::new(),
        types: TypeTable::default(),
        refs,
        consts: vec![Value::DInt(7)],
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
    let initial_outer = Value::Struct(std::sync::Arc::new(StructValue::from_untyped_parts(
        SmolStr::new("OUTER_T"),
        IndexMap::from([(
            SmolStr::new("INNER"),
            Value::Struct(std::sync::Arc::new(StructValue::from_untyped_parts(
                SmolStr::new("INNER_T"),
                IndexMap::from([(SmolStr::new("VALUE"), Value::DInt(0))]),
            ))),
        )]),
    )));
    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::DInt(0));
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let outcome = try_execute_pou_with_register_ir_with_locals(
        &mut runtime,
        &module,
        pou_id,
        None,
        Some(&[initial_outer]),
        false,
        0,
        None,
    )
    .expect("execute register program");
    assert!(
        outcome.is_some(),
        "expected register execution, got stack fallback"
    );
    assert_eq!(runtime.storage().get_global("g0"), Some(&Value::DInt(7)));

    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(
        profile.register_program_fallbacks, 0,
        "expected no register fallback reasons, got {:?}",
        profile.fallback_reasons
    );
}

#[test]
fn register_lowering_error_fallback_reason_includes_pou_name_and_message() {
    let mut code = Vec::new();
    code.push(0x02);
    emit_i32(&mut code, 4096);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, Vec::new(), 0);

    let mut runtime = Runtime::new();
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("fallback decision");
    assert_eq!(outcome, RegisterExecutionOutcome::FallbackToStack);

    let profile = runtime.vm_register_profile_snapshot();
    assert!(
        profile.fallback_reasons.iter().any(|entry| {
            entry.reason.contains("lowering_error")
                && entry.reason.contains("MAIN")
                && entry.reason.contains("invalid jump target")
                && entry.count == 1
        }),
        "expected lowering_error fallback reason with pou name and message, got {:?}",
        profile.fallback_reasons
    );
}
