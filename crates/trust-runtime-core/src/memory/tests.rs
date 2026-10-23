use super::*;
use crate::value::{ref_indices_from_iter, ArrayValue, StructValue};

#[test]
fn variable_storage_clone_recovers_from_poisoned_cache_lock() {
    let storage = std::sync::Arc::new(VariableStorage::new());
    let poisoned = std::sync::Arc::clone(&storage);
    let _ = std::thread::spawn(move || {
        let _guard = poisoned
            .instance_field_offsets
            .write()
            .expect("test cache lock");
        panic!("poison instance field cache");
    })
    .join();

    let cloned = (*storage).clone();
    assert!(cloned.globals.is_empty());
}

#[test]
fn instance_field_cache_is_scoped_per_instance() {
    let mut storage = VariableStorage::new();
    let first = storage.create_instance("FB");
    let second = storage.create_instance("FB");

    assert!(storage.set_instance_var(first, "ACC", Value::DInt(7)));

    let first_ref = storage
        .ref_for_instance(first, "ACC")
        .expect("missing first ACC ref");
    assert_eq!(first_ref.location, MemoryLocation::Instance(first));
    assert_eq!(first_ref.offset, 0);

    let second_ref = storage.ref_for_instance(second, "ACC");
    assert!(second_ref.is_none());

    let cache = storage
        .instance_field_offsets
        .read()
        .expect("cache poisoned");
    assert_eq!(cache.get(&(first, SmolStr::new("ACC"))), Some(&Some(0)));
    assert_eq!(cache.get(&(second, SmolStr::new("ACC"))), Some(&None));
}

#[test]
fn direct_instance_field_miss_cache_invalidates_on_new_insert() {
    let mut storage = VariableStorage::new();
    let instance = storage.create_instance("FB");

    assert!(storage.ref_for_instance(instance, "LATE").is_none());
    assert_eq!(
        storage
            .instance_field_offsets
            .read()
            .expect("cache poisoned")
            .get(&(instance, SmolStr::new("LATE"))),
        Some(&None)
    );

    assert!(storage.set_instance_var(instance, "LATE", Value::Bool(true)));
    assert!(storage
        .instance_field_offsets
        .read()
        .expect("cache poisoned")
        .get(&(instance, SmolStr::new("LATE")))
        .is_none());

    let resolved = storage
        .ref_for_instance(instance, "LATE")
        .expect("late field should resolve after insert");
    assert_eq!(resolved.location, MemoryLocation::Instance(instance));
    assert_eq!(resolved.offset, 0);
}

#[test]
fn recursive_instance_field_cache_invalidates_when_child_adds_shadowing_field() {
    let mut storage = VariableStorage::new();
    let base = storage.create_instance("BASE");
    let derived = storage.create_instance("DERIVED");
    storage
        .get_instance_mut(derived)
        .expect("derived instance")
        .parent = Some(base);

    assert!(storage.set_instance_var(base, "ACC", Value::DInt(11)));

    let inherited = storage
        .ref_for_instance_recursive(derived, "ACC")
        .expect("inherited field should resolve");
    assert_eq!(inherited.location, MemoryLocation::Instance(base));
    assert_eq!(
        storage
            .recursive_instance_field_resolutions
            .read()
            .expect("cache poisoned")
            .get(&(derived, SmolStr::new("ACC")))
            .copied(),
        Some(RecursiveInstanceFieldResolution {
            owner_depth: 1,
            offset: inherited.offset,
        })
    );

    assert!(storage.set_instance_var(derived, "ACC", Value::DInt(22)));
    assert!(storage
        .recursive_instance_field_resolutions
        .read()
        .expect("cache poisoned")
        .get(&(derived, SmolStr::new("ACC")))
        .is_none());

    let shadowed = storage
        .ref_for_instance_recursive(derived, "ACC")
        .expect("shadowed field should resolve");
    assert_eq!(shadowed.location, MemoryLocation::Instance(derived));
    assert_eq!(shadowed.offset, 0);
    assert!(matches!(
        storage.read_by_ref(shadowed).expect("shadowed field value"),
        Value::DInt(22)
    ));
}

