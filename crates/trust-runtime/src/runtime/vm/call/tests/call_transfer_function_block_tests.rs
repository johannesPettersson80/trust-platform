/// Spec 12 §7.7: a later output target that is rejected leaves the earlier input field unchanged.
#[test]
fn call_transfer_rejected_later_output_target_keeps_earlier_input_field() {
    let mut runtime = Runtime::new();
    let instance = runtime.storage.create_instance("FB");
    assert!(runtime
        .storage
        .set_instance_var(instance, "A", Value::DInt(1)));
    runtime.storage.set_global("S", Value::String("OLD".into()));
    let target = runtime
        .storage
        .ref_for_global("S")
        .expect("string output target");
    let (module, pou_id) = manual_vm_function_block_module(vec![
        VmParamMeta {
            name: SmolStr::new("A"),
            type_id: 0,
            direction: 0,
            default_const_idx: None,
        },
        VmParamMeta {
            name: SmolStr::new("B"),
            type_id: 0,
            direction: 1,
            default_const_idx: None,
        },
    ]);

    let Err(error) = super::bind_vm_function_block_arguments(
        &mut runtime,
        &module,
        &empty_caller_frame(),
        pou_id,
        instance,
        &[
            expr_arg(Some("A"), Value::DInt(5)),
            target_arg(Some("B"), target),
        ],
    ) else {
        panic!("an untyped string output target must reject the whole binding");
    };

    assert!(matches!(error, VmTrap::Runtime(RuntimeError::TypeMismatch)));
    assert_eq!(instance_field(&runtime, instance, "A"), Value::DInt(1));
    assert_eq!(
        runtime.storage.get_global("S"),
        Some(&Value::String("OLD".into()))
    );
}
