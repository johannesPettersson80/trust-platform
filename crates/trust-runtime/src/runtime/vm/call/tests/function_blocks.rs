#[test]
fn bind_builtin_function_block_arguments_binds_named_inputs_and_outputs() {
    let mut runtime = Runtime::new();
    let instance = seed_ctu_instance(&mut runtime);
    runtime.storage.set_global("Q_OUT", Value::Bool(false));
    runtime.storage.set_global("CV_OUT", Value::Int(0));
    let q_ref = runtime
        .storage
        .ref_for_global("Q_OUT")
        .expect("Q output ref");
    let cv_ref = runtime
        .storage
        .ref_for_global("CV_OUT")
        .expect("CV output ref");

    let bindings = super::bind_builtin_function_block_arguments(
        &mut runtime,
        &empty_caller_frame(),
        &SmolStr::new("CTU"),
        "CTU",
        instance,
        &[
            expr_arg(Some("CU"), Value::Bool(true)),
            expr_arg(Some("R"), Value::Bool(false)),
            expr_arg(Some("PV"), Value::Int(2)),
            target_arg(Some("Q"), q_ref),
            target_arg(Some("CV"), cv_ref),
        ],
    )
    .expect("builtin FB named binding");

    assert_eq!(bindings.len(), 2);
    assert_eq!(instance_field(&runtime, instance, "CU"), Value::Bool(true));
    assert_eq!(instance_field(&runtime, instance, "PV"), Value::Int(2));
}

#[test]
fn bind_builtin_function_block_arguments_preserves_omitted_inputs() {
    let mut runtime = Runtime::new();
    let instance = seed_ctu_instance(&mut runtime);
    assert!(runtime
        .storage
        .set_instance_var(instance, "CU", Value::Bool(true)));
    assert!(runtime
        .storage
        .set_instance_var(instance, "PV", Value::Int(7)));
    runtime.storage.set_global("Q_OUT", Value::Bool(false));
    let q_ref = runtime
        .storage
        .ref_for_global("Q_OUT")
        .expect("Q output ref");

    let bindings = super::bind_builtin_function_block_arguments(
        &mut runtime,
        &empty_caller_frame(),
        &SmolStr::new("CTU"),
        "CTU",
        instance,
        &[target_arg(Some("Q"), q_ref)],
    )
    .expect("builtin FB omitted input binding");

    assert_eq!(bindings.len(), 1);
    assert_eq!(instance_field(&runtime, instance, "CU"), Value::Bool(true));
    assert_eq!(instance_field(&runtime, instance, "PV"), Value::Int(7));
}

#[test]
fn bind_builtin_function_block_arguments_accepts_exact_positional_and_rejects_extra() {
    let mut runtime = Runtime::new();
    let instance = seed_ctu_instance(&mut runtime);
    runtime.storage.set_global("Q_OUT", Value::Bool(false));
    runtime.storage.set_global("CV_OUT", Value::Int(0));
    let q_ref = runtime
        .storage
        .ref_for_global("Q_OUT")
        .expect("Q output ref");
    let cv_ref = runtime
        .storage
        .ref_for_global("CV_OUT")
        .expect("CV output ref");

    let exact = super::bind_builtin_function_block_arguments(
        &mut runtime,
        &empty_caller_frame(),
        &SmolStr::new("CTU"),
        "CTU",
        instance,
        &[
            expr_arg(None, Value::Bool(true)),
            expr_arg(None, Value::Bool(false)),
            expr_arg(None, Value::Int(2)),
            target_arg(None, q_ref.clone()),
            target_arg(None, cv_ref.clone()),
        ],
    )
    .expect("exact builtin FB positional binding");

    assert_eq!(exact.len(), 2);
    assert_eq!(instance_field(&runtime, instance, "CU"), Value::Bool(true));

    let err = super::bind_builtin_function_block_arguments(
        &mut runtime,
        &empty_caller_frame(),
        &SmolStr::new("CTU"),
        "CTU",
        instance,
        &[
            expr_arg(None, Value::Bool(true)),
            expr_arg(None, Value::Bool(false)),
            expr_arg(None, Value::Int(2)),
            target_arg(None, q_ref),
            target_arg(None, cv_ref),
            expr_arg(None, Value::Int(99)),
        ],
    )
    .expect_err("extra builtin FB positional arg should fail");

    assert!(
        matches!(err, VmTrap::InvalidNativeCall(message) if message.contains("too many positional arguments"))
    );
}

