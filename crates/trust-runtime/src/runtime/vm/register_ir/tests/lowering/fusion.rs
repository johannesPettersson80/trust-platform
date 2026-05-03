#[test]
fn register_ir_fuse_rejects_partial_self_field_dynamic_windows() {
    assert_fuses(
        vec![
            RegisterInstr::LoadSelf {
                dest: RegisterId(0),
            },
            RegisterInstr::RefField {
                base: RegisterId(0),
                field_idx: 5,
                dest: RegisterId(1),
            },
            RegisterInstr::LoadDynamic {
                reference: RegisterId(1),
                dest: RegisterId(2),
            },
            RegisterInstr::Return,
        ],
        vec![
            RegisterInstr::LoadSelfFieldDynamic {
                field_idx: 5,
                dest: RegisterId(2),
            },
            RegisterInstr::Return,
        ],
    );

    assert_no_fusion(vec![
        RegisterInstr::LoadSelf {
            dest: RegisterId(0),
        },
        RegisterInstr::RefField {
            base: RegisterId(0),
            field_idx: 5,
            dest: RegisterId(1),
        },
        RegisterInstr::LoadDynamic {
            reference: RegisterId(9),
            dest: RegisterId(2),
        },
        RegisterInstr::Return,
    ]);
    assert_no_fusion(vec![
        RegisterInstr::LoadSelf {
            dest: RegisterId(0),
        },
        RegisterInstr::RefField {
            base: RegisterId(0),
            field_idx: 5,
            dest: RegisterId(1),
        },
        RegisterInstr::LoadDynamic {
            reference: RegisterId(1),
            dest: RegisterId(2),
        },
        RegisterInstr::RefField {
            base: RegisterId(0),
            field_idx: 6,
            dest: RegisterId(9),
        },
    ]);
    assert_no_fusion(vec![
        RegisterInstr::LoadSelf {
            dest: RegisterId(0),
        },
        RegisterInstr::RefField {
            base: RegisterId(0),
            field_idx: 5,
            dest: RegisterId(1),
        },
        RegisterInstr::LoadDynamic {
            reference: RegisterId(1),
            dest: RegisterId(2),
        },
        RegisterInstr::LoadDynamic {
            reference: RegisterId(1),
            dest: RegisterId(9),
        },
    ]);

    assert_fuses(
        vec![
            RegisterInstr::LoadSelf {
                dest: RegisterId(0),
            },
            RegisterInstr::RefField {
                base: RegisterId(0),
                field_idx: 5,
                dest: RegisterId(1),
            },
            RegisterInstr::StoreDynamic {
                reference: RegisterId(1),
                value: RegisterId(2),
            },
            RegisterInstr::Return,
        ],
        vec![
            RegisterInstr::StoreSelfFieldDynamic {
                field_idx: 5,
                value: RegisterId(2),
            },
            RegisterInstr::Return,
        ],
    );
    assert_no_fusion(vec![
        RegisterInstr::LoadSelf {
            dest: RegisterId(0),
        },
        RegisterInstr::RefField {
            base: RegisterId(0),
            field_idx: 5,
            dest: RegisterId(1),
        },
        RegisterInstr::StoreDynamic {
            reference: RegisterId(9),
            value: RegisterId(2),
        },
        RegisterInstr::Return,
    ]);
    assert_no_fusion(vec![
        RegisterInstr::LoadSelf {
            dest: RegisterId(0),
        },
        RegisterInstr::RefField {
            base: RegisterId(0),
            field_idx: 5,
            dest: RegisterId(1),
        },
        RegisterInstr::StoreDynamic {
            reference: RegisterId(1),
            value: RegisterId(2),
        },
        RegisterInstr::RefField {
            base: RegisterId(0),
            field_idx: 6,
            dest: RegisterId(9),
        },
    ]);
    assert_no_fusion(vec![
        RegisterInstr::LoadSelf {
            dest: RegisterId(0),
        },
        RegisterInstr::RefField {
            base: RegisterId(0),
            field_idx: 5,
            dest: RegisterId(1),
        },
        RegisterInstr::StoreDynamic {
            reference: RegisterId(1),
            value: RegisterId(2),
        },
        RegisterInstr::LoadDynamic {
            reference: RegisterId(1),
            dest: RegisterId(9),
        },
    ]);
}

