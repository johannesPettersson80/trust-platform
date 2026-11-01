//! A command's write on the runtime thread (specification 39, §12.2.4, §12.8): the point must be
//! a variable of the program that may be written (the binding checks' own rule: not a constant,
//! not a function block output), the value is converted to its type and must fit it, a text
//! within its capacity, and the value read back after the write is returned with the value before
//! it. The point is found and written through the running program's symbol and type tables
//! (specification 44 §13.3), at a boundary of its scan.

use std::fmt;

use serde_json::{json, Value as Json};
use smol_str::SmolStr;
use trust_runtime_core::native::image::{
    type_kind, Image, MemberDetail, SectionId, TypeRecord, MEMBER_BYTES, SYMBOL_CONSTANT,
    SYMBOL_OUTPUT, SYMBOL_PROGRAM,
};
use trust_runtime_core::native::memory::{
    enum_value, enum_values, type_name, type_shape, TypeShape,
};
use trust_runtime_core::native::reflect::type_base;

use crate::error::StableErrorCode;
use crate::hmi::member_paths::{locate_member, MemberPath, PathSegment};
use crate::hmi::write_value::value_for_target_with;
use crate::runtime::program_access::{Located, ProgramRead, ProgramWrite};
use crate::value::Value;

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

/// A write the runtime refused (specification 39, §12.8 item 5): why, in words, and the stable
/// code of a refusal that has one. The refusal reads as its reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteRefused {
    /// Why the point was not written.
    pub reason: String,
    /// `runtime_string_capacity_exceeded` for a text longer than the capacity of its point.
    pub error_code: Option<&'static str>,
    /// The runtime on which a named account must log in before the command is sent: the owner of a
    /// remote point that holds no session for the operator (specification 39, remote operation).
    pub login: Option<String>,
}

impl WriteRefused {
    /// The owner `runtime` of a remote point holds no session for the operator: the command was not
    /// sent, and the refusal names the runtime to log in to. Its reason, `owner-login:<runtime>`, is
    /// what the audit records.
    pub fn owner_login(runtime: &str) -> Self {
        Self {
            reason: format!("owner-login:{runtime}"),
            error_code: None,
            login: Some(runtime.to_string()),
        }
    }
}

impl From<String> for WriteRefused {
    fn from(reason: String) -> Self {
        Self {
            reason,
            error_code: None,
            login: None,
        }
    }
}

impl From<&str> for WriteRefused {
    fn from(reason: &str) -> Self {
        Self::from(reason.to_string())
    }
}

impl fmt::Display for WriteRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.reason)
    }
}

impl std::ops::Deref for WriteRefused {
    type Target = str;

    fn deref(&self) -> &str {
        &self.reason
    }
}

/// A write checked and converted, not made yet.
struct Prepared {
    /// The point as written, which a refusal names.
    shown: String,
    at: Located,
    old: Value,
    value: Value,
}

/// Writes a point of the running program at a boundary of its scan, or says why it cannot be
/// written.
///
/// # Errors
///
/// The [`WriteRefused`] of a point that is no variable of the program, may not be written from
/// outside it, or does not take the value; the point keeps its value then.
pub fn write_point(
    program: &mut dyn ProgramWrite,
    path: &MemberPath,
    action: &WriteAction,
) -> Result<Written, WriteRefused> {
    let prepared = prepare(&*program, path, action)?;
    commit(program, prepared)
}

/// Writes points together (specification 39, §21.4 item 4): every write is checked and converted
/// before any is made, so when one cannot be made none is; a write that fails after others were
/// made puts those back. The program sees all of them, or none, at its next execution.
///
/// # Errors
///
/// The [`WriteRefused`] of the first point that cannot be written; every point keeps its value
/// then.
pub fn write_points(
    program: &mut dyn ProgramWrite,
    writes: &[(MemberPath, WriteAction)],
) -> Result<Vec<Written>, WriteRefused> {
    let prepared = writes
        .iter()
        .map(|(path, action)| prepare(&*program, path, action))
        .collect::<Result<Vec<_>, _>>()?;
    let mut made: Vec<(Located, Written)> = Vec::with_capacity(prepared.len());
    for write in prepared {
        let at = write.at;
        match commit(program, write) {
            Ok(written) => made.push((at, written)),
            Err(refused) => {
                for (at, written) in made.iter().rev() {
                    // The value was read from this place a moment ago, so it is taken again.
                    let _ = program.write_at(at, &written.old);
                }
                return Err(refused);
            }
        }
    }
    Ok(made.into_iter().map(|(_, written)| written).collect())
}

