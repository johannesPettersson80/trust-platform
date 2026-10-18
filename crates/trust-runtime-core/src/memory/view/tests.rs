//! Focused behaviour of the store-access seam over today's `Value` storage.
//!
//! Every expected value is a literal from the specification line the test names.
//! These are the assertions `memory/tests.rs` and `engine/path_write.rs` make
//! against the old storage functions, moved onto the API, so a wrong seam fails
//! here rather than two slices later (C1-8a note §6).

use super::{StoreAccess, StoreMutate};
use crate::collections::OrderedMap as IndexMap;
use crate::error::RuntimeError;
use crate::memory::{Generation, MemoryLocation, VariableStorage};
use crate::value::{
    ref_indices_from_iter, ArrayValue, RefSegment, StructValue, Value, MAX_STRING_CAPACITY,
};
use crate::vm::VmTrap;
use smol_str::SmolStr;
use std::sync::Arc;

/// Read one global path through the seam and materialize it.
fn read_global(storage: &VariableStorage, path: &[RefSegment]) -> Value {
    let view = storage
        .read_path(MemoryLocation::Global, 0, path)
        .expect("the path resolves");
    storage.materialize(&view).expect("the leaf materializes")
}

fn axis() -> Value {
    Value::Struct(Arc::new(StructValue::from_untyped_parts(
        SmolStr::new("AXIS_REF"),
        IndexMap::from([
            (SmolStr::new("InternalIndex"), Value::UInt(1)),
            (SmolStr::new("Scale"), Value::UInt(9)),
        ]),
    )))
}

#[test]
fn view_read_of_a_global_field_equals_the_stored_value() {
    // spec 34 §6.2 (MEM-10): a prepared member read returns that member's value.
    let mut storage = VariableStorage::new();
    storage.set_global("AXIS", axis());

    let view = storage
        .read_path(
            MemoryLocation::Global,
            0,
            &[RefSegment::Field(SmolStr::new("Scale"))],
        )
        .expect("Scale resolves");
    // A step that lands one member early (1) or swaps Field/Index fails here.
    assert_eq!(
        storage.materialize(&view).expect("Scale materializes"),
        Value::UInt(9)
    );
}

#[test]
fn view_read_of_an_array_element_uses_the_declared_lower_bound() {
    // spec 02 §3.4: ARRAY[1..3] holds subscripts 1 through 3; subscript 1 is the
    // first element and subscript 0 is outside the declared range.
    let mut storage = VariableStorage::new();
    storage.set_global(
        "GRID",
        Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![Value::Int(11), Value::Int(22), Value::Int(33)],
            vec![(1, 3)],
        ))),
    );

    let first = storage
        .read_path(
            MemoryLocation::Global,
            0,
            &[RefSegment::Index(ref_indices_from_iter([1]))],
        )
        .expect("subscript 1 resolves");
    // An element read that ignores the lower bound returns 22 here.
    assert_eq!(
        storage.materialize(&first).expect("first element"),
        Value::Int(11)
    );

    let last = storage
        .read_path(
            MemoryLocation::Global,
            0,
            &[RefSegment::Index(ref_indices_from_iter([3]))],
        )
        .expect("subscript 3 resolves");
    assert_eq!(
        storage.materialize(&last).expect("last element"),
        Value::Int(33)
    );

    // A read that skips the lower-bound check finds an element at subscript 0 and
    // returns 11 instead of the NullReference trap.
    assert!(matches!(
        storage.read_path(
            MemoryLocation::Global,
            0,
            &[RefSegment::Index(ref_indices_from_iter([0]))],
        ),
        Err(VmTrap::NullReference)
    ));
}

#[test]
fn view_read_of_a_missing_field_is_a_null_reference() {
    // spec 34 MEM-05: a missing target keeps the stable NullReference trap; it
    // must never become a silent default.
    let mut storage = VariableStorage::new();
    storage.set_global("AXIS", axis());

    assert!(matches!(
        storage.read_path(
            MemoryLocation::Global,
            0,
            &[RefSegment::Field(SmolStr::new("Missing"))],
        ),
        Err(VmTrap::NullReference)
    ));
}

