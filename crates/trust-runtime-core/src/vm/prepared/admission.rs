//! Execution-profile checks beyond the portable wire validator.
use super::*;
use crate::bytecode::{
    StorageRole, TypeData, NATIVE_CALL_KIND_FUNCTION, NATIVE_CALL_KIND_FUNCTION_BLOCK,
    NATIVE_CALL_KIND_METHOD, NATIVE_CALL_KIND_STDLIB,
};
use crate::stdlib::{fbs, time, StandardLibrary};
use crate::vm::module::VmNativeSymbolSpec;

pub(super) fn check_construction_demand(
    layout: &StorageLayout,
    roots: &ConstructionRoots,
    limits: PreparationLimits,
) -> Result<(), RuntimeError> {
    // Wire validation independently recomputes these demands from TYPE_TABLE and
    // declaration ownership; only the resulting validated numbers are used here.
    for declaration in &layout.entries {
        if declaration.construction_nodes as usize > limits.max_construction_values {
            return Err(RuntimeError::PreparationLimit);
        }
    }
    let mut total = 0usize;
    for root in roots
        .entries
        .iter()
        .filter(|root| root.parent_root_idx.is_none())
    {
        let declaration = &layout.entries[root.declaration_idx as usize];
        total = total
            .checked_add(declaration.construction_nodes as usize)
            .ok_or(RuntimeError::Overflow)?;
        if total > limits.max_construction_values {
            return Err(RuntimeError::PreparationLimit);
        }
    }
    Ok(())
}

pub(super) fn check_imports(
    vm: &VmModule,
    initializers: &InitializerIndex,
    layout: &StorageLayout,
    registry: &StandardLibrary,
    budget: &mut PreparationBudget,
) -> Result<bool, RuntimeError> {
    let mut wall_clock = false;
    for range in vm
        .pou_by_id
        .values()
        .map(|pou| pou.code_start..pou.code_end)
        .chain(initializers.entries.iter().map(|entry| {
            entry.code_offset as usize..(entry.code_offset + entry.code_length) as usize
        }))
    {
        let code = vm.code.get(range).ok_or_else(|| {
            invalid_bytecode(smol_str::SmolStr::new_static("invalid executable range"))
        })?;
        let mut pc = 0usize;
        while pc < code.len() {
            budget.charge(0, 1)?;
            let opcode = code[pc];
            let width = super::super::opcode_operand_len(opcode).ok_or_else(|| {
                invalid_bytecode(smol_str::SmolStr::new_static(
                    "unsupported executable opcode",
                ))
            })?;
            let next = pc
                .checked_add(1 + width)
                .filter(|end| *end <= code.len())
                .ok_or_else(|| {
                    invalid_bytecode(smol_str::SmolStr::new_static(
                        "truncated executable instruction",
                    ))
                })?;
            if opcode == 0x09 {
                let operand = |offset: usize| {
                    u32::from_le_bytes(
                        code[pc + 1 + offset..pc + 5 + offset]
                            .try_into()
                            .expect("validated native instruction width"),
                    )
                };
                let kind = operand(0);
                let spec = vm
                    .native_symbol_spec(operand(4))
                    .map_err(super::super::VmTrap::into_runtime_error)?;
                let VmNativeSymbolSpec::Parsed {
                    normalized_target_name,
                    resolved_function_pou_id,
                    conversion_spec,
                    arg_specs,
                    ..
                } = spec
                else {
                    return Err(invalid_bytecode(smol_str::SmolStr::new_static(
                        "invalid native import descriptor",
                    )));
                };
                let receiver_count = usize::from(matches!(
                    kind,
                    NATIVE_CALL_KIND_FUNCTION_BLOCK | NATIVE_CALL_KIND_METHOD
                ));
                if arg_specs.len() + receiver_count != operand(8) as usize {
                    return Err(invalid_bytecode(smol_str::SmolStr::new_static(
                        "native argument descriptor count mismatch",
                    )));
                }
                match kind {
                    NATIVE_CALL_KIND_STDLIB => {
                        let name = normalized_target_name.as_str();
                        if conversion_spec.is_none()
                            && registry.get(name).is_none()
                            && !time::is_runtime_clock_name(name)
                            && !time::is_split_name(name)
                        {
                            return Err(RuntimeError::ProfileUnsupported(
                                smol_str::SmolStr::new_static(
                                    "unsupported native standard-function import",
                                ),
                            ));
                        }
                        if time::is_runtime_clock_name(name) {
                            if !arg_specs.is_empty() {
                                return Err(invalid_bytecode(smol_str::SmolStr::new_static(
                                    "clock import takes no arguments",
                                )));
                            }
                        } else if let Some(params) = time::split_parameter_names(name) {
                            let params = crate::stdlib::StdParams::Fixed(
                                params.iter().map(smol_str::SmolStr::new).collect(),
                            );
                            let positions = check_arguments(&params, arg_specs, budget)?;
                            if positions
                                .iter()
                                .zip(arg_specs)
                                .any(|(position, arg)| *position != 0 && !arg.is_target)
                            {
                                return Err(invalid_bytecode(smol_str::SmolStr::new_static(
                                    "native output requires a writable target",
                                )));
                            }
                        } else if conversion_spec.is_some() {
                            check_arguments(
                                &crate::stdlib::StdParams::Fixed(alloc::vec!["IN".into()].into()),
                                arg_specs,
                                budget,
                            )?;
                        } else if let Some(entry) = registry.get(name) {
                            check_arguments(entry.params, arg_specs, budget)?;
                        }
                        wall_clock |= name == "CURRENT_DT";
                    }
                    NATIVE_CALL_KIND_FUNCTION => {
                        if resolved_function_pou_id.is_none() {
                            return Err(invalid_bytecode(smol_str::SmolStr::new_static(
                                "unresolved native function import",
                            )));
                        }
                    }
                    // Receiver identity and dynamic interface dispatch remain checked
                    // at execution. Static template contracts are checked below.
                    NATIVE_CALL_KIND_FUNCTION_BLOCK | NATIVE_CALL_KIND_METHOD => {}
                    _ => {
                        return Err(invalid_bytecode(smol_str::SmolStr::new_static(
                            "unsupported native call kind",
                        )))
                    }
                }
            }
            pc = next;
        }
    }
    for (name, pou) in &vm.function_block_ids {
        if let Some(kind) = fbs::builtin_kind_uppercase(name) {
            check_builtin_state(vm, layout, *pou, kind, budget)?;
            check_builtin_parameters(vm, *pou, name, kind, budget)?;
        }
    }
    Ok(wall_clock)
}

