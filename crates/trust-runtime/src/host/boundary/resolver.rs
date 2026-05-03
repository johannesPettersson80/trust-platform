use smol_str::SmolStr;

use crate::io::IoAddress;
use crate::memory::InstanceId;
use crate::program_model::{Expr, LValue};
use crate::value::Value;
use crate::Runtime;

use super::BoundaryError;

enum RootResolution {
    Global,
    ProgramVar { instance_id: InstanceId },
}

pub(crate) fn resolve_read(runtime: &Runtime, path: &str) -> Result<Value, BoundaryError> {
    if let Some(value) = runtime.storage().get_global(path) {
        return Ok(value.clone());
    }
    if let Some((instance_id, _)) = find_program_var_instance(runtime, path)? {
        return runtime
            .storage()
            .get_instance_var_recursive(instance_id, path)
            .cloned()
            .ok_or_else(|| BoundaryError::UnresolvedName { path: path.into() });
    }
    if is_simple_name(path) {
        return Err(BoundaryError::UnresolvedName { path: path.into() });
    }

    let expr = parse_read_path(runtime, path)?;
    let current_instance = match expr_root_name(&expr)
        .ok_or_else(|| BoundaryError::UnsupportedPathSyntax {
            path: path.into(),
            reason: "path must start with a declared name".to_string(),
        })
        .and_then(|root| classify_root(runtime, path, root.as_str()))?
    {
        RootResolution::Global => None,
        RootResolution::ProgramVar { instance_id } => Some(instance_id),
    };

    crate::helper_eval::eval_storage_expr_with_stdlib(
        runtime.storage(),
        runtime.registry(),
        &runtime.profile(),
        current_instance,
        Some(runtime.stdlib()),
        &expr,
    )
    .map_err(|error| BoundaryError::from_runtime(path, error))
}

pub(crate) fn resolve_write(
    runtime: &mut Runtime,
    path: &str,
    value: Value,
) -> Result<(), BoundaryError> {
    if is_simple_name(path) {
        if runtime.storage().get_global(path).is_some() {
            runtime.storage_mut().set_global(SmolStr::new(path), value);
            return Ok(());
        }
        if let Some((instance_id, _)) = find_program_var_instance(runtime, path)? {
            if runtime
                .storage_mut()
                .set_instance_var(instance_id, SmolStr::new(path), value)
            {
                return Ok(());
            }
            return Err(BoundaryError::InternalFailure {
                context: "program variable instance disappeared during write",
            });
        }
        return Err(BoundaryError::UnresolvedName { path: path.into() });
    }

    let target = parse_write_path(runtime, path)?;
    let current_instance = match target
        .root_name()
        .ok_or_else(|| BoundaryError::UnsupportedPathSyntax {
            path: path.into(),
            reason: "assignment path must start with a declared name".to_string(),
        })
        .and_then(|root| classify_root(runtime, path, root.as_str()))?
    {
        RootResolution::Global => None,
        RootResolution::ProgramVar { instance_id } => Some(instance_id),
    };
    let registry = runtime.registry().clone();
    let profile = runtime.profile();
    crate::helper_eval::write_storage_lvalue(
        runtime.storage_mut(),
        &registry,
        &profile,
        current_instance,
        &target,
        value,
    )
    .map_err(|error| BoundaryError::from_runtime(path, error))
}

pub(crate) fn resolve_bind(
    runtime: &mut Runtime,
    path: &str,
    address: IoAddress,
) -> Result<(), BoundaryError> {
    if !is_simple_name(path) {
        return Err(BoundaryError::UnsupportedPathSyntax {
            path: path.into(),
            reason: "direct binding currently supports declared scalar names only".to_string(),
        });
    }

    if runtime.storage().get_global(path).is_some() {
        runtime.io_mut().bind(path, address);
        return Ok(());
    }
    if let Some((instance_id, _)) = find_program_var_instance(runtime, path)? {
        let Some(reference) = runtime
            .storage()
            .ref_for_instance_recursive(instance_id, path)
        else {
            return Err(BoundaryError::UndeclaredBinding { path: path.into() });
        };
        runtime.io_mut().bind_ref(reference, address);
        return Ok(());
    }

    Err(BoundaryError::UndeclaredBinding { path: path.into() })
}

fn parse_read_path(runtime: &Runtime, path: &str) -> Result<Expr, BoundaryError> {
    let mut registry = runtime.registry().clone();
    crate::harness::parse_debug_expression(path, &mut registry, runtime.profile(), &[]).map_err(
        |error| BoundaryError::UnsupportedPathSyntax {
            path: path.into(),
            reason: error.to_string(),
        },
    )
}

fn parse_write_path(runtime: &Runtime, path: &str) -> Result<LValue, BoundaryError> {
    let mut registry = runtime.registry().clone();
    crate::harness::parse_debug_lvalue(path, &mut registry, runtime.profile(), &[]).map_err(
        |error| BoundaryError::UnsupportedPathSyntax {
            path: path.into(),
            reason: error.to_string(),
        },
    )
}

fn classify_root(
    runtime: &Runtime,
    full_path: &str,
    root_name: &str,
) -> Result<RootResolution, BoundaryError> {
    if runtime.storage().get_global(root_name).is_some() {
        return Ok(RootResolution::Global);
    }
    if let Some((instance_id, _)) = find_program_var_instance(runtime, root_name)? {
        return Ok(RootResolution::ProgramVar { instance_id });
    }
    Err(BoundaryError::UnresolvedName {
        path: SmolStr::new(full_path),
    })
}

fn find_program_var_instance(
    runtime: &Runtime,
    name: &str,
) -> Result<Option<(InstanceId, SmolStr)>, BoundaryError> {
    let storage = runtime.storage();
    let mut match_entry = None;
    let mut candidates = Vec::new();
    for program in runtime.programs().values() {
        let Some(Value::Instance(id)) = storage.get_global(program.name.as_ref()) else {
            continue;
        };
        if storage.get_instance_var_recursive(*id, name).is_some() {
            candidates.push(SmolStr::new(format!("{}.{}", program.name, name)));
            match_entry = Some((*id, program.name.clone()));
        }
    }

    match candidates.len() {
        0 => Ok(None),
        1 => Ok(match_entry),
        _ => Err(BoundaryError::AmbiguousName {
            path: name.into(),
            candidates,
        }),
    }
}

fn expr_root_name(expr: &Expr) -> Option<&SmolStr> {
    match expr {
        Expr::Name(name) => Some(name),
        Expr::Index { target, .. } | Expr::Field { target, .. } => expr_root_name(target),
        Expr::Deref(_) | Expr::This | Expr::Super => None,
        Expr::Literal(_)
        | Expr::ArrayInitializer(_)
        | Expr::StructInitializer(_)
        | Expr::SizeOf(_)
        | Expr::Call { .. }
        | Expr::Unary { .. }
        | Expr::Binary { .. }
        | Expr::Ref(_) => None,
    }
}

fn is_simple_name(path: &str) -> bool {
    !path
        .chars()
        .any(|ch| matches!(ch, '.' | '[' | ']' | '(' | ')' | '^'))
}
