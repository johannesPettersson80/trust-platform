use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::memory::layout::CELL_BYTES;
use crate::memory::view::{StoreAccess, StoreMutate, ValueView};
use crate::memory::MemoryLocation;
use crate::value::{string_element_position_in, RefSegment, Value, ValueRef, ValueRefView};

use super::context::ReferenceContext;
use super::errors::VmTrap;
use super::frames::{FrameStack, VmFrame};
use super::module::{invalid_bytecode, VmModule, VmRef};
use super::type_policy::vm_type_for_path;
use super::{Operand, Place, PlaceDestination};

/// Read a static reference after policy and clone-budget checks.
#[cfg(any(feature = "hir", feature = "register-ir", test))]
pub fn load_ref(
    runtime: &impl ReferenceContext,
    module: &VmModule,
    frames: &FrameStack,
    ref_idx: u32,
) -> Result<Value, VmTrap> {
    value_operand(load_static(runtime, module, frames, ref_idx, false)?)
}

/// Read a static reference as an operand (C1-10c): a compound that a store may copy as bytes is
/// left where it lies, as its place, and charged as the copy that loading it is (TIME-08); any
/// other value is read as [`load_ref`] reads it.
pub(crate) fn load_ref_operand(
    runtime: &impl ReferenceContext,
    module: &VmModule,
    frames: &FrameStack,
    ref_idx: u32,
) -> Result<Operand, VmTrap> {
    load_static(runtime, module, frames, ref_idx, true)
}

fn load_static(
    runtime: &impl ReferenceContext,
    module: &VmModule,
    frames: &FrameStack,
    ref_idx: u32,
    places: bool,
) -> Result<Operand, VmTrap> {
    match peek_ref_view(runtime, module, frames, ref_idx) {
        Ok(view) => return view_operand(runtime, &view, places),
        Err(VmTrap::NullReference) => {}
        Err(error) => return Err(error),
    }
    // A string element is materialized as CHAR/WCHAR, so it has no borrowed Value.
    let reference = load_ref_view(runtime, module, frames, ref_idx)?;
    dynamic_load_view(runtime, frames, reference).map(Operand::Value)
}

/// The operand a load of `view` pushes: its place, when `places` allows one and a store may copy
/// it (C1-10c), or the value it shows. Either is charged as one copy of the value: a copy charges
/// what it copies, so the owned value and the stored view have the same charge (TIME-08).
fn view_operand(
    runtime: &impl ReferenceContext,
    view: &ValueView<'_>,
    places: bool,
) -> Result<Operand, VmTrap> {
    if places && let Some(place) = runtime.place_of(view) {
        runtime.before_view_clone(view).map_err(VmTrap::Runtime)?;
        return Ok(Operand::Place(place));
    }
    let value = materialize_in_buffers(runtime, view)?;
    runtime
        .before_value_clone(&value)
        .map_err(VmTrap::Runtime)?;
    Ok(Operand::Value(value))
}

/// The value of an operand read with places turned off.
fn value_operand(operand: Operand) -> Result<Value, VmTrap> {
    match operand {
        Operand::Value(value) => Ok(value),
        Operand::Place(_) => Err(VmTrap::Runtime(RuntimeError::InvalidExecutionState)),
    }
}

/// The value the compound `place` stands for, read from the flat block it lies in (C1-10c). The
/// load that pushed the place charged its copy, so this read charges nothing.
pub(crate) fn materialize_place(
    runtime: &impl ReferenceContext,
    place: Place,
) -> Result<Value, RuntimeError> {
    runtime
        .place_view(place)
        .ok_or(RuntimeError::InvalidExecutionState)?
        .materialize()
}