#[test]
fn register_ir_fuse_covers_ref_binary_variants_and_guard_failures() {
    assert_fuses(
        binary_ref_to_ref_window(),
        vec![RegisterInstr::BinaryRefToRef {
            op: BinaryOp::Add,
            left_ref_idx: 10,
            right_ref_idx: 11,
            dest_ref_idx: 12,
        }],
    );
    assert_fuses(
        binary_ref_const_window(),
        vec![RegisterInstr::BinaryRefConstToRef {
            op: BinaryOp::Add,
            left_ref_idx: 10,
            const_idx: 7,
            dest_ref_idx: 12,
        }],
    );
    assert_fuses(
        prefixed(binary_ref_const_window()),
        vec![
            RegisterInstr::Nop,
            RegisterInstr::BinaryRefConstToRef {
                op: BinaryOp::Add,
                left_ref_idx: 10,
                const_idx: 7,
                dest_ref_idx: 12,
            },
        ],
    );
    assert_fuses(
        binary_const_ref_window(),
        vec![RegisterInstr::BinaryConstRefToRef {
            op: BinaryOp::Add,
            const_idx: 7,
            right_ref_idx: 11,
            dest_ref_idx: 12,
        }],
    );
    assert_fuses(
        prefixed(binary_const_ref_window()),
        vec![
            RegisterInstr::Nop,
            RegisterInstr::BinaryConstRefToRef {
                op: BinaryOp::Add,
                const_idx: 7,
                right_ref_idx: 11,
                dest_ref_idx: 12,
            },
        ],
    );

    for mut window in [
        binary_ref_to_ref_window(),
        binary_ref_const_window(),
        binary_const_ref_window(),
    ] {
        if let RegisterInstr::Binary { right, .. } = &mut window[2] {
            *right = RegisterId(9);
        }
        assert_no_fusion(window);
    }
    for mut window in [
        binary_ref_to_ref_window(),
        binary_ref_const_window(),
        binary_const_ref_window(),
    ] {
        if let RegisterInstr::StoreRef { src, .. } = &mut window[3] {
            *src = RegisterId(9);
        }
        assert_no_fusion(window);
    }
    for mut window in [
        binary_ref_to_ref_window(),
        binary_ref_const_window(),
        binary_const_ref_window(),
    ] {
        window.push(RegisterInstr::SizeOfValue {
            src: RegisterId(0),
            dest: RegisterId(9),
        });
        assert_no_fusion(window);
    }
    for mut window in [
        binary_ref_to_ref_window(),
        binary_ref_const_window(),
        binary_const_ref_window(),
    ] {
        window.push(RegisterInstr::SizeOfValue {
            src: RegisterId(1),
            dest: RegisterId(9),
        });
        assert_no_fusion(window);
    }
    for mut window in [
        binary_ref_to_ref_window(),
        binary_ref_const_window(),
        binary_const_ref_window(),
    ] {
        window.push(RegisterInstr::SizeOfValue {
            src: RegisterId(2),
            dest: RegisterId(9),
        });
        assert_no_fusion(window);
    }
}

