//! Prepared cell layouts and the region ranges they reserve (specification 34 §6.2, MEM-10).
//!
//! Preparation computes one [`TypeLayout`](crate::memory::layout::TypeLayout) per TYPE_TABLE
//! entry and maps globals, instance templates, POU locals and initializer results to cell ranges
//! (design "Prepared layouts"). It charges the bytes of those layout tables and the work of
//! building them; a region's cells are recorded here and charged when construction reserves the
//! region (specification 34 §6.2). Nothing reads them at run time yet; later slices store values
//! by them.

use super::*;
use crate::bytecode::{StorageOwner, BYTECODE_MAX_TYPE_NESTING};
use crate::memory::layout::{
    compute_type_layouts, CellRange, InstanceTemplateLayout, LayoutRegion, TypeLayout,
};

/// The cell layouts of one prepared module: one per TYPE_TABLE entry, plus the region ranges of
/// its globals, instance templates, POU locals and initializer results.
#[derive(Debug, Default)]
pub(crate) struct PreparedLayouts {
    types: Vec<TypeLayout>,
    globals: Vec<Option<CellRange>>,
    templates: BTreeMap<u32, InstanceTemplateLayout>,
    locals: BTreeMap<u32, CellRange>,
    initializer_results: Vec<Option<CellRange>>,
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
        // Everything charged from here to the end of this function is the layout tables' own
        // demand, recorded so later slices can see it (specification 34 §6.2).
        let charged_before = budget.usage();
        let types = compute_type_layouts(
            &vm.types,
            &mut |id: u32| -> Result<u32, RuntimeError> {
                let physical = vm.physical_layout(id).ok_or(RuntimeError::TypeMismatch)?;
                u32::try_from(physical.byte_len()?).map_err(|_| RuntimeError::Overflow)
            },
            &mut |bytes, work| budget.charge(bytes, work),
        )?;

        let mut persistent_cursor = 0u32;
        let mut globals: Vec<Option<CellRange>> = Vec::new();
        globals.resize(layout.entries.len(), None);
        for (index, declaration) in layout.entries.iter().enumerate() {
            if declaration.owner != StorageOwner::Global || !declaration.owns_storage() {
                continue;
            }
            let Some(type_id) = declaration.type_id else {
                continue;
            };
            let cells = cells_of(&types, type_id)?;
            globals[index] = Some(CellRange {
                region: LayoutRegion::Persistent,
                start: persistent_cursor,
                cells,
            });
            persistent_cursor = persistent_cursor
                .checked_add(cells)
                .ok_or(RuntimeError::Overflow)?;
        }
        budget.records::<Option<CellRange>>(globals.len())?;

        // A template's own cells are its POU's instance declarations; a derived POU places its
        // parent's template cells first (design "Prepared layouts").
        let mut own: BTreeMap<u32, u32> = BTreeMap::new();
        for declaration in &layout.entries {
            if declaration.owner != StorageOwner::Instance || !declaration.owns_storage() {
                continue;
            }
            let Some(pou) = declaration.owner_pou_id else {
                continue;
            };
            let Some(type_id) = declaration.type_id else {
                continue;
            };
            let cells = cells_of(&types, type_id)?;
            let slot = own.entry(pou).or_insert(0);
            *slot = slot.checked_add(cells).ok_or(RuntimeError::Overflow)?;
        }
        let mut templates = BTreeMap::new();
        for &pou in own.keys() {
            let total = template_total(pou, &own, vm, 0)?;
            templates.insert(
                pou,
                InstanceTemplateLayout {
                    range: CellRange {
                        region: LayoutRegion::Persistent,
                        start: persistent_cursor,
                        cells: total,
                    },
                    parent: vm.parent_pou_ids.get(&pou).copied(),
                },
            );
            persistent_cursor = persistent_cursor
                .checked_add(total)
                .ok_or(RuntimeError::Overflow)?;
        }
        budget.map::<u32, InstanceTemplateLayout>(templates.len())?;

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
            let cells = frame_cells(&types, layout, pou)?;
            locals.insert(
                pou,
                CellRange {
                    region: LayoutRegion::Activation,
                    start: activation_cursor,
                    cells,
                },
            );
            activation_cursor = activation_cursor
                .checked_add(cells)
                .ok_or(RuntimeError::Overflow)?;
        }
        budget.map::<u32, CellRange>(locals.len())?;

        let mut initializer_results: Vec<Option<CellRange>> = Vec::new();
        initializer_results.resize(initializers.entries.len(), None);
        for (id, entry) in initializers.entries.iter().enumerate() {
            let Some(type_id) = vm.ref_type(entry.result_ref_idx) else {
                continue;
            };
            let cells = cells_of(&types, type_id)?;
            initializer_results[id] = Some(CellRange {
                region: LayoutRegion::Activation,
                start: activation_cursor,
                cells,
            });
            activation_cursor = activation_cursor
                .checked_add(cells)
                .ok_or(RuntimeError::Overflow)?;
        }
        budget.records::<Option<CellRange>>(initializer_results.len())?;

        // A region's cells are storage that construction reserves, so preparation records the
        // ranges above and charges the cells then, not here (specification 34 §6.2).
        let charged_after = budget.usage();
        Ok(Self {
            types,
            globals,
            templates,
            locals,
            initializer_results,
            // Usage only grows across the charges above.
            charged: PreparationUsage {
                bytes: charged_after.bytes - charged_before.bytes,
                work: charged_after.work - charged_before.work,
            },
        })
    }
}

