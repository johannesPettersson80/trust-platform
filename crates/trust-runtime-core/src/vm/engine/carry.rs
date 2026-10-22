//! The values an online change carries from the running program into its candidate (specification
//! 34 §11.3; decision OC-1, 19 October 2026, and the owner's "keep what fits", 20 October 2026).
//!
//! The candidate is constructed with its own declarations' initial values. This pass then gives each
//! variable of the candidate that the running program declares at the same place, by the same name,
//! the running program's value, retained or not: a global, a function static, a variable of a
//! program instance, and the members of every function-block instance they hold, recursively. Names
//! are compared without regard to ASCII case, as IEC names are.
//!
//! - A value whose declared type has the same shape in both programs ([`shape::same`]) is copied as
//!   bytes, except that an instance leaf keeps the candidate's instance, whose members are carried in
//!   turn, and a reference or interface leaf is carried last.
//! - A value whose declaration changed keeps what fits: an elementary value converted to its new type
//!   when that type holds it ([`fit::elementary`]), a structure's fields by name, an array's elements
//!   at the indices both arrays have, an instance's members by name. Anything else keeps its initial
//!   value.
//! - A reference or interface value, once every instance is known, names the same variable in the
//!   candidate when the candidate has it with the type the reference names, and is NULL otherwise.
//! - Constants are not carried (the new program's value wins), nor are in-out members, which every
//!   call binds again, aliases, compiler slots and frames. A function static, a method static and a
//!   native-state slot (a timer's state) are carried only when the running program gave them a value;
//!   a native-state slot only into an instance of the same function block.
//!
//! The change never fails because of a value. Each variable that does not go on with the running
//! value as it was is recorded as an event ([`CarriedValueEvent`]), at most [`MAX_EVENTS`] of them
//! and then their count. A byte copy builds no value and allocates nothing.

use alloc::string::String;
use core::fmt::Write as _;

use super::*;
use crate::bytecode::{StorageOwner, StorageRole, TypeData};
use crate::memory::layout::ByteRange;
use crate::memory::view::StoreAccess;
use crate::memory::MemoryLocation;

mod fit;
mod places;
mod shape;

use places::{
    carried, copy, declared_name, element_count, element_range, element_total, field_range,
    holds_leaves, matching_global, member_range, next_index, persistent, pou_name, same_kind,
    stride,
};

/// The most variables an online change records an event for; the others are counted in one more
/// event, so that the change takes a bounded amount of memory for them.
pub const MAX_EVENTS: usize = 32;

/// What an online change did with the value of one variable of its candidate that does not go on
/// with the running program's value as it was (specification 34 §11.3). `name` is the variable's
/// path: `total`, `Plant.motor.speed`, `Plant.table[3]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CarriedValueEvent {
    /// The running program declares no such variable at this place: it starts with its declared
    /// initial value.
    New {
        /// The variable's path.
        name: SmolStr,
    },
    /// Its declaration changed, and the running value was converted to the new type, which holds it.
    Converted {
        /// The variable's path.
        name: SmolStr,
    },
    /// Its declaration changed, and the new type does not hold the running value: it starts with
    /// its declared initial value.
    NotKept {
        /// The variable's path.
        name: SmolStr,
    },
    /// A reference or interface value whose target the candidate does not have, or has with another
    /// type: it is NULL.
    Cleared {
        /// The variable's path.
        name: SmolStr,
    },
    /// The number of further variables, beyond [`MAX_EVENTS`], that did not go on with their running
    /// values as they were.
    More {
        /// How many.
        count: usize,
    },
}

impl EngineState<'_> {
    /// Give this candidate of an online change the values of `source`, the program it replaces, as
    /// the module documentation says, and publish the marker image the first scan loads. Returns the
    /// events of the variables that did not keep their running values as they were.
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of an allowance or of the physical deadline the work exceeds, or of a
    /// store that is not as construction left it. A value never refuses the change.
    pub(super) fn carry_running_values(
        &mut self,
        source: &EngineState<'_>,
    ) -> Result<Vec<CarriedValueEvent>, RuntimeError> {
        let mut carry = Carry::new(source)?;
        carry.globals(self)?;
        carry.programs(self)?;
        carry.references(self)?;
        // A carried variable located at %M must show its value in the marker image the first scan
        // loads (specification 11 §6.7).
        self.publish_marker_image()?;
        Ok(carry.events.finish())
    }
}

