//! A command's write on the runtime thread (specification 39, §12.2.4, §12.8): the point must be
//! a variable of the program that may be written (the binding checks' own rule: not a constant,
//! not a function block output), the value is converted to its type, and the value read back
//! after the write is returned with the value before it.

use serde_json::{json, Value as Json};

use crate::hmi::binding_checks::{global_write_block, member_write_block};
use crate::hmi::member_paths::{resolve_member, resolve_member_with, MemberPath, PathSegment};
use crate::hmi::write_value::value_for_target_in;
use crate::runtime::RuntimeMetadata;
use crate::value::Value;
use crate::Runtime;

#[cfg(test)]
mod tests;

/// What a command does to its point.
#[derive(Debug, Clone, PartialEq)]
pub enum WriteAction {
    /// Writes this value, converted to the point's type.
    Set(Json),
    /// Writes the opposite of a BOOL's current value.
    Toggle,
    /// Writes a selector's option: its name to an enumeration, its position (from 0) to a
    /// number (§5.8).
    Choice {
        /// The option as the control lists it.
        name: String,
        /// Its position in the list.
        index: usize,
    },
}

/// The point before and after a write.
#[derive(Debug, Clone, PartialEq)]
pub struct Written {
    /// The value before the write.
    pub old: Value,
    /// The value read back after the write.
    pub readback: Value,
}

/// A write checked and converted, not made yet.
struct Prepared {
    /// The point as written, which the boundary's writer reads.
    shown: String,
    old: Value,
    value: Value,
}

/// Writes a point, or says why it cannot be written.
pub fn write_point(
    runtime: &mut Runtime,
    path: &MemberPath,
    action: &WriteAction,
) -> Result<Written, String> {
    let metadata = runtime.metadata_snapshot();
    let prepared = prepare(runtime, &metadata, path, action)?;
    commit(runtime, path, prepared)
}

/// Writes points together (specification 39, §21.4 item 4): every write is checked and converted
/// before any is made, so when one cannot be made none is; a write that fails after others were
/// made puts those back. The program sees all of them, or none, at its next scan.
pub fn write_points(
    runtime: &mut Runtime,
    writes: &[(MemberPath, WriteAction)],
) -> Result<Vec<Written>, String> {
    let metadata = runtime.metadata_snapshot();
    let prepared = writes
        .iter()
        .map(|(path, action)| prepare(runtime, &metadata, path, action))
        .collect::<Result<Vec<_>, _>>()?;
    let mut made: Vec<(String, Written)> = Vec::with_capacity(prepared.len());
    for ((path, _), write) in writes.iter().zip(prepared) {
        let shown = write.shown.clone();
        match commit(runtime, path, write) {
            Ok(written) => made.push((shown, written)),
            Err(reason) => {
                for (shown, written) in made.iter().rev() {
                    let _ = crate::boundary::resolve_write(runtime, shown, written.old.clone());
                }
                return Err(reason);
            }
        }
    }
    Ok(made.into_iter().map(|(_, written)| written).collect())
}

/// The point's value now and the value to write, or why it cannot be written: the point must be
/// a variable of the program that may be written, and the value must fit its type.
fn prepare(
    runtime: &Runtime,
    metadata: &RuntimeMetadata,
    path: &MemberPath,
    action: &WriteAction,
) -> Result<Prepared, String> {
    let shown = path_text(path);
    let mut block = global_write_block(metadata, &path.root);
    let old = resolve_member_with(path, runtime.storage(), |type_name, member| {
        if block.is_none() {
            block = member_write_block(metadata, type_name, member);
        }
    })
    .cloned()
    .ok_or_else(|| format!("\"{shown}\" is not a variable of the program"))?;
    if let Some(reason) = block {
        return Err(format!("\"{shown}\" {reason} and cannot be written"));
    }
    let value = match action {
        WriteAction::Set(requested) => value_for_target_in(metadata.registry(), &old, requested)?,
        WriteAction::Choice { name, index } => match old {
            Value::Enum(_) => value_for_target_in(metadata.registry(), &old, &json!(name))?,
            _ => value_for_target_in(metadata.registry(), &old, &json!(index))?,
        },
        WriteAction::Toggle => match old {
            Value::Bool(state) => Value::Bool(!state),
            _ => {
                return Err(format!(
                    "\"{shown}\" is not a BOOL, so it cannot be toggled"
                ))
            }
        },
    };
    Ok(Prepared { shown, old, value })
}

/// Makes a prepared write and reads the point back.
fn commit(runtime: &mut Runtime, path: &MemberPath, write: Prepared) -> Result<Written, String> {
    let Prepared { shown, old, value } = write;
    crate::boundary::resolve_write(runtime, &shown, value).map_err(|error| error.to_string())?;
    let readback = resolve_member(path, runtime.storage())
        .cloned()
        .ok_or_else(|| format!("\"{shown}\" could not be read back"))?;
    Ok(Written { old, readback })
}

/// A path as written, which the boundary's writer reads.
pub fn path_text(path: &MemberPath) -> String {
    let mut text = path.root.to_string();
    for segment in &path.segments {
        match segment {
            PathSegment::Field(name) => {
                text.push('.');
                text.push_str(name);
            }
            PathSegment::Index(indices) => {
                let list = indices
                    .iter()
                    .map(i64::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                text.push('[');
                text.push_str(&list);
                text.push(']');
            }
        }
    }
    text
}
