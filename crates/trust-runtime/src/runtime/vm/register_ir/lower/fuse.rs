use super::*;

macro_rules! advance_fuse_index {
    ($index:expr, $consumed:expr, $instruction_count:expr) => {{
        let index = $index;
        let consumed = $consumed;
        let instruction_count = $instruction_count;
        assert!(
            consumed > 0,
            "register-ir fuse must consume at least one instruction"
        );
        let next_index = index
            .checked_add(consumed)
            .expect("register-ir fuse index overflow");
        assert!(
            next_index > index && next_index <= instruction_count,
            "register-ir fuse advanced from {index} by {consumed} past {instruction_count}"
        );
        next_index
    }};
}

pub(in crate::runtime::vm::register_ir) fn fuse_register_block_instructions(
    instructions: &[RegisterInstr],
) -> Vec<RegisterInstr> {
    if instructions.len() < 4 {
        return instructions.to_vec();
    }
    let mut fused = Vec::with_capacity(instructions.len());
    let mut index = 0usize;
    while index < instructions.len() {
        if let Some((instruction, consumed)) = try_fuse_instruction_window(instructions, index) {
            fused.push(instruction);
            index = advance_fuse_index!(index, consumed, instructions.len());
            continue;
        }
        fused.push(instructions[index].clone());
        index = advance_fuse_index!(index, 1, instructions.len());
    }
    fused
}

fn try_fuse_instruction_window(
    instructions: &[RegisterInstr],
    index: usize,
) -> Option<(RegisterInstr, usize)> {
    if index + 2 < instructions.len() {
        if let (
            RegisterInstr::LoadSelf { dest: self_reg },
            RegisterInstr::RefField {
                base,
                field_idx,
                dest: field_reg,
            },
            RegisterInstr::LoadDynamic { reference, dest },
        ) = (
            &instructions[index],
            &instructions[index + 1],
            &instructions[index + 2],
        ) {
            if base == self_reg
                && reference == field_reg
                && !register_used_after(instructions, index + 3, *self_reg)
                && !register_used_after(instructions, index + 3, *field_reg)
            {
                return Some((
                    RegisterInstr::LoadSelfFieldDynamic {
                        field_idx: *field_idx,
                        dest: *dest,
                    },
                    3,
                ));
            }
        }

        if let (
            RegisterInstr::LoadSelf { dest: self_reg },
            RegisterInstr::RefField {
                base,
                field_idx,
                dest: field_reg,
            },
            RegisterInstr::StoreDynamic { reference, value },
        ) = (
            &instructions[index],
            &instructions[index + 1],
            &instructions[index + 2],
        ) {
            if base == self_reg
                && reference == field_reg
                && !register_used_after(instructions, index + 3, *self_reg)
                && !register_used_after(instructions, index + 3, *field_reg)
            {
                return Some((
                    RegisterInstr::StoreSelfFieldDynamic {
                        field_idx: *field_idx,
                        value: *value,
                    },
                    3,
                ));
            }
        }
    }

    if index + 3 >= instructions.len() {
        return None;
    }

    if let (
        RegisterInstr::LoadRef {
            dest: left_reg,
            ref_idx: left_ref_idx,
        },
        RegisterInstr::LoadRef {
            dest: right_reg,
            ref_idx: right_ref_idx,
        },
        RegisterInstr::Binary {
            op,
            left,
            right,
            dest,
        },
        RegisterInstr::StoreRef { ref_idx, src },
    ) = (
        &instructions[index],
        &instructions[index + 1],
        &instructions[index + 2],
        &instructions[index + 3],
    ) {
        if left == left_reg
            && right == right_reg
            && src == dest
            && !register_used_after(instructions, index + 4, *left_reg)
            && !register_used_after(instructions, index + 4, *right_reg)
            && !register_used_after(instructions, index + 4, *dest)
        {
            return Some((
                RegisterInstr::BinaryRefToRef {
                    op: *op,
                    left_ref_idx: *left_ref_idx,
                    right_ref_idx: *right_ref_idx,
                    dest_ref_idx: *ref_idx,
                },
                4,
            ));
        }
    }

    if let (
        RegisterInstr::LoadRef {
            dest: left_reg,
            ref_idx: left_ref_idx,
        },
        RegisterInstr::LoadConst {
            dest: const_reg,
            const_idx,
        },
        RegisterInstr::Binary {
            op,
            left,
            right,
            dest,
        },
        RegisterInstr::StoreRef { ref_idx, src },
    ) = (
        &instructions[index],
        &instructions[index + 1],
        &instructions[index + 2],
        &instructions[index + 3],
    ) {
        if left == left_reg
            && right == const_reg
            && src == dest
            && !register_used_after(instructions, index + 4, *left_reg)
            && !register_used_after(instructions, index + 4, *const_reg)
            && !register_used_after(instructions, index + 4, *dest)
        {
            return Some((
                RegisterInstr::BinaryRefConstToRef {
                    op: *op,
                    left_ref_idx: *left_ref_idx,
                    const_idx: *const_idx,
                    dest_ref_idx: *ref_idx,
                },
                4,
            ));
        }
    }

    if let (
        RegisterInstr::LoadConst {
            dest: const_reg,
            const_idx,
        },
        RegisterInstr::LoadRef {
            dest: right_reg,
            ref_idx: right_ref_idx,
        },
        RegisterInstr::Binary {
            op,
            left,
            right,
            dest,
        },
        RegisterInstr::StoreRef { ref_idx, src },
    ) = (
        &instructions[index],
        &instructions[index + 1],
        &instructions[index + 2],
        &instructions[index + 3],
    ) {
        if left == const_reg
            && right == right_reg
            && src == dest
            && !register_used_after(instructions, index + 4, *const_reg)
            && !register_used_after(instructions, index + 4, *right_reg)
            && !register_used_after(instructions, index + 4, *dest)
        {
            return Some((
                RegisterInstr::BinaryConstRefToRef {
                    op: *op,
                    const_idx: *const_idx,
                    right_ref_idx: *right_ref_idx,
                    dest_ref_idx: *ref_idx,
                },
                4,
            ));
        }
    }

    if let (
        RegisterInstr::LoadRef {
            dest: ref_reg,
            ref_idx,
        },
        RegisterInstr::LoadConst {
            dest: const_reg,
            const_idx,
        },
        RegisterInstr::Binary {
            op,
            left,
            right,
            dest,
        },
        RegisterInstr::JumpIf {
            cond,
            jump_if_true,
            target,
        },
    ) = (
        &instructions[index],
        &instructions[index + 1],
        &instructions[index + 2],
        &instructions[index + 3],
    ) {
        if is_cmp_binary_op(*op)
            && left == ref_reg
            && right == const_reg
            && cond == dest
            && !register_used_after(instructions, index + 4, *ref_reg)
            && !register_used_after(instructions, index + 4, *const_reg)
            && !register_used_after(instructions, index + 4, *dest)
        {
            return Some((
                RegisterInstr::CmpRefConstJumpIf {
                    op: *op,
                    ref_idx: *ref_idx,
                    const_idx: *const_idx,
                    jump_if_true: *jump_if_true,
                    target: *target,
                },
                4,
            ));
        }
    }

    None
}