#[test]
fn bind_builtin_function_block_arguments_allows_omitted_positional_outputs() {
    let mut runtime = Runtime::new();
    let instance = seed_ctu_instance(&mut runtime);

    let bindings = super::bind_builtin_function_block_arguments(
        &mut runtime,
        &empty_caller_frame(),
        &SmolStr::new("CTU"),
        "CTU",
        instance,
        &[
            expr_arg(None, Value::Bool(true)),
            expr_arg(None, Value::Bool(false)),
            expr_arg(None, Value::Int(3)),
        ],
    )
    .expect("builtin FB positional inputs may omit trailing outputs");

    assert!(bindings.is_empty());
    assert_eq!(instance_field(&runtime, instance, "CU"), Value::Bool(true));
    assert_eq!(instance_field(&runtime, instance, "PV"), Value::Int(3));
}

#[test]
fn bind_builtin_function_block_arguments_supports_inout_rebinding() {
    let mut runtime = Runtime::new();
    runtime.register_function_block(FunctionBlockDef {
        name: SmolStr::new("CTU"),
        base: None,
        params: vec![Param {
            name: SmolStr::new("ACC"),
            type_id: TypeId::DINT,
            direction: ParamDirection::InOut,
            address: None,
            default: None,
        }],
        vars: Vec::new(),
        temps: Vec::new(),
        using: Vec::new(),
        methods: Vec::new(),
        body: Vec::new(),
    });
    let instance = runtime.storage.create_instance("CTU");
    assert!(runtime
        .storage
        .set_instance_var(instance, "ACC", Value::DInt(0)));
    runtime.storage.set_global("ACC_SRC", Value::DInt(5));
    let acc_ref = runtime
        .storage
        .ref_for_global("ACC_SRC")
        .expect("ACC source ref");

    let bindings = super::bind_builtin_function_block_arguments(
        &mut runtime,
        &empty_caller_frame(),
        &SmolStr::new("CTU"),
        "CTU",
        instance,
        &[target_arg(Some("ACC"), acc_ref)],
    )
    .expect("builtin FB IN_OUT binding should copy input into field");

    assert_eq!(bindings.len(), 1);
    assert_eq!(instance_field(&runtime, instance, "ACC"), Value::DInt(5));
}

#[test]
fn bind_vm_function_block_arguments_supports_mixed_out_and_inout_rebinding() {
    let mut runtime = Runtime::new();
    let instance = runtime.storage.create_instance("FB");
    assert!(runtime
        .storage
        .set_instance_var(instance, "OUT", crate::value::Value::DInt(0)));
    assert!(runtime
        .storage
        .set_instance_var(instance, "ACC", crate::value::Value::DInt(11)));
    runtime.storage.set_global("OUT", Value::DInt(0));
    runtime.storage.set_global("ACC_SRC", Value::DInt(5));
    let out_ref = runtime
        .storage
        .ref_for_global("OUT")
        .expect("out global ref");
    let acc_ref = runtime
        .storage
        .ref_for_global("ACC_SRC")
        .expect("acc source ref");
    let (module, pou_id) = manual_vm_function_block_module(vec![
        VmParamMeta {
            name: SmolStr::new("OUT"),
            type_id: 0,
            direction: 1,
            default_const_idx: None,
        },
        VmParamMeta {
            name: SmolStr::new("ACC"),
            type_id: 0,
            direction: 2,
            default_const_idx: None,
        },
    ]);

    let bindings = super::bind_vm_function_block_arguments(
        &mut runtime,
        &module,
        &empty_caller_frame(),
        pou_id,
        instance,
        &[
            target_arg(Some("OUT"), out_ref.clone()),
            target_arg(Some("ACC"), acc_ref.clone()),
        ],
    )
    .expect("mixed OUT and IN_OUT binding should succeed");

    assert_eq!(bindings.len(), 2);
}

#[test]
fn bind_vm_function_block_arguments_preserves_omitted_input_field() {
    let mut runtime = Runtime::new();
    let instance = runtime.storage.create_instance("FB");
    assert!(runtime
        .storage
        .set_instance_var(instance, "IN", Value::DInt(41)));
    let (module, pou_id) = manual_vm_function_block_module(vec![VmParamMeta {
        name: SmolStr::new("IN"),
        type_id: 0,
        direction: 0,
        default_const_idx: None,
    }]);

    let bindings = super::bind_vm_function_block_arguments(
        &mut runtime,
        &module,
        &empty_caller_frame(),
        pou_id,
        instance,
        &[],
    )
    .expect("omitted IN should preserve current field value");

    assert!(bindings.is_empty());
    assert_eq!(instance_field(&runtime, instance, "IN"), Value::DInt(41));
}

