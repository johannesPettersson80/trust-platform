#[test]
fn dispatch_native_stdlib_runtime_clock_accepts_zero_args_only() {
    let mut runtime = Runtime::new();
    let mut frame = empty_caller_frame();

    let value = dispatch_stdlib(&mut runtime, &mut frame, "TIME", &[])
        .expect("TIME with no args should read runtime clock");
    assert!(matches!(value, Value::Time(_)));

    let err = dispatch_stdlib(
        &mut runtime,
        &mut frame,
        "TIME",
        &[expr_arg(None, Value::DInt(1))],
    )
    .expect_err("TIME rejects args");
    assert_invalid_argument_count(err, 0, 1);
}

#[test]
fn dispatch_native_split_date_positional_writes_outputs_and_checks_arity() {
    let mut runtime = Runtime::new();
    let mut frame = empty_caller_frame();
    let input = Value::Date(DateValue::new(0));
    let (year, month, day) = time::split_date(&input, runtime.profile).expect("split date");
    let args = vec![
        expr_arg(None, input.clone()),
        output_target_arg(&mut runtime, None, "YEAR"),
        output_target_arg(&mut runtime, None, "MONTH"),
        output_target_arg(&mut runtime, None, "DAY"),
    ];

    let value = dispatch_stdlib(&mut runtime, &mut frame, "SPLIT_DATE", &args)
        .expect("SPLIT_DATE positional");

    assert_eq!(value, Value::Null);
    assert_eq!(
        runtime.storage.get_global("YEAR"),
        Some(&Value::DInt(year as i32))
    );
    assert_eq!(
        runtime.storage.get_global("MONTH"),
        Some(&Value::DInt(month as i32))
    );
    assert_eq!(
        runtime.storage.get_global("DAY"),
        Some(&Value::DInt(day as i32))
    );

    let short_args = vec![
        expr_arg(None, input),
        output_target_arg(&mut runtime, None, "YEAR_SHORT"),
        output_target_arg(&mut runtime, None, "MONTH_SHORT"),
    ];
    let err = dispatch_stdlib(&mut runtime, &mut frame, "SPLIT_DATE", &short_args)
        .expect_err("SPLIT_DATE requires all positional outputs");
    assert_invalid_argument_count(err, 4, 3);
}

#[test]
fn dispatch_native_split_named_variants_write_outputs() {
    let profile = Runtime::new().profile;
    let cases = vec![
        {
            let input = Value::Tod(TimeOfDayValue::new(0));
            let (hour, minute, second, millis) =
                time::split_tod(&input, profile).expect("split tod");
            (
                "SPLIT_TOD",
                input,
                vec!["HOUR", "MINUTE", "SECOND", "MILLISECOND"],
                vec![hour, minute, second, millis],
            )
        },
        {
            let input = Value::LTod(LTimeOfDayValue::new(0));
            let (hour, minute, second, millis) = time::split_ltod(&input).expect("split ltod");
            (
                "SPLIT_LTOD",
                input,
                vec!["HOUR", "MINUTE", "SECOND", "MILLISECOND"],
                vec![hour, minute, second, millis],
            )
        },
        {
            let input = Value::Dt(DateTimeValue::new(0));
            let (year, month, day, hour, minute, second, millis) =
                time::split_dt(&input, profile).expect("split dt");
            (
                "SPLIT_DT",
                input,
                vec![
                    "YEAR",
                    "MONTH",
                    "DAY",
                    "HOUR",
                    "MINUTE",
                    "SECOND",
                    "MILLISECOND",
                ],
                vec![year, month, day, hour, minute, second, millis],
            )
        },
        {
            let input = Value::Ldt(LDateTimeValue::new(0));
            let (year, month, day, hour, minute, second, millis) =
                time::split_ldt(&input).expect("split ldt");
            (
                "SPLIT_LDT",
                input,
                vec![
                    "YEAR",
                    "MONTH",
                    "DAY",
                    "HOUR",
                    "MINUTE",
                    "SECOND",
                    "MILLISECOND",
                ],
                vec![year, month, day, hour, minute, second, millis],
            )
        },
    ];

    for (name, input, output_names, expected) in cases {
        let mut runtime = Runtime::new();
        let mut frame = empty_caller_frame();
        let mut args = vec![expr_arg(Some("IN"), input)];
        for output_name in &output_names {
            args.push(output_target_arg(
                &mut runtime,
                Some(output_name),
                output_name,
            ));
        }

        let value = dispatch_stdlib(&mut runtime, &mut frame, name, &args)
            .unwrap_or_else(|err| panic!("{name} failed: {err:?}"));

        assert_eq!(value, Value::Null);
        for (output_name, expected) in output_names.iter().zip(expected) {
            assert_eq!(
                runtime.storage.get_global(output_name),
                Some(&Value::DInt(expected as i32)),
                "{name}.{output_name}"
            );
        }
    }
}

