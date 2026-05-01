use super::*;

pub(in crate::runtime::vm::register_ir) fn verify_register_program(
    program: &RegisterProgram,
) -> Result<(), RuntimeError> {
    if program.blocks.is_empty() {
        return Err(invalid_bytecode("register-ir program has no blocks"));
    }
    let known_blocks = program
        .blocks
        .iter()
        .map(|block| block.id)
        .collect::<HashSet<_>>();
    if !known_blocks.contains(&program.entry_block) {
        return Err(invalid_bytecode(format!(
            "register-ir entry block {} missing",
            program.entry_block
        )));
    }

    for block in &program.blocks {
        let mut defined = (0..block.entry_stack_depth)
            .map(RegisterId)
            .collect::<BTreeSet<_>>();
        for instr in &block.instructions {
            match instr {
                RegisterInstr::LoadConst { dest, .. }
                | RegisterInstr::LoadRef { dest, .. }
                | RegisterInstr::LoadRefAddr { dest, .. }
                | RegisterInstr::LoadNull { dest }
                | RegisterInstr::LoadSelf { dest }
                | RegisterInstr::LoadSuper { dest }
                | RegisterInstr::LoadSelfFieldDynamic { dest, .. }
                | RegisterInstr::SizeOfType { dest, .. } => {
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::Move { src, dest } => {
                    verify_src(src, &defined)?;
                    verify_move_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::SizeOfValue { src, dest } => {
                    verify_src(src, &defined)?;
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::RefField { base, dest, .. } => {
                    verify_src(base, &defined)?;
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::RefIndex {
                    base, index, dest, ..
                } => {
                    verify_src(base, &defined)?;
                    verify_src(index, &defined)?;
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::LoadDynamic { reference, dest } => {
                    verify_src(reference, &defined)?;
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::StoreDynamic { reference, value } => {
                    verify_src(reference, &defined)?;
                    verify_src(value, &defined)?;
                }
                RegisterInstr::StoreSelfFieldDynamic { value, .. } => {
                    verify_src(value, &defined)?;
                }
                RegisterInstr::CallNative { args, dest, .. } => {
                    for arg in args {
                        verify_src(arg, &defined)?;
                    }
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::Unary { src, dest, .. } => {
                    verify_src(src, &defined)?;
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::Binary {
                    left, right, dest, ..
                } => {
                    verify_src(left, &defined)?;
                    verify_src(right, &defined)?;
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::CmpRefConstJumpIf { target, .. } => {
                    verify_target(target, &known_blocks)?;
                }
                RegisterInstr::StoreRef { src, .. } => {
                    verify_src(src, &defined)?;
                }
                RegisterInstr::Jump { target } => verify_target(target, &known_blocks)?,
                RegisterInstr::JumpIf { cond, target, .. } => {
                    verify_src(cond, &defined)?;
                    verify_target(target, &known_blocks)?;
                }
                RegisterInstr::BinaryRefToRef { .. }
                | RegisterInstr::BinaryRefConstToRef { .. }
                | RegisterInstr::BinaryConstRefToRef { .. }
                | RegisterInstr::Nop
                | RegisterInstr::Return
                | RegisterInstr::VmFallback { .. } => {}
            }
        }
    }

    Ok(())
}

fn verify_dest(
    dest: &RegisterId,
    max_registers: u32,
    defined: &mut BTreeSet<RegisterId>,
) -> Result<(), RuntimeError> {
    if dest.index() >= max_registers {
        return Err(invalid_bytecode(format!(
            "register-ir destination register {} out of bounds (max={max_registers})",
            dest.index()
        )));
    }
    if !defined.insert(*dest) {
        return Err(invalid_bytecode(format!(
            "register-ir destination register {} redefined in block",
            dest.index()
        )));
    }
    Ok(())
}

fn verify_move_dest(
    dest: &RegisterId,
    max_registers: u32,
    defined: &mut BTreeSet<RegisterId>,
) -> Result<(), RuntimeError> {
    if dest.index() >= max_registers {
        return Err(invalid_bytecode(format!(
            "register-ir destination register {} out of bounds (max={max_registers})",
            dest.index()
        )));
    }
    defined.insert(*dest);
    Ok(())
}

fn verify_src(src: &RegisterId, defined: &BTreeSet<RegisterId>) -> Result<(), RuntimeError> {
    if !defined.contains(src) {
        return Err(invalid_bytecode(format!(
            "register-ir source register {} used before definition",
            src.index()
        )));
    }
    Ok(())
}

fn verify_target(target: &BlockTarget, known_blocks: &HashSet<u32>) -> Result<(), RuntimeError> {
    if let BlockTarget::Block(id) = target {
        if !known_blocks.contains(id) {
            return Err(invalid_bytecode(format!(
                "register-ir unknown block target {id}",
            )));
        }
    }
    Ok(())
}
