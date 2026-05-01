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

#[test]
fn register_executor_runs_supported_program() {
    let mut code = Vec::new();
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x40);
    code.push(0x21);
    emit_u32(&mut code, 0);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(1)], 1);

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::DInt(41));

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    assert_eq!(runtime.storage().get_global("g0"), Some(&Value::DInt(42)));
}

#[test]
fn register_executor_profile_records_hot_blocks_for_supported_program() {
    let mut code = Vec::new();
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x40);
    code.push(0x21);
    emit_u32(&mut code, 0);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(1)], 1);

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::DInt(41));
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);

    let profile = runtime.vm_register_profile_snapshot();
    assert!(profile.enabled);
    assert_eq!(profile.register_programs_executed, 1);
    assert_eq!(profile.register_program_fallbacks, 0);
    assert!(
        profile
            .hot_blocks
            .iter()
            .any(|block| block.pou_id == pou_id && block.hits >= 1),
        "expected at least one hot block for executed POU",
    );
}

#[test]
fn register_executor_profile_records_dynamic_ref_and_instance_lookup_counters() {
    let mut code = Vec::new();
    code.push(0x22);
    emit_u32(&mut code, 0);
    code.push(0x30);
    emit_u32(&mut code, 0);
    code.push(0x32);
    code.push(0x21);
    emit_u32(&mut code, 1);
    code.push(0x22);
    emit_u32(&mut code, 0);
    code.push(0x30);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x33);
    code.push(0x23);
    code.push(0x30);
    emit_u32(&mut code, 1);
    code.push(0x32);
    code.push(0x21);
    emit_u32(&mut code, 2);
    code.push(0x23);
    code.push(0x30);
    emit_u32(&mut code, 1);
    code.push(0x10);
    emit_u32(&mut code, 1);
    code.push(0x33);
    code.push(0x06);

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
    let refs = vec![
        VmRef::Global {
            offset: 0,
            path: RefPath::new(),
        },
        VmRef::Global {
            offset: 1,
            path: RefPath::new(),
        },
        VmRef::Global {
            offset: 2,
            path: RefPath::new(),
        },
    ];
    let module = VmModule {
        code,
        strings: vec![SmolStr::new("VALUE"), SmolStr::new("ACC")],
        types: TypeTable::default(),
        refs,
        consts: vec![Value::DInt(11), Value::DInt(13)],
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

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global(
        "g0",
        Value::Struct(std::sync::Arc::new(StructValue::from_untyped_parts(
            SmolStr::new("CELL_T"),
            IndexMap::from([(SmolStr::new("VALUE"), Value::DInt(7))]),
        ))),
    );
    runtime.storage_mut().set_global("g1", Value::DInt(0));
    runtime.storage_mut().set_global("g2", Value::DInt(0));
    let instance_id = runtime.storage_mut().create_instance("COUNTER");
    assert!(runtime
        .storage_mut()
        .set_instance_var(instance_id, "ACC", Value::DInt(9)));
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let outcome =
        try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, Some(instance_id))
            .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    assert_eq!(runtime.storage().get_global("g1"), Some(&Value::DInt(7)));
    assert_eq!(runtime.storage().get_global("g2"), Some(&Value::DInt(9)));
    assert_eq!(
        runtime.storage().get_global("g0"),
        Some(&Value::Struct(std::sync::Arc::new(
            StructValue::from_untyped_parts(
                SmolStr::new("CELL_T"),
                IndexMap::from([(SmolStr::new("VALUE"), Value::DInt(11))]),
            )
        )))
    );
    assert_eq!(
        runtime.storage().get_instance_var(instance_id, "ACC"),
        Some(&Value::DInt(13))
    );

    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(profile.register_program_fallbacks, 0);
    assert_eq!(profile.ref_ops.load_ref, 0);
    assert_eq!(profile.ref_ops.store_ref, 2);
    assert_eq!(profile.ref_ops.load_ref_addr, 2);
    assert_eq!(profile.ref_ops.ref_field, 4);
    assert_eq!(profile.ref_ops.ref_index, 0);
    assert_eq!(profile.ref_ops.load_dynamic, 2);
    assert_eq!(profile.ref_ops.store_dynamic, 2);
    assert_eq!(profile.ref_ops.instance_field_lookups, 2);
    assert_eq!(profile.value_ops.read_value_clones, 0);
}

#[test]
fn register_executor_profile_records_function_block_call_counters() {
    let source = r#"
            FUNCTION_BLOCK Counter
            VAR_INPUT
                inc : BOOL;
            END_VAR
            VAR_OUTPUT
                value : INT;
            END_VAR

            IF inc THEN
                value := value + INT#1;
            END_IF;
            END_FUNCTION_BLOCK

            PROGRAM Main
            VAR
                fb : Counter;
                out_count : INT := INT#0;
            END_VAR
            fb(inc := TRUE, value => out_count);
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
    assert_eq!(harness.get_output("out_count"), Some(Value::Int(1)));

    let profile = harness.runtime().vm_register_profile_snapshot();
    assert_eq!(profile.register_program_fallbacks, 0);
    assert_eq!(profile.call_ops.function_block_call_entries, 1);
    assert_eq!(profile.call_ops.parameter_bindings, 2);
    assert_eq!(profile.call_ops.output_copy_backs, 1);
    assert!(profile.call_ops.frame_pushes >= 2);
    assert!(profile.call_ops.frame_pops >= 2);
    assert_eq!(profile.value_ops.binding_expr_clones, 0);
    assert_eq!(profile.value_ops.output_value_clones, 0);
}

