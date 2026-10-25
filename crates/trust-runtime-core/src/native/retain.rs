//! The retained variables of a compiled program (specification 44 §5.10, §11; specification 11
//! §6.7): their checkpoint, written from their areas through the retain map and the type table,
//! and a checkpoint restored by name into them.
//!
//! A capture visits the retain map in its order and writes each variable under the key the map
//! names, specification 11 §6.7's: a global by its declared name, a program variable as
//! `@program/<program>/<variable>` and a variable of a function block or class instance by its
//! path, with the bytes the STRN v3 payload codec writes for the same value: a scalar's tag and its
//! bytes, a text as UTF-8, an enumeration by its type and value names, an array with its bounds, a
//! structure with its type and member names. No `Value` is built, so a capture into a reserved
//! payload allocates nothing (specification 34 MEM-04). A variable whose type holds an instance or
//! a reference is left out of the checkpoint, as the interpreter leaves it out. A checkpoint written
//! by the interpreter therefore restores here, and one written here restores into the interpreter.
//!
//! A restore applies a checkpoint by name, with the interpreter's migration rules: a safe widening
//! of a number, a text cut to its capacity, an enumeration rebuilt with its declared spelling, a
//! structure rebuilt member by member, a member the checkpoint lacks at its starting value, a key
//! nothing declares reported as an orphan. Each entry's new bytes are worked out and checked before
//! any area changes, so one value the program refuses leaves every retained variable as it was.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_plc_abi::{AREA_GLOBALS, AREA_INSTANCES, AREA_RETAIN};

use super::image::{type_kind as kind, AreaRecord, Image, RetainRecord, SectionId, AREA_HAS_INIT};
use super::values::{
    damaged, read, read_unsigned, read_value, text_length, variant_of, wide_length, wide_units,
    width, AreaBytes, Types, Writer,
};
use crate::error::RuntimeError;
use crate::native::values::MAX_TYPE_NESTING;
use crate::retain::codec::{
    encode_array_header, encode_enumeration, encode_string, encode_struct_header, PayloadWriter,
    ValueTag, NO_PHYSICAL_CHECKPOINT,
};
use crate::retain::RetainApplyEvent;
use crate::retain::RetainSnapshot;

/// The bytes of a retained variable: its area, its offset in the area and its bytes. A restore
/// works them out; a warm start keeps them across the start.
#[derive(Debug, PartialEq, Eq)]
pub struct Kept {
    /// The area: globals, retain or instances.
    pub area: u8,
    /// The offset in the area.
    pub offset: usize,
    /// The variable's bytes.
    pub bytes: Vec<u8>,
}

/// Appends the checkpoint entries of the retained variables to `payload`, in the retain map's
/// order, each read from its area as `area` gives it (specification 44 §5.10, §11).
///
/// # Errors
///
/// [`RuntimeError::RetainStore`] for a value no checkpoint holds: a non-finite `REAL` or `LREAL`, a
/// text that is not UTF-8 or UTF-16, an enumeration number its type does not declare.
/// [`RuntimeError::InvalidBundle`] for a retain map or a type table that does not read.
pub(crate) fn fill<'a, A: AreaBytes + ?Sized + 'a>(
    image: &Image<'_>,
    area: impl Fn(u8) -> Option<&'a A>,
    payload: &mut PayloadWriter<'_>,
) -> Result<(), RuntimeError> {
    let types = Types::of(image)?;
    each_retained(image, |retained| {
        if !retainable(&types, retained.type_index, 0)? {
            return Ok(());
        }
        let bytes = area(retained.area).ok_or_else(|| damaged("retain map"))?;
        payload.entry_with(&[retained.key], |out| {
            encode(&types, bytes, retained.offset, retained.type_index, out, 0)
        })
    })
}

/// The most bytes a capture of `image`'s retained variables appends, the entry count included: each retained variable
/// whose type holds no instance, under its key, at the largest encoding of its type. A capture
/// reserves this much so that no capture grows it (specification 34 MEM-09).
///
/// # Errors
///
/// [`RuntimeError::InvalidBundle`] for a retain map or a type table that does not read,
/// [`RuntimeError::TypeMismatch`] for a retained type no checkpoint can hold, and
/// [`RuntimeError::Overflow`] when the bound exceeds the address range.
pub fn payload_bound(image: &Image<'_>) -> Result<usize, RuntimeError> {
    let types = Types::of(image)?;
    let mut total = 4usize;
    each_retained(image, |retained| {
        if retainable(&types, retained.type_index, 0)? {
            total = sum(&[
                total,
                4,
                retained.key.len(),
                bound(&types, retained.type_index, 0)?,
            ])?;
        }
        Ok(())
    })?;
    Ok(total)
}

