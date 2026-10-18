//! The cell arm of the store-access seam (specification 34 §6.2, MEM-10; C1-9b).
//!
//! The arm reads the globals and the persistent instances from their cells, through the prepared
//! layouts. A [`crate::memory::view::ValueView::Cells`] names the cells of one value and reads them
//! without copying.
//! Locals, per-call instances and the rest of the `Value` half stay on the `values` field of the
//! arm, which is today's [`crate::memory::VariableStorage`], so their reads are unchanged. Until C1-9d the engine builds only
//! the `Value` store, so no answer of a running engine changes.
//!
//! The reads take the prepared module through `CellReader`: a cell view needs the layouts that
//! place its members and elements, and the store-access trait carries no prepared module. The
//! routing in `vm::engine::store_access` supplies it. The write side is C1-9c.

use alloc::collections::BTreeMap;

use smol_str::SmolStr;

use crate::bytecode::{StorageOwner, TypeData, BYTECODE_MAX_TYPE_NESTING};
use crate::collections::OrderedMap;
use crate::error::RuntimeError;
use crate::memory::cells::{read_value, Cell};
use crate::memory::layout::CellRange;
use crate::memory::regions::{CellRegions, CellStorage};
use crate::memory::view::ValueView;
use crate::memory::{Generation, InstanceData, InstanceId, MemoryLocation, VariableStorage};
use crate::value::{array_offset_i64, RefPath, RefSegment, Value, ValueRef, ValueRefView};
use crate::vm::{PreparedModule, VmTrap};

/// The primitive encoding of `STRING` (`prim_id` 24; the encodings of
/// `vm::type_policy::primitive_value_matches`).
const STRING_PRIMITIVE: u16 = 24;

/// The layouts and the region a cell view reads through: the prepared module places its members
/// and elements, and the region holds its cells and text buffers.
#[derive(Clone, Copy)]
pub struct CellsView<'a> {
    prepared: &'a PreparedModule,
    regions: &'a CellRegions,
}

impl<'a> CellsView<'a> {
    /// The member or element `segment` names inside the value of `type_id` at `range`, as its range
    /// and type. The member's cells must lie inside `range` (specification 34 §6.2, MEM-10).
    pub(crate) fn member(
        self,
        range: CellRange,
        type_id: u32,
        segment: &RefSegment,
    ) -> Option<(CellRange, u32)> {
        let (offset, child) = step(self.prepared, type_id, segment)?;
        let cells = self.prepared.type_layout(child)?.cells;
        if offset.checked_add(cells)? > range.cells {
            return None;
        }
        let start = range.start.checked_add(offset)?;
        Some((
            CellRange {
                region: range.region,
                start,
                cells,
            },
            child,
        ))
    }

    /// The UTF-8 text of the `STRING` at `range`, borrowed from its buffer. `None` for a `WSTRING`,
    /// whose buffer holds UTF-16 code units and has no UTF-8 text (specification 34 §3.1 of the C1-9
    /// note; C1-7).
    pub(crate) fn string_text(self, range: CellRange, type_id: u32) -> Option<&'a str> {
        if primitive_of(self.prepared, type_id)? != STRING_PRIMITIVE {
            return None;
        }
        let Cell::Text(slot) = self.regions.persistent().cells(range)?.first()? else {
            return None;
        };
        core::str::from_utf8(self.regions.text().bytes(*slot)?).ok()
    }

    /// The owned value of the cells at `range`, read with `read_value` (the only allocating read).
    pub(crate) fn materialize(self, range: CellRange, type_id: u32) -> Result<Value, RuntimeError> {
        let cells = self
            .regions
            .persistent()
            .cells(range)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        read_value(cells, type_id, self.prepared, self.regions.text())
    }
}

