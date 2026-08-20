#[derive(Debug, Clone)]
struct NativeSymbolArgs {
    target_name: String,
    args: Vec<NativeArgShape>,
}

#[derive(Debug, Clone)]
struct NativeArgShape {
    name: Option<String>,
    is_target: bool,
}

fn validate_param_direction_metadata(entry: &PouEntry) -> Result<(), BytecodeError> {
    for param in &entry.params {
        if !matches!(param.direction, 0..=2) {
            return Err(BytecodeError::InvalidSection(
                format!("invalid parameter direction {}", param.direction).into(),
            ));
        }
    }
    Ok(())
}

fn validate_param_direction_calls(
    strings: &StringTable,
    types: &TypeTable,
    var_meta: Option<&VarMeta>,
    index: &PouIndex,
    pou: &PouEntry,
    code: &[u8],
) -> Result<(), BytecodeError> {
    let mut reader = BytecodeReader::new(code);
    while reader.remaining() > 0 {
        let opcode = reader.read_u8()?;
        match opcode {
            0x00 | 0x01 | 0x06 | 0x11 | 0x12 | 0x13 | 0x23 | 0x24 | 0x25 | 0x31 | 0x32
            | 0x33 | 0x40..=0x49 | 0x4C | 0x50..=0x55 | 0x61 => {}
            0x02..=0x05 | 0x07 | 0x10 | 0x30 | 0x60 | 0x62..=0x64 | 0x70 => {
                let _ = reader.read_u32()?;
            }
            0x08 => {
                let _interface_type_id = reader.read_u32()?;
                let _slot = reader.read_u32()?;
            }
            0x09 => {
                let kind = reader.read_u32()?;
                let symbol_idx = reader.read_u32()?;
                let _arg_count = reader.read_u32()?;
                validate_native_call_param_directions(
                    strings, types, var_meta, index, pou, kind, symbol_idx,
                )?;
            }
            0x20..=0x22 => {
                let _ref_idx = reader.read_u32()?;
            }
            _ => return Err(BytecodeError::InvalidOpcode(opcode)),
        }
    }
    Ok(())
}

fn validate_native_call_param_directions(
    strings: &StringTable,
    types: &TypeTable,
    var_meta: Option<&VarMeta>,
    index: &PouIndex,
    pou: &PouEntry,
    kind: u32,
    symbol_idx: u32,
) -> Result<(), BytecodeError> {
    let Some(symbol) = strings.entries.get(symbol_idx as usize) else {
        return Ok(());
    };
    let Some(symbol_args) = parse_native_symbol_args(symbol.as_str()) else {
        return Ok(());
    };
    let Some(callee_id) = resolve_call_callee_pou_id(
        strings,
        types,
        var_meta,
        index,
        pou,
        kind,
        &symbol_args.target_name,
    ) else {
        return Ok(());
    };
    let Some(callee) = index.entries.iter().find(|entry| entry.id == callee_id) else {
        return Ok(());
    };
    validate_call_arg_shapes(strings, callee, &symbol_args.args)
}

fn validate_call_arg_shapes(
    strings: &StringTable,
    callee: &PouEntry,
    args: &[NativeArgShape],
) -> Result<(), BytecodeError> {
    let positional = args.iter().all(|arg| arg.name.is_none());
    if positional {
        for (index, arg) in args.iter().enumerate() {
            if let Some(param) = callee.params.get(index) {
                validate_arg_shape_for_param(strings, param, arg)?;
            }
        }
        return Ok(());
    }

    let mut consumed = vec![false; args.len()];
    let mut ordered_named_index = 0usize;
    for param in &callee.params {
        let Some(arg_index) =
            resolve_call_arg_index(strings, args, &consumed, param, &mut ordered_named_index)?
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
    arg: &NativeArgShape,
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
    args: &[NativeArgShape],
    consumed: &[bool],
    param: &ParamEntry,
    ordered_named_index: &mut usize,
) -> Result<Option<usize>, BytecodeError> {
    let param_name = strings
        .entries
        .get(param.name_idx as usize)
        .ok_or(BytecodeError::InvalidIndex {
            kind: "string".into(),
            index: param.name_idx,
        })?;
    if let Some(index) = args.iter().enumerate().find_map(|(index, arg)| {
        (!consumed[index]
            && arg
                .name
                .as_deref()
                .is_some_and(|name| name.eq_ignore_ascii_case(param_name.as_str())))
        .then_some(index)
    }) {
        return Ok(Some(index));
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
    strings: &StringTable,
    types: &TypeTable,
    var_meta: Option<&VarMeta>,
    index: &PouIndex,
    pou: &PouEntry,
    kind: u32,
    target_name: &str,
) -> Option<u32> {
    match kind {
        NATIVE_CALL_KIND_FUNCTION => find_pou_id_by_name(strings, index, target_name, PouKind::Function),
        NATIVE_CALL_KIND_FUNCTION_BLOCK => {
            function_block_pou_from_var_meta(strings, types, var_meta, pou, target_name).or_else(
                || find_pou_id_by_name(strings, index, target_name, PouKind::FunctionBlock),
            )
        }
        _ => None,
    }
}

fn function_block_pou_from_var_meta(
    strings: &StringTable,
    types: &TypeTable,
    var_meta: Option<&VarMeta>,
    pou: &PouEntry,
    target_name: &str,
) -> Option<u32> {
    let var_meta = var_meta?;
    let pou_name = strings.entries.get(pou.name_idx as usize)?;
    let qualified = format!("{pou_name}.{target_name}");
    let type_id = var_meta.entries.iter().find_map(|entry| {
        let name = strings.entries.get(entry.name_idx as usize)?;
        (name.eq_ignore_ascii_case(&qualified) || name.eq_ignore_ascii_case(target_name))
            .then_some(entry.type_id)
    })?;
    pou_id_for_function_block_type(types, type_id, 0)
}

fn pou_id_for_function_block_type(types: &TypeTable, type_id: u32, depth: usize) -> Option<u32> {
    if depth > 64 {
        return None;
    }
    let entry = types.entries.get(type_id as usize)?;
    match &entry.data {
        TypeData::Pou { pou_id } if entry.kind == TypeKind::FunctionBlock => Some(*pou_id),
        TypeData::Alias { target_type_id } => {
            pou_id_for_function_block_type(types, *target_type_id, depth + 1)
        }
        _ => None,
    }
}

fn find_pou_id_by_name(
    strings: &StringTable,
    index: &PouIndex,
    target_name: &str,
    kind: PouKind,
) -> Option<u32> {
    index.entries.iter().find_map(|entry| {
        if entry.kind != kind {
            return None;
        }
        let name = strings.entries.get(entry.name_idx as usize)?;
        name.eq_ignore_ascii_case(target_name).then_some(entry.id)
    })
}

fn parse_native_symbol_args(symbol: &str) -> Option<NativeSymbolArgs> {
    let mut parts = symbol.split('|');
    let target_name = parts.next()?.to_owned();
    let mut args = Vec::new();
    for raw in parts {
        let (is_target, suffix) = if let Some(rest) = raw.strip_prefix('E') {
            (false, rest)
        } else {
            let rest = raw.strip_prefix('T')?;
            (true, rest)
        };
        let name = if suffix.is_empty() {
            None
        } else {
            let named = suffix.strip_prefix(':')?;
            if named.is_empty() {
                return None;
            }
            Some(named.to_owned())
        };
        args.push(NativeArgShape { name, is_target });
    }
    Some(NativeSymbolArgs { target_name, args })
}
