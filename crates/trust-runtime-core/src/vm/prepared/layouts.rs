//! Prepared layouts and the byte ranges of the flat blocks (specification 34 §6.2, MEM-12).
//!
//! Preparation computes one [`TypeLayout`](crate::memory::layout::TypeLayout) per TYPE_TABLE
//! entry and maps globals, instance templates, POU locals and initializer results to byte ranges,
//! each variable at its declared size and alignment (design "Prepared layouts"). It charges the
//! bytes of those layout tables and the work of building them; a region's bytes are recorded here
//! and charged when construction reserves the region (specification 34 §6.2).

use super::*;
use crate::bytecode::{
    InitializationStage, InitializerBodyKind, StorageOwner, StorageRole, TypeData,
};
use crate::collections::SortedMap;
use crate::memory::layout::{
    align_up, compute_type_layouts, ByteRange, ByteShape, InstanceTemplateLayout, LayoutRegion,
    TypeLayout, INSTANCE_ALIGN,
};
use crate::vm::type_policy::is_wildcard_dim;

/// The layouts of one prepared module: one per TYPE_TABLE entry, plus the byte ranges of its
/// globals, instance templates, POU locals and initializer results, and each declaration's byte
/// offset within the block or the template that holds it.
#[derive(Debug, Default)]
pub(crate) struct PreparedLayouts {
    types: Vec<TypeLayout>,
    globals: Vec<Option<ByteRange>>,
    templates: SortedMap<u32, InstanceTemplateLayout>,
    /// The reference leaves of each POU's own instance variables, by their offset from the
    /// template's first byte, and of each POU's frame, by their offset from the frame's first byte,
    /// ascending, for the activation stack's reference table (C1-19c4). A POU without any has no
    /// entry.
    instance_reference_leaves: LeafLists,
    frame_reference_leaves: LeafLists,
    locals: SortedMap<u32, ByteRange>,
    initializer_results: Vec<Option<ByteRange>>,
    /// Each storage declaration's byte offset: a global's in the persistent block, an instance
    /// variable's in its POU's template and a frame variable's in its POU's frame.
    offsets: Vec<u32>,
    /// What these layout tables and their computation were charged (specification 34 §6.2).
    charged: PreparationUsage,
}

/// The type layouts, for the table probe of a prepared program (MEM-14).
#[cfg(test)]
impl PreparedLayouts {
    pub(super) fn type_layouts(&self) -> &[crate::memory::layout::TypeLayout] {
        &self.types
    }
}

/// Each POU's reference leaves, ascending, as a range of one list that every POU shares
/// (specification 34 MEM-14: lists are ranges into shared arrays, not one vector per entry).
#[derive(Debug, Default)]
struct LeafLists {
    ranges: SortedMap<u32, (u32, u32)>,
    leaves: Vec<u32>,
}

impl LeafLists {
    /// The lists of `pairs`, each a POU and one of its leaves, charged through the budget.
    fn of(
        mut pairs: Vec<(u32, u32)>,
        budget: &mut PreparationBudget,
    ) -> Result<Self, RuntimeError> {
        pairs.sort_unstable();
        let pous = pairs
            .iter()
            .enumerate()
            .filter(|&(at, &(pou, _))| at == 0 || pairs[at - 1].0 != pou)
            .count();
        budget.records::<(u32, (u32, u32))>(pous)?;
        budget.records::<u32>(pairs.len())?;
        let mut ranges =
            SortedMap::try_with_capacity(pous).map_err(|_| RuntimeError::PreparationLimit)?;
        let mut leaves = Vec::new();
        leaves
            .try_reserve_exact(pairs.len())
            .map_err(|_| RuntimeError::PreparationLimit)?;
        for (at, &(pou, leaf)) in pairs.iter().enumerate() {
            if at == 0 || pairs[at - 1].0 != pou {
                let start = u32::try_from(at).map_err(|_| RuntimeError::Overflow)?;
                ranges.insert(pou, (start, start));
            }
            leaves.push(leaf);
            if let Some(range) = ranges.get_mut(&pou) {
                range.1 = u32::try_from(at + 1).map_err(|_| RuntimeError::Overflow)?;
            }
        }
        Ok(Self { ranges, leaves })
    }