#[test]
fn declared_instance_field_offset_reuses_type_layout_for_declared_fields() {
    let mut storage = VariableStorage::new();
    let first = storage.create_instance("FB");
    let second = storage.create_instance("FB");

    assert!(storage.set_instance_var(first, "IN", Value::Bool(false)));
    assert!(storage.set_instance_var(first, "OUT", Value::Bool(false)));
    assert!(storage.set_instance_var(second, "IN", Value::Bool(true)));
    assert!(storage.set_instance_var(second, "OUT", Value::Bool(true)));
    assert!(storage.set_instance_var(first, "__hidden", Value::Bool(true)));

    let first_in = storage
        .declared_instance_field_offset(first, "IN")
        .expect("first IN offset");
    let second_in = storage
        .declared_instance_field_offset(second, "IN")
        .expect("second IN offset");
    assert_eq!(first_in, 0);
    assert_eq!(second_in, first_in);

    let second_out = storage
        .declared_instance_field_offset(second, "OUT")
        .expect("second OUT offset");
    assert_eq!(second_out, 1);
    assert!(matches!(
        storage
            .read_instance_field_by_offset(second, second_out)
            .expect("second OUT value"),
        Value::Bool(true)
    ));
}

#[test]
fn declared_instance_field_offset_skips_inherited_fields() {
    let mut storage = VariableStorage::new();
    let base = storage.create_instance("BASE");
    let derived = storage.create_instance("DERIVED");
    storage
        .get_instance_mut(derived)
        .expect("derived instance")
        .parent = Some(base);

    assert!(storage.set_instance_var(base, "PARENT_PARAM", Value::DInt(5)));
    assert!(storage
        .declared_instance_field_offset(derived, "PARENT_PARAM")
        .is_none());

    let inherited = storage
        .ref_for_instance_recursive(derived, "PARENT_PARAM")
        .expect("recursive inherited field");
    assert_eq!(inherited.location, MemoryLocation::Instance(base));
    assert_eq!(inherited.offset, 0);
}

#[test]
fn resolved_instance_field_ref_prefers_direct_field_before_parent_fallback() {
    let mut storage = VariableStorage::new();
    let base = storage.create_instance("BASE");
    let derived = storage.create_instance("DERIVED");
    storage
        .get_instance_mut(derived)
        .expect("derived instance")
        .parent = Some(base);

    assert!(storage.set_instance_var(base, "ACC", Value::DInt(5)));
    assert!(storage.set_instance_var(derived, "ACC", Value::DInt(9)));
    let direct = storage
        .resolved_instance_field_ref(derived, "ACC")
        .expect("direct field ref");
    assert_eq!(direct.location, MemoryLocation::Instance(derived));
    assert_eq!(direct.offset, 0);

    let parent_only = storage
        .resolved_instance_field_ref(derived, "BASE_ONLY")
        .is_none();
    assert!(parent_only);

    assert!(storage.set_instance_var(base, "BASE_ONLY", Value::DInt(12)));
    let inherited = storage
        .resolved_instance_field_ref(derived, "BASE_ONLY")
        .expect("inherited field ref");
    assert_eq!(inherited.location, MemoryLocation::Instance(base));
    assert_eq!(inherited.offset, 1);
}

#[test]
fn direct_instance_field_offset_reads_and_writes_without_value_ref() {
    let mut storage = VariableStorage::new();
    let instance = storage.create_instance("FB");
    assert!(storage.set_instance_var(instance, "ACC", Value::DInt(11)));

    let offset = storage
        .declared_instance_field_offset(instance, "ACC")
        .expect("ACC offset");
    assert!(matches!(
        storage
            .read_instance_field_by_offset(instance, offset)
            .expect("read by offset"),
        Value::DInt(11)
    ));

    assert!(storage.write_instance_field_by_offset(instance, offset, Value::DInt(22)));
    assert!(matches!(
        storage
            .read_instance_field_by_offset(instance, offset)
            .expect("updated read by offset"),
        Value::DInt(22)
    ));
}