/// The view of a static reference after read-policy checks (C1-9d1): a stored leaf, or a local of the
/// current frame, which is a `Value` of the `Values` half.
pub fn peek_ref_view<'a>(
    runtime: &'a impl ReferenceContext,
    module: &'a VmModule,
    frames: &'a FrameStack,
    ref_idx: u32,
) -> Result<ValueView<'a>, VmTrap> {
    let address = load_ref_view(runtime, module, frames, ref_idx)?;
    runtime
        .check_reference_read(address)
        .map_err(VmTrap::Runtime)?;
    let reference = module
        .refs
        .get(ref_idx as usize)
        .ok_or_else(|| VmTrap::InvalidRefIndex(ref_idx))?;
    let storage = runtime.storage();

    match reference {
        VmRef::InitializerResult {
            initializer_id,
            path,
        } => {
            let reference = runtime.initializer_reference(*initializer_id, path)?;
            storage.read_path(reference.location, reference.offset, path)
        }

        VmRef::Global { offset, path } if path.is_empty() => {
            storage.read_slot(MemoryLocation::Global, *offset)
        }
        VmRef::Global { offset, path } => storage.read_path(MemoryLocation::Global, *offset, path),
        VmRef::Local { offset, path, .. } => {
            let frame = frames.current().ok_or_else(|| VmTrap::CallStackUnderflow)?;
            let slot = if path.is_empty() {
                frame.local_slot_index(ref_idx)?
            } else {
                *offset
            };
            // A slot on the activation stack is read where it lies (C1-11).
            if runtime.frame_slot_in_block(frame, slot) {
                return storage.read_path(
                    MemoryLocation::Local(frame.reference_frame_id()),
                    slot,
                    path,
                );
            }
            if path.is_empty() {
                frame.locals.get(slot).map(ValueView::Value).ok_or_else(|| {
                    VmTrap::InvalidLocalRef {
                        ref_index: ref_idx,
                        start: frame.local_ref_start,
                        count: frame.local_ref_count,
                    }
                })
            } else {
                peek_vm_local_ref(frame, slot, path)
            }
        }
        _ => storage.read_path(address.location, address.offset, address.path),
    }
}

/// Resolve a static descriptor to a runtime reference value.
pub fn load_ref_addr(
    runtime: &impl ReferenceContext,
    module: &VmModule,
    frames: &FrameStack,
    ref_idx: u32,
) -> Result<ValueRef, VmTrap> {
    let reference = load_ref_view(runtime, module, frames, ref_idx)?;
    runtime
        .before_reference_path_clone(reference)
        .map_err(VmTrap::Runtime)?;
    owned_reference(runtime, reference)
}

/// An owned copy of `reference` whose path and index segments are in the buffers `runtime` keeps
/// (spec 34 ARCH-05), so copying a static path allocates nothing after the first scan.
fn owned_reference(
    runtime: &impl ReferenceContext,
    reference: ValueRefView<'_>,
) -> Result<ValueRef, VmTrap> {
    // A reference to a whole variable has no path, and an empty path needs no buffer.
    let mut path = if reference.path.is_empty() {
        crate::value::RefPath::new()
    } else {
        runtime.take_reference_path()
    };
    path.try_reserve_exact(reference.path.len())
        .map_err(|_| RuntimeError::Overflow)?;
    for segment in reference.path {
        path.push(match segment {
            RefSegment::Field(name) => RefSegment::Field(name.clone()),
            RefSegment::Index(indices) => {
                let mut copy = runtime.take_reference_indices();
                copy.try_reserve_exact(indices.len())
                    .map_err(|_| RuntimeError::Overflow)?;
                copy.extend_from_slice(indices);
                RefSegment::Index(copy)
            }
        });
    }
    Ok(ValueRef {
        location: reference.location,
        offset: u32::try_from(reference.offset).map_err(|_| RuntimeError::Overflow)?,
        generation: reference.generation,
        path,
    })
}

/// The owned value of `view`. A reference is copied into the buffers `runtime` keeps, so loading
/// a reference variable allocates nothing once the runtime keeps buffers (spec 34 ARCH-05); any
/// other value is materialized as the view gives it.
fn materialize_in_buffers(
    runtime: &impl ReferenceContext,
    view: &ValueView<'_>,
) -> Result<Value, VmTrap> {
    let borrowed = match view.borrowed_value() {
        Some(Value::Reference(Some(reference))) => Some(reference),
        Some(_) => None,
        None => view.cell_reference().map_err(VmTrap::Runtime)?,
    };
    match borrowed {
        Some(reference) => Ok(Value::Reference(Some(owned_reference(
            runtime,
            reference.as_view(),
        )?))),
        None => view.materialize().map_err(VmTrap::Runtime),
    }
}

/// A one-subscript index segment in an index buffer `runtime` keeps (spec 34 ARCH-05).
fn index_segment(runtime: &impl ReferenceContext, index: i64) -> RefSegment {
    let mut indices = runtime.take_reference_indices();
    indices.push(index);
    RefSegment::Index(indices)
}