fn register_used_after(
    instructions: &[RegisterInstr],
    start_index: usize,
    register: RegisterId,
) -> bool {
    instructions[start_index..]
        .iter()
        .any(|instruction| instruction_reads_register(instruction, register))
}

pub(in crate::runtime::vm::register_ir) fn instruction_reads_register(
    instruction: &RegisterInstr,
    register: RegisterId,
) -> bool {
    match instruction {
        RegisterInstr::CallNative { args, .. } => args.contains(&register),
        RegisterInstr::SizeOfValue { src, .. } => *src == register,
        RegisterInstr::RefField { base, .. } => *base == register,
        RegisterInstr::RefIndex { base, index, .. } => *base == register || *index == register,
        RegisterInstr::LoadDynamic { reference, .. } => *reference == register,
        RegisterInstr::StoreSelfFieldDynamic { value, .. } => *value == register,
        RegisterInstr::StoreDynamic { reference, value } => {
            *reference == register || *value == register
        }
        RegisterInstr::Unary { src, .. } => *src == register,
        RegisterInstr::Binary { left, right, .. } => *left == register || *right == register,
        RegisterInstr::StoreRef { src, .. } => *src == register,
        RegisterInstr::JumpIf { cond, .. } => *cond == register,
        _ => false,
    }
}

pub(in crate::runtime::vm::register_ir) fn is_cmp_binary_op(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge
    )
}