#[test]
fn view_text_borrow_has_the_stored_length() {
    // spec 02 §6: 'Ä' (U+00C4) is two UTF-8 bytes, so the stored text "Äb" is
    // three bytes long.
    let mut storage = VariableStorage::new();
    storage.set_global("TEXT", Value::String("Äb".into()));

    let view = storage
        .read_slot(MemoryLocation::Global, 0)
        .expect("the text slot resolves");
    let text = storage
        .text(&view)
        .expect("the text is borrowed, not cloned");
    // A borrow that trims or drops bytes returns "" or "b" here.
    assert_eq!(text, "Äb");
    assert_eq!(text.len(), 3);
}

#[test]
fn view_borrow_returns_the_stored_leaf_without_copying() {
    // spec 34 §6.2 (MEM-10): a view borrows the stored member itself, so the
    // engine's copy charge sees the stored value and not a copy of it.
    let mut storage = VariableStorage::new();
    storage.set_global("AXIS", axis());

    let view = storage
        .read_path(
            MemoryLocation::Global,
            0,
            &[RefSegment::Field(SmolStr::new("Scale"))],
        )
        .expect("Scale resolves");
    let borrowed = storage.borrow(&view).expect("the member borrows");

    // A borrow that copies the member, or that lands on the root
    // (`InternalIndex` = 1), is not the stored field: only the stored field has
    // this address.
    let root = storage.get_global("AXIS").expect("AXIS is stored");
    let Value::Struct(root) = root else {
        panic!("AXIS is a structure");
    };
    let stored = root.field("Scale").expect("Scale is stored");
    assert!(core::ptr::eq(borrowed, stored));
    assert_eq!(borrowed, &Value::UInt(9));
}

#[test]
fn view_write_of_a_whole_scalar_normalizes_for_its_target() {
    // spec 34 MEM-06: assignment copies into the destination's own type, so an
    // INT written into a DINT slot is stored as DINT.
    let mut storage = VariableStorage::new();
    storage.set_global("COUNT", Value::DInt(1));
    let reference = storage.ref_for_global("COUNT").expect("COUNT resolves");

    assert!(storage.write_slot(
        reference.location,
        reference.offset as usize,
        Value::Int(42)
    ));
    // A write that bypasses normalize_assignment_for_target stores Value::Int(42).
    assert_eq!(storage.get_global("COUNT"), Some(&Value::DInt(42)));
}

#[test]
fn view_commit_of_a_prepared_path_write_leaves_earlier_segments_unchanged() {
    // spec 34 MEM-06: a typed path write is all-or-nothing. Preparing it changes
    // nothing, and committing the leaf leaves every earlier segment intact.
    let mut storage = VariableStorage::new();
    let original = Value::Struct(Arc::new(StructValue::from_untyped_parts(
        SmolStr::new("Outer"),
        IndexMap::from([
            (SmolStr::new("left"), Value::DInt(1)),
            (
                SmolStr::new("inner"),
                Value::Struct(Arc::new(StructValue::from_untyped_parts(
                    SmolStr::new("Inner"),
                    IndexMap::from([
                        (SmolStr::new("first"), Value::DInt(2)),
                        (SmolStr::new("second"), Value::DInt(3)),
                    ]),
                ))),
            ),
        ]),
    )));
    storage.set_global("ROOT", original.clone());
    let path = [
        RefSegment::Field(SmolStr::new("inner")),
        RefSegment::Field(SmolStr::new("second")),
    ];

    let prepared = storage
        .prepare_write(MemoryLocation::Global, 0, &path, &Value::DInt(9))
        .expect("preparing a scalar field write does not fail")
        .expect("the path resolves");
    // Preparing stages the patch; it must not publish any part of it.
    assert_eq!(storage.get_global("ROOT"), Some(&original));

    assert!(storage.commit_write(&path, Value::DInt(9), prepared, MAX_STRING_CAPACITY));

    assert_eq!(
        read_global(&storage, &[RefSegment::Field(SmolStr::new("left"))]),
        Value::DInt(1)
    );
    // A commit that replaces the earlier `inner` segment wholesale loses `first`.
    assert_eq!(
        read_global(
            &storage,
            &[
                RefSegment::Field(SmolStr::new("inner")),
                RefSegment::Field(SmolStr::new("first")),
            ],
        ),
        Value::DInt(2)
    );
    assert_eq!(
        read_global(
            &storage,
            &[
                RefSegment::Field(SmolStr::new("inner")),
                RefSegment::Field(SmolStr::new("second")),
            ],
        ),
        Value::DInt(9)
    );
}