/// The new bytes of the retained variables with the checkpoint `snapshot` restored into what
/// `current` gives of each area, by name, and the events of the restore in the checkpoint's order
/// (specification 11 §6.7; specification 44 §11).
///
/// Each entry goes to the retained variable its key names, without regard to case. A value is
/// converted to the variable's type by the interpreter's rules: a number widened within its family,
/// a `REAL` to an `LREAL`, a text cut to the capacity, an enumeration of the same type and value
/// rebuilt with the declared spelling, an array of the same bounds element by element, a structure
/// member by member, where a member the value lacks takes its starting value and a member the type
/// lacks is dropped. A converted value is reported with
/// [`RetainApplyEvent::Migrated`]; a key that names nothing is reported with
/// [`RetainApplyEvent::Orphan`]; a key that names a variable without `RETAIN` leaves it alone. A
/// retained variable the checkpoint does not name keeps what `current` holds.
///
/// # Errors
///
/// [`RuntimeError::RetainSubrangeViolation`] for a value outside its subrange,
/// [`RuntimeError::RetainMigration`] for any other value the variable's type refuses or two
/// retained variables with one key, and [`RuntimeError::InvalidBundle`] for tables that do not
/// read. Nothing is returned then, so the caller's areas keep their values.
pub fn stage<'a>(
    image: &Image<'_>,
    snapshot: &RetainSnapshot,
    current: impl Fn(u8) -> Option<&'a [u8]>,
) -> Result<(Vec<Kept>, Vec<RetainApplyEvent>), RuntimeError> {
    let types = Types::of(image)?;
    let mut destinations = BTreeMap::new();
    let mut variables = Vec::new();
    each_retained(image, |retained| {
        if destinations
            .insert(retained.key.to_ascii_uppercase(), variables.len())
            .is_some()
        {
            return Err(RuntimeError::RetainMigration(
                "ambiguous retained destination label".into(),
            ));
        }
        variables.push((retained.area, retained.offset, retained.type_index));
        Ok(())
    })?;
    let mut starting: BTreeMap<u8, Vec<u8>> = BTreeMap::new();
    let mut staged = Vec::new();
    let mut events = Vec::new();
    for (key, value) in snapshot.values() {
        let Some(&index) = destinations.get(&key.to_ascii_uppercase()) else {
            if !names_variable(image, &types, key) {
                events.push(RetainApplyEvent::Orphan { name: key.clone() });
            }
            continue;
        };
        let (area, offset, type_index) = variables[index];
        // A variable that holds an instance or a reference is never restored (specification 10).
        if !retainable(&types, type_index, 0)? {
            continue;
        }
        let size =
            usize::try_from(types.get(type_index)?.size).map_err(|_| RuntimeError::Overflow)?;
        let end = offset.checked_add(size).ok_or(RuntimeError::Overflow)?;
        let present = current(area)
            .and_then(|bytes| bytes.get(offset..end))
            .ok_or_else(|| damaged("retain map"))?;
        if let alloc::collections::btree_map::Entry::Vacant(entry) = starting.entry(area) {
            entry.insert(starting_bytes(image, area)?);
        }
        let start = starting
            .get(&area)
            .and_then(|bytes| bytes.get(offset..end))
            .ok_or_else(|| damaged("retain map"))?;
        let mut bytes = present.to_vec();
        let mut writer = Writer {
            types: &types,
            staged: &mut bytes,
            starting: start,
        };
        writer
            .write(type_index, 0, value, 0)
            .map_err(|error| migration_error(key, error))?;
        let after = read_value(&types, bytes.as_slice(), 0, type_index)
            .map_err(|error| migration_error(key, error))?;
        if &after != value {
            events.push(RetainApplyEvent::Migrated {
                name: key.clone(),
                before: value.clone(),
                after,
            });
        }
        staged.push(Kept {
            area,
            offset,
            bytes,
        });
    }
    Ok((staged, events))
}

