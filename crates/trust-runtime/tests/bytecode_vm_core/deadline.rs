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

#[test]
fn vm_forward_only_instruction_stream_enforces_execution_deadline() {
    let source = r#"
        PROGRAM Main
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let mut body = vec![0x00; 4096];
    body.push(0x06);
    replace_main_body(&mut module, &body);

    let mut harness = vm_harness_from_module(source, &module);
    harness
        .runtime_mut()
        .set_execution_deadline(Instant::now().checked_sub(StdDuration::from_millis(1)));
    let cycle = harness.cycle();
    assert!(
        cycle
            .errors
            .iter()
            .any(|err| matches!(err, RuntimeError::ExecutionTimeout)),
        "expected ExecutionTimeout for forward-only bytecode, got {:?}",
        cycle.errors
    );
}