/// One step of a variable's path.
#[derive(Clone, Copy)]
enum Segment<'n> {
    Name(&'n str),
    Index(i64),
}

/// A reference or interface leaf of the candidate, carried once every instance is known.
struct Deferred<'n> {
    /// The running program's leaf.
    source: ByteRange,
    /// Its type in the running program.
    source_type: u32,
    /// The candidate's leaf.
    target: ByteRange,
    /// Its type in the candidate.
    target_type: u32,
    /// The path of the variable that holds it, for its event.
    path: Vec<Segment<'n>>,
}

/// What happened to one variable's value, for its event.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    New,
    Converted,
    NotKept,
    Cleared,
}

/// The events of one change: at most [`MAX_EVENTS`], then their count. While the elements of an
/// array whose element type changed are carried, their events are muted and the array records one,
/// so that one variable takes one event, whatever its length.
struct Events {
    list: Vec<CarriedValueEvent>,
    more: usize,
    /// How many arrays the elements being carried belong to whose element type changed.
    muted: usize,
    /// Whether a muted element did not keep its value.
    lost: bool,
}

impl Events {
    fn new() -> Result<Self, RuntimeError> {
        let mut list = Vec::new();
        // Bookkeeping of the change (TIME-08), reserved once.
        list.try_reserve_exact(MAX_EVENTS + 1)
            .map_err(|_| RuntimeError::Overflow)?;
        Ok(Self {
            list,
            more: 0,
            muted: 0,
            lost: false,
        })
    }

    fn finish(mut self) -> Vec<CarriedValueEvent> {
        if self.more > 0 {
            self.list.push(CarriedValueEvent::More { count: self.more });
        }
        self.list
    }
}

struct Carry<'s, 'p, 'n> {
    source: &'s EngineState<'p>,
    /// Each running instance and the candidate's instance at the same place.
    instances: BTreeMap<u32, u32>,
    deferred: Vec<Deferred<'n>>,
    events: Events,
    /// The path of the variable being carried.
    path: Vec<Segment<'n>>,
}

impl<'s, 'p, 'n> Carry<'s, 'p, 'n> {
    fn new(source: &'s EngineState<'p>) -> Result<Self, RuntimeError> {
        Ok(Self {
            source,
            instances: BTreeMap::new(),
            deferred: Vec::new(),
            events: Events::new()?,
            path: Vec::new(),
        })
    }

    /// The path of the variable being carried, with `last` after it when given.
    fn name(&self, last: Option<Segment<'_>>) -> SmolStr {
        let mut text = String::new();
        for segment in self.path.iter().copied().chain(last) {
            match segment {
                Segment::Name(name) => {
                    if !text.is_empty() {
                        text.push('.');
                    }
                    text.push_str(name);
                }
                Segment::Index(index) => {
                    let _ = write!(text, "[{index}]");
                }
            }
        }
        SmolStr::new(text)
    }

    /// Record what happened to the variable being carried, or to its member `last`.
    fn record(&mut self, kind: Kind, last: Option<Segment<'_>>) {
        if self.events.muted > 0 {
            self.events.lost |= matches!(kind, Kind::NotKept | Kind::Cleared);
            return;
        }
        if self.events.list.len() >= MAX_EVENTS {
            self.events.more = self.events.more.saturating_add(1);
            return;
        }
        let name = self.name(last);
        self.events.list.push(match kind {
            Kind::New => CarriedValueEvent::New { name },
            Kind::Converted => CarriedValueEvent::Converted { name },
            Kind::NotKept => CarriedValueEvent::NotKept { name },
            Kind::Cleared => CarriedValueEvent::Cleared { name },
        });
    }

