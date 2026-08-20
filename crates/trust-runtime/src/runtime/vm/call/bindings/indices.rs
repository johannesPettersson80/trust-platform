use super::VmNativeArg;
use crate::runtime::vm::errors::VmTrap;

pub(super) fn resolve_vm_arg_indices(
    params: &[crate::runtime::vm::VmParamMeta],
    args: &[VmNativeArg],
) -> Result<Vec<Option<usize>>, VmTrap> {
    let positional_count = args.iter().take_while(|arg| arg.name.is_none()).count();
    if positional_count > params.len() {
        return Err(VmTrap::InvalidNativeCall(
            format!(
                "too many positional arguments: expected at most {}, got {positional_count}",
                params.len()
            )
            .into(),
        ));
    }
    if args
        .iter()
        .skip(positional_count)
        .any(|arg| arg.name.is_none())
    {
        return Err(VmTrap::InvalidNativeCall(
            "positional argument cannot follow a named argument".into(),
        ));
    }

    let mut indices = vec![None; params.len()];
    for (index, slot) in indices.iter_mut().enumerate().take(positional_count) {
        *slot = Some(index);
    }
    for (arg_index, arg) in args.iter().enumerate().skip(positional_count) {
        let name = arg
            .name
            .as_deref()
            .expect("named suffix checked before argument resolution");
        let Some(param_index) = params
            .iter()
            .position(|param| param.name.eq_ignore_ascii_case(name))
        else {
            return Err(VmTrap::InvalidNativeCall(
                format!("unexpected named argument '{name}'").into(),
            ));
        };
        if indices[param_index].is_some() {
            return Err(VmTrap::InvalidNativeCall(
                format!(
                    "duplicate argument for parameter '{}'",
                    params[param_index].name
                )
                .into(),
            ));
        }
        indices[param_index] = Some(arg_index);
    }
    Ok(indices)
}
