#[test]
fn register_ir_lowering_handles_linear_arithmetic_main() {
    let source = r#"
            PROGRAM Main
            VAR
                count : DINT := 0;
            END_VAR
            count := count + 1;
            END_PROGRAM
        "#;
    let (vm_module, pou_id) = vm_module_and_main_pou(source);
    let lowered = lower_pou_to_register_ir(&vm_module, pou_id).expect("lower register ir");
    verify_register_program(&lowered).expect("verify register ir");

    assert_eq!(lowered.entry_block, 0);
    assert!(lowered.max_registers > 0);
    assert!(!lowered.blocks.is_empty());
    let all_instr = lowered
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect::<Vec<_>>();
    assert!(
        all_instr.iter().any(|instr| {
            matches!(
                instr,
                RegisterInstr::Binary { .. }
                    | RegisterInstr::BinaryRefToRef { .. }
                    | RegisterInstr::BinaryRefConstToRef { .. }
                    | RegisterInstr::BinaryConstRefToRef { .. }
            )
        }),
        "expected arithmetic lowering to emit binary register instruction",
    );
    assert!(
        all_instr.iter().any(|instr| {
            matches!(
                instr,
                RegisterInstr::StoreRef { .. }
                    | RegisterInstr::BinaryRefToRef { .. }
                    | RegisterInstr::BinaryRefConstToRef { .. }
                    | RegisterInstr::BinaryConstRefToRef { .. }
            )
        }),
        "expected store lowering to emit register store instruction",
    );
}

#[test]
fn register_ir_stack_normalization_preserves_protected_registers_and_cycles() {
    let mut next_register = 2;
    let mut instructions = Vec::new();
    let protected = normalize_stack_for_block_exit(
        &mut next_register,
        &mut instructions,
        &[RegisterId(1)],
        Some(RegisterId(0)),
    )
    .expect("normalize clobbering stack")
    .expect("protected register must move to temp");
    assert_ne!(protected, RegisterId(0));
    assert!(matches!(
        instructions.first(),
        Some(RegisterInstr::Move {
            src: RegisterId(0),
            dest,
        }) if *dest == protected
    ));

    let mut no_clobber_next = 3;
    let mut no_clobber = Vec::new();
    let preserved = normalize_stack_for_block_exit(
        &mut no_clobber_next,
        &mut no_clobber,
        &[RegisterId(0), RegisterId(1)],
        Some(RegisterId(2)),
    )
    .expect("normalize non-clobbering stack");
    assert_eq!(preserved, Some(RegisterId(2)));
    assert!(
        no_clobber
            .iter()
            .all(|instruction| !matches!(instruction, RegisterInstr::Move { src: RegisterId(2), .. })),
        "non-clobbered protected register should not be moved: {no_clobber:?}",
    );

    let mut cycle_next = 2;
    let mut cycle_moves = Vec::new();
    normalize_stack_for_block_exit(
        &mut cycle_next,
        &mut cycle_moves,
        &[RegisterId(1), RegisterId(0)],
        None,
    )
    .expect("normalize register cycle");
    let mut symbolic = ["slot1", "slot0", "scratch"];
    for instruction in &cycle_moves {
        let RegisterInstr::Move { src, dest } = instruction else {
            panic!("expected only move instructions, got {instruction:?}");
        };
        symbolic[dest.index() as usize] = symbolic[src.index() as usize];
    }
    assert_eq!(symbolic[0], "slot0");
    assert_eq!(symbolic[1], "slot1");

    let mut two_cycle_next = 4;
    let mut two_cycle_moves = Vec::new();
    normalize_stack_for_block_exit(
        &mut two_cycle_next,
        &mut two_cycle_moves,
        &[RegisterId(1), RegisterId(0), RegisterId(3), RegisterId(2)],
        None,
    )
    .expect("normalize two independent register cycles");
    let mut two_cycle_symbolic = ["slot1", "slot0", "slot3", "slot2", "scratch"];
    for instruction in &two_cycle_moves {
        let RegisterInstr::Move { src, dest } = instruction else {
            panic!("expected only move instructions, got {instruction:?}");
        };
        two_cycle_symbolic[dest.index() as usize] = two_cycle_symbolic[src.index() as usize];
    }
    assert_eq!(two_cycle_symbolic[0], "slot0");
    assert_eq!(two_cycle_symbolic[1], "slot1");
    assert_eq!(two_cycle_symbolic[2], "slot2");
    assert_eq!(two_cycle_symbolic[3], "slot3");
}