#[test]
fn view_write_of_a_foreign_generation_reference_writes_nothing() {
    // spec 34 §11.3 (MEM-07): a stale handle writes nothing; a checked write
    // refuses it with ReferenceLifetime before any change.
    let mut storage = VariableStorage::new();
    storage.set_global("COUNT", Value::DInt(1));
    let mut reference = storage.ref_for_global("COUNT").expect("COUNT resolves");
    reference.generation = Generation(1);

    assert_eq!(
        storage.write_reference(reference.as_view(), Value::DInt(9)),
        Err(RuntimeError::ReferenceLifetime)
    );
    // A stale handle writing into a new installation changes this value.
    assert_eq!(storage.get_global("COUNT"), Some(&Value::DInt(1)));

    // The same reference bound to the installed generation writes.
    reference.generation = Generation::FIRST;
    assert_eq!(
        storage.write_reference(reference.as_view(), Value::DInt(9)),
        Ok(true)
    );
    assert_eq!(storage.get_global("COUNT"), Some(&Value::DInt(9)));
}

#[test]
fn view_read_of_a_foreign_generation_reference_reads_nothing() {
    // spec 34 §11.3 (MEM-07): a read through a reference of another generation
    // returns nothing; the same reference in this generation reads its value.
    let mut storage = VariableStorage::new();
    storage.set_global("COUNT", Value::DInt(7));
    let mut reference = storage.ref_for_global("COUNT").expect("COUNT resolves");
    reference.generation = Generation(1);

    // A stale handle that resolves anyway reads the current installation's value.
    assert!(matches!(
        storage.read_reference(reference.as_view()),
        Err(VmTrap::NullReference)
    ));

    reference.generation = Generation::FIRST;
    let view = storage
        .read_reference(reference.as_view())
        .expect("the installed generation reads");
    assert_eq!(
        storage.materialize(&view).expect("the value materializes"),
        Value::DInt(7)
    );
}

#[test]
fn view_instance_var_forwarding_selects_the_named_member_and_writes_it_raw() {
    // spec 34 §6.2 (MEM-10): an instance's member is selected by its declared
    // name. The forwarding read lands on the wrong member if it ignores the name.
    let mut storage = VariableStorage::new();
    let instance = storage.create_instance("Probe");
    storage.set_instance_var(instance, "number", Value::DInt(7));
    storage.set_instance_var(instance, "level", Value::Int(3));

    let view = storage
        .read_instance_var(instance, "number")
        .expect("the declared member resolves");
    // A read that ignores the name returns the neighbouring member here.
    assert_eq!(
        storage.materialize(&view).expect("number materializes"),
        Value::DInt(7)
    );

    assert!(storage.write_instance_var(instance, "number", Value::Int(42)));
    // The forwarding write replaces the named member with the exact value; a
    // forwarding write that normalizes for the target would store DInt(42), and a
    // write that lands on the wrong member would leave `number` at 7.
    assert_eq!(
        storage.get_instance_var(instance, "number"),
        Some(&Value::Int(42))
    );
    assert_eq!(
        storage.get_instance_var(instance, "level"),
        Some(&Value::Int(3))
    );
}