#[test]
fn register_executor_profile_avoids_clone_counters_for_struct_inout_function_block() {
    let source = r#"
            TYPE AXIS_REF :
            STRUCT
                AxisId : UDINT;
                InternalIndex : UINT;
            END_STRUCT
            END_TYPE

            FUNCTION_BLOCK TouchAxis
            VAR_IN_OUT
                Axis : AXIS_REF;
            END_VAR
            VAR_OUTPUT
                Done : BOOL;
            END_VAR

            Axis.InternalIndex := Axis.InternalIndex + UINT#1;
            Done := TRUE;
            END_FUNCTION_BLOCK

            PROGRAM Main
            VAR
                Axis : AXIS_REF;
                Fb : TouchAxis;
            END_VAR

            Axis.AxisId := UDINT#1;
            Axis.InternalIndex := UINT#1;
            Fb(Axis := Axis);
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

    let profile = harness.runtime().vm_register_profile_snapshot();
    assert_eq!(profile.register_program_fallbacks, 0);
    assert!(profile.call_ops.function_block_call_entries >= 1);
    assert!(profile.call_ops.parameter_bindings >= 1);
    assert!(profile.call_ops.output_copy_backs >= 1);
    assert_eq!(
        profile.value_ops.read_value_clones, 0,
        "profile: {:?}",
        profile.value_ops
    );
    assert_eq!(
        profile.value_ops.output_value_clones, 0,
        "profile: {:?}",
        profile.value_ops
    );
}

#[test]
fn read_register_with_counts_records_clone_then_move_reads() {
    let mut profile = RegisterProfileState::default();
    profile.set_enabled(true);
    let mut registers = vec![Value::DInt(7)];
    let mut remaining = vec![2_u32];

    let first = read_register_with_counts(
        &mut profile,
        registers.as_mut_slice(),
        remaining.as_mut_slice(),
        RegisterId(0),
    )
    .expect("first read");
    let second = read_register_with_counts(
        &mut profile,
        registers.as_mut_slice(),
        remaining.as_mut_slice(),
        RegisterId(0),
    )
    .expect("second read");

    assert_eq!(first, Value::DInt(7));
    assert_eq!(second, Value::DInt(7));
    assert_eq!(registers[0], Value::Null);
    let snapshot = profile.snapshot();
    assert_eq!(snapshot.value_ops.register_read_clones, 1);
    assert_eq!(snapshot.value_ops.register_read_moves, 1);
}

#[test]
fn register_executor_falls_back_when_lowering_contains_unsupported_opcode() {
    let mut code = Vec::new();
    code.push(0x07);
    emit_u32(&mut code, 0);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, Vec::new(), 0);
    let mut runtime = Runtime::new();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("fallback decision");
    assert_eq!(outcome, RegisterExecutionOutcome::FallbackToStack);
}

#[test]
fn register_executor_profile_records_ref_op_counters_for_load_ref_store_ref_program() {
    let mut code = Vec::new();
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x21);
    emit_u32(&mut code, 1);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, Vec::new(), 2);

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::DInt(41));
    runtime.storage_mut().set_global("g1", Value::DInt(0));
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    assert_eq!(runtime.storage().get_global("g1"), Some(&Value::DInt(41)));

    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(profile.register_program_fallbacks, 0);
    assert_eq!(profile.ref_ops.load_ref, 1);
    assert_eq!(profile.ref_ops.store_ref, 1);
    assert_eq!(profile.ref_ops.load_ref_addr, 0);
    assert_eq!(profile.ref_ops.ref_field, 0);
    assert_eq!(profile.ref_ops.ref_index, 0);
    assert_eq!(profile.ref_ops.load_dynamic, 0);
    assert_eq!(profile.ref_ops.store_dynamic, 0);
    assert_eq!(profile.ref_ops.instance_field_lookups, 0);
    assert_eq!(profile.value_ops.read_value_clones, 0);
    assert_eq!(profile.value_ops.register_read_moves, 1);
    assert_eq!(profile.value_ops.register_read_clones, 0);
}

#[test]
fn register_executor_profile_avoids_clone_counter_for_scalar_load_const() {
    let mut code = Vec::new();
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x21);
    emit_u32(&mut code, 0);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(41)], 1);

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::DInt(0));
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    assert_eq!(runtime.storage().get_global("g0"), Some(&Value::DInt(41)));

    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(profile.register_program_fallbacks, 0);
    assert_eq!(profile.value_ops.const_load_clones, 0);
}

#[test]
fn register_executor_profile_avoids_clone_counters_for_borrowed_ref_const_binary_guard() {
    let mut code = Vec::new();
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x40);
    code.push(0x21);
    emit_u32(&mut code, 1);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(1)], 2);

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::DInt(41));
    runtime.storage_mut().set_global("g1", Value::DInt(0));
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    assert_eq!(runtime.storage().get_global("g1"), Some(&Value::DInt(42)));

    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(profile.register_program_fallbacks, 0);
    assert_eq!(profile.value_ops.read_value_clones, 0);
    assert_eq!(profile.value_ops.const_load_clones, 0);
}