#[test]
fn dispatch_native_split_named_rejects_duplicate_output_name() {
    let mut runtime = Runtime::new();
    let mut frame = empty_caller_frame();
    let args = vec![
        expr_arg(Some("IN"), Value::Date(DateValue::new(0))),
        output_target_arg(&mut runtime, Some("YEAR"), "YEAR_DUP"),
        output_target_arg(&mut runtime, Some("MONTH"), "MONTH_DUP_A"),
        output_target_arg(&mut runtime, Some("MONTH"), "MONTH_DUP_B"),
    ];

    let err = dispatch_stdlib(&mut runtime, &mut frame, "SPLIT_DATE", &args)
        .expect_err("duplicate split output should fail");

    assert!(matches!(
        err,
        VmTrap::Runtime(RuntimeError::InvalidArgumentName(name)) if name == "MONTH"
    ));
}

#[test]
fn resolve_native_symbol_specs_caches_resolved_function_id() {
    let mut specs = vec![
        preparse_native_symbol_spec(&SmolStr::new("Add|E:a")),
        preparse_native_symbol_spec(&SmolStr::new("Len|E:in")),
    ];
    let mut function_ids = HashMap::new();
    function_ids.insert(SmolStr::new("ADD"), 7);

    super::resolve_native_symbol_specs(&mut specs, &function_ids);

    match &specs[0] {
        VmNativeSymbolSpec::Parsed {
            resolved_function_pou_id,
            ..
        } => assert_eq!(*resolved_function_pou_id, Some(7)),
        VmNativeSymbolSpec::ParseError(err) => panic!("unexpected parse error: {err}"),
    }
    match &specs[1] {
        VmNativeSymbolSpec::Parsed {
            resolved_function_pou_id,
            ..
        } => assert_eq!(*resolved_function_pou_id, None),
        VmNativeSymbolSpec::ParseError(err) => panic!("unexpected parse error: {err}"),
    }
}