/// A persistent instance held in cells: its template POU, the range of its own members, its parent
/// and its declared type name (specification 34 §6.2; O6, O8).
#[allow(
    dead_code,
    reason = "C1-9d records instances at construction; until then only the tests record them"
)]
#[derive(Debug)]
struct PersistentInstance {
    pou: u32,
    range: CellRange,
    parent: Option<InstanceId>,
    type_name: SmolStr,
}

/// The cell arm's state (specification 34 §6.2; C1-9b): the cell store, the `Value` half that holds
/// locals and per-call instances, and the persistent instances by identity.
#[allow(
    dead_code,
    reason = "C1-9d builds the cell arm at construction; until then only the tests build it"
)]
#[derive(Debug, Default)]
pub(crate) struct CellArm {
    cells: CellStorage,
    values: VariableStorage,
    instances: BTreeMap<InstanceId, PersistentInstance>,
}

impl CellArm {
    /// Record that `instance`, of template `pou` and type `type_name`, occupies the cells of `range`
    /// with `parent` as its parent. C1-9d records the instances as it constructs them.
    #[allow(
        dead_code,
        reason = "C1-9d records instances at construction; until then only the tests record them"
    )]
    pub(crate) fn register_instance(
        &mut self,
        instance: InstanceId,
        pou: u32,
        range: CellRange,
        parent: Option<InstanceId>,
        type_name: &str,
    ) {
        self.instances.insert(
            instance,
            PersistentInstance {
                pou,
                range,
                parent,
                type_name: SmolStr::new(type_name),
            },
        );
    }
}

/// Where a path ends: in cells of the arm, or in a place of the `Value` half.
enum Target<'p> {
    /// The cells of a value of this type at this range.
    Cells(CellRange, u32),
    /// A place the `Value` half reads: location, slot and the path still to walk.
    Values(MemoryLocation, usize, &'p [RefSegment]),
}

/// The reads of the cell arm (specification 34 §6.2, MEM-10; C1-9b): the reads of
/// [`crate::memory::view::StoreAccess`] over the cells of an arm, with the prepared module that places
/// them. Each read takes the reader by value, so a view it returns borrows the arm and not the reader.
#[derive(Clone, Copy)]
pub(crate) struct CellReader<'a> {
    prepared: &'a PreparedModule,
    arm: &'a CellArm,
}

impl<'a> CellReader<'a> {
    /// A reader of `arm`, placing its cells through `prepared`.
    pub(crate) fn new(prepared: &'a PreparedModule, arm: &'a CellArm) -> Self {
        Self { prepared, arm }
    }

    /// The installation generation of the arm's references (specification 34 §11.3, MEM-07).
    pub(crate) fn generation(self) -> Generation {
        self.arm.cells.generation()
    }