/// Whether `snapshot` restores into a fresh start of `image`, as its warm start after a power-on
/// restores it: the check of an install (specification 34 §11.2, decision G13). The events are
/// those of that restore.
///
/// # Errors
///
/// As [`stage`].
pub fn check(
    image: &Image<'_>,
    snapshot: &RetainSnapshot,
) -> Result<Vec<RetainApplyEvent>, RuntimeError> {
    let mut starting: BTreeMap<u8, Vec<u8>> = BTreeMap::new();
    for area in [AREA_GLOBALS, AREA_RETAIN, AREA_INSTANCES] {
        starting.insert(area, starting_bytes(image, area)?);
    }
    stage(image, snapshot, |area| {
        starting.get(&area).map(Vec::as_slice)
    })
    .map(|(_, events)| events)
}

/// The bytes of every retained variable the retain map names whose type a checkpoint holds, read
/// from what `area` gives of each area: what a warm start keeps while every other variable, an
/// instance or a reference in the retain area too, takes its starting value (specification 44
/// §6.7, §11).
///
/// # Errors
///
/// [`RuntimeError::InvalidBundle`] for a retain map or a type table that does not read.
pub(crate) fn retained_bytes<'a, A: AreaBytes + ?Sized + 'a>(
    image: &Image<'_>,
    area: impl Fn(u8) -> Option<&'a A>,
) -> Result<Vec<Kept>, RuntimeError> {
    let types = Types::of(image)?;
    let mut kept = Vec::new();
    each_retained(image, |retained| {
        if !retainable(&types, retained.type_index, 0)? {
            return Ok(());
        }
        let size = usize::try_from(types.get(retained.type_index)?.size)
            .map_err(|_| RuntimeError::Overflow)?;
        let mut bytes = vec![0u8; size];
        if !area(retained.area)
            .ok_or_else(|| damaged("retain map"))?
            .read(retained.offset, &mut bytes)
        {
            return Err(damaged("retain map"));
        }
        kept.push(Kept {
            area: retained.area,
            offset: retained.offset,
            bytes,
        });
        Ok(())
    })?;
    Ok(kept)
}

/// The error of a value the variable `key` refuses, as the interpreter reports it: a subrange
/// violation with its numbers, an exhausted budget as it is, anything else as a migration error
/// that names the key.
fn migration_error(key: &SmolStr, error: RuntimeError) -> RuntimeError {
    match error {
        RuntimeError::SubrangeViolation {
            value,
            lower,
            upper,
        } => RuntimeError::RetainSubrangeViolation {
            name: key.clone(),
            value,
            lower,
            upper,
        },
        RuntimeError::Overflow | RuntimeError::InvalidBundle(_) => error,
        _ => RuntimeError::RetainMigration(
            format!("invalid retained value for '{key}': {error}").into(),
        ),
    }
}

/// The starting bytes of the area `id` of `image`, zero where `INIT` gives none; none for an area
/// the image does not have.
fn starting_bytes(image: &Image<'_>, id: u8) -> Result<Vec<u8>, RuntimeError> {
    let areas = image
        .table::<AreaRecord>()
        .map_err(|_| damaged("area table"))?;
    let Some(area) = areas.iter().find(|area| area.id == u32::from(id)) else {
        return Ok(Vec::new());
    };
    let length = usize::try_from(area.length).map_err(|_| RuntimeError::Overflow)?;
    let mut bytes = vec![0u8; length];
    if area.flags & AREA_HAS_INIT != 0 {
        let start = usize::try_from(area.init_offset).map_err(|_| RuntimeError::Overflow)?;
        let init = start
            .checked_add(length)
            .and_then(|end| image.section_bytes(SectionId::Init).get(start..end))
            .ok_or_else(|| damaged("starting memory"))?;
        bytes.copy_from_slice(init);
    }
    Ok(bytes)
}

