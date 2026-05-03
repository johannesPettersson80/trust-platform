#[test]
fn bind_vm_function_block_arguments_skips_omitted_out_without_field_resolution() {
    let mut runtime = Runtime::new();
    let instance = runtime.storage.create_instance("FB");
    let (module, pou_id) = manual_vm_function_block_module(vec![VmParamMeta {
        name: SmolStr::new("OUT"),
        direction: 1,
        default_const_idx: None,
    }]);
    let caller_frame = empty_caller_frame();

    let bindings = super::bind_vm_function_block_arguments(
        &mut runtime,
        &module,
        &caller_frame,
        pou_id,
        instance,
        &[],
    )
    .expect("omitted OUT should skip field binding resolution");

    assert!(bindings.is_empty());
}

#[test]
fn bind_vm_function_block_arguments_skips_omitted_inout_without_field_resolution() {
    let mut runtime = Runtime::new();
    let instance = runtime.storage.create_instance("FB");
    let (module, pou_id) = manual_vm_function_block_module(vec![VmParamMeta {
        name: SmolStr::new("ACC"),
        direction: 2,
        default_const_idx: None,
    }]);
    let caller_frame = empty_caller_frame();

    let bindings = super::bind_vm_function_block_arguments(
        &mut runtime,
        &module,
        &caller_frame,
        pou_id,
        instance,
        &[],
    )
    .expect("omitted IN_OUT should skip field binding resolution");

    assert!(bindings.is_empty());
}

#[test]
fn resolve_named_arg_index_prefers_in_order_next_argument() {
    let args = vec![
        super::VmNativeArg {
            name: Some(SmolStr::new("ENABLE")),
            value: super::VmNativeArgValue::Expr(Value::Bool(true)),
        },
        super::VmNativeArg {
            name: Some(SmolStr::new("VALUE")),
            value: super::VmNativeArgValue::Expr(Value::DInt(1)),
        },
    ];
    let consumed = vec![false, false];
    let mut ordered_named_index = 0usize;

    let first = super::resolve_named_arg_index(
        &args,
        &consumed,
        &SmolStr::new("Enable"),
        &mut ordered_named_index,
    );
    let second = super::resolve_named_arg_index(
        &args,
        &consumed,
        &SmolStr::new("Value"),
        &mut ordered_named_index,
    );

    assert_eq!(first, Some(0));
    assert_eq!(second, Some(1));
}

#[test]
fn resolve_named_arg_index_handles_omitted_middle_parameter() {
    let args = vec![
        super::VmNativeArg {
            name: Some(SmolStr::new("ENABLE")),
            value: super::VmNativeArgValue::Expr(Value::Bool(true)),
        },
        super::VmNativeArg {
            name: Some(SmolStr::new("VALUE")),
            value: super::VmNativeArgValue::Expr(Value::DInt(1)),
        },
    ];
    let consumed = vec![false, false];
    let mut ordered_named_index = 0usize;

    let first = super::resolve_named_arg_index(
        &args,
        &consumed,
        &SmolStr::new("Enable"),
        &mut ordered_named_index,
    );
    let missing = super::resolve_named_arg_index(
        &args,
        &consumed,
        &SmolStr::new("Increment"),
        &mut ordered_named_index,
    );
    let second = super::resolve_named_arg_index(
        &args,
        &consumed,
        &SmolStr::new("Value"),
        &mut ordered_named_index,
    );

    assert_eq!(first, Some(0));
    assert_eq!(missing, None);
    assert_eq!(second, Some(1));
}

#[test]
fn resolve_named_arg_index_skips_consumed_prefix_and_stops_at_end() {
    let args = vec![
        expr_arg(Some("OTHER"), Value::DInt(1)),
        expr_arg(Some("ENABLE"), Value::Bool(false)),
        expr_arg(Some("VALUE"), Value::DInt(2)),
    ];
    let consumed = vec![false, false, true];
    let mut ordered_named_index = 2usize;
    let exhausted = super::resolve_named_arg_index(
        &args,
        &consumed,
        &SmolStr::new("Value"),
        &mut ordered_named_index,
    );

    assert_eq!(exhausted, None);
    assert_eq!(ordered_named_index, 3);
}

#[test]
fn resolve_named_arg_index_advances_nonzero_ordered_match() {
    let args = vec![
        expr_arg(Some("ENABLE"), Value::Bool(true)),
        expr_arg(Some("VALUE"), Value::DInt(1)),
    ];
    let consumed = vec![true, false];
    let mut ordered_named_index = 1usize;

    let value = super::resolve_named_arg_index(
        &args,
        &consumed,
        &SmolStr::new("Value"),
        &mut ordered_named_index,
    );

    assert_eq!(value, Some(1));
    assert_eq!(ordered_named_index, 2);
}

