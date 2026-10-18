//! The store-access seam the engine reaches variable storage through.
//!
//! `StoreAccess` reads variables and stages typed path writes while storage is
//! borrowed immutably; `StoreMutate` publishes a prepared write or restores a
//! slot with `&mut self`. A read returns an opaque `ValueView`; the only `&`
//! [`Value`] a caller can obtain is
//! [`StoreAccess::borrow`](crate::memory::view::StoreAccess::borrow), the C1-8
//! compatibility read that C1-9 removes, so the rest of the API can switch to
//! the cell store behind the same API (design
//! `~/projects/.artifacts/bounded-c/design.md` lines 361-367; specification 34
//! §6.2, MEM-10). The traits never charge: every `charge_*`, `before_*` and
//! `check_*` call stays on the engine, where it is today.
//!
//! The seam over today's `VariableStorage` (the `Value` tree) is in the child module `values`: each
//! method forwards to the same storage function the engine calls today (C1-8a note §3), so no result,
//! trap, error, charge or allocation changes. The cell arm's side is in `memory::cell_access`.

use super::{Generation, InstanceData, InstanceId, MemoryLocation, PreparedStorageWrite};
use crate::error::RuntimeError;
use crate::memory::cell_access::{CellPatch, CellsView};
use crate::memory::layout::CellRange;
use crate::value::{
    read_string_element, read_value_path_borrowed, single_string_index, string_element_count,
    RefSegment, Value, ValueRef, ValueRefView,
};
use crate::vm::VmTrap;
use smol_str::SmolStr;

/// A variable, or one path inside it, as the store holds it.
///
/// Opaque: only [`StoreAccess`] observes a view, its field is private and, apart
/// from the compatibility read [`StoreAccess::borrow`], no method hands out a
/// `&`[`Value`] into storage.
#[derive(Clone, Copy)]
pub enum ValueView<'a> {
    /// A value of today's `Value` tree, borrowed from storage (the `Values` arm).
    Value(&'a Value),
    /// The cells of one value of type `type_id` at `range`, read through `cells` (the cell arm,
    /// C1-9b). The view carries what it reads through, so it needs no store to step, text or
    /// materialize; it borrows nothing from the `Value` tree, so [`StoreAccess::borrow`] gives
    /// `None` for it.
    Cells {
        /// The layouts and the region the cells are read from.
        cells: CellsView<'a>,
        /// The cell range the value occupies.
        range: CellRange,
        /// The `TYPE_TABLE` type the cells hold.
        type_id: u32,
    },
}

impl<'a> ValueView<'a> {
    /// A view borrowing `value` from storage.
    fn borrowed(value: &'a Value) -> Self {
        Self::Value(value)
    }

    /// Step one compound segment into a child borrowed from the same storage, or
    /// `None` when the child is absent.
    fn member(&self, segment: &RefSegment) -> Option<Self> {
        match *self {
            Self::Value(value) => {
                read_value_path_borrowed(value, core::slice::from_ref(segment)).map(Self::borrowed)
            }
            Self::Cells {
                cells,
                range,
                type_id,
            } => cells
                .member(range, type_id, segment)
                .map(|(range, type_id)| Self::Cells {
                    cells,
                    range,
                    type_id,
                }),
        }
    }