    fn record_new(&mut self, last: Segment<'_>) {
        self.record(Kind::New, Some(last));
    }

    /// The globals and the function statics of the candidate.
    fn globals(&mut self, target: &mut EngineState<'n>) -> Result<(), RuntimeError> {
        let after: &'n PreparedModule = target.prepared;
        let before = self.source.prepared;
        for (id, declaration) in after.storage_declarations().enumerate() {
            if declaration.owner != StorageOwner::Global || !carried(&declaration) {
                continue;
            }
            let id = u32::try_from(id).map_err(|_| RuntimeError::Overflow)?;
            let name = declared_name(after, &declaration)?;
            let owner = match declaration.owner_pou_id {
                Some(function) => Some(pou_name(after, function)?),
                None => None,
            };
            if let Some(owner) = owner {
                self.path.push(Segment::Name(owner));
            }
            match matching_global(before, after, &declaration) {
                Some(old) => {
                    let source = before
                        .global_range(old)
                        .ok_or(RuntimeError::InvalidExecutionState)?;
                    let at = after
                        .global_range(id)
                        .ok_or(RuntimeError::InvalidExecutionState)?;
                    self.path.push(Segment::Name(name));
                    self.slot(target, (old, source), (id, at), None)?;
                    self.path.pop();
                }
                None => self.record_new(Segment::Name(name)),
            }
            if owner.is_some() {
                self.path.pop();
            }
            target.check_entry_deadline()?;
        }
        Ok(())
    }

    /// The program instances of the candidate, by the names of their roots.
    fn programs(&mut self, target: &mut EngineState<'n>) -> Result<(), RuntimeError> {
        let after: &'n PreparedModule = target.prepared;
        let before = self.source.prepared;
        for (index, root) in after.roots.entries.iter().enumerate() {
            let declaration = after
                .storage_declaration(root.declaration_idx as usize)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            if root.parent_root_idx.is_some() || declaration.role != StorageRole::ProgramRoot {
                continue;
            }
            let Some(new) = target.data.construction.roots.get(index).copied().flatten() else {
                continue;
            };
            let name = declared_name(after, &declaration)?;
            let old = before
                .roots
                .entries
                .iter()
                .enumerate()
                .find_map(|(old_index, old_root)| {
                    let old_declaration =
                        before.storage_declaration(old_root.declaration_idx as usize)?;
                    (old_root.parent_root_idx.is_none()
                        && old_declaration.role == StorageRole::ProgramRoot
                        && declared_name(before, &old_declaration)
                            .is_ok_and(|old_name| old_name.eq_ignore_ascii_case(name)))
                    .then(|| {
                        self.source
                            .data
                            .construction
                            .roots
                            .get(old_index)
                            .copied()
                            .flatten()
                    })
                    .flatten()
                });
            match old {
                Some(old) => {
                    self.path.push(Segment::Name(name));
                    self.instance(target, old, new)?;
                    self.path.pop();
                }
                None => self.record_new(Segment::Name(name)),
            }
            target.check_entry_deadline()?;
        }
        Ok(())
    }