    /// The leaves of `pou`, ascending.
    fn leaves(&self, pou: u32) -> &[u32] {
        self.ranges.get(&pou).map_or(&[], |&(start, end)| {
            &self.leaves[start as usize..end as usize]
        })
    }
}

impl PreparedLayouts {
    /// Compute every layout once, charged through the preparation budget.
    ///
    /// An instance variable lies in the template of the POU whose instances hold it: a method's
    /// statics in the template of the POU that owns the method (`methods`), as the declaration
    /// index groups them, so each receiver keeps its own (specification 12 §11.5: an instance
    /// template includes the hidden method statics, whose ownership follows the method's owner).
    pub(crate) fn build(
        vm: &VmModule,
        layout: &StorageLayout,
        initializers: &InitializerIndex,
        methods: &crate::collections::SortedMap<u32, u32>,
        budget: &mut PreparationBudget,
    ) -> Result<Self, RuntimeError> {
        let charged_before = budget.usage();
        let types = compute_type_layouts(
            &vm.types,
            &vm.strings,
            &mut |id: u32| -> Result<Option<u32>, RuntimeError> {
                vm.physical_layout(id)
                    .map(|physical| {
                        u32::try_from(physical.byte_len()?).map_err(|_| RuntimeError::Overflow)
                    })
                    .transpose()
            },
            &mut |bytes, work| budget.charge(bytes, work),
        )?;
        let mut offsets = Vec::new();
        offsets
            .try_reserve_exact(layout.entries.len())
            .map_err(|_| RuntimeError::PreparationLimit)?;
        offsets.resize(layout.entries.len(), 0u32);
        budget.records::<u32>(offsets.len())?;

        // The globals, one after the other in the persistent block, each at its alignment.
        let mut persistent_cursor = 0u32;
        let mut globals: Vec<Option<ByteRange>> = Vec::new();
        globals.resize(layout.entries.len(), None);
        for (index, declaration) in layout.entries.iter().enumerate() {
            if declaration.owner != StorageOwner::Global || !declaration.owns_storage() {
                continue;
            }
            let Some(type_id) = declaration.type_id else {
                continue;
            };
            let shape = shape_of(&types, type_id)?;
            let start = align_up(persistent_cursor, shape.align)?;
            globals[index] = Some(ByteRange {
                region: LayoutRegion::Persistent,
                start,
                bytes: shape.bytes,
            });
            offsets[index] = start;
            persistent_cursor = slot_end(start, shape, declaration.role)?;
        }
        budget.records::<Option<ByteRange>>(globals.len())?;

        // Each POU's own instance variables, at their alignment from the template's first byte.
        let mut own: BTreeMap<u32, u32> = BTreeMap::new();
        for (index, declaration) in layout.entries.iter().enumerate() {
            if declaration.owner != StorageOwner::Instance || !declaration.owns_storage() {
                continue;
            }
            let (Some(pou), Some(type_id)) = (declaration.owner_pou_id, declaration.type_id) else {
                continue;
            };
            let pou = methods.get(&pou).copied().unwrap_or(pou);
            let shape = shape_of(&types, type_id)?;
            let cursor = own.entry(pou).or_insert(0);
            let start = align_up(*cursor, shape.align)?;
            offsets[index] = start;
            *cursor = slot_end(start, shape, declaration.role)?;
        }
        let mut templates = SortedMap::new();
        for &pou in own.keys().chain(vm.parent_pou_ids.keys()) {
            templates.insert(
                pou,
                InstanceTemplateLayout {
                    range: ByteRange {
                        region: LayoutRegion::Persistent,
                        start: 0,
                        bytes: align_up(own.get(&pou).copied().unwrap_or(0), INSTANCE_ALIGN)?,
                    },
                    parent: vm.parent_pou_ids.get(&pou).copied(),
                },
            );
        }
        templates.shrink_to_fit();
        budget.records::<(u32, InstanceTemplateLayout)>(templates.len())?;

        // Each POU's frame variables, then the initializer results, on the activation stack.
        let mut activation_cursor = 0u32;
        let mut locals = SortedMap::new();
        for declaration in &layout.entries {
            if declaration.owner != StorageOwner::Frame || !declaration.owns_storage() {
                continue;
            }
            let Some(pou) = declaration.owner_pou_id else {
                continue;
            };
            if locals.contains_key(&pou) {
                continue;
            }
            let bytes = frame_bytes(&types, layout, pou, &mut offsets)?;
            let start = align_up(activation_cursor, INSTANCE_ALIGN)?;
            locals.insert(
                pou,
                ByteRange {
                    region: LayoutRegion::Activation,
                    start,
                    bytes,
                },
            );
            activation_cursor = start.checked_add(bytes).ok_or(RuntimeError::Overflow)?;
        }
        locals.shrink_to_fit();
        budget.records::<(u32, ByteRange)>(locals.len())?;

        // The reference leaves of each template's own variables and of each frame (C1-19c4).
        let mut instance_leaves = Vec::new();
        let mut frame_leaves = Vec::new();
        for (index, declaration) in layout.entries.iter().enumerate() {
            let owner = match declaration.owner {
                StorageOwner::Instance => &mut instance_leaves,
                StorageOwner::Frame => &mut frame_leaves,
                StorageOwner::Global => continue,
            };
            let (Some(pou), Some(type_id)) = (declaration.owner_pou_id, declaration.type_id) else {
                continue;
            };
            let pou = match declaration.owner {
                StorageOwner::Instance => methods.get(&pou).copied().unwrap_or(pou),
                _ => pou,
            };
            let Some(type_layout) = types.get(type_id as usize).filter(|type_layout| {
                declaration.owns_storage() && !type_layout.reference_leaves.is_empty()
            }) else {
                continue;
            };
            for leaf in &type_layout.reference_leaves {
                owner.push((
                    pou,
                    offsets[index]
                        .checked_add(leaf.byte_offset)
                        .ok_or(RuntimeError::Overflow)?,
                ));
            }
        }
        let instance_reference_leaves = LeafLists::of(instance_leaves, budget)?;
        let frame_reference_leaves = LeafLists::of(frame_leaves, budget)?;

        let mut initializer_results: Vec<Option<ByteRange>> = Vec::new();
        initializer_results.resize(initializers.entries.len(), None);
        for (id, entry) in initializers.entries.iter().enumerate() {
            let Some(type_id) = vm.ref_type(entry.result_ref_idx) else {
                continue;
            };
            let shape = shape_of(&types, type_id)?;
            let start = align_up(activation_cursor, shape.align)?;
            initializer_results[id] = Some(ByteRange {
                region: LayoutRegion::Activation,
                start,
                bytes: shape.bytes,
            });
            activation_cursor = end_of(start, shape)?;
        }
        budget.records::<Option<ByteRange>>(initializer_results.len())?;

        let charged_after = budget.usage();
        Ok(Self {
            types,
            globals,
            templates,
            instance_reference_leaves,
            frame_reference_leaves,
            locals,
            initializer_results,
            offsets,
            charged: PreparationUsage {
                bytes: charged_after.bytes - charged_before.bytes,
                work: charged_after.work - charged_before.work,
            },
        })
    }
}

