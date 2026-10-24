//! The references, pointers and interface values an online change carries (specification 44 §7.3,
//! §12.2): each names the same variable or instance in the new program when the new program has it
//! with the type the reference names, and is `NULL` otherwise.
//!
//! A reference names its target by place: an area, an offset and an extent. The change finds what
//! lies there in the running program, from the top-level variable that holds it, a global or a
//! variable of a program instance, through members and elements by the type table, and follows the
//! same path in the new program: the variable of the same name, the members of the same names, the
//! elements at the same indices. A reference to a variable names one of the type the reference
//! names; a character reference names a text, whose capacity holds its character; a reference of
//! kind instance, an interface value or a reference taken as a function block, names an instance:
//! an interface value one that holds a class id, a reference one of the type it names or of a type
//! derived from it. The type table says which interface a variable has but not which classes
//! implement it, so an interface value goes to the instance at the same place, whatever its class.

use alloc::vec::Vec;

use trust_plc_abi::module::GENERATION;
use trust_plc_abi::{RefValue, AREA_GLOBALS, AREA_INSTANCES, AREA_RETAIN};

use super::plan::fetch;
use super::shape::{
    alike, base_of, find_member, holds_class, is_instance, is_structure, resolve, word, Array, Ty,
};
use super::{reserve, Carry, Kind, Node, Role, Tables, Walk};
use crate::bytecode::BYTECODE_MAX_TYPE_NESTING;
use crate::error::RuntimeError;
use crate::native::image::type_kind as kind;
use crate::native::values::{damaged, Types};

/// The bytes of a reference (specification 44 §7.3).
const BYTES: usize = 16;

/// A top-level variable of the running program, a global or a variable of a program instance, by
/// the bytes it spans in its area.
#[derive(Clone, Copy)]
pub(super) struct Root {
    area: u8,
    offset: u32,
    end: u32,
    symbol: u32,
}

/// The running program's top-level variables, ordered by area and offset.
fn roots(running: &Tables<'_>) -> Result<Vec<Root>, RuntimeError> {
    let mut roots = Vec::new();
    for (index, symbol) in (0u32..).zip(running.symbols.iter()) {
        if !matches!(running.role(&symbol)?, Role::Global | Role::Variable { .. }) {
            continue;
        }
        let end = symbol
            .offset
            .checked_add(symbol.size)
            .ok_or_else(|| damaged("symbol table"))?;
        reserve(&mut roots, 1)?;
        roots.push(Root {
            area: symbol.area,
            offset: symbol.offset,
            end,
            symbol: index,
        });
    }
    roots.sort_unstable_by_key(|root| (root.area, root.offset));
    Ok(roots)
}

impl Carry {
    /// The symbol of the running program's top-level variable whose bytes in `area` hold those from
    /// `start` to `end`; the variables are ordered by place at the first reference a change meets.
    fn root(
        &mut self,
        running: &Tables<'_>,
        area: u8,
        start: u32,
        end: u32,
    ) -> Result<Option<u32>, RuntimeError> {
        if self.roots.is_none() {
            self.roots = Some(roots(running)?);
        }
        let roots = self.roots.as_deref().unwrap_or_default();
        let after = roots.partition_point(|root| (root.area, root.offset) <= (area, start));
        Ok(after
            .checked_sub(1)
            .and_then(|index| roots.get(index))
            .filter(|root| root.area == area && end <= root.end)
            .map(|root| root.symbol))
    }
}

