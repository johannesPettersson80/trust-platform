use super::*;

pub(in crate::runtime::vm::register_ir) fn compile_tier1_block(
    module: &VmModule,
    block: &RegisterBlock,
    key: Tier1BlockKey,
) -> Result<Tier1CompiledBlock, String> {
    let mut instructions = Vec::with_capacity(block.instructions.len());
    for instruction in &block.instructions {
        let compiled = match instruction {
            RegisterInstr::Nop => Tier1CompiledInstr::Nop,
            RegisterInstr::LoadConst { dest, const_idx } => {
                let value = module
                    .consts
                    .get(*const_idx as usize)
                    .cloned()
                    .ok_or_else(|| "invalid_const_idx".to_string())?;
                Tier1CompiledInstr::LoadConst { dest: *dest, value }
            }
            RegisterInstr::LoadNull { dest } => Tier1CompiledInstr::LoadNull { dest: *dest },
            RegisterInstr::LoadSelf { dest } => Tier1CompiledInstr::LoadSelf { dest: *dest },
            RegisterInstr::LoadSuper { dest } => Tier1CompiledInstr::LoadSuper { dest: *dest },
            RegisterInstr::LoadSelfFieldDynamic { field_idx, dest } => {
                let field = module
                    .strings
                    .get(*field_idx as usize)
                    .cloned()
                    .ok_or_else(|| format!("invalid_string_index:{field_idx}"))?;
                Tier1CompiledInstr::LoadSelfFieldDynamic { field, dest: *dest }
            }
            RegisterInstr::StoreSelfFieldDynamic { field_idx, value } => {
                let field = module
                    .strings
                    .get(*field_idx as usize)
                    .cloned()
                    .ok_or_else(|| format!("invalid_string_index:{field_idx}"))?;
                Tier1CompiledInstr::StoreSelfFieldDynamic {
                    field,
                    value: *value,
                }
            }
            RegisterInstr::Move { src, dest } => Tier1CompiledInstr::Move {
                src: *src,
                dest: *dest,
            },
            RegisterInstr::CallNative {
                kind,
                symbol_idx,
                args,
                dest,
            } => Tier1CompiledInstr::CallNative {
                kind: *kind,
                symbol_idx: *symbol_idx,
                args: args.clone().into_boxed_slice(),
                dest: *dest,
            },
            RegisterInstr::LoadRef { dest, ref_idx } => Tier1CompiledInstr::LoadRef {
                dest: *dest,
                ref_idx: *ref_idx,
            },
            RegisterInstr::LoadRefAddr { dest, ref_idx } => Tier1CompiledInstr::LoadRefAddr {
                dest: *dest,
                ref_idx: *ref_idx,
            },
            RegisterInstr::StoreRef { ref_idx, src } => Tier1CompiledInstr::StoreRef {
                ref_idx: *ref_idx,
                src: *src,
            },
            RegisterInstr::RefField {
                base,
                field_idx,
                dest,
            } => {
                let field = module
                    .strings
                    .get(*field_idx as usize)
                    .cloned()
                    .ok_or_else(|| "invalid_string_idx".to_string())?;
                Tier1CompiledInstr::RefField {
                    base: *base,
                    field,
                    dest: *dest,
                }
            }
            RegisterInstr::RefIndex { base, index, dest } => Tier1CompiledInstr::RefIndex {
                base: *base,
                index: *index,
                dest: *dest,
            },
            RegisterInstr::LoadDynamic { reference, dest } => Tier1CompiledInstr::LoadDynamic {
                reference: *reference,
                dest: *dest,
            },
            RegisterInstr::StoreDynamic { reference, value } => Tier1CompiledInstr::StoreDynamic {
                reference: *reference,
                value: *value,
            },
            RegisterInstr::Unary { op, src, dest } => Tier1CompiledInstr::Unary {
                op: *op,
                src: *src,
                dest: *dest,
            },
            RegisterInstr::Binary {
                op,
                left,
                right,
                dest,
            } => Tier1CompiledInstr::BinaryDIntGuard {
                op: *op,
                left: *left,
                right: *right,
                dest: *dest,
            },
            RegisterInstr::BinaryRefToRef {
                op,
                left_ref_idx,
                right_ref_idx,
                dest_ref_idx,
            } => Tier1CompiledInstr::BinaryRefToRefDIntGuard {
                op: *op,
                left_ref_idx: *left_ref_idx,
                right_ref_idx: *right_ref_idx,
                dest_ref_idx: *dest_ref_idx,
            },
            RegisterInstr::BinaryRefConstToRef {
                op,
                left_ref_idx,
                const_idx,
                dest_ref_idx,
            } => Tier1CompiledInstr::BinaryRefConstToRefDIntGuard {
                op: *op,
                left_ref_idx: *left_ref_idx,
                const_idx: *const_idx,
                dest_ref_idx: *dest_ref_idx,
            },
            RegisterInstr::BinaryConstRefToRef {
                op,
                const_idx,
                right_ref_idx,
                dest_ref_idx,
            } => Tier1CompiledInstr::BinaryConstRefToRefDIntGuard {
                op: *op,
                const_idx: *const_idx,
                right_ref_idx: *right_ref_idx,
                dest_ref_idx: *dest_ref_idx,
            },
            RegisterInstr::CmpRefConstJumpIf {
                op,
                ref_idx,
                const_idx,
                jump_if_true,
                target,
            } => {
                if !is_cmp_binary_op(*op) {
                    return Err(format!("unsupported_cmp_op:{op:?}").to_ascii_lowercase());
                }
                Tier1CompiledInstr::CmpRefConstJumpIfDIntGuard {
                    op: *op,
                    ref_idx: *ref_idx,
                    const_idx: *const_idx,
                    jump_if_true: *jump_if_true,
                    target: *target,
                }
            }
            RegisterInstr::Jump { target } => Tier1CompiledInstr::Jump { target: *target },
            RegisterInstr::JumpIf {
                cond,
                jump_if_true,
                target,
            } => Tier1CompiledInstr::JumpIf {
                cond: *cond,
                jump_if_true: *jump_if_true,
                target: *target,
            },
            RegisterInstr::Return => Tier1CompiledInstr::Return,
            RegisterInstr::SizeOfType { .. } => {
                return Err("unsupported_instr:size_of_type".to_string())
            }
            RegisterInstr::SizeOfValue { .. } => {
                return Err("unsupported_instr:size_of_value".to_string())
            }
            RegisterInstr::VmFallback { opcode, .. } => {
                return Err(format!(
                    "unsupported_instr:vm_fallback_opcode_{opcode:#04x}"
                ));
            }
        };
        instructions.push(compiled);
    }

    Ok(Tier1CompiledBlock { key, instructions })
}