    /// Read the view of the path `path` inside the slot at `location`/`offset`.
    ///
    /// # Errors
    ///
    /// [`VmTrap::NullReference`] when the slot or a step of the path names no value.
    pub(crate) fn read_path(
        self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
    ) -> Result<ValueView<'a>, VmTrap> {
        match self.locate(location, offset, path)? {
            Target::Cells(range, type_id) => Ok(self.view(range, type_id)),
            Target::Values(location, offset, rest) => self.values_slot(location, offset, rest),
        }
    }

    /// The view of field `name` of `instance`, searched through its parents: the field the instance
    /// declares, else the nearest ancestor's.
    ///
    /// # Errors
    ///
    /// [`VmTrap::NullReference`] when no instance on the chain declares `name`.
    pub(crate) fn read_instance_field(
        self,
        instance: InstanceId,
        name: &str,
    ) -> Result<ValueView<'a>, VmTrap> {
        match self.field(instance, name, &[])? {
            Target::Cells(range, type_id) => Ok(self.view(range, type_id)),
            Target::Values(location, offset, rest) => self.values_slot(location, offset, rest),
        }
    }

    /// The view of a whole slot, without a path.
    ///
    /// # Errors
    ///
    /// [`VmTrap::NullReference`] when the slot names no value.
    pub(crate) fn read_slot(
        self,
        location: MemoryLocation,
        offset: usize,
    ) -> Result<ValueView<'a>, VmTrap> {
        self.read_path(location, offset, &[])
    }

    /// The view of a reference's slot and path, in this arm's generation. A reference of another
    /// generation names no state here (specification 34 §11.3, MEM-07).
    ///
    /// # Errors
    ///
    /// [`VmTrap::NullReference`] for a foreign generation, or when its target names no value.
    pub(crate) fn read_reference(
        self,
        reference: ValueRefView<'_>,
    ) -> Result<ValueView<'a>, VmTrap> {
        if reference.generation != self.generation() {
            return Err(VmTrap::NullReference);
        }
        self.read_path(reference.location, reference.offset, reference.path)
    }

    /// The view of one variable of the persistent instance `instance`, or of a `Value`-half
    /// instance, selected by its declared name.
    pub(crate) fn read_instance_var(
        self,
        instance: InstanceId,
        name: &str,
    ) -> Option<ValueView<'a>> {
        if self.arm.instances.contains_key(&instance) {
            let (_, _, range, type_id) = self.member_by(instance, |_, member| member == name)?;
            return Some(self.view(range, type_id));
        }
        self.arm
            .values
            .get_instance_var(instance, name)
            .map(ValueView::Value)
    }

    /// The view of the global named `name`.
    pub(crate) fn read_global(self, name: &str) -> Option<ValueView<'a>> {
        let ordinal = self.global_index(name)?;
        let (range, type_id) = self.global(ordinal)?;
        Some(self.view(range, type_id))
    }

    /// The declared name of the global at `offset`, in declaration order.
    pub(crate) fn global_name(self, offset: usize) -> Option<&'a SmolStr> {
        let declaration = *self
            .prepared
            .declarations(StorageOwner::Global, None)
            .get(offset)?;
        let name_idx = self
            .prepared
            .layout
            .entries
            .get(declaration as usize)?
            .name_idx;
        self.prepared.strings().get(name_idx as usize)
    }

    /// The reference to the global named `name`, whose ordinal is its declaration order.
    pub(crate) fn ref_for_global(self, name: &str) -> Option<ValueRef> {
        let ordinal = self.global_index(name)?;
        self.value_ref(MemoryLocation::Global, ordinal)
    }

    /// The reference to the variable `name` of `instance`, in its own variables.
    pub(crate) fn ref_for_instance(self, instance: InstanceId, name: &str) -> Option<ValueRef> {
        if self.arm.instances.contains_key(&instance) {
            let (ordinal, _, _, _) = self.member_by(instance, |_, member| member == name)?;
            return self.value_ref(MemoryLocation::Instance(instance), ordinal);
        }
        self.arm.values.ref_for_instance(instance, name)
    }

    /// The reference to the variable `name` of `instance` or of its nearest ancestor that declares it.
    pub(crate) fn ref_for_instance_recursive(
        self,
        instance: InstanceId,
        name: &str,
    ) -> Option<ValueRef> {
        let mut current = instance;
        for _ in 0..=self.arm.instances.len() {
            if !self.arm.instances.contains_key(&current) {
                return self.arm.values.ref_for_instance_recursive(current, name);
            }
            if let Some((ordinal, _, _, _)) = self.member_by(current, |_, member| member == name) {
                return self.value_ref(MemoryLocation::Instance(current), ordinal);
            }
            current = self.arm.instances.get(&current)?.parent?;
        }
        None
    }

    /// The reference to field `name` of `instance`: the field it declares, else an inherited one.
    pub(crate) fn resolved_instance_field_ref(
        self,
        instance: InstanceId,
        name: &str,
    ) -> Option<ValueRef> {
        self.ref_for_instance(instance, name)
            .or_else(|| self.ref_for_instance_recursive(instance, name))
    }

    /// The position of `name` among the variables `instance` declares itself.
    pub(crate) fn declared_instance_field_offset(
        self,
        instance: InstanceId,
        name: &str,
    ) -> Option<usize> {
        if self.arm.instances.contains_key(&instance) {
            return self
                .member_by(instance, |_, member| member == name)
                .map(|(ordinal, _, _, _)| ordinal);
        }
        self.arm
            .values
            .declared_instance_field_offset(instance, name)
    }

    /// An owned copy of the instance record: its members materialized from their cells.
    pub(crate) fn snapshot_instance(self, instance: InstanceId) -> Option<InstanceData> {
        let Some(persistent) = self.arm.instances.get(&instance) else {
            return self.arm.values.get_instance(instance).cloned();
        };
        let mut variables = OrderedMap::default();
        let mut ordinal = 0usize;
        while let Some((_, name, range, type_id)) =
            self.member_by(instance, |index, _| index == ordinal)
        {
            let value = self.view(range, type_id).materialize().ok()?;
            variables.insert(SmolStr::new(name), value);
            ordinal += 1;
        }
        Some(InstanceData {
            type_name: persistent.type_name.clone(),
            variables,
            parent: persistent.parent,
        })
    }

    /// The declared type name of `instance`, or `None` when no such instance exists.
    pub(crate) fn instance_type_name(self, instance: InstanceId) -> Option<&'a SmolStr> {
        match self.arm.instances.get(&instance) {
            Some(persistent) => Some(&persistent.type_name),
            None => self
                .arm
                .values
                .get_instance(instance)
                .map(|data| &data.type_name),
        }
    }

    /// The parent of `instance`: `Ok(None)` when it has none, `NullReference` when no such instance
    /// exists.
    pub(crate) fn instance_parent(
        self,
        instance: InstanceId,
    ) -> Result<Option<InstanceId>, VmTrap> {
        match self.arm.instances.get(&instance) {
            Some(persistent) => Ok(persistent.parent),
            None => self
                .arm
                .values
                .get_instance(instance)
                .map(|data| data.parent)
                .ok_or(VmTrap::NullReference),
        }
    }

    /// The view of the cells of `range`, a value of `type_id`, through this reader's region.
    fn view(self, range: CellRange, type_id: u32) -> ValueView<'a> {
        ValueView::Cells {
            cells: CellsView {
                prepared: self.prepared,
                regions: self.arm.cells.regions(),
            },
            range,
            type_id,
        }
    }

    /// The `Value` half's view of a slot, or `NullReference` when the slot holds no value.
    fn values_slot(
        self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
    ) -> Result<ValueView<'a>, VmTrap> {
        self.arm
            .values
            .read_by_ref_parts(location, offset, path)
            .map(ValueView::Value)
            .ok_or(VmTrap::NullReference)
    }

    /// The cells and type of the global at `ordinal` in declaration order, or `None` when that
    /// declaration owns no cells.
    fn global(self, ordinal: usize) -> Option<(CellRange, u32)> {
        let declaration = *self
            .prepared
            .declarations(StorageOwner::Global, None)
            .get(ordinal)?;
        let range = self
            .arm
            .cells
            .regions()
            .global_range(declaration as usize)?;
        let type_id = self
            .prepared
            .layout
            .entries
            .get(declaration as usize)?
            .type_id?;
        Some((range, type_id))
    }

    /// The declaration-order ordinal of the global named `name`.
    fn global_index(self, name: &str) -> Option<usize> {
        self.prepared
            .declarations(StorageOwner::Global, None)
            .iter()
            .position(|&declaration| {
                self.prepared
                    .layout
                    .entries
                    .get(declaration as usize)
                    .and_then(|entry| member_name(self.prepared, entry.name_idx))
                    == Some(name)
            })
    }

    /// The reference of this arm's generation to the slot `ordinal` of `location`.
    fn value_ref(self, location: MemoryLocation, ordinal: usize) -> Option<ValueRef> {
        Some(ValueRef {
            location,
            offset: u32::try_from(ordinal).ok()?,
            generation: self.generation(),
            path: RefPath::new(),
        })
    }

    /// The member of persistent `instance` that `select` accepts, as its ordinal among the members
    /// that own storage, its name, its range and its type. Members lie in declaration order, each
    /// after the cells of the members before it (specification 34 §6.2, O6).
    fn member_by(
        self,
        instance: InstanceId,
        select: impl Fn(usize, &str) -> bool,
    ) -> Option<(usize, &'a str, CellRange, u32)> {
        let persistent = self.arm.instances.get(&instance)?;
        let mut offset = 0u32;
        let mut ordinal = 0usize;
        for declaration in self.prepared.instance_declarations(persistent.pou) {
            let Some(type_id) = declaration.type_id.filter(|_| declaration.owns_storage()) else {
                continue;
            };
            let name = member_name(self.prepared, declaration.name_idx)?;
            let cells = self.prepared.type_layout(type_id)?.cells;
            if select(ordinal, name) {
                let start = persistent.range.start.checked_add(offset)?;
                let range = CellRange {
                    region: persistent.range.region,
                    start,
                    cells,
                };
                return Some((ordinal, name, range, type_id));
            }
            offset = offset.checked_add(cells)?;
            ordinal += 1;
        }
        None
    }

    /// The instance a cell holds: an instance cell names its instance.
    fn instance_at(self, range: CellRange) -> Result<InstanceId, VmTrap> {
        let cell = self
            .arm
            .cells
            .regions()
            .persistent()
            .cells(range)
            .and_then(<[Cell]>::first);
        if let Some(Cell::Instance(instance)) = cell {
            Ok(*instance)
        } else {
            Err(VmTrap::NullReference)
        }
    }

    /// Where the slot `offset` of `location` and the path lead: the cells of a global or of a
    /// persistent instance's member, or a place of the `Value` half.
    fn locate<'p>(
        self,
        location: MemoryLocation,
        offset: usize,
        path: &'p [RefSegment],
    ) -> Result<Target<'p>, VmTrap> {
        match location {
            MemoryLocation::Global => match self.global(offset) {
                Some((range, type_id)) => self.walk(range, type_id, path),
                None => Ok(Target::Values(location, offset, path)),
            },
            MemoryLocation::Instance(instance) if self.arm.instances.contains_key(&instance) => {
                let (_, _, range, type_id) = self
                    .member_by(instance, |ordinal, _| ordinal == offset)
                    .ok_or(VmTrap::NullReference)?;
                self.walk(range, type_id, path)
            }
            _ => Ok(Target::Values(location, offset, path)),
        }
    }

    /// Walk `path` from the value of `type_id` at `range`. A field of a function block or an
    /// interface value is read through the instance its cell holds, as the `Value` tree does.
    fn walk<'p>(
        self,
        mut range: CellRange,
        mut type_id: u32,
        path: &'p [RefSegment],
    ) -> Result<Target<'p>, VmTrap> {
        for (index, segment) in path.iter().enumerate() {
            if is_instance(self.prepared, type_id) {
                let RefSegment::Field(name) = segment else {
                    return Err(VmTrap::NullReference);
                };
                let instance = self.instance_at(range)?;
                return self.field(instance, name.as_str(), &path[index + 1..]);
            }
            let (offset, child) =
                step(self.prepared, type_id, segment).ok_or(VmTrap::NullReference)?;
            let cells = self
                .prepared
                .type_layout(child)
                .ok_or(VmTrap::NullReference)?
                .cells;
            let start = range
                .start
                .checked_add(offset)
                .ok_or(VmTrap::NullReference)?;
            range = CellRange {
                region: range.region,
                start,
                cells,
            };
            type_id = child;
        }
        Ok(Target::Cells(range, type_id))
    }

    /// The field `name` of `instance`, searched through its parents, then the rest of `rest`
    /// walked from it. An instance of the `Value` half is searched there.
    fn field<'p>(
        self,
        instance: InstanceId,
        name: &str,
        rest: &'p [RefSegment],
    ) -> Result<Target<'p>, VmTrap> {
        let mut current = instance;
        for _ in 0..=self.arm.instances.len() {
            let Some(persistent) = self.arm.instances.get(&current) else {
                let reference = self
                    .arm
                    .values
                    .ref_for_instance_recursive(current, name)
                    .ok_or(VmTrap::NullReference)?;
                return Ok(Target::Values(
                    reference.location,
                    reference.offset as usize,
                    rest,
                ));
            };
            if let Some((_, _, range, type_id)) =
                self.member_by(current, |_, member| member == name)
            {
                return self.walk(range, type_id, rest);
            }
            current = persistent.parent.ok_or(VmTrap::NullReference)?;
        }
        Err(VmTrap::NullReference)
    }
}