    /// The stored text bytes of a text leaf, borrowed. `None` for any other value. A cell
    /// `WSTRING` has no UTF-8 text, so it gives `None` (specification 34 §6.2, C1-9b).
    pub(crate) fn text(&self) -> Option<&'a str> {
        match *self {
            Self::Value(value) => match value {
                Value::String(text) => Some(text.as_str()),
                Value::WString(text) => Some(text.as_str()),
                _ => None,
            },
            Self::Cells {
                cells,
                range,
                type_id,
            } => cells.string_text(range, type_id),
        }
    }

    /// The `Value` this view borrows from storage, or `None` for a view of cells.
    pub(crate) fn borrowed_value(&self) -> Option<&'a Value> {
        match *self {
            Self::Value(value) => Some(value),
            Self::Cells { .. } => None,
        }
    }

    /// The owned `Value` of this view. A `Value` view clones the leaf the walk resolved; a cell
    /// view reads its cells (`read_value`), which is the only allocating read.
    pub(crate) fn materialize(&self) -> Result<Value, RuntimeError> {
        match *self {
            Self::Value(value) => Ok(value.clone()),
            Self::Cells {
                cells,
                range,
                type_id,
            } => cells.materialize(range, type_id),
        }
    }

    /// The owned `Value` of this view, and whether copying it cloned a payload: `false` for a scalar, a
    /// structure, an instance or `Null`, as `vm::materialize_borrowed_value` reports (C1-9d1).
    pub(crate) fn materialize_flagged(&self) -> Result<(Value, bool), RuntimeError> {
        match *self {
            Self::Value(value) => Ok(crate::vm::materialize_borrowed_value(value)),
            Self::Cells { .. } => {
                let value = self.materialize()?;
                let cloned = !matches!(
                    value,
                    Value::Bool(_)
                        | Value::SInt(_)
                        | Value::Int(_)
                        | Value::DInt(_)
                        | Value::LInt(_)
                        | Value::USInt(_)
                        | Value::UInt(_)
                        | Value::UDInt(_)
                        | Value::ULInt(_)
                        | Value::Real(_)
                        | Value::LReal(_)
                        | Value::Byte(_)
                        | Value::Word(_)
                        | Value::DWord(_)
                        | Value::LWord(_)
                        | Value::Time(_)
                        | Value::LTime(_)
                        | Value::Date(_)
                        | Value::LDate(_)
                        | Value::Tod(_)
                        | Value::LTod(_)
                        | Value::Dt(_)
                        | Value::Ldt(_)
                        | Value::Char(_)
                        | Value::WChar(_)
                        | Value::Struct(_)
                        | Value::Instance(_)
                        | Value::Null
                );
                Ok((value, cloned))
            }
        }
    }

    /// The scalar this view holds, copied without allocation (S, C1-9d1): a scalar or `Null`. `None`
    /// for a value that owns storage (text, array, structure, enumeration, reference, instance), which
    /// the caller materializes or walks.
    pub(crate) fn scalar(&self) -> Result<Option<Value>, RuntimeError> {
        match *self {
            Self::Value(value) => Ok(match value {
                Value::Bool(_)
                | Value::SInt(_)
                | Value::Int(_)
                | Value::DInt(_)
                | Value::LInt(_)
                | Value::USInt(_)
                | Value::UInt(_)
                | Value::UDInt(_)
                | Value::ULInt(_)
                | Value::Real(_)
                | Value::LReal(_)
                | Value::Byte(_)
                | Value::Word(_)
                | Value::DWord(_)
                | Value::LWord(_)
                | Value::Time(_)
                | Value::LTime(_)
                | Value::Date(_)
                | Value::LDate(_)
                | Value::Tod(_)
                | Value::LTod(_)
                | Value::Dt(_)
                | Value::Ldt(_)
                | Value::Char(_)
                | Value::WChar(_)
                | Value::Null => Some(value.clone()),
                Value::String(_)
                | Value::WString(_)
                | Value::Array(_)
                | Value::Struct(_)
                | Value::Enum(_)
                | Value::Reference(_)
                | Value::Instance(_) => None,
            }),
            Self::Cells {
                cells,
                range,
                type_id,
            } => cells.scalar(range, type_id),
        }
    }

    /// The instance this view holds (S, C1-9d1): an instance value, or the instance cell of a function
    /// block or interface. `None` for any other value.
    pub(crate) fn instance(&self) -> Result<Option<InstanceId>, RuntimeError> {
        match *self {
            Self::Value(value) => Ok(match value {
                Value::Instance(instance) => Some(*instance),
                _ => None,
            }),
            Self::Cells {
                cells,
                range,
                type_id,
            } => cells.instance(range, type_id),
        }
    }

    /// The UTF-8 length of the text this view holds (C1-9d1): the bytes of a `STRING`, and the UTF-8
    /// length of a `WSTRING`'s code units (TIME-08 counts UTF-8). `None` for any other value.
    pub(crate) fn text_utf8_len(&self) -> Result<Option<usize>, RuntimeError> {
        match *self {
            Self::Value(value) => Ok(match value {
                Value::String(text) => Some(text.len()),
                Value::WString(text) => Some(text.len()),
                _ => None,
            }),
            Self::Cells {
                cells,
                range,
                type_id,
            } => cells.text_utf8_len(range, type_id),
        }
    }

    /// Whether this view holds a structure (C1-9d1).
    pub(crate) fn is_struct(&self) -> bool {
        match *self {
            Self::Value(value) => matches!(value, Value::Struct(_)),
            Self::Cells { cells, type_id, .. } => cells.is_struct(type_id),
        }
    }

    /// Whether this view holds a structure that declares a member named `name` (C1-9d1).
    pub(crate) fn has_field(&self, name: &str) -> bool {
        match *self {
            Self::Value(value) => match value {
                Value::Struct(structure) => structure.contains_field(name),
                _ => false,
            },
            Self::Cells { cells, type_id, .. } => cells.has_field(type_id, name),
        }
    }

    /// The declared bounds of each dimension of the array this view holds (C1-9d1), or `None` for any
    /// other value.
    pub(crate) fn array_dimensions(&self) -> Option<&'a [(i64, i64)]> {
        match *self {
            Self::Value(value) => match value {
                Value::Array(array) => Some(array.dimensions()),
                _ => None,
            },
            Self::Cells { cells, type_id, .. } => cells.array_dimensions(type_id),
        }
    }

    /// The number of characters (Unicode scalar values) of the text this view holds (C1-9d3a): the count
    /// that `LEN` and a subscript use (DEV-017). A cell `WSTRING` has no borrowed UTF-8 text, so the
    /// cells count its UTF-16 code units' characters in place. `None` for any other value.
    pub(crate) fn text_char_count(&self) -> Result<Option<usize>, RuntimeError> {
        match *self {
            Self::Value(value) => Ok(match value {
                Value::String(text) => Some(string_element_count(text.as_str())),
                Value::WString(text) => Some(string_element_count(text.as_str())),
                _ => None,
            }),
            Self::Cells {
                cells,
                range,
                type_id,
            } => cells.text_char_count(range, type_id),
        }
    }

    /// Whether this view holds a text: `Some(false)` for a `STRING`, `Some(true)` for a `WSTRING`, and
    /// `None` for any other value (C1-9d1).
    pub(crate) fn text_is_wide(&self) -> Option<bool> {
        match *self {
            Self::Value(value) => match value {
                Value::String(_) => Some(false),
                Value::WString(_) => Some(true),
                _ => None,
            },
            Self::Cells { cells, type_id, .. } => cells.text_is_wide(type_id),
        }
    }

    /// The view of the member or element `path` names inside this view (C1-9d1), or `None` when a step
    /// names no value. A `Value` view walks as [`read_value_path_borrowed`] does, so a structure's
    /// physical path is canonical; a text is not walked here (see [`ValueView::text_element`]).
    pub(crate) fn walk(&self, path: &[RefSegment]) -> Option<Self> {
        match *self {
            Self::Value(value) => read_value_path_borrowed(value, path).map(Self::borrowed),
            Self::Cells { .. } => path
                .iter()
                .try_fold(*self, |view, segment| view.member(segment)),
        }
    }

    /// The character that `indices` select in the text this view holds (C1-9d1, T): a `CHAR` of a
    /// `STRING` or a `WCHAR` of a `WSTRING`, as `materialize_value_path` synthesizes it. `None` when the
    /// selection names no character or the view holds no text. A cell `WSTRING` is UTF-16 and has no
    /// UTF-8 text to borrow, so its character is decoded from the code units in place (C1-9d3a).
    pub(crate) fn text_element(&self, indices: &[i64]) -> Result<Option<Value>, RuntimeError> {
        let Some(index) = single_string_index(indices) else {
            return Ok(None);
        };
        match *self {
            Self::Value(value) => Ok(match value {
                Value::String(text) => read_string_element(text.as_str(), index, false).ok(),
                Value::WString(text) => read_string_element(text.as_str(), index, true).ok(),
                _ => None,
            }),
            Self::Cells {
                cells,
                range,
                type_id,
            } => match cells.text_is_wide(type_id) {
                Some(false) => Ok(cells
                    .string_text(range, type_id)
                    .and_then(|text| read_string_element(text, index, false).ok())),
                Some(true) => cells.wide_text_element(range, type_id, index),
                None => Ok(None),
            },
        }
    }

    /// The number of members of the structure, or of elements of the array, that a cell view holds
    /// (C1-9d2a, item (e)). `None` for any other value; a `Value` view is walked as its `Value`.
    pub(crate) fn cell_child_count(&self) -> Result<Option<usize>, RuntimeError> {
        match *self {
            Self::Value(_) => Ok(None),
            Self::Cells { cells, type_id, .. } => cells.child_count(type_id),
        }
    }

    /// The view of member or element `ordinal` of the cell structure or array this view holds
    /// (C1-9d2a, item (e)): a member in declaration order, an element in flat order.
    pub(crate) fn cell_child(&self, ordinal: usize) -> Result<Self, RuntimeError> {
        match *self {
            Self::Value(_) => Err(RuntimeError::InvalidExecutionState),
            Self::Cells {
                cells,
                range,
                type_id,
            } => cells
                .child(range, type_id, ordinal)
                .map(|(range, type_id)| Self::Cells {
                    cells,
                    range,
                    type_id,
                }),
        }
    }

    /// The reference a cell reference leaf holds (C1-9d2b-1), borrowed: `None` for a null reference, for
    /// any other cell and for a `Value` view, which a caller reads as its `Value`.
    pub(crate) fn cell_reference(&self) -> Result<Option<&'a ValueRef>, RuntimeError> {
        match *self {
            Self::Value(_) => Ok(None),
            Self::Cells {
                cells,
                range,
                type_id,
            } => cells.reference_leaf(range, type_id),
        }
    }

    /// Whether this view holds a cell array of primitives, enumerations or subranges (C1-9d2b-1).
    pub(crate) fn cell_array_of_primitives(&self) -> bool {
        match *self {
            Self::Value(_) => false,
            Self::Cells { cells, type_id, .. } => cells.array_of_primitives(type_id),
        }
    }

    /// The instance a function block cell holds, which a walk owns (C1-9d2a, item (e)). An interface
    /// cell holds an instance that it does not own, so it gives `None`, as does any other cell.
    pub(crate) fn owned_cell_instance(&self) -> Result<Option<InstanceId>, RuntimeError> {
        match *self {
            Self::Cells {
                cells,
                range,
                type_id,
            } if cells.is_pou(type_id) => cells.instance(range, type_id),
            _ => Ok(None),
        }
    }
}

