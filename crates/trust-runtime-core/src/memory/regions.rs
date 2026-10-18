//! The cell regions of the bounded value store (specification 34 §6.2, MEM-10; C1-9a).
//!
//! [`crate::memory::regions::CellStorage`] is the store the engine switches to in C1-9d. Its
//! construction reads the declarations and the prepared layouts, charges the bytes of the persistent
//! region (its cells, its reference records and its text buffers) through the construction
//! allowance, and only then reserves them, once. Each persistent root gets its own cell range (O8),
//! and each text leaf occurrence of a global, of every array element and of every root gets its own
//! buffer (O10). Until C1-9d the engine keeps running on [`crate::memory::VariableStorage`], does
//! not construct this store, and nothing reads these regions yet.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::bytecode::{StorageOwner, TypeData, TypeKind, BYTECODE_MAX_TYPE_NESTING};
use crate::error::RuntimeError;
use crate::memory::cells::CellStore;
use crate::memory::layout::{CellRange, LayoutRegion, TextLeaf};
use crate::memory::text::TextStore;
use crate::memory::{ActivationStore, Generation, Identities};
use crate::value::ValueRef;
use crate::vm::PreparedModule;

/// The persistent region's cell ranges, its cells and reference records, and its text buffers
/// (specification 34 §6.2).
#[derive(Debug, Default)]
pub struct CellRegions {
    /// The cell range of each global declaration, by its STORAGE_LAYOUT index.
    globals: Vec<Option<CellRange>>,
    /// The cell range of each persistent root, by its CONSTRUCTION_ROOTS index. Every root has
    /// its own range (O8), so two instances of one function block type get two ranges.
    roots: Vec<Option<CellRange>>,
    /// The region's cells and reference records.
    persistent: CellStore,
    /// The region's text buffers: one per text leaf occurrence (O10).
    text: TextStore,
    /// The non-null reference leaves and the `ValueRef` each holds, keyed by the leaf's cell
    /// (O9, C1-9c). The reference records stay unused until C1-11, so the reference a leaf holds
    /// lives here, on the `Value` side.
    references: BTreeMap<u32, ValueRef>,
}

impl CellRegions {
    /// The cell range of the global declaration at STORAGE_LAYOUT index `index`, or `None` when
    /// that declaration owns no persistent cells.
    #[must_use]
    pub fn global_range(&self, index: usize) -> Option<CellRange> {
        self.globals.get(index).copied().flatten()
    }

    /// The cell range of the persistent root at CONSTRUCTION_ROOTS index `index`, or `None` when
    /// that root is not an instance.
    #[must_use]
    pub fn root_range(&self, index: usize) -> Option<CellRange> {
        self.roots.get(index).copied().flatten()
    }

    /// The region's cells and reference records.
    #[must_use]
    pub fn persistent(&self) -> &CellStore {
        &self.persistent
    }

    /// The region's text buffers.
    #[must_use]
    pub fn text(&self) -> &TextStore {
        &self.text
    }

    /// The `ValueRef` a non-null reference leaf at `cell` holds, or `None` for a null leaf.
    pub(crate) fn reference(&self, cell: u32) -> Option<&ValueRef> {
        self.references.get(&cell)
    }

    /// The persistent cells, the text buffers and the reference values, mutably at once (C1-9c).
    pub(crate) fn split_mut(
        &mut self,
    ) -> (&mut CellStore, &mut TextStore, &mut BTreeMap<u32, ValueRef>) {
        (&mut self.persistent, &mut self.text, &mut self.references)
    }
}

/// The bounded value store's cells (specification 34 §6.2): the regions, the activation half and
/// the identities. C1-9a builds it only through [`CellStorage::construct`]; the engine constructs it
/// and charges it from C1-9d, when it switches (O18).
#[derive(Debug, Default)]
#[allow(
    dead_code,
    reason = "the engine reads the activation and identity halves from C1-9d; C1-9a only constructs the store"
)]
pub struct CellStorage {
    regions: CellRegions,
    activations: ActivationStore,
    identities: Identities,
}