#[test]
fn register_ir_lowering_covers_nop_null_and_full_binary_opcode_family() {
    let mut code = Vec::new();
    code.push(0x00);
    code.push(0x25);
    code.push(0x12);
    for opcode in [0x42, 0x43, 0x44, 0x48] {
        code.push(0x10);
        emit_u32(&mut code, 0);
        code.push(0x10);
        emit_u32(&mut code, 1);
        code.push(opcode);
        code.push(0x12);
    }
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(12), Value::DInt(3)], 0);
    let lowered = lower_pou_to_register_ir(&module, pou_id).expect("lower register ir");
    verify_register_program(&lowered).expect("verify register ir");

    let all_instr = lowered
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect::<Vec<_>>();
    assert!(all_instr.iter().any(|instr| matches!(instr, RegisterInstr::Nop)));
    assert!(all_instr
        .iter()
        .any(|instr| matches!(instr, RegisterInstr::LoadNull { .. })));
    for op in [BinaryOp::Mul, BinaryOp::Div, BinaryOp::Mod, BinaryOp::Xor] {
        assert!(
            all_instr.iter().any(|instr| matches!(instr, RegisterInstr::Binary { op: actual, .. } if *actual == op)),
            "expected lowered binary op {op:?}, got {all_instr:?}",
        );
    }
}

#[test]
fn register_ir_lowering_accepts_valid_call_native_and_swap_stack_depths() {
    let mut call_code = Vec::new();
    call_code.push(0x10);
    emit_u32(&mut call_code, 0);
    call_code.push(0x10);
    emit_u32(&mut call_code, 1);
    call_code.push(0x09);
    emit_u32(&mut call_code, 0);
    emit_u32(&mut call_code, 0);
    emit_u32(&mut call_code, 1);
    call_code.push(0x12);
    call_code.push(0x12);
    call_code.push(0x06);
    let (call_module, call_pou_id) =
        manual_vm_module(call_code, vec![Value::DInt(1), Value::DInt(2)], 0);
    let lowered_call =
        lower_pou_to_register_ir(&call_module, call_pou_id).expect("lower CALL_NATIVE");
    assert!(lowered_call
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .any(|instr| matches!(instr, RegisterInstr::CallNative { args, .. } if args.len() == 1)));

    let mut swap_code = Vec::new();
    for const_idx in 0..3 {
        swap_code.push(0x10);
        emit_u32(&mut swap_code, const_idx);
    }
    swap_code.push(0x13);
    swap_code.push(0x12);
    swap_code.push(0x12);
    swap_code.push(0x12);
    swap_code.push(0x06);
    let (swap_module, swap_pou_id) = manual_vm_module(
        swap_code,
        vec![Value::DInt(1), Value::DInt(2), Value::DInt(3)],
        0,
    );
    lower_pou_to_register_ir(&swap_module, swap_pou_id).expect("lower SWAP with depth 3");
}

#[test]
fn register_ir_lowering_does_not_normalize_after_return() {
    let mut code = Vec::new();
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 1);
    code.push(0x13);
    code.push(0x12);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(1), Value::DInt(2)], 0);
    let lowered = lower_pou_to_register_ir(&module, pou_id).expect("lower register ir");
    let block = lowered.blocks.first().expect("lowered block");
    let return_index = block
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, RegisterInstr::Return))
        .expect("return instruction");
    assert!(
        block.instructions[return_index + 1..].is_empty(),
        "return must terminate lowering without trailing normalization moves: {:?}",
        block.instructions
    );
}