/// An opaque staged typed path write (specification 34 MEM-06).
///
/// [`StoreAccess::prepare_write`] returns one while storage is borrowed
/// immutably; [`StoreMutate::commit_write`] publishes it once. Its field stays
/// private so the stores' patches are built only by their own arm.
pub struct PreparedWrite(pub(crate) PreparedWriteKind);

/// What a staged write holds (C1-9c).
pub(crate) enum PreparedWriteKind {
    /// A patch of the `Value` half. `consumed` leading segments of the path were walked by the cell
    /// arm before it reached that half, so the commit takes the path from there.
    Values {
        consumed: usize,
        staged: PreparedStorageWrite,
    },
    /// A checked leaf of the cells: its range, its type and the value it will hold (R4).
    Cells(CellPatch),
}

impl PreparedWrite {
    /// Whether publishing this patch needs the engine's final deadline check. A cell patch copies
    /// one checked leaf, so it needs none.
    pub fn requires_final_deadline_check(&self) -> bool {
        match &self.0 {
            PreparedWriteKind::Values { staged, .. } => staged.requires_final_deadline_check(),
            PreparedWriteKind::Cells(_) => false,
        }
    }

    /// Run the caller's alias preflight over the staged patch before it is published. A cell patch
    /// has no alias to check: its leaf is copied, not shared.
    pub fn validate_aliases(
        &self,
        check: &mut impl FnMut(&Value) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        match &self.0 {
            PreparedWriteKind::Values { staged, .. } => staged.validate_aliases(check),
            PreparedWriteKind::Cells(_) => Ok(()),
        }
    }
}

