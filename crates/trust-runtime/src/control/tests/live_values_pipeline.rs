// The shared dispatcher answers `io.list`, `io.read`, `io.write`, `io.force`, `io.unforce`, `set`
// and `var.*` (specification 11, §6.8.1; specification 34 §4.3.1). These tests keep the host
// wiring the deleted copies proved: the snapshot slot's lock and the conversion of the hosted
// snapshot into the portable one (specification 34, ARCH-11).

const LIVE_VALUES_SOURCE: &str = r#"
PROGRAM Main
END_PROGRAM
"#;

fn live_values_result(response: ControlResponse) -> serde_json::Value {
    assert!(response.ok, "request failed: {:?}", response.error);
    response.into_result_value().expect("response result")
}

fn poison<T>(mutex: &Arc<Mutex<T>>) {
    let mutex = Arc::clone(mutex);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _guard = mutex.lock().expect("lock before poison");
        panic!("intentional contract-test poison");
    }));
    assert!(result.is_err(), "poison setup must panic");
}

#[test]
fn io_list_fails_closed_when_snapshot_lock_is_unavailable() {
    let state = hmi_test_state(LIVE_VALUES_SOURCE);
    poison(&state.io_snapshot);

    let response = handle_request_value(json!({"id": 3, "type": "io.list"}), &state, None);
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("I/O snapshot unavailable"));
}

#[test]
fn io_read_fails_closed_when_snapshot_lock_is_unavailable() {
    let state = hmi_test_state(LIVE_VALUES_SOURCE);
    poison(&state.io_snapshot);

    let response = handle_request_value(json!({"id": 4, "type": "io.read"}), &state, None);
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("I/O snapshot unavailable"));
}

#[test]
fn io_list_answers_the_hosted_snapshot_through_the_portable_one() {
    let state = hmi_test_state(LIVE_VALUES_SOURCE);
    let snapshot = crate::io::IoSnapshot {
        scan: Some(5),
        forced: Vec::new(),
        inputs: Vec::new(),
        outputs: vec![crate::io::IoSnapshotEntry {
            name: Some(SmolStr::new("Lamp")),
            address: crate::io::IoAddress::parse("%QX0.0").expect("address"),
            value_type: Some(trust_hir::TypeId::BOOL),
            value_type_name: None,
            value: crate::io::IoSnapshotValue::Value(crate::value::Value::Bool(true)),
            source: None,
        }],
        memory: Vec::new(),
    };
    *state.io_snapshot.lock().expect("snapshot lock") = Some(snapshot);

    let response = handle_request_value(json!({"id": 6, "type": "io.list"}), &state, None);
    assert!(response.ok, "request failed: {:?}", response.error);
    assert_eq!(
        response.result_value(),
        Some(json!({
            "scan": 5,
            "inputs": [],
            "outputs": [{
                "name": "Lamp",
                "address": "%QX0.0",
                "value": "Bool(true)",
                "forced": false,
                "valueType": "BOOL",
            }],
            "memory": [],
        }))
    );
}

#[test]
fn declared_retain_targets_keep_control_status_labels_and_apply_to_real_storage() {
    let source = "VAR_GLOBAL RETAIN kept : LINT := 1; END_VAR PROGRAM Main VAR RETAIN member : LINT := 2; END_VAR END_PROGRAM";
    let state = hmi_test_state(source);
    let mut runtime = crate::harness::CompileSession::from_source(source)
        .build_runtime()
        .unwrap();
    runtime.set_debug_control(state.debug.clone());
    assert_eq!(
        live_values_result(handle_request_value(
            json!({"id": 901, "type": "set", "params": {"target": "retain:kept", "value": "7"}}),
            &state,
            None
        )),
        json!({"status":"queued"})
    );
    assert_eq!(
        live_values_result(handle_request_value(
            json!({"id": 902, "type": "set", "params": {"target": "retain:@program/Main/member", "value": "8"}}),
            &state,
            None
        )),
        json!({"status":"queued"})
    );
    runtime.execute_cycle().unwrap();
    state.debug.refresh_snapshot_from_storage_with_abi(
        runtime.storage(),
        runtime.current_time(),
        runtime.prepared().storage_abi(),
    );
    assert_eq!(
        live_values_result(super::variable_handlers::handle_eval(
            903,
            Some(json!({"expr":"@program/Main/member"})),
            &state
        )),
        json!({"value":"LInt(8)"})
    );
    assert_eq!(
        runtime.storage().get_global("kept"),
        Some(&crate::value::Value::LInt(7))
    );
    assert!(runtime.storage().retain().is_empty());
    assert_eq!(
        live_values_result(handle_request_value(
            json!({"id": 904, "type": "var.force", "params": {"target": "retain:kept", "value": "9"}}),
            &state,
            None
        )),
        json!({"status":"forced"})
    );
    assert_eq!(
        live_values_result(handle_request_value(
            json!({"id": 905, "type": "var.forced"}),
            &state,
            None
        ))["vars"][0]["target"],
        "retain:kept"
    );
    runtime.execute_cycle().unwrap();
    assert_eq!(
        runtime.storage().get_global("kept"),
        Some(&crate::value::Value::LInt(9))
    );
    assert_eq!(
        live_values_result(handle_request_value(
            json!({"id": 906, "type": "var.unforce", "params": {"target": "retain:kept"}}),
            &state,
            None
        )),
        json!({"status":"released"})
    );
    runtime
        .write_global("kept", crate::value::Value::LInt(10))
        .unwrap();
    runtime.execute_cycle().unwrap();
    assert_eq!(
        runtime.storage().get_global("kept"),
        Some(&crate::value::Value::LInt(10))
    );
    assert!(!runtime.faulted());
}