/// The bytes of `pou`'s frame variables, each at its alignment, rounded up to [`INSTANCE_ALIGN`];
/// records each variable's offset in `offsets`.
fn frame_bytes(
    types: &[TypeLayout],
    layout: &StorageLayout,
    pou: u32,
    offsets: &mut [u32],
) -> Result<u32, RuntimeError> {
    let mut cursor = 0u32;
    for (index, declaration) in layout.entries.iter().enumerate() {
        if declaration.owner != StorageOwner::Frame
            || !declaration.owns_storage()
            || declaration.owner_pou_id != Some(pou)
        {
            continue;
        }
        let Some(type_id) = declaration.type_id else {
            continue;
        };
        let shape = shape_of(types, type_id)?;
        let start = align_up(cursor, shape.align)?;
        offsets[index] = start;
        cursor = end_of(start, shape)?;
    }
    align_up(cursor, INSTANCE_ALIGN)
}

/// The declared size and alignment of `type_id` from its prepared layout.
fn shape_of(types: &[TypeLayout], type_id: u32) -> Result<ByteShape, RuntimeError> {
    types
        .get(type_id as usize)
        .map(|layout| layout.shape)
        .ok_or(RuntimeError::TypeMismatch)
}

/// The first byte after a value of `shape` at `start`.
fn end_of(start: u32, shape: ByteShape) -> Result<u32, RuntimeError> {
    start.checked_add(shape.bytes).ok_or(RuntimeError::Overflow)
}