/// A POU's template cell count: its own cells followed by its parent chain's, parent cells first.
fn template_total(
    pou: u32,
    own: &BTreeMap<u32, u32>,
    vm: &VmModule,
    depth: usize,
) -> Result<u32, RuntimeError> {
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    let mut total = own.get(&pou).copied().unwrap_or(0);
    if let Some(parent) = vm.parent_pou_ids.get(&pou).copied() {
        total = total
            .checked_add(template_total(parent, own, vm, depth + 1)?)
            .ok_or(RuntimeError::Overflow)?;
    }
    Ok(total)
}

/// A POU's frame-local cell count: the sum of its frame declarations' type layouts.
fn frame_cells(
    types: &[TypeLayout],
    layout: &StorageLayout,
    pou: u32,
) -> Result<u32, RuntimeError> {
    let mut cells = 0u32;
    for declaration in &layout.entries {
        if declaration.owner != StorageOwner::Frame
            || !declaration.owns_storage()
            || declaration.owner_pou_id != Some(pou)
        {
            continue;
        }
        let Some(type_id) = declaration.type_id else {
            continue;
        };
        cells = cells
            .checked_add(cells_of(types, type_id)?)
            .ok_or(RuntimeError::Overflow)?;
    }
    Ok(cells)
}

/// The prepared cell count of one TYPE_TABLE entry.
fn cells_of(types: &[TypeLayout], type_id: u32) -> Result<u32, RuntimeError> {
    types
        .get(type_id as usize)
        .map(|layout| layout.cells)
        .ok_or(RuntimeError::TypeMismatch)
}

impl PreparedModule {
    /// The preparation bytes and work the layout tables and their computation were charged
    /// (specification 34 §6.2). A region's cells are not part of it: construction charges them
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

    /// The persistent cell range a global declaration owns, absent for other owners.
    #[must_use]
    pub fn global_cells(&self, declaration: u32) -> Option<CellRange> {
        self.layouts
            .globals
            .get(declaration as usize)
            .copied()
            .flatten()
    }

    /// The reserved cell range of a POU's instance template, with its parent's cells first.
    #[must_use]
    pub fn instance_template_cells(&self, pou: u32) -> Option<InstanceTemplateLayout> {
        self.layouts.templates.get(&pou).copied()
    }

    /// The activation cell range of a POU's frame locals.
    #[must_use]
    pub fn local_cells(&self, pou: u32) -> Option<CellRange> {
        self.layouts.locals.get(&pou).copied()
    }

    /// The activation cell range of an initializer's staged result.
    #[must_use]
    pub fn initializer_result_cells(&self, initializer: u32) -> Option<CellRange> {
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