#[test]
fn register_executor_profile_avoids_clone_counters_for_borrowed_ref_ref_non_dint_binary() {
    let mut code = Vec::new();
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x20);
    emit_u32(&mut code, 1);
    code.push(0x47);
    code.push(0x21);
    emit_u32(&mut code, 2);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, Vec::new(), 3);

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::Bool(false));
    runtime.storage_mut().set_global("g1", Value::Bool(true));
    runtime.storage_mut().set_global("g2", Value::Bool(false));
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    assert_eq!(runtime.storage().get_global("g2"), Some(&Value::Bool(true)));

    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(profile.register_program_fallbacks, 0);
    assert_eq!(profile.value_ops.read_value_clones, 0);
}

#[test]
fn register_executor_profile_avoids_clone_counters_for_borrowed_ref_const_non_dint_binary() {
    let mut code = Vec::new();
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x47);
    code.push(0x21);
    emit_u32(&mut code, 1);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, vec![Value::Bool(true)], 2);

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::Bool(false));
    runtime.storage_mut().set_global("g1", Value::Bool(false));
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    assert_eq!(runtime.storage().get_global("g1"), Some(&Value::Bool(true)));

    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(profile.register_program_fallbacks, 0);
    assert_eq!(profile.value_ops.read_value_clones, 0);
    assert_eq!(profile.value_ops.const_load_clones, 0);
}

#[test]
fn register_executor_profile_records_fallback_reason() {
    let mut code = Vec::new();
    code.push(0x07);
    emit_u32(&mut code, 0);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, Vec::new(), 0);
    let mut runtime = Runtime::new();
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("fallback decision");
    assert_eq!(outcome, RegisterExecutionOutcome::FallbackToStack);

    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(profile.register_programs_executed, 0);
    assert_eq!(profile.register_program_fallbacks, 1);
    assert!(
        profile
            .fallback_reasons
            .iter()
            .any(|entry| entry.reason.starts_with("unsupported_opcode") && entry.count == 1),
        "expected unsupported opcode fallback reason in profile snapshot",
    );
}

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

#[test]
fn register_lowering_cache_hits_after_first_execution() {
    let mut code = Vec::new();
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x40);
    code.push(0x21);
    emit_u32(&mut code, 0);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(1)], 1);

    let mut runtime = Runtime::new();
    runtime.set_vm_register_lowering_cache_enabled(true);
    runtime.reset_vm_register_lowering_cache();
    runtime.storage_mut().set_global("g0", Value::DInt(1));

    let first = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("first execution");
    let second = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("second execution");
    assert_eq!(first, RegisterExecutionOutcome::Executed);
    assert_eq!(second, RegisterExecutionOutcome::Executed);

    let snapshot = runtime.vm_register_lowering_cache_snapshot();
    assert!(snapshot.enabled);
    assert_eq!(snapshot.cached_entries, 1);
    assert_eq!(snapshot.misses, 1);
    assert_eq!(snapshot.hits, 1);
    assert_eq!(snapshot.build_errors, 0);
}

#[test]
fn register_lowering_cache_caches_lowering_errors() {
    let mut code = Vec::new();
    code.push(0x02);
    emit_i32(&mut code, 4096);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, Vec::new(), 0);

    let mut runtime = Runtime::new();
    runtime.set_vm_register_lowering_cache_enabled(true);
    runtime.reset_vm_register_lowering_cache();

    let first = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("first fallback");
    let second = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("second fallback");
    assert_eq!(first, RegisterExecutionOutcome::FallbackToStack);
    assert_eq!(second, RegisterExecutionOutcome::FallbackToStack);

    let snapshot = runtime.vm_register_lowering_cache_snapshot();
    assert!(snapshot.enabled);
    assert_eq!(snapshot.cached_entries, 1);
    assert_eq!(snapshot.misses, 1);
    assert_eq!(snapshot.hits, 1);
    assert_eq!(snapshot.build_errors, 1);
}

#[test]
fn register_executor_tier1_specialized_executor_keeps_startup_path_cold_until_hot_threshold() {
    let mut code = Vec::new();
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x40);
    code.push(0x21);
    emit_u32(&mut code, 0);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(1)], 1);

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::DInt(41));
    runtime.set_vm_tier1_specialized_executor_enabled(true);
    runtime.reset_vm_tier1_specialized_executor();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    let snapshot = runtime.vm_tier1_specialized_executor_snapshot();
    assert_eq!(snapshot.compile_attempts, 0);
    assert_eq!(snapshot.block_executions, 0);
}

#[test]
fn tier1_compiler_accepts_load_ref_addr_dynamic_block() {
    let mut code = Vec::new();
    code.push(0x22);
    emit_u32(&mut code, 0);
    code.push(0x32);
    code.push(0x21);
    emit_u32(&mut code, 1);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, Vec::new(), 2);

    let lowered = lower_pou_to_register_ir(&module, pou_id).expect("lower register ir");
    verify_register_program(&lowered).expect("verify register ir");
    let block = lowered
        .blocks
        .iter()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, RegisterInstr::LoadRefAddr { .. }))
                && block
                    .instructions
                    .iter()
                    .any(|instruction| matches!(instruction, RegisterInstr::LoadDynamic { .. }))
        })
        .expect("load-ref-addr block");
    let key = super::tier1_block_key(&module, pou_id, block);
    assert!(
        super::compile_tier1_block(&module, block, key).is_ok(),
        "expected tier-1 compiler to accept LoadRefAddr block: {:?}",
        block.instructions
    );
}

