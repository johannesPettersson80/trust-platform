use super::*;

pub(super) struct NativeSymbolArgs<'a> {
    target_name: &'a str,
    pub(super) args: Vec<NativeArgShape<'a>>,
}

pub(super) struct NativeArgShape<'a> {
    name: Option<&'a str>,
    pub(super) is_target: bool,
}

pub(super) fn validate_param_direction_metadata(
    entry: &PouEntry,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    for param in &entry.params {
        budget.work(1)?;
        if !matches!(param.direction, 0..=2) {
            return Err(BytecodeError::InvalidSection(
                format!("invalid parameter direction {}", param.direction).into(),
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_param_direction_calls(
    tables: &ValidationContext<'_>,
    pou: Option<&PouEntry>,
    instructions: &[Instruction],
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    // Ownerless initializer bodies cannot call user POUs; visibility admission
    // rejects those calls before this signature pass.
    let Some(pou) = pou else {
        return Ok(());
    };
    for instruction in instructions {
        budget.work(1)?;
        if instruction.opcode == 0x09 {
            budget.temporary(|budget| {
                validate_native_call_param_directions(
                    tables,
                    pou,
                    instruction.operand(0),
                    instruction.operand(1),
                    budget,
                )
            })?;
        }
    }
    Ok(())
}

fn validate_native_call_param_directions(
    tables: &ValidationContext<'_>,
    pou: &PouEntry,
    kind: u32,
    symbol_idx: u32,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let Some(symbol) = tables.strings.entries.get(symbol_idx as usize) else {
        return Ok(());
    };
    let Some(symbol_args) = parse_native_symbol_args(symbol.as_str(), budget)? else {
        return Ok(());
    };
    let Some(callee_id) =
        resolve_call_callee_pou_id(tables, pou, kind, symbol_args.target_name, budget)?
    else {
        return Ok(());
    };
    let Some(callee) = tables.pou(callee_id, budget)? else {
        return Ok(());
    };
    validate_call_arg_shapes(tables.strings, callee, &symbol_args.args, budget)
}

fn validate_call_arg_shapes(
    strings: &StringTable,
    callee: &PouEntry,
    args: &[NativeArgShape<'_>],
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    budget.work(args.len())?;
    if args.iter().all(|arg| arg.name.is_none()) {
        for (index, arg) in args.iter().enumerate() {
            budget.work(1)?;
            if let Some(param) = callee.params.get(index) {
                validate_arg_shape_for_param(strings, param, arg)?;
            }
        }
        return Ok(());
    }
    let mut consumed = Vec::new();
    budget.reserve(&mut consumed, args.len())?;
    consumed.resize(args.len(), false);
    let mut ordered_named_index = 0;
    for param in &callee.params {
        budget.work(1)?;
        let Some(arg_index) = resolve_call_arg_index(
            strings,
            args,
            &consumed,
            param,
            &mut ordered_named_index,
            budget,
        )?
        else {
            continue;
        };
        consumed[arg_index] = true;
        validate_arg_shape_for_param(strings, param, &args[arg_index])?;
    }
    Ok(())
}

fn validate_arg_shape_for_param(
    strings: &StringTable,
    param: &ParamEntry,
    arg: &NativeArgShape<'_>,
) -> Result<(), BytecodeError> {
    if matches!(param.direction, 1 | 2) && !arg.is_target {
        let param_name = strings
            .entries
            .get(param.name_idx as usize)
            .map(|name| name.as_str())
            .unwrap_or("<invalid>");
        return Err(BytecodeError::InvalidSection(
            format!("parameter '{param_name}' requires target argument").into(),
        ));
    }
    Ok(())
}

fn resolve_call_arg_index(
    strings: &StringTable,
    args: &[NativeArgShape<'_>],
    consumed: &[bool],
    param: &ParamEntry,
    ordered_named_index: &mut usize,
    budget: &mut ValidationBudget,
) -> Result<Option<usize>, BytecodeError> {
    let param_name =
        strings
            .entries
            .get(param.name_idx as usize)
            .ok_or(BytecodeError::InvalidIndex {
                kind: "string".into(),
                index: param.name_idx,
            })?;
    for (index, arg) in args.iter().enumerate() {
        budget.work(1)?;
        if consumed[index] {
            continue;
        }
        let Some(name) = arg.name else {
            continue;
        };
        if budget.compare_names(name, param_name.as_str())?.is_eq() {
            return Ok(Some(index));
        }
    }
    if *ordered_named_index < args.len()
        && !consumed[*ordered_named_index]
        && args[*ordered_named_index].name.is_none()
    {
        let index = *ordered_named_index;
        *ordered_named_index += 1;
        return Ok(Some(index));
    }
    Ok(None)
}

fn resolve_call_callee_pou_id(
    tables: &ValidationContext<'_>,
    pou: &PouEntry,
    kind: u32,
    target_name: &str,
    budget: &mut ValidationBudget,
) -> Result<Option<u32>, BytecodeError> {
    match kind {
        NATIVE_CALL_KIND_FUNCTION => {
            find_pou_id_by_name(tables, target_name, PouKind::Function, budget)
        }
        NATIVE_CALL_KIND_FUNCTION_BLOCK => {
            if let Some(id) = function_block_pou_from_var_meta(tables, pou, target_name, budget)? {
                return Ok(Some(id));
            }
            if tables.initializers.is_some() {
                Ok(None)
            } else {
                find_pou_id_by_name(tables, target_name, PouKind::FunctionBlock, budget)
            }
        }
        _ => Ok(None),
    }
}

fn function_block_pou_from_var_meta(
    tables: &ValidationContext<'_>,
    pou: &PouEntry,
    target_name: &str,
    budget: &mut ValidationBudget,
) -> Result<Option<u32>, BytecodeError> {
    let source_type = if tables.initializers.is_some() {
        tables.declared_call_type(pou, target_name, budget)?
    } else {
        None
    };
    let type_id = if let Some(type_id) = source_type {
        type_id
    } else {
        if tables.initializers.is_some() {
            return Ok(None);
        }
        let Some(meta) = tables.var_meta else {
            return Ok(None);
        };
        let Some(pou_name) = tables.strings.entries.get(pou.name_idx as usize) else {
            return Ok(None);
        };
        let qualified = budget.concat(&[pou_name.as_str(), ".", target_name])?;
        let qualified_position = tables.named_var(&qualified, budget)?;
        let bare_position = tables.named_var(target_name, budget)?;
        let position = match (qualified_position, bare_position) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let Some(position) = position else {
            return Ok(None);
        };
        meta.entries[position].type_id
    };
    if tables.initializers.is_none() {
        // Preserve the legacy opportunistic lookup's failure and tie behavior.
        let mut ty = type_id;
        for _ in 0..=crate::bytecode::BYTECODE_MAX_CONST_NESTING {
            budget.work(1)?;
            let Some(entry) = tables.types.entries.get(ty as usize) else {
                return Ok(None);
            };
            match entry.data {
                TypeData::Pou { pou_id } if entry.kind == TypeKind::FunctionBlock => {
                    return Ok(Some(pou_id))
                }
                TypeData::Alias { target_type_id } => ty = target_type_id,
                _ => return Ok(None),
            }
        }
        return Ok(None);
    }
    let resolved = tables.function_block_type(type_id, budget)?;
    if source_type.is_some() && resolved.is_none() {
        return Err(RejectionReason::SourceFreeReceiverNotFunctionBlock.into());
    }
    Ok(resolved)
}

fn find_pou_id_by_name(
    tables: &ValidationContext<'_>,
    target_name: &str,
    kind: PouKind,
    budget: &mut ValidationBudget,
) -> Result<Option<u32>, BytecodeError> {
    tables.named_pou(kind, target_name, budget)
}

pub(super) fn parse_native_symbol_args<'a>(
    symbol: &'a str,
    budget: &mut ValidationBudget,
) -> Result<Option<NativeSymbolArgs<'a>>, BytecodeError> {
    budget.work(symbol.len())?;
    let mut parts = symbol.split('|');
    let Some(target_name) = parts.next() else {
        return Ok(None);
    };
    let mut args = Vec::new();
    for raw in parts {
        let (is_target, suffix) = if let Some(rest) = raw.strip_prefix('E') {
            (false, rest)
        } else if let Some(rest) = raw.strip_prefix('T') {
            (true, rest)
        } else {
            return Ok(None);
        };
        let name = if suffix.is_empty() {
            None
        } else {
            let Some(named) = suffix.strip_prefix(':').filter(|name| !name.is_empty()) else {
                return Ok(None);
            };
            Some(named)
        };
        budget.push(&mut args, NativeArgShape { name, is_target })?;
    }
    Ok(Some(NativeSymbolArgs { target_name, args }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::VarMetaEntry;
    use alloc::vec;

    #[test]
    fn function_block_binding_uses_first_wire_match_across_bare_and_qualified_names() {
        let strings = StringTable {
            entries: vec!["Main".into(), "motor".into(), "MAIN.MOTOR".into()],
        };
        let index = PouIndex {
            entries: vec![PouEntry {
                id: 1,
                name_idx: 0,
                kind: PouKind::Program,
                code_offset: 0,
                code_length: 0,
                local_ref_start: 0,
                local_ref_count: 0,
                return_type_id: None,
                owner_pou_id: None,
                params: vec![],
                class_meta: None,
            }],
        };
        let types = TypeTable {
            offsets: vec![],
            entries: [10, 20]
                .map(|pou_id| TypeEntry {
                    kind: TypeKind::FunctionBlock,
                    name_idx: None,
                    data: TypeData::Pou { pou_id },
                })
                .to_vec(),
        };
        let pool = ConstPool::default();
        let refs = RefTable::default();
        let mut meta = VarMeta {
            entries: vec![
                VarMetaEntry {
                    name_idx: 1,
                    type_id: 0,
                    ref_idx: 0,
                    retain: 0,
                    init_const_idx: None,
                },
                VarMetaEntry {
                    name_idx: 2,
                    type_id: 1,
                    ref_idx: 1,
                    retain: 0,
                    init_const_idx: None,
                },
            ],
        };
        for expected in [10, 20] {
            let mut budget = ValidationBudget::new(ValidationLimits::default());
            let tables = ValidationContext::new(
                &strings,
                &index,
                &types,
                &pool,
                &refs,
                Some(&meta),
                &mut budget,
            )
            .unwrap();
            assert_eq!(
                function_block_pou_from_var_meta(&tables, &index.entries[0], "MoToR", &mut budget)
                    .unwrap(),
                Some(expected)
            );
            meta.entries.reverse();
        }
    }
}
