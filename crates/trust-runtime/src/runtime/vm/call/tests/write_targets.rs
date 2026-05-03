#[test]
fn vm_fb_out_source_reads_direct_instance_field() {
    let mut runtime = Runtime::new();
    let instance = runtime.storage.create_instance("FB");
    assert!(runtime
        .storage
        .set_instance_var(instance, "OUT", crate::value::Value::DInt(41)));

    let offset = runtime
        .storage
        .declared_instance_field_offset(instance, "OUT")
        .expect("declared OUT offset");
    let source = VmFbOutSource::Direct {
        instance_id: instance,
        offset,
    };

    assert!(matches!(
        source.read(&runtime).expect("direct out source read"),
        crate::value::Value::DInt(41)
    ));
}

#[test]
fn vm_fb_out_source_reads_reference_field() {
    let mut runtime = Runtime::new();
    let instance = runtime.storage.create_instance("FB");
    assert!(runtime
        .storage
        .set_instance_var(instance, "OUT", crate::value::Value::DInt(17)));

    let reference = runtime
        .storage
        .ref_for_instance_recursive(instance, "OUT")
        .expect("reference OUT field");
    let source = VmFbOutSource::Reference(reference.clone());
    assert_eq!(reference.location, MemoryLocation::Instance(instance));
    assert!(matches!(
        source.read(&runtime).expect("reference out source read"),
        crate::value::Value::DInt(17)
    ));
}

#[test]
fn vm_fb_field_binding_out_source_uses_direct_for_declared_fields() {
    let mut runtime = Runtime::new();
    let instance = runtime.storage.create_instance("FB");
    assert!(runtime
        .storage
        .set_instance_var(instance, "OUT", crate::value::Value::DInt(23)));

    let binding = super::VmFbFieldBinding::resolve(&runtime, instance, &SmolStr::new("OUT"))
        .expect("declared field binding");
    match binding.out_source() {
        VmFbOutSource::Direct {
            instance_id,
            offset,
        } => {
            assert_eq!(instance_id, instance);
            assert_eq!(offset, 0);
        }
        VmFbOutSource::Reference(_) => panic!("expected direct output source"),
    }
}

#[test]
fn vm_fb_field_binding_out_source_falls_back_to_reference_for_inherited_fields() {
    let mut runtime = Runtime::new();
    let base = runtime.storage.create_instance("BASE");
    let derived = runtime.storage.create_instance("DERIVED");
    runtime
        .storage
        .get_instance_mut(derived)
        .expect("derived instance")
        .parent = Some(base);
    assert!(runtime
        .storage
        .set_instance_var(base, "OUT", crate::value::Value::DInt(29)));

    let binding = super::VmFbFieldBinding::resolve(&runtime, derived, &SmolStr::new("OUT"))
        .expect("inherited field binding");
    match binding.out_source() {
        VmFbOutSource::Reference(reference) => {
            assert_eq!(reference.location, MemoryLocation::Instance(base));
            assert_eq!(reference.offset, 0);
        }
        VmFbOutSource::Direct { .. } => panic!("expected inherited fallback reference"),
    }
}

#[test]
fn vm_fb_field_binding_reads_writes_and_reports_invalid_direct_offset() {
    let mut runtime = Runtime::new();
    let instance = runtime.storage.create_instance("FB");
    assert!(runtime
        .storage
        .set_instance_var(instance, "OUT", Value::DInt(23)));

    let binding = super::VmFbFieldBinding::resolve(&runtime, instance, &SmolStr::new("OUT"))
        .expect("field binding");
    assert_eq!(binding.read(&runtime), Some(&Value::DInt(23)));
    assert!(binding.write(&mut runtime, Value::DInt(24)));
    assert_eq!(instance_field(&runtime, instance, "OUT"), Value::DInt(24));

    let invalid = super::VmFbFieldBinding::Direct {
        instance_id: instance,
        offset: 99,
    };
    assert!(!invalid.write(&mut runtime, Value::DInt(25)));
}