fn load_ref_view<'a>(
    runtime: &impl ReferenceContext,
    module: &'a VmModule,
    frames: &FrameStack,
    ref_idx: u32,
) -> Result<crate::value::ValueRefView<'a>, VmTrap> {
    let reference = module
        .refs
        .get(ref_idx as usize)
        .ok_or_else(|| VmTrap::InvalidRefIndex(ref_idx))?;
    let (location, offset, path) = match reference {
        VmRef::InitializerResult {
            initializer_id,
            path,
        } => return runtime.initializer_reference(*initializer_id, path),
        VmRef::Global { offset, path } => (MemoryLocation::Global, *offset, path.as_slice()),
        VmRef::Local { offset, path, .. } => (
            MemoryLocation::Local(
                frames
                    .current()
                    .ok_or_else(|| VmTrap::CallStackUnderflow)?
                    .reference_frame_id(),
            ),
            *offset,
            path.as_slice(),
        ),
        _ => runtime_access_target(
            runtime,
            reference,
            frames.current().ok_or_else(|| VmTrap::CallStackUnderflow)?,
        )?,
    };
    // A reference the program takes belongs to the installed generation.
    Ok(crate::value::ValueRefView {
        location,
        offset,
        generation: runtime.storage().generation(),
        path,
    })
}

/// Write a static reference through type, lifetime and storage-policy gates.
pub fn store_ref(
    runtime: &mut impl ReferenceContext,
    module: &VmModule,
    frames: &mut FrameStack,
    ref_idx: u32,
    value: Value,
) -> Result<(), VmTrap> {
    let reference = module
        .refs
        .get(ref_idx as usize)
        .ok_or_else(|| VmTrap::InvalidRefIndex(ref_idx))?;
    let value = match module.ref_type(ref_idx) {
        Some(ty) => runtime
            .normalize_assignment(module, ty, value)
            .map_err(VmTrap::Runtime)?,
        None => value,
    };
    let address = load_ref_view(runtime, module, frames, ref_idx)?;
    runtime
        .check_reference_write(address, &value)
        .map_err(VmTrap::Runtime)?;
    let capacity = runtime.reference_string_capacity(address);

    match reference {
        VmRef::InitializerResult {
            initializer_id,
            path,
        } => {
            let reference = runtime.initializer_reference(*initializer_id, path)?;
            if runtime
                .write_typed_storage(
                    reference.location,
                    reference.offset,
                    reference.path,
                    value,
                    capacity,
                )
                .map_err(VmTrap::Runtime)?
            {
                Ok(())
            } else {
                Err(VmTrap::NullReference)
            }
        }

        VmRef::Global { offset, path } if path.is_empty() => {
            if runtime
                .storage_mut()
                .write_slot(MemoryLocation::Global, *offset, value)
            {
                Ok(())
            } else {
                Err(VmTrap::NullReference)
            }
        }
        VmRef::Global { offset, path } => {
            if runtime
                .write_typed_storage(MemoryLocation::Global, *offset, path, value, capacity)
                .map_err(VmTrap::Runtime)?
            {
                Ok(())
            } else {
                Err(VmTrap::NullReference)
            }
        }
        VmRef::Local { offset, path, .. } => {
            let frame = frames
                .current_mut()
                .ok_or_else(|| VmTrap::CallStackUnderflow)?;
            let slot = if path.is_empty() {
                frame.local_slot_index(ref_idx)?
            } else {
                *offset
            };
            // A slot on the activation stack is written where it lies (C1-11).
            if runtime.frame_slot_in_block(frame, slot) {
                let location = MemoryLocation::Local(frame.reference_frame_id());
                return if runtime
                    .write_typed_storage(location, slot, path, value, capacity)
                    .map_err(VmTrap::Runtime)?
                {
                    Ok(())
                } else {
                    Err(VmTrap::NullReference)
                };
            }
            if path.is_empty() {
                frame.store_local(ref_idx, value)
            } else {
                write_vm_local_ref(frame, slot, path, value, runtime, capacity)
            }
        }
        _ => {
            if runtime
                .write_typed_storage(
                    address.location,
                    address.offset,
                    address.path,
                    value,
                    capacity,
                )
                .map_err(VmTrap::Runtime)?
            {
                Ok(())
            } else {
                Err(VmTrap::NullReference)
            }
        }
    }
}

