//! Prepared layouts and the byte ranges of the flat blocks (specification 34 §6.2, MEM-12).
//!
//! Preparation computes one [`TypeLayout`](crate::memory::layout::TypeLayout) per TYPE_TABLE
//! entry and maps globals, instance templates, POU locals and initializer results to byte ranges,
//! each variable at its declared size and alignment (design "Prepared layouts"). It charges the
//! bytes of those layout tables and the work of building them; a region's bytes are recorded here
//! and charged when construction reserves the region (specification 34 §6.2).

use super::*;
use crate::bytecode::{
    InitializationStage, InitializerBodyKind, StorageOwner, StorageRole, TypeData, TypeKind,
    BYTECODE_MAX_TYPE_NESTING,
};
use crate::memory::layout::{
    align_up, compute_type_layouts, ByteRange, ByteShape, InstanceTemplateLayout, LayoutRegion,
    TypeLayout, INSTANCE_ALIGN,
};

/// The layouts of one prepared module: one per TYPE_TABLE entry, plus the byte ranges of its
/// globals, instance templates, POU locals and initializer results, and each declaration's byte
/// offset within the block or the template that holds it.
#[derive(Debug, Default)]
pub(crate) struct PreparedLayouts {
    types: Vec<TypeLayout>,
    globals: Vec<Option<ByteRange>>,
    templates: BTreeMap<u32, InstanceTemplateLayout>,
    /// The reference leaves of each POU's own instance variables, by their offset from the
    /// template's first byte, and of each POU's frame, by their offset from the frame's first byte,
    /// ascending, for the activation stack's reference table (C1-19c4). A POU without any has no
    /// entry.
    instance_reference_leaves: BTreeMap<u32, Vec<u32>>,
    frame_reference_leaves: BTreeMap<u32, Vec<u32>>,
    locals: BTreeMap<u32, ByteRange>,
    initializer_results: Vec<Option<ByteRange>>,
    /// Each storage declaration's byte offset: a global's in the persistent block, an instance
    /// variable's in its POU's template and a frame variable's in its POU's frame.
    offsets: Vec<u32>,
    /// What these layout tables and their computation were charged (specification 34 §6.2).
    charged: PreparationUsage,
}

impl PreparedLayouts {
    /// Compute every layout once, charged through the preparation budget.
    pub(crate) fn build(
        vm: &VmModule,
        layout: &StorageLayout,
        initializers: &InitializerIndex,
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
            let shape = shape_of(&types, type_id)?;
            let cursor = own.entry(pou).or_insert(0);
            let start = align_up(*cursor, shape.align)?;
            offsets[index] = start;
            *cursor = slot_end(start, shape, declaration.role)?;
        }
        let mut templates = BTreeMap::new();
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
        budget.map::<u32, InstanceTemplateLayout>(templates.len())?;

        // Each POU's frame variables, then the initializer results, on the activation stack.
        let mut activation_cursor = 0u32;
        let mut locals = BTreeMap::new();
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
        budget.map::<u32, ByteRange>(locals.len())?;

