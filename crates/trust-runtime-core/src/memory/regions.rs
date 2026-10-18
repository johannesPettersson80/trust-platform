//! The cell regions of the bounded value store (specification 34 §6.2, MEM-10; C1-9a).
//!
//! [`crate::memory::regions::CellStorage`] is the store the engine switches to in C1-9d. Its
//! construction reads the declarations and the prepared layouts, charges the bytes of the persistent
//! region (its cells, its reference records, its text buffers, its reference table, its range tables and
//! the leaf lists the construction builds) through the construction allowance, and only then reserves the
//! region, once. Each persistent root gets its own cell range (O8), and each text leaf occurrence of a
//! global, of every array element and of every root gets its own buffer (O10). Construction then writes
//! every declared leaf's initial value into its cells (R2), with the date/time profile of the engine
//! that constructs it (C1-9d2b-1). The engine constructs this store when it is asked to
//! (`vm::engine`) and otherwise keeps running on [`crate::memory::VariableStorage`].

use alloc::vec::Vec;

use smol_str::SmolStr;

use crate::bytecode::{
    StorageOwner, StorageRole, TypeData, TypeEntry, TypeKind, BYTECODE_MAX_TYPE_NESTING,
};
use crate::error::RuntimeError;
use crate::memory::cells::{write_value_in, Cell, CellStore, ReferenceTable};
use crate::memory::layout::{CellRange, LayoutRegion, TextLeaf};
use crate::memory::text::TextStore;
use crate::memory::{ActivationStore, Generation, Identities, InstanceId};
use crate::value::{DateTimeProfile, EnumValue, Value, ValueRef};
use crate::vm::construction::values::scalar;
use crate::vm::type_policy::{normalize_primitive_value, resolved_primitive_shape};
use crate::vm::PreparedModule;

/// The persistent region's cell ranges, its cells and reference records, its text buffers and its
/// reference table (specification 34 §6.2).
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
    /// The reference leaves and the `ValueRef` each non-null one holds, by cell (O9, R3, R5). Reserved
    /// at construction, one entry per reference leaf.
    references: ReferenceTable,
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

    /// The region's reference table.
    pub(crate) fn references(&self) -> &ReferenceTable {
        &self.references
    }

    /// The `ValueRef` a non-null reference leaf at `cell` holds, or `None` for a null leaf.
    pub(crate) fn reference(&self, cell: u32) -> Option<&ValueRef> {
        self.references.get(cell)
    }

    /// The persistent cells, the text buffers and the reference table, mutably at once (C1-9c).
    pub(crate) fn split_mut(&mut self) -> (&mut CellStore, &mut TextStore, &mut ReferenceTable) {
        (&mut self.persistent, &mut self.text, &mut self.references)
    }

    /// Write every declared leaf's initial value into its cells (R2): the declaration's constant when
    /// it has one, else the default of its type under `profile`, the date/time profile of the engine
    /// that constructs the store. A leaf whose value only construction can give (an instance, which the
    /// engine creates) keeps its empty cells.
    fn write_initial_values(
        &mut self,
        prepared: &PreparedModule,
        profile: DateTimeProfile,
        initials: &[Initial],
    ) -> Result<(), RuntimeError> {
        let (persistent, text, references) = self.split_mut();
        let mut defaults = DefaultWriter {
            prepared,
            profile,
            text,
            references,
        };
        for initial in initials {
            let cells = persistent
                .cells_mut(initial.range)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            match initial.constant {
                Some(index) => {
                    let value = prepared
                        .vm
                        .consts
                        .get(index as usize)
                        .ok_or(RuntimeError::InvalidExecutionState)?;
                    write_value_in(
                        cells,
                        initial.type_id,
                        value,
                        prepared,
                        defaults.text,
                        Some(&mut *defaults.references),
                        initial.range.start,
                    )?;
                }
                None => defaults.write(cells, initial.type_id, initial.range.start, 0)?,
            }
        }
        Ok(())
    }
}

/// The bounded value store's cells (specification 34 §6.2): the regions, the activation half and
/// the identities. It is built only through [`CellStorage::construct`]; the engine constructs and
/// charges it when it builds the cell arm (O18; C1-9d2b-1), and switches to it by default in C1-9d3.
#[derive(Debug, Default)]
#[allow(
    dead_code,
    reason = "the engine reads the activation half from C1-9d3; until then the engine's Value half holds the activations"
)]
pub struct CellStorage {
    regions: CellRegions,
    activations: ActivationStore,
    identities: Identities,
}