impl CellStorage {
    /// Build the persistent region of `prepared` for `generation` (specification 34 §6.2; O18).
    ///
    /// Before anything is reserved, `charge` receives the bytes the region takes: its cells and
    /// reference records as [`CellStore::reservation_bytes`] counts them, plus its text buffers'
    /// bytes. A refused charge returns its error, and nothing is reserved.
    ///
    /// # Errors
    ///
    /// Returns the error `charge` refuses with, a preparation error when the layouts and the
    /// declarations disagree, or an allocation refusal.
    pub fn construct(
        prepared: &PreparedModule,
        generation: Generation,
        charge: impl FnOnce(usize) -> Result<(), RuntimeError>,
    ) -> Result<Self, RuntimeError> {
        let extent = PersistentExtent::of(prepared)?;
        let cell_bytes = CellStore::reservation_bytes(extent.cells, extent.references)
            .ok_or(RuntimeError::Overflow)?;
        charge(
            cell_bytes
                .checked_add(extent.text_bytes()?)
                .ok_or(RuntimeError::Overflow)?,
        )?;
        Ok(Self {
            regions: CellRegions {
                globals: extent.globals,
                roots: extent.roots,
                persistent: CellStore::reserve(extent.cells, extent.references)?,
                text: TextStore::reserve(&extent.texts)?,
                references: BTreeMap::new(),
            },
            activations: ActivationStore::default(),
            identities: Identities {
                generation,
                ..Identities::default()
            },
        })
    }

    /// The persistent region and its ranges.
    #[must_use]
    pub fn regions(&self) -> &CellRegions {
        &self.regions
    }
}

/// What the persistent region holds, read from the declarations and the prepared layouts
/// (specification 34 §6.2): the global and root ranges, the cells and reference records they
/// need, and one text leaf per text occurrence in cell order.
pub(crate) struct PersistentExtent {
    globals: Vec<Option<CellRange>>,
    roots: Vec<Option<CellRange>>,
    pub(crate) cells: u32,
    pub(crate) references: u32,
    texts: Vec<TextLeaf>,
}

impl PersistentExtent {
    /// Read the persistent region of `prepared`: the globals first, then each persistent root in
    /// CONSTRUCTION_ROOTS order (specification 34 §6.2, O6, O8). A root that is not an inheritance
    /// parent reserves its whole inheritance chain: its own cells, then its parent's, and so on up
    /// (R0, C1-9c). The inheritance parent of a derived root lies inside that root's range and
    /// reserves no cells of its own, so no cell is reserved twice.
    pub(crate) fn of(prepared: &PreparedModule) -> Result<Self, RuntimeError> {
        let mut leaves = Leaves::default();
        let mut globals = alloc::vec![None; prepared.layout.entries.len()];
        let mut cells = 0u32;
        for (slot, declaration) in prepared.layout.entries.iter().enumerate() {
            if declaration.owner != StorageOwner::Global || !declaration.owns_storage() {
                continue;
            }
            let Some(type_id) = declaration.type_id else {
                continue;
            };
            let index = u32::try_from(slot).map_err(|_| RuntimeError::Overflow)?;
            let range = prepared
                .global_cells(index)
                .ok_or(RuntimeError::TypeMismatch)?;
            globals[slot] = Some(range);
            cells = cells
                .checked_add(range.cells)
                .ok_or(RuntimeError::Overflow)?;
            leaves.add_value(prepared, type_id, range.start, 0)?;
        }

        let mut roots: Vec<Option<CellRange>> = alloc::vec![None; prepared.roots.entries.len()];
        for (index, root) in prepared.roots.entries.iter().enumerate() {
            let Some(pou) = root.template_pou_id else {
                continue;
            };
            let range = if root.is_inheritance_parent() {
                let child = root.parent_root_idx.ok_or(RuntimeError::TypeMismatch)?;
                inherited_range(prepared, &roots, child, pou)?
            } else {
                let chain = chain_of(prepared, pou)?;
                let total = chain_total(&chain)?;
                let range = CellRange {
                    region: LayoutRegion::Persistent,
                    start: cells,
                    cells: total,
                };
                let mut base = range.start;
                for &(member, own) in &chain {
                    if leaves.add_instance(prepared, member, base)? != own {
                        return Err(RuntimeError::TypeMismatch);
                    }
                    base = base.checked_add(own).ok_or(RuntimeError::Overflow)?;
                }
                cells = cells.checked_add(total).ok_or(RuntimeError::Overflow)?;
                range
            };
            roots[index] = Some(range);
        }
        Ok(Self {
            globals,
            roots,
            cells,
            references: leaves.references,
            texts: leaves.texts,
        })
    }