/// The first byte after the slot of a declaration of `role` whose value of `shape` is at `start`:
/// the value, and its presence byte when the role keeps one.
fn slot_end(start: u32, shape: ByteShape, role: StorageRole) -> Result<u32, RuntimeError> {
    end_of(start, shape)?
        .checked_add(u32::from(holds_presence(role)))
        .ok_or(RuntimeError::Overflow)
}

/// Whether a declaration of `role` holds no value until something gives it one: a function static
/// until its static action runs and a native-state slot until its native import first writes it.
/// Its slot keeps a presence byte after its value, zero until then, so it reads as the null value
/// as it does in the `Value` store (specification 34 MEM-12).
pub(crate) fn holds_presence(role: StorageRole) -> bool {
    matches!(role, StorageRole::Static | StorageRole::NativeState)
}

impl PreparedModule {
    /// The preparation bytes and work the layout tables and their computation were charged
    /// (specification 34 §6.2). A region's bytes are not part of it: construction charges them
    /// when it reserves the region.
    #[must_use]
    pub fn layout_preparation_usage(&self) -> PreparationUsage {
        self.layouts.charged
    }

    /// The prepared layout of one TYPE_TABLE entry (specification 34 §6.2).
    #[must_use]
    pub fn type_layout(&self, type_id: u32) -> Option<&TypeLayout> {
        self.layouts.types.get(type_id as usize)
    }

    /// Every prepared type layout, indexed by TYPE_TABLE ordinal (specification 34 §6.2).
    #[must_use]
    pub fn type_layouts(&self) -> &[TypeLayout] {
        &self.layouts.types
    }

    /// The persistent byte range a global declaration owns, absent for other owners.
    #[must_use]
    pub fn global_range(&self, declaration: u32) -> Option<ByteRange> {
        self.layouts
            .globals
            .get(declaration as usize)
            .copied()
            .flatten()
    }

    /// A POU's instance template: its own bytes from the instance's first byte, and its parent's
    /// template (specification 34 §6.2, O6, O7).
    #[must_use]
    pub fn instance_template(&self, pou: u32) -> Option<InstanceTemplateLayout> {
        self.layouts.templates.get(&pou).copied()
    }

    /// The byte offset of storage declaration `declaration`: a global's in the persistent block, an
    /// instance variable's in its POU's template and a frame variable's in its POU's frame
    /// (MEM-12).
    #[must_use]
    pub fn declaration_offset(&self, declaration: u32) -> Option<u32> {
        self.layouts.offsets.get(declaration as usize).copied()
    }

    /// The block offset of the presence byte of the storage declaration `declaration` whose value
    /// lies at `start` in its block, or `None` for a declaration that keeps none (MEM-12): a
    /// function static's and a native-state slot's, right after their value.
    #[must_use]
    pub fn presence_offset(&self, declaration: u32, start: u32) -> Option<u32> {
        // A scan asks this of every access to a slot that may keep presence, so it reads the
        // declaration's compact record, not the program's image (MEM-14).
        let entry = self.scan_declaration(declaration as usize)?;
        if !holds_presence(entry.role) {
            return None;
        }
        start.checked_add(self.type_layout(entry.type_id?)?.shape.bytes)
    }