/// Store the compound `place` stands for through a static reference (C1-10c). A destination of the
/// place's type in the flat block takes a copy of its bytes, checked and charged as [`store_ref`]
/// checks and charges the value; a local, an initializer's result or any other destination is given
/// the value, read from the place.
pub(crate) fn store_ref_place(
    runtime: &mut impl ReferenceContext,
    module: &VmModule,
    frames: &mut FrameStack,
    ref_idx: u32,
    place: Place,
) -> Result<(), VmTrap> {
    let reference = module
        .refs
        .get(ref_idx as usize)
        .ok_or_else(|| VmTrap::InvalidRefIndex(ref_idx))?;
    // Locals and initializer results are in the `Value` half until the activation block holds them
    // (C1-11, C1-13).
    if !matches!(
        reference,
        VmRef::Local { .. } | VmRef::InitializerResult { .. }
    ) {
        let address = load_ref_view(runtime, module, frames, ref_idx)?;
        if let Some(destination) =
            runtime.place_destination(address, place, module.ref_type(ref_idx))
        {
            return copy_place_to(runtime, address, place, destination);
        }
    }
    let value = materialize_place(runtime, place).map_err(VmTrap::Runtime)?;
    store_ref(runtime, module, frames, ref_idx, value)
}

/// Store the compound `place` stands for through a dynamic reference (C1-10c), as
/// [`store_ref_place`] stores it through a static one.
pub(crate) fn dynamic_store_place(
    runtime: &mut impl ReferenceContext,
    module: &VmModule,
    frames: &mut FrameStack,
    reference: &ValueRef,
    place: Place,
) -> Result<(), VmTrap> {
    let frame = frames.current().ok_or_else(|| VmTrap::CallStackUnderflow)?;
    if !frame.owns_local_reference(reference) {
        let declared = dynamic_ref_type(runtime, module, frame, reference)?;
        if let Some(destination) = runtime.place_destination(reference.as_view(), place, declared) {
            return copy_place_to(runtime, reference.as_view(), place, destination);
        }
    }
    let value = materialize_place(runtime, place).map_err(VmTrap::Runtime)?;
    dynamic_store_ref(runtime, module, frames, reference, value)
}

/// Check a write of `place` through `reference` as a write of its value is checked, then copy its
/// bytes to `destination`. A value of the destination's type needs no normalization, and the cell
/// write it replaces charges nothing beyond the check (TIME-08).
fn copy_place_to(
    runtime: &mut impl ReferenceContext,
    reference: ValueRefView<'_>,
    place: Place,
    destination: PlaceDestination,
) -> Result<(), VmTrap> {
    runtime
        .check_place_write(reference, place)
        .map_err(VmTrap::Runtime)?;
    if runtime.copy_place(place, destination) {
        Ok(())
    } else {
        Err(VmTrap::NullReference)
    }
}

/// Consume a non-null reference operand or return a VM trap.
pub fn pop_reference(stack: &mut super::stack::OperandStack) -> Result<ValueRef, VmTrap> {
    let value = stack.pop()?;
    match value {
        Value::Reference(Some(reference)) => Ok(reference),
        Value::Reference(None) => Err(VmTrap::NullReference),
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch)),
    }
}

/// Extend an owned reference with a checked field selection.
pub fn dynamic_ref_field(
    runtime: &impl ReferenceContext,
    frames: &FrameStack,
    mut reference: ValueRef,
    field: SmolStr,
) -> Result<ValueRef, VmTrap> {
    let target = peek_dynamic_ref(runtime, frames, &reference)?;
    if target.is_struct() {
        if !target.has_field(field.as_str()) {
            return Err(VmTrap::Runtime(RuntimeError::UndefinedField(field)));
        }
        reserve_reference_segment(runtime, &mut reference, 0)?;
        reference.path.push(RefSegment::Field(field));
        return Ok(reference);
    }
    match target.instance().map_err(VmTrap::Runtime)? {
        Some(instance_id) => runtime
            .storage()
            .resolved_instance_field_ref(instance_id, field.as_str())
            .ok_or_else(|| VmTrap::Runtime(RuntimeError::UndefinedField(field))),
        None => Err(VmTrap::Runtime(RuntimeError::TypeMismatch)),
    }
}