#[test]
fn bind_vm_function_block_arguments_accepts_exact_positional_and_rejects_extra() {
    let mut runtime = Runtime::new();
    let instance = runtime.storage.create_instance("FB");
    assert!(runtime
        .storage
        .set_instance_var(instance, "IN", Value::DInt(0)));
    assert!(runtime
        .storage
        .set_instance_var(instance, "OUT", Value::DInt(0)));
    runtime.storage.set_global("OUT_TARGET", Value::DInt(0));
    let out_ref = runtime
        .storage
        .ref_for_global("OUT_TARGET")
        .expect("out target ref");
    let (module, pou_id) = manual_vm_function_block_module(vec![
        VmParamMeta {
            name: SmolStr::new("IN"),
            type_id: 0,
            direction: 0,
            default_const_idx: None,
        },
        VmParamMeta {
            name: SmolStr::new("OUT"),
            type_id: 0,
            direction: 1,
            default_const_idx: None,
        },
    ]);

    let exact = super::bind_vm_function_block_arguments(
        &mut runtime,
        &module,
        &empty_caller_frame(),
        pou_id,
        instance,
        &[
            expr_arg(None, Value::DInt(9)),
            target_arg(None, out_ref.clone()),
        ],
    )
    .expect("exact VM FB positional binding");

    assert_eq!(exact.len(), 1);
    assert_eq!(instance_field(&runtime, instance, "IN"), Value::DInt(9));

    let err = super::bind_vm_function_block_arguments(
        &mut runtime,
        &module,
        &empty_caller_frame(),
        pou_id,
        instance,
        &[
            expr_arg(None, Value::DInt(9)),
            target_arg(None, out_ref),
            expr_arg(None, Value::DInt(99)),
        ],
    )
    .expect_err("extra VM FB positional arg should fail");

    assert!(
        matches!(err, VmTrap::InvalidNativeCall(message) if message.contains("too many positional arguments"))
    );
}

#[test]
fn bind_vm_call_arguments_accepts_exact_positional_and_rejects_extra() {
    let mut runtime = Runtime::new();
    let (module, pou_id) = manual_vm_function_module(
        "DoWork",
        vec![
            VmParamMeta {
                name: SmolStr::new("A"),
                type_id: 0,
                direction: 0,
                default_const_idx: None,
            },
            VmParamMeta {
                name: SmolStr::new("B"),
                type_id: 0,
                direction: 0,
                default_const_idx: None,
            },
        ],
        false,
    );

    let (locals, out_bindings) = super::bind_vm_call_arguments(
        &mut runtime,
        &module,
        &empty_caller_frame(),
        pou_id,
        &[
            expr_arg(None, Value::DInt(1)),
            expr_arg(None, Value::DInt(2)),
        ],
    )
    .expect("exact VM call positional binding");

    assert!(out_bindings.is_empty());
    assert_eq!(locals, vec![Value::DInt(1), Value::DInt(2)]);

    let err = super::bind_vm_call_arguments(
        &mut runtime,
        &module,
        &empty_caller_frame(),
        pou_id,
        &[
            expr_arg(None, Value::DInt(1)),
            expr_arg(None, Value::DInt(2)),
            expr_arg(None, Value::DInt(3)),
        ],
    )
    .expect_err("extra VM call positional arg should fail");

    assert!(
        matches!(err, VmTrap::InvalidNativeCall(message) if message.contains("too many positional arguments"))
    );
}

#[test]
fn bind_vm_call_arguments_allows_omitted_trailing_positional_input() {
    let mut runtime = Runtime::new();
    let (module, pou_id) = manual_vm_function_module(
        "DoWork",
        vec![
            VmParamMeta {
                name: SmolStr::new("A"),
                type_id: 0,
                direction: 0,
                default_const_idx: None,
            },
            VmParamMeta {
                name: SmolStr::new("B"),
                type_id: 0,
                direction: 0,
                default_const_idx: None,
            },
        ],
        false,
    );

    let (locals, out_bindings) = super::bind_vm_call_arguments(
        &mut runtime,
        &module,
        &empty_caller_frame(),
        pou_id,
        &[expr_arg(None, Value::DInt(1))],
    )
    .expect("trailing positional input may be omitted");

    assert!(out_bindings.is_empty());
    assert_eq!(locals, vec![Value::DInt(1), Value::Null]);
}