/// The point's place, its value now and the value to write, or why it cannot be written: the
/// point must be a variable of the program that may be written, and the value must fit its type.
fn prepare(
    program: &dyn ProgramRead,
    path: &MemberPath,
    action: &WriteAction,
) -> Result<Prepared, WriteRefused> {
    let shown = path_text(path);
    let at = locate_member(path, program)
        .ok_or_else(|| format!("\"{shown}\" is not a variable of the program"))?;
    if let Some(reason) = write_block(program, path) {
        return Err(format!("\"{shown}\" {reason} and cannot be written").into());
    }
    let old = program.read_at(&at).map_err(|_| {
        format!(
            "a {} cannot be written from the HMI",
            program.type_name_of(&at)
        )
    })?;
    let image = program.program_image();
    let values_of = |name: &str| enumeration_values(&image, at.type_index, name);
    let value = match action {
        WriteAction::Set(requested) => value_for_target_with(&old, requested, values_of)?,
        WriteAction::Choice { name, index } => match old {
            Value::Enum(_) => value_for_target_with(&old, &json!(name), values_of)?,
            _ => value_for_target_with(&old, &json!(index), values_of)?,
        },
        WriteAction::Toggle => match old {
            Value::Bool(state) => Value::Bool(!state),
            _ => return Err(format!("\"{shown}\" is not a BOOL, so it cannot be toggled").into()),
        },
    };
    fits_capacity(&image, at.type_index, &value, &shown)?;
    Ok(Prepared {
        shown,
        at,
        old,
        value,
    })
}

/// Makes a prepared write and reads the point back.
fn commit(program: &mut dyn ProgramWrite, write: Prepared) -> Result<Written, WriteRefused> {
    let Prepared {
        shown,
        at,
        old,
        value,
    } = write;
    program
        .write_at(&at, &value)
        .map_err(|error| WriteRefused::from(error.to_string()))?;
    let readback = program
        .read_at(&at)
        .map_err(|_| format!("\"{shown}\" could not be read back"))?;
    Ok(Written { old, readback })
}

/// A text fits its point, a variable or a method's input, of type `type_index` when it is no
/// longer than the capacity of the point's `STRING[n]` or `WSTRING[n]`, counted in bytes of UTF-8
/// for a `STRING` and in UTF-16 code units for a `WSTRING`; a declaration without a length is
/// `STRING[80]` or `WSTRING[80]` (specification 39, §12.8 item 5, §21.2 item 5). A longer text is
/// refused, never cut; the refusal names the point as `shown`.
pub(crate) fn fits_capacity(
    image: &Image<'_>,
    type_index: u32,
    value: &Value,
    shown: &str,
) -> Result<(), WriteRefused> {
    let Ok(TypeShape::Text { capacity, .. }) = type_shape(image, type_index) else {
        return Ok(());
    };
    let (length, unit) = match value {
        Value::String(text) => (text.len(), "bytes of UTF-8"),
        Value::WString(text) => (text.encode_utf16().count(), "UTF-16 code units"),
        _ => return Ok(()),
    };
    if u64::try_from(length).unwrap_or(u64::MAX) <= u64::from(capacity) {
        return Ok(());
    }
    Err(WriteRefused {
        reason: format!("\"{shown}\" holds at most {capacity} {unit}; the text has {length}"),
        error_code: Some(StableErrorCode::RuntimeStringCapacityExceeded.as_str()),
        login: None,
    })
}

