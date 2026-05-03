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
            direction: 1,
            default_const_idx: None,
        },
        VmParamMeta {
            name: SmolStr::new("ACC"),
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
            direction: 0,
            default_const_idx: None,
        },
        VmParamMeta {
            name: SmolStr::new("OUT"),
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
                direction: 0,
                default_const_idx: None,
            },
            VmParamMeta {
                name: SmolStr::new("B"),
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
                direction: 0,
                default_const_idx: None,
            },
            VmParamMeta {
                name: SmolStr::new("B"),
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
