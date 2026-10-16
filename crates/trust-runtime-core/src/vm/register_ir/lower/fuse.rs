use super::super::registers::for_each_instruction_register_read;
use super::*;

pub(super) struct FusedRegisterBlock {
    pub(super) instructions: Vec<RegisterInstr>,
    pub(super) costs: Vec<usize>,
    pub(super) cost_steps: Vec<Vec<usize>>,
}

/// Fuse equivalent instruction windows, retaining every externally used value.
#[cfg(any(test, feature = "test-support"))]
pub fn fuse_register_block_instructions(
    instructions: &[RegisterInstr],
) -> Result<Vec<RegisterInstr>, RuntimeError> {
    let costs = vec![1; instructions.len()];
    fuse_register_block_instructions_with_costs(instructions, &costs).map(|(body, _)| body)
}

/// Fuse a block while retaining total and per-source-instruction work charges.
#[cfg(any(test, feature = "test-support"))]
pub fn fuse_register_block_instructions_with_costs(
    instructions: &[RegisterInstr],
    instruction_costs: &[usize],
) -> Result<(Vec<RegisterInstr>, Vec<usize>), RuntimeError> {
    let fused = fuse_register_block_instructions_with_cost_steps(instructions, instruction_costs)?;
    Ok((fused.instructions, fused.costs))
}

pub(super) fn fuse_register_block_instructions_with_cost_steps(
    instructions: &[RegisterInstr],
    instruction_costs: &[usize],
) -> Result<FusedRegisterBlock, RuntimeError> {
    if instructions.len() != instruction_costs.len() {
        return Err(invalid_bytecode(
            "register-ir instruction-cost table length mismatch",
        ));
    }
    if instructions.len() < 4 {
        return Ok(FusedRegisterBlock {
            instructions: instructions.to_vec(),
            costs: instruction_costs.to_vec(),
            cost_steps: instruction_costs.iter().map(|cost| vec![*cost]).collect(),
        });
    }
    // Registers are not assumed dense here: this helper also accepts raw fixture
    // programs. One indexed pass bounds work by reads * log(distinct registers),
    // rather than rescanning the entire remaining block for every window.
    let last_reads = last_register_reads(instructions);
    let mut fused = Vec::with_capacity(instructions.len());
    let mut fused_costs = Vec::with_capacity(instructions.len());
    let mut fused_cost_steps = Vec::with_capacity(instructions.len());
    let mut index = 0usize;
    while index < instructions.len() {
        if let Some((instruction, consumed)) =
            try_fuse_instruction_window(instructions, index, &last_reads)
        {
            let end = index.checked_add(consumed).ok_or(RuntimeError::Overflow)?;
            let costs = instruction_costs
                .get(index..end)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            let cost = costs.iter().try_fold(0usize, |total, cost| {
                total.checked_add(*cost).ok_or(RuntimeError::Overflow)
            })?;
            fused.push(instruction);
            fused_costs.push(cost);
            fused_cost_steps.push(costs.to_vec());
            index = end;
            continue;
        }
        fused.push(instructions[index].clone());
        fused_costs.push(instruction_costs[index]);
        fused_cost_steps.push(vec![instruction_costs[index]]);
        index += 1;
    }
    Ok(FusedRegisterBlock {
        instructions: fused,
        costs: fused_costs,
        cost_steps: fused_cost_steps,
    })
}

fn last_register_reads(instructions: &[RegisterInstr]) -> BTreeMap<RegisterId, usize> {
    let mut reads = BTreeMap::new();
    for (index, instruction) in instructions.iter().enumerate() {
        for_each_instruction_register_read(instruction, |register| {
            reads.insert(register, index);
        });
    }
    reads
}

fn try_fuse_instruction_window(
    instructions: &[RegisterInstr],
    index: usize,
    last_reads: &BTreeMap<RegisterId, usize>,
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
        ) && base == self_reg
            && reference == field_reg
            && !register_used_after(last_reads, index + 3, *self_reg)
            && !register_used_after(last_reads, index + 3, *field_reg)
        {
            return Some((
                RegisterInstr::LoadSelfFieldDynamic {
                    field_idx: *field_idx,
                    dest: *dest,
                },
                3,
            ));
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
        ) && base == self_reg
            && reference == field_reg
            && value != self_reg
            && value != field_reg
            && !register_used_after(last_reads, index + 3, *self_reg)
            && !register_used_after(last_reads, index + 3, *field_reg)
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
    ) && left == left_reg
        && right == right_reg
        && src == dest
        && !register_used_after(last_reads, index + 4, *left_reg)
        && !register_used_after(last_reads, index + 4, *right_reg)
        && !register_used_after(last_reads, index + 4, *dest)
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
    ) && left == left_reg
        && right == const_reg
        && src == dest
        && !register_used_after(last_reads, index + 4, *left_reg)
        && !register_used_after(last_reads, index + 4, *const_reg)
        && !register_used_after(last_reads, index + 4, *dest)
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
    ) && left == const_reg
        && right == right_reg
        && src == dest
        && !register_used_after(last_reads, index + 4, *const_reg)
        && !register_used_after(last_reads, index + 4, *right_reg)
        && !register_used_after(last_reads, index + 4, *dest)
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
    ) && is_cmp_binary_op(*op)
        && left == ref_reg
        && right == const_reg
        && cond == dest
        && !register_used_after(last_reads, index + 4, *ref_reg)
        && !register_used_after(last_reads, index + 4, *const_reg)
        && !register_used_after(last_reads, index + 4, *dest)
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

    None
}