#[test]
fn bind_vm_call_arguments_rejects_legacy_untyped_string_output_target() {
    let mut runtime = Runtime::new();
    runtime
        .storage
        .set_global("LEGACY_TEXT", Value::String("OLD".into()));
    let target = runtime
        .storage
        .ref_for_global("LEGACY_TEXT")
        .expect("legacy string target");
    let (module, pou_id) = manual_vm_function_module(
        "EmitText",
        vec![VmParamMeta {
            name: SmolStr::new("TEXT"),
            type_id: 0,
            direction: 1,
            default_const_idx: None,
        }],
        false,
    );

    let error = super::bind_vm_call_arguments(
        &mut runtime,
        &module,
        &empty_caller_frame(),
        pou_id,
        &[target_arg(None, target)],
    )
    .expect_err("metadata-free string copyback must fail before callee execution");

    assert!(matches!(error, VmTrap::Runtime(RuntimeError::TypeMismatch)));
    assert_eq!(
        runtime.storage.get_global("LEGACY_TEXT"),
        Some(&Value::String("OLD".into()))
    );
}

#[test]
fn bind_vm_call_arguments_rejects_string_inout_capacity_mismatch_before_copy_in() {
    let mut runtime = Runtime::new();
    runtime
        .storage
        .set_global("CALLER_TEXT", Value::String("ABCDEFGHIJKLMNOPQRST".into()));
    let target = runtime
        .storage
        .ref_for_global("CALLER_TEXT")
        .expect("caller string ref");
    let (mut module, pou_id) = manual_vm_function_module(
        "ObserveText",
        vec![VmParamMeta {
            name: SmolStr::new("TEXT"),
            type_id: 0,
            direction: 2,
            default_const_idx: None,
        }],
        false,
    );
    install_string_inout_types(&mut module, target.clone());

    let error = super::bind_vm_call_arguments(
        &mut runtime,
        &module,
        &empty_caller_frame(),
        pou_id,
        &[target_arg(None, target)],
    )
    .expect_err("crafted STRING[20] -> STRING[5] IN_OUT must fail before execution");

    assert!(matches!(error, VmTrap::Runtime(RuntimeError::TypeMismatch)));
    assert_eq!(
        runtime.storage.get_global("CALLER_TEXT"),
        Some(&Value::String("ABCDEFGHIJKLMNOPQRST".into()))
    );
}

#[test]
fn bind_vm_function_block_arguments_rejects_string_inout_capacity_mismatch_before_copy_in() {
    let mut runtime = Runtime::new();
    let instance = runtime.storage.create_instance("FB");
    assert!(runtime
        .storage
        .set_instance_var(instance, "PREFIX", Value::DInt(1)));
    assert!(runtime
        .storage
        .set_instance_var(instance, "TEXT", Value::String("OLD".into())));
    runtime
        .storage
        .set_global("CALLER_TEXT", Value::String("ABCDEFGHIJKLMNOPQRST".into()));
    let target = runtime
        .storage
        .ref_for_global("CALLER_TEXT")
        .expect("caller string ref");
    let (mut module, pou_id) = manual_vm_function_block_module(vec![
        VmParamMeta {
            name: SmolStr::new("PREFIX"),
            type_id: 2,
            direction: 0,
            default_const_idx: None,
        },
        VmParamMeta {
            name: SmolStr::new("TEXT"),
            type_id: 0,
            direction: 2,
            default_const_idx: None,
        },
    ]);
    install_string_inout_types(&mut module, target.clone());

    let error = super::bind_vm_function_block_arguments(
        &mut runtime,
        &module,
        &empty_caller_frame(),
        pou_id,
        instance,
        &[expr_arg(None, Value::DInt(99)), target_arg(None, target)],
    )
    .expect_err("crafted STRING[20] -> STRING[5] FB IN_OUT must fail before execution");

    assert!(matches!(error, VmTrap::Runtime(RuntimeError::TypeMismatch)));
    assert_eq!(
        runtime.storage.get_global("CALLER_TEXT"),
        Some(&Value::String("ABCDEFGHIJKLMNOPQRST".into()))
    );
    assert_eq!(
        instance_field(&runtime, instance, "PREFIX"),
        Value::DInt(1),
        "later IN_OUT rejection must happen before earlier FB input mutation"
    );
    assert_eq!(
        instance_field(&runtime, instance, "TEXT"),
        Value::String("OLD".into())
    );
}

