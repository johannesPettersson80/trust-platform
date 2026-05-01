#[test]
fn vm_enforces_execution_deadline() {
    let source = r#"
        PROGRAM Main
        WHILE TRUE DO
        END_WHILE;
        END_PROGRAM
    "#;
    let mut harness = vm_harness(source);
    harness
        .runtime_mut()
        .set_execution_deadline(Instant::now().checked_sub(StdDuration::from_millis(1)));
    let cycle = harness.cycle();
    assert!(
        cycle
            .errors
            .iter()
            .any(|err| matches!(err, RuntimeError::ExecutionTimeout)),
        "expected ExecutionTimeout, got {:?}",
        cycle.errors
    );
}

