use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::bundle_builder::collect_project_source_files;
use crate::bytecode::{SectionData, SectionId, TypeTable};
use crate::config::RuntimeConfig;
use crate::error::RuntimeError;
use crate::execution_backend::ExecutionBackend;
use crate::harness::{bytecode_module_from_source, CompileSession, TestHarness};
use crate::program_model::{apply_binary, apply_unary, BinaryOp, UnaryOp};
use crate::value::{DateTimeProfile, RefPath, RefSegment, StructValue, Value};
use crate::{RestartMode, Runtime};

use super::super::{VmPouEntry, VmRef};
use super::{
    invalid_bytecode, lower_pou_to_register_ir, read_register_with_counts,
    try_execute_pou_with_register_ir, try_execute_pou_with_register_ir_with_locals,
    verify_register_program, BlockTarget, RegisterExecutionOutcome, RegisterId, RegisterInstr,
    RegisterProfileState, RegisterProgram, VmModule,
};

fn vm_module_and_main_pou(source: &str) -> (VmModule, u32) {
    let bytecode = bytecode_module_from_source(source).expect("compile bytecode");
    let vm_module = VmModule::from_bytecode(&bytecode).expect("decode vm module");
    let main_key = SmolStr::new("MAIN");
    let pou_id = vm_module
        .program_ids
        .get(&main_key)
        .copied()
        .expect("main pou id");
    (vm_module, pou_id)
}

fn manual_vm_module(code: Vec<u8>, consts: Vec<Value>, ref_count: usize) -> (VmModule, u32) {
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

    let refs = (0..ref_count)
        .map(|offset| VmRef::Global {
            offset,
            path: RefPath::new(),
        })
        .collect();

    (
        VmModule {
            code,
            strings: Vec::new(),
            types: TypeTable::default(),
            refs,
            consts,
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
        },
        pou_id,
    )
}

fn emit_u32(code: &mut Vec<u8>, value: u32) {
    code.extend_from_slice(&value.to_le_bytes());
}

fn emit_i32(code: &mut Vec<u8>, value: i32) {
    code.extend_from_slice(&value.to_le_bytes());
}

fn patch_i32(code: &mut [u8], operand_start: usize, value: i32) {
    let bytes = value.to_le_bytes();
    code[operand_start..operand_start + 4].copy_from_slice(&bytes);
}

fn read_u32_operand(
    code: &[u8],
    pc: &mut usize,
    code_end: usize,
    opcode: u8,
) -> Result<u32, RuntimeError> {
    if *pc + 4 > code_end {
        return Err(invalid_bytecode(format!(
            "parity stack executor operand overflow for opcode 0x{opcode:02X}",
        )));
    }
    let bytes = [code[*pc], code[*pc + 1], code[*pc + 2], code[*pc + 3]];
    *pc += 4;
    Ok(u32::from_le_bytes(bytes))
}

fn read_i32_operand(
    code: &[u8],
    pc: &mut usize,
    code_end: usize,
    opcode: u8,
) -> Result<i32, RuntimeError> {
    if *pc + 4 > code_end {
        return Err(invalid_bytecode(format!(
            "parity stack executor operand overflow for opcode 0x{opcode:02X}",
        )));
    }
    let bytes = [code[*pc], code[*pc + 1], code[*pc + 2], code[*pc + 3]];
    *pc += 4;
    Ok(i32::from_le_bytes(bytes))
}

fn pop_stack_value(stack: &mut Vec<Value>, opcode: u8) -> Result<Value, RuntimeError> {
    stack.pop().ok_or_else(|| {
        invalid_bytecode(format!(
            "parity stack executor stack underflow on opcode 0x{opcode:02X}",
        ))
    })
}

fn pop_bool_condition(stack: &mut Vec<Value>) -> Result<bool, RuntimeError> {
    match stack.pop() {
        Some(Value::Bool(value)) => Ok(value),
        Some(_) => Err(RuntimeError::TypeMismatch),
        None => Err(invalid_bytecode(
            "parity stack executor stack underflow on conditional jump",
        )),
    }
}

fn jump_target_within(
    pc_after_operand: usize,
    offset: i32,
    code_start: usize,
    code_end: usize,
) -> Result<usize, RuntimeError> {
    let target = (pc_after_operand as i64) + i64::from(offset);
    if target < code_start as i64 || target > code_end as i64 {
        return Err(invalid_bytecode(format!(
            "parity stack executor invalid jump target {target}",
        )));
    }
    Ok(target as usize)
}