#[test]
fn output_copyback_rejects_untyped_null_string_target_without_writing() {
    let mut runtime = Runtime::new();
    runtime.storage.set_global("LEGACY_TEXT", Value::Null);
    let reference = runtime
        .storage
        .ref_for_global("LEGACY_TEXT")
        .expect("legacy null target");
    let target = super::VmWriteTarget::from_reference(&reference);
    let module = manual_vm_function_module("EmitText", Vec::new(), false).0;
    let caller_frame = empty_caller_frame();

    let error = super::normalize_output_copyback_value(
        &runtime,
        &module,
        &caller_frame,
        &target,
        None,
        Value::String("UNBOUNDED".into()),
    )
    .expect_err("metadata-free null target must reject string copyback");

    assert!(matches!(error, VmTrap::Runtime(RuntimeError::TypeMismatch)));
    assert_eq!(
        runtime.storage.get_global("LEGACY_TEXT"),
        Some(&Value::Null)
    );
}

#[test]
fn output_copyback_rejects_nonstring_for_declared_string_null_target() {
    let mut runtime = Runtime::new();
    runtime.storage.set_global("TEXT", Value::Null);
    let reference = runtime
        .storage
        .ref_for_global("TEXT")
        .expect("declared string target");
    let target = super::VmWriteTarget::from_reference(&reference);
    let (mut module, _) = manual_vm_function_module("EmitText", Vec::new(), false);
    install_string_inout_types(&mut module, reference);
    let caller_frame = empty_caller_frame();

    let error = super::normalize_output_copyback_value(
        &runtime,
        &module,
        &caller_frame,
        &target,
        Some(1),
        Value::DInt(7),
    )
    .expect_err("declared STRING target must reject a non-string output even while unassigned");

    assert!(matches!(error, VmTrap::Runtime(RuntimeError::TypeMismatch)));
    assert_eq!(runtime.storage.get_global("TEXT"), Some(&Value::Null));
}

#[test]
fn output_binding_rejects_conflicting_reference_types_for_nonstring_target() {
    let mut runtime = Runtime::new();
    runtime.storage.set_global("VALUE", Value::DInt(1));
    let reference = runtime
        .storage
        .ref_for_global("VALUE")
        .expect("numeric target");
    let (mut module, pou_id) = manual_vm_function_module(
        "EmitValue",
        vec![VmParamMeta {
            name: SmolStr::new("VALUE"),
            type_id: 0,
            direction: 1,
            default_const_idx: None,
        }],
        false,
    );
    module.types = crate::bytecode::TypeTable {
        offsets: Vec::new(),
        entries: vec![
            primitive_type(8, 0),
            primitive_type(4, 0),
        ],
    };
    for type_idx in 0..=1 {
        module.refs.push(super::super::VmRef::Global {
            offset: reference.offset,
            path: reference.path.clone(),
        });
        module.ref_types.insert(type_idx, type_idx);
    }

    let error = super::bind_vm_call_arguments(
        &mut runtime,
        &module,
        &empty_caller_frame(),
        pou_id,
        &[target_arg(None, reference)],
    )
    .expect_err("conflicting canonical reference types must fail closed for numeric targets");

    assert!(matches!(
        error,
        VmTrap::Runtime(RuntimeError::InvalidBytecode(message))
            if message.contains("conflicting type metadata")
    ));
    assert_eq!(runtime.storage.get_global("VALUE"), Some(&Value::DInt(1)));
}

fn primitive_type(prim_id: u16, max_length: u16) -> crate::bytecode::TypeEntry {
    crate::bytecode::TypeEntry {
        kind: crate::bytecode::TypeKind::Primitive,
        name_idx: None,
        data: crate::bytecode::TypeData::Primitive {
            prim_id,
            max_length,
        },
    }
}

fn install_string_inout_types(module: &mut VmModule, target: ValueRef) {
    module.types = crate::bytecode::TypeTable {
        offsets: Vec::new(),
        entries: vec![
            crate::bytecode::TypeEntry {
                kind: crate::bytecode::TypeKind::Primitive,
                name_idx: None,
                data: crate::bytecode::TypeData::Primitive {
                    prim_id: 24,
                    max_length: 5,
                },
            },
            crate::bytecode::TypeEntry {
                kind: crate::bytecode::TypeKind::Primitive,
                name_idx: None,
                data: crate::bytecode::TypeData::Primitive {
                    prim_id: 24,
                    max_length: 20,
                },
            },
            primitive_type(8, 0),
        ],
    };
    assert_eq!(target.location, MemoryLocation::Global);
    module.refs.push(super::super::VmRef::Global {
        offset: target.offset,
        path: target.path,
    });
    module.ref_types.insert(0, 1);
}
