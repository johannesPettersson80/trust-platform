// A caller without an error channel (the infallible enqueue and force methods) reports a refused
// request as write_rejected with the stable text, and queues nothing. Only the bounded `try_*`
// methods return the refusal itself.

const REFUSAL_SOURCE: &str = r#"
VAR_GLOBAL
    Counter : INT := 0;
END_VAR
PROGRAM MainProgram
END_PROGRAM
"#;

#[test]
fn infallible_enqueue_on_a_full_queue_reports_write_rejected_and_queues_nothing() {
    use crate::value::Value;
    let state = hmi_test_state(REFUSAL_SOURCE);
    for index in 0..1_024 {
        state
            .debug
            .try_enqueue_global_write(format!("v{index}"), Value::LInt(1))
            .expect("a filler fits the bound");
    }

    state.debug.enqueue_global_write("extra", Value::LInt(2));

    let rejections: Vec<String> = state
        .debug
        .drain_runtime_events()
        .into_iter()
        .filter_map(|event| match event {
            crate::debug::RuntimeEvent::WriteRejected { target, error, .. } => {
                Some(format!("{target}: {error}"))
            }
            _ => None,
        })
        .collect();
    assert_eq!(rejections, vec!["extra: request queue full".to_string()]);
    let writes = state.debug.drain_var_writes();
    assert_eq!(writes.len(), 1_024, "the refused write must not be queued");
    assert!(writes.iter().all(|write| !matches!(
        &write.target,
        PendingVarTarget::Global(name) if name == "extra"
    )));
}

#[test]
fn infallible_force_on_a_full_queue_reports_write_rejected_and_forces_nothing() {
    use crate::value::Value;
    let state = hmi_test_state(REFUSAL_SOURCE);
    for index in 0..1_024 {
        state
            .debug
            .try_force_global(format!("f{index}"), Value::LInt(1))
            .expect("a filler fits the bound");
    }

    state.debug.force_global("extra", Value::LInt(2));

    let rejections = state
        .debug
        .drain_runtime_events()
        .into_iter()
        .filter(|event| {
            matches!(
                event,
                crate::debug::RuntimeEvent::WriteRejected { target, error, .. }
                    if target.as_str() == "extra" && error == "request queue full"
            )
        })
        .count();
    assert_eq!(rejections, 1, "one write_rejected for the refused force");
    let forced = state.debug.forced_snapshot().vars;
    assert_eq!(forced.len(), 1_024, "the refused force must not be applied");
    assert!(forced.iter().all(|entry| !matches!(
        &entry.target,
        crate::debug::ForcedVarTarget::Global(name) if name == "extra"
    )));
}
