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
//! This slice implements the seam over today's `VariableStorage` (the `Value`
//! tree). Each method forwards to the same storage function the engine calls today
//! (C1-8a note §3), so no result, trap, error, charge or allocation changes.

use super::{MemoryLocation, PreparedStorageWrite, VariableStorage};
use crate::error::RuntimeError;
use crate::value::{read_value_path_borrowed, RefSegment, Value, ValueRefView};
use crate::vm::VmTrap;

/// A variable, or one path inside it, as the store holds it.
///
/// Opaque: only [`StoreAccess`] observes a view, its field is private and, apart
/// from the compatibility read [`StoreAccess::borrow`], no method hands out a
/// `&`[`Value`] into storage.
pub struct ValueView<'a> {
    value: &'a Value,
}

impl<'a> ValueView<'a> {
    /// A view borrowing `value` from storage.
    fn borrowed(value: &'a Value) -> Self {
        Self { value }
    }

    /// Step one compound segment into a child borrowed from the same storage, or
    /// `None` when the child is absent.
    fn member(&self, segment: &RefSegment) -> Option<Self> {
        read_value_path_borrowed(self.value, core::slice::from_ref(segment)).map(Self::borrowed)
    }

    /// The stored text bytes of a text leaf, borrowed. `None` for any other value.
    fn text(&self) -> Option<&'a str> {
        match self.value {
            Value::String(text) => Some(text.as_str()),
            Value::WString(text) => Some(text.as_str()),
            _ => None,
        }
    }

    /// The owned `Value` of this view. The view already holds the resolved leaf,
    /// so this is the clone `materialize_by_ref_parts`/`materialize_value_path`
    /// end their walk with.
    fn materialize(&self) -> Value {
        self.value.clone()
    }
}

/// An opaque staged typed path write (specification 34 MEM-06).
///
/// [`StoreAccess::prepare_write`] returns one while storage is borrowed
/// immutably; [`StoreMutate::commit_write`] publishes it once. Its field stays
/// private so C1-9 can replace today's `PreparedStorageWrite` with a
/// cell-based patch.
pub struct PreparedWrite(PreparedStorageWrite);

impl PreparedWrite {
    /// Whether publishing this patch needs the engine's final deadline check,
    /// forwarding to `PreparedStorageWrite::requires_final_deadline_check`.
    pub fn requires_final_deadline_check(&self) -> bool {
        self.0.requires_final_deadline_check()
    }

    /// Run the caller's alias preflight over the staged patch before it is
    /// published, forwarding to `PreparedStorageWrite::validate_aliases`.
    pub fn validate_aliases(
        &self,
        check: &mut impl FnMut(&Value) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        self.0.validate_aliases(check)
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
    /// This is the C1-8 compatibility read: it hands out a `&`[`Value`] into
    /// today's `Value`-tree storage, which the C1-9 cell store cannot serve. C1-9
    /// removes it by moving its three callers to view-based reads and charges:
    /// `read_storage_path` and `read_storage_slot` in `vm::dispatch_refs`, and
    /// `charge_storage_path_write` in `vm::engine::path_write` (C1-8a note §4). The
    /// engine reads the stored value through this for its copy charge and step-into
    /// decisions while today's storage is a `Value` tree (C1-8a).
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

impl StoreAccess for VariableStorage {
    #[inline]
    fn read_path<'a>(
        &'a self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
    ) -> Result<ValueView<'a>, VmTrap> {
        let value = if path.is_empty() {
            self.read_direct_slot_by_location(location, offset)
        } else {
            self.read_by_ref_parts(location, offset, path)
        };
        value.map(ValueView::borrowed).ok_or(VmTrap::NullReference)
    }

    #[inline]
    fn read_slot<'a>(
        &'a self,
        location: MemoryLocation,
        offset: usize,
    ) -> Result<ValueView<'a>, VmTrap> {
        self.read_direct_slot_by_location(location, offset)
            .map(ValueView::borrowed)
            .ok_or(VmTrap::NullReference)
    }

    #[inline]
    fn read_reference<'a>(&'a self, reference: ValueRefView<'a>) -> Result<ValueView<'a>, VmTrap> {
        if reference.generation != self.generation() {
            return Err(VmTrap::NullReference);
        }
        self.read_by_ref_parts(reference.location, reference.offset, reference.path)
            .map(ValueView::borrowed)
            .ok_or(VmTrap::NullReference)
    }

    #[inline]
    fn member<'a>(
        &'a self,
        view: &ValueView<'a>,
        segment: &RefSegment,
    ) -> Result<ValueView<'a>, VmTrap> {
        view.member(segment).ok_or(VmTrap::NullReference)
    }

    #[inline]
    fn text<'a>(&'a self, view: &ValueView<'a>) -> Option<&'a str> {
        view.text()
    }

    #[inline]
    fn borrow<'a>(&'a self, view: &ValueView<'a>) -> Option<&'a Value> {
        Some(view.value)
    }

    #[inline]
    fn materialize(&self, view: &ValueView<'_>) -> Result<Value, RuntimeError> {
        Ok(view.materialize())
    }

    #[inline]
    fn prepare_write(
        &self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: &Value,
    ) -> Result<Option<PreparedWrite>, RuntimeError> {
        self.prepare_by_ref_parts_write(location, offset, path, value)
            .map(|prepared| prepared.map(PreparedWrite))
    }
}

impl StoreMutate for VariableStorage {
    #[inline]
    fn write_slot(&mut self, location: MemoryLocation, offset: usize, value: Value) -> bool {
        self.write_direct_slot_by_location(location, offset, value)
    }

    #[inline]
    fn write_reference(
        &mut self,
        reference: ValueRefView<'_>,
        value: Value,
    ) -> Result<bool, RuntimeError> {
        if reference.generation != self.generation() {
            return Err(RuntimeError::ReferenceLifetime);
        }
        self.write_by_ref_parts_checked(reference.location, reference.offset, reference.path, value)
    }

    #[inline]
    fn commit_write(
        &mut self,
        path: &[RefSegment],
        value: Value,
        prepared: PreparedWrite,
        capacity: usize,
    ) -> bool {
        self.commit_prepared_write(path, value, prepared.0, capacity)
    }

    #[inline]
    fn restore_slot(&mut self, location: MemoryLocation, offset: usize, value: Value) {
        self.restore_direct_slot(location, offset, value);
    }
}

#[cfg(all(test, feature = "std"))]
mod tests;