#[test]
fn bind_vm_call_arguments_rejects_too_many_positional_arguments() {
    let mut runtime = Runtime::new();
    let (module, pou_id) = manual_vm_function_module(
        "DoWork",
        vec![VmParamMeta {
            name: SmolStr::new("IN"),
            direction: 0,
            default_const_idx: None,
        }],
        false,
    );
    let err = super::bind_vm_call_arguments(
        &mut runtime,
        &module,
        &empty_caller_frame(),
        pou_id,
        &[
            expr_arg(None, Value::DInt(1)),
            expr_arg(None, Value::DInt(2)),
        ],
    )
    .expect_err("extra positional argument should fail");

    match err {
        super::VmTrap::InvalidNativeCall(message) => {
            assert!(message.contains("too many positional arguments"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn bind_vm_call_arguments_keeps_omitted_middle_named_input_as_null() {
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
            VmParamMeta {
                name: SmolStr::new("C"),
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
            expr_arg(Some("A"), Value::DInt(1)),
            expr_arg(Some("C"), Value::DInt(3)),
        ],
    )
    .expect("named omission should bind remaining params");

    assert!(out_bindings.is_empty());
    assert_eq!(locals, vec![Value::DInt(1), Value::Null, Value::DInt(3)]);
}

#[test]
fn bind_stdlib_named_values_rejects_duplicate_named_argument() {
    let mut runtime = Runtime::new();
    let params = crate::stdlib::StdParams::Fixed(vec![SmolStr::new("IN"), SmolStr::new("N")]);
    let err = super::bind_stdlib_named_values(
        &mut runtime,
        &empty_caller_frame(),
        &params,
        &[
            expr_arg(Some("IN"), Value::DInt(1)),
            expr_arg(Some("IN"), Value::DInt(2)),
        ],
    )
    .expect_err("duplicate stdlib named arg should fail");

    assert!(matches!(
        err,
        super::VmTrap::Runtime(RuntimeError::InvalidArgumentName(name))
            if name == SmolStr::new("IN")
    ));
}

#[test]
fn dispatch_native_stdlib_binds_fixed_and_variadic_positional_values() {
    let mut runtime = Runtime::new();
    let mut frame = empty_caller_frame();

    let selected = dispatch_stdlib(
        &mut runtime,
        &mut frame,
        "SEL",
        &[
            expr_arg(None, Value::Bool(true)),
            expr_arg(None, Value::Int(4)),
            expr_arg(None, Value::Int(7)),
        ],
    )
    .expect("SEL positional binding");
    assert_eq!(selected, Value::Int(7));

    let sum = dispatch_stdlib(
        &mut runtime,
        &mut frame,
        "ADD",
        &[
            expr_arg(None, Value::DInt(2)),
            expr_arg(None, Value::DInt(3)),
        ],
    )
    .expect("ADD variadic positional binding");
    assert_eq!(sum, Value::DInt(5));

    let sum_three = dispatch_stdlib(
        &mut runtime,
        &mut frame,
        "ADD",
        &[
            expr_arg(None, Value::DInt(1)),
            expr_arg(None, Value::DInt(2)),
            expr_arg(None, Value::DInt(3)),
        ],
    )
    .expect("ADD accepts more than the variadic minimum");
    assert_eq!(sum_three, Value::DInt(6));

    let err = dispatch_stdlib(
        &mut runtime,
        &mut frame,
        "ADD",
        &[expr_arg(None, Value::DInt(2))],
    )
    .expect_err("ADD requires the variadic minimum");
    assert_invalid_argument_count(err, 2, 1);

    let err = dispatch_stdlib(
        &mut runtime,
        &mut frame,
        "MUX",
        &[
            expr_arg(None, Value::DInt(0)),
            expr_arg(None, Value::DInt(10)),
        ],
    )
    .expect_err("MUX requires fixed K plus the variadic minimum");
    assert_invalid_argument_count(err, 3, 2);
}

#[test]
fn bind_stdlib_positional_values_enforces_fixed_plus_variadic_minimum() {
    let mut runtime = Runtime::new();
    let params = StdParams::Variadic {
        fixed: vec![SmolStr::new("K")],
        prefix: SmolStr::new("IN"),
        start: 0,
        min: 2,
    };
    let err = super::bind_stdlib_positional_values(
        &mut runtime,
        &empty_caller_frame(),
        &params,
        &[
            expr_arg(None, Value::DInt(0)),
            expr_arg(None, Value::DInt(10)),
        ],
    )
    .expect_err("fixed plus variadic minimum must be checked before stdlib dispatch");

    assert_invalid_argument_count(err, 3, 2);
}

#[test]
fn bind_stdlib_named_values_fixed_reorders_by_parameter_order() {
    let mut runtime = Runtime::new();
    let params = StdParams::Fixed(vec![SmolStr::new("IN"), SmolStr::new("N")]);
    let values = super::bind_stdlib_named_values(
        &mut runtime,
        &empty_caller_frame(),
        &params,
        &[
            expr_arg(Some("N"), Value::DInt(2)),
            expr_arg(Some("IN"), Value::DInt(1)),
        ],
    )
    .expect("fixed named stdlib binding");

    assert_eq!(values, vec![Value::DInt(1), Value::DInt(2)]);
}

#[test]
fn bind_stdlib_named_values_variadic_reorders_suffixes() {
    let mut runtime = Runtime::new();
    let params = StdParams::Variadic {
        fixed: Vec::new(),
        prefix: SmolStr::new("IN"),
        start: 1,
        min: 2,
    };
    let values = super::bind_stdlib_named_values(
        &mut runtime,
        &empty_caller_frame(),
        &params,
        &[
            expr_arg(Some("IN2"), Value::DInt(20)),
            expr_arg(Some("IN1"), Value::DInt(10)),
        ],
    )
    .expect("variadic named suffix binding");

    assert_eq!(values, vec![Value::DInt(10), Value::DInt(20)]);
}

#[test]
fn bind_stdlib_named_values_variadic_reports_exact_count_edges() {
    let mut runtime = Runtime::new();
    let params = StdParams::Variadic {
        fixed: vec![SmolStr::new("K")],
        prefix: SmolStr::new("IN"),
        start: 0,
        min: 2,
    };

    let missing_fixed = super::bind_stdlib_named_values(
        &mut runtime,
        &empty_caller_frame(),
        &params,
        &[
            expr_arg(Some("IN0"), Value::DInt(10)),
            expr_arg(Some("IN1"), Value::DInt(11)),
        ],
    )
    .expect_err("missing fixed variadic arg should fail");
    assert_invalid_argument_count(missing_fixed, 3, 2);

    let too_few_variadic = super::bind_stdlib_named_values(
        &mut runtime,
        &empty_caller_frame(),
        &params,
        &[
            expr_arg(Some("K"), Value::DInt(1)),
            expr_arg(Some("IN0"), Value::DInt(10)),
        ],
    )
    .expect_err("variadic minimum should be enforced");
    assert_invalid_argument_count(too_few_variadic, 3, 2);

    let hole = super::bind_stdlib_named_values(
        &mut runtime,
        &empty_caller_frame(),
        &params,
        &[
            expr_arg(Some("K"), Value::DInt(1)),
            expr_arg(Some("IN0"), Value::DInt(10)),
            expr_arg(Some("IN2"), Value::DInt(12)),
        ],
    )
    .expect_err("variadic suffix hole should fail");
    assert_invalid_argument_count(hole, 4, 3);
}

#[test]
fn bind_stdlib_named_values_variadic_rejects_hole() {
    let mut runtime = Runtime::new();
    let params = crate::stdlib::StdParams::Variadic {
        fixed: vec![SmolStr::new("IN")],
        prefix: SmolStr::new("IN"),
        start: 2,
        min: 2,
    };
    let err = super::bind_stdlib_named_values(
        &mut runtime,
        &empty_caller_frame(),
        &params,
        &[
            expr_arg(Some("IN"), Value::DInt(1)),
            expr_arg(Some("IN2"), Value::DInt(2)),
            expr_arg(Some("IN4"), Value::DInt(4)),
        ],
    )
    .expect_err("variadic hole should fail");

    assert!(matches!(
        err,
        super::VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
            expected: 4,
            got: 3,
        })
    ));
}
