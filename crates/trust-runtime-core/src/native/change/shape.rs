//! The types of the two programs of an online change, read from their type tables (specification
//! 44 §5.8): a type with its aliases resolved, whether a type has the same shape in both programs,
//! so that a value of it is copied as bytes, the members of a structure or an instance, and the
//! elements of an array.
//!
//! Two programs have their own type tables, so a type is compared by what it is, not by its index:
//! an elementary type by its kind, a text by its capacity, an enumeration by its values' names and
//! numbers, a subrange by its base and bounds, an array by its bounds and its element, a structure
//! by its members' names, offsets, flags and types, a function block or class by its name as well,
//! since a library block's own state means what it means only in that block, and a reference by
//! its target. The names of structure and enumeration types do not matter: the bytes and what they
//! mean do.
//!
//! A derived function block or class lies as its base first (specification 44 §7.2). The build
//! lists the base's members among the derived type's own; a table that holds the base as one
//! member, `__BASE`, is read the same way, its members counted as the instance's own where they
//! lie.

use alloc::vec::Vec;

use super::reserve;
use crate::error::RuntimeError;
use crate::native::image::{type_kind as kind, TypeRecord, SYMBOL_CONSTANT, SYMBOL_IN_OUT};
use crate::native::values::MAX_TYPE_NESTING;
use crate::native::values::{damaged, width, Types};

/// The member that holds a derived function block's or class's base (specification 44 §6.4).
const BASE: &str = "__BASE";
/// The member in which the root of a hierarchy holds its instance's class id.
pub(super) const CLASS: &str = "__CLASS";
/// The flags of a member a change does not carry: an in-out, which every call binds again, and a
/// constant, whose value is the new program's.
pub(super) const NOT_CARRIED: u32 = (SYMBOL_IN_OUT | SYMBOL_CONSTANT) as u32;

/// A type with its aliases resolved: its index in its table and its record.
#[derive(Clone, Copy)]
pub(super) struct Ty {
    /// The index of the record.
    pub(super) index: u32,
    /// The record.
    pub(super) record: TypeRecord,
}

impl Ty {
    /// The type's kind.
    pub(super) fn kind(&self) -> u32 {
        self.record.kind
    }

    /// The bytes of a value of the type.
    pub(super) fn size(&self) -> u32 {
        self.record.size
    }
}

/// Type `index` of `types` with its aliases resolved. A record whose size is not the one its kind
/// gives is damaged, so that a value's bytes are read only where they lie.
pub(super) fn resolve(types: &Types<'_>, index: u32) -> Result<Ty, RuntimeError> {
    let mut index = index;
    for _ in 0..=MAX_TYPE_NESTING {
        let record = types.get(index)?;
        if record.kind != kind::ALIAS {
            return if sound(&record) {
                Ok(Ty { index, record })
            } else {
                Err(damaged("type table"))
            };
        }
        index = word(types, &record)?;
    }
    Err(damaged("type table"))
}

/// The number in the first four bytes of a type's detail: an alias's base, a reference's target.
pub(super) fn word(types: &Types<'_>, record: &TypeRecord) -> Result<u32, RuntimeError> {
    types
        .detail(record, 4)?
        .first_chunk::<4>()
        .map(|word| u32::from_le_bytes(*word))
        .ok_or_else(|| damaged("type table"))
}

/// Whether a record's size is the one its kind gives.
fn sound(record: &TypeRecord) -> bool {
    let size = u64::from(record.size);
    let count = u64::from(record.count);
    match record.kind {
        kind::ENUM | kind::SUBRANGE => matches!(size, 1 | 2 | 4 | 8),
        kind::STRING => size >= 2 + count,
        kind::WSTRING => size >= 2 + 2 * count,
        kind::REFERENCE | kind::POINTER | kind::INTERFACE => size == 16,
        other => width(other).is_none_or(|width| width as u64 == size),
    }
}

/// Whether type `old` of the running program's table and type `new` of the new program's have the
/// same shape: the same bytes, each with the same meaning, so that a value of the one is a value
/// of the other.
pub(super) fn same(
    running: &Types<'_>,
    old: u32,
    changed: &Types<'_>,
    new: u32,
) -> Result<bool, RuntimeError> {
    same_at(running, old, changed, new, 0)
}

/// Whether types `a` and `b` of one table have the same shape.
pub(super) fn alike(types: &Types<'_>, a: u32, b: u32) -> Result<bool, RuntimeError> {
    Ok(a == b || same(types, a, types, b)?)
}