    /// The members of the candidate's instance `new` from the running instance `old` at the same
    /// place, by name, and then the members their bases hold.
    fn instance(
        &mut self,
        target: &mut EngineState<'n>,
        old: InstanceId,
        new: InstanceId,
    ) -> Result<(), RuntimeError> {
        if self.instances.contains_key(&old.0) {
            return Ok(());
        }
        // Bookkeeping of the change (TIME-08); the map's overflow guard stays.
        self.instances
            .len()
            .checked_add(1)
            .ok_or(RuntimeError::Overflow)?;
        self.instances.insert(old.0, new.0);
        let after: &'n PreparedModule = target.prepared;
        let before = self.source.prepared;
        let (old_pou, old_range, old_parent) = persistent(self.source, old)?;
        let (new_pou, new_range, new_parent) = persistent(target, new)?;
        let same_block = pou_name(before, old_pou)?.eq_ignore_ascii_case(pou_name(after, new_pou)?);
        for &id in after.declarations(StorageOwner::Instance, Some(new_pou)) {
            let declaration = after
                .storage_declaration(id as usize)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            if !carried(&declaration)
                || (declaration.role == StorageRole::NativeState && !same_block)
            {
                continue;
            }
            let name = declared_name(after, &declaration)?;
            let old_id = before
                .declarations(StorageOwner::Instance, Some(old_pou))
                .iter()
                .copied()
                .find(|&old_id| {
                    before
                        .storage_declaration(old_id as usize)
                        .is_some_and(|old| {
                            same_kind(old.role, declaration.role)
                                && carried(&old)
                                && declared_name(before, &old)
                                    .is_ok_and(|old_name| old_name.eq_ignore_ascii_case(name))
                        })
                });
            let Some(old_id) = old_id else {
                // A hidden slot of a new block is no variable of the program's.
                if matches!(
                    declaration.role,
                    StorageRole::Variable | StorageRole::Parameter | StorageRole::Static
                ) {
                    self.record_new(Segment::Name(name));
                }
                continue;
            };
            let source = member_range(before, old_range, old_id)?;
            let at = member_range(after, new_range, id)?;
            self.path.push(Segment::Name(name));
            self.slot(target, (old_id, source), (id, at), Some((old, new)))?;
            self.path.pop();
        }
        if let (Some(old_parent), Some(new_parent)) = (old_parent, new_parent) {
            self.instance(target, old_parent, new_parent)?;
        }
        Ok(())
    }

    /// One declared slot: the value of declaration `old` at `source` into declaration `new` at `at`.
    /// A static or a native-state slot is carried only when the running program gave it a value, and
    /// then the candidate's slot holds one too; a static keeps its once-marker, so its first use does
    /// not initialize it again. `owners` are the running and the candidate's instance of an instance
    /// member.
    fn slot(
        &mut self,
        target: &mut EngineState<'n>,
        (old, source): (u32, ByteRange),
        (new, at): (u32, ByteRange),
        owners: Option<(InstanceId, InstanceId)>,
    ) -> Result<(), RuntimeError> {
        let before = self.source.prepared;
        let after: &'n PreparedModule = target.prepared;
        let old_declaration = before
            .storage_declaration(old as usize)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let new_declaration = after
            .storage_declaration(new as usize)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let old_type = old_declaration
            .type_id
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let new_type = new_declaration
            .type_id
            .ok_or(RuntimeError::InvalidExecutionState)?;
        if let Some(presence) = before.presence_offset(old, source.start) {
            let flag = ByteRange {
                start: presence,
                bytes: 1,
                ..source
            };
            let held = self
                .source
                .data
                .reader(before)
                .view_cells()
                .bytes(flag)
                .is_some_and(|byte| byte.first() == Some(&1));
            if !held {
                return Ok(());
            }
        }
        self.value(target, (source, old_type), (at, new_type))?;
        if let Some(presence) = after.presence_offset(new, at.start) {
            let flag = ByteRange {
                start: presence,
                bytes: 1,
                ..at
            };
            if !target.data.store.write_bytes(&flag, &[1]) {
                return Err(RuntimeError::InvalidExecutionState);
            }
        }
        if new_declaration.role == StorageRole::Static {
            self.once_marker(target, old, new, owners)?;
        }
        Ok(())
    }

