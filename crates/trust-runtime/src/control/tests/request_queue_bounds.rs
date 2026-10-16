// Specification 11 ("request queue full") and specification 34 section 4.3.1: each bounded control
// queue holds 1,024 entries. A request that would add an entry past the bound is refused with
// `request queue full` and changes nothing; re-queuing a target that is already queued is never
// refused.

const QUEUE_BOUNDS_SOURCE: &str = r#"
VAR_GLOBAL
    Counter : INT := 0;
END_VAR
PROGRAM MainProgram
END_PROGRAM
"#;

fn queue_bounds_request(
    state: &ControlState,
    id: u64,
    kind: &str,
    params: serde_json::Value,
) -> ControlResponse {
    handle_request_value(
        json!({"id": id, "type": kind, "params": params}),
        state,
        None,
    )
}

#[test]
fn io_write_queue_refuses_the_request_past_1024_and_queues_nothing() {
    let state = hmi_test_state(QUEUE_BOUNDS_SOURCE);
    for id in 1..=1_024 {
        let response = queue_bounds_request(
            &state,
            id,
            "io.write",
            json!({"address": "%MW0", "value": "1"}),
        );
        assert!(
            response.ok,
            "io.write {id} must queue: {:?}",
            response.error
        );
    }

    let refused = queue_bounds_request(
        &state,
        1_025,
        "io.write",
        json!({"address": "%MW0", "value": "2"}),
    );
    assert!(!refused.ok, "the 1,025th io.write must be refused");
    assert_eq!(refused.error.as_deref(), Some("request queue full"));
    assert_eq!(
        state.debug.drain_io_writes().len(),
        1_024,
        "the refused write must not be queued"
    );
}

#[test]
fn io_force_queue_refuses_a_new_address_past_1024_and_replaces_a_forced_one() {
    use crate::value::Value;
    let state = hmi_test_state(QUEUE_BOUNDS_SOURCE);
    for word in 0..1_024 {
        let address = format!("%MW{word}");
        let response = queue_bounds_request(
            &state,
            word + 1,
            "io.force",
            json!({"address": address, "value": "1"}),
        );
        assert!(
            response.ok,
            "{address} must be forced: {:?}",
            response.error
        );
    }

    let refused = queue_bounds_request(
        &state,
        2_000,
        "io.force",
        json!({"address": "%MW1024", "value": "1"}),
    );
    assert_eq!(refused.error.as_deref(), Some("request queue full"));

    let replaced = queue_bounds_request(
        &state,
        2_001,
        "io.force",
        json!({"address": "%MW0", "value": "2"}),
    );
    assert!(
        replaced.ok,
        "re-forcing a forced address is never refused: {:?}",
        replaced.error
    );

    let refused_address = crate::io::IoAddress::parse("%MW1024").expect("address parses");
    let first_address = crate::io::IoAddress::parse("%MW0").expect("address parses");
    let forced = state.debug.forced_snapshot().io;
    assert_eq!(forced.len(), 1_024, "the refused force must not be applied");
    assert!(
        forced
            .iter()
            .all(|(address, _)| *address != refused_address),
        "%MW1024 must not be forced"
    );
    let first_value = forced
        .iter()
        .find(|(address, _)| *address == first_address)
        .map(|(_, value)| value.clone());
    assert_eq!(
        first_value,
        Some(Value::LInt(2)),
        "the re-force replaces %MW0 in place"
    );
}

#[test]
fn set_queue_refuses_a_new_target_past_1024_and_replaces_a_queued_one() {
    use crate::value::Value;
    let state = hmi_test_state(QUEUE_BOUNDS_SOURCE);
    for index in 0..1_024 {
        let response = queue_bounds_request(
            &state,
            index + 1,
            "set",
            json!({"target": format!("global:v{index}"), "value": "1"}),
        );
        assert!(response.ok, "set {index} must queue: {:?}", response.error);
    }

    let refused = queue_bounds_request(
        &state,
        2_000,
        "set",
        json!({"target": "global:v1024", "value": "1"}),
    );
    assert_eq!(refused.error.as_deref(), Some("request queue full"));

    let replaced = queue_bounds_request(
        &state,
        2_001,
        "set",
        json!({"target": "global:v0", "value": "2"}),
    );
    assert!(
        replaced.ok,
        "re-setting a queued target is never refused: {:?}",
        replaced.error
    );

    let writes = state.debug.drain_var_writes();
    assert_eq!(writes.len(), 1_024, "the refused set must not be queued");
    assert!(
        writes.iter().all(|write| !matches!(
            &write.target,
            PendingVarTarget::Global(name) if name == "v1024"
        )),
        "global:v1024 must not be queued"
    );
    let first = writes
        .iter()
        .find(|write| matches!(&write.target, PendingVarTarget::Global(name) if name == "v0"))
        .expect("global:v0 stays queued");
    assert_eq!(
        first.value,
        Value::LInt(2),
        "the re-set replaces v0 in place"
    );
}

#[test]
fn var_force_queue_refuses_a_new_target_past_1024_and_replaces_a_forced_one() {
    use crate::value::Value;
    let state = hmi_test_state(QUEUE_BOUNDS_SOURCE);
    for index in 0..1_024 {
        let response = queue_bounds_request(
            &state,
            index + 1,
            "var.force",
            json!({"target": format!("global:f{index}"), "value": "1"}),
        );
        assert!(
            response.ok,
            "var.force {index} must apply: {:?}",
            response.error
        );
    }

    let refused = queue_bounds_request(
        &state,
        2_000,
        "var.force",
        json!({"target": "global:f1024", "value": "1"}),
    );
    assert_eq!(refused.error.as_deref(), Some("request queue full"));

    let replaced = queue_bounds_request(
        &state,
        2_001,
        "var.force",
        json!({"target": "global:f0", "value": "2"}),
    );
    assert!(
        replaced.ok,
        "re-forcing a forced target is never refused: {:?}",
        replaced.error
    );

    let forced = state.debug.forced_snapshot().vars;
    assert_eq!(forced.len(), 1_024, "the refused force must not be applied");
    assert!(
        forced.iter().all(|entry| !matches!(
            &entry.target,
            crate::debug::ForcedVarTarget::Global(name) if name == "f1024"
        )),
        "global:f1024 must not be forced"
    );
    let first = forced
        .iter()
        .find(|entry| {
            matches!(
                &entry.target,
                crate::debug::ForcedVarTarget::Global(name) if name == "f0"
            )
        })
        .expect("global:f0 stays forced");
    assert_eq!(
        first.value,
        Value::LInt(2),
        "the re-force replaces f0 in place"
    );
}