/// The names and numbers of the values of the enumeration of type `type_index`, named `name`.
pub(crate) fn enumeration_values(
    image: &Image<'_>,
    type_index: u32,
    name: &str,
) -> Option<Vec<(SmolStr, i64)>> {
    let names = enum_values(image, type_index);
    if names.is_empty() {
        return None;
    }
    names
        .into_iter()
        .map(|variant| match enum_value(image, name, &variant)? {
            Value::Enum(value) => Some((variant, value.numeric_value())),
            _ => None,
        })
        .collect()
}

/// Why `path` cannot be written from outside the program, if it cannot, as the binding checks
/// decide it (§7.1): a constant global, a constant of a program or a function block, or an output
/// of a function block or of a block it extends. The first such part of the path decides.
pub(crate) fn write_block(program: &dyn ProgramRead, path: &MemberPath) -> Option<String> {
    let image = program.program_image();
    let (_, root) = image.find_symbol(&path.root.to_ascii_uppercase())?;
    if root.flags & SYMBOL_PROGRAM == 0 && root.flags & SYMBOL_CONSTANT != 0 {
        return Some("is a constant".to_string());
    }
    let mut current = program.locate(&path.root).ok()?;
    for segment in &path.segments {
        current = match segment {
            PathSegment::Field(name) => {
                if let Some(reason) = member_block(&image, current.type_index, name) {
                    return Some(reason);
                }
                program.member(&current, name)?
            }
            PathSegment::Index(indices) => program.element(&current, indices)?,
        };
    }
    None
}

/// Why the member `name` of an instance of type `type_index` cannot be written from outside it:
/// a function block's output names the block that declares it.
fn member_block(image: &Image<'_>, type_index: u32, name: &str) -> Option<String> {
    let kind = image.table::<TypeRecord>().ok()?.get(type_index)?.kind;
    let flags = member_flags(image, type_index, name)?;
    match kind {
        type_kind::FUNCTION_BLOCK if flags & u32::from(SYMBOL_OUTPUT) != 0 => Some(format!(
            "is an output of {}",
            declaring_type(image, type_index, name)
        )),
        type_kind::FUNCTION_BLOCK | type_kind::PROGRAM
            if flags & u32::from(SYMBOL_CONSTANT) != 0 =>
        {
            Some("is a constant".to_string())
        }
        _ => None,
    }
}

/// The `SYMBOL_` flags of the member `name` of type `type_index`, as its type record lists them
/// (specification 44 §5.8), compared without case; `None` for a type without such a member.
/// The members of a derived block include those it inherits.
fn member_flags(image: &Image<'_>, type_index: u32, name: &str) -> Option<u32> {
    let record = image.table::<TypeRecord>().ok()?.get(type_index)?;
    if !matches!(
        record.kind,
        type_kind::FUNCTION_BLOCK | type_kind::CLASS | type_kind::PROGRAM
    ) {
        return None;
    }
    let start = usize::try_from(record.detail).ok()?;
    let length = usize::try_from(record.count)
        .ok()?
        .checked_mul(MEMBER_BYTES)?;
    let details = image
        .section_bytes(SectionId::Types)
        .get(start..start.checked_add(length)?)?;
    details
        .as_chunks::<MEMBER_BYTES>()
        .0
        .iter()
        .map(|member| MemberDetail::read(member))
        .find(|member| {
            image.string(member.name).is_some_and(|declared| {
                !declared.starts_with("__") && declared.eq_ignore_ascii_case(name)
            })
        })
        .map(|member| member.flags)
}

/// The block that declares the member `name` of type `type_index`: the type itself, or the base
/// furthest down its chain that has the member.
fn declaring_type(image: &Image<'_>, type_index: u32, name: &str) -> SmolStr {
    let mut owner = type_index;
    // Inheritance chains are short; the bound only guards against a malformed table.
    for _ in 0..32 {
        match type_base(image, owner) {
            Ok(Some(base)) if member_flags(image, base, name).is_some() => owner = base,
            _ => break,
        }
    }
    type_name(image, owner)
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