fn same_at(
    running: &Types<'_>,
    old: u32,
    changed: &Types<'_>,
    new: u32,
    depth: usize,
) -> Result<bool, RuntimeError> {
    // Only a type that names itself through a reference nests this deep; it is never the same.
    if depth > MAX_TYPE_NESTING {
        return Ok(false);
    }
    let old = resolve(running, old)?.record;
    let new = resolve(changed, new)?.record;
    if old.kind != new.kind || old.size != new.size || old.count != new.count {
        return Ok(false);
    }
    Ok(match new.kind {
        // The capacities are the counts, compared above.
        kind::STRING | kind::WSTRING => true,
        kind::ENUM => {
            let mut same = true;
            for ((old_name, old_number), (new_name, new_number)) in
                running.variants(&old)?.zip(changed.variants(&new)?)
            {
                if old_number != new_number
                    || !running
                        .string(old_name)?
                        .eq_ignore_ascii_case(changed.string(new_name)?)
                {
                    same = false;
                    break;
                }
            }
            same
        }
        kind::SUBRANGE => running.subrange(&old)? == changed.subrange(&new)?,
        kind::ARRAY => {
            let (old_element, old_bounds) = running.array(&old)?;
            let (new_element, new_bounds) = changed.array(&new)?;
            old_bounds.eq(new_bounds)
                && same_at(running, old_element, changed, new_element, depth + 1)?
        }
        kind::STRUCT | kind::UNION | kind::FUNCTION_BLOCK | kind::CLASS | kind::PROGRAM => {
            let named = is_instance(new.kind);
            if named
                && !running
                    .string(old.name)?
                    .eq_ignore_ascii_case(changed.string(new.name)?)
            {
                return Ok(false);
            }
            let mut same = true;
            for (a, b) in running.members(&old)?.zip(changed.members(&new)?) {
                if a.offset != b.offset
                    || a.flags & NOT_CARRIED != b.flags & NOT_CARRIED
                    || !running
                        .string(a.name)?
                        .eq_ignore_ascii_case(changed.string(b.name)?)
                    || !same_at(running, a.type_index, changed, b.type_index, depth + 1)?
                {
                    same = false;
                    break;
                }
            }
            same
        }
        kind::REFERENCE | kind::POINTER => same_at(
            running,
            word(running, &old)?,
            changed,
            word(changed, &new)?,
            depth + 1,
        )?,
        kind::INTERFACE => running
            .string(old.name)?
            .eq_ignore_ascii_case(changed.string(new.name)?),
        other => width(other).is_some(),
    })
}

/// Whether a value of type `index` is carried as its bytes are: it holds no reference, pointer or
/// interface value, which a change names anew, no class id, which is always the new program's,
/// and no in-out or constant member, which a change does not carry.
pub(super) fn plain(types: &Types<'_>, index: u32) -> Result<bool, RuntimeError> {
    plain_at(types, index, 0)
}

fn plain_at(types: &Types<'_>, index: u32, depth: usize) -> Result<bool, RuntimeError> {
    if depth > MAX_TYPE_NESTING {
        return Err(damaged("type table"));
    }
    let ty = resolve(types, index)?.record;
    Ok(match ty.kind {
        kind::REFERENCE | kind::POINTER | kind::INTERFACE => false,
        kind::ARRAY => plain_at(types, types.array(&ty)?.0, depth + 1)?,
        kind::STRUCT | kind::UNION | kind::FUNCTION_BLOCK | kind::CLASS | kind::PROGRAM => {
            let mut plain = true;
            for member in types.members(&ty)? {
                if member.flags & NOT_CARRIED != 0
                    || types.string(member.name)?.eq_ignore_ascii_case(CLASS)
                    || !plain_at(types, member.type_index, depth + 1)?
                {
                    plain = false;
                    break;
                }
            }
            plain
        }
        _ => true,
    })
}

/// Whether the members of the structure `ty` share bytes, as a union's and a `STRUCT OVERLAP`'s
/// do, so that its value is kept only into the same layout (specification 44 §12.2). Members that
/// lie out of their order count as sharing.
pub(super) fn overlapping(types: &Types<'_>, ty: &Ty) -> Result<bool, RuntimeError> {
    if ty.kind() == kind::UNION {
        return Ok(true);
    }
    let mut end = 0u64;
    for member in types.members(&ty.record)? {
        let offset = u64::from(member.offset);
        if offset < end {
            return Ok(true);
        }
        end = offset + u64::from(resolve(types, member.type_index)?.size());
    }
    Ok(false)
}

