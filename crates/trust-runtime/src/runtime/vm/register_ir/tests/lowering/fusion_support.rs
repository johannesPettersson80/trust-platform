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
fn register_ir_fuse_preserves_unmatched_windows_and_fused_tail() {
    let unmatched = vec![
        RegisterInstr::LoadConst {
            dest: RegisterId(0),
            const_idx: 0,
        },
        RegisterInstr::LoadConst {
            dest: RegisterId(1),
            const_idx: 1,
        },
        RegisterInstr::Binary {
            op: BinaryOp::Add,
            left: RegisterId(0),
            right: RegisterId(1),
            dest: RegisterId(2),
        },
        RegisterInstr::StoreDynamic {
            reference: RegisterId(3),
            value: RegisterId(2),
        },
        RegisterInstr::Return,
    ];
    assert_eq!(fuse_register_block_instructions(&unmatched), unmatched);

    let fused_with_tail = vec![
        RegisterInstr::LoadRef {
            dest: RegisterId(0),
            ref_idx: 10,
        },
        RegisterInstr::LoadRef {
            dest: RegisterId(1),
            ref_idx: 11,
        },
        RegisterInstr::Binary {
            op: BinaryOp::Add,
            left: RegisterId(0),
            right: RegisterId(1),
            dest: RegisterId(2),
        },
        RegisterInstr::StoreRef {
            ref_idx: 12,
            src: RegisterId(2),
        },
        RegisterInstr::LoadConst {
            dest: RegisterId(3),
            const_idx: 99,
        },
    ];
    assert_eq!(
        fuse_register_block_instructions(&fused_with_tail),
        vec![
            RegisterInstr::BinaryRefToRef {
                op: BinaryOp::Add,
                left_ref_idx: 10,
                right_ref_idx: 11,
                dest_ref_idx: 12,
            },
            RegisterInstr::LoadConst {
                dest: RegisterId(3),
                const_idx: 99,
            },
        ]
    );

    let fused_exact = &fused_with_tail[..4];
    assert_eq!(
        fuse_register_block_instructions(fused_exact),
        vec![RegisterInstr::BinaryRefToRef {
            op: BinaryOp::Add,
            left_ref_idx: 10,
            right_ref_idx: 11,
            dest_ref_idx: 12,
        }]
    );
}

fn assert_fuses(instructions: Vec<RegisterInstr>, expected: Vec<RegisterInstr>) {
    assert_eq!(fuse_register_block_instructions(&instructions), expected);
}

fn assert_no_fusion(instructions: Vec<RegisterInstr>) {
    assert_eq!(fuse_register_block_instructions(&instructions), instructions);
}

fn binary_ref_to_ref_window() -> Vec<RegisterInstr> {
    vec![
        RegisterInstr::LoadRef {
            dest: RegisterId(0),
            ref_idx: 10,
        },
        RegisterInstr::LoadRef {
            dest: RegisterId(1),
            ref_idx: 11,
        },
        RegisterInstr::Binary {
            op: BinaryOp::Add,
            left: RegisterId(0),
            right: RegisterId(1),
            dest: RegisterId(2),
        },
        RegisterInstr::StoreRef {
            ref_idx: 12,
            src: RegisterId(2),
        },
    ]
}

fn binary_ref_const_window() -> Vec<RegisterInstr> {
    vec![
        RegisterInstr::LoadRef {
            dest: RegisterId(0),
            ref_idx: 10,
        },
        RegisterInstr::LoadConst {
            dest: RegisterId(1),
            const_idx: 7,
        },
        RegisterInstr::Binary {
            op: BinaryOp::Add,
            left: RegisterId(0),
            right: RegisterId(1),
            dest: RegisterId(2),
        },
        RegisterInstr::StoreRef {
            ref_idx: 12,
            src: RegisterId(2),
        },
    ]
}

fn binary_const_ref_window() -> Vec<RegisterInstr> {
    vec![
        RegisterInstr::LoadConst {
            dest: RegisterId(0),
            const_idx: 7,
        },
        RegisterInstr::LoadRef {
            dest: RegisterId(1),
            ref_idx: 11,
        },
        RegisterInstr::Binary {
            op: BinaryOp::Add,
            left: RegisterId(0),
            right: RegisterId(1),
            dest: RegisterId(2),
        },
        RegisterInstr::StoreRef {
            ref_idx: 12,
            src: RegisterId(2),
        },
    ]
}

fn cmp_ref_const_jump_window() -> Vec<RegisterInstr> {
    vec![
        RegisterInstr::LoadRef {
            dest: RegisterId(0),
            ref_idx: 10,
        },
        RegisterInstr::LoadConst {
            dest: RegisterId(1),
            const_idx: 7,
        },
        RegisterInstr::Binary {
            op: BinaryOp::Eq,
            left: RegisterId(0),
            right: RegisterId(1),
            dest: RegisterId(2),
        },
        RegisterInstr::JumpIf {
            cond: RegisterId(2),
            jump_if_true: true,
            target: BlockTarget::Exit,
        },
    ]
}

fn prefixed(instructions: Vec<RegisterInstr>) -> Vec<RegisterInstr> {
    let mut prefixed = vec![RegisterInstr::Nop];
    prefixed.extend(instructions);
    prefixed
}

