#[test]
fn debug_boundary_requests_fail_closed_for_stale_or_missing_names() {
    let source = r#"
PROGRAM Main
VAR
    drive : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);
    assert!(state.debug.snapshot().is_some(), "debug snapshot");

    let stale_frame = handle_request_value(
        json!({
            "id": 10,
            "type": "debug.scopes",
            "params": { "frame_id": 9999 }
        }),
        &state,
        None,
    );
    assert!(!stale_frame.ok, "stale frame must fail closed");
    assert!(
        stale_frame
            .error
            .as_deref()
            .is_some_and(|error| error.contains("unknown frame id")),
        "unexpected stale-frame error: {:?}",
        stale_frame.error
    );

    let unknown_reference = handle_request_value(
        json!({
            "id": 11,
            "type": "debug.variables",
            "params": { "variables_reference": 4242 }
        }),
        &state,
        None,
    );
    assert!(
        !unknown_reference.ok,
        "unknown variable handle must not return an empty successful list"
    );
    assert!(
        unknown_reference
            .error
            .as_deref()
            .is_some_and(|error| error.contains("unknown variables reference")),
        "unexpected variable-reference error: {:?}",
        unknown_reference.error
    );

    let unknown_eval = handle_request_value(
        json!({
            "id": 12,
            "type": "debug.evaluate",
            "params": { "expression": "not_declared" }
        }),
        &state,
        None,
    );
    assert!(!unknown_eval.ok, "unknown debug eval name must fail closed");
}

#[test]
fn debug_boundary_instance_values_use_type_names() {
    // Specification 13, "Runtime debug state, mutation, and projection invariants": an entry of the
    // instance list is named `<type>#<id>` and shows the public type name as its value and its
    // type, with no evaluate name and a handle of its own. A compiled program lists its program
    // instances in its `Instances` scope, each by the index of its symbol, its identity
    // (specification 44 §13.2). Worked: the one program instance `Main` is the one entry,
    // `Main#<index of the symbol MAIN>`, with value and type `Main` and the handle after the
    // scope's own. An instance holds no value, so `debug.evaluate` of the name `Main` in the
    // compiled form of version 1 (tag 2, the length 4 in two bytes little-endian, `Main`; base64
    // `AgQATWFpbg==`) is refused with `expr_invalid` and answers no result (specification 11
    // §6.13.2).
    // Wrong behaviour caught: an instance shown by its internal id (`Instance(..)`) instead of its
    // type name, an instance listed under another name or without its handle, and an evaluation
    // that makes a value of an instance.
    let source = r#"
PROGRAM Main
VAR
    drive : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);
    let snapshot = state.debug.snapshot().expect("debug snapshot");
    let main_symbol = {
        use trust_runtime_core::native::memory::ProgramMemory as _;
        snapshot
            .compiled
            .as_deref()
            .expect("the compiled program's state")
            .image()
            .find_symbol("MAIN")
            .expect("the symbol of the program instance Main")
            .0
    };

    let instances_ref = debugger_handle(
        &state.debug,
        crate::debug::dap::VariableHandle::CompiledInstances,
    );
    let variables = handle_request_value(
        json!({
            "id": 20,
            "type": "debug.variables",
            "params": { "variables_reference": instances_ref }
        }),
        &state,
        None,
    );
    assert!(
        variables.ok,
        "debug.variables should succeed: {:?}",
        variables.error
    );
    assert_eq!(
        variables.result.expect("a result")["variables"],
        json!([{
            "name": format!("Main#{main_symbol}"),
            "value": "Main",
            "type": "Main",
            "variablesReference": instances_ref + 1
        }]),
        "the instance shows its type name, never an internal id"
    );

    let evaluated = handle_request_value(
        json!({
            "id": 21,
            "type": "debug.evaluate",
            "params": { "expr_version": 1, "expr": "AgQATWFpbg==" }
        }),
        &state,
        None,
    );
    assert!(!evaluated.ok, "an instance evaluates to no value");
    assert_eq!(evaluated.error_code.as_deref(), Some("expr_invalid"));
    assert!(evaluated.result.is_none(), "{:?}", evaluated.result);
}

#[test]
fn debug_boundary_io_snapshot_poison_is_an_error() {
    let source = r#"
PROGRAM Main
VAR
    drive : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);
    assert!(state.debug.snapshot().is_some(), "debug snapshot");

    let poisoned = state.io_snapshot.clone();
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let _ = std::panic::catch_unwind(move || {
        let _guard = poisoned.lock().expect("lock before poison");
        panic!("poison debug I/O snapshot");
    });
    std::panic::set_hook(original_hook);

    let response = handle_request_value(
        json!({
            "id": 13,
            "type": "debug.variables",
            "params": { "variables_reference": 4242 }
        }),
        &state,
        None,
    );
    assert!(!response.ok, "poisoned I/O snapshot must fail closed");
    assert!(
        response
            .error
            .as_deref()
            .is_some_and(|error| error.contains("I/O snapshot unavailable")),
        "unexpected poisoned-lock error: {:?}",
        response.error
    );
}