/// Reads one variable, or one path inside it, from the store behind the engine.
pub trait StoreAccess {
    /// The view of `path` inside the slot at `location`/`offset`.
    fn read_path<'a>(
        &'a self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
    ) -> Result<ValueView<'a>, VmTrap>;

    /// The view of a whole slot, without a path.
    fn read_slot<'a>(
        &'a self,
        location: MemoryLocation,
        offset: usize,
    ) -> Result<ValueView<'a>, VmTrap>;

    /// The view of a reference's slot and path, in this storage's generation.
    fn read_reference<'a>(&'a self, reference: ValueRefView<'a>) -> Result<ValueView<'a>, VmTrap>;

    /// The view of one instance variable selected by its declared name.
    ///
    /// A C1-8b forwarding method: today's storage names an instance's variables by
    /// string in an ordered map, which a computed cell range cannot carry, and a
    /// built-in function-block executor holds no layout to resolve the name to an
    /// offset. C1-9 resolves the name through the instance template's layout at
    /// this boundary and retires the method (C1-8b note §5; specification 34 §6.2,
    /// MEM-10). Callers: `vm::edge`, `stdlib::fbs::instance`, `vm::engine::cycle`,
    /// whose retained capture reads each retained program variable this way, and the
    /// C1-8c `vm::engine::initialization::staging`,
    /// `vm::engine::retained_graph`, `vm::engine::restart` and
    /// `vm::engine::retain_apply`.
    fn read_instance_var<'a>(&'a self, instance: InstanceId, name: &str) -> Option<ValueView<'a>>;

    /// The view of one global variable selected by its declared name.
    ///
    /// A C1-8c forwarding method, the global counterpart of
    /// [`StoreAccess::read_instance_var`]: today's storage names globals by string in
    /// an ordered map, while a cell range is reached by offset and the retained
    /// capture holds a declaration's name rather than its slot. C1-9 resolves the
    /// name through the prepared layout at this boundary and retires the method
    /// (specification 34 §6.2, MEM-10). Callers: `vm::engine::cycle`,
    /// `vm::engine::initialization::staging`, `vm::engine::retained_graph` and
    /// `vm::engine::retain_apply`.
    fn read_global<'a>(&'a self, name: &str) -> Option<ValueView<'a>>;

    /// The declared name of the global at slot `offset`, in declaration order.
    ///
    /// A C1-8c forwarding method, the name counterpart of
    /// [`StoreAccess::read_global`]: the retained graph rebinds a retained reference
    /// to the candidate's global of the same name, and finds that name from the slot
    /// the reference holds. A name is declaration metadata, not a value, so no stored
    /// value leaves the seam here. C1-9 serves the name from the prepared global
    /// declaration (specification 34 §6.2, MEM-10). Callers:
    /// `vm::engine::retained_graph`.
    fn global_name(&self, offset: usize) -> Option<&SmolStr>;

    /// The view of one instance member resolved through the instance's inheritance
    /// chain.
    ///
    /// A C1-8c forwarding method: today's storage resolves the member by name
    /// against the instance and then its ancestors, and the path-write charge walk
    /// needs that resolution before it may borrow the member. C1-9 resolves the name
    /// through the instance template's layout chain at this boundary and retires the
    /// method (specification 34 §6.2, MEM-10). Callers: `vm::engine::path_write`.
    fn read_instance_field<'a>(
        &'a self,
        instance: InstanceId,
        name: &str,
    ) -> Result<ValueView<'a>, VmTrap>;

    /// The reference to the global variable named `name`, in this storage's generation.
    ///
    /// A C1-8c forwarding method, the reference counterpart of
    /// [`StoreAccess::read_global`]: the retained graph rebinds a retained reference
    /// to the candidate's global of the same name, and needs the slot the candidate
    /// holds it in. C1-9 resolves the name through the prepared layout at this
    /// boundary and retires the method (specification 34 §6.2, MEM-10). Callers:
    /// `vm::engine::retained_graph`.
    fn ref_for_global(&self, name: &str) -> Option<ValueRef>;

    /// The reference to the instance variable `name` of `instance`, in this storage's
    /// generation.
    ///
    /// A C1-8c forwarding method, the reference counterpart of
    /// [`StoreAccess::read_instance_var`]. C1-9 resolves the name through the instance
    /// template's layout at this boundary and retires the method (specification 34
    /// §6.2, MEM-10). Callers: `vm::engine::retained_graph`.
    fn ref_for_instance(&self, instance: InstanceId, name: &str) -> Option<ValueRef>;

    /// An owned copy of one instance record, taken before an initializer may write it.
    ///
    /// A C1-8c forwarding method for the initializer rollback (specification 34
    /// MEM-09). Like the saved root of [`StoreMutate::restore_slot`], the copy is a
    /// journal: it goes back only through [`StoreMutate::restore_instance`]. C1-9
    /// materializes the instance's cells here. Callers:
    /// `vm::engine::initialization::staging`.
    fn snapshot_instance(&self, instance: InstanceId) -> Option<InstanceData>;

    /// Step into one compound segment without cloning the parent.
    fn member<'a>(
        &'a self,
        view: &ValueView<'a>,
        segment: &RefSegment,
    ) -> Result<ValueView<'a>, VmTrap>;

    /// The text bytes of a text leaf, borrowed, without cloning.
    fn text<'a>(&'a self, view: &ValueView<'a>) -> Option<&'a str>;

    /// Borrow the stored leaf a view names, without copying it.
    ///
    /// This is the C1-8 compatibility read. The engine has no caller left: the retained encoder
    /// reads views since the cell encoder landed (C1-9d2b-2), and every other engine read goes
    /// through the view. A cell view answers `None`; the cycle port forwards the read to hosts.
    fn borrow<'a>(&'a self, view: &ValueView<'a>) -> Option<&'a Value>;

    /// The owned `Value` of a view; the only allocating read.
    fn materialize(&self, view: &ValueView<'_>) -> Result<Value, RuntimeError>;

    /// Stage a typed path write while storage is still borrowed immutably.
    fn prepare_write(
        &self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: &Value,
    ) -> Result<Option<PreparedWrite>, RuntimeError>;

    /// The installation generation of this store. Every reference it returns carries
    /// it, and reference reads and writes resolve only references of this generation
    /// (specification 34 §11.3, MEM-07).
    ///
    /// A C1-9 P3 forwarding method for the shared reference paths, which build references
    /// into the store. Callers: `vm::dispatch_refs`, `vm::call::bindings::targets`.
    fn generation(&self) -> Generation;

    /// The declared type name of `instance`, or `None` when no such instance exists.
    ///
    /// A C1-9 P3 forwarding method for the shared call and reference paths. Callers:
    /// `vm::call`, `vm::reference_attempt`.
    fn instance_type_name(&self, instance: InstanceId) -> Option<&SmolStr>;

    /// The parent of `instance`, which `SUPER` names: `Ok(None)` when it has no parent,
    /// and `NullReference` when no such instance exists (specification 34 §6.2, MEM-10).
    ///
    /// A C1-9 P3 forwarding method for the `SUPER` instructions. Callers: `vm::dispatch`,
    /// `vm::register_ir`.
    fn instance_parent(&self, instance: InstanceId) -> Result<Option<InstanceId>, VmTrap>;

    /// The reference to field `name` of `instance`, searched through its inheritance
    /// chain, in this store's generation.
    ///
    /// A C1-9 P3 forwarding method. Callers: `vm::dispatch`, `vm::call::bindings::targets`.
    fn ref_for_instance_recursive(&self, instance: InstanceId, name: &str) -> Option<ValueRef>;

    /// The reference to field `name` of `instance`: the field the instance declares,
    /// else an inherited one, in this store's generation.
    ///
    /// A C1-9 P3 forwarding method. Callers: `vm::dispatch_refs`, `vm::register_ir`.
    fn resolved_instance_field_ref(&self, instance: InstanceId, name: &str) -> Option<ValueRef>;

    /// The position of field `name` among the fields `instance` declares itself, or
    /// `None` when it declares no such field.
    ///
    /// A C1-9 P3 forwarding method. Callers: `vm::call::bindings::targets`.
    fn declared_instance_field_offset(&self, instance: InstanceId, name: &str) -> Option<usize>;

    /// The view of the local variable `name` of the current frame (C1-9d2a, R3). Locals are `Value`-half
    /// state, so every arm answers from the `Value` store. Callers: the name resolution of the host's
    /// expression evaluator (`helper_eval`).
    fn read_local<'a>(&'a self, name: &str) -> Option<ValueView<'a>>;

    /// The reference to the local variable `name` of the current frame, in this store's generation
    /// (C1-9d2a, R3). Locals are `Value`-half state, as [`StoreAccess::read_local`] states.
    fn ref_for_local(&self, name: &str) -> Option<ValueRef>;

    /// The view of the retained variable `name` (C1-9d2a, R3). Retained variables are `Value`-half state,
    /// as [`StoreAccess::read_local`] states.
    fn read_retain<'a>(&'a self, name: &str) -> Option<ValueView<'a>>;
}