    /// The candidate's once-marker and initialization marker of the static `new`, when the running
    /// program holds them for `old`: a function static's belong to no instance, a method static's to
    /// the instance that holds it (`owners`, the running and the candidate's).
    fn once_marker(
        &mut self,
        target: &mut EngineState<'n>,
        old: u32,
        new: u32,
        owners: Option<(InstanceId, InstanceId)>,
    ) -> Result<(), RuntimeError> {
        let (old_owner, new_owner) = match owners {
            Some((old_owner, new_owner)) => (Some(old_owner), Some(new_owner)),
            None => (None, None),
        };
        let construction = &self.source.data.construction;
        let marker = (new, new_owner);
        if construction.once.contains(&(old, old_owner))?
            && !target.data.construction.once.contains(&marker)?
        {
            let plan = target.data.construction.once.insertion_plan(marker)?;
            target.charge_sorted_growth(Ok(plan.demand()))?;
            target
                .data
                .construction
                .once
                .prepare_insert(plan)?
                .publish();
        }
        if construction
            .initialized_declarations
            .contains(&(old, old_owner))?
            && !target
                .data
                .construction
                .initialized_declarations
                .contains(&marker)?
        {
            let plan = target
                .data
                .construction
                .initialized_declarations
                .insertion_plan(marker)?;
            target.charge_sorted_growth(Ok(plan.demand()))?;
            target
                .data
                .construction
                .initialized_declarations
                .prepare_insert(plan)?
                .publish();
        }
        Ok(())
    }

    /// The running value of type `old_type` at `source` into the candidate's place `at` of type
    /// `new_type`.
    fn value(
        &mut self,
        target: &mut EngineState<'n>,
        (source, old_type): (ByteRange, u32),
        (at, new_type): (ByteRange, u32),
    ) -> Result<(), RuntimeError> {
        target.charge_work_units(1)?;
        let before = self.source.prepared;
        let after: &'n PreparedModule = target.prepared;
        let same = shape::same(before, old_type, after, new_type);
        if same && !holds_leaves(after, new_type) {
            return copy(self.source, source, target, at);
        }
        match (shape::data(before, old_type), shape::data(after, new_type)) {
            (Some(TypeData::Pou { .. }), Some(TypeData::Pou { .. })) => {
                let old = self
                    .source
                    .data
                    .reader(before)
                    .place_view(source, old_type)
                    .instance()?;
                let new = target
                    .data
                    .reader(after)
                    .place_view(at, new_type)
                    .instance()?;
                match (old, new) {
                    (Some(old), Some(new)) => self.instance(target, old, new),
                    _ => Ok(()),
                }
            }
            (Some(TypeData::Reference { .. }), Some(TypeData::Reference { .. }))
            | (Some(TypeData::Interface { .. }), Some(TypeData::Interface { .. })) => {
                // Bookkeeping of the change (TIME-08), reserved as it grows.
                self.deferred
                    .try_reserve(1)
                    .map_err(|_| RuntimeError::Overflow)?;
                let mut path = Vec::new();
                path.try_reserve_exact(self.path.len())
                    .map_err(|_| RuntimeError::Overflow)?;
                path.extend_from_slice(&self.path);
                self.deferred.push(Deferred {
                    source,
                    source_type: old_type,
                    target: at,
                    target_type: new_type,
                    path,
                });
                Ok(())
            }
            (
                Some(TypeData::Struct { fields: old_fields }),
                Some(TypeData::Struct { fields: new_fields }),
            ) if after.vm.physical_layout_ref(new_type).is_none()
                && before.vm.physical_layout_ref(old_type).is_none() =>
            {
                for (ordinal, field) in new_fields.iter().enumerate() {
                    let name = after
                        .vm
                        .strings
                        .get(field.name_idx as usize)
                        .ok_or(RuntimeError::InvalidExecutionState)?;
                    let old_field = old_fields.iter().enumerate().find(|(_, old_field)| {
                        before
                            .vm
                            .strings
                            .get(old_field.name_idx as usize)
                            .is_some_and(|old_name| old_name.eq_ignore_ascii_case(name))
                    });
                    let Some((old_ordinal, old_field)) = old_field else {
                        self.record_new(Segment::Name(name));
                        continue;
                    };
                    let field_source = field_range(before, old_type, source, old_ordinal)?;
                    let field_at = field_range(after, new_type, at, ordinal)?;
                    self.path.push(Segment::Name(name));
                    self.value(
                        target,
                        (field_source, old_field.type_id),
                        (field_at, field.type_id),
                    )?;
                    self.path.pop();
                }
                Ok(())
            }
            (
                Some(TypeData::Array {
                    elem_type_id: old_element,
                    dims: old_dims,
                }),
                Some(TypeData::Array {
                    elem_type_id: new_element,
                    dims: new_dims,
                }),
            ) if old_dims.len() == new_dims.len() && !new_dims.is_empty() => self.elements(
                target,
                (source, old_type, *old_element, old_dims),
                (at, new_type, *new_element, new_dims),
            ),
            _ if fit::is_elementary(before, old_type) && fit::is_elementary(after, new_type) => {
                let value = self
                    .source
                    .data
                    .reader(before)
                    .place_view(source, old_type)
                    .materialize()?;
                match fit::elementary(before, old_type, after, new_type, &value) {
                    Some(kept) => {
                        target
                            .data
                            .store
                            .write_in_place(at, new_type, &kept, after)?;
                        self.record(Kind::Converted, None);
                    }
                    None => {
                        self.record(Kind::NotKept, None);
                    }
                }
                Ok(())
            }
            _ => {
                self.record(Kind::NotKept, None);
                Ok(())
            }
        }
    }