/// Extend a reference with a borrowed field name.
#[cfg(any(feature = "hir", feature = "register-ir"))]
pub fn dynamic_ref_field_borrowed(
    runtime: &impl ReferenceContext,
    frames: &FrameStack,
    reference: &ValueRef,
    field: SmolStr,
) -> Result<ValueRef, VmTrap> {
    let target = peek_dynamic_ref(runtime, frames, reference)?;
    if target.is_struct() {
        if !target.has_field(field.as_str()) {
            return Err(VmTrap::Runtime(RuntimeError::UndefinedField(field)));
        }
        runtime
            .before_reference_path_clone(reference.as_view())
            .map_err(VmTrap::Runtime)?;
        // TIME-08: the added field segment costs one unit and one cell, as the owned path
        // in `reserve_reference_segment` does.
        runtime
            .before_reference_path_growth(CELL_BYTES, 1)
            .map_err(VmTrap::Runtime)?;
        let capacity = reference
            .path
            .len()
            .checked_add(1)
            .ok_or_else(|| RuntimeError::Overflow)?;
        let mut path = alloc::vec::Vec::new();
        path.try_reserve_exact(capacity)
            .map_err(|_| RuntimeError::Overflow)?;
        path.extend_from_slice(&reference.path);
        let mut next = ValueRef {
            location: reference.location,
            offset: reference.offset,
            generation: reference.generation,
            path,
        };
        next.path.push(RefSegment::Field(field));
        return Ok(next);
    }
    match target.instance().map_err(VmTrap::Runtime)? {
        Some(instance_id) => runtime
            .storage()
            .resolved_instance_field_ref(instance_id, field.as_str())
            .ok_or_else(|| VmTrap::Runtime(RuntimeError::UndefinedField(field))),
        None => Err(VmTrap::Runtime(RuntimeError::TypeMismatch)),
    }
}

/// Consume index operands and append a checked array selection.
pub fn dynamic_ref_index(
    runtime: &impl ReferenceContext,
    frames: &FrameStack,
    mut reference: ValueRef,
    index: i64,
) -> Result<ValueRef, VmTrap> {
    // Support chained indexing for multidimensional arrays by extending a trailing
    // partial index segment (e.g. [i] -> [i, j]) against the base array dimensions.
    if let Some(RefSegment::Index(existing)) = reference.path.last() {
        let base_path = &reference.path[..reference.path.len().saturating_sub(1)];
        if let Some(dims) = peek_dynamic_ref_path(runtime, frames, reference.as_view(), base_path)?
            .array_dimensions()
            && existing.len() < dims.len()
        {
            let (lower, upper) = dims[existing.len()];
            if index < lower || index > upper {
                return Err(VmTrap::Runtime(RuntimeError::IndexOutOfBounds {
                    index,
                    lower,
                    upper,
                }));
            }
            // TIME-08: the added subscript costs one unit and one cell, whatever the capacity
            // of the trailing index vector.
            runtime
                .before_reference_path_growth(CELL_BYTES, 1)
                .map_err(VmTrap::Runtime)?;
            if let Some(RefSegment::Index(indices)) = reference.path.last_mut() {
                indices
                    .try_reserve_exact(1)
                    .map_err(|_| RuntimeError::Overflow)?;
                indices.push(index);
                return Ok(reference);
            }
        }
    }

    let target = peek_dynamic_ref(runtime, frames, &reference)?;
    if let Some(dims) = target.array_dimensions() {
        let Some((lower, upper)) = dims.first().copied() else {
            return Err(VmTrap::Runtime(RuntimeError::TypeMismatch));
        };
        if index < lower || index > upper {
            return Err(VmTrap::Runtime(RuntimeError::IndexOutOfBounds {
                index,
                lower,
                upper,
            }));
        }
        reserve_reference_segment(runtime, &mut reference, 1)?;
        reference.path.push(index_segment(runtime, index));
        return Ok(reference);
    }
    // A STRING or a WSTRING: its length is charged before the element is checked (TIME-08). The
    // characters are counted from the view, so that a cell WSTRING, which has no borrowed UTF-8 text, is
    // checked as a `Value` is (C1-9d3a).
    let Some(length) = target.text_utf8_len().map_err(VmTrap::Runtime)? else {
        return Err(VmTrap::Runtime(RuntimeError::TypeMismatch));
    };
    runtime
        .before_reference_path_growth(0, length)
        .map_err(VmTrap::Runtime)?;
    let count = target
        .text_char_count()
        .map_err(VmTrap::Runtime)?
        .ok_or_else(|| VmTrap::Runtime(RuntimeError::TypeMismatch))?;
    string_element_position_in(count, index).map_err(VmTrap::Runtime)?;
    reserve_reference_segment(runtime, &mut reference, 1)?;
    reference.path.push(index_segment(runtime, index));
    Ok(reference)
}