/// The member name of a `STRING_TABLE` record.
fn member_name(prepared: &PreparedModule, name_idx: u32) -> Option<&str> {
    prepared
        .strings()
        .get(name_idx as usize)
        .map(SmolStr::as_str)
}

/// The `TYPE_TABLE` ordinal of `type_id` with its aliases resolved, and the type's data.
fn resolve_alias(prepared: &PreparedModule, type_id: u32) -> Option<(u32, &TypeData)> {
    let mut current = type_id;
    for _ in 0..=BYTECODE_MAX_TYPE_NESTING {
        let entry = prepared.type_table().entries.get(current as usize)?;
        match &entry.data {
            TypeData::Alias { target_type_id } => current = *target_type_id,
            data => return Some((current, data)),
        }
    }
    None
}

/// The primitive encoding `type_id` resolves to through its aliases and subranges.
fn primitive_of(prepared: &PreparedModule, type_id: u32) -> Option<u16> {
    let mut current = type_id;
    for _ in 0..=BYTECODE_MAX_TYPE_NESTING {
        let entry = prepared.type_table().entries.get(current as usize)?;
        match &entry.data {
            TypeData::Primitive { prim_id, .. } => return Some(*prim_id),
            TypeData::Alias { target_type_id }
            | TypeData::Subrange {
                base_type_id: target_type_id,
                ..
            } => current = *target_type_id,
            _ => return None,
        }
    }
    None
}