#[test]
fn direct_slot_helpers_cover_global_local_and_instance_locations() {
    let mut storage = VariableStorage::new();
    storage.set_global("G", Value::DInt(1));
    let frame_id = storage.push_frame("MAIN");
    assert!(storage.set_local("L", Value::DInt(2)));
    let instance = storage.create_instance("FB");
    assert!(storage.set_instance_var(instance, "I", Value::DInt(3)));

    let global_ref = storage.ref_for_global("G").expect("global ref");
    let local_ref = storage.ref_for_local("L").expect("local ref");
    let instance_ref = storage
        .ref_for_instance(instance, "I")
        .expect("instance ref");

    assert_eq!(local_ref.location, MemoryLocation::Local(frame_id));
    assert!(matches!(
        storage.read_direct_slot_by_location(MemoryLocation::Global, global_ref.offset),
        Some(&Value::DInt(1))
    ));
    assert!(matches!(
        storage.read_direct_slot_by_location(local_ref.location, local_ref.offset),
        Some(&Value::DInt(2))
    ));
    assert!(matches!(
        storage.read_direct_slot_by_location(instance_ref.location, instance_ref.offset),
        Some(&Value::DInt(3))
    ));

    assert!(storage.write_direct_slot_by_location(
        MemoryLocation::Global,
        global_ref.offset,
        Value::DInt(11)
    ));
    assert!(storage.write_direct_slot_by_location(
        local_ref.location,
        local_ref.offset,
        Value::DInt(12)
    ));
    assert!(storage.write_direct_slot_by_location(
        instance_ref.location,
        instance_ref.offset,
        Value::DInt(13)
    ));

    assert!(matches!(storage.get_global("G"), Some(&Value::DInt(11))));
    assert!(matches!(storage.get_local("L"), Some(&Value::DInt(12))));
    assert!(matches!(
        storage.get_instance_var(instance, "I"),
        Some(&Value::DInt(13))
    ));
}

#[test]
fn direct_slot_helpers_match_empty_path_ref_helpers() {
    let mut storage = VariableStorage::new();
    storage.set_global("G", Value::DInt(5));
    let frame_id = storage.push_frame("MAIN");
    assert!(storage.set_local("L", Value::DInt(6)));
    let instance = storage.create_instance("FB");
    assert!(storage.set_instance_var(instance, "I", Value::DInt(7)));

    let refs = [
        storage.ref_for_global("G").expect("global ref"),
        storage.ref_for_local("L").expect("local ref"),
        storage
            .ref_for_instance(instance, "I")
            .expect("instance ref"),
    ];
    assert_eq!(refs[1].location, MemoryLocation::Local(frame_id));

    for reference in refs {
        let direct = storage
            .read_direct_slot_by_location(reference.location, reference.offset)
            .expect("direct slot read");
        let generic = storage
            .read_by_ref_parts(reference.location, reference.offset, &[])
            .expect("generic empty-path read");
        assert_eq!(direct, generic);
    }
}

#[test]
fn borrowed_value_ref_helpers_match_owned_helpers() {
    let mut storage = VariableStorage::new();
    let instance = storage.create_instance("FB");
    assert!(storage.set_instance_var(instance, "ACC", Value::DInt(11)));

    let reference = storage
        .ref_for_instance(instance, "ACC")
        .expect("instance field reference");
    assert!(matches!(
        storage.read_by_ref_ref(&reference).expect("borrowed read"),
        Value::DInt(11)
    ));
    assert!(matches!(
        storage.read_by_ref(reference.clone()).expect("owned read"),
        Value::DInt(11)
    ));

    assert!(storage.write_by_ref_ref(&reference, Value::DInt(22)));
    assert!(matches!(
        storage
            .read_by_ref_ref(&reference)
            .expect("updated borrowed read"),
        Value::DInt(22)
    ));
}