#[test]
fn register_ir_lowering_emits_control_flow_blocks_for_loops() {
    let source = r#"
            PROGRAM Main
            VAR
                i : DINT := 0;
                acc : DINT := 0;
            END_VAR
            WHILE i < 3 DO
                acc := acc + i;
                i := i + 1;
            END_WHILE;
            END_PROGRAM
        "#;
    let (vm_module, pou_id) = vm_module_and_main_pou(source);
    let lowered = lower_pou_to_register_ir(&vm_module, pou_id).expect("lower register ir");
    verify_register_program(&lowered).expect("verify register ir");

    assert!(
        lowered.blocks.len() >= 2,
        "expected loop lowering to produce multiple blocks"
    );
    assert!(
        lowered
            .blocks
            .iter()
            .flat_map(|block| block.instructions.iter())
            .any(|instr| matches!(
                instr,
                RegisterInstr::Jump {
                    target: BlockTarget::Block(_)
                } | RegisterInstr::JumpIf {
                    target: BlockTarget::Block(_),
                    ..
                }
            )),
        "expected branch instructions targeting lowered blocks"
    );
}

#[test]
fn register_ir_lowering_handles_case_selector_live_across_branch_blocks() {
    let source = r#"
            PROGRAM Main
            VAR
                selector : UINT := UINT#2;
                output : UINT := UINT#0;
            END_VAR

            CASE selector OF
                UINT#1:
                    output := UINT#10;
                UINT#2:
                    output := UINT#20;
                ELSE
                    output := UINT#30;
            END_CASE;
            END_PROGRAM
        "#;

    let (vm_module, pou_id) = vm_module_and_main_pou(source);
    let lowered = lower_pou_to_register_ir(&vm_module, pou_id).expect("lower register ir");
    verify_register_program(&lowered).expect("verify register ir");
    assert_no_fallback(&lowered);
}

#[test]
fn register_ir_lowering_handles_string_case_selector() {
    let source = r#"
            PROGRAM Main
            VAR
                selector : STRING := 'B';
                output : UINT := UINT#0;
            END_VAR

            CASE selector OF
                'A':
                    output := UINT#10;
                'B':
                    output := UINT#20;
                ELSE
                    output := UINT#30;
            END_CASE;
            END_PROGRAM
        "#;

    let (vm_module, pou_id) = vm_module_and_main_pou(source);
    let lowered = lower_pou_to_register_ir(&vm_module, pou_id).expect("lower register ir");
    verify_register_program(&lowered).expect("verify register ir");
    assert_no_fallback(&lowered);
}

#[test]
fn register_executor_runs_case_program_without_fallback() {
    let source = r#"
            VAR_GLOBAL
                g_selector : UINT := UINT#2;
                g_output : UINT := UINT#0;
            END_VAR

            PROGRAM Main
            CASE g_selector OF
                UINT#1:
                    g_output := UINT#10;
                UINT#2:
                    g_output := UINT#20;
                ELSE
                    g_output := UINT#30;
            END_CASE;
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
    assert_eq!(harness.get_output("g_output"), Some(Value::UInt(20)));

    let profile = harness.runtime().vm_register_profile_snapshot();
    assert!(profile.register_programs_executed >= 1);
    assert_eq!(
        profile.register_program_fallbacks, 0,
        "expected no register fallbacks, got {:?}",
        profile.fallback_reasons
    );
}