fn register_used_after(
    last_reads: &BTreeMap<RegisterId, usize>,
    start_index: usize,
    register: RegisterId,
) -> bool {
    last_reads
        .get(&register)
        .is_some_and(|last| *last >= start_index)
}

/// Whether an instruction observes a register, using the accounting use table.
#[cfg(any(test, feature = "test-support"))]
pub fn instruction_reads_register(instruction: &RegisterInstr, register: RegisterId) -> bool {
    let mut found = false;
    for_each_instruction_register_read(instruction, |read| found |= read == register);
    found
}

/// Whether an operator returns the BOOL result required by comparison fusion.
pub fn is_cmp_binary_op(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assignment(first: u32) -> [RegisterInstr; 4] {
        let left = RegisterId(first);
        let right = RegisterId(first + 1);
        let result = RegisterId(first + 2);
        [
            RegisterInstr::LoadRef {
                dest: left,
                ref_idx: 0,
            },
            RegisterInstr::LoadConst {
                dest: right,
                const_idx: 0,
            },
            RegisterInstr::Binary {
                op: BinaryOp::Add,
                left,
                right,
                dest: result,
            },
            RegisterInstr::StoreRef {
                ref_idx: 0,
                src: result,
            },
        ]
    }

    #[test]
    fn fusion_preserves_a_value_observed_by_a_normalization_move() {
        let mut instructions = assignment(0).to_vec();
        instructions.push(RegisterInstr::Move {
            src: RegisterId(2),
            dest: RegisterId(3),
        });
        assert!(instruction_reads_register(&instructions[4], RegisterId(2)));
        assert_eq!(
            fuse_register_block_instructions(&instructions).unwrap(),
            instructions
        );
    }

    #[test]
    fn self_field_store_keeps_any_intermediate_used_as_its_value() {
        for value in [RegisterId(0), RegisterId(1)] {
            let instructions = vec![
                RegisterInstr::LoadSelf {
                    dest: RegisterId(0),
                },
                RegisterInstr::RefField {
                    base: RegisterId(0),
                    field_idx: 0,
                    dest: RegisterId(1),
                },
                RegisterInstr::StoreDynamic {
                    reference: RegisterId(1),
                    value,
                },
                RegisterInstr::Return,
            ];
            assert_eq!(
                fuse_register_block_instructions(&instructions).unwrap(),
                instructions
            );
        }
    }

    #[test]
    fn one_large_straight_line_block_uses_one_sparse_last_read_index() {
        const WINDOWS: u32 = 16_384;
        let instructions = (0..WINDOWS)
            .flat_map(|window| assignment(window * 3))
            .collect::<Vec<_>>();
        let reads = last_register_reads(&instructions);
        assert_eq!(reads.len(), WINDOWS as usize * 3);
        assert_eq!(
            reads.get(&RegisterId(WINDOWS * 3 - 1)),
            Some(&(instructions.len() - 1))
        );
        let fused = fuse_register_block_instructions_with_cost_steps(
            &instructions,
            &vec![1; instructions.len()],
        )
        .unwrap();
        assert_eq!(fused.instructions.len(), WINDOWS as usize);
        assert!(fused
            .instructions
            .iter()
            .all(|instruction| matches!(instruction, RegisterInstr::BinaryRefConstToRef { .. })));
        assert_eq!(fused.costs, vec![4; WINDOWS as usize]);
        assert!(fused.cost_steps.iter().all(|costs| costs == &[1, 1, 1, 1]));
    }

    #[test]
    fn sparse_register_identity_does_not_size_an_allocation_by_the_raw_id() {
        let instructions = [RegisterInstr::Move {
            src: RegisterId(u32::MAX),
            dest: RegisterId(0),
        }];
        let reads = last_register_reads(&instructions);
        assert_eq!(reads.len(), 1);
        assert!(register_used_after(&reads, 0, RegisterId(u32::MAX)));
        assert!(!register_used_after(&reads, 1, RegisterId(u32::MAX)));
    }

    #[test]
    fn malformed_cost_tables_return_errors_instead_of_panicking() {
        let instructions = assignment(0);
        assert!(fuse_register_block_instructions_with_costs(&instructions, &[1]).is_err());
        assert_eq!(
            fuse_register_block_instructions_with_costs(&instructions, &[usize::MAX, 1, 1, 1]),
            Err(RuntimeError::Overflow)
        );
    }
}