/// Whether `key` names a variable that `image` declares, retained or not: a global by its name, a
/// program's variable as `@program/<program>/<variable>`, or a variable of an instance by its path,
/// followed through the symbol table and, past an array element, through the type table. A restore
/// leaves a variable declared without `RETAIN` alone and reports no orphan for it, as the
/// interpreter does (specification 11 §6.7).
fn names_variable(image: &Image<'_>, types: &Types<'_>, key: &str) -> bool {
    let folded = key.to_ascii_uppercase();
    let path = match folded.strip_prefix("@PROGRAM/") {
        Some(rest) => match rest.split_once('/') {
            Some((program, variable)) if !variable.contains('/') => {
                format!("{program}.{variable}")
            }
            _ => return false,
        },
        None => folded,
    };
    let (head, mut rest) = path.split_at(path.find('[').unwrap_or(path.len()));
    let Some((_, symbol)) = image.find_symbol(head) else {
        return false;
    };
    let mut ty = symbol.type_index;
    while !rest.is_empty() {
        let Ok(record) = types.get(ty) else {
            return false;
        };
        if let Some(after) = rest.strip_prefix('[') {
            let Some((indexes, tail)) = after.split_once(']') else {
                return false;
            };
            if record.kind != kind::ARRAY {
                return false;
            }
            let Ok((element, mut dimensions)) = types.array(&record) else {
                return false;
            };
            let mut parts = indexes.split(',');
            let inside = dimensions.all(|(lower, upper)| {
                parts
                    .next()
                    .and_then(|part| part.trim().parse::<i64>().ok())
                    .is_some_and(|index| (lower..=upper).contains(&index))
            });
            if !inside || parts.next().is_some() {
                return false;
            }
            ty = element;
            rest = tail;
        } else if let Some(after) = rest.strip_prefix('.') {
            let end = after.find(['.', '[']).unwrap_or(after.len());
            let (name, tail) = after.split_at(end);
            let Ok(mut members) = types.members(&record) else {
                return false;
            };
            let Some(member) = members.find(|member| {
                types
                    .string(member.name)
                    .is_ok_and(|member| member.eq_ignore_ascii_case(name))
            }) else {
                return false;
            };
            ty = member.type_index;
            rest = tail;
        } else {
            return false;
        }
    }
    true
}

/// The sum of `values`, or [`RuntimeError::Overflow`].
fn sum(values: &[usize]) -> Result<usize, RuntimeError> {
    values.iter().try_fold(0usize, |total, value| {
        total.checked_add(*value).ok_or(RuntimeError::Overflow)
    })
}

/// A variable of the retain map: its key, its area, its type and its offset in the area.
struct Retained<'i> {
    key: &'i str,
    area: u8,
    type_index: u32,
    offset: usize,
}

/// Visits the variables of the retain map in its order (specification 44 §5.10).
fn each_retained<'i>(
    image: &Image<'i>,
    mut visit: impl FnMut(Retained<'i>) -> Result<(), RuntimeError>,
) -> Result<(), RuntimeError> {
    let map = image
        .table::<RetainRecord>()
        .map_err(|_| damaged("retain map"))?;
    for record in map.iter() {
        let key = image
            .string(record.key)
            .ok_or_else(|| damaged("retain map"))?;
        let area = u8::try_from(record.area)
            .ok()
            .filter(|area| matches!(*area, AREA_GLOBALS | AREA_RETAIN | AREA_INSTANCES))
            .ok_or_else(|| damaged("retain map"))?;
        visit(Retained {
            key,
            area,
            type_index: record.type_index,
            offset: usize::try_from(record.offset).map_err(|_| RuntimeError::Overflow)?,
        })?;
    }
    Ok(())
}

/// Whether a value of type `index` may go into a checkpoint: not when it holds a function block,
/// class or program instance, an interface, a reference or a pointer, at any depth, as the
/// interpreter leaves out a value that holds an instance or a reference (specification 11 §6.7).
fn retainable(types: &Types<'_>, index: u32, depth: usize) -> Result<bool, RuntimeError> {
    if depth > MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    let ty = types.get(index)?;
    Ok(match ty.kind {
        kind::FUNCTION_BLOCK
        | kind::CLASS
        | kind::INTERFACE
        | kind::REFERENCE
        | kind::POINTER
        | kind::PROGRAM => false,
        kind::ARRAY => retainable(types, types.array(&ty)?.0, depth + 1)?,
        kind::STRUCT | kind::UNION => {
            for member in types.members(&ty)? {
                if !retainable(types, member.type_index, depth + 1)? {
                    return Ok(false);
                }
            }
            true
        }
        _ => true,
    })
}