#[test]
fn register_executor_tier1_specialized_executor_executes_load_ref_addr_block() {
    let mut code = Vec::new();
    code.push(0x22);
    emit_u32(&mut code, 0);
    code.push(0x32);
    code.push(0x21);
    emit_u32(&mut code, 1);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, Vec::new(), 2);

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::DInt(41));
    runtime.storage_mut().set_global("g1", Value::DInt(0));
    runtime.set_vm_tier1_specialized_executor_enabled(true);
    runtime.reset_vm_tier1_specialized_executor();
    runtime.vm_tier1_specialized_executor.hot_block_threshold = 1;

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    assert_eq!(runtime.storage().get_global("g1"), Some(&Value::DInt(41)));

    let snapshot = runtime.vm_tier1_specialized_executor_snapshot();
    assert!(snapshot.compile_successes >= 1, "snapshot={snapshot:?}");
    assert!(snapshot.block_executions >= 1, "snapshot={snapshot:?}");
    assert_eq!(snapshot.compile_failures, 0, "snapshot={snapshot:?}");
}

#[test]
fn tier1_compiler_accepts_load_super_dynamic_block() {
    let mut code = Vec::new();
    code.push(0x24);
    code.push(0x30);
    emit_u32(&mut code, 0);
    code.push(0x32);
    code.push(0x21);
    emit_u32(&mut code, 0);
    code.push(0x06);

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
    let module = VmModule {
        code,
        strings: vec![SmolStr::new("COUNT")],
        types: TypeTable::default(),
        refs: vec![VmRef::Global {
            offset: 0,
            path: RefPath::new(),
        }],
        consts: Vec::new(),
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

    let lowered = lower_pou_to_register_ir(&module, pou_id).expect("lower register ir");
    verify_register_program(&lowered).expect("verify register ir");
    let block = lowered
        .blocks
        .iter()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, RegisterInstr::LoadSuper { .. }))
                && block
                    .instructions
                    .iter()
                    .any(|instruction| matches!(instruction, RegisterInstr::LoadDynamic { .. }))
        })
        .expect("load-super block");
    let key = super::tier1_block_key(&module, pou_id, block);
    assert!(
        super::compile_tier1_block(&module, block, key).is_ok(),
        "expected tier-1 compiler to accept LoadSuper block: {:?}",
        block.instructions
    );
}

#[test]
fn register_executor_tier1_specialized_executor_executes_load_super_block() {
    let mut code = Vec::new();
    code.push(0x24);
    code.push(0x30);
    emit_u32(&mut code, 0);
    code.push(0x32);
    code.push(0x21);
    emit_u32(&mut code, 0);
    code.push(0x06);

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
    let module = VmModule {
        code,
        strings: vec![SmolStr::new("COUNT")],
        types: TypeTable::default(),
        refs: vec![VmRef::Global {
            offset: 0,
            path: RefPath::new(),
        }],
        consts: Vec::new(),
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

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::DInt(0));
    let base = runtime.storage_mut().create_instance("BASE");
    let derived = runtime.storage_mut().create_instance("DERIVED");
    runtime
        .storage_mut()
        .get_instance_mut(derived)
        .expect("derived instance")
        .parent = Some(base);
    assert!(runtime
        .storage_mut()
        .set_instance_var(base, "COUNT", Value::DInt(10)));
    runtime.set_vm_tier1_specialized_executor_enabled(true);
    runtime.reset_vm_tier1_specialized_executor();
    runtime.vm_tier1_specialized_executor.hot_block_threshold = 1;
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, Some(derived))
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    assert_eq!(runtime.storage().get_global("g0"), Some(&Value::DInt(10)));

    let tier1 = runtime.vm_tier1_specialized_executor_snapshot();
    assert!(tier1.compile_successes >= 1, "snapshot={tier1:?}");
    assert!(tier1.block_executions >= 1, "snapshot={tier1:?}");
    assert_eq!(tier1.compile_failures, 0, "snapshot={tier1:?}");
    assert_eq!(tier1.deopt_count, 0, "snapshot={tier1:?}");

    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(profile.register_program_fallbacks, 0, "profile={profile:?}");
    assert_eq!(profile.ref_ops.load_dynamic, 1, "profile={profile:?}");
    assert_eq!(
        profile.ref_ops.instance_field_lookups, 1,
        "profile={profile:?}"
    );
}