#[test]
fn register_ir_fuse_covers_compare_jump_guards() {
    assert_fuses(
        cmp_ref_const_jump_window(),
        vec![RegisterInstr::CmpRefConstJumpIf {
            op: BinaryOp::Eq,
            ref_idx: 10,
            const_idx: 7,
            jump_if_true: true,
            target: BlockTarget::Exit,
        }],
    );
    assert_fuses(
        prefixed(cmp_ref_const_jump_window()),
        vec![
            RegisterInstr::Nop,
            RegisterInstr::CmpRefConstJumpIf {
                op: BinaryOp::Eq,
                ref_idx: 10,
                const_idx: 7,
                jump_if_true: true,
                target: BlockTarget::Exit,
            },
        ],
    );

    let mut non_cmp = cmp_ref_const_jump_window();
    if let RegisterInstr::Binary { op, .. } = &mut non_cmp[2] {
        *op = BinaryOp::Add;
    }
    assert_no_fusion(non_cmp);

    let mut wrong_left = cmp_ref_const_jump_window();
    if let RegisterInstr::Binary { left, .. } = &mut wrong_left[2] {
        *left = RegisterId(9);
    }
    assert_no_fusion(wrong_left);

    let mut wrong_right = cmp_ref_const_jump_window();
    if let RegisterInstr::Binary { right, .. } = &mut wrong_right[2] {
        *right = RegisterId(9);
    }
    assert_no_fusion(wrong_right);

    let mut wrong_cond = cmp_ref_const_jump_window();
    if let RegisterInstr::JumpIf { cond, .. } = &mut wrong_cond[3] {
        *cond = RegisterId(9);
    }
    assert_no_fusion(wrong_cond);

    for register in [RegisterId(0), RegisterId(1), RegisterId(2)] {
        let mut live_after = cmp_ref_const_jump_window();
        live_after.push(RegisterInstr::SizeOfValue {
            src: register,
            dest: RegisterId(9),
        });
        assert_no_fusion(live_after);
    }
}

#[test]
fn register_ir_fuse_instruction_read_detection_covers_all_operands() {
    let target = RegisterId(7);
    let other = RegisterId(8);
    let third = RegisterId(9);

    let cases = [
        RegisterInstr::CallNative {
            kind: 0,
            symbol_idx: 0,
            args: vec![other, target],
            dest: third,
        },
        RegisterInstr::SizeOfValue {
            src: target,
            dest: third,
        },
        RegisterInstr::RefField {
            base: target,
            field_idx: 0,
            dest: third,
        },
        RegisterInstr::RefIndex {
            base: target,
            index: other,
            dest: third,
        },
        RegisterInstr::RefIndex {
            base: other,
            index: target,
            dest: third,
        },
        RegisterInstr::LoadDynamic {
            reference: target,
            dest: third,
        },
        RegisterInstr::StoreSelfFieldDynamic {
            field_idx: 0,
            value: target,
        },
        RegisterInstr::StoreDynamic {
            reference: target,
            value: other,
        },
        RegisterInstr::StoreDynamic {
            reference: other,
            value: target,
        },
        RegisterInstr::Unary {
            op: UnaryOp::Not,
            src: target,
            dest: third,
        },
        RegisterInstr::Binary {
            op: BinaryOp::Add,
            left: target,
            right: other,
            dest: third,
        },
        RegisterInstr::Binary {
            op: BinaryOp::Add,
            left: other,
            right: target,
            dest: third,
        },
        RegisterInstr::StoreRef {
            ref_idx: 0,
            src: target,
        },
        RegisterInstr::JumpIf {
            cond: target,
            jump_if_true: true,
            target: BlockTarget::Exit,
        },
    ];

    for instruction in cases {
        assert!(
            instruction_reads_register(&instruction, target),
            "expected {instruction:?} to read {target:?}",
        );
        assert!(
            !instruction_reads_register(&instruction, RegisterId(42)),
            "unexpected unrelated read for {instruction:?}",
        );
    }

    assert!(!instruction_reads_register(
        &RegisterInstr::LoadConst {
            dest: target,
            const_idx: 0,
        },
        target,
    ));
    assert!(!instruction_reads_register(
        &RegisterInstr::Move {
            src: target,
            dest: other,
        },
        target,
    ));
}