/// Whether `kind` is a function block's, a class's or a program's: its values are instances.
pub(super) fn is_instance(kind: u32) -> bool {
    matches!(kind, kind::FUNCTION_BLOCK | kind::CLASS | kind::PROGRAM)
}

/// Whether `kind` is a structure's or a union's.
pub(super) fn is_structure(kind: u32) -> bool {
    matches!(kind, kind::STRUCT | kind::UNION)
}

/// A member of a structure or an instance as a change walks it.
#[derive(Clone, Copy)]
pub(super) struct Member<'i> {
    /// Its declared name.
    pub(super) name: &'i str,
    /// Its symbol flags.
    pub(super) flags: u32,
    /// Its type.
    pub(super) ty: Ty,
    /// The offset of its first byte in its area.
    pub(super) offset: u32,
    /// The name of the type that declares it.
    pub(super) owner: &'i str,
}

/// The first member, in their order, of the structure or instance of type `ty` at `at` that
/// `wanted` takes; an instance's base's members, held through `__BASE`, count as its own, where
/// they lie. Each member lies within its owner's bytes, or the type table is damaged.
pub(super) fn find_member<'i>(
    types: &Types<'i>,
    ty: &Ty,
    at: u32,
    wanted: &mut dyn FnMut(&Member<'i>) -> Result<bool, RuntimeError>,
) -> Result<Option<Member<'i>>, RuntimeError> {
    find_at(types, ty, at, wanted, 0)
}

/// Each member of the structure or instance of type `ty` at `at`, as [`find_member`] finds them.
pub(super) fn each_member<'i>(
    types: &Types<'i>,
    ty: &Ty,
    at: u32,
    each: &mut dyn FnMut(Member<'i>) -> Result<(), RuntimeError>,
) -> Result<(), RuntimeError> {
    find_at(
        types,
        ty,
        at,
        &mut |member| each(*member).map(|()| false),
        0,
    )
    .map(|_| ())
}

fn find_at<'i>(
    types: &Types<'i>,
    ty: &Ty,
    at: u32,
    wanted: &mut dyn FnMut(&Member<'i>) -> Result<bool, RuntimeError>,
    depth: usize,
) -> Result<Option<Member<'i>>, RuntimeError> {
    if depth > MAX_TYPE_NESTING {
        return Err(damaged("type table"));
    }
    let owner = types.string(ty.record.name)?;
    for detail in types.members(&ty.record)? {
        let name = types.string(detail.name)?;
        let member_ty = resolve(types, detail.type_index)?;
        let inside = u64::from(detail.offset) + u64::from(member_ty.size()) <= u64::from(ty.size());
        let offset = at
            .checked_add(detail.offset)
            .filter(|_| inside)
            .ok_or_else(|| damaged("type table"))?;
        if name.eq_ignore_ascii_case(BASE) && is_instance(member_ty.kind()) {
            if let Some(found) = find_at(types, &member_ty, offset, wanted, depth + 1)? {
                return Ok(Some(found));
            }
            continue;
        }
        let member = Member {
            name,
            flags: detail.flags,
            ty: member_ty,
            offset,
            owner,
        };
        if wanted(&member)? {
            return Ok(Some(member));
        }
    }
    Ok(None)
}

/// Whether an instance of `ty` holds a class id: its first member, a base's through `__BASE`, is
/// `__CLASS`, as the instance of a type that implements an interface or takes part in inheritance
/// holds it (specification 44 §6.4).
pub(super) fn holds_class(types: &Types<'_>, ty: &Ty) -> Result<bool, RuntimeError> {
    if !is_instance(ty.kind()) {
        return Ok(false);
    }
    let first = find_member(types, ty, 0, &mut |_| Ok(true))?;
    Ok(first.is_some_and(|member| member.offset == 0 && member.name.eq_ignore_ascii_case(CLASS)))
}

