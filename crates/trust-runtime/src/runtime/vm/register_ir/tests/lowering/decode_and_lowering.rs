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
fn register_ir_decode_rejects_rot_underflow_and_accepts_exact_depth() {
    for (opcode, available, label) in [(0x14, 2_usize, "ROT3"), (0x15, 3_usize, "ROT4")] {
        let mut code = Vec::new();
        for const_idx in 0..available {
            code.push(0x10);
            emit_u32(&mut code, const_idx as u32);
        }
        code.push(opcode);
        code.push(0x06);
        let consts = (0..available)
            .map(|value| Value::DInt(value as i32))
            .collect::<Vec<_>>();
        let (module, pou_id) = manual_vm_module(code, consts, 0);
        let pou = module.pou(pou_id).expect("manual pou");
        let decoded = decode_pou(&module, pou.code_start, pou.code_end).expect("decode pou");
        let leaders =
            collect_block_leaders(&decoded, pou.code_start, pou.code_end).expect("leaders");
        let err = compute_block_entry_stack_depths(
            &decoded,
            &leaders,
            pou.code_start,
            pou.code_end,
        )
        .expect_err("ROT underflow must fail stack-depth analysis");
        let RuntimeError::InvalidBytecode(message) = err else {
            panic!("expected invalid bytecode for {label} underflow");
        };
        assert!(
            message.contains(label),
            "expected {label} underflow message, got {message}",
        );
    }

    let mut code = Vec::new();
    for const_idx in 0..3 {
        code.push(0x10);
        emit_u32(&mut code, const_idx);
    }
    code.push(0x14);
    code.push(0x10);
    emit_u32(&mut code, 3);
    code.push(0x15);
    code.push(0x06);
    let consts = (0..4).map(Value::DInt).collect::<Vec<_>>();
    let (module, pou_id) = manual_vm_module(code, consts, 0);
    let pou = module.pou(pou_id).expect("manual pou");
    let decoded = decode_pou(&module, pou.code_start, pou.code_end).expect("decode pou");
    let leaders = collect_block_leaders(&decoded, pou.code_start, pou.code_end).expect("leaders");
    compute_block_entry_stack_depths(&decoded, &leaders, pou.code_start, pou.code_end)
        .expect("ROT exact stack depth should be accepted");
}

#[test]
fn register_ir_decode_rejects_conflicting_block_entry_depths() {
    let mut code = Vec::new();
    code.push(0x10);
    emit_u32(&mut code, 0);
    let branch_pc = code.len();
    code.push(0x04);
    emit_i32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 1);
    let jump_pc = code.len();
    code.push(0x02);
    emit_i32(&mut code, 0);
    let target_pc = code.len();
    code.push(0x06);
    patch_i32(
        &mut code,
        branch_pc + 1,
        target_pc as i32 - (branch_pc + 5) as i32,
    );
    patch_i32(
        &mut code,
        jump_pc + 1,
        target_pc as i32 - (jump_pc + 5) as i32,
    );

    let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(0), Value::DInt(1)], 0);
    let err =
        lower_pou_to_register_ir(&module, pou_id).expect_err("conflicting entry depth must fail");
    let RuntimeError::InvalidBytecode(message) = err else {
        panic!("expected invalid bytecode for conflicting entry depth");
    };
    assert!(
        message.contains("inconsistent block-entry stack depth"),
        "unexpected conflicting-depth message: {message}",
    );
}

#[test]
fn register_ir_decode_leaders_exclude_exit_and_unconditional_fallthrough() {
    let mut conditional_to_exit = Vec::new();
    conditional_to_exit.push(0x10);
    emit_u32(&mut conditional_to_exit, 0);
    conditional_to_exit.push(0x03);
    emit_i32(&mut conditional_to_exit, 0);
    let (module, pou_id) = manual_vm_module(conditional_to_exit, vec![Value::Bool(true)], 0);
    let pou = module.pou(pou_id).expect("manual pou");
    let decoded = decode_pou(&module, pou.code_start, pou.code_end).expect("decode pou");
    let leaders = collect_block_leaders(&decoded, pou.code_start, pou.code_end).expect("leaders");
    assert_eq!(leaders, vec![pou.code_start]);
    compute_block_entry_stack_depths(&decoded, &leaders, pou.code_start, pou.code_end)
        .expect("conditional branch at code_end should not require an exit leader");

    let mut jump_over_trailing = Vec::new();
    jump_over_trailing.push(0x02);
    emit_i32(&mut jump_over_trailing, 0);
    jump_over_trailing.push(0x00);
    jump_over_trailing.push(0x06);
    let jump_target = jump_over_trailing.len();
    patch_i32(&mut jump_over_trailing, 1, jump_target as i32 - 5);
    let (module, pou_id) = manual_vm_module(jump_over_trailing, Vec::new(), 0);
    let pou = module.pou(pou_id).expect("manual pou");
    let decoded = decode_pou(&module, pou.code_start, pou.code_end).expect("decode pou");
    let leaders = collect_block_leaders(&decoded, pou.code_start, pou.code_end).expect("leaders");
    assert_eq!(leaders, vec![pou.code_start]);
}

#[test]
fn register_ir_decode_return_stops_entry_depth_propagation() {
    let mut code = Vec::new();
    code.push(0x06);
    code.push(0x10);
    emit_u32(&mut code, 0);
    let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(0)], 0);
    let pou = module.pou(pou_id).expect("manual pou");
    let decoded = decode_pou(&module, pou.code_start, pou.code_end).expect("decode pou");
    let entry_depths =
        compute_block_entry_stack_depths(&decoded, &[pou.code_start, 1], pou.code_start, pou.code_end)
            .expect("entry stack depths");
    assert_eq!(entry_depths.get(&pou.code_start), Some(&0));
    assert!(
        !entry_depths.contains_key(&1),
        "RETURN must not propagate stack depth into the following block: {entry_depths:?}",
    );
}

#[test]
fn register_ir_lowering_preserves_fallback_operands() {
    let mut code = Vec::new();
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x62);
    emit_u32(&mut code, 0x1234_5678);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(1)], 0);
    let lowered = lower_pou_to_register_ir(&module, pou_id).expect("lower fallback opcode");
    let fallback_operands = lowered
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .find_map(|instruction| match instruction {
            RegisterInstr::VmFallback { opcode: 0x62, operands } => Some(operands),
            _ => None,
        })
        .expect("0x62 fallback instruction");
    assert_eq!(fallback_operands.as_slice(), [0x78, 0x56, 0x34, 0x12]);
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