#[test]
fn resolve_named_arg_index_falls_back_for_out_of_order_named_arguments() {
    let args = vec![
        super::VmNativeArg {
            name: Some(SmolStr::new("VALUE")),
            value: super::VmNativeArgValue::Expr(Value::DInt(1)),
        },
        super::VmNativeArg {
            name: Some(SmolStr::new("ENABLE")),
            value: super::VmNativeArgValue::Expr(Value::Bool(true)),
        },
    ];
    let mut consumed = vec![false, false];
    let mut ordered_named_index = 0usize;

    let enable = super::resolve_named_arg_index(
        &args,
        &consumed,
        &SmolStr::new("Enable"),
        &mut ordered_named_index,
    );
    consumed[enable.expect("enable index")] = true;
    let value = super::resolve_named_arg_index(
        &args,
        &consumed,
        &SmolStr::new("Value"),
        &mut ordered_named_index,
    );

    assert_eq!(enable, Some(1));
    assert_eq!(value, Some(0));
}

#[test]
fn unpack_native_call_payload_preserves_receiver_and_argument_order() {
    let mut operand_stack = OperandStack::default();
    let target_ref = ValueRef {
        location: MemoryLocation::Global,
        offset: 3,
        path: RefPath::new(),
    };
    operand_stack
        .push(Value::Instance(crate::memory::InstanceId(7)))
        .expect("push receiver");
    operand_stack.push(Value::DInt(11)).expect("push expr arg");
    operand_stack
        .push(Value::Reference(Some(target_ref.clone())))
        .expect("push target arg");

    let (receiver, args) = super::unpack_native_call_payload(
        &mut operand_stack,
        &[
            VmNativeArgSpec {
                name: Some(SmolStr::new("lhs")),
                is_target: false,
            },
            VmNativeArgSpec {
                name: Some(SmolStr::new("out")),
                is_target: true,
            },
        ],
        1,
    )
    .expect("decode payload");

    assert_eq!(
        receiver,
        Some(Value::Instance(crate::memory::InstanceId(7)))
    );
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].name.as_deref(), Some("lhs"));
    assert!(matches!(
        args[0].value,
        super::VmNativeArgValue::Expr(Value::DInt(11))
    ));
    assert_eq!(args[1].name.as_deref(), Some("out"));
    match &args[1].value {
        super::VmNativeArgValue::Target(reference) => assert_eq!(reference, &target_ref),
        super::VmNativeArgValue::Expr(_) => panic!("expected target arg"),
    }
}

#[test]
fn preparse_native_symbol_spec_parses_named_and_target_args() {
    let entry = preparse_native_symbol_spec(&SmolStr::new("Add|E:a|T:out"));
    match entry {
        VmNativeSymbolSpec::Parsed {
            target_name,
            normalized_target_name,
            resolved_function_pou_id,
            conversion_spec,
            arg_specs,
        } => {
            assert_eq!(target_name, SmolStr::new("Add"));
            assert_eq!(normalized_target_name, SmolStr::new("ADD"));
            assert_eq!(resolved_function_pou_id, None);
            assert!(conversion_spec.is_none());
            assert_eq!(arg_specs.len(), 2);
            assert_eq!(arg_specs[0].name.as_deref(), Some("a"));
            assert!(!arg_specs[0].is_target);
            assert_eq!(arg_specs[1].name.as_deref(), Some("out"));
            assert!(arg_specs[1].is_target);
        }
        VmNativeSymbolSpec::ParseError(err) => {
            panic!("unexpected parse error: {err}");
        }
    }
}

#[test]
fn preparse_native_symbol_spec_caches_conversion_spec() {
    let entry = preparse_native_symbol_spec(&SmolStr::new("INT_TO_UDINT|E:IN"));
    match entry {
        VmNativeSymbolSpec::Parsed {
            normalized_target_name,
            conversion_spec,
            ..
        } => {
            assert_eq!(normalized_target_name, SmolStr::new("INT_TO_UDINT"));
            assert!(conversion_spec.is_some());
        }
        VmNativeSymbolSpec::ParseError(err) => {
            panic!("unexpected parse error: {err}");
        }
    }
}

#[test]
fn bind_conversion_value_accepts_positional_and_named_in_only() {
    let mut runtime = Runtime::new();
    let frame = empty_caller_frame();

    let positional = [expr_arg(None, Value::DInt(7))];
    assert_eq!(
        super::bind_conversion_value(&mut runtime, &frame, &positional).unwrap(),
        Value::DInt(7)
    );

    let named = [expr_arg(Some("in"), Value::DInt(8))];
    assert_eq!(
        super::bind_conversion_value(&mut runtime, &frame, &named).unwrap(),
        Value::DInt(8)
    );

    let wrong_name = [expr_arg(Some("OUT"), Value::DInt(9))];
    let err = super::bind_conversion_value(&mut runtime, &frame, &wrong_name)
        .expect_err("conversion only accepts IN");
    assert!(matches!(
        err,
        VmTrap::Runtime(RuntimeError::InvalidArgumentName(name)) if name == "OUT"
    ));

    let mixed = [
        expr_arg(None, Value::DInt(1)),
        expr_arg(Some("IN"), Value::DInt(2)),
    ];
    let err = super::bind_conversion_value(&mut runtime, &frame, &mixed)
        .expect_err("mixed named and positional args should fail before arity");
    assert!(matches!(
        err,
        VmTrap::Runtime(RuntimeError::InvalidArgumentName(name)) if name == "<unnamed>"
    ));
}