        // The reference leaves of each template's own variables and of each frame (C1-19c4).
        let mut instance_reference_leaves: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        let mut frame_reference_leaves: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        let mut leaf_count = 0usize;
        for (index, declaration) in layout.entries.iter().enumerate() {
            let owner = match declaration.owner {
                StorageOwner::Instance => &mut instance_reference_leaves,
                StorageOwner::Frame => &mut frame_reference_leaves,
                StorageOwner::Global => continue,
            };
            let (Some(pou), Some(type_id)) = (declaration.owner_pou_id, declaration.type_id) else {
                continue;
            };
            let Some(type_layout) = types.get(type_id as usize).filter(|type_layout| {
                declaration.owns_storage() && !type_layout.reference_leaves.is_empty()
            }) else {
                continue;
            };
            let leaves = owner.entry(pou).or_default();
            for leaf in &type_layout.reference_leaves {
                leaves.push(
                    offsets[index]
                        .checked_add(leaf.byte_offset)
                        .ok_or(RuntimeError::Overflow)?,
                );
                leaf_count += 1;
            }
        }
        for leaves in instance_reference_leaves
            .values_mut()
            .chain(frame_reference_leaves.values_mut())
        {
            leaves.sort_unstable();
        }
        budget
            .map::<u32, Vec<u32>>(instance_reference_leaves.len() + frame_reference_leaves.len())?;
        budget.records::<u32>(leaf_count)?;

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
        let entry = self.layout.entries.get(declaration as usize)?;
        if !holds_presence(entry.role) {
            return None;
        }
        start.checked_add(self.type_layout(entry.type_id?)?.shape.bytes)
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
            let (instances, _, _) = self.call_instance_room(pou).ok_or(RuntimeError::Overflow)?;
            largest = largest.max(
                range
                    .bytes
                    .checked_add(instances)
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
        (0..self.initializers.entries.len())
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
        let entry = self.initializers.entries.get(id as usize)?;
        let declaration = self.layout.entries.get(entry.declaration_idx? as usize)?;
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
                && (entry.code_length == 0 || self.default_body(entry, result))
                && self.recipe_free(result, 0));
        if !in_place || !super::byte_types::same_type(self, result, slot_type, 0) {
            return None;
        }
        let bytes = align_up(self.type_layout(result)?.shape.bytes, INSTANCE_ALIGN).ok()?;
        Some((result, bytes))
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
            let (_, count, _) = self.call_instance_room(pou).ok_or(RuntimeError::Overflow)?;
            largest = largest.max(count);
        }
        usize::try_from(largest).map_err(|_| RuntimeError::Overflow)
    }

    /// The reference leaves of a frame of `pou`, by their offset from the frame's first byte, in
    /// ascending order (C1-19c4).
    pub(crate) fn frame_reference_leaves(&self, pou: u32) -> &[u32] {
        self.layouts
            .frame_reference_leaves
            .get(&pou)
            .map_or(&[], Vec::as_slice)
    }

    /// The reference leaves of the own variables of an instance of `pou`, by their offset from the
    /// first byte of its own range, in ascending order (C1-19c4).
    pub(crate) fn instance_reference_leaves(&self, pou: u32) -> &[u32] {
        self.layouts
            .instance_reference_leaves
            .get(&pou)
            .map_or(&[], Vec::as_slice)
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
            let (_, _, instances) = self.call_instance_room(pou).ok_or(RuntimeError::Overflow)?;
            let leaves = self
                .frame_reference_leaves(pou)
                .len()
                .checked_add(instances as usize)
                .ok_or(RuntimeError::Overflow)?;
            largest = largest.max(leaves);
        }
        // A staged result evaluated in place lies above the frame it initializes (C1-13).
        let staged = (0..self.initializers.entries.len())
            .filter_map(|id| self.in_place_result(u32::try_from(id).ok()?))
            .filter_map(|(result, _)| self.type_layout(result))
            .map(|layout| layout.reference_leaves.len())
            .max()
            .unwrap_or(0);
        largest.checked_add(staged).ok_or(RuntimeError::Overflow)
    }

    /// The bytes, the number and the reference leaves of the instances a frame of `pou` creates for
    /// its function-block and class variables, which lie on the activation stack above the frame
    /// (C1-11c, C1-19c4).
    fn call_instance_room(&self, pou: u32) -> Option<(u32, u32, u32)> {
        let mut room = (0u32, 0u32, 0u32);
        for declaration in &self.layout.entries {
            if declaration.owner != StorageOwner::Frame
                || declaration.owner_pou_id != Some(pou)
                || !declaration.owns_storage()
            {
                continue;
            }
            let Some(type_id) = declaration.type_id else {
                continue;
            };
            let (bytes, count, leaves) = self.instance_tree(type_id, 0)?;
            room = (
                room.0.checked_add(bytes)?,
                room.1.checked_add(count)?,
                room.2.checked_add(leaves)?,
            );
        }
        Some(room)
    }

    /// The bytes, the number and the reference leaves of the instances on the activation stack a
    /// value of `type_id` holds: a function block or a class its instance, one for each level of
    /// its inheritance chain with that level's own variables, and the instances those variables
    /// hold; an array each element's; a structure each member's.
    fn instance_tree(&self, type_id: u32, depth: usize) -> Option<(u32, u32, u32)> {
        if depth > BYTECODE_MAX_TYPE_NESTING {
            return None;
        }
        let mut current = type_id;
        let entry = loop {
            let entry = self.vm.types.entries.get(current as usize)?;
            match &entry.data {
                TypeData::Alias { target_type_id } => current = *target_type_id,
                _ => break entry,
            }
        };
        match &entry.data {
            TypeData::Pou { pou_id }
                if matches!(entry.kind, TypeKind::FunctionBlock | TypeKind::Class) =>
            {
                let mut total = (0u32, 0u32, 0u32);
                let mut level = Some(*pou_id);
                for _ in 0..=BYTECODE_MAX_TYPE_NESTING {
                    let Some(pou) = level else {
                        break;
                    };
                    let own = self
                        .instance_template(pou)
                        .map_or(0, |template| template.range.bytes);
                    let leaves = u32::try_from(self.instance_reference_leaves(pou).len()).ok()?;
                    total = (
                        total.0.checked_add(own)?,
                        total.1.checked_add(1)?,
                        total.2.checked_add(leaves)?,
                    );
                    for &id in self.declarations(StorageOwner::Instance, Some(pou)) {
                        let member = self.layout.entries.get(id as usize)?;
                        let Some(member_type) = member.type_id.filter(|_| member.owns_storage())
                        else {
                            continue;
                        };
                        let (bytes, count, leaves) = self.instance_tree(member_type, depth + 1)?;
                        total = (
                            total.0.checked_add(bytes)?,
                            total.1.checked_add(count)?,
                            total.2.checked_add(leaves)?,
                        );
                    }
                    level = self.vm.parent_pou_ids.get(&pou).copied();
                }
                Some(total)
            }
            TypeData::Array { elem_type_id, dims } => {
                let (bytes, count, leaves) = self.instance_tree(*elem_type_id, depth + 1)?;
                // An array of values that hold no instance holds none, whatever its bounds (an
                // `ARRAY[*]` parameter has none of its own).
                if (bytes, count, leaves) == (0, 0, 0) {
                    return Some((0, 0, 0));
                }
                let elements = dims.iter().try_fold(1u32, |elements, (lower, upper)| {
                    let length = u32::try_from(upper.checked_sub(*lower)?.checked_add(1)?).ok()?;
                    elements.checked_mul(length)
                })?;
                Some((
                    bytes.checked_mul(elements)?,
                    count.checked_mul(elements)?,
                    leaves.checked_mul(elements)?,
                ))
            }
            TypeData::Struct { fields } | TypeData::Union { fields } => {
                fields.iter().try_fold((0u32, 0u32, 0u32), |total, field| {
                    let (bytes, count, leaves) = self.instance_tree(field.type_id, depth + 1)?;
                    Some((
                        total.0.checked_add(bytes)?,
                        total.1.checked_add(count)?,
                        total.2.checked_add(leaves)?,
                    ))
                })
            }
            _ => Some((0, 0, 0)),
        }
    }

    /// Where slot `slot` of a frame of `pou` lies in the frame's bytes on the activation stack
    /// (specification 34 MEM-12; C1-11): its offset from the frame's first byte and its type. A
    /// variable whose value holds a reference lies there too, its references in the activation
    /// stack's reference table (C1-19c4); a compiler scratch slot, which has no type, stays a value
    /// of the frame, so it has none.
    pub(crate) fn frame_slot(&self, pou: u32, slot: usize) -> Option<(u32, u32)> {
        let (id, declaration) = self.declaration(StorageOwner::Frame, Some(pou), slot)?;
        if !declaration.owns_storage() || declaration.owner_pou_id != Some(pou) {
            return None;
        }
        let type_id = declaration.type_id?;
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