fn check_builtin_state(
    vm: &VmModule,
    layout: &StorageLayout,
    pou: u32,
    kind: fbs::BuiltinFbKind,
    budget: &mut PreparationBudget,
) -> Result<(), RuntimeError> {
    let expected = fbs::state::builtin_state_layout(kind);
    budget.charge(
        0,
        layout
            .entries
            .len()
            .checked_mul(2)
            .ok_or(RuntimeError::Overflow)?,
    )?;
    let count = layout
        .entries
        .iter()
        .filter(|entry| entry.owner_pou_id == Some(pou) && entry.role == StorageRole::NativeState)
        .count();
    budget.charge(
        count
            .checked_mul(core::mem::size_of::<&crate::bytecode::StorageDeclaration>())
            .ok_or(RuntimeError::Overflow)?,
        count,
    )?;
    let actual: Vec<_> = layout
        .entries
        .iter()
        .filter(|entry| entry.owner_pou_id == Some(pou) && entry.role == StorageRole::NativeState)
        .collect();
    if actual.len() != expected.len() {
        return Err(invalid_bytecode(smol_str::SmolStr::new_static(
            "native FB state layout mismatch",
        )));
    }
    for (name, primitive) in expected {
        let entry = actual
            .iter()
            .find(|entry| vm.strings[entry.name_idx as usize].as_str() == *name)
            .ok_or_else(|| {
                invalid_bytecode(smol_str::SmolStr::new_static(
                    "missing native FB state slot",
                ))
            })?;
        let ty = entry
            .type_id
            .and_then(|id| crate::vm::type_policy::resolved_alias_type(&vm.types, id, 0))
            .and_then(|id| vm.types.entries.get(id as usize))
            .ok_or_else(|| {
                invalid_bytecode(smol_str::SmolStr::new_static(
                    "untyped native FB state slot",
                ))
            })?;
        if !matches!(&ty.data,TypeData::Primitive{prim_id,max_length:0} if *prim_id==*primitive) {
            return Err(invalid_bytecode(smol_str::SmolStr::new_static(
                "native FB state slot type mismatch",
            )));
        }
    }
    Ok(())
}

fn primitive(vm: &VmModule, id: u32) -> Option<u16> {
    let id = crate::vm::type_policy::resolved_alias_type(&vm.types, id, 0)?;
    match &vm.types.entries.get(id as usize)?.data {
        TypeData::Primitive { prim_id, .. } => Some(*prim_id),
        _ => None,
    }
}

