//! The retained variables of a compiled program (specification 44 §5.10, §11; specification 11
//! §6.7): their checkpoint, written from the retain area through the retain map and the type table,
//! and a checkpoint restored by name into the retain area.
//!
//! A capture visits the retain map in its order and writes each variable under the key of
//! specification 11 §6.7, a global by its declared name and a program variable as
//! `@program/<program>/<variable>`, with the bytes the STRN v3 payload codec writes for the same
//! value: a scalar's tag and its bytes, a text as UTF-8, an enumeration by its type and value names,
//! an array with its bounds, a structure with its type and member names. No `Value` is built, so a
//! capture into a reserved payload allocates nothing (specification 34 MEM-04). A variable whose
//! type holds a function block instance is left out of the checkpoint, as the interpreter leaves an
//! instance out. A checkpoint written by the interpreter therefore restores here, and one written
//! here restores into the interpreter.
//!
//! A restore applies a checkpoint by name to a copy of the retain area, with the interpreter's
//! migration rules: a safe widening of a number, a text cut to its capacity, an enumeration
//! rebuilt with its declared spelling, a structure rebuilt member by member, a member the
//! checkpoint lacks at its starting value, a key nothing declares reported as an orphan. Every
//! entry is checked before the area changes, so one value the program refuses leaves every
//! retained variable as it was.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_plc_abi::{AREA_GLOBALS, AREA_INSTANCES, AREA_RETAIN};

use super::image::{
    type_kind as kind, AreaRecord, Image, MemberDetail, RetainRecord, SectionId, SymbolRecord,
    Table, TypeRecord, AREA_HAS_INIT, MEMBER_BYTES, NONE, SYMBOL_PROGRAM,
};
use crate::bytecode::BYTECODE_MAX_TYPE_NESTING;
use crate::collections::OrderedMap;
use crate::error::RuntimeError;
use crate::retain::codec::{
    encode_array_header, encode_enumeration, encode_string, encode_struct_header, PayloadWriter,
    ValueTag, NO_PHYSICAL_CHECKPOINT,
};
use crate::retain::RetainSnapshot;
use crate::value::{
    string_capacity_cut, ArrayValue, DateTimeValue, DateValue, Duration, EnumValue, LDateTimeValue,
    LDateValue, LTimeOfDayValue, StructValue, TimeOfDayValue, Value,
};
use crate::vm::RetainApplyEvent;

/// The bytes of a compiled program's retain area, where the program holds them.
pub(crate) trait AreaBytes {
    /// The area's length in bytes.
    fn len(&self) -> usize;

    /// Copies the bytes from `offset` into `out`; `false` when they are not all in the area.
    fn read(&self, offset: usize, out: &mut [u8]) -> bool;
}

impl AreaBytes for [u8] {
    fn len(&self) -> usize {
        <[u8]>::len(self)
    }

    fn read(&self, offset: usize, out: &mut [u8]) -> bool {
        match offset
            .checked_add(out.len())
            .and_then(|end| self.get(offset..end))
        {
            Some(bytes) => {
                out.copy_from_slice(bytes);
                true
            }
            None => false,
        }
    }
}

