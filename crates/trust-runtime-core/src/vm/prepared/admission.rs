//! Execution-profile checks beyond the portable wire validator.
use super::*;
use crate::bytecode::{
    StorageRole, TypeData, NATIVE_CALL_KIND_FUNCTION, NATIVE_CALL_KIND_FUNCTION_BLOCK,
    NATIVE_CALL_KIND_METHOD, NATIVE_CALL_KIND_STDLIB,
};
use crate::error::ImportRefusal;
use crate::stdlib::contracts::{
    self, FunctionBlockImport, FunctionImport, FunctionSignature, ImportCapability,
};
use crate::stdlib::StdParams;
use crate::vm::module::{VmNativeArgSpec, VmNativeSymbolSpec};
use smol_str::SmolStr;

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

/// Match every standard-library call and standard function-block template with its built-in
/// contract (spec 34 §5.2.1). Returns whether the artifact needs the platform wall clock.
pub(super) fn check_imports(
    vm: &VmModule,
    initializers: &InitializerIndex,
    layout: &StorageLayout,
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
                        let import = contracts::resolve_function(
                            normalized_target_name,
                            conversion_spec.is_some(),
                        )
                        .ok_or_else(|| {
                            refused(normalized_target_name, ImportRefusal::NoContract)
                        })?;
                        check_function_call(normalized_target_name, &import, arg_specs, budget)?;
                        wall_clock |=
                            import.contract.capability == Some(ImportCapability::WallClock);
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
    for (name_idx, pou) in vm.function_block_ids.iter() {
        let name = SmolStr::new(vm.strings[name_idx as usize].to_ascii_uppercase());
        if let Some(import) = contracts::function_block(&name) {
            check_builtin_state(vm, layout, pou, &name, &import, budget)?;
            check_builtin_parameters(vm, pou, &name, &import, budget)?;
        }
    }
    Ok(wall_clock)
}

/// The stable refusal of an import that has no contract or does not match it.
fn refused(import: &SmolStr, reason: ImportRefusal) -> RuntimeError {
    RuntimeError::ImportUnsupported {
        import: import.clone(),
        reason,
    }
}

/// Match one standard-library call descriptor with the signature of its contract.
fn check_function_call(
    name: &SmolStr,
    import: &FunctionImport,
    args: &[VmNativeArgSpec],
    budget: &mut PreparationBudget,
) -> Result<(), RuntimeError> {
    match import.signature {
        FunctionSignature::Clock => {
            if !args.is_empty() {
                return Err(refused(name, ImportRefusal::ParameterCount));
            }
        }
        FunctionSignature::Split(params) => {
            let positions = check_arguments(name, params, None, args, budget)?;
            if positions
                .iter()
                .zip(args)
                .any(|(position, arg)| *position != 0 && !arg.is_target)
            {
                return Err(refused(name, ImportRefusal::OutputTarget));
            }
        }
        FunctionSignature::Conversion => {
            check_arguments(name, &["IN"], None, args, budget)?;
        }
        FunctionSignature::Registered(StdParams::Fixed(fixed)) => {
            check_arguments(name, &fixed[..], None, args, budget)?;
        }
        FunctionSignature::Registered(StdParams::Variadic {
            fixed,
            prefix,
            start,
            min,
        }) => {
            check_arguments(name, &fixed[..], Some((prefix, *start, *min)), args, budget)?;
        }
    }
    Ok(())
}

