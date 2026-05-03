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
fn register_ir_verifier_rejects_undefined_source_register() {
    let program = RegisterProgram {
        pou_id: 1,
        entry_block: 0,
        max_registers: 1,
        blocks: vec![RegisterBlock {
            id: 0,
            start_pc: 0,
            end_pc: 1,
            entry_stack_depth: 0,
            instructions: vec![RegisterInstr::StoreRef {
                ref_idx: 0,
                src: RegisterId(0),
            }],
        }],
    };

    let err = verify_register_program(&program).expect_err("verification should fail");
    assert_invalid_bytecode_contains(err, "source register 0 used before definition");
}

#[test]
fn register_ir_verifier_rejects_move_destination_out_of_bounds() {
    let program = RegisterProgram {
        pou_id: 1,
        entry_block: 0,
        max_registers: 1,
        blocks: vec![RegisterBlock {
            id: 0,
            start_pc: 0,
            end_pc: 1,
            entry_stack_depth: 1,
            instructions: vec![RegisterInstr::Move {
                src: RegisterId(0),
                dest: RegisterId(1),
            }],
        }],
    };

    let err = verify_register_program(&program).expect_err("verification should fail");
    assert_invalid_bytecode_contains(err, "destination register 1 out of bounds");
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