#[test]
fn register_executor_tier1_specialized_executor_executes_bool_or_without_deopt() {
    let mut code = Vec::new();
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x47);
    code.push(0x21);
    emit_u32(&mut code, 0);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, vec![Value::Bool(true)], 1);

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::Bool(false));
    runtime.set_vm_tier1_specialized_executor_enabled(true);
    runtime.reset_vm_tier1_specialized_executor();
    runtime.vm_tier1_specialized_executor.hot_block_threshold = 1;
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    assert_eq!(runtime.storage().get_global("g0"), Some(&Value::Bool(true)));

    let snapshot = runtime.vm_tier1_specialized_executor_snapshot();
    assert!(snapshot.compile_successes >= 1, "snapshot={snapshot:?}");
    assert!(snapshot.block_executions >= 1, "snapshot={snapshot:?}");
    assert_eq!(snapshot.deopt_count, 0, "snapshot={snapshot:?}");
    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(
        profile.value_ops.read_value_clones, 0,
        "profile={profile:?}"
    );
    assert_eq!(
        profile.value_ops.const_load_clones, 0,
        "profile={profile:?}"
    );
}

#[test]
fn tier1_compiler_accepts_call_native_function_blocks() {
    let source = r#"
            VAR_GLOBAL
                g_value : DINT;
            END_VAR

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
            g_value := fb.Value;
            END_PROGRAM
        "#;

    let bytecode = bytecode_module_from_source(source).expect("compile bytecode");
    let vm_module = VmModule::from_bytecode(&bytecode).expect("decode vm module");
    let main_pou_id = vm_module
        .program_ids
        .get(&SmolStr::new("MAIN"))
        .copied()
        .expect("main pou id");
    let lowered = lower_pou_to_register_ir(&vm_module, main_pou_id).expect("lower register ir");
    verify_register_program(&lowered).expect("verify register ir");
    let block = lowered
        .blocks
        .iter()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, RegisterInstr::CallNative { .. }))
        })
        .expect("call-native block");
    let key = super::tier1_block_key(&vm_module, main_pou_id, block);
    assert!(
        super::compile_tier1_block(&vm_module, block, key).is_ok(),
        "expected tier-1 compiler to accept CallNative block: {:?}",
        block.instructions
    );
}

#[test]
fn register_executor_tier1_specialized_executor_executes_function_call_block() {
    let source = r#"
            VAR_GLOBAL
                g_value : DINT;
            END_VAR

            FUNCTION AddOne : DINT
            VAR_INPUT
                Input : DINT;
            END_VAR

            AddOne := Input + DINT#1;
            END_FUNCTION

            PROGRAM Main
            g_value := AddOne(Input := DINT#41);
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

    let result = harness.cycle();
    assert!(
        result.errors.is_empty(),
        "cycle errors: {:?}",
        result.errors
    );

    assert_eq!(harness.get_output("g_value"), Some(Value::DInt(42)));
    let snapshot = harness.runtime().vm_tier1_specialized_executor_snapshot();
    assert!(snapshot.compile_successes >= 1, "snapshot={snapshot:?}");
    assert!(snapshot.block_executions >= 1, "snapshot={snapshot:?}");
    assert_eq!(snapshot.compile_failures, 0, "snapshot={snapshot:?}");
}

#[test]
fn register_executor_tier1_specialized_executor_executes_function_block_call_block() {
    let source = r#"
            VAR_GLOBAL
                g_value : DINT;
            END_VAR

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
            g_value := fb.Value;
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
    assert!(snapshot.compile_successes >= 1, "snapshot={snapshot:?}");
    assert!(snapshot.block_executions >= 1, "snapshot={snapshot:?}");
    assert_eq!(snapshot.compile_failures, 0, "snapshot={snapshot:?}");
}

#[test]
fn register_executor_tier1_specialized_executor_records_compile_failure_reason_for_unsupported_instruction(
) {
    let mut code = Vec::new();
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x61);
    code.push(0x12);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(7)], 0);

    let mut runtime = Runtime::new();
    runtime.set_vm_tier1_specialized_executor_enabled(true);
    runtime.reset_vm_tier1_specialized_executor();
    runtime.vm_tier1_specialized_executor.hot_block_threshold = 1;

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);

    let snapshot = runtime.vm_tier1_specialized_executor_snapshot();
    assert_eq!(snapshot.compile_attempts, 1);
    assert_eq!(snapshot.compile_failures, 1);
    assert!(
            snapshot.compile_failure_reasons.iter().any(|entry| {
                entry.reason == "unsupported_instr:size_of_value" && entry.count >= 1
            }),
            "expected SizeOfValue compile failure reason in tier-1 specialized executor snapshot, got {snapshot:?}",
        );
}

#[test]
fn register_executor_tier1_specialized_executor_executes_non_dint_binary_without_deopt() {
    let mut code = Vec::new();
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x40);
    code.push(0x21);
    emit_u32(&mut code, 0);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, vec![Value::Int(1)], 1);

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::Int(0));
    runtime.set_vm_tier1_specialized_executor_enabled(true);
    runtime.reset_vm_tier1_specialized_executor();

    for _ in 0..80 {
        let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
            .expect("execute register program");
        assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    }

    assert_eq!(runtime.storage().get_global("g0"), Some(&Value::Int(80)));
    let snapshot = runtime.vm_tier1_specialized_executor_snapshot();
    assert!(snapshot.compile_attempts >= 1);
    assert!(snapshot.compile_successes >= 1);
    assert!(snapshot.block_executions >= 1);
    assert_eq!(snapshot.deopt_count, 0, "snapshot={snapshot:?}");
    assert!(snapshot.deopt_reasons.is_empty(), "snapshot={snapshot:?}");
}