/// Admit one new owned path segment that carries `subscripts` subscripts, then reserve its
/// slot. Each added segment and each added subscript costs one unit and one cell, whatever the
/// path vector's capacity (spec 34 TIME-08). A path with no room left moves into a buffer the
/// runtime keeps, and its own buffer is given back, so the reservation allocates nothing once the
/// runtime keeps buffers (ARCH-05).
fn reserve_reference_segment(
    runtime: &impl ReferenceContext,
    reference: &mut ValueRef,
    subscripts: usize,
) -> Result<(), VmTrap> {
    let subscript_bytes = subscripts
        .checked_mul(CELL_BYTES)
        .ok_or_else(|| RuntimeError::Overflow)?;
    let bytes = CELL_BYTES
        .checked_add(subscript_bytes)
        .ok_or_else(|| RuntimeError::Overflow)?;
    let work = subscripts
        .checked_add(1)
        .ok_or_else(|| RuntimeError::Overflow)?;
    runtime
        .before_reference_path_growth(bytes, work)
        .map_err(VmTrap::Runtime)?;
    if reference.path.len() == reference.path.capacity() {
        let mut path = runtime.take_reference_path();
        path.append(&mut reference.path);
        runtime.recycle_reference_path(core::mem::replace(&mut reference.path, path));
    }
    reference
        .path
        .try_reserve_exact(1)
        .map_err(|_| RuntimeError::Overflow)?;
    Ok(())
}

/// Borrow a dynamic target after resolving frame and instance ownership.
pub fn peek_dynamic_ref<'a>(
    runtime: &'a impl ReferenceContext,
    frames: &'a FrameStack,
    reference: &ValueRef,
) -> Result<ValueView<'a>, VmTrap> {
    peek_dynamic_ref_path(runtime, frames, reference.as_view(), &reference.path)
}

fn peek_dynamic_ref_path<'a>(
    runtime: &'a impl ReferenceContext,
    frames: &'a FrameStack,
    reference: crate::value::ValueRefView<'_>,
    path: &[RefSegment],
) -> Result<ValueView<'a>, VmTrap> {
    runtime
        .check_reference_read(reference)
        .map_err(VmTrap::Runtime)?;
    if frames.current().is_some_and(|frame| {
        reference.location == MemoryLocation::Local(frame.reference_frame_id())
            && !runtime.frame_slot_in_block(frame, reference.offset)
    }) {
        let frame = frames.current().ok_or_else(|| VmTrap::CallStackUnderflow)?;
        let root = frame
            .locals
            .get(reference.offset)
            .ok_or_else(|| VmTrap::NullReference)?;
        return ValueView::Value(root)
            .walk(path)
            .ok_or_else(|| VmTrap::NullReference);
    }
    runtime
        .storage()
        .read_path(reference.location, reference.offset, path)
}

/// Read a dynamic target with policy and pre-copy charging.
#[cfg(any(feature = "hir", feature = "register-ir", test))]
pub fn dynamic_load_ref(
    runtime: &impl ReferenceContext,
    frames: &FrameStack,
    reference: &ValueRef,
) -> Result<Value, VmTrap> {
    dynamic_load_view(runtime, frames, reference.as_view())
}

/// Read a dynamic target as an operand (C1-10c): a compound that a store may copy as bytes as its
/// place, any other value as [`dynamic_load_ref`] reads it.
pub(crate) fn dynamic_load_operand(
    runtime: &impl ReferenceContext,
    frames: &FrameStack,
    reference: &ValueRef,
) -> Result<Operand, VmTrap> {
    match peek_dynamic_ref_path(runtime, frames, reference.as_view(), &reference.path) {
        Ok(view) => view_operand(runtime, &view, true),
        Err(VmTrap::NullReference) => {
            dynamic_load_view(runtime, frames, reference.as_view()).map(Operand::Value)
        }
        Err(error) => Err(error),
    }
}

fn dynamic_load_view(
    runtime: &impl ReferenceContext,
    frames: &FrameStack,
    reference: crate::value::ValueRefView<'_>,
) -> Result<Value, VmTrap> {
    match peek_dynamic_ref_path(runtime, frames, reference, reference.path) {
        Ok(view) => value_operand(view_operand(runtime, &view, false)?),
        Err(VmTrap::NullReference) => {
            let (last, prefix) = reference
                .path
                .split_last()
                .ok_or_else(|| VmTrap::NullReference)?;
            let parent = peek_dynamic_ref_path(runtime, frames, reference, prefix)?;
            // The parent is charged as a whole copy, from its view (C1-9d2a, item (a)).
            runtime
                .before_view_clone(&parent)
                .map_err(VmTrap::Runtime)?;
            charge_materialize_path_work(runtime, &parent, core::slice::from_ref(last))
                .map_err(VmTrap::Runtime)?;
            materialize_segment(&parent, last)
                .map_err(VmTrap::Runtime)?
                .ok_or_else(|| VmTrap::NullReference)
        }
        Err(error) => Err(error),
    }
}