/// Whether `type_id` is a function block or an interface, whose cell holds an instance.
fn is_instance(prepared: &PreparedModule, type_id: u32) -> bool {
    resolve_alias(prepared, type_id)
        .is_some_and(|(_, data)| matches!(data, TypeData::Pou { .. } | TypeData::Interface { .. }))
}

/// The offset within a value of `type_id` of the member or element `segment` names, and the type of
/// that member or element. A member is matched by its exact name, as the `Value` tree matches it;
/// an element is found by its IEC index against the declared bounds.
fn step(prepared: &PreparedModule, type_id: u32, segment: &RefSegment) -> Option<(u32, u32)> {
    let (resolved, data) = resolve_alias(prepared, type_id)?;
    match (segment, data) {
        (RefSegment::Field(name), TypeData::Struct { fields }) => {
            let layout = prepared.type_layout(resolved)?;
            let ordinal = layout
                .members
                .iter()
                .position(|member| member_name(prepared, member.name_idx) == Some(name.as_str()))?;
            Some((
                layout.members.get(ordinal)?.offset,
                fields.get(ordinal)?.type_id,
            ))
        }
        (RefSegment::Index(indices), TypeData::Array { elem_type_id, dims }) => {
            let element = array_offset_i64(dims, indices)?;
            let stride = prepared.type_layout(resolved)?.array.as_ref()?.stride;
            Some((
                u32::try_from(element).ok()?.checked_mul(stride)?,
                *elem_type_id,
            ))
        }
        _ => None,
    }
}

#[cfg(all(test, feature = "std"))]
mod fixture;
#[cfg(all(test, feature = "std"))]
mod tests;