/// Appends the checkpoint entries of the retained variables in `area`, the program's retain area,
/// to `payload`, in the retain map's order (specification 44 §5.10, §11).
///
/// # Errors
///
/// [`RuntimeError::RetainStore`] for a value no checkpoint holds: a non-finite `REAL` or `LREAL`, a
/// text that is not UTF-8 or UTF-16, an enumeration number its type does not declare.
/// [`RuntimeError::InvalidBundle`] for a retain map or a type table that does not read.
pub(crate) fn fill<A: AreaBytes + ?Sized>(
    image: &Image<'_>,
    area: &A,
    payload: &mut PayloadWriter<'_>,
) -> Result<(), RuntimeError> {
    let types = Types::of(image)?;
    each_retained(image, |retained| {
        if !retainable(&types, retained.type_index, 0)? {
            return Ok(());
        }
        retained.key.with_pieces(|pieces| {
            payload.entry_with(pieces, |out| {
                encode(&types, area, retained.offset, retained.type_index, out, 0)
            })
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

/// The retain area of `image` with the checkpoint `snapshot` restored into `current`, the bytes the
/// program's retain area holds now, by name, and the events of the restore in the checkpoint's order
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
/// read. Nothing is returned then, so the caller's retain area keeps its values.
pub fn stage(
    image: &Image<'_>,
    snapshot: &RetainSnapshot,
    current: &[u8],
) -> Result<(Vec<u8>, Vec<RetainApplyEvent>), RuntimeError> {
    let types = Types::of(image)?;
    let starting = starting_bytes(image)?;
    if current.len() != starting.len() {
        return Err(damaged("retain area"));
    }
    let mut destinations = BTreeMap::new();
    each_retained(image, |retained| {
        if destinations
            .insert(
                retained.key.folded(),
                (retained.type_index, retained.offset),
            )
            .is_some()
        {
            return Err(RuntimeError::RetainMigration(
                "ambiguous retained destination label".into(),
            ));
        }
        Ok(())
    })?;
    let mut staged = current.to_vec();
    let mut events = Vec::new();
    for (key, value) in snapshot.values() {
        let Some(&(type_index, offset)) = destinations.get(&key.to_ascii_uppercase()) else {
            if !declares_unretained(image, key) {
                events.push(RetainApplyEvent::Orphan { name: key.clone() });
            }
            continue;
        };
        let mut restore = Restore {
            types: &types,
            staged: &mut staged,
            starting: &starting,
        };
        restore
            .write(type_index, offset, value, 0)
            .map_err(|error| migration_error(key, error))?;
        let after = decode(&types, staged.as_slice(), offset, type_index, 0)
            .map_err(|error| migration_error(key, error))?;
        if &after != value {
            events.push(RetainApplyEvent::Migrated {
                name: key.clone(),
                before: value.clone(),
                after,
            });
        }
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
    let starting = starting_bytes(image)?;
    stage(image, snapshot, &starting).map(|(_, events)| events)
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

/// The starting bytes of the retain area of `image`, zero where `INIT` gives none.
fn starting_bytes(image: &Image<'_>) -> Result<Vec<u8>, RuntimeError> {
    let areas = image
        .table::<AreaRecord>()
        .map_err(|_| damaged("area table"))?;
    let Some(area) = areas.iter().find(|area| area.id == u32::from(AREA_RETAIN)) else {
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

/// Whether `key` names a variable that `image` declares without `RETAIN`: a non-retained global by
/// its name, or a non-retained variable of a program instance as `@program/<program>/<variable>`.
/// The interpreter leaves such a variable alone and reports no orphan (specification 11 §6.7).
fn declares_unretained(image: &Image<'_>, key: &str) -> bool {
    let folded = key.to_ascii_uppercase();
    let (canonical, program) = match folded.strip_prefix("@PROGRAM/") {
        Some(rest) => match rest.split_once('/') {
            Some((program, variable)) if !variable.contains(['/', '.']) => {
                (format!("{program}.{variable}"), true)
            }
            _ => return false,
        },
        None => (folded, false),
    };
    let Some((_, symbol)) = image.find_symbol(&canonical) else {
        return false;
    };
    if program {
        let parent_is_program = image
            .table::<SymbolRecord>()
            .ok()
            .and_then(|symbols| symbols.get(symbol.parent))
            .is_some_and(|parent| parent.flags & SYMBOL_PROGRAM != 0);
        parent_is_program && symbol.area == AREA_INSTANCES
    } else {
        symbol.parent == NONE && symbol.area == AREA_GLOBALS
    }
}

/// The failure of a table of the image that does not read as the build wrote it.
fn damaged(what: &str) -> RuntimeError {
    RuntimeError::InvalidBundle(SmolStr::new(format!(
        "the program image's {what} does not read"
    )))
}

/// The sum of `values`, or [`RuntimeError::Overflow`].
fn sum(values: &[usize]) -> Result<usize, RuntimeError> {
    values.iter().try_fold(0usize, |total, value| {
        total.checked_add(*value).ok_or(RuntimeError::Overflow)
    })
}

/// The checkpoint key of a retained variable (specification 11 §6.7).
#[derive(Clone, Copy)]
enum Key<'i> {
    /// A global, by its declared name.
    Global(&'i str),
    /// A variable of a program instance, `@program/<program>/<variable>`.
    Program { program: &'i str, variable: &'i str },
}

impl Key<'_> {
    /// Calls `write` with the key as the pieces it is written in.
    fn with_pieces<R>(&self, write: impl FnOnce(&[&str]) -> R) -> R {
        match *self {
            Self::Global(name) => write(&[name]),
            Self::Program { program, variable } => write(&["@program/", program, "/", variable]),
        }
    }

    /// The key's length in bytes.
    fn len(&self) -> usize {
        self.with_pieces(|pieces| pieces.iter().map(|piece| piece.len()).sum())
    }

    /// The key in capitals, as a key is compared on load.
    fn folded(&self) -> String {
        let mut key = String::with_capacity(self.len());
        self.with_pieces(|pieces| {
            for piece in pieces {
                key.push_str(piece);
            }
        });
        key.make_ascii_uppercase();
        key
    }
}

/// A variable of the retain map: its key, its type and its offset in the retain area.
struct Retained<'i> {
    key: Key<'i>,
    type_index: u32,
    offset: usize,
}

/// Visits the variables of the retain map in its order. A global's key is its display name, a
/// program variable's the display names of its program instance and of itself (specification 44
/// §5.10).
fn each_retained<'i>(
    image: &Image<'i>,
    mut visit: impl FnMut(Retained<'i>) -> Result<(), RuntimeError>,
) -> Result<(), RuntimeError> {
    let map = image
        .table::<RetainRecord>()
        .map_err(|_| damaged("retain map"))?;
    let symbols = image
        .table::<SymbolRecord>()
        .map_err(|_| damaged("symbol table"))?;
    for record in map.iter() {
        let symbol = symbols
            .get(record.symbol)
            .filter(|symbol| symbol.area == AREA_RETAIN && symbol.offset == record.offset)
            .ok_or_else(|| damaged("retain map"))?;
        let display = image
            .string(symbol.display)
            .ok_or_else(|| damaged("symbol table"))?;
        let key = if symbol.parent == NONE {
            Key::Global(display)
        } else {
            let parent = symbols
                .get(symbol.parent)
                .filter(|parent| parent.flags & SYMBOL_PROGRAM != 0)
                .ok_or_else(|| damaged("retain map"))?;
            let program = image
                .string(parent.display)
                .ok_or_else(|| damaged("symbol table"))?;
            let variable = display
                .strip_prefix(program)
                .and_then(|rest| rest.strip_prefix('.'))
                .ok_or_else(|| damaged("symbol table"))?;
            Key::Program { program, variable }
        };
        visit(Retained {
            key,
            type_index: symbol.type_index,
            offset: usize::try_from(record.offset).map_err(|_| RuntimeError::Overflow)?,
        })?;
    }
    Ok(())
}

/// The type table with its detail list (specification 44 §5.8).
struct Types<'i> {
    image: Image<'i>,
    records: Table<'i, TypeRecord>,
    section: &'i [u8],
}

impl<'i> Types<'i> {
    fn of(image: &Image<'i>) -> Result<Self, RuntimeError> {
        Ok(Self {
            image: *image,
            records: image
                .table::<TypeRecord>()
                .map_err(|_| damaged("type table"))?,
            section: image.section_bytes(SectionId::Types),
        })
    }

    fn get(&self, index: u32) -> Result<TypeRecord, RuntimeError> {
        self.records.get(index).ok_or_else(|| damaged("type table"))
    }

    fn string(&self, offset: u32) -> Result<&'i str, RuntimeError> {
        self.image
            .string(offset)
            .ok_or_else(|| damaged("string pool"))
    }

    /// `bytes` bytes of the detail of `ty`.
    fn detail(&self, ty: &TypeRecord, bytes: usize) -> Result<&'i [u8], RuntimeError> {
        let start = usize::try_from(ty.detail).map_err(|_| RuntimeError::Overflow)?;
        start
            .checked_add(bytes)
            .and_then(|end| self.section.get(start..end))
            .ok_or_else(|| damaged("type table"))
    }

    /// `count` records of `width` bytes in the detail of `ty`.
    fn records_of(
        &self,
        ty: &TypeRecord,
        prefix: usize,
        width: usize,
    ) -> Result<&'i [u8], RuntimeError> {
        let count = usize::try_from(ty.count).map_err(|_| RuntimeError::Overflow)?;
        let bytes = count
            .checked_mul(width)
            .and_then(|bytes| bytes.checked_add(prefix))
            .ok_or(RuntimeError::Overflow)?;
        self.detail(ty, bytes).map(|detail| &detail[prefix..])
    }

    /// The members of a structure, in their declared order.
    fn members(
        &self,
        ty: &TypeRecord,
    ) -> Result<impl Iterator<Item = MemberDetail> + 'i, RuntimeError> {
        Ok(self
            .records_of(ty, 0, MEMBER_BYTES)?
            .as_chunks::<MEMBER_BYTES>()
            .0
            .iter()
            .map(|member| MemberDetail::read(member)))
    }

    /// The values of an enumeration: each name's string offset and its number.
    fn variants(
        &self,
        ty: &TypeRecord,
    ) -> Result<impl Iterator<Item = (u32, i64)> + 'i, RuntimeError> {
        Ok(self
            .records_of(ty, 0, 16)?
            .as_chunks::<16>()
            .0
            .iter()
            .map(|value| (u32_at(value, 0), i64_at(value, 8))))
    }

    /// A subrange's elementary base kind and its bounds.
    fn subrange(&self, ty: &TypeRecord) -> Result<(u32, i64, i64), RuntimeError> {
        let detail = self.detail(ty, 24)?;
        Ok((u32_at(detail, 0), i64_at(detail, 8), i64_at(detail, 16)))
    }

    /// An array's element type and the bounds of its dimensions.
    fn array(
        &self,
        ty: &TypeRecord,
    ) -> Result<(u32, impl ExactSizeIterator<Item = (i64, i64)> + Clone + 'i), RuntimeError> {
        let element = u32_at(self.detail(ty, 8)?, 0);
        let dimensions = self
            .records_of(ty, 8, 16)?
            .as_chunks::<16>()
            .0
            .iter()
            .map(|bounds| (i64_at(bounds, 0), i64_at(bounds, 8)));
        Ok((element, dimensions))
    }

    /// The elements of an array of `dimensions` and the bytes between two of them.
    fn layout(
        &self,
        element: u32,
        dimensions: impl Iterator<Item = (i64, i64)>,
    ) -> Result<(usize, usize), RuntimeError> {
        let mut count = 1usize;
        for (lower, upper) in dimensions {
            let length = upper
                .checked_sub(lower)
                .and_then(|span| span.checked_add(1))
                .and_then(|length| usize::try_from(length).ok())
                .ok_or_else(|| damaged("type table"))?;
            count = count.checked_mul(length).ok_or(RuntimeError::Overflow)?;
        }
        let stride =
            usize::try_from(self.get(element)?.size).map_err(|_| RuntimeError::Overflow)?;
        Ok((count, stride))
    }
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    let mut word = [0u8; 4];
    word.copy_from_slice(&bytes[at..at + 4]);
    u32::from_le_bytes(word)
}

fn i64_at(bytes: &[u8], at: usize) -> i64 {
    let mut word = [0u8; 8];
    word.copy_from_slice(&bytes[at..at + 8]);
    i64::from_le_bytes(word)
}

/// Whether a value of type `index` may go into a checkpoint: not when it holds a function block,
/// class or program instance, an interface, a reference or a pointer, at any depth, as the
/// interpreter leaves out a value that holds an instance or a reference (specification 11 §6.7).
fn retainable(types: &Types<'_>, index: u32, depth: usize) -> Result<bool, RuntimeError> {
    if depth > BYTECODE_MAX_TYPE_NESTING {
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

/// The codec's tag of an elementary kind and the bytes of its value.
fn scalar(kind: u32) -> Option<(ValueTag, usize)> {
    Some(match kind {
        kind::BOOL => (ValueTag::Bool, 1),
        kind::SINT => (ValueTag::SInt, 1),
        kind::INT => (ValueTag::Int, 2),
        kind::DINT => (ValueTag::DInt, 4),
        kind::LINT => (ValueTag::LInt, 8),
        kind::USINT => (ValueTag::USInt, 1),
        kind::UINT => (ValueTag::UInt, 2),
        kind::UDINT => (ValueTag::UDInt, 4),
        kind::ULINT => (ValueTag::ULInt, 8),
        kind::REAL => (ValueTag::Real, 4),
        kind::LREAL => (ValueTag::LReal, 8),
        kind::BYTE => (ValueTag::Byte, 1),
        kind::WORD => (ValueTag::Word, 2),
        kind::DWORD => (ValueTag::DWord, 4),
        kind::LWORD => (ValueTag::LWord, 8),
        kind::TIME => (ValueTag::Time, 8),
        kind::LTIME => (ValueTag::LTime, 8),
        kind::DATE => (ValueTag::Date, 8),
        kind::LDATE => (ValueTag::LDate, 8),
        kind::TOD => (ValueTag::Tod, 8),
        kind::LTOD => (ValueTag::LTod, 8),
        kind::DT => (ValueTag::Dt, 8),
        kind::LDT => (ValueTag::Ldt, 8),
        kind::CHAR => (ValueTag::Char, 1),
        kind::WCHAR => (ValueTag::WChar, 2),
        _ => return None,
    })
}

/// The largest encoding of a value of type `index`.
fn bound(types: &Types<'_>, index: u32, depth: usize) -> Result<usize, RuntimeError> {
    if depth > BYTECODE_MAX_TYPE_NESTING {
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
            let (_, width) = scalar(base).ok_or(RuntimeError::TypeMismatch)?;
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
        other => scalar(other)
            .map(|(_, width)| 1 + width)
            .ok_or(RuntimeError::TypeMismatch),
    }
}

/// Reads the bytes at `offset` of `area` into `out`.
fn read<A: AreaBytes + ?Sized>(
    area: &A,
    offset: usize,
    out: &mut [u8],
) -> Result<(), RuntimeError> {
    if area.read(offset, out) {
        Ok(())
    } else {
        Err(damaged("retain map"))
    }
}

/// The number of `width` bytes at `offset`, zero-extended.
fn read_unsigned<A: AreaBytes + ?Sized>(
    area: &A,
    offset: usize,
    width: usize,
) -> Result<u64, RuntimeError> {
    let mut bytes = [0u8; 8];
    read(area, offset, &mut bytes[..width.min(8)])?;
    Ok(u64::from_le_bytes(bytes))
}

/// `unsigned`, a number of `width` bytes, sign-extended.
fn sign_extend(unsigned: u64, width: usize) -> i64 {
    let shift = 64 - 8 * u32::try_from(width.clamp(1, 8)).unwrap_or(8);
    (i64::from_le_bytes(unsigned.to_le_bytes()) << shift) >> shift
}

/// The declared value of the enumeration `ty` that the `width` bytes `unsigned` hold: its name's
/// string offset and its number. The base is signed or unsigned; one reading is a declared value.
fn variant_of(
    types: &Types<'_>,
    ty: &TypeRecord,
    unsigned: u64,
    width: usize,
) -> Result<(u32, i64), RuntimeError> {
    let signed = sign_extend(unsigned, width);
    let zero_extended = i64::try_from(unsigned).unwrap_or(signed);
    types
        .variants(ty)?
        .find(|(_, number)| *number == signed || *number == zero_extended)
        .ok_or_else(|| {
            RuntimeError::RetainStore(SmolStr::new(format!(
                "cannot persist {signed}, which is no value of enumeration {}",
                types.string(ty.name).unwrap_or("")
            )))
        })
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
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    let ty = types.get(index)?;
    match ty.kind {
        kind::ENUM => {
            let width = usize::try_from(ty.size).map_err(|_| RuntimeError::Overflow)?;
            let unsigned = read_unsigned(area, offset, width)?;
            let (name, number) = variant_of(types, &ty, unsigned, width)?;
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
    let (tag, width) = scalar(kind).ok_or(RuntimeError::TypeMismatch)?;
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

/// The length of the text at `offset`, at most `capacity`.
fn text_length<A: AreaBytes + ?Sized>(
    area: &A,
    offset: usize,
    capacity: u32,
) -> Result<usize, RuntimeError> {
    let length = read_unsigned(area, offset, 2)?;
    if length > u64::from(capacity) {
        return Err(RuntimeError::RetainStore(
            "cannot persist a text longer than its capacity".into(),
        ));
    }
    usize::try_from(length).map_err(|_| RuntimeError::Overflow)
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

/// The length in code units of the `WSTRING` at `offset`, at most `capacity`, whose units all lie
/// in `area`.
fn wide_length<A: AreaBytes + ?Sized>(
    area: &A,
    offset: usize,
    capacity: u32,
) -> Result<usize, RuntimeError> {
    let length = text_length(area, offset, capacity)?;
    let end = length
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(offset))
        .and_then(|end| end.checked_add(2))
        .ok_or(RuntimeError::Overflow)?;
    if end > area.len() {
        return Err(damaged("retain map"));
    }
    Ok(length)
}

/// The UTF-16 code units of the `WSTRING` of `length` units at `offset`, which [`wide_length`]
/// found in the area.
fn wide_units<A: AreaBytes + ?Sized>(
    area: &A,
    offset: usize,
    length: usize,
) -> impl Iterator<Item = u16> + '_ {
    (0..length).map(move |unit| {
        let mut bytes = [0u8; 2];
        // `wide_length` found every unit in the area.
        let _ = area.read(offset + 2 + 2 * unit, &mut bytes);
        u16::from_le_bytes(bytes)
    })
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

/// The family of an integer or bit-string kind, for the widening a restore allows.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    Signed,
    Unsigned,
    Bits,
}

/// The family and the width of an integer or bit-string kind.
fn integer_kind(kind: u32) -> Option<(Family, usize)> {
    Some(match kind {
        kind::SINT => (Family::Signed, 1),
        kind::INT => (Family::Signed, 2),
        kind::DINT => (Family::Signed, 4),
        kind::LINT => (Family::Signed, 8),
        kind::USINT => (Family::Unsigned, 1),
        kind::UINT => (Family::Unsigned, 2),
        kind::UDINT => (Family::Unsigned, 4),
        kind::ULINT => (Family::Unsigned, 8),
        kind::BYTE => (Family::Bits, 1),
        kind::WORD => (Family::Bits, 2),
        kind::DWORD => (Family::Bits, 4),
        kind::LWORD => (Family::Bits, 8),
        _ => return None,
    })
}

/// The family, the width and the number of an integer or bit-string value.
fn integer_value(value: &Value) -> Option<(Family, usize, i128)> {
    Some(match *value {
        Value::SInt(v) => (Family::Signed, 1, i128::from(v)),
        Value::Int(v) => (Family::Signed, 2, i128::from(v)),
        Value::DInt(v) => (Family::Signed, 4, i128::from(v)),
        Value::LInt(v) => (Family::Signed, 8, i128::from(v)),
        Value::USInt(v) => (Family::Unsigned, 1, i128::from(v)),
        Value::UInt(v) => (Family::Unsigned, 2, i128::from(v)),
        Value::UDInt(v) => (Family::Unsigned, 4, i128::from(v)),
        Value::ULInt(v) => (Family::Unsigned, 8, i128::from(v)),
        Value::Byte(v) => (Family::Bits, 1, i128::from(v)),
        Value::Word(v) => (Family::Bits, 2, i128::from(v)),
        Value::DWord(v) => (Family::Bits, 4, i128::from(v)),
        Value::LWord(v) => (Family::Bits, 8, i128::from(v)),
        _ => return None,
    })
}

/// A checkpoint applied to a copy of the retain area.
struct Restore<'a, 'i> {
    types: &'a Types<'i>,
    staged: &'a mut [u8],
    starting: &'a [u8],
}

impl Restore<'_, '_> {
    /// The `length` staged bytes at `offset`.
    fn slot(&mut self, offset: usize, length: usize) -> Result<&mut [u8], RuntimeError> {
        offset
            .checked_add(length)
            .and_then(|end| self.staged.get_mut(offset..end))
            .ok_or_else(|| damaged("retain map"))
    }

    /// Writes `bytes` at `offset`.
    fn put(&mut self, offset: usize, bytes: &[u8]) -> Result<(), RuntimeError> {
        self.slot(offset, bytes.len())?.copy_from_slice(bytes);
        Ok(())
    }

    /// Writes `value` as a value of type `index` at `offset`, converted by the interpreter's
    /// migration rules (specification 11 §6.7).
    fn write(
        &mut self,
        index: u32,
        offset: usize,
        value: &Value,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        if depth > BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        let ty = self.types.get(index)?;
        match ty.kind {
            kind::ENUM => {
                let Value::Enum(value) = value else {
                    return Err(RuntimeError::TypeMismatch);
                };
                if !value
                    .type_name()
                    .eq_ignore_ascii_case(self.types.string(ty.name)?)
                {
                    return Err(RuntimeError::TypeMismatch);
                }
                let mut declared = None;
                for (name, number) in self.types.variants(&ty)? {
                    if number == value.numeric_value()
                        && self
                            .types
                            .string(name)?
                            .eq_ignore_ascii_case(value.variant_name())
                    {
                        declared = Some(number);
                        break;
                    }
                }
                let number = declared.ok_or(RuntimeError::TypeMismatch)?;
                let width = usize::try_from(ty.size).map_err(|_| RuntimeError::Overflow)?;
                self.put(offset, &number.to_le_bytes()[..width.min(8)])
            }
            kind::SUBRANGE => {
                let (base, lower, upper) = self.types.subrange(&ty)?;
                self.write_scalar(base, offset, value)?;
                let (_, _, number) = integer_value(value)
                    .filter(|(family, _, _)| *family != Family::Bits)
                    .ok_or(RuntimeError::TypeMismatch)?;
                if number < i128::from(lower) || number > i128::from(upper) {
                    return Err(RuntimeError::SubrangeViolation {
                        value: number,
                        lower: i128::from(lower),
                        upper: i128::from(upper),
                    });
                }
                Ok(())
            }
            kind::STRING => {
                let Value::String(text) = value else {
                    return Err(RuntimeError::TypeMismatch);
                };
                let capacity = usize::try_from(ty.count).map_err(|_| RuntimeError::Overflow)?;
                let end = string_capacity_cut(text, false, capacity).unwrap_or(text.len());
                let kept = &text.as_bytes()[..end];
                self.write_text(offset, capacity, kept.len(), kept)
            }
            kind::WSTRING => {
                let Value::WString(text) = value else {
                    return Err(RuntimeError::TypeMismatch);
                };
                let capacity = usize::try_from(ty.count).map_err(|_| RuntimeError::Overflow)?;
                let end = string_capacity_cut(text, true, capacity).unwrap_or(text.len());
                let mut units = Vec::with_capacity(capacity.saturating_mul(2));
                for unit in text[..end].encode_utf16() {
                    units.extend_from_slice(&unit.to_le_bytes());
                }
                self.write_text(offset, capacity * 2, units.len() / 2, &units)
            }
            kind::ARRAY => {
                let Value::Array(array) = value else {
                    return Err(RuntimeError::TypeMismatch);
                };
                let (element, dimensions) = self.types.array(&ty)?;
                if !array.dimensions().iter().copied().eq(dimensions.clone()) {
                    return Err(RuntimeError::TypeMismatch);
                }
                let (count, stride) = self.types.layout(element, dimensions)?;
                if array.elements().len() != count {
                    return Err(RuntimeError::TypeMismatch);
                }
                for (position, element_value) in array.elements().iter().enumerate() {
                    let at = position
                        .checked_mul(stride)
                        .and_then(|from| from.checked_add(offset))
                        .ok_or(RuntimeError::Overflow)?;
                    self.write(element, at, element_value, depth + 1)?;
                }
                Ok(())
            }
            kind::STRUCT => {
                let Value::Struct(structure) = value else {
                    return Err(RuntimeError::TypeMismatch);
                };
                if structure.physical_layout().is_some() {
                    return Err(RuntimeError::RetainMigration(
                        "retained physical backing requires a compatible physical destination"
                            .into(),
                    ));
                }
                for member in self.types.members(&ty)? {
                    let name = self.types.string(member.name)?;
                    let at = offset
                        .checked_add(
                            usize::try_from(member.offset).map_err(|_| RuntimeError::Overflow)?,
                        )
                        .ok_or(RuntimeError::Overflow)?;
                    let field = structure
                        .fields()
                        .iter()
                        .find(|(field, _)| field.eq_ignore_ascii_case(name));
                    match field {
                        Some((_, field)) => self.write(member.type_index, at, field, depth + 1)?,
                        // A member the saved value lacks takes its starting value: its declared
                        // initial value, or its type's.
                        None => {
                            let size = usize::try_from(self.types.get(member.type_index)?.size)
                                .map_err(|_| RuntimeError::Overflow)?;
                            let source = self.starting;
                            let starting = at
                                .checked_add(size)
                                .and_then(|end| source.get(at..end))
                                .ok_or_else(|| damaged("retain map"))?;
                            self.put(at, starting)?;
                        }
                    }
                }
                Ok(())
            }
            other => self.write_scalar(other, offset, value),
        }
    }

    /// Writes a text of `length` units, `bytes` long, after its length, and clears the rest of
    /// its `room` bytes.
    fn write_text(
        &mut self,
        offset: usize,
        room: usize,
        length: usize,
        bytes: &[u8],
    ) -> Result<(), RuntimeError> {
        let length = u16::try_from(length).map_err(|_| RuntimeError::Overflow)?;
        self.put(offset, &length.to_le_bytes())?;
        let text = self.slot(offset + 2, room)?;
        let (written, rest) = text.split_at_mut(bytes.len().min(room));
        written.copy_from_slice(&bytes[..written.len()]);
        rest.fill(0);
        Ok(())
    }

    /// Writes the elementary `value` as kind `kind` at `offset`: a number widened within its family,
    /// a `REAL` as an `LREAL`, every other value only as its own kind (specification 11 §6.7).
    fn write_scalar(
        &mut self,
        kind: u32,
        offset: usize,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        if let Some((family, width)) = integer_kind(kind) {
            let (source, source_width, number) =
                integer_value(value).ok_or(RuntimeError::TypeMismatch)?;
            if source != family || source_width > width {
                return Err(RuntimeError::TypeMismatch);
            }
            // The number fits the wider kind, so its low bytes are its value.
            let bytes = match family {
                Family::Signed => i64::try_from(number)
                    .map_err(|_| RuntimeError::TypeMismatch)?
                    .to_le_bytes(),
                Family::Unsigned | Family::Bits => u64::try_from(number)
                    .map_err(|_| RuntimeError::TypeMismatch)?
                    .to_le_bytes(),
            };
            return self.put(offset, &bytes[..width]);
        }
        let bytes: ([u8; 8], usize) = match (kind, value) {
            (kind::BOOL, Value::Bool(flag)) => ([u8::from(*flag), 0, 0, 0, 0, 0, 0, 0], 1),
            (kind::REAL, Value::Real(number)) => (widen(&number.to_le_bytes()), 4),
            (kind::LREAL, Value::Real(number)) => (f64::from(*number).to_le_bytes(), 8),
            (kind::LREAL, Value::LReal(number)) => (number.to_le_bytes(), 8),
            (kind::TIME, Value::Time(span)) | (kind::LTIME, Value::LTime(span)) => {
                (span.as_nanos().to_le_bytes(), 8)
            }
            (kind::DATE, Value::Date(date)) => (date.ticks().to_le_bytes(), 8),
            (kind::LDATE, Value::LDate(date)) => (date.nanos().to_le_bytes(), 8),
            (kind::TOD, Value::Tod(time)) => (time.ticks().to_le_bytes(), 8),
            (kind::LTOD, Value::LTod(time)) => (time.nanos().to_le_bytes(), 8),
            (kind::DT, Value::Dt(stamp)) => (stamp.ticks().to_le_bytes(), 8),
            (kind::LDT, Value::Ldt(stamp)) => (stamp.nanos().to_le_bytes(), 8),
            (kind::CHAR, Value::Char(character)) => ([*character, 0, 0, 0, 0, 0, 0, 0], 1),
            (kind::WCHAR, Value::WChar(character)) => (widen(&character.to_le_bytes()), 2),
            _ => return Err(RuntimeError::TypeMismatch),
        };
        self.put(offset, &bytes.0[..bytes.1])
    }
}

/// `bytes` at the start of eight.
fn widen(bytes: &[u8]) -> [u8; 8] {
    let mut wide = [0u8; 8];
    wide[..bytes.len()].copy_from_slice(bytes);
    wide
}

/// The value of type `index` at `offset` of `area`, as the interpreter holds it: an enumeration by
/// its declared names, a structure by its declared type and member names.
fn decode(
    types: &Types<'_>,
    area: &[u8],
    offset: usize,
    index: u32,
    depth: usize,
) -> Result<Value, RuntimeError> {
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    let ty = types.get(index)?;
    match ty.kind {
        kind::ENUM => {
            let width = usize::try_from(ty.size).map_err(|_| RuntimeError::Overflow)?;
            let unsigned = read_unsigned(area, offset, width)?;
            let (name, number) = variant_of(types, &ty, unsigned, width)?;
            Ok(Value::Enum(Box::new(EnumValue::from_canonical_parts(
                SmolStr::new(types.string(ty.name)?),
                SmolStr::new(types.string(name)?),
                number,
            ))))
        }
        kind::SUBRANGE => decode_scalar(area, offset, types.subrange(&ty)?.0),
        kind::STRING => {
            let length = text_length(area, offset, ty.count)?;
            let bytes = offset
                .checked_add(2)
                .and_then(|start| area.get(start..start.checked_add(length)?))
                .ok_or_else(|| damaged("retain map"))?;
            let text = core::str::from_utf8(bytes).map_err(|_| RuntimeError::TypeMismatch)?;
            Ok(Value::String(SmolStr::new(text)))
        }
        kind::WSTRING => {
            let length = wide_length(area, offset, ty.count)?;
            let units: Vec<u16> = wide_units(area, offset, length).collect();
            let text = String::from_utf16(&units).map_err(|_| RuntimeError::TypeMismatch)?;
            Ok(Value::WString(text))
        }
        kind::ARRAY => {
            let (element, dimensions) = types.array(&ty)?;
            let (count, stride) = types.layout(element, dimensions.clone())?;
            let mut elements = Vec::with_capacity(count);
            for position in 0..count {
                let at = position
                    .checked_mul(stride)
                    .and_then(|from| from.checked_add(offset))
                    .ok_or(RuntimeError::Overflow)?;
                elements.push(decode(types, area, at, element, depth + 1)?);
            }
            Ok(Value::Array(Box::new(ArrayValue::from_canonical_parts(
                elements,
                dimensions.collect(),
            ))))
        }
        kind::STRUCT => {
            let mut fields = OrderedMap::default();
            for member in types.members(&ty)? {
                let at = offset
                    .checked_add(
                        usize::try_from(member.offset).map_err(|_| RuntimeError::Overflow)?,
                    )
                    .ok_or(RuntimeError::Overflow)?;
                fields.insert(
                    SmolStr::new(types.string(member.name)?),
                    decode(types, area, at, member.type_index, depth + 1)?,
                );
            }
            Ok(Value::Struct(Arc::new(StructValue::from_canonical_parts(
                SmolStr::new(types.string(ty.name)?),
                fields,
            ))))
        }
        other => decode_scalar(area, offset, other),
    }
}

/// The elementary value of kind `kind` at `offset` of `area`.
fn decode_scalar(area: &[u8], offset: usize, kind: u32) -> Result<Value, RuntimeError> {
    let (_, width) = scalar(kind).ok_or(RuntimeError::TypeMismatch)?;
    let unsigned = read_unsigned(area, offset, width)?;
    let signed = sign_extend(unsigned, width);
    let narrow = |_| RuntimeError::TypeMismatch;
    Ok(match kind {
        kind::BOOL => Value::Bool(unsigned != 0),
        kind::SINT => Value::SInt(i8::try_from(signed).map_err(narrow)?),
        kind::INT => Value::Int(i16::try_from(signed).map_err(narrow)?),
        kind::DINT => Value::DInt(i32::try_from(signed).map_err(narrow)?),
        kind::LINT => Value::LInt(signed),
        kind::USINT => Value::USInt(u8::try_from(unsigned).map_err(narrow)?),
        kind::UINT => Value::UInt(u16::try_from(unsigned).map_err(narrow)?),
        kind::UDINT => Value::UDInt(u32::try_from(unsigned).map_err(narrow)?),
        kind::ULINT => Value::ULInt(unsigned),
        kind::BYTE => Value::Byte(u8::try_from(unsigned).map_err(narrow)?),
        kind::WORD => Value::Word(u16::try_from(unsigned).map_err(narrow)?),
        kind::DWORD => Value::DWord(u32::try_from(unsigned).map_err(narrow)?),
        kind::LWORD => Value::LWord(unsigned),
        kind::REAL => Value::Real(f32::from_bits(u32::try_from(unsigned).map_err(narrow)?)),
        kind::LREAL => Value::LReal(f64::from_bits(unsigned)),
        kind::TIME => Value::Time(Duration::from_nanos(signed)),
        kind::LTIME => Value::LTime(Duration::from_nanos(signed)),
        kind::DATE => Value::Date(DateValue::new(signed)),
        kind::LDATE => Value::LDate(LDateValue::new(signed)),
        kind::TOD => Value::Tod(TimeOfDayValue::new(signed)),
        kind::LTOD => Value::LTod(LTimeOfDayValue::new(signed)),
        kind::DT => Value::Dt(DateTimeValue::new(signed)),
        kind::LDT => Value::Ldt(LDateTimeValue::new(signed)),
        kind::CHAR => Value::Char(u8::try_from(unsigned).map_err(narrow)?),
        kind::WCHAR => Value::WChar(u16::try_from(unsigned).map_err(narrow)?),
        _ => return Err(RuntimeError::TypeMismatch),
    })
}

#[cfg(test)]
mod tests;