#[test]
fn vm_write_target_uses_direct_storage_for_empty_path_instance_refs() {
    let mut runtime = Runtime::new();
    let instance = runtime.storage.create_instance("FB");
    assert!(runtime
        .storage
        .set_instance_var(instance, "OUT", Value::DInt(31)));
    let reference = runtime
        .storage
        .ref_for_instance(instance, "OUT")
        .expect("instance output ref");
    let target = VmWriteTarget::from_reference(&reference);
    assert!(matches!(
        target.clone(),
        VmWriteTarget::DirectStorage {
            location: MemoryLocation::Instance(id),
            offset: 0
        } if id == instance
    ));

    let caller_frame = VmFrame {
        pou_id: 0,
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 0,
        locals: vec![],
        runtime_instance: None,
        instance_owner: None,
    };
    assert!(matches!(
        target
            .read(&mut runtime, &caller_frame)
            .expect("direct instance read"),
        Value::DInt(31)
    ));
}

#[test]
fn vm_write_target_uses_direct_storage_for_empty_path_global_refs() {
    let mut runtime = Runtime::new();
    runtime.storage.set_global("OUT", Value::DInt(17));
    let reference = runtime
        .storage
        .ref_for_global("OUT")
        .expect("global output ref");
    let target = VmWriteTarget::from_reference(&reference);
    assert!(matches!(
        target.clone(),
        VmWriteTarget::DirectStorage {
            location: MemoryLocation::Global,
            offset: 0
        }
    ));

    let caller_frame = VmFrame {
        pou_id: 0,
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 0,
        locals: vec![],
        runtime_instance: None,
        instance_owner: None,
    };
    assert!(matches!(
        target
            .read(&mut runtime, &caller_frame)
            .expect("direct target read"),
        Value::DInt(17)
    ));
}

#[test]
fn vm_write_target_uses_caller_local_direct_for_empty_path_vm_locals() {
    let mut runtime = Runtime::new();
    let reference = ValueRef {
        location: MemoryLocation::Local(FrameId(VM_LOCAL_SENTINEL_FRAME_ID)),
        offset: 0,
        path: RefPath::new(),
    };
    let target = VmWriteTarget::from_reference(&reference);
    assert!(matches!(
        target.clone(),
        VmWriteTarget::CallerLocalDirect { offset: 0 }
    ));

    let mut caller_frame = VmFrame {
        pou_id: 0,
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 1,
        locals: vec![Value::DInt(21)],
        runtime_instance: None,
        instance_owner: None,
    };
    assert!(matches!(
        target
            .read(&mut runtime, &caller_frame)
            .expect("local direct read"),
        Value::DInt(21)
    ));
    target
        .write(&mut runtime, &mut caller_frame, Value::DInt(42))
        .expect("local direct write");
    assert!(matches!(
        caller_frame.locals.first(),
        Some(&Value::DInt(42))
    ));
}

#[test]
fn vm_write_target_keeps_nested_path_targets_on_reference_fallback() {
    let reference = ValueRef {
        location: MemoryLocation::Global,
        offset: 0,
        path: [RefSegment::Field(SmolStr::new("VALUE"))]
            .into_iter()
            .collect(),
    };
    let target = VmWriteTarget::from_reference(&reference);
    assert!(matches!(target.clone(), VmWriteTarget::Reference(_)));
}

#[test]
fn read_vm_target_value_matches_generic_reference_path_across_reference_shapes() {
    let mut runtime = Runtime::new();
    runtime.storage.set_global("GLOBAL", Value::DInt(7));
    let mut struct_fields = IndexMap::new();
    struct_fields.insert(SmolStr::new("VALUE"), Value::DInt(8));
    runtime.storage.set_global(
        "STRUCT",
        Value::Struct(std::sync::Arc::new(StructValue::from_untyped_parts(
            SmolStr::new("TEST_STRUCT"),
            struct_fields,
        ))),
    );
    let instance = runtime.storage.create_instance("FB");
    assert!(runtime
        .storage
        .set_instance_var(instance, "ACC", Value::DInt(9)));

    let global_ref = runtime
        .storage
        .ref_for_global("GLOBAL")
        .expect("global ref");
    let struct_ref = runtime
        .storage
        .ref_for_global("STRUCT")
        .expect("struct ref");
    let nested_ref = ValueRef {
        location: struct_ref.location,
        offset: struct_ref.offset,
        path: [RefSegment::Field(SmolStr::new("VALUE"))]
            .into_iter()
            .collect(),
    };
    let instance_ref = runtime
        .storage
        .ref_for_instance(instance, "ACC")
        .expect("instance ref");

    let caller_frame = VmFrame {
        pou_id: 0,
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 1,
        locals: vec![Value::DInt(10)],
        runtime_instance: None,
        instance_owner: None,
    };
    let local_ref = ValueRef {
        location: MemoryLocation::Local(FrameId(VM_LOCAL_SENTINEL_FRAME_ID)),
        offset: 0,
        path: RefPath::new(),
    };

    for reference in [global_ref, nested_ref, instance_ref, local_ref] {
        let direct = super::read_vm_target_value(&mut runtime, &caller_frame, &reference)
            .expect("direct target value");
        let generic = super::read_vm_reference(&mut runtime, &caller_frame, &reference)
            .expect("generic target value");
        assert_eq!(direct, generic);
    }
}