#[test]
fn register_executor_tier1_specialized_executor_cache_capacity_evicts_old_blocks() {
    let mut code_a = Vec::new();
    code_a.push(0x20);
    emit_u32(&mut code_a, 0);
    code_a.push(0x10);
    emit_u32(&mut code_a, 0);
    code_a.push(0x40);
    code_a.push(0x21);
    emit_u32(&mut code_a, 0);
    code_a.push(0x06);
    let (module_a, pou_a) = manual_vm_module(code_a, vec![Value::DInt(1)], 1);

    let mut code_b = Vec::new();
    code_b.push(0x20);
    emit_u32(&mut code_b, 0);
    code_b.push(0x10);
    emit_u32(&mut code_b, 0);
    code_b.push(0x41);
    code_b.push(0x21);
    emit_u32(&mut code_b, 0);
    code_b.push(0x06);
    let (module_b, pou_b) = manual_vm_module(code_b, vec![Value::DInt(1)], 1);

    let mut runtime = Runtime::new();
    runtime.vm_tier1_specialized_executor.set_enabled(true);
    runtime.vm_tier1_specialized_executor.hot_block_threshold = 1;
    runtime.vm_tier1_specialized_executor.cache_capacity = 1;
    runtime.reset_vm_tier1_specialized_executor();
    runtime.vm_tier1_specialized_executor.hot_block_threshold = 1;
    runtime.vm_tier1_specialized_executor.cache_capacity = 1;

    runtime.storage_mut().set_global("g0", Value::DInt(10));
    try_execute_pou_with_register_ir(&mut runtime, &module_a, pou_a, None)
        .expect("execute module a");
    runtime.storage_mut().set_global("g0", Value::DInt(10));
    try_execute_pou_with_register_ir(&mut runtime, &module_b, pou_b, None)
        .expect("execute module b");

    let snapshot = runtime.vm_tier1_specialized_executor_snapshot();
    assert_eq!(snapshot.cached_blocks, 1);
    assert!(
        snapshot.cache_evictions >= 1,
        "expected at least one cache eviction with cap=1",
    );
}

#[test]
fn register_executor_tier1_specialized_executor_cache_hits_reuse_compiled_block_arc() {
    let key = super::tier1::Tier1BlockKey {
        module_ptr: 1,
        pou_id: 2,
        block_id: 3,
        start_pc: 4,
    };
    let compiled = std::sync::Arc::new(super::tier1::Tier1CompiledBlock {
        key,
        instructions: vec![super::tier1::Tier1CompiledInstr::Return],
    });
    let mut state = super::RegisterTier1SpecializedExecutorState::default();

    state.insert_compiled_block(std::sync::Arc::clone(&compiled));
    let fetched = state.compiled_block(&key).cloned().expect("compiled block");

    assert!(std::sync::Arc::ptr_eq(&compiled, &fetched));
}

#[test]
fn register_deadline_stride_checks_first_and_stride_boundaries() {
    assert!(super::should_check_register_deadline(0));
    assert!(!super::should_check_register_deadline(1));
    assert!(super::should_check_register_deadline(
        super::REGISTER_DEADLINE_CHECK_STRIDE
    ));
    assert!(super::should_check_register_deadline(
        super::REGISTER_DEADLINE_CHECK_STRIDE * 2
    ));
}

#[test]
fn register_execution_buffers_reuse_clears_frames_and_register_files() {
    super::VM_REGISTER_FRAME_STACK_POOL.with(|pool| pool.borrow_mut().clear());
    super::VM_REGISTER_FILE_POOL.with(|pool| pool.borrow_mut().clear());
    super::VM_REGISTER_READ_COUNTS_POOL.with(|pool| pool.borrow_mut().clear());

    {
        let mut buffers = super::RegisterExecutionBuffers::acquire(3);
        let (frames, registers, remaining_reads, _) = buffers.buffers_mut();
        frames
            .push(super::super::frames::VmFrame {
                pou_id: 1,
                return_pc: 2,
                code_start: 3,
                code_end: 4,
                local_ref_start: 0,
                local_ref_count: 1,
                locals: vec![Value::DInt(9)],
                runtime_instance: None,
                instance_owner: None,
            })
            .expect("push pooled frame");
        registers[0] = Value::DInt(7);
        remaining_reads[0] = 11;
    }

    let mut buffers = super::RegisterExecutionBuffers::acquire(3);
    let (frames, registers, remaining_reads, _) = buffers.buffers_mut();
    assert!(frames.is_empty());
    assert!(registers.iter().all(|value| matches!(value, Value::Null)));
    assert!(remaining_reads.iter().all(|count| *count == 0));
}

// ── P2 register-executor corpus diagnostic tests ──