impl CellStorage {
    /// Build the persistent region of `prepared` for `generation` (specification 34 §6.2; O18) and
    /// write its declared initial values (R2), with the default date/time profile.
    ///
    /// Before anything is reserved, `charge` receives the bytes the region takes: its cells and
    /// reference records as [`CellStore::reservation_bytes`] counts them, its text buffers' bytes, its
    /// reference table's bytes, its `globals` and `roots` range tables and the leaf lists the
    /// construction builds (R6). A refused charge returns its error, and nothing is reserved.
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
        Self::construct_with_profile(prepared, generation, DateTimeProfile::default(), charge)
    }

    /// [`CellStorage::construct`] with the date/time profile of the engine that constructs the store:
    /// the default of a `DATE` or `DATE_AND_TIME` leaf is the profile's epoch (specification 34 §6.2;
    /// R3).
    ///
    /// # Errors
    ///
    /// As [`CellStorage::construct`].
    pub(crate) fn construct_with_profile(
        prepared: &PreparedModule,
        generation: Generation,
        profile: DateTimeProfile,
        charge: impl FnOnce(usize) -> Result<(), RuntimeError>,
    ) -> Result<Self, RuntimeError> {
        let extent = PersistentExtent::of(prepared)?;
        let reference_count =
            u32::try_from(extent.references.len()).map_err(|_| RuntimeError::Overflow)?;
        let cell_bytes = CellStore::reservation_bytes(extent.cells, reference_count)
            .ok_or(RuntimeError::Overflow)?;
        let table_bytes = ReferenceTable::reservation_bytes(extent.references.len())
            .ok_or(RuntimeError::Overflow)?;
        let bookkeeping_bytes = extent.bookkeeping_bytes()?;
        let bytes = cell_bytes
            .checked_add(extent.text_bytes()?)
            .and_then(|bytes| bytes.checked_add(table_bytes))
            .and_then(|bytes| bytes.checked_add(bookkeeping_bytes))
            .ok_or(RuntimeError::Overflow)?;
        charge(bytes)?;
        let mut regions = CellRegions {
            globals: extent.globals,
            roots: extent.roots,
            persistent: CellStore::reserve(extent.cells, reference_count)?,
            text: TextStore::reserve(&extent.texts)?,
            references: ReferenceTable::reserve(extent.references)?,
        };
        regions.write_initial_values(prepared, profile, &extent.initials)?;
        Ok(Self {
            regions,
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

    /// Issue the next instance identity from the one counter that both kinds of instance share
    /// (specification 34 §6.2; R4). The counter only advances, so no identity is issued twice.
    pub(crate) fn issue_instance_id(&mut self) -> Result<InstanceId, RuntimeError> {
        let id = self.identities.next_instance_id;
        self.identities.next_instance_id = id.checked_add(1).ok_or(RuntimeError::Overflow)?;
        Ok(InstanceId(id))
    }
}

/// What the persistent region holds, read from the declarations and the prepared layouts
/// (specification 34 §6.2): the global and root ranges, the cells and reference leaves they need, one
/// text leaf per text occurrence in cell order, and the initial value of each declared leaf.
pub(crate) struct PersistentExtent {
    globals: Vec<Option<CellRange>>,
    roots: Vec<Option<CellRange>>,
    pub(crate) cells: u32,
    /// The cell of each reference leaf, in cell order (O9).
    pub(crate) references: Vec<u32>,
    texts: Vec<TextLeaf>,
    initials: Vec<Initial>,
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
            // A function static is initialized by its function's static action when the resource first
            // starts, and again by its first call after a restart. Until then it is empty, as the `Value`
            // store's `Null` slot is, so it keeps its empty cells.
            if declaration.role != StorageRole::Static {
                leaves.initials.push(Initial {
                    range,
                    type_id,
                    constant: declaration.default_const_idx,
                });
            }
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
        leaves.references.sort_unstable();
        Ok(Self {
            globals,
            roots,
            cells,
            references: leaves.references,
            texts: leaves.texts,
            initials: leaves.initials,
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

    /// The bytes of the tables the region keeps and of the lists construction builds while it walks the
    /// layouts (specification 34 §6.2; R6, O18): one range entry per STORAGE_LAYOUT declaration in the
    /// `globals` table and one per CONSTRUCTION_ROOTS record in the `roots` table, which the region keeps,
    /// and the transient text-leaf, reference-leaf and initial-value lists. They are memory the
    /// construction holds, so the charge counts them with the cells before anything is reserved. Each
    /// entry has the same size on every target, which the assertions below state.
    fn bookkeeping_bytes(&self) -> Result<usize, RuntimeError> {
        let tables = self
            .globals
            .len()
            .checked_add(self.roots.len())
            .and_then(|entries| entries.checked_mul(RANGE_ENTRY_BYTES));
        let lists = self
            .texts
            .len()
            .checked_mul(TEXT_LEAF_BYTES)
            .and_then(|texts| {
                self.references
                    .len()
                    .checked_mul(REFERENCE_LEAF_BYTES)
                    .and_then(|references| texts.checked_add(references))
            })
            .and_then(|leaves| {
                self.initials
                    .len()
                    .checked_mul(INITIAL_VALUE_BYTES)
                    .and_then(|initials| leaves.checked_add(initials))
            });
        tables
            .zip(lists)
            .and_then(|(tables, lists)| tables.checked_add(lists))
            .ok_or(RuntimeError::Overflow)
    }
}

/// The bytes of one entry of the `globals` and `roots` range tables (R6).
const RANGE_ENTRY_BYTES: usize = 12;
/// The bytes of one entry of the transient text-leaf list (R6).
const TEXT_LEAF_BYTES: usize = 8;
/// The bytes of one entry of the transient reference-leaf list (R6): a cell index, a `u32`.
const REFERENCE_LEAF_BYTES: usize = 4;
/// The bytes of one entry of the transient initial-value list (R6).
const INITIAL_VALUE_BYTES: usize = 24;

// The entries have the sizes the specification states on every target, since none holds a pointer.
const _: () = assert!(core::mem::size_of::<Option<CellRange>>() == RANGE_ENTRY_BYTES);
const _: () = assert!(core::mem::size_of::<TextLeaf>() == TEXT_LEAF_BYTES);
const _: () = assert!(core::mem::size_of::<Initial>() == INITIAL_VALUE_BYTES);

/// A declared leaf whose initial value construction writes (R2): its cells, its type and the constant
/// its declaration names, when it names one.
struct Initial {
    range: CellRange,
    type_id: u32,
    constant: Option<u32>,
}

/// The text leaves, the reference leaves and the initial values found while walking the persistent
/// region.
#[derive(Default)]
struct Leaves {
    texts: Vec<TextLeaf>,
    references: Vec<u32>,
    initials: Vec<Initial>,
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
            // A native-state slot has no declared default: its native import gives it a value on first
            // use, and until then it is empty, as the `Value` store's `Null` slot is. It keeps its empty
            // cells, as an instance leaf does.
            if declaration.role != StorageRole::NativeState {
                self.initials.push(Initial {
                    range: CellRange {
                        region: LayoutRegion::Persistent,
                        start: offset_of(base, offset)?,
                        cells,
                    },
                    type_id,
                    constant: declaration.default_const_idx,
                });
            }
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
            TypeData::Reference { .. } => self.references.push(base),
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

/// What a default is written with: the prepared layouts, the engine's date/time profile and the region's
/// text buffers and reference table.
struct DefaultWriter<'a> {
    prepared: &'a PreparedModule,
    profile: DateTimeProfile,
    text: &'a mut TextStore,
    references: &'a mut ReferenceTable,
}

impl DefaultWriter<'_> {
    /// Write the default of the `TYPE_TABLE` type `type_id` into `cells`, the cells of one value of it
    /// that starts at `cell_start` (R2). A structure and an array are written leaf by leaf, so no whole
    /// default value is built. A type whose value only construction can give, an instance, or a type
    /// C1-15 places, an overlapping structure or a union, keeps its cells as they are.
    fn write(
        &mut self,
        cells: &mut [Cell],
        type_id: u32,
        cell_start: u32,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        if depth > BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        let prepared = self.prepared;
        let entry = prepared
            .vm
            .types
            .entries
            .get(type_id as usize)
            .ok_or(RuntimeError::TypeMismatch)?;
        match &entry.data {
            TypeData::Alias { target_type_id } => {
                self.write(cells, *target_type_id, cell_start, depth + 1)
            }
            TypeData::Struct { fields } if entry.kind != TypeKind::OverlappingStruct => {
                let layout = layout_of(prepared, type_id)?;
                for (member, field) in layout.members.iter().zip(fields) {
                    let member_cells = layout_of(prepared, field.type_id)?.cells;
                    self.write(
                        part(cells, member.offset, member_cells)?,
                        field.type_id,
                        offset_of(cell_start, member.offset)?,
                        depth + 1,
                    )?;
                }
                Ok(())
            }
            TypeData::Array { elem_type_id, .. } => {
                let layout = layout_of(prepared, type_id)?;
                let Some(array) = &layout.array else {
                    return Err(RuntimeError::TypeMismatch);
                };
                // An `ARRAY[*]` has no cells of its own, so there is no element to write.
                if array.stride == 0 {
                    return Ok(());
                }
                for element in 0..layout.cells / array.stride {
                    let offset = element
                        .checked_mul(array.stride)
                        .ok_or(RuntimeError::Overflow)?;
                    self.write(
                        part(cells, offset, array.stride)?,
                        *elem_type_id,
                        offset_of(cell_start, offset)?,
                        depth + 1,
                    )?;
                }
                Ok(())
            }
            _ => match default_leaf(prepared, self.profile, entry)? {
                Some(value) => write_value_in(
                    cells,
                    type_id,
                    &value,
                    prepared,
                    self.text,
                    Some(&mut *self.references),
                    cell_start,
                ),
                None => Ok(()),
            },
        }
    }
}

/// The default value of a leaf type: a scalar, a text, an enumeration's first variant, a null
/// reference, or a subrange's lower bound (IEC 61131-3 defaults, as construction gives them). A `DATE`
/// or `DATE_AND_TIME` defaults to the epoch of `profile`. `None` for an instance and for a type the cells
/// place later (C1-15).
fn default_leaf(
    prepared: &PreparedModule,
    profile: DateTimeProfile,
    entry: &TypeEntry,
) -> Result<Option<Value>, RuntimeError> {
    Ok(match &entry.data {
        TypeData::Primitive { prim_id, .. } => Some(scalar::default(*prim_id, profile)?),
        TypeData::Subrange {
            base_type_id,
            lower,
            ..
        } => {
            let (prim_id, max_length) =
                resolved_primitive_shape(prepared.type_table(), *base_type_id, 0)
                    .ok_or(RuntimeError::TypeMismatch)?;
            Some(normalize_primitive_value(
                prim_id,
                max_length,
                Value::LInt(*lower),
            )?)
        }
        TypeData::Enum { variants, .. } => {
            let variant = variants.first().ok_or(RuntimeError::TypeMismatch)?;
            let name = string_of(prepared, variant.name_idx)?;
            Some(Value::Enum(alloc::boxed::Box::new(
                EnumValue::from_canonical_parts(type_name(prepared, entry), name, variant.value),
            )))
        }
        TypeData::Reference { .. } => Some(Value::Reference(None)),
        TypeData::Struct { .. }
        | TypeData::Union { .. }
        | TypeData::Pou { .. }
        | TypeData::Interface { .. }
        | TypeData::Array { .. }
        | TypeData::Alias { .. } => None,
    })
}

/// The cells `offset` to `offset + len` of `cells`, or a refusal when the layout does not fit them.
fn part(cells: &mut [Cell], offset: u32, len: u32) -> Result<&mut [Cell], RuntimeError> {
    let start = usize::try_from(offset).map_err(|_| RuntimeError::Overflow)?;
    let end = start
        .checked_add(usize::try_from(len).map_err(|_| RuntimeError::Overflow)?)
        .ok_or(RuntimeError::Overflow)?;
    cells
        .get_mut(start..end)
        .ok_or(RuntimeError::InvalidExecutionState)
}

/// The `STRING_TABLE` name of a type entry, or the empty name of an anonymous type.
fn type_name(prepared: &PreparedModule, entry: &TypeEntry) -> SmolStr {
    entry
        .name_idx
        .and_then(|index| prepared.strings().get(index as usize))
        .cloned()
        .unwrap_or_default()
}

/// The `STRING_TABLE` record `name_idx` names.
fn string_of(prepared: &PreparedModule, name_idx: u32) -> Result<SmolStr, RuntimeError> {
    prepared
        .strings()
        .get(name_idx as usize)
        .cloned()
        .ok_or(RuntimeError::InvalidExecutionState)
}

/// One type's prepared layout.
fn layout_of(
    prepared: &PreparedModule,
    type_id: u32,
) -> Result<&crate::memory::layout::TypeLayout, RuntimeError> {
    prepared
        .type_layout(type_id)
        .ok_or(RuntimeError::InvalidExecutionState)
}

/// The cell `relative` cells after `base`, or an overflow refusal.
fn offset_of(base: u32, relative: u32) -> Result<u32, RuntimeError> {
    base.checked_add(relative).ok_or(RuntimeError::Overflow)
}

/// The own cells of `pou`'s template: none for a POU without a template, which has no own variables
/// and no parent (O7).
pub(crate) fn own_cells(prepared: &PreparedModule, pou: u32) -> u32 {
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
