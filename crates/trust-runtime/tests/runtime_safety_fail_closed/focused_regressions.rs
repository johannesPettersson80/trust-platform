use super::*;

#[test]
fn rejected_shared_cycle_latches_fault_and_delivers_safe_outputs_before_plc_execution() {
    let source = r#"
CONFIGURATION C
VAR_GLOBAL shared : INT := 1; END_VAR
PROGRAM P : Main;
END_CONFIGURATION
PROGRAM Main
VAR q AT %QX0.0 : BOOL := TRUE; calls : INT; END_VAR
calls := calls + 1;
END_PROGRAM
"#;
    let source_runtime = TestHarness::from_source(source).unwrap().into_runtime();
    let shared = SharedGlobals::from_runtime(vec!["shared".into()], &source_runtime).unwrap();
    let mut destination = TestHarness::from_source(&source.replace("INT := 1", "BOOL := FALSE"))
        .unwrap()
        .into_runtime();
    destination
        .write_io(&IoAddress::parse("%QX0.0").unwrap(), Value::Bool(true))
        .unwrap();
    let writes = Arc::new(Mutex::new(Vec::new()));
    destination.add_io_driver(
        "recording",
        Box::new(RecordingDriver::new(Arc::clone(&writes))),
    );
    destination.set_fault_policy(FaultPolicy::SafeHalt);
    let mut safe_state = IoSafeState::default();
    safe_state
        .outputs
        .push((IoAddress::parse("%QX0.0").unwrap(), Value::Bool(false)));
    destination.set_io_safe_state(safe_state);
    let mut runner = ResourceRunner::new(destination, StepClock::new(), Duration::from_millis(10));

    assert_eq!(
        runner.tick_with_shared(&shared),
        Err(RuntimeError::TypeMismatch)
    );
    assert!(runner.runtime().faulted());
    assert_eq!(
        runner.runtime().storage().get_global("shared"),
        Some(&Value::Bool(false))
    );
    assert_eq!(writes.lock().unwrap().as_slice(), &[vec![0]]);
    assert_eq!(shared.get("shared"), Some(Value::Int(1)));
    let Some(Value::Instance(program)) = runner.runtime().storage().get_global("P") else {
        panic!("declared program root")
    };
    assert_eq!(
        runner
            .runtime()
            .storage()
            .get_instance_var(*program, "calls"),
        Some(&Value::Int(0))
    );
    assert_eq!(
        runner.tick_with_shared(&shared),
        Err(RuntimeError::ResourceFaulted)
    );
}

#[test]
fn logical_limit_keeps_its_diagnosis_while_using_the_configured_watchdog_action() {
    let source =
        "PROGRAM Main VAR q AT %QX0.0 : BOOL := TRUE; END_VAR WHILE TRUE DO END_WHILE; END_PROGRAM";
    let bytes = trust_runtime::harness::CompileSession::from_source(source)
        .build_bytecode_bytes()
        .unwrap();
    let limits = trust_runtime_core::vm::PreparationLimits {
        max_work: 4096,
        ..Default::default()
    };
    let mut runtime = Runtime::from_bytes(&bytes, limits).unwrap();
    let address = IoAddress::parse("%QX0.0").unwrap();
    runtime.write_io(&address, Value::Bool(true)).unwrap();
    let writes = Arc::new(Mutex::new(Vec::new()));
    runtime.add_io_driver(
        "recording",
        Box::new(RecordingDriver::new(Arc::clone(&writes))),
    );
    let mut safe = IoSafeState::default();
    safe.outputs.push((address, Value::Bool(false)));
    runtime.set_io_safe_state(safe);
    runtime.set_fault_policy(FaultPolicy::Halt);
    runtime.set_watchdog_policy(WatchdogPolicy {
        enabled: true,
        timeout: Duration::from_secs(60),
        action: WatchdogAction::SafeHalt,
    });
    let mut handle = ResourceRunner::new(runtime, StepClock::new(), Duration::from_millis(10))
        .spawn("quota-watchdog-cause")
        .unwrap();
    wait_for_fault(&handle);
    handle.stop();
    handle.join().unwrap();
    assert_eq!(handle.state(), ResourceState::Faulted);
    assert!(
        matches!(handle.last_error(), Some(RuntimeError::ExecutionLimit {
        kind: trust_runtime_core::error::ExecutionLimitKind::Work, used, requested, limit: 4096,
    }) if used <= 4096 && requested > 4096 - used)
    );
    assert_eq!(
        writes.lock().unwrap().last(),
        Some(&vec![0]),
        "watchdog SafeHalt, not ordinary Halt, owns the selected action"
    );
}