    /// The bytes the text buffers reserve: one capacity per text leaf.
    fn text_bytes(&self) -> Result<usize, RuntimeError> {
        self.texts.iter().try_fold(0usize, |bytes, leaf| {
            let capacity =
                usize::try_from(leaf.capacity_bytes).map_err(|_| RuntimeError::Overflow)?;
            bytes.checked_add(capacity).ok_or(RuntimeError::Overflow)
        })
    }
}

/// The text leaves and reference leaves found while walking the persistent region.
#[derive(Default)]
struct Leaves {
    texts: Vec<TextLeaf>,
    references: u32,
}

impl Leaves {
    /// Add the leaves of one persistent instance of template `pou` placed at cell `base`: its own
    /// declarations in order, each at its offset within the template. Answers the cells the
    /// declarations take, which the template's own cell count must equal (specification 34 §6.2).
    fn add_instance(
        &mut self,
        prepared: &PreparedModule,
        pou: u32,
        base: u32,
    ) -> Result<u32, RuntimeError> {
        let mut offset = 0u32;
        for declaration in prepared.instance_declarations(pou) {
            let Some(type_id) = declaration.type_id else {
                continue;
            };
            if !declaration.owns_storage() {
                continue;
            }
            self.add_value(prepared, type_id, offset_of(base, offset)?, 0)?;
            let cells = prepared
                .type_layout(type_id)
                .ok_or(RuntimeError::TypeMismatch)?
                .cells;
            offset = offset.checked_add(cells).ok_or(RuntimeError::Overflow)?;
        }
        Ok(offset)
    }