#[test]
fn write_output_int_inspects_target_type_without_read_clone() {
    let mut runtime = Runtime::new();
    runtime.storage.set_global("OUT", Value::DInt(0));
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let reference = runtime
        .storage
        .ref_for_global("OUT")
        .expect("global output ref");
    let target = super::VmWriteTarget::from_reference(&reference);
    let mut caller_frame = empty_caller_frame();

    super::write_output_int(&mut runtime, &mut caller_frame, &target, 17)
        .expect("write output int");

    assert_eq!(runtime.storage.get_global("OUT"), Some(&Value::DInt(17)));
    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(profile.value_ops.read_value_clones, 0);
}

#[test]
fn write_output_int_preserves_integer_target_widths() {
    let cases = [
        (Value::SInt(0), -8, Value::SInt(-8)),
        (Value::Int(0), -9, Value::Int(-9)),
        (Value::DInt(0), 10, Value::DInt(10)),
        (Value::LInt(0), -11, Value::LInt(-11)),
        (Value::USInt(0), 12, Value::USInt(12)),
        (Value::UInt(0), 13, Value::UInt(13)),
        (Value::UDInt(0), 14, Value::UDInt(14)),
        (Value::ULInt(0), 15, Value::ULInt(15)),
    ];

    for (initial, value, expected) in cases {
        assert_eq!(write_int_to_global(initial, value).unwrap(), expected);
    }
}

#[test]
fn write_output_int_rejects_unsigned_negative_values() {
    for initial in [
        Value::USInt(0),
        Value::UInt(0),
        Value::UDInt(0),
        Value::ULInt(0),
    ] {
        let err = write_int_to_global(initial, -1).expect_err("negative unsigned output");
        assert!(matches!(err, VmTrap::Runtime(RuntimeError::Overflow)));
    }
}

#[test]
fn write_vm_reference_updates_nested_vm_local_path() {
    let mut runtime = Runtime::new();
    let mut fields = IndexMap::new();
    fields.insert(SmolStr::new("VALUE"), Value::DInt(1));
    let mut caller_frame = VmFrame {
        pou_id: 0,
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 1,
        locals: vec![Value::Struct(std::sync::Arc::new(
            StructValue::from_untyped_parts(SmolStr::new("LOCAL_STRUCT"), fields),
        ))],
        runtime_instance: None,
        instance_owner: None,
    };
    let reference = ValueRef {
        location: MemoryLocation::Local(FrameId(VM_LOCAL_SENTINEL_FRAME_ID)),
        offset: 0,
        path: [RefSegment::Field(SmolStr::new("VALUE"))]
            .into_iter()
            .collect(),
    };

    let target = VmWriteTarget::from_reference(&reference);
    target
        .write(&mut runtime, &mut caller_frame, Value::DInt(99))
        .expect("write nested local reference");

    let value = super::read_vm_reference(&mut runtime, &caller_frame, &reference)
        .expect("read nested local reference");
    assert_eq!(value, Value::DInt(99));
}

#[test]
fn read_vm_target_value_avoids_clone_counter_for_scalar_direct_target() {
    let mut runtime = Runtime::new();
    runtime.storage.set_global("OUT", Value::DInt(23));
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let reference = runtime
        .storage
        .ref_for_global("OUT")
        .expect("global output ref");
    let caller_frame = empty_caller_frame();

    let value = super::read_vm_target_value(&mut runtime, &caller_frame, &reference)
        .expect("read target value");

    assert_eq!(value, Value::DInt(23));
    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(profile.value_ops.read_value_clones, 0);
}

#[test]
fn preparse_native_symbol_spec_preserves_parse_error_message() {
    let entry = preparse_native_symbol_spec(&SmolStr::new("Add|Q:oops"));
    match entry {
        VmNativeSymbolSpec::ParseError(err) => {
            assert!(err.contains("must start with E/T"));
        }
        VmNativeSymbolSpec::Parsed { .. } => {
            panic!("expected parse error");
        }
    }
}