#[test]
fn debug_boundary_variables_page_an_array_as_a_chip_does() {
    // Specification 11 §6.13.3, "Variable pages": the computer cuts a page with the same shared code
    // as a chip, so the same request lists the same entries. Worked from an array of 250 DINT
    // elements holding their index: a request that names neither `start` nor `count` lists the first
    // page, `[0]` to `[99]`; `start` 240 with `count` 100 lists the last ten, `[240]` to `[249]`,
    // named by their index in the whole array; `start` 100 alone lists `[100]` to `[199]`.
    // The array's handle is one the program's debugger hands out, as `debug.variables` lists from
    // the debugger's handles (specification 44 §13.2).
    // Wrong behaviour caught: a computer handler that drops `filter`, `start` and `count` (every
    // request would list all 250 elements), and a page whose elements are named from `[0]`.
    let source = r#"
PROGRAM Main
VAR
    drive : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);
    assert!(state.debug.snapshot().is_some(), "debug snapshot");
    let elements = (0..250).map(crate::value::Value::DInt).collect::<Vec<_>>();
    let array = crate::value::ArrayValue::from_untyped_parts(elements, vec![(0, 249)])
        .expect("an array of 250 elements");
    let reference = debugger_handle(
        &state.debug,
        crate::debug::dap::VariableHandle::Array(array),
    );
    let list = |params: serde_json::Value| {
        let response = handle_request_value(
            json!({"id": 30, "type": "debug.variables", "params": params}),
            &state,
            None,
        );
        assert!(response.ok, "debug.variables failed: {:?}", response.error);
        response.result.expect("a result")["variables"].clone()
    };
    let element = |index: u32| {
        json!({
            "name": format!("[{index}]"),
            "value": index.to_string(),
            "type": "DINT",
            "variablesReference": 0,
            "evaluateName": format!("[{index}]")
        })
    };

    let first = list(json!({"variables_reference": reference}));
    assert_eq!(first.as_array().map(Vec::len), Some(100));
    assert_eq!(first[0], element(0));
    assert_eq!(first[99], element(99));

    let last = list(json!({
        "variables_reference": reference,
        "filter": "indexed",
        "start": 240,
        "count": 100
    }));
    assert_eq!(last.as_array().map(Vec::len), Some(10));
    assert_eq!(last[0], element(240));
    assert_eq!(last[9], element(249));

    let from = list(json!({"variables_reference": reference, "start": 100}));
    assert_eq!(from.as_array().map(Vec::len), Some(100));
    assert_eq!(from[0], element(100));
    assert_eq!(from[99], element(199));
}

#[test]
fn debug_boundary_containers_report_their_indexed_and_named_counts() {
    // Specification 11 §6.13.3, "Variable pages": an entry whose value is an array reports
    // `indexedVariables` and one whose value is a structure reports `namedVariables`, the sizes of
    // the whole containers. Worked: a structure `Outer` with the member `values`, an array of three
    // DINT elements, and the member `inner`, a structure of one member; the computer lists them
    // with the handles that follow the structure's own, and the counts 3 and 1. The structure's
    // handle is one the program's debugger hands out, as `debug.variables` lists from the
    // debugger's handles (specification 44 §13.2).
    // Wrong behaviour caught: counts missing on the computer, and the two swapped.
    let source = r#"
PROGRAM Main
VAR
    drive : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);
    assert!(state.debug.snapshot().is_some(), "debug snapshot");
    let values = crate::value::ArrayValue::from_untyped_parts(
        vec![
            crate::value::Value::DInt(1),
            crate::value::Value::DInt(2),
            crate::value::Value::DInt(3),
        ],
        vec![(0, 2)],
    )
    .expect("an array of three elements");
    let inner = crate::value::StructValue::from_untyped_parts(
        "Inner".into(),
        IndexMap::from([("n".into(), crate::value::Value::DInt(1))]),
    );
    let outer = crate::value::StructValue::from_untyped_parts(
        "Outer".into(),
        IndexMap::from([
            (
                "values".into(),
                crate::value::Value::Array(Box::new(values)),
            ),
            ("inner".into(), crate::value::Value::Struct(inner.into())),
        ]),
    );
    let reference = debugger_handle(
        &state.debug,
        crate::debug::dap::VariableHandle::Struct(outer),
    );

    let response = handle_request_value(
        json!({
            "id": 31,
            "type": "debug.variables",
            "params": {"variables_reference": reference}
        }),
        &state,
        None,
    );

    assert!(response.ok, "debug.variables failed: {:?}", response.error);
    assert_eq!(
        response.result.expect("a result")["variables"],
        json!([
            {
                "name": "values",
                "value": "[3]",
                "type": "ARRAY",
                "variablesReference": reference + 1,
                "evaluateName": "values",
                "indexedVariables": 3
            },
            {
                "name": "inner",
                "value": "Inner {...}",
                "type": "Inner",
                "variablesReference": reference + 2,
                "evaluateName": "inner",
                "namedVariables": 1
            }
        ])
    );
}