fn execute_stack_subset(
    module: &VmModule,
    pou_id: u32,
    refs: &mut [Value],
) -> Result<(), RuntimeError> {
    let pou = module.pou(pou_id).ok_or_else(|| {
        invalid_bytecode(format!(
            "missing pou id {pou_id} for parity stack execution"
        ))
    })?;
    let mut stack = Vec::new();
    let mut pc = pou.code_start;
    let mut budget = 10_000_usize;
    let profile = DateTimeProfile::default();

    while pc < pou.code_end {
        if budget == 0 {
            return Err(invalid_bytecode(
                "parity stack executor budget exceeded (possible infinite loop)",
            ));
        }
        budget = budget.saturating_sub(1);

        let opcode = module.code[pc];
        pc += 1;
        match opcode {
            0x00 => {}
            0x02 => {
                let offset = read_i32_operand(&module.code, &mut pc, pou.code_end, opcode)?;
                pc = jump_target_within(pc, offset, pou.code_start, pou.code_end)?;
            }
            0x03 | 0x04 => {
                let offset = read_i32_operand(&module.code, &mut pc, pou.code_end, opcode)?;
                let condition = pop_bool_condition(&mut stack)?;
                let should_jump = (opcode == 0x03 && condition) || (opcode == 0x04 && !condition);
                if should_jump {
                    pc = jump_target_within(pc, offset, pou.code_start, pou.code_end)?;
                }
            }
            0x06 => return Ok(()),
            0x10 => {
                let const_idx = read_u32_operand(&module.code, &mut pc, pou.code_end, opcode)?;
                let value = module
                    .consts
                    .get(const_idx as usize)
                    .cloned()
                    .ok_or_else(|| invalid_bytecode(format!("invalid const index {const_idx}")))?;
                stack.push(value);
            }
            0x11 => {
                let value = stack.last().cloned().ok_or_else(|| {
                    invalid_bytecode("parity stack executor stack underflow on DUP")
                })?;
                stack.push(value);
            }
            0x12 => {
                let _ = pop_stack_value(&mut stack, opcode)?;
            }
            0x13 => {
                if stack.len() < 2 {
                    return Err(invalid_bytecode(
                        "parity stack executor stack underflow on SWAP",
                    ));
                }
                let len = stack.len();
                stack.swap(len - 1, len - 2);
            }
            0x20 => {
                let ref_idx = read_u32_operand(&module.code, &mut pc, pou.code_end, opcode)?;
                let value = refs
                    .get(ref_idx as usize)
                    .cloned()
                    .ok_or_else(|| invalid_bytecode(format!("invalid ref index {ref_idx}")))?;
                stack.push(value);
            }
            0x21 => {
                let ref_idx = read_u32_operand(&module.code, &mut pc, pou.code_end, opcode)?;
                let value = pop_stack_value(&mut stack, opcode)?;
                let slot = refs
                    .get_mut(ref_idx as usize)
                    .ok_or_else(|| invalid_bytecode(format!("invalid ref index {ref_idx}")))?;
                *slot = value;
            }
            0x40..=0x55 => {
                let op = match opcode {
                    0x40 => BinaryOp::Add,
                    0x41 => BinaryOp::Sub,
                    0x42 => BinaryOp::Mul,
                    0x43 => BinaryOp::Div,
                    0x44 => BinaryOp::Mod,
                    0x46 => BinaryOp::And,
                    0x47 => BinaryOp::Or,
                    0x48 => BinaryOp::Xor,
                    0x50 => BinaryOp::Eq,
                    0x51 => BinaryOp::Ne,
                    0x52 => BinaryOp::Lt,
                    0x53 => BinaryOp::Le,
                    0x54 => BinaryOp::Gt,
                    0x55 => BinaryOp::Ge,
                    _ => {
                        let unary = match opcode {
                            0x45 => UnaryOp::Neg,
                            0x49 => UnaryOp::Not,
                            _ => {
                                return Err(invalid_bytecode(format!(
                                    "unsupported opcode 0x{opcode:02X} in parity stack executor",
                                )))
                            }
                        };
                        let value = pop_stack_value(&mut stack, opcode)?;
                        stack.push(apply_unary(unary, value)?);
                        continue;
                    }
                };
                let right = pop_stack_value(&mut stack, opcode)?;
                let left = pop_stack_value(&mut stack, opcode)?;
                let result = apply_binary(op, left, right, &profile)?;
                stack.push(result);
            }
            _ => {
                return Err(invalid_bytecode(format!(
                    "unsupported opcode 0x{opcode:02X} in parity stack executor",
                )));
            }
        }
    }

    Ok(())
}

fn read_register_value(registers: &[Value], register: RegisterId) -> Result<Value, RuntimeError> {
    registers
        .get(register.index() as usize)
        .cloned()
        .ok_or_else(|| {
            invalid_bytecode(format!(
                "parity register executor read out-of-bounds register {}",
                register.index()
            ))
        })
}

