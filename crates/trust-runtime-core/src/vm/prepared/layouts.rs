//! Prepared layouts and the byte ranges of the flat blocks (specification 34 §6.2, MEM-12).
//!
//! Preparation computes one [`TypeLayout`](crate::memory::layout::TypeLayout) per TYPE_TABLE
//! entry and maps globals, instance templates, POU locals and initializer results to byte ranges,
//! each variable at its declared size and alignment (design "Prepared layouts"). It charges the
//! bytes of those layout tables and the work of building them; a region's bytes are recorded here
//! and charged when construction reserves the region (specification 34 §6.2).

use super::*;
use crate::bytecode::{StorageOwner, StorageRole};
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
            &mut |id: u32| -> Result<u32, RuntimeError> {
                let physical = vm.physical_layout(id).ok_or(RuntimeError::TypeMismatch)?;
                u32::try_from(physical.byte_len()?).map_err(|_| RuntimeError::Overflow)
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