/// Publishes a prepared write, or restores a saved slot, with `&mut self`.
pub trait StoreMutate {
    /// Replace one whole slot, normalized for its target.
    fn write_slot(&mut self, location: MemoryLocation, offset: usize, value: Value) -> bool;

    /// Write through a reference in this storage's generation. A reference of
    /// another generation is refused with [`RuntimeError::ReferenceLifetime`]
    /// and nothing is written (specification 34 §11.3, MEM-07).
    fn write_reference(
        &mut self,
        reference: ValueRefView<'_>,
        value: Value,
    ) -> Result<bool, RuntimeError>;

    /// Replace, or insert, one instance variable selected by its declared name,
    /// without normalizing it for a target.
    ///
    /// A C1-8b forwarding method, the write counterpart of
    /// [`StoreAccess::read_instance_var`]: a name may name a member not yet present
    /// in today's value map, so the seam's slot write (which requires an existing
    /// offset) cannot express it. The name is admitted as an owned `SmolStr` so a
    /// caller that already holds one moves it instead of building a second, keeping
    /// today's allocation. C1-9 resolves the name through the instance template's
    /// layout at this boundary and retires the method. Callers: `vm::edge`,
    /// `stdlib::fbs::instance`, `vm::engine::initialization`,
    /// `vm::engine::retained_graph`, `vm::engine::restart` and
    /// `vm::engine::retain_apply`.
    fn write_instance_var(
        &mut self,
        instance: InstanceId,
        name: impl Into<SmolStr>,
        value: Value,
    ) -> bool;