impl Walk<'_, '_, '_> {
    /// The reference, pointer or interface value at `old` of type `old_ty` into `new` of type
    /// `new_ty`: `NULL` stays `NULL`; one that names a variable or instance the new program has,
    /// with the type the new reference type names, names it there, in generation 1 (specification
    /// 44 §7.3); any other is `NULL` and records `Cleared`.
    pub(super) fn reference(
        &mut self,
        old: Node,
        old_ty: &Ty,
        new: Node,
        new_ty: &Ty,
    ) -> Result<(), RuntimeError> {
        let mut bytes = [0u8; BYTES];
        fetch(self.memory, old.spot(), &mut bytes)?;
        let held = decode(&bytes);
        if held.kind == RefValue::NULL {
            return self
                .carry
                .plan
                .write(new.spot(), &encode(&RefValue::NULL_REF));
        }
        match self.repoint(&held, old_ty, new_ty)? {
            Some(named) => self.carry.plan.write(new.spot(), &encode(&named)),
            None => {
                self.carry
                    .plan
                    .write(new.spot(), &encode(&RefValue::NULL_REF))?;
                self.record(Kind::Cleared, None);
                Ok(())
            }
        }
    }

    /// The reference that names in the new program, as a reference of type `new_ty`, what `held`,
    /// a reference of type `old_ty`, names in the running program; `None` when the new program does
    /// not have it with the type the new reference names.
    fn repoint(
        &mut self,
        held: &RefValue,
        old_ty: &Ty,
        new_ty: &Ty,
    ) -> Result<Option<RefValue>, RuntimeError> {
        let (running, changed) = (self.running, self.changed);
        // A reference of another generation names nothing, and none names a frame (§7.3).
        if held.generation != GENERATION
            || !matches!(held.area, AREA_GLOBALS | AREA_RETAIN | AREA_INSTANCES)
        {
            return Ok(None);
        }
        let Some(end) = held.offset.checked_add(held.extent.max(1)) else {
            return Ok(None);
        };
        let Some(root) = self.carry.root(running, held.area, held.offset, end)? else {
            return Ok(None);
        };
        let symbol = running.symbol(root)?;
        let global = running.role(&symbol)? == Role::Global;
        let Some(counterpart) = changed.variable(running.string(symbol.canonical)?, global)? else {
            return Ok(None);
        };
        let old_named = named_type(&running.types, old_ty)?;
        let new_named = named_type(&changed.types, new_ty)?;
        let (mut from, mut to) = (Node::of(&symbol), Node::of(&counterpart));
        for _ in 0..=BYTECODE_MAX_TYPE_NESTING {
            let old_at = resolve(&running.types, from.ty)?;
            let new_at = resolve(&changed.types, to.ty)?;
            if from.offset == held.offset
                && names(&running.types, held, &old_at, old_named.as_ref())?
            {
                return named_anew(&changed.types, held, to, &new_at, new_named.as_ref());
            }
            let Some((next_from, next_to)) =
                self.inner(held, end, (from, &old_at), (to, &new_at))?
            else {
                return Ok(None);
            };
            (from, to) = (next_from, next_to);
        }
        Err(damaged("type table"))
    }

    /// The member or element of the running program's value `from` that holds the reference's
    /// target, which ends at `end`, and the new program's member of the same name or element at the
    /// same indices in `to`; `None` when either has none.
    fn inner(
        &self,
        held: &RefValue,
        end: u32,
        (from, old_ty): (Node, &Ty),
        (to, new_ty): (Node, &Ty),
    ) -> Result<Option<(Node, Node)>, RuntimeError> {
        let (running, changed) = (self.running, self.changed);
        let compound = |of: u32| is_structure(of) || is_instance(of);
        if compound(old_ty.kind()) && compound(new_ty.kind()) {
            let holder = find_member(&running.types, old_ty, from.offset, &mut |member| {
                Ok(member.offset <= held.offset
                    && u64::from(end) <= u64::from(member.offset) + u64::from(member.ty.size()))
            })?;
            let Some(holder) = holder else {
                return Ok(None);
            };
            let counterpart = find_member(&changed.types, new_ty, to.offset, &mut |member| {
                Ok(member.name.eq_ignore_ascii_case(holder.name))
            })?;
            return Ok(
                counterpart.map(|counterpart| (from.member(&holder), to.member(&counterpart)))
            );
        }
        if old_ty.kind() != kind::ARRAY || new_ty.kind() != kind::ARRAY {
            return Ok(None);
        }
        let old_array = Array::of(&running.types, old_ty)?;
        let new_array = Array::of(&changed.types, new_ty)?;
        let stride = u64::from(old_array.element.size());
        let relative = u64::from(held.offset.saturating_sub(from.offset));
        let Some(position) = relative.checked_div(stride) else {
            return Ok(None);
        };
        // The element holds the whole target.
        let element_end = position
            .checked_add(1)
            .and_then(|elements| elements.checked_mul(stride))
            .and_then(|bytes| bytes.checked_add(u64::from(from.offset)));
        if element_end.is_none_or(|element_end| u64::from(end) > element_end) {
            return Ok(None);
        }
        let mut indices = Vec::new();
        reserve(&mut indices, old_array.dims.len())?;
        if old_array.indices(position, &mut indices).is_none() {
            return Ok(None);
        }
        let Some(target) = new_array.position(&indices) else {
            return Ok(None);
        };
        Ok(Some((
            from.element(&old_array.element, position)?,
            to.element(&new_array.element, target)?,
        )))
    }
}

