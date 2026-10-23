//! The starting values of a frame's variables (IEC §6.5.1, §6.5.2.1, Table 13 3d; specification
//! 44 §6.7): a function's or a method's result, outputs and locals, and a program's or a function
//! block's `VAR_TEMP`, start at each call at their declared initial value or at their type's
//! default, as the runtime starts them, and an input a call leaves out takes its default. A frame
//! variable is declared zero; the build computes its starting value at the program's layout, and
//! the call sets the parts of it that are not zero.

use trust_hir::TypeId;

use super::lower::{Lowerer, Place};
use super::model::Model;
use super::types::{Elem, Kind, Ty};
use super::NativeError;
use crate::program_model::Expr;

/// Starts the frame variable at `place`, declared as `name`, at the value the runtime starts it at
/// each call: its initial value `initializer`, else the default of its type, trust-hir's
/// `type_id`.
///
/// # Errors
///
/// [`NativeError`] for a starting value the generator does not compile: one that holds a
/// function block instance, or an initial value that names a variable.
pub(super) fn start(
    lowerer: &mut Lowerer<'_, '_>,
    place: &Place,
    type_id: TypeId,
    initializer: Option<&Expr>,
    name: &str,
) -> Result<(), NativeError> {
    let bytes = starting_bytes(lowerer.model, place.ty, type_id, initializer, name)?;
    let mut lines = Vec::new();
    statements(lowerer.model, &place.expr, place.ty, &bytes, 0, &mut lines)?;
    for line in lines {
        lowerer.line(&line);
    }
    Ok(())
}

/// The value an input a call leaves out takes, as the Rust argument of its scalar type `ty`: its
/// default `initializer`, else its type's default (IEC §6.6.1.2.2).
///
/// # Errors
///
/// [`NativeError`] for a default that names a variable.
pub(super) fn omitted_scalar(
    model: &mut Model<'_>,
    ty: Ty,
    type_id: TypeId,
    initializer: Option<&Expr>,
    name: &str,
) -> Result<String, NativeError> {
    let bytes = starting_bytes(model, ty, type_id, initializer, name)?;
    let Some(elem) = model.types.info(ty).scalar() else {
        return Err(NativeError::unsupported(format!(
            "the default of input {name}"
        )));
    };
    Ok(if elem == Elem::Bool {
        (bytes[0] != 0).to_string()
    } else {
        scalar(elem, &bytes)
    })
}

/// The bytes, at the program's layout, of the value a variable of `ty` starts at:
/// `initializer` evaluated by the build, when it names no variable, else its type's default.
///
/// # Errors
///
/// [`NativeError`] for a value that holds a function block instance, an initializer that names a
/// variable, or a value the runtime cannot construct.
pub(super) fn starting_bytes(
    model: &mut Model<'_>,
    ty: Ty,
    type_id: TypeId,
    initializer: Option<&Expr>,
    name: &str,
) -> Result<Vec<u8>, NativeError> {
    if holds_instance(model, ty, initializer.is_some()) {
        return Err(NativeError::unsupported(format!(
            "the starting value of {name}, which holds a function block instance"
        )));
    }
    let builder = model.builder;
    let abi = crate::value::StorageAbi::default();
    let value = match initializer {
        Some(expr) if names_nothing(expr) => {
            crate::harness::initializer::evaluate_initializer_with_abi(
                builder.storage(),
                builder.registry(),
                abi,
                builder.initializer_catalog(),
                &builder.profile(),
                None,
                builder.stdlib(),
                expr,
                type_id,
            )
        }
        Some(_) => {
            return Err(NativeError::unsupported(format!(
                "the initial value of {name}, which names a variable"
            )));
        }
        None => crate::harness::initializer::default_value_for_type_id_with_abi(
            builder.storage(),
            builder.registry(),
            abi,
            builder.initializer_catalog(),
            &builder.profile(),
            None,
            builder.stdlib(),
            type_id,
        ),
    }
    .map_err(|error| NativeError::Compile(format!("the starting value of {name}: {error}")))?;
    super::init::value_bytes(builder.storage(), model, &value, ty)
}

/// Whether a value of `ty` holds an instance of a function block whose starting state the build
/// does not compute: one of a user function block, or of a standard one given an initial value.
fn holds_instance(model: &Model<'_>, ty: Ty, initialized: bool) -> bool {
    match &model.types.info(ty).kind {
        Kind::Fb { .. } => true,
        Kind::StdFb(_) => initialized,
        Kind::Array { elem, .. } => holds_instance(model, *elem, initialized),
        Kind::Struct { members } => members
            .iter()
            .any(|member| holds_instance(model, member.ty, initialized)),
        _ => false,
    }
}

/// Whether `expr` names no variable: literals, aggregates of them, a repetition count and
/// operators over them.
fn names_nothing(expr: &Expr) -> bool {
    match expr {
        Expr::Literal(_) | Expr::SizeOf(_) => true,
        Expr::ArrayInitializer(items) => items.iter().all(names_nothing),
        Expr::StructInitializer(fields) => fields.iter().all(|(_, value)| names_nothing(value)),
        Expr::Unary { expr, .. } => names_nothing(expr),
        Expr::Binary { left, right, .. } => names_nothing(left) && names_nothing(right),
        // `4(9)` in an array's initial value repeats 9 four times.
        Expr::Call { target, args } => {
            matches!(**target, Expr::Literal(_))
                && args.iter().all(|arg| match &arg.value {
                    crate::program_model::ArgValue::Expr(value) => names_nothing(value),
                    crate::program_model::ArgValue::Target(_) => false,
                })
        }
        _ => false,
    }
}