#[test]
fn diagnostic_find_fallback_opcodes_in_corpus() {
    let fixtures: &[(&str, &str)] = &[
        (
            "call-binding",
            r#"
                FUNCTION Add : INT
                VAR_INPUT a : INT; b : INT := INT#2; END_VAR
                Add := a + b;
                END_FUNCTION
                FUNCTION Bump : INT
                VAR_IN_OUT x : INT; END_VAR
                VAR_INPUT inc : INT := INT#1; END_VAR
                x := x + inc; Bump := x;
                END_FUNCTION
                PROGRAM Main
                VAR v : INT := INT#10; out_named : INT := INT#0;
                    out_default : INT := INT#0; out_inout : INT := INT#0; END_VAR
                out_named := Add(b := INT#4, a := INT#3);
                out_default := Add(a := INT#3);
                out_inout := Bump(v, INT#5);
                END_PROGRAM
            "#,
        ),
        (
            "string-stdlib",
            r#"
                PROGRAM Main
                VAR out_left : STRING := ''; out_mid : STRING := '';
                    out_find_found : INT := INT#0; out_find_missing : INT := INT#0;
                    out_w_replace : WSTRING := ""; out_w_insert : WSTRING := ""; END_VAR
                out_left := LEFT(IN := 'ABCDE', L := INT#3);
                out_mid := MID(IN := 'ABCDE', L := INT#2, P := INT#2);
                out_find_found := FIND(IN1 := 'ABCDE', IN2 := 'BC');
                out_find_missing := FIND(IN1 := 'BC', IN2 := 'ABCDE');
                out_w_replace := REPLACE(IN1 := "ABCDE", IN2 := "Z", L := INT#2, P := INT#3);
                out_w_insert := INSERT(IN1 := "ABE", IN2 := "CD", P := INT#3);
                END_PROGRAM
            "#,
        ),
        (
            "refs-sizeof",
            r#"
                TYPE
                    Inner : STRUCT arr : ARRAY[0..2] OF INT; END_STRUCT;
                    Outer : STRUCT inner : Inner; END_STRUCT;
                END_TYPE
                PROGRAM Main
                VAR o : Outer; idx : INT := INT#1; value_cell : INT := INT#4;
                    r_value : REF_TO INT; r_outer : REF_TO Outer;
                    out_ref : INT := INT#0; out_after_write : INT := INT#0;
                    out_nested_chain : INT := INT#0; out_size_type_int : DINT := DINT#0; END_VAR
                r_value := REF(value_cell);
                r_outer := REF(o);
                out_ref := r_value^;
                r_value^ := r_value^ + INT#3;
                out_after_write := r_value^;
                out_nested_chain := r_outer^.inner.arr[idx];
                out_size_type_int := SIZEOF(INT);
                END_PROGRAM
            "#,
        ),
    ];

    for (name, source) in fixtures {
        let (vm_module, pou_id) = vm_module_and_main_pou(source);
        let lowered = lower_pou_to_register_ir(&vm_module, pou_id);
        match lowered {
            Err(e) => {
                panic!("fixture '{name}': lowering error: {e:?}");
            }
            Ok(program) => {
                let fallbacks: Vec<_> = program
                    .blocks
                    .iter()
                    .flat_map(|b| b.instructions.iter())
                    .filter_map(|i| match i {
                        RegisterInstr::VmFallback { opcode, .. } => Some(*opcode),
                        _ => None,
                    })
                    .collect();
                if !fallbacks.is_empty() {
                    let opcodes_hex: Vec<_> =
                        fallbacks.iter().map(|o| format!("0x{o:02X}")).collect();
                    panic!(
                        "fixture '{name}': has VmFallback instructions for opcodes: [{}]",
                        opcodes_hex.join(", ")
                    );
                }
                let has_complex = super::lowered_uses_complex_local_paths(&vm_module, &program);
                if has_complex {
                    // Find which ref indices are complex
                    let mut complex_refs = Vec::new();
                    for instr in program.blocks.iter().flat_map(|b| b.instructions.iter()) {
                        let ref_idx = match instr {
                            RegisterInstr::LoadRef { ref_idx, .. }
                            | RegisterInstr::LoadRefAddr { ref_idx, .. }
                            | RegisterInstr::StoreRef { ref_idx, .. } => *ref_idx,
                            _ => continue,
                        };
                        if let Some(VmRef::Local { path, .. }) =
                            vm_module.refs.get(ref_idx as usize)
                        {
                            if !path.is_empty() {
                                complex_refs.push(ref_idx);
                            }
                        }
                    }
                    panic!(
                            "fixture '{name}': blocked by complex_local_ref_path, ref indices: {complex_refs:?}"
                        );
                }
                eprintln!(
                    "fixture '{name}': PASS (no fallback instructions, no complex local refs)"
                );
            }
        }
    }
}

#[test]
fn diagnostic_execute_corpus_through_register_ir() {
    use crate::execution_backend::ExecutionBackend;
    use crate::harness::{bytecode_bytes_from_source, TestHarness};
    use crate::RestartMode;

    let fixtures: &[(&str, &str)] = &[
        (
            "call-binding",
            r#"
                FUNCTION Add : INT
                VAR_INPUT a : INT; b : INT := INT#2; END_VAR
                Add := a + b;
                END_FUNCTION
                FUNCTION Bump : INT
                VAR_IN_OUT x : INT; END_VAR
                VAR_INPUT inc : INT := INT#1; END_VAR
                x := x + inc; Bump := x;
                END_FUNCTION
                PROGRAM Main
                VAR v : INT := INT#10; out_named : INT := INT#0;
                    out_default : INT := INT#0; out_inout : INT := INT#0; END_VAR
                out_named := Add(b := INT#4, a := INT#3);
                out_default := Add(a := INT#3);
                out_inout := Bump(v, INT#5);
                END_PROGRAM
            "#,
        ),
        (
            "string-stdlib",
            r#"
                PROGRAM Main
                VAR out_left : STRING := ''; out_mid : STRING := '';
                    out_find_found : INT := INT#0; out_find_missing : INT := INT#0;
                    out_w_replace : WSTRING := ""; out_w_insert : WSTRING := ""; END_VAR
                out_left := LEFT(IN := 'ABCDE', L := INT#3);
                out_mid := MID(IN := 'ABCDE', L := INT#2, P := INT#2);
                out_find_found := FIND(IN1 := 'ABCDE', IN2 := 'BC');
                out_find_missing := FIND(IN1 := 'BC', IN2 := 'ABCDE');
                out_w_replace := REPLACE(IN1 := "ABCDE", IN2 := "Z", L := INT#2, P := INT#3);
                out_w_insert := INSERT(IN1 := "ABE", IN2 := "CD", P := INT#3);
                END_PROGRAM
            "#,
        ),
        (
            "refs-sizeof",
            r#"
                TYPE
                    Inner : STRUCT arr : ARRAY[0..2] OF INT; END_STRUCT;
                    Outer : STRUCT inner : Inner; END_STRUCT;
                END_TYPE
                PROGRAM Main
                VAR o : Outer; idx : INT := INT#1; value_cell : INT := INT#4;
                    r_value : REF_TO INT; r_outer : REF_TO Outer;
                    out_ref : INT := INT#0; out_after_write : INT := INT#0;
                    out_nested_chain : INT := INT#0; out_size_type_int : DINT := DINT#0; END_VAR
                r_value := REF(value_cell);
                r_outer := REF(o);
                out_ref := r_value^;
                r_value^ := r_value^ + INT#3;
                out_after_write := r_value^;
                out_nested_chain := r_outer^.inner.arr[idx];
                out_size_type_int := SIZEOF(INT);
                END_PROGRAM
            "#,
        ),
    ];

    for (name, source) in fixtures {
        let mut harness = TestHarness::from_source(source).expect("create harness");
        let bytes = bytecode_bytes_from_source(source).expect("compile bytecode");
        harness
            .runtime_mut()
            .apply_bytecode_bytes(&bytes, None)
            .expect("apply bytecode");
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
        if !result.errors.is_empty() {
            panic!("fixture '{name}': cycle errors: {:?}", result.errors);
        }

        let snapshot = harness.runtime().vm_register_profile_snapshot();
        eprintln!(
            "fixture '{name}': executed={}, fallbacks={}, reasons={:?}",
            snapshot.register_programs_executed,
            snapshot.register_program_fallbacks,
            snapshot.fallback_reasons,
        );
        assert!(
                snapshot.register_programs_executed > 0,
                "fixture '{name}': expected register execution, got 0 executed and {} fallbacks, reasons: {:?}",
                snapshot.register_program_fallbacks,
                snapshot.fallback_reasons,
            );
        assert_eq!(
            snapshot.register_program_fallbacks, 0,
            "fixture '{name}': expected zero register fallbacks, reasons: {:?}",
            snapshot.fallback_reasons
        );
    }
}

#[test]
fn diagnostic_register_ir_callee_path_populates_lowering_cache() {
    use crate::execution_backend::ExecutionBackend;
    use crate::harness::{bytecode_bytes_from_source, TestHarness};
    use crate::RestartMode;

    let source = r#"
            FUNCTION Add : INT
            VAR_INPUT
                a : INT;
                b : INT := INT#2;
            END_VAR
            Add := a + b;
            END_FUNCTION

            FUNCTION Bump : INT
            VAR_IN_OUT
                x : INT;
            END_VAR
            VAR_INPUT
                inc : INT := INT#1;
            END_VAR
            x := x + inc;
            Bump := x;
            END_FUNCTION

            PROGRAM Main
            VAR
                v : INT := INT#10;
                out_named : INT := INT#0;
                out_default : INT := INT#0;
                out_inout : INT := INT#0;
            END_VAR

            out_named := Add(b := INT#4, a := INT#3);
            out_default := Add(a := INT#3);
            out_inout := Bump(v, INT#5);
            END_PROGRAM
        "#;

    let mut harness = TestHarness::from_source(source).expect("create harness");
    let bytes = bytecode_bytes_from_source(source).expect("compile bytecode");
    harness
        .runtime_mut()
        .apply_bytecode_bytes(&bytes, None)
        .expect("apply bytecode");
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
        .set_vm_register_lowering_cache_enabled(true);
    harness.runtime_mut().reset_vm_register_lowering_cache();

    let first = harness.cycle();
    assert!(
        first.errors.is_empty(),
        "first cycle errors: {:?}",
        first.errors
    );
    let second = harness.cycle();
    assert!(
        second.errors.is_empty(),
        "second cycle errors: {:?}",
        second.errors
    );

    let cache = harness.runtime().vm_register_lowering_cache_snapshot();
    assert!(
        cache.cached_entries >= 2,
        "expected main + callee programs cached, got {} entries",
        cache.cached_entries
    );
    assert!(
        cache.hits > 0,
        "expected lowering-cache hits after second cycle, snapshot={cache:?}"
    );
}