    /// Replace one global variable selected by its declared name, or store a declared global
    /// that owns no cells on the `Value` half (the program root).
    ///
    /// Answers `false`, and stores nothing, for a name no declaration holds: the cell arm admits
    /// declared names only (specification 34 §6.2, MEM-10; O15). Callers:
    /// `vm::engine::initialization::construction`, `vm::engine::retained_graph` and
    /// `vm::engine::retain_apply`.
    fn set_global(&mut self, name: impl Into<SmolStr>, value: Value) -> bool;

    /// Allocate a fresh instance identity of `type_name`.
    ///
    /// A C1-8c forwarding method, the structural counterpart of
    /// [`StoreMutate::write_instance_var`]: an identity is issued by the store today,
    /// and C1-9 issues it against the instance region (specification 34 §6.2,
    /// MEM-10). Callers: `vm::engine::initialization::construction`.
    fn try_create_instance(
        &mut self,
        type_name: impl Into<SmolStr>,
    ) -> Result<InstanceId, RuntimeError>;

    /// Release one instance and everything that named it. Returns whether the instance
    /// existed.
    ///
    /// A C1-8c forwarding method: the rollback of a reserve that failed after the
    /// identity was issued. The released record is dropped inside the store, so no
    /// value tree leaves the seam. C1-9 releases the instance's region (specification
    /// 34 §6.2, MEM-10). Callers: `vm::engine::initialization::construction`.
    fn remove_instance(&mut self, id: InstanceId) -> bool;