/// Whether an instance of `derived` is an instance of `base`: the two have the same shape, or the
/// members of `base`, its own base's among them, are the first members of `derived`, at the same
/// offsets and of the same types, as a derived type lies (specification 44 §7.2). The type table
/// names no type's base, so the layout is what a change can check.
pub(super) fn base_of(types: &Types<'_>, base: &Ty, derived: &Ty) -> Result<bool, RuntimeError> {
    if !is_instance(base.kind()) || !is_instance(derived.kind()) || base.size() > derived.size() {
        return Ok(false);
    }
    if alike(types, base.index, derived.index)? {
        return Ok(true);
    }
    let mut expected = Vec::new();
    each_member(types, base, 0, &mut |member| {
        reserve(&mut expected, 1)?;
        expected.push(member);
        Ok(())
    })?;
    if expected.is_empty() {
        return Ok(false);
    }
    let mut matched = 0;
    let mut differs = false;
    find_member(types, derived, 0, &mut |member| {
        let Some(wanted) = expected.get(matched) else {
            return Ok(true);
        };
        if wanted.offset != member.offset
            || !wanted.name.eq_ignore_ascii_case(member.name)
            || !alike(types, wanted.ty.index, member.ty.index)?
        {
            differs = true;
            return Ok(true);
        }
        matched += 1;
        Ok(matched == expected.len())
    })?;
    Ok(!differs && matched == expected.len())
}

/// An array type: its element type, the bounds of its dimensions and its number of elements, which
/// lie in a row, the last index fastest (specification 44 §7.2).
pub(super) struct Array {
    /// The element type.
    pub(super) element: Ty,
    /// The lower and upper bound of each dimension.
    pub(super) dims: Vec<(i64, i64)>,
    /// The number of elements.
    pub(super) count: u64,
}

impl Array {
    /// The array type `ty` of `types`: at least one dimension, each of at least one element, and
    /// every element within the array's bytes, or the type table is damaged.
    pub(super) fn of(types: &Types<'_>, ty: &Ty) -> Result<Self, RuntimeError> {
        let (element, bounds) = types.array(&ty.record)?;
        let element = resolve(types, element)?;
        let mut dims = Vec::new();
        reserve(&mut dims, bounds.len())?;
        let mut count = 1u64;
        for (lower, upper) in bounds {
            count = length(lower, upper)
                .and_then(|length| count.checked_mul(length))
                .ok_or_else(|| damaged("type table"))?;
            dims.push((lower, upper));
        }
        let fits = count
            .checked_mul(u64::from(element.size()))
            .is_some_and(|bytes| bytes <= u64::from(ty.size()));
        if dims.is_empty() || !fits {
            return Err(damaged("type table"));
        }
        Ok(Self {
            element,
            dims,
            count,
        })
    }

    /// The flat position of the element at `indices`, or `None` when an index lies outside its
    /// bounds.
    pub(super) fn position(&self, indices: &[i64]) -> Option<u64> {
        if indices.len() != self.dims.len() {
            return None;
        }
        let mut position = 0u64;
        for (&(lower, upper), &index) in self.dims.iter().zip(indices) {
            if !(lower..=upper).contains(&index) {
                return None;
            }
            let offset = u64::try_from(index.checked_sub(lower)?).ok()?;
            position = position
                .checked_mul(length(lower, upper)?)?
                .checked_add(offset)?;
        }
        Some(position)
    }

    /// The indices of the element at flat `position`, into `indices`; `None` past the last element.
    pub(super) fn indices(&self, position: u64, indices: &mut Vec<i64>) -> Option<()> {
        if position >= self.count {
            return None;
        }
        indices.clear();
        let (mut rest, mut block) = (position, self.count);
        for &(lower, upper) in &self.dims {
            block = block.checked_div(length(lower, upper)?)?;
            let offset = rest.checked_div(block)?;
            rest = rest.checked_rem(block)?;
            indices.push(lower.checked_add(i64::try_from(offset).ok()?)?);
        }
        Some(())
    }

    /// Steps `indices` to the next element in flat order, the last index fastest; `false` after
    /// the last element.
    pub(super) fn step(&self, indices: &mut [i64]) -> bool {
        for (index, &(lower, upper)) in indices.iter_mut().zip(&self.dims).rev() {
            if *index < upper {
                *index += 1;
                return true;
            }
            *index = lower;
        }
        false
    }
}

/// The number of elements from `lower` to `upper`, at least one.
fn length(lower: i64, upper: i64) -> Option<u64> {
    upper
        .checked_sub(lower)?
        .checked_add(1)
        .and_then(|length| u64::try_from(length).ok())
        .filter(|length| *length > 0)
}