    /// The elements of an array whose declaration changed: each element at an index both arrays
    /// have, and the others keep their initial values. A one-dimensional array of the same element
    /// type without instance or reference leaves is copied as one run of bytes.
    #[allow(clippy::type_complexity)]
    fn elements(
        &mut self,
        target: &mut EngineState<'n>,
        (source, old_type, old_element, old_dims): (ByteRange, u32, u32, &[(i64, i64)]),
        (at, new_type, new_element, new_dims): (ByteRange, u32, u32, &'n [(i64, i64)]),
    ) -> Result<(), RuntimeError> {
        let before = self.source.prepared;
        let after: &'n PreparedModule = target.prepared;
        let old_stride = stride(before, old_type)?;
        let new_stride = stride(after, new_type)?;
        if let ([(old_lower, old_upper)], [(new_lower, new_upper)]) = (old_dims, new_dims)
            && shape::same(before, old_element, after, new_element)
            && !holds_leaves(after, new_element)
        {
            let lower = (*old_lower).max(*new_lower);
            let upper = (*old_upper).min(*new_upper);
            if lower > upper {
                self.record(Kind::NotKept, None);
                return Ok(());
            }
            let count = element_count(lower, upper)?;
            let run =
                |range: ByteRange, from: i64, stride: u32| -> Result<ByteRange, RuntimeError> {
                    let skip = u32::try_from(lower - from).map_err(|_| RuntimeError::Overflow)?;
                    Ok(ByteRange {
                        start: range
                            .start
                            .checked_add(skip.checked_mul(stride).ok_or(RuntimeError::Overflow)?)
                            .ok_or(RuntimeError::Overflow)?,
                        bytes: count.checked_mul(stride).ok_or(RuntimeError::Overflow)?,
                        ..range
                    })
                };
            copy(
                self.source,
                run(source, *old_lower, old_stride)?,
                target,
                run(at, *new_lower, new_stride)?,
            )?;
            if (lower, upper) != (*old_lower, *old_upper)
                || (lower, upper) != (*new_lower, *new_upper)
            {
                self.record(Kind::Converted, None);
            }
            return Ok(());
        }
        // Every element of the candidate's array, in flat order; an index outside the running
        // array's bounds keeps its initial value.
        let mut indices = Vec::new();
        indices
            .try_reserve_exact(new_dims.len())
            .map_err(|_| RuntimeError::Overflow)?;
        indices.extend(new_dims.iter().map(|(lower, _)| *lower));
        let mut dropped = false;
        // The elements of an array whose element type changed record no event each: the array
        // records one, below.
        let element_changed = !shape::same(before, old_element, after, new_element);
        let outer_lost = core::mem::take(&mut self.events.lost);
        if element_changed {
            self.events.muted += 1;
        }
        loop {
            let old_offset = crate::value::array_offset_i64(old_dims, &indices);
            match old_offset {
                Some(old_offset) => {
                    let new_offset = crate::value::array_offset_i64(new_dims, &indices)
                        .ok_or(RuntimeError::InvalidExecutionState)?;
                    let element_source =
                        element_range(before, old_element, source, old_offset, old_stride)?;
                    let element_at = element_range(after, new_element, at, new_offset, new_stride)?;
                    if let [index] = indices[..] {
                        self.path.push(Segment::Index(index));
                    }
                    self.value(
                        target,
                        (element_source, old_element),
                        (element_at, new_element),
                    )?;
                    if indices.len() == 1 {
                        self.path.pop();
                    }
                }
                None => dropped = true,
            }
            if !next_index(&mut indices, new_dims) {
                break;
            }
        }
        if element_changed {
            self.events.muted -= 1;
        }
        let lost = core::mem::replace(&mut self.events.lost, outer_lost);
        if element_changed && lost {
            self.record(Kind::NotKept, None);
        } else if element_changed || dropped || element_total(old_dims) != element_total(new_dims) {
            self.record(Kind::Converted, None);
        }
        Ok(())
    }

    /// The deferred reference and interface leaves: each names the same variable or instance in the
    /// candidate, or is NULL.
    fn references(&mut self, target: &mut EngineState<'n>) -> Result<(), RuntimeError> {
        let deferred = core::mem::take(&mut self.deferred);
        let before = self.source.prepared;
        let after: &'n PreparedModule = target.prepared;
        for leaf in deferred {
            target.charge_work_units(1)?;
            let view = self
                .source
                .data
                .reader(before)
                .place_view(leaf.source, leaf.source_type);
            let carried = match shape::data(after, leaf.target_type) {
                Some(TypeData::Reference { target_type_id }) => match view.cell_reference()? {
                    None => continue,
                    Some(reference) => {
                        let reference = reference.to_owned()?;
                        self.reference(target, reference, *target_type_id)?
                            .map(|reference| Value::Reference(Some(reference)))
                    }
                },
                Some(TypeData::Interface { .. }) => match view.instance()? {
                    None => continue,
                    Some(old) => self
                        .instances
                        .get(&old.0)
                        .map(|new| Value::Instance(InstanceId(*new))),
                },
                _ => return Err(RuntimeError::InvalidExecutionState),
            };
            match carried {
                Some(value) => {
                    target.data.store.write_in_place(
                        leaf.target,
                        leaf.target_type,
                        &value,
                        after,
                    )?;
                }
                None => {
                    self.path = leaf.path;
                    self.record(Kind::Cleared, None);
                }
            }
        }
        self.path.clear();
        Ok(())
    }

    /// The candidate's reference to the variable that the running program's `reference` names, when
    /// the candidate has it with a type of the same shape as `referenced`, the type the candidate's
    /// reference names. A reference to a frame, or of another generation, names nothing to carry.
    fn reference(
        &self,
        target: &EngineState<'n>,
        reference: ValueRef,
        referenced: u32,
    ) -> Result<Option<ValueRef>, RuntimeError> {
        let source = self.source;
        if reference.generation != StoreAccess::generation(source) {
            return Ok(None);
        }
        let reference = source.path_form(&reference);
        let reader = source.data.reader(source.prepared);
        let mapped = match reference.location {
            MemoryLocation::Global => reader
                .global_name(reference.offset as usize)
                .and_then(|name| target.ref_for_global(name)),
            MemoryLocation::Instance(old) => {
                let Some(new) = self.instances.get(&old.0) else {
                    return Ok(None);
                };
                reader
                    .instance_member_name(old, reference.offset as usize)
                    .and_then(|name| target.ref_for_instance(InstanceId(*new), name))
            }
            _ => None,
        };
        let Some(mut mapped) = mapped else {
            return Ok(None);
        };
        mapped.path = reference.path;
        let after = target.prepared;
        let named = target.data.reader(after).read_path(
            mapped.location,
            mapped.offset as usize,
            &mapped.path,
        );
        let Ok(crate::memory::view::ValueView::Cells { type_id, .. }) = named else {
            return Ok(None);
        };
        if !shape::same(after, type_id, after, referenced) {
            return Ok(None);
        }
        Ok(Some(target.flatten_reference(mapped)))
    }
}