/// The codec's tag of an elementary kind.
fn tag(kind: u32) -> Option<ValueTag> {
    Some(match kind {
        kind::BOOL => ValueTag::Bool,
        kind::SINT => ValueTag::SInt,
        kind::INT => ValueTag::Int,
        kind::DINT => ValueTag::DInt,
        kind::LINT => ValueTag::LInt,
        kind::USINT => ValueTag::USInt,
        kind::UINT => ValueTag::UInt,
        kind::UDINT => ValueTag::UDInt,
        kind::ULINT => ValueTag::ULInt,
        kind::REAL => ValueTag::Real,
        kind::LREAL => ValueTag::LReal,
        kind::BYTE => ValueTag::Byte,
        kind::WORD => ValueTag::Word,
        kind::DWORD => ValueTag::DWord,
        kind::LWORD => ValueTag::LWord,
        kind::TIME => ValueTag::Time,
        kind::LTIME => ValueTag::LTime,
        kind::DATE => ValueTag::Date,
        kind::LDATE => ValueTag::LDate,
        kind::TOD => ValueTag::Tod,
        kind::LTOD => ValueTag::LTod,
        kind::DT => ValueTag::Dt,
        kind::LDT => ValueTag::Ldt,
        kind::CHAR => ValueTag::Char,
        kind::WCHAR => ValueTag::WChar,
        _ => return None,
    })
}

/// The largest encoding of a value of type `index`.
fn bound(types: &Types<'_>, index: u32, depth: usize) -> Result<usize, RuntimeError> {
    if depth > MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    let ty = types.get(index)?;
    let capacity = usize::try_from(ty.count).map_err(|_| RuntimeError::Overflow)?;
    match ty.kind {
        kind::ENUM => {
            let mut longest = 0;
            for (name, _) in types.variants(&ty)? {
                longest = longest.max(types.string(name)?.len());
            }
            sum(&[1, 4, types.string(ty.name)?.len(), 4, longest, 8])
        }
        kind::SUBRANGE => {
            let (base, _, _) = types.subrange(&ty)?;
            let width = width(base).ok_or(RuntimeError::TypeMismatch)?;
            Ok(1 + width)
        }
        kind::STRING => sum(&[1, 4, capacity]),
        // A UTF-16 code unit is at most three UTF-8 bytes; a pair of them is four.
        kind::WSTRING => sum(&[1, 4, capacity.checked_mul(3).ok_or(RuntimeError::Overflow)?]),
        kind::ARRAY => {
            let (element, dimensions) = types.array(&ty)?;
            let header = sum(&[
                1,
                4,
                4,
                dimensions
                    .len()
                    .checked_mul(16)
                    .ok_or(RuntimeError::Overflow)?,
            ])?;
            let (count, _) = types.layout(element, dimensions)?;
            let elements = count
                .checked_mul(bound(types, element, depth + 1)?)
                .ok_or(RuntimeError::Overflow)?;
            sum(&[header, elements])
        }
        kind::STRUCT => {
            let mut total = sum(&[1, 4, types.string(ty.name)?.len(), 4, 1])?;
            for member in types.members(&ty)? {
                total = sum(&[
                    total,
                    4,
                    types.string(member.name)?.len(),
                    bound(types, member.type_index, depth + 1)?,
                ])?;
            }
            Ok(total)
        }
        other => width(other)
            .map(|width| 1 + width)
            .ok_or(RuntimeError::TypeMismatch),
    }
}

/// Appends the STRN v3 encoding of the value of type `index` at `offset` in `area`: the bytes the
/// codec writes for the same value held as a `Value`.
fn encode<A: AreaBytes + ?Sized>(
    types: &Types<'_>,
    area: &A,
    offset: usize,
    index: u32,
    out: &mut Vec<u8>,
    depth: usize,
) -> Result<(), RuntimeError> {
    if depth > MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    let ty = types.get(index)?;
    match ty.kind {
        kind::ENUM => {
            let width = usize::try_from(ty.size).map_err(|_| RuntimeError::Overflow)?;
            let unsigned = read_unsigned(area, offset, width)?;
            let (name, number) = variant_of(types, &ty, unsigned, width)?.ok_or_else(|| {
                RuntimeError::RetainStore(SmolStr::new(format!(
                    "cannot persist a value enumeration {} does not declare",
                    types.string(ty.name).unwrap_or("")
                )))
            })?;
            encode_enumeration(out, types.string(ty.name)?, types.string(name)?, number);
            Ok(())
        }
        kind::SUBRANGE => encode_scalar(area, offset, types.subrange(&ty)?.0, out),
        kind::STRING => encode_text(area, offset, ty.count, out),
        kind::WSTRING => encode_wide_text(area, offset, ty.count, out),
        kind::ARRAY => {
            let (element, dimensions) = types.array(&ty)?;
            let (count, stride) = types.layout(element, dimensions.clone())?;
            encode_array_header(out, count, dimensions);
            for position in 0..count {
                let at = position
                    .checked_mul(stride)
                    .and_then(|from| from.checked_add(offset))
                    .ok_or(RuntimeError::Overflow)?;
                encode(types, area, at, element, out, depth + 1)?;
            }
            Ok(())
        }
        kind::STRUCT => {
            let count = usize::try_from(ty.count).map_err(|_| RuntimeError::Overflow)?;
            encode_struct_header(out, types.string(ty.name)?, count);
            for member in types.members(&ty)? {
                encode_string(out, types.string(member.name)?);
                let at = offset
                    .checked_add(
                        usize::try_from(member.offset).map_err(|_| RuntimeError::Overflow)?,
                    )
                    .ok_or(RuntimeError::Overflow)?;
                encode(types, area, at, member.type_index, out, depth + 1)?;
            }
            // A compiled structure has no physical backing, so no checkpoint follows its members.
            out.push(NO_PHYSICAL_CHECKPOINT);
            Ok(())
        }
        // A `BOOL` member of a `STRUCT OVERLAP` that takes one bit of its byte.
        kind::BOOL => match types.bit(&ty)? {
            Some(bit) => {
                out.push(ValueTag::Bool as u8);
                out.push(u8::from((read_unsigned(area, offset, 1)? >> bit) & 1 != 0));
                Ok(())
            }
            None => encode_scalar(area, offset, kind::BOOL, out),
        },
        other => encode_scalar(area, offset, other, out),
    }
}