/// The value one path segment selects inside a view (C1-9d1): a member or an element, or the
/// character a text selects. `None` when the segment names nothing.
pub(crate) fn materialize_segment(
    view: &ValueView<'_>,
    segment: &RefSegment,
) -> Result<Option<Value>, RuntimeError> {
    match (view.walk(core::slice::from_ref(segment)), segment) {
        (Some(child), _) => child.materialize().map(Some),
        (None, RefSegment::Index(indices)) => view.text_element(indices),
        (None, RefSegment::Field(_)) => Ok(None),
    }
}

/// Reading one character scans the text up to its index: TEXT work (spec 34 TIME-08).
pub(crate) fn charge_materialize_path_work(
    runtime: &impl ReferenceContext,
    root: &ValueView<'_>,
    path: &[RefSegment],
) -> Result<(), RuntimeError> {
    let Some((RefSegment::Index(_), prefix)) = path.split_last() else {
        return Ok(());
    };
    let Some(value) = root.walk(prefix) else {
        return Ok(());
    };
    let Some(bytes) = value.text_utf8_len()? else {
        return Ok(());
    };
    let work = bytes.checked_mul(2).ok_or_else(|| RuntimeError::Overflow)?;
    runtime.charge_text_work(work)
}

/// Write a dynamic target with policy, type and allocation checks.
pub fn dynamic_store_ref(
    runtime: &mut impl ReferenceContext,
    module: &VmModule,
    frames: &mut FrameStack,
    reference: &ValueRef,
    value: Value,
) -> Result<(), VmTrap> {
    runtime
        .check_reference_write(reference.as_view(), &value)
        .map_err(VmTrap::Runtime)?;
    let capacity = runtime.reference_string_capacity(reference.as_view());
    let frame = frames.current().ok_or_else(|| VmTrap::CallStackUnderflow)?;
    let value = normalize_dynamic_store_value(runtime, module, frame, reference, value)?;
    if frames.current().is_some_and(|frame| {
        frame.owns_local_reference(reference)
            && !runtime.frame_slot_in_block(frame, reference.offset as usize)
    }) {
        let frame = frames
            .current_mut()
            .ok_or_else(|| VmTrap::CallStackUnderflow)?;
        return write_vm_local_ref(
            frame,
            reference.offset as usize,
            &reference.path,
            value,
            runtime,
            capacity,
        );
    }
    if runtime
        .write_typed_storage(
            reference.location,
            reference.offset as usize,
            &reference.path,
            value,
            capacity,
        )
        .map_err(VmTrap::Runtime)?
    {
        Ok(())
    } else {
        Err(VmTrap::NullReference)
    }
}

fn peek_vm_local_ref<'a>(
    frame: &'a VmFrame,
    offset: usize,
    path: &[RefSegment],
) -> Result<ValueView<'a>, VmTrap> {
    let root = frame
        .locals
        .get(offset)
        .ok_or_else(|| VmTrap::NullReference)?;
    ValueView::Value(root)
        .walk(path)
        .ok_or_else(|| VmTrap::NullReference)
}

fn write_vm_local_ref(
    frame: &mut VmFrame,
    offset: usize,
    path: &[RefSegment],
    value: Value,
    runtime: &impl ReferenceContext,
    capacity: usize,
) -> Result<(), VmTrap> {
    let root = frame
        .locals
        .get_mut(offset)
        .ok_or_else(|| VmTrap::NullReference)?;
    runtime
        .before_path_write(root, path, &value)
        .map_err(VmTrap::Runtime)?;
    if runtime
        .write_typed_local_checked(root, path, value, capacity)
        .map_err(VmTrap::Runtime)?
    {
        Ok(())
    } else {
        Err(VmTrap::NullReference)
    }
}

fn normalize_dynamic_store_value(
    runtime: &impl ReferenceContext,
    module: &VmModule,
    frame: &VmFrame,
    reference: &ValueRef,
    value: Value,
) -> Result<Value, VmTrap> {
    let Some(type_idx) = dynamic_ref_type(runtime, module, frame, reference)? else {
        return Ok(value);
    };
    runtime
        .normalize_assignment(module, type_idx, value)
        .map_err(VmTrap::Runtime)
}