fn write_register_value(
    registers: &mut [Value],
    register: RegisterId,
    value: Value,
) -> Result<(), RuntimeError> {
    let slot = registers
        .get_mut(register.index() as usize)
        .ok_or_else(|| {
            invalid_bytecode(format!(
                "parity register executor write out-of-bounds register {}",
                register.index()
            ))
        })?;
    *slot = value;
    Ok(())
}

fn execute_register_subset(
    module: &VmModule,
    program: &RegisterProgram,
    refs: &mut [Value],
) -> Result<(), RuntimeError> {
    let mut registers = vec![Value::Null; program.max_registers as usize];
    let mut current_block = program.entry_block;
    let mut budget = 10_000_usize;
    let mut block_to_index = HashMap::new();
    for (index, block) in program.blocks.iter().enumerate() {
        block_to_index.insert(block.id, index);
    }
    let profile = DateTimeProfile::default();

    loop {
        if budget == 0 {
            return Err(invalid_bytecode(
                "parity register executor budget exceeded (possible infinite loop)",
            ));
        }
        budget = budget.saturating_sub(1);
        let block_index = block_to_index.get(&current_block).copied().ok_or_else(|| {
            invalid_bytecode(format!(
                "parity register executor missing block {current_block}"
            ))
        })?;
        let block = &program.blocks[block_index];
        let mut control_target = None;

        for instruction in &block.instructions {
            match instruction {
                RegisterInstr::Nop => {}
                RegisterInstr::LoadConst { dest, const_idx } => {
                    let value =
                        module
                            .consts
                            .get(*const_idx as usize)
                            .cloned()
                            .ok_or_else(|| {
                                invalid_bytecode(format!(
                                    "parity register executor invalid const index {const_idx}",
                                ))
                            })?;
                    write_register_value(&mut registers, *dest, value)?;
                }
                RegisterInstr::LoadNull { dest } => {
                    write_register_value(&mut registers, *dest, Value::Null)?;
                }
                RegisterInstr::LoadSelf { .. } => {
                    return Err(invalid_bytecode(
                        "parity register executor does not support LOAD_SELF",
                    ));
                }
                RegisterInstr::LoadSuper { .. } => {
                    return Err(invalid_bytecode(
                        "parity register executor does not support LOAD_SUPER",
                    ));
                }
                RegisterInstr::Move { src, dest } => {
                    let value = read_register_value(&registers, *src)?;
                    write_register_value(&mut registers, *dest, value)?;
                }
                RegisterInstr::LoadRef { dest, ref_idx } => {
                    let value = refs.get(*ref_idx as usize).cloned().ok_or_else(|| {
                        invalid_bytecode(format!(
                            "parity register executor invalid ref index {ref_idx}",
                        ))
                    })?;
                    write_register_value(&mut registers, *dest, value)?;
                }
                RegisterInstr::LoadRefAddr { .. } => {
                    return Err(invalid_bytecode(
                        "parity register executor does not support LOAD_REF_ADDR",
                    ));
                }
                RegisterInstr::StoreRef { ref_idx, src } => {
                    let value = read_register_value(&registers, *src)?;
                    let slot = refs.get_mut(*ref_idx as usize).ok_or_else(|| {
                        invalid_bytecode(format!(
                            "parity register executor invalid ref index {ref_idx}",
                        ))
                    })?;
                    *slot = value;
                }
                RegisterInstr::Unary { op, src, dest } => {
                    let src = read_register_value(&registers, *src)?;
                    let result = apply_unary(*op, src)?;
                    write_register_value(&mut registers, *dest, result)?;
                }
                RegisterInstr::Binary {
                    op,
                    left,
                    right,
                    dest,
                } => {
                    let left = read_register_value(&registers, *left)?;
                    let right = read_register_value(&registers, *right)?;
                    let result = apply_binary(*op, left, right, &profile)?;
                    write_register_value(&mut registers, *dest, result)?;
                }
                RegisterInstr::BinaryRefToRef {
                    op,
                    left_ref_idx,
                    right_ref_idx,
                    dest_ref_idx,
                } => {
                    let left = refs.get(*left_ref_idx as usize).cloned().ok_or_else(|| {
                        invalid_bytecode(format!(
                            "parity register executor invalid ref index {left_ref_idx}",
                        ))
                    })?;
                    let right = refs.get(*right_ref_idx as usize).cloned().ok_or_else(|| {
                        invalid_bytecode(format!(
                            "parity register executor invalid ref index {right_ref_idx}",
                        ))
                    })?;
                    let result = apply_binary(*op, left, right, &profile)?;
                    let slot = refs.get_mut(*dest_ref_idx as usize).ok_or_else(|| {
                        invalid_bytecode(format!(
                            "parity register executor invalid ref index {dest_ref_idx}",
                        ))
                    })?;
                    *slot = result;
                }
                RegisterInstr::BinaryRefConstToRef {
                    op,
                    left_ref_idx,
                    const_idx,
                    dest_ref_idx,
                } => {
                    let left = refs.get(*left_ref_idx as usize).cloned().ok_or_else(|| {
                        invalid_bytecode(format!(
                            "parity register executor invalid ref index {left_ref_idx}",
                        ))
                    })?;
                    let right =
                        module
                            .consts
                            .get(*const_idx as usize)
                            .cloned()
                            .ok_or_else(|| {
                                invalid_bytecode(format!(
                                    "parity register executor invalid const index {const_idx}",
                                ))
                            })?;
                    let result = apply_binary(*op, left, right, &profile)?;
                    let slot = refs.get_mut(*dest_ref_idx as usize).ok_or_else(|| {
                        invalid_bytecode(format!(
                            "parity register executor invalid ref index {dest_ref_idx}",
                        ))
                    })?;
                    *slot = result;
                }
                RegisterInstr::BinaryConstRefToRef {
                    op,
                    const_idx,
                    right_ref_idx,
                    dest_ref_idx,
                } => {
                    let left =
                        module
                            .consts
                            .get(*const_idx as usize)
                            .cloned()
                            .ok_or_else(|| {
                                invalid_bytecode(format!(
                                    "parity register executor invalid const index {const_idx}",
                                ))
                            })?;
                    let right = refs.get(*right_ref_idx as usize).cloned().ok_or_else(|| {
                        invalid_bytecode(format!(
                            "parity register executor invalid ref index {right_ref_idx}",
                        ))
                    })?;
                    let result = apply_binary(*op, left, right, &profile)?;
                    let slot = refs.get_mut(*dest_ref_idx as usize).ok_or_else(|| {
                        invalid_bytecode(format!(
                            "parity register executor invalid ref index {dest_ref_idx}",
                        ))
                    })?;
                    *slot = result;
                }
                RegisterInstr::CmpRefConstJumpIf {
                    op,
                    ref_idx,
                    const_idx,
                    jump_if_true,
                    target,
                } => {
                    let left = refs.get(*ref_idx as usize).cloned().ok_or_else(|| {
                        invalid_bytecode(format!(
                            "parity register executor invalid ref index {ref_idx}",
                        ))
                    })?;
                    let right =
                        module
                            .consts
                            .get(*const_idx as usize)
                            .cloned()
                            .ok_or_else(|| {
                                invalid_bytecode(format!(
                                    "parity register executor invalid const index {const_idx}",
                                ))
                            })?;
                    let result = apply_binary(*op, left, right, &profile)?;
                    let condition = match result {
                        Value::Bool(value) => value,
                        _ => return Err(RuntimeError::TypeMismatch),
                    };
                    if condition == *jump_if_true {
                        control_target = Some(*target);
                        break;
                    }
                }
                RegisterInstr::CallNative { .. }
                | RegisterInstr::SizeOfType { .. }
                | RegisterInstr::SizeOfValue { .. }
                | RegisterInstr::RefField { .. }
                | RegisterInstr::RefIndex { .. }
                | RegisterInstr::LoadDynamic { .. }
                | RegisterInstr::StoreDynamic { .. }
                | RegisterInstr::LoadSelfFieldDynamic { .. }
                | RegisterInstr::StoreSelfFieldDynamic { .. } => {
                    return Err(invalid_bytecode(
                            "parity register executor does not support native-call/sizeof/dynamic-ref ops",
                        ));
                }
                RegisterInstr::Jump { target } => {
                    control_target = Some(*target);
                    break;
                }
                RegisterInstr::JumpIf {
                    cond,
                    jump_if_true,
                    target,
                } => {
                    let cond = read_register_value(&registers, *cond)?;
                    let cond = match cond {
                        Value::Bool(value) => value,
                        _ => return Err(RuntimeError::TypeMismatch),
                    };
                    if cond == *jump_if_true {
                        control_target = Some(*target);
                        break;
                    }
                }
                RegisterInstr::Return => return Ok(()),
                RegisterInstr::VmFallback { opcode, .. } => {
                    return Err(invalid_bytecode(format!(
                        "parity register executor encountered fallback opcode 0x{opcode:02X}",
                    )));
                }
            }
        }

        match control_target {
            Some(BlockTarget::Block(next)) => current_block = next,
            Some(BlockTarget::Exit) => return Ok(()),
            None => {
                if let Some(next_block) = program.blocks.get(block_index + 1) {
                    current_block = next_block.id;
                } else {
                    return Ok(());
                }
            }
        }
    }
}

fn assert_no_fallback(program: &RegisterProgram) {
    assert!(
        program
            .blocks
            .iter()
            .flat_map(|block| block.instructions.iter())
            .all(|instruction| !matches!(instruction, RegisterInstr::VmFallback { .. })),
        "parity program unexpectedly lowered unsupported opcodes to VmFallback",
    );
}