/// Appends an elementary value of kind `kind`: its tag and its little-endian bytes, a `BOOL` as 0 or
/// 1; a non-finite `REAL` or `LREAL` is refused as the codec refuses it.
fn encode_scalar<A: AreaBytes + ?Sized>(
    area: &A,
    offset: usize,
    kind: u32,
    out: &mut Vec<u8>,
) -> Result<(), RuntimeError> {
    let (tag, width) = tag(kind)
        .zip(width(kind))
        .ok_or(RuntimeError::TypeMismatch)?;
    let mut bytes = [0u8; 8];
    read(area, offset, &mut bytes[..width])?;
    match kind {
        kind::BOOL => bytes[0] = u8::from(bytes[0] != 0),
        kind::REAL if !f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).is_finite() => {
            return Err(RuntimeError::RetainStore(
                "cannot persist non-finite REAL retain value".into(),
            ));
        }
        kind::LREAL if !f64::from_le_bytes(bytes).is_finite() => {
            return Err(RuntimeError::RetainStore(
                "cannot persist non-finite LREAL retain value".into(),
            ));
        }
        _ => {}
    }
    out.push(tag as u8);
    out.extend_from_slice(&bytes[..width]);
    Ok(())
}

/// Appends a `STRING`: its tag, its length and its UTF-8 bytes, read where they lie.
fn encode_text<A: AreaBytes + ?Sized>(
    area: &A,
    offset: usize,
    capacity: u32,
    out: &mut Vec<u8>,
) -> Result<(), RuntimeError> {
    let length = text_length(area, offset, capacity)?;
    out.push(ValueTag::String as u8);
    out.extend_from_slice(
        &u32::try_from(length)
            .map_err(|_| RuntimeError::Overflow)?
            .to_le_bytes(),
    );
    let start = out.len();
    out.resize(start + length, 0);
    read(area, offset + 2, &mut out[start..])?;
    if core::str::from_utf8(&out[start..]).is_err() {
        return Err(RuntimeError::RetainStore(
            "cannot persist a STRING that is not UTF-8".into(),
        ));
    }
    Ok(())
}

/// Appends a `WSTRING`: its tag, its length in UTF-8 bytes and its text as UTF-8, as the codec
/// writes a `WSTRING`.
fn encode_wide_text<A: AreaBytes + ?Sized>(
    area: &A,
    offset: usize,
    capacity: u32,
    out: &mut Vec<u8>,
) -> Result<(), RuntimeError> {
    let length = wide_length(area, offset, capacity)?;
    let mut bytes = 0usize;
    for character in char::decode_utf16(wide_units(area, offset, length)) {
        let character = character.map_err(|_| {
            RuntimeError::RetainStore("cannot persist a WSTRING that is not UTF-16".into())
        })?;
        bytes += character.len_utf8();
    }
    out.push(ValueTag::WString as u8);
    out.extend_from_slice(
        &u32::try_from(bytes)
            .map_err(|_| RuntimeError::Overflow)?
            .to_le_bytes(),
    );
    let mut buffer = [0u8; 4];
    for character in char::decode_utf16(wide_units(area, offset, length)).flatten() {
        out.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