fn check_builtin_state(
    vm: &VmModule,
    layout: &StorageLayout,
    pou: u32,
    name: &SmolStr,
    import: &FunctionBlockImport,
    budget: &mut PreparationBudget,
) -> Result<(), RuntimeError> {
    let expected = import.state;
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
        return Err(refused(name, ImportRefusal::StateLayout));
    }
    for (slot, primitive) in expected {
        let entry = actual
            .iter()
            .find(|entry| &vm.strings[entry.name_idx as usize] == *slot)
            .ok_or_else(|| refused(name, ImportRefusal::StateLayout))?;
        let ty = entry
            .type_id
            .and_then(|id| crate::vm::type_policy::resolved_alias_type(&vm.types, id, 0))
            .and_then(|id| vm.types.entries.get(id as usize))
            .ok_or_else(|| refused(name, ImportRefusal::StateLayout))?;
        if !matches!(&ty.data,TypeData::Primitive{prim_id,max_length:0} if *prim_id==*primitive) {
            return Err(refused(name, ImportRefusal::StateLayout));
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
    name: &SmolStr,
    import: &FunctionBlockImport,
    budget: &mut PreparationBudget,
) -> Result<(), RuntimeError> {
    // Native algorithms address these fields by name and direction. The generic
    // counter width is bound dynamically; concrete variants keep their fixed width.
    let params = vm.pou_params(pou).ok_or_else(|| {
        invalid_bytecode(smol_str::SmolStr::new_static("missing native FB signature"))
    })?;
    if params.len() != import.parameters.len() {
        return Err(refused(name, ImportRefusal::ParameterCount));
    }
    for expected in import.parameters {
        budget.charge(
            0,
            params
                .len()
                .checked_mul(expected.name.len())
                .ok_or(RuntimeError::Overflow)?,
        )?;
        let param = params
            .iter()
            .find(|param| param.name.eq_ignore_ascii_case(expected.name))
            .ok_or_else(|| refused(name, ImportRefusal::ParameterName))?;
        if param.direction != u8::from(expected.output)
            || primitive(vm, param.type_id) != Some(import.primitive(expected.ty))
        {
            return Err(refused(name, ImportRefusal::ParameterType));
        }
    }
    Ok(())
}

/// Bind call descriptors to parameter positions as execution binds them (specification 12
/// §7.7): an exact or minimum count, then formal names without duplicates or gaps.
fn check_arguments(
    import: &SmolStr,
    fixed: &[impl AsRef<str>],
    variadic: Option<(&SmolStr, usize, usize)>,
    args: &[VmNativeArgSpec],
    budget: &mut PreparationBudget,
) -> Result<Vec<usize>, RuntimeError> {
    budget.charge(
        args.len()
            .checked_mul(core::mem::size_of::<usize>())
            .ok_or(RuntimeError::Overflow)?,
        args.len(),
    )?;
    let minimum = fixed.len() + variadic.map_or(0, |(_, _, min)| min);
    if args.len() < minimum || (variadic.is_none() && args.len() != minimum) {
        return Err(refused(import, ImportRefusal::ParameterCount));
    }
    if args.iter().all(|arg| arg.name.is_none()) {
        return Ok((0..args.len()).collect());
    }
    let mut positions = Vec::with_capacity(args.len());
    for arg in args {
        // Named and positional arguments are never mixed in one call.
        let name = arg
            .name
            .as_ref()
            .ok_or_else(|| refused(import, ImportRefusal::ParameterName))?;
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
            .position(|parameter| parameter.as_ref().eq_ignore_ascii_case(name))
        {
            index
        } else {
            let (prefix, start, _) =
                variadic.ok_or_else(|| refused(import, ImportRefusal::ParameterName))?;
            if !name
                .get(..prefix.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
            {
                return Err(refused(import, ImportRefusal::ParameterName));
            }
            name.get(prefix.len()..)
                .and_then(|suffix| suffix.parse::<usize>().ok())
                .and_then(|index| index.checked_sub(start))
                .and_then(|index| index.checked_add(fixed.len()))
                .ok_or_else(|| refused(import, ImportRefusal::ParameterName))?
        };
        // A position past the supplied count leaves a gap in the variadic suffix.
        if position >= args.len() || positions.contains(&position) {
            return Err(refused(import, ImportRefusal::ParameterName));
        }
        positions.push(position);
    }
    Ok(positions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::RuntimeError;

    fn arg(name: Option<&str>, is_target: bool) -> VmNativeArgSpec {
        VmNativeArgSpec {
            name: name.map(SmolStr::new),
            is_target,
        }
    }

    fn check(name: &str, args: &[VmNativeArgSpec]) -> Result<(), RuntimeError> {
        let name = SmolStr::new(name);
        let import = contracts::function(&name).expect("built-in import");
        let mut budget = PreparationBudget::new(PreparationLimits::default());
        check_function_call(&name, &import, args, &mut budget)
    }

    fn refusal(name: &str, reason: ImportRefusal) -> Result<(), RuntimeError> {
        Err(RuntimeError::ImportUnsupported {
            import: name.into(),
            reason,
        })
    }

    #[test]
    fn split_outputs_must_be_writable_targets_by_position_or_name() {
        let input = arg(None, false);
        let target = arg(None, true);
        assert_eq!(
            check(
                "SPLIT_DATE",
                &[
                    input.clone(),
                    target.clone(),
                    target.clone(),
                    target.clone()
                ]
            ),
            Ok(())
        );
        assert_eq!(
            check(
                "SPLIT_DATE",
                &[input.clone(), input.clone(), target.clone(), target.clone()]
            ),
            refusal("SPLIT_DATE", ImportRefusal::OutputTarget)
        );
        // Named outputs bind by formal name, case-insensitively, in any order.
        assert_eq!(
            check(
                "SPLIT_TOD",
                &[
                    arg(Some("millisecond"), true),
                    arg(Some("IN"), false),
                    arg(Some("Hour"), true),
                    arg(Some("SECOND"), true),
                    arg(Some("MINUTE"), true),
                ]
            ),
            Ok(())
        );
        assert_eq!(
            check(
                "SPLIT_TOD",
                &[
                    arg(Some("MILLISECOND"), true),
                    arg(Some("IN"), false),
                    arg(Some("HOUR"), false),
                    arg(Some("SECOND"), true),
                    arg(Some("MINUTE"), true),
                ]
            ),
            refusal("SPLIT_TOD", ImportRefusal::OutputTarget)
        );
        assert_eq!(
            check("SPLIT_DT", &[input.clone(), target.clone()]),
            refusal("SPLIT_DT", ImportRefusal::ParameterCount)
        );
    }

    #[test]
    fn call_shapes_that_execution_would_not_bind_are_refused_by_name() {
        let value = arg(None, false);
        assert_eq!(
            check("TIME", std::slice::from_ref(&value)),
            refusal("TIME", ImportRefusal::ParameterCount)
        );
        assert_eq!(check("CURRENT_DT", &[]), Ok(()));
        assert_eq!(
            check("ADD", std::slice::from_ref(&value)),
            refusal("ADD", ImportRefusal::ParameterCount)
        );
        assert_eq!(
            check("ADD", &[value.clone(), value.clone(), value.clone()]),
            Ok(())
        );
        assert_eq!(
            check("SUB", &[value.clone(), value.clone(), value.clone()]),
            refusal("SUB", ImportRefusal::ParameterCount)
        );
        assert_eq!(
            check("ADD", &[arg(Some("IN1"), false), arg(Some("IN3"), false)]),
            refusal("ADD", ImportRefusal::ParameterName),
            "a gap in the variadic suffix"
        );
        assert_eq!(
            check("ADD", &[arg(Some("IN2"), false), arg(Some("in1"), false)]),
            Ok(())
        );
        assert_eq!(
            check("ADD", &[arg(Some("IN1"), false), arg(Some("IN1"), false)]),
            refusal("ADD", ImportRefusal::ParameterName),
            "a repeated name"
        );
        assert_eq!(
            check("SUB", &[arg(Some("IN1"), false), value.clone()]),
            refusal("SUB", ImportRefusal::ParameterName),
            "named and positional arguments together"
        );
        assert_eq!(
            check(
                "MUX",
                &[
                    arg(Some("K"), false),
                    arg(Some("IN0"), false),
                    arg(Some("IN1"), false)
                ]
            ),
            Ok(())
        );
        assert_eq!(
            check(
                "MUX",
                &[
                    arg(Some("K"), false),
                    arg(Some("X0"), false),
                    arg(Some("IN1"), false)
                ]
            ),
            refusal("MUX", ImportRefusal::ParameterName)
        );
        assert_eq!(check("INT_TO_DINT", &[arg(Some("in"), false)]), Ok(()));
        assert_eq!(
            check("INT_TO_DINT", &[arg(Some("OUT"), false)]),
            refusal("INT_TO_DINT", ImportRefusal::ParameterName)
        );
        assert_eq!(
            check("INT_TO_DINT", &[value.clone(), value]),
            refusal("INT_TO_DINT", ImportRefusal::ParameterCount)
        );
    }

    #[test]
    fn exhausted_preparation_budget_is_not_reported_as_an_import_refusal() {
        let name = SmolStr::new("SQRT");
        let import = contracts::function(&name).unwrap();
        let mut budget = PreparationBudget::new(PreparationLimits {
            max_preparation_work: 0,
            ..PreparationLimits::default()
        });
        assert_eq!(
            check_function_call(&name, &import, &[arg(None, false)], &mut budget),
            Err(RuntimeError::PreparationLimit)
        );
    }
}