#[test]
fn register_executor_runs_string_case_program_without_fallback() {
    let source = r#"
            VAR_GLOBAL
                g_selector : STRING := 'B';
                g_output : UINT := UINT#0;
            END_VAR

            PROGRAM Main
            CASE g_selector OF
                'A':
                    g_output := UINT#10;
                'B':
                    g_output := UINT#20;
                ELSE
                    g_output := UINT#30;
            END_CASE;
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
    assert_eq!(harness.get_output("g_output"), Some(Value::UInt(20)));

    let profile = harness.runtime().vm_register_profile_snapshot();
    assert!(profile.register_programs_executed >= 1);
    assert_eq!(
        profile.register_program_fallbacks, 0,
        "expected no register fallbacks, got {:?}",
        profile.fallback_reasons
    );
}

#[test]
fn register_executor_fb_omitted_input_uses_initializer_then_reuses_stored_value() {
    let source = r#"
            FUNCTION_BLOCK Adjust
            VAR_INPUT
                base : INT;
                inc : INT := INT#5;
            END_VAR
            VAR_OUTPUT
                result : INT;
            END_VAR
            result := base + inc;
            END_FUNCTION_BLOCK

            PROGRAM Main
            VAR
                fb : Adjust;
                first : INT := INT#0;
                second : INT := INT#0;
                third : INT := INT#0;
            END_VAR

            fb(base := INT#3);
            first := fb.result;

            fb(base := INT#3, inc := INT#9);
            second := fb.result;

            fb(base := INT#3);
            third := fb.result;
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
    assert_eq!(harness.get_output("first"), Some(Value::Int(8)));
    assert_eq!(harness.get_output("second"), Some(Value::Int(12)));
    assert_eq!(harness.get_output("third"), Some(Value::Int(12)));

    let profile = harness.runtime().vm_register_profile_snapshot();
    assert!(profile.register_programs_executed >= 1);
    assert_eq!(
        profile.register_program_fallbacks, 0,
        "expected no register fallbacks, got {:?}",
        profile.fallback_reasons
    );
}

#[test]
fn register_executor_runs_multi_label_case_program_without_fallback() {
    let source = r#"
            VAR_GLOBAL
                g_selector : UINT := UINT#3;
                g_output : UINT := UINT#0;
            END_VAR

            PROGRAM Main
            CASE g_selector OF
                UINT#1:
                    g_output := UINT#10;
                UINT#2:
                    g_output := UINT#20;
                UINT#3:
                    g_output := UINT#30;
                ELSE
                    g_output := UINT#99;
            END_CASE;
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
    assert_eq!(harness.get_output("g_output"), Some(Value::UInt(30)));

    let profile = harness.runtime().vm_register_profile_snapshot();
    assert!(profile.register_programs_executed >= 1);
    assert_eq!(
        profile.register_program_fallbacks, 0,
        "expected no register fallbacks, got {:?}",
        profile.fallback_reasons
    );
}

#[test]
fn register_executor_runs_case_branch_with_nested_if_without_fallback() {
    let source = r#"
            VAR_GLOBAL
                g_current_step : UINT := UINT#30;
                g_last_error : UINT := UINT#0;
                g_power_status : BOOL := TRUE;
            END_VAR

            PROGRAM Main
            CASE g_current_step OF
                UINT#10:
                    IF FALSE THEN
                        g_current_step := UINT#20;
                    END_IF;
                UINT#20:
                    IF FALSE THEN
                        g_current_step := UINT#30;
                    END_IF;
                UINT#30:
                    IF g_power_status THEN
                        g_current_step := UINT#40;
                    END_IF;
                ELSE
                    g_last_error := UINT#512;
                    g_current_step := UINT#900;
            END_CASE;

            IF g_last_error <> UINT#0 THEN
                g_current_step := UINT#900;
            END_IF;
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
    assert_eq!(harness.get_output("g_current_step"), Some(Value::UInt(40)));
    assert_eq!(harness.get_output("g_last_error"), Some(Value::UInt(0)));

    let profile = harness.runtime().vm_register_profile_snapshot();
    assert!(profile.register_programs_executed >= 1);
    assert_eq!(
        profile.register_program_fallbacks, 0,
        "expected no register fallbacks, got {:?}",
        profile.fallback_reasons
    );
}

#[test]
fn register_executor_progresses_motion_demo_to_step_40_without_error_by_cycle_three() {
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/plcopen_motion_single_axis_demo");
    let runtime_config =
        RuntimeConfig::load(project.join("runtime.toml")).expect("load runtime config");
    let cycle_budget = runtime_config.cycle_interval;
    let compile_sources =
        collect_project_source_files(&project, None).expect("collect project sources");
    let session = CompileSession::from_sources(compile_sources);
    let mut runtime = session.build_runtime().expect("build runtime");
    runtime
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("set backend");
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    for cycle in 0..3 {
        runtime.execute_cycle().unwrap_or_else(|err| {
            panic!("cycle {} failed: {err}", cycle + 1);
        });
        runtime.advance_time(cycle_budget);
    }

    assert_eq!(
        runtime.storage().get_global("g_motion_demo_current_step"),
        Some(&Value::UInt(40))
    );
    assert_eq!(
        runtime.storage().get_global("g_motion_demo_last_error"),
        Some(&Value::Word(0))
    );

    let profile = runtime.vm_register_profile_snapshot();
    assert!(profile.register_programs_executed >= 1);
    assert_eq!(
        profile.register_program_fallbacks, 0,
        "expected no register fallbacks, got {:?}",
        profile.fallback_reasons
    );
}

#[test]
fn register_ir_verifier_rejects_unknown_block_target() {
    let source = r#"
            PROGRAM Main
            END_PROGRAM
        "#;
    let (vm_module, pou_id) = vm_module_and_main_pou(source);
    let mut lowered = lower_pou_to_register_ir(&vm_module, pou_id).expect("lower register ir");
    lowered.blocks[0].instructions.push(RegisterInstr::Jump {
        target: BlockTarget::Block(9999),
    });
    let err = verify_register_program(&lowered).expect_err("verification should fail");
    let RuntimeError::InvalidBytecode(message) = err else {
        panic!("expected InvalidBytecode verification error");
    };
    assert!(
        message.contains("unknown block target"),
        "unexpected verification message: {message}",
    );
}

#[test]
fn register_ir_lowering_rejects_invalid_jump_target() {
    let source = r#"
            PROGRAM Main
            END_PROGRAM
        "#;
    let mut bytecode = bytecode_module_from_source(source).expect("compile bytecode");
    let main_id = {
        let strings = match bytecode.section(SectionId::StringTable) {
            Some(SectionData::StringTable(strings)) => strings,
            _ => panic!("missing string table"),
        };
        let index = match bytecode.section(SectionId::PouIndex) {
            Some(SectionData::PouIndex(index)) => index,
            _ => panic!("missing pou index"),
        };
        index
            .entries
            .iter()
            .find(|entry| strings.entries[entry.name_idx as usize].eq_ignore_ascii_case("MAIN"))
            .map(|entry| entry.id)
            .expect("main entry id")
    };

    let mut body = Vec::new();
    body.push(0x02);
    body.extend_from_slice(&(4096_i32).to_le_bytes());
    body.push(0x06);

    let new_offset =
        if let Some(SectionData::PouBodies(code)) = bytecode.section_mut(SectionId::PouBodies) {
            let offset = code.len() as u32;
            code.extend_from_slice(&body);
            offset
        } else {
            panic!("missing POU_BODIES");
        };
    if let Some(SectionData::PouIndex(index)) = bytecode.section_mut(SectionId::PouIndex) {
        for entry in &mut index.entries {
            if entry.id == main_id {
                entry.code_offset = new_offset;
                entry.code_length = body.len() as u32;
            }
        }
    } else {
        panic!("missing POU_INDEX");
    }
    bytecode.sections.retain(|section| {
        section.id != SectionId::DebugMap.as_raw()
            && section.id != SectionId::DebugStringTable.as_raw()
    });

    let vm_module = VmModule::from_bytecode(&bytecode).expect("decode vm module");
    let pou_id = vm_module
        .program_ids
        .get(&SmolStr::new("MAIN"))
        .copied()
        .expect("main pou id");
    let err = lower_pou_to_register_ir(&vm_module, pou_id).expect_err("invalid jump must fail");
    let RuntimeError::InvalidBytecode(message) = err else {
        panic!("expected InvalidBytecode lowering error");
    };
    assert!(
        message.contains("invalid jump target"),
        "unexpected lowering message: {message}",
    );
}

#[test]
fn register_ir_parity_matches_stack_subset_linear_program() {
    let mut code = Vec::new();
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x40);
    code.push(0x21);
    emit_u32(&mut code, 0);
    code.push(0x06);
    let consts = vec![Value::DInt(1)];
    let (module, pou_id) = manual_vm_module(code, consts, 1);
    let lowered = lower_pou_to_register_ir(&module, pou_id).expect("lower register ir");
    verify_register_program(&lowered).expect("verify register ir");
    assert_no_fallback(&lowered);

    let mut stack_refs = vec![Value::DInt(41)];
    execute_stack_subset(&module, pou_id, &mut stack_refs).expect("execute stack subset");
    let mut register_refs = vec![Value::DInt(41)];
    execute_register_subset(&module, &lowered, &mut register_refs)
        .expect("execute register subset");

    assert_eq!(register_refs, stack_refs);
    assert_eq!(register_refs, vec![Value::DInt(42)]);
}

#[test]
fn register_ir_parity_matches_stack_subset_loop_program() {
    let mut code = Vec::new();
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x21);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x21);
    emit_u32(&mut code, 1);

    let loop_check_pc = code.len();
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 2);
    code.push(0x52);

    let jump_false_pc = code.len();
    code.push(0x04);
    emit_i32(&mut code, 0);

    code.push(0x20);
    emit_u32(&mut code, 1);
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x40);
    code.push(0x21);
    emit_u32(&mut code, 1);
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 1);
    code.push(0x40);
    code.push(0x21);
    emit_u32(&mut code, 0);

    let jump_back_pc = code.len();
    code.push(0x02);
    emit_i32(&mut code, 0);

    let loop_end_pc = code.len();
    code.push(0x06);

    let jump_false_offset = loop_end_pc as i32 - (jump_false_pc + 5) as i32;
    patch_i32(&mut code, jump_false_pc + 1, jump_false_offset);
    let jump_back_offset = loop_check_pc as i32 - (jump_back_pc + 5) as i32;
    patch_i32(&mut code, jump_back_pc + 1, jump_back_offset);

    let consts = vec![Value::DInt(0), Value::DInt(1), Value::DInt(3)];
    let (module, pou_id) = manual_vm_module(code, consts, 2);
    let lowered = lower_pou_to_register_ir(&module, pou_id).expect("lower register ir");
    verify_register_program(&lowered).expect("verify register ir");
    assert_no_fallback(&lowered);

    let mut stack_refs = vec![Value::DInt(7), Value::DInt(7)];
    execute_stack_subset(&module, pou_id, &mut stack_refs).expect("execute stack subset");
    let mut register_refs = vec![Value::DInt(7), Value::DInt(7)];
    execute_register_subset(&module, &lowered, &mut register_refs)
        .expect("execute register subset");

    assert_eq!(register_refs, stack_refs);
    assert_eq!(register_refs, vec![Value::DInt(3), Value::DInt(3)]);
}

#[test]
fn dint_mod_zero_fast_path_matches_generic_error_contract() {
    let fast_path =
        super::apply_dint_binary_guard_borrowed(BinaryOp::Mod, &Value::DInt(10), &Value::DInt(0));
    let generic_path = apply_binary(
        BinaryOp::Mod,
        Value::LInt(10),
        Value::SInt(0),
        &DateTimeProfile::default(),
    )
    .map(Some);

    assert_eq!(fast_path, Err(RuntimeError::ModuloByZero));
    assert_eq!(fast_path, generic_path);
}
