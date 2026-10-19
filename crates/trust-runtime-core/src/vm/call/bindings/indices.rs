use super::VmNativeArg;
use crate::vm::errors::VmTrap;
use crate::vm::module::VmParamMeta;
use alloc::format;

/// The formal-to-actual matching of a call that has no prepared plan: the positional arguments
/// first, each to the formal at its position, then the named ones, each to the formal of its name
/// in any case. [`match_vm_args`] checks the whole matching when it is made, with the refusals in
/// the order the arguments give them, and [`ArgMatch::actual`] finds a formal's actual again
/// without a table, so a call allocates nothing (spec 34 MEM-09; census C6).
#[derive(Debug, Clone, Copy)]
pub struct ArgMatch<'a> {
    params: &'a [VmParamMeta],
    positional: usize,
}

impl ArgMatch<'_> {
    /// The index in `args`, the arguments the matching was made for, of the actual argument
    /// bound to formal `formal`, or `None` when the call passes none. Only the arguments' names
    /// are read, so an argument whose value the binding has taken still names its formal.
    pub fn actual(&self, args: &[VmNativeArg], formal: usize) -> Option<usize> {
        if formal < self.positional {
            return Some(formal);
        }
        self.params.get(formal)?;
        (self.positional..args.len()).find(|&index| {
            args[index]
                .name
                .as_deref()
                .and_then(|actual| formal_of(self.params, actual))
                == Some(formal)
        })
    }
}

/// The formal that the named argument `name` binds: the first of that name in any case.
fn formal_of(params: &[VmParamMeta], name: &str) -> Option<usize> {
    params
        .iter()
        .position(|param| param.name.eq_ignore_ascii_case(name))
}

/// Match the actual arguments `args` of a call to the formals `params`.
///
/// # Errors
///
/// [`VmTrap::InvalidNativeCall`] for more positional arguments than formals, a positional
/// argument after a named one, a name that no formal has, and a second argument for one formal,
/// whichever the arguments reach first.
pub fn match_vm_args<'a>(
    params: &'a [VmParamMeta],
    args: &[VmNativeArg],
) -> Result<ArgMatch<'a>, VmTrap> {
    let positional = args.iter().take_while(|arg| arg.name.is_none()).count();
    if positional > params.len() {
        return Err(VmTrap::InvalidNativeCall(
            format!(
                "too many positional arguments: expected at most {}, got {positional}",
                params.len()
            )
            .into(),
        ));
    }
    if args.iter().skip(positional).any(|arg| arg.name.is_none()) {
        return Err(VmTrap::InvalidNativeCall(
            "positional argument cannot follow a named argument".into(),
        ));
    }
    for (arg_index, arg) in args.iter().enumerate().skip(positional) {
        let name = arg
            .name
            .as_deref()
            .expect("named suffix checked before argument resolution");
        let Some(param_index) = formal_of(params, name) else {
            return Err(VmTrap::InvalidNativeCall(
                format!("unexpected named argument '{name}'").into(),
            ));
        };
        // The formal is already bound by a positional argument or an earlier named one.
        let taken = param_index < positional
            || args[positional..arg_index].iter().any(|earlier| {
                earlier
                    .name
                    .as_deref()
                    .and_then(|earlier| formal_of(params, earlier))
                    == Some(param_index)
            });
        if taken {
            return Err(VmTrap::InvalidNativeCall(
                format!(
                    "duplicate argument for parameter '{}'",
                    params[param_index].name
                )
                .into(),
            ));
        }
    }
    Ok(ArgMatch { params, positional })
}