/// Determine the declared type reached by a dynamic reference.
pub fn dynamic_ref_type(
    runtime: &impl ReferenceContext,
    module: &VmModule,
    frame: &VmFrame,
    reference: &ValueRef,
) -> Result<Option<u32>, VmTrap> {
    if frame.owns_local_reference(reference) {
        if reference.offset >= frame.local_ref_count {
            return Ok(None);
        }
        let offset = reference.offset;
        let Some(base_type) = frame
            .local_ref_start
            .checked_add(offset)
            .and_then(|ref_idx| module.ref_type(ref_idx))
        else {
            return Ok(None);
        };
        return Ok(vm_type_for_path(module, base_type, &reference.path));
    }

    let candidates = runtime
        .dynamic_reference_candidates(module, reference.as_view())
        .map_err(VmTrap::Runtime)?;
    let count = candidates.map_or(module.refs.len(), <[u32]>::len);
    let mut resolved = None;
    for ordinal in 0..count {
        let ref_idx = match candidates {
            Some(ids) => ids[ordinal],
            None => u32::try_from(ordinal).map_err(|_| VmTrap::InvalidRefIndex(u32::MAX))?,
        };
        let candidate = module
            .refs
            .get(ref_idx as usize)
            .ok_or_else(|| VmTrap::InvalidRefIndex(ref_idx))?;
        let path = match (reference.location, candidate) {
            (MemoryLocation::Global, VmRef::Global { offset, path })
            | (MemoryLocation::Instance(_), VmRef::Instance { offset, path, .. })
                if *offset == reference.offset as usize =>
            {
                path
            }
            _ => continue,
        };
        let Some(suffix) = reference.path.as_slice().strip_prefix(path.as_slice()) else {
            continue;
        };
        let Some(base_type_idx) = module.ref_type(ref_idx) else {
            continue;
        };
        let (location, offset, _) = match runtime_access_target(runtime, candidate, frame) {
            Ok(target) => target,
            Err(VmTrap::Runtime(RuntimeError::NullReference)) => continue,
            Err(VmTrap::Runtime(error)) => return Err(VmTrap::Runtime(error)),
            Err(_) => continue,
        };
        if location == reference.location && offset == reference.offset as usize {
            let Some(type_idx) = vm_type_for_path(module, base_type_idx, suffix) else {
                continue;
            };
            if resolved.is_some_and(|existing| existing != type_idx) {
                return Err(VmTrap::Runtime(invalid_bytecode(
                    "conflicting type metadata for runtime reference",
                )));
            }
            resolved = Some(type_idx);
        }
    }
    Ok(resolved)
}

/// Convert an IEC integer index to the common checked signed representation.
pub fn index_to_i64(value: Value) -> Result<i64, VmTrap> {
    match value {
        Value::SInt(v) => Ok(v as i64),
        Value::Int(v) => Ok(v as i64),
        Value::DInt(v) => Ok(v as i64),
        Value::LInt(v) => Ok(v),
        Value::USInt(v) => Ok(v as i64),
        Value::UInt(v) => Ok(v as i64),
        Value::UDInt(v) => Ok(v as i64),
        Value::ULInt(v) => {
            i64::try_from(v).map_err(|_| VmTrap::Runtime(RuntimeError::TypeMismatch))
        }
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch)),
    }
}

fn runtime_access_target<'a>(
    runtime: &impl ReferenceContext,
    reference: &'a VmRef,
    frame: &VmFrame,
) -> Result<(MemoryLocation, usize, &'a [RefSegment]), VmTrap> {
    match reference {
        VmRef::InitializerResult { .. } => {
            Err(VmTrap::UnsupportedRefLocation("initializer-result"))
        }

        VmRef::Global { offset, path } => Ok((MemoryLocation::Global, *offset, path.as_slice())),
        VmRef::Instance {
            owner_instance_id,
            offset,
            path,
        } => {
            let runtime_owner = runtime.resolve_instance_owner(*owner_instance_id, frame)?;
            Ok((
                MemoryLocation::Instance(runtime_owner),
                *offset,
                path.as_slice(),
            ))
        }
        VmRef::Local {
            owner_frame_id,
            offset,
            path,
        } => Err(VmTrap::UnsupportedRefLocation(if path.is_empty() {
            let _ = owner_frame_id;
            let _ = offset;
            "local"
        } else {
            "local-path"
        })),
        VmRef::Retain { offset, path } => {
            let _ = (offset, path);
            Err(VmTrap::UnsupportedRefLocation("retain"))
        }
        VmRef::Io { area, offset, path } => {
            let _ = (area, offset, path);
            Err(VmTrap::UnsupportedRefLocation("io"))
        }
    }
}

#[cfg(test)]
#[path = "dispatch_refs_tests.rs"]
mod dispatch_refs_tests;