/// Equal array elements at least this many in a row are set by one loop.
const RUN: usize = 4;

/// Appends to `out` the statements that set `place`, a frame variable of type `ty` that holds
/// zero, to the value whose bytes at the program's layout are `bytes`; `depth` names the loop
/// variables of nested arrays apart.
///
/// # Errors
///
/// [`NativeError`] for a reference or an interface that is not `NULL`, which no frame variable
/// starts at.
pub(super) fn statements(
    model: &Model<'_>,
    place: &str,
    ty: Ty,
    bytes: &[u8],
    depth: usize,
    out: &mut Vec<String>,
) -> Result<(), NativeError> {
    if bytes.iter().all(|byte| *byte == 0) {
        return Ok(());
    }
    let info = model.types.info(ty);
    match &info.kind {
        Kind::Elem(elem) | Kind::Enum { base: elem, .. } | Kind::Subrange { base: elem, .. } => {
            out.push(format!("{place}.set({});", scalar(*elem, bytes)));
        }
        Kind::Str(_) => {
            let length = usize::from(u16::from_le_bytes([bytes[0], bytes[1]]));
            out.push(format!(
                "lib::assign_bytes(({place}).view(), {});",
                super::lower::byte_string(&bytes[2..2 + length])
            ));
        }
        Kind::WStr(_) => {
            return Err(NativeError::unsupported(format!(
                "a WSTRING frame variable {place} that starts with a text"
            )));
        }
        Kind::Array { elem, dims } => {
            array(model, place, *elem, dims, bytes, depth, out)?;
        }
        Kind::Struct { members } | Kind::Fb { members, .. } => {
            for member in members {
                let size = model.types.info(member.ty).size as usize;
                let start = member.offset as usize;
                statements(
                    model,
                    &format!("{place}.{}", member.field()),
                    member.ty,
                    &bytes[start..start + size],
                    depth,
                    out,
                )?;
            }
        }
        Kind::StdFb(fb) => {
            // The library's fields are the variables' names without the hidden ones' `__`.
            for (name, elem, offset, _) in fb.members() {
                let part = &bytes[offset..offset + elem.size() as usize];
                if part.iter().any(|byte| *byte != 0) {
                    out.push(format!(
                        "{place}.{}.set({});",
                        name.trim_start_matches("__"),
                        scalar(elem, part)
                    ));
                }
            }
        }
        Kind::Interface | Kind::Ref { .. } => {
            return Err(NativeError::unsupported(format!(
                "a frame variable {place} that starts at a reference"
            )));
        }
    }
    Ok(())
}

/// The elements of an array of `dims` over `elem`, one dimension at a time, equal elements in a
/// row set by a loop.
fn array(
    model: &Model<'_>,
    place: &str,
    elem: Ty,
    dims: &[(i64, i64)],
    bytes: &[u8],
    depth: usize,
    out: &mut Vec<String>,
) -> Result<(), NativeError> {
    let Some(((lower, upper), inner)) = dims.split_first() else {
        return statements(model, place, elem, bytes, depth, out);
    };
    let length = usize::try_from(upper - lower + 1).unwrap_or(0);
    if length == 0 {
        return Ok(());
    }
    let block = bytes.len() / length;
    let blocks: Vec<&[u8]> = bytes.chunks_exact(block).collect();
    let mut index = 0;
    while index < length {
        let mut end = index + 1;
        while end < length && blocks[end] == blocks[index] {
            end += 1;
        }
        if end - index >= RUN && blocks[index].iter().any(|byte| *byte != 0) {
            let item = format!("e{depth}");
            let mut body = Vec::new();
            array(
                model,
                &item,
                elem,
                inner,
                blocks[index],
                depth + 1,
                &mut body,
            )?;
            out.push(format!("for {item} in &{place}[{index}..{end}] {{"));
            out.extend(body.into_iter().map(|line| format!("    {line}")));
            out.push("}".into());
        } else {
            for (offset, block) in blocks[index..end].iter().enumerate() {
                array(
                    model,
                    &format!("{place}[{}]", index + offset),
                    elem,
                    inner,
                    block,
                    depth,
                    out,
                )?;
            }
        }
        index = end;
    }
    Ok(())
}

/// The Rust literal of the scalar of `elem` in the first bytes of `bytes`, as its cell stores it.
fn scalar(elem: Elem, bytes: &[u8]) -> String {
    let size = elem.size() as usize;
    let mut wide = [0u8; 8];
    wide[..size].copy_from_slice(&bytes[..size]);
    let raw = u64::from_le_bytes(wide);
    let rust = elem.stored();
    match elem {
        Elem::Real => format!("f32::from_bits({raw:#010x})"),
        Elem::LReal => format!("f64::from_bits({raw:#018x})"),
        _ if rust.starts_with('i') => {
            let shift = 64 - 8 * size;
            #[allow(clippy::cast_possible_wrap)]
            let value = ((raw << shift) as i64) >> shift;
            format!("{value}{rust}")
        }
        _ => format!("{raw}{rust}"),
    }
}