#[test]
fn recursive_lookup_does_not_cache_parent_chain_miss() {
    let mut storage = VariableStorage::new();
    let base = storage.create_instance("BASE");
    let derived = storage.create_instance("DERIVED");
    storage
        .get_instance_mut(derived)
        .expect("derived instance")
        .parent = Some(base);

    assert!(storage
        .ref_for_instance_recursive(derived, "LATE")
        .is_none());
    assert!(storage
        .recursive_instance_field_resolutions
        .read()
        .expect("cache poisoned")
        .get(&(derived, SmolStr::new("LATE")))
        .is_none());

    assert!(storage.set_instance_var(base, "LATE", Value::Bool(true)));

    let resolved = storage
        .ref_for_instance_recursive(derived, "LATE")
        .expect("parent field should resolve after insert");
    assert_eq!(resolved.location, MemoryLocation::Instance(base));
    assert_eq!(resolved.offset, 0);
    assert!(matches!(
        storage.read_by_ref(resolved).expect("parent field value"),
        Value::Bool(true)
    ));
}

#[test]
fn write_by_ref_path_preserves_struct_copy_on_write_isolation() {
    let mut storage = VariableStorage::new();
    let shared = Value::Struct(std::sync::Arc::new(StructValue::from_untyped_parts(
        SmolStr::new("AXIS_REF"),
        IndexMap::from([(SmolStr::new("InternalIndex"), Value::UInt(1))]),
    )));
    storage.set_global("left", shared.clone());
    storage.set_global("right", shared);

    assert!(storage.write_by_ref_parts(
        MemoryLocation::Global,
        0,
        &[RefSegment::Field(SmolStr::new("InternalIndex"))],
        Value::UInt(7),
    ));

    let left = storage.get_global("left").expect("left global");
    let right = storage.get_global("right").expect("right global");
    let Value::Struct(left_struct) = left else {
        panic!("left should be struct");
    };
    let Value::Struct(right_struct) = right else {
        panic!("right should be struct");
    };
    assert_eq!(left_struct.field("InternalIndex"), Some(&Value::UInt(7)));
    assert_eq!(right_struct.field("InternalIndex"), Some(&Value::UInt(1)));
}

#[test]
fn read_and_write_by_ref_handle_extreme_array_bounds_without_overflow() {
    let mut storage = VariableStorage::new();
    storage.set_global(
        "GRID",
        Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![Value::DInt(7)],
            vec![(i64::MIN, i64::MAX)],
        ))),
    );
    let mut reference = storage.ref_for_global("GRID").expect("grid ref");
    reference
        .path
        .push(RefSegment::Index(ref_indices_from_iter([i64::MIN])));

    let read = storage
        .read_by_ref(reference.clone())
        .expect("read extreme lower bound");
    assert_eq!(read, &Value::DInt(7));

    assert!(storage.write_by_ref(reference.clone(), Value::DInt(9)));
    let updated = storage
        .read_by_ref(reference)
        .expect("read updated lower bound");
    assert_eq!(updated, &Value::DInt(9));
}

#[test]
fn read_and_write_by_ref_non_ascii_string_uses_character_elements() {
    let mut storage = VariableStorage::new();
    storage.set_global("TEXT", Value::String("ÄBC".into()));
    let mut reference = storage.ref_for_global("TEXT").expect("text ref");
    reference
        .path
        .push(RefSegment::Index(ref_indices_from_iter([1])));

    let read = storage
        .materialize_by_ref(reference.clone())
        .expect("read non-ascii string element");
    assert_eq!(read, Value::Char(0xC4));

    reference.path.clear();
    reference
        .path
        .push(RefSegment::Index(ref_indices_from_iter([2])));
    assert!(storage.write_by_ref(reference.clone(), Value::Char(b'X')));
    assert_eq!(
        storage.get_global("TEXT"),
        Some(&Value::String("ÄXC".into()))
    );
}