    /// Release every instance `keep` refuses, in one pass, and return how many were
    /// released.
    ///
    /// A C1-8c forwarding method: the retirement of an owner's instances (TIME-08,
    /// specification 34 MEM-09). C1-9 releases the region of each refused instance.
    /// Callers: `vm::engine::initialization::construction`.
    fn retain_instances(&mut self, keep: impl FnMut(InstanceId) -> bool) -> usize;

    /// Put a snapshot of an instance record back over the instance's current record,
    /// without normalizing any value. An absent instance is left unchanged.
    ///
    /// A C1-8c forwarding method, the write counterpart of
    /// [`StoreAccess::snapshot_instance`], used only by the initializer rollback
    /// (specification 34 MEM-09). C1-9 restores the instance's cells. Callers:
    /// `vm::engine::initialization::staging`.
    fn restore_instance(&mut self, instance: InstanceId, data: InstanceData);

    /// Publish the prepared patch once, applying the receiving text capacity.
    fn commit_write(
        &mut self,
        path: &[RefSegment],
        value: Value,
        prepared: PreparedWrite,
        capacity: usize,
    ) -> bool;

    /// Put a saved root back without normalization (journals only).
    fn restore_slot(&mut self, location: MemoryLocation, offset: usize, value: Value);
}

/// The value a reference names in `store`, copied (C1-9d2a, R3): the answer that
/// `VariableStorage::materialize_by_ref` gives, read through the seam, so that the host's expression
/// evaluator reads a cell store too. A reference of another generation names nothing. A STRING or
/// WSTRING element has no stored `Value`, so it is the character its parent text holds.
pub fn materialize_ref_value<S: StoreAccess + ?Sized>(
    store: &S,
    reference: ValueRefView<'_>,
) -> Option<Value> {
    if reference.generation != store.generation() {
        return None;
    }
    if let Ok(view) = store.read_reference(reference) {
        return store.materialize(&view).ok();
    }
    let (last, prefix) = reference.path.split_last()?;
    let parent = store
        .read_path(reference.location, reference.offset, prefix)
        .ok()?;
    crate::vm::dispatch_refs::materialize_segment(&parent, last)
        .ok()
        .flatten()
}

mod values;

#[cfg(all(test, feature = "std"))]
mod tests;