fn check_builtin_parameters(
    vm: &VmModule,
    pou: u32,
    name: &str,
    kind: fbs::BuiltinFbKind,
    budget: &mut PreparationBudget,
) -> Result<(), RuntimeError> {
    use fbs::BuiltinFbKind::*;
    // Native algorithms address these fields by name and direction. The generic
    // counter width is bound dynamically; concrete variants keep their fixed width.
    let number = if name.ends_with("_ULINT") {
        13
    } else if name.ends_with("_UDINT") {
        12
    } else if name.ends_with("_LINT") {
        9
    } else if name.ends_with("_DINT") {
        8
    } else if name.ends_with("_INT") {
        7
    } else {
        0x100
    };
    let duration = if name.ends_with("_LTIME") { 17 } else { 16 };
    let expected: &[(&str, u8, u16)] = match kind {
        Rs => &[("S", 0, 1), ("R1", 0, 1), ("Q1", 1, 1)],
        Sr => &[("S1", 0, 1), ("R", 0, 1), ("Q1", 1, 1)],
        RTrig | FTrig => &[("CLK", 0, 1), ("Q", 1, 1)],
        Ctu => &[
            ("CU", 0, 1),
            ("R", 0, 1),
            ("PV", 0, number),
            ("Q", 1, 1),
            ("CV", 1, number),
        ],
        Ctd => &[
            ("CD", 0, 1),
            ("LD", 0, 1),
            ("PV", 0, number),
            ("Q", 1, 1),
            ("CV", 1, number),
        ],
        Ctud => &[
            ("CU", 0, 1),
            ("CD", 0, 1),
            ("R", 0, 1),
            ("LD", 0, 1),
            ("PV", 0, number),
            ("QU", 1, 1),
            ("QD", 1, 1),
            ("CV", 1, number),
        ],
        Ton | Tof | Tp => &[
            ("IN", 0, 1),
            ("PT", 0, duration),
            ("Q", 1, 1),
            ("ET", 1, duration),
        ],
    };
    let params = vm.pou_params(pou).ok_or_else(|| {
        invalid_bytecode(smol_str::SmolStr::new_static("missing native FB signature"))
    })?;
    if params.len() != expected.len() {
        return Err(invalid_bytecode(smol_str::SmolStr::new_static(
            "native FB signature arity mismatch",
        )));
    }
    for (name, direction, ty) in expected {
        budget.charge(
            0,
            params
                .len()
                .checked_mul(name.len())
                .ok_or(RuntimeError::Overflow)?,
        )?;
        let param = params
            .iter()
            .find(|param| param.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                invalid_bytecode(smol_str::SmolStr::new_static(
                    "native FB signature name mismatch",
                ))
            })?;
        if param.direction != *direction || primitive(vm, param.type_id) != Some(*ty) {
            return Err(invalid_bytecode(smol_str::SmolStr::new_static(
                "native FB signature type or direction mismatch",
            )));
        }
    }
    Ok(())
}

fn check_arguments(
    params: &crate::stdlib::StdParams,
    args: &[crate::vm::module::VmNativeArgSpec],
    budget: &mut PreparationBudget,
) -> Result<Vec<usize>, RuntimeError> {
    use crate::stdlib::StdParams;
    budget.charge(
        args.len()
            .checked_mul(core::mem::size_of::<usize>())
            .ok_or(RuntimeError::Overflow)?,
        args.len(),
    )?;
    let (fixed, minimum, variadic) = match params {
        StdParams::Fixed(fixed) => (fixed, fixed.len(), None),
        StdParams::Variadic {
            fixed,
            prefix,
            start,
            min,
        } => (fixed, fixed.len() + *min, Some((prefix, *start))),
    };
    if args.len() < minimum || (variadic.is_none() && args.len() != minimum) {
        return Err(invalid_bytecode(smol_str::SmolStr::new_static(
            "native signature argument count mismatch",
        )));
    }
    if args.iter().all(|arg| arg.name.is_none()) {
        return Ok((0..args.len()).collect());
    }
    let mut positions = Vec::with_capacity(args.len());
    for arg in args {
        let name = arg.name.as_ref().ok_or_else(|| {
            invalid_bytecode(smol_str::SmolStr::new_static(
                "mixed positional and named native arguments",
            ))
        })?;
        budget.charge(
            0,
            fixed
                .len()
                .checked_mul(name.len())
                .and_then(|n| n.checked_add(positions.len()))
                .ok_or(RuntimeError::Overflow)?,
        )?;
        let position = if let Some(index) = fixed
            .iter()
            .position(|parameter| parameter.eq_ignore_ascii_case(name))
        {
            index
        } else {
            let (prefix, start) = variadic.ok_or_else(|| {
                invalid_bytecode(smol_str::SmolStr::new_static("unknown native parameter"))
            })?;
            if !name
                .get(..prefix.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
            {
                return Err(invalid_bytecode(smol_str::SmolStr::new_static(
                    "unknown variadic parameter",
                )));
            }
            let index = name
                .get(prefix.len()..)
                .and_then(|suffix| suffix.parse::<usize>().ok())
                .and_then(|index| index.checked_sub(start))
                .and_then(|index| index.checked_add(fixed.len()))
                .ok_or_else(|| {
                    invalid_bytecode(smol_str::SmolStr::new_static(
                        "invalid variadic parameter index",
                    ))
                })?;
            index
        };
        if position >= args.len() || positions.contains(&position) {
            return Err(invalid_bytecode(smol_str::SmolStr::new_static(
                "sparse or duplicate native parameters",
            )));
        }
        positions.push(position);
    }
    Ok(positions)
}