/// The type a reference type `ty` names: the target of a `REF_TO` or a `POINTER TO`; none for an
/// interface, whose values name instances of the classes that implement it.
fn named_type(types: &Types<'_>, ty: &Ty) -> Result<Option<Ty>, RuntimeError> {
    if ty.kind() == kind::INTERFACE {
        return Ok(None);
    }
    resolve(types, word(types, &ty.record)?).map(Some)
}

/// Whether `held`, a reference that names a type `named` (none for an interface value), names the
/// running program's value of type `ty` at its offset.
fn names(
    types: &Types<'_>,
    held: &RefValue,
    ty: &Ty,
    named: Option<&Ty>,
) -> Result<bool, RuntimeError> {
    Ok(match (held.kind, named) {
        (RefValue::VARIABLE | RefValue::POINTER, Some(named)) => {
            ty.size() == held.extent && alike(types, ty.index, named.index)?
        }
        (RefValue::CHARACTER, Some(named)) => {
            ty.size() == held.extent
                && character_of(ty, named)
                && (1..=ty.record.count).contains(&u32::from(held.char_index))
        }
        (RefValue::INSTANCE, None) => held.extent <= ty.size() && holds_class(types, ty)?,
        (RefValue::INSTANCE, Some(named)) => held.extent <= ty.size() && base_of(types, named, ty)?,
        _ => false,
    })
}

/// The reference that names the new program's value of type `ty` at `at` as `held` named the
/// running program's, when `ty` is what the new reference type names (`named`, none for an
/// interface value): the reference's kind and character index kept, its extent the type it names.
fn named_anew(
    types: &Types<'_>,
    held: &RefValue,
    at: Node,
    ty: &Ty,
    named: Option<&Ty>,
) -> Result<Option<RefValue>, RuntimeError> {
    let extent = match (held.kind, named) {
        (RefValue::VARIABLE | RefValue::POINTER, Some(named))
            if alike(types, ty.index, named.index)? =>
        {
            ty.size()
        }
        (RefValue::CHARACTER, Some(named))
            if character_of(ty, named) && u32::from(held.char_index) <= ty.record.count =>
        {
            ty.size()
        }
        (RefValue::INSTANCE, None) if holds_class(types, ty)? => ty.size(),
        (RefValue::INSTANCE, Some(named)) if base_of(types, named, ty)? => named.size(),
        _ => return Ok(None),
    };
    Ok(Some(RefValue {
        area: at.area,
        kind: held.kind,
        char_index: held.char_index,
        offset: at.offset,
        extent,
        generation: GENERATION,
    }))
}

/// Whether `text` is a text of the characters `character` names: a `STRING` of `CHAR`, a `WSTRING`
/// of `WCHAR`.
fn character_of(text: &Ty, character: &Ty) -> bool {
    matches!(
        (text.kind(), character.kind()),
        (kind::STRING, kind::CHAR) | (kind::WSTRING, kind::WCHAR)
    )
}

/// The reference in `bytes`, laid out as `RefValue` is (specification 44 §7.3).
fn decode(bytes: &[u8; BYTES]) -> RefValue {
    let [area, sort, c0, c1, o0, o1, o2, o3, e0, e1, e2, e3, g0, g1, g2, g3] = *bytes;
    RefValue {
        area,
        kind: sort,
        char_index: u16::from_le_bytes([c0, c1]),
        offset: u32::from_le_bytes([o0, o1, o2, o3]),
        extent: u32::from_le_bytes([e0, e1, e2, e3]),
        generation: u32::from_le_bytes([g0, g1, g2, g3]),
    }
}

/// The bytes of `reference`, laid out as `RefValue` is.
fn encode(reference: &RefValue) -> [u8; BYTES] {
    let mut bytes = [0u8; BYTES];
    bytes[0] = reference.area;
    bytes[1] = reference.kind;
    bytes[2..4].copy_from_slice(&reference.char_index.to_le_bytes());
    bytes[4..8].copy_from_slice(&reference.offset.to_le_bytes());
    bytes[8..12].copy_from_slice(&reference.extent.to_le_bytes());
    bytes[12..16].copy_from_slice(&reference.generation.to_le_bytes());
    bytes
}