    /// Add the text and reference leaves of one value of `type_id` placed at cell `base`. Arrays
    /// expand per element and structures per member, so each leaf occurrence is counted once
    /// (O10). Unions and overlapping structures hold no leaves here: C1-15 places them
    /// (specification 34 §6.2).
    fn add_value(
        &mut self,
        prepared: &PreparedModule,
        type_id: u32,
        base: u32,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        if depth > BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        let entry = prepared
            .vm
            .types
            .entries
            .get(type_id as usize)
            .ok_or(RuntimeError::TypeMismatch)?;
        let layout = prepared
            .type_layout(type_id)
            .ok_or(RuntimeError::TypeMismatch)?;
        match &entry.data {
            TypeData::Primitive { .. } => {
                for leaf in &layout.text_leaves {
                    self.texts.push(TextLeaf {
                        offset: offset_of(base, leaf.offset)?,
                        capacity_bytes: leaf.capacity_bytes,
                    });
                }
            }
            TypeData::Reference { .. } => {
                self.references = self
                    .references
                    .checked_add(1)
                    .ok_or(RuntimeError::Overflow)?;
            }
            TypeData::Alias { target_type_id } => {
                self.add_value(prepared, *target_type_id, base, depth + 1)?;
            }
            TypeData::Array { elem_type_id, .. } => {
                let Some(array) = &layout.array else {
                    return Err(RuntimeError::TypeMismatch);
                };
                // An `ARRAY[*]` has no cells of its own (`layout.rs`), so it holds no leaves here.
                if array.stride == 0 {
                    return Ok(());
                }
                for element in 0..layout.cells / array.stride {
                    let stride_offset = element
                        .checked_mul(array.stride)
                        .ok_or(RuntimeError::Overflow)?;
                    let start = offset_of(base, stride_offset)?;
                    self.add_value(prepared, *elem_type_id, start, depth + 1)?;
                }
            }
            TypeData::Struct { fields } if entry.kind != TypeKind::OverlappingStruct => {
                for (field, member) in fields.iter().zip(&layout.members) {
                    let start = offset_of(base, member.offset)?;
                    self.add_value(prepared, field.type_id, start, depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// The cell `relative` cells after `base`, or an overflow refusal.
fn offset_of(base: u32, relative: u32) -> Result<u32, RuntimeError> {
    base.checked_add(relative).ok_or(RuntimeError::Overflow)
}

/// The own cells of `pou`'s template: none for a POU without a template, which has no own variables
/// and no parent (O7).
fn own_cells(prepared: &PreparedModule, pou: u32) -> u32 {
    prepared
        .instance_template_cells(pou)
        .map_or(0, |template| template.range.cells)
}

/// The POUs of `pou`'s inheritance chain, `pou` first and then each parent, each with the own cells of
/// its template (O6).
fn chain_of(prepared: &PreparedModule, pou: u32) -> Result<Vec<(u32, u32)>, RuntimeError> {
    let mut chain = Vec::new();
    let mut current = Some(pou);
    for _ in 0..=BYTECODE_MAX_TYPE_NESTING {
        let Some(member) = current else {
            return Ok(chain);
        };
        let parent = prepared
            .instance_template_cells(member)
            .and_then(|template| template.parent);
        chain.push((member, own_cells(prepared, member)));
        current = parent;
    }
    Err(RuntimeError::TypeMismatch)
}

/// The cells of a whole inheritance chain: the sum of its own cells.
fn chain_total(chain: &[(u32, u32)]) -> Result<u32, RuntimeError> {
    chain.iter().try_fold(0u32, |sum, &(_, own)| {
        sum.checked_add(own).ok_or(RuntimeError::Overflow)
    })
}

/// The range of the inheritance parent `pou` of the root `child` (R0): it starts where the child's own
/// cells end, spans its own chain, and must lie inside the child's range.
fn inherited_range(
    prepared: &PreparedModule,
    roots: &[Option<CellRange>],
    child: u32,
    pou: u32,
) -> Result<CellRange, RuntimeError> {
    let child_range = roots
        .get(child as usize)
        .copied()
        .flatten()
        .ok_or(RuntimeError::TypeMismatch)?;
    let child_pou = prepared
        .roots
        .entries
        .get(child as usize)
        .and_then(|root| root.template_pou_id)
        .ok_or(RuntimeError::TypeMismatch)?;
    let start = child_range
        .start
        .checked_add(own_cells(prepared, child_pou))
        .ok_or(RuntimeError::Overflow)?;
    let cells = chain_total(&chain_of(prepared, pou)?)?;
    let end = start.checked_add(cells).ok_or(RuntimeError::Overflow)?;
    let child_end = child_range
        .start
        .checked_add(child_range.cells)
        .ok_or(RuntimeError::Overflow)?;
    if end > child_end {
        return Err(RuntimeError::TypeMismatch);
    }
    Ok(CellRange {
        region: LayoutRegion::Persistent,
        start,
        cells,
    })
}

impl CellStorage {
    /// The installation generation this store's references carry (specification 34 §11.3, MEM-07).
    #[must_use]
    pub fn generation(&self) -> Generation {
        self.identities.generation
    }
}

impl CellStorage {
    /// The regions, mutably: the cell arm's writes publish into them (C1-9c).
    pub(crate) fn regions_mut(&mut self) -> &mut CellRegions {
        &mut self.regions
    }
}

/// Test seams for the cell arm (C1-9b): a fixture writes its initial values through these accessors.
#[cfg(test)]
impl CellRegions {
    /// The persistent cells and the text buffers, mutably.
    pub(crate) fn parts_mut(&mut self) -> (&mut CellStore, &mut TextStore) {
        (&mut self.persistent, &mut self.text)
    }
}