    /// The POUs that have a frame on the activation stack, in ascending order.
    pub(crate) fn frame_pous(&self) -> impl Iterator<Item = u32> + '_ {
        self.layouts.locals.keys().copied()
    }

    /// The activation byte range of a POU's frame locals.
    #[must_use]
    pub fn local_range(&self, pou: u32) -> Option<ByteRange> {
        self.layouts.locals.get(&pou).copied()
    }

    /// The bytes the activation stack reserves on the flat store (specification 34 MEM-12; C1-11):
    /// one frame of the largest POU for each call level construction reserves. A frame takes its
    /// locals' bytes on top of its caller's, and no chain of calls is deeper than those levels, so
    /// no chain needs more.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::Overflow`] when the bytes exceed `u32`.
    pub(crate) fn activation_reservation(&self) -> Result<u32, RuntimeError> {
        let mut largest = 0u32;
        for (&pou, range) in &self.layouts.locals {
            let room = self.call_instance_room(pou).ok_or(RuntimeError::Overflow)?;
            largest = largest.max(
                range
                    .bytes
                    .checked_add(room.bytes)
                    .ok_or(RuntimeError::Overflow)?,
            );
        }
        // An initializer evaluated in place stages its result above the frame it initializes, and
        // a call its body makes lies above that result (C1-13).
        let largest = largest
            .checked_add(self.largest_in_place_result())
            .ok_or(RuntimeError::Overflow)?;
        u32::try_from(self.reserved_call_levels())
            .ok()
            .and_then(|levels| levels.checked_mul(largest))
            .ok_or(RuntimeError::Overflow)
    }

    /// The staged results the activation stack holds at most (C1-13): one for each reserved call
    /// level, when some initializer is evaluated in place, else none.
    pub(crate) fn result_block_reservation(&self) -> usize {
        if self.largest_in_place_result() == 0 {
            0
        } else {
            self.reserved_call_levels()
        }
    }

    /// The bytes of the largest staged result of an initializer evaluated in place, or 0.
    fn largest_in_place_result(&self) -> u32 {
        (0..self.initializer_count())
            .filter_map(|id| self.in_place_result(u32::try_from(id).ok()?))
            .map(|(_, bytes)| bytes)
            .max()
            .unwrap_or(0)
    }

    /// The type and the bytes of the staged result of initializer `id` when the flat store evaluates
    /// it in place (specification 34 MEM-12, ARCH-05; C1-13): the initializer of a frame variable
    /// whose slot lies on the activation stack, whose result is an array, a structure or a text that
    /// a store copies as bytes, or a structure that lies as its backing built by its default alone
    /// (C1-15b), and whose value the value path builds in a staged slot (a body, or a default without
    /// a literal). The bytes are rounded up to [`INSTANCE_ALIGN`], so that a frame
    /// pushed above the result stays aligned. `None` for every other initializer.
    pub(crate) fn in_place_result(&self, id: u32) -> Option<(u32, u32)> {
        let entry = self.initializer(id as usize)?;
        let declaration = self.storage_declaration(entry.declaration_idx? as usize)?;
        if declaration.owner != StorageOwner::Frame
            || entry.body_kind != InitializerBodyKind::Action
        {
            return None;
        }
        let staged = entry.code_length != 0
            || (entry.stage == InitializationStage::Default
                && declaration.default_const_idx.is_none());
        if !staged {
            return None;
        }
        let (_, slot_type) =
            self.frame_slot(declaration.owner_pou_id?, declaration.slot as usize)?;
        let result = self.vm.ref_type(entry.result_ref_idx)?;
        // A structure that lies as its backing is built in place by its default, when no recipe can
        // build a value of it or of a part of it (C1-15b). So is an array or a structure that holds
        // references, by its default alone, which holds none (C1-19c4): any other body could give
        // it references its table keeps, whose bytes the commit could not copy.
        let in_place = super::byte_types::copies_as_bytes(self, result)
            || (super::byte_types::backing_place(self, result) && self.recipe_free(result, 0))
            || (super::byte_types::copies_with_references(self, result)
                && (entry.code_length == 0 || self.default_body(&entry, result))
                && self.recipe_free(result, 0));
        if !in_place || !super::byte_types::same_type(self, result, slot_type, 0) {
            return None;
        }
        let bytes = align_up(self.type_layout(result)?.shape.bytes, INSTANCE_ALIGN).ok()?;
        Some((result, bytes))
    }

    /// The type of the result of initializer `id` when the flat store builds it in its variable's own
    /// bytes (specification 34 MEM-12, ARCH-05; C1-22): the default, from no body or from the body the
    /// encoder writes for a variable that declares no initial value, of a global or of a variable of an
    /// instance, of an array, a structure or a text that a store copies as bytes, or of a structure
    /// that lies as its backing and that no recipe builds, which holds no reference and no instance.
    /// Such a result needs no room on the activation stack: the value path's commit would write all
    /// of its bytes into the variable anyway. `None` for every other initializer.
    pub(crate) fn persistent_in_place_result(&self, id: u32) -> Option<u32> {
        let entry = self.initializer(id as usize)?;
        let declaration = self.storage_declaration(entry.declaration_idx? as usize)?;
        if !matches!(
            declaration.owner,
            StorageOwner::Global | StorageOwner::Instance
        ) || entry.body_kind != InitializerBodyKind::Action
            || entry.stage != InitializationStage::Default
        {
            return None;
        }
        let result = self.vm.ref_type(entry.result_ref_idx)?;
        let default = if entry.code_length == 0 {
            declaration.default_const_idx.is_none()
        } else {
            self.default_body(&entry, result)
        };
        let layout = self.type_layout(result)?;
        let plain = layout.reference_leaves.is_empty() && layout.instance_leaves.is_empty();
        let in_place = super::byte_types::copies_as_bytes(self, result)
            || (super::byte_types::backing_place(self, result) && self.recipe_free(result, 0));
        (default
            && plain
            && in_place
            && super::byte_types::same_type(self, result, declaration.type_id?, 0))
        .then_some(result)
    }

    /// Whether the result of some initializer is built in its variable's own bytes (C1-22), which
    /// then takes a staged-result record of its own while it is built.
    pub(crate) fn has_persistent_in_place_result(&self) -> bool {
        (0..self.initializer_count()).any(|id| {
            u32::try_from(id).is_ok_and(|id| self.persistent_in_place_result(id).is_some())
        })
    }

    /// Whether the body of initializer `entry` only builds the default of its result's type `result`
    /// and stores it as the result, as the encoder writes the body of a variable that declares no
    /// initial value: `DEFAULT_TYPED` of that type, then the store of the result (C1-19c4).
    fn default_body(&self, entry: &crate::bytecode::InitializerEntry, result: u32) -> bool {
        let start = entry.code_offset as usize;
        let Some(code) = start
            .checked_add(entry.code_length as usize)
            .and_then(|end| self.vm.code.get(start..end))
        else {
            return false;
        };
        let [opcode, a, b, c, d, store, e, f, g, h] = *code else {
            return false;
        };
        opcode == crate::bytecode::opcodes::DEFAULT_TYPED
            && super::byte_types::same_type(self, u32::from_le_bytes([a, b, c, d]), result, 0)
            && store == 0x21
            && u32::from_le_bytes([e, f, g, h]) == entry.result_ref_idx
    }

    /// Whether no recipe, in any context, builds a value of `type_id` or of a member or an element of
    /// it at any depth, so that its default is built from the types alone (C1-15b).
    fn recipe_free(&self, type_id: u32, depth: usize) -> bool {
        if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return false;
        }
        if self.recipes.iter().any(|recipe| recipe.1 == type_id) {
            return false;
        }
        match self
            .vm
            .types
            .entries
            .get(type_id as usize)
            .map(|entry| &entry.data)
        {
            Some(TypeData::Alias { target_type_id }) => {
                self.recipe_free(*target_type_id, depth + 1)
            }
            Some(TypeData::Array { elem_type_id, .. }) => {
                self.recipe_free(*elem_type_id, depth + 1)
            }
            Some(TypeData::Struct { fields } | TypeData::Union { fields }) => fields
                .iter()
                .all(|field| self.recipe_free(field.type_id, depth + 1)),
            Some(_) => true,
            None => false,
        }
    }

    /// The per-call instances the activation stack holds at most (C1-11c): for each reserved call
    /// level, as many as the frame of any POU creates.
    pub(crate) fn call_instance_reservation(&self) -> Result<usize, RuntimeError> {
        self.largest_call_instances()?
            .checked_mul(self.reserved_call_levels())
            .ok_or(RuntimeError::Overflow)
    }

    /// The most instances on the activation stack that the frame of any POU creates (C1-11c), which
    /// bounds the identities one call's bookkeeping lists hold (C1-19c).
    pub(crate) fn largest_call_instances(&self) -> Result<usize, RuntimeError> {
        let mut largest = 0u32;
        for &pou in self.layouts.locals.keys() {
            let room = self.call_instance_room(pou).ok_or(RuntimeError::Overflow)?;
            largest = largest.max(room.count);
        }
        usize::try_from(largest).map_err(|_| RuntimeError::Overflow)
    }

    /// The reference leaves of a frame of `pou`, by their offset from the frame's first byte, in
    /// ascending order (C1-19c4).
    pub(crate) fn frame_reference_leaves(&self, pou: u32) -> &[u32] {
        self.layouts.frame_reference_leaves.leaves(pou)
    }

    /// The reference leaves of the own variables of an instance of `pou`, by their offset from the
    /// first byte of its own range, in ascending order (C1-19c4).
    pub(crate) fn instance_reference_leaves(&self, pou: u32) -> &[u32] {
        self.layouts.instance_reference_leaves.leaves(pou)
    }

    /// The reference leaves the activation stack holds at most (C1-19c4): for each reserved call
    /// level, [`Self::largest_call_reference_leaves`]. None when no POU has a frame.
    pub(crate) fn activation_reference_reservation(&self) -> Result<usize, RuntimeError> {
        self.largest_call_reference_leaves()?
            .checked_mul(self.reserved_call_levels())
            .ok_or(RuntimeError::Overflow)
    }

    /// The most reference leaves one call level holds on the activation stack (C1-19c4): those of
    /// the frame of any POU together with those of the instances its variables hold, and those of
    /// the largest staged result of an initializer evaluated in place.
    pub(crate) fn largest_call_reference_leaves(&self) -> Result<usize, RuntimeError> {
        let mut largest = 0usize;
        for &pou in self.layouts.locals.keys() {
            let room = self.call_instance_room(pou).ok_or(RuntimeError::Overflow)?;
            let leaves = self
                .frame_reference_leaves(pou)
                .len()
                .checked_add(room.leaves as usize)
                .ok_or(RuntimeError::Overflow)?;
            largest = largest.max(leaves);
        }
        // A staged result evaluated in place lies above the frame it initializes (C1-13).
        let staged = (0..self.initializer_count())
            .filter_map(|id| self.in_place_result(u32::try_from(id).ok()?))
            .filter_map(|(result, _)| self.type_layout(result))
            .map(|layout| layout.reference_leaves.len())
            .max()
            .unwrap_or(0);
        largest.checked_add(staged).ok_or(RuntimeError::Overflow)
    }

    /// Where slot `slot` of a frame of `pou` lies in the frame's bytes on the activation stack
    /// (specification 34 MEM-12; C1-11): its offset from the frame's first byte and its type. A
    /// variable whose value holds a reference lies there too, its references in the activation
    /// stack's reference table (C1-19c4); a compiler scratch slot, which has no type, stays a value
    /// of the frame, so it has none. So does an `ARRAY[*]` parameter: it has no bytes of its own,
    /// and each call gives it the shape of its actual argument (specification 12 §6.3; IEC
    /// 61131-3 §6.5.3), so the frame holds the argument's value (C1-19d).
    pub(crate) fn frame_slot(&self, pou: u32, slot: usize) -> Option<(u32, u32)> {
        let (id, declaration) = self.declaration(StorageOwner::Frame, Some(pou), slot)?;
        if !declaration.owns_storage() || declaration.owner_pou_id != Some(pou) {
            return None;
        }
        let type_id = declaration.type_id?;
        let variable_length = self
            .type_layout(type_id)?
            .array
            .as_ref()
            .is_some_and(|array| array.dims.iter().copied().any(is_wildcard_dim));
        if variable_length {
            return None;
        }
        Some((self.declaration_offset(id)?, type_id))
    }

    /// The activation byte range of an initializer's staged result.
    #[must_use]
    pub fn initializer_result_range(&self, initializer: u32) -> Option<ByteRange> {
        self.layouts
            .initializer_results
            .get(initializer as usize)
            .copied()
            .flatten()
    }

    /// Resolve a `REF_FIELD` member name to its member ordinal in a structure layout.
    #[must_use]
    pub fn ref_field_member_ordinal(&self, type_id: u32, name: &str) -> Option<u32> {
        let layout = self.layouts.types.get(type_id as usize)?;
        let ordinal = layout.members.iter().position(|member| {
            self.vm
                .strings
                .get(member.name_idx as usize)
                .is_some_and(|member_name| member_name.eq_ignore_ascii_case(name))
        })?;
        u32::try_from(ordinal).ok()
    }
}

impl crate::memory::figure::HeapBytes for PreparedLayouts {
    fn add_heap(&self, count: &mut crate::memory::figure::HeapCount) {
        count.of(&self.types);
        count.of(&self.globals);
        count.of(&self.templates);
        count.of(&self.instance_reference_leaves);
        count.of(&self.frame_reference_leaves);
        count.of(&self.locals);
        count.of(&self.initializer_results);
        count.of(&self.offsets);
    }
}

impl crate::memory::figure::HeapBytes for LeafLists {
    fn add_heap(&self, count: &mut crate::memory::figure::HeapCount) {
        count.of(&self.ranges);
        count.of(&self.leaves);
    }
}
