use trust_runtime::execution_backend::ExecutionBackend;
use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

fn vm_harness(source: &str) -> TestHarness {
    let mut harness = TestHarness::from_source(source).expect("compile harness");
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("select vm backend");
    harness
        .runtime_mut()
        .restart(trust_runtime::RestartMode::Cold)
        .expect("restart runtime");
    harness
}

fn vm_harness_sources(sources: &[&str]) -> TestHarness {
    let mut harness = TestHarness::from_sources(sources).expect("compile harness");
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("select vm backend");
    harness
        .runtime_mut()
        .restart(trust_runtime::RestartMode::Cold)
        .expect("restart runtime");
    harness
}

#[test]
fn named_constants_drive_parenthesized_string_lengths_in_runtime_and_vm() {
    let source = r#"
        VAR_GLOBAL CONSTANT
            STRING_LENGTH : INT := INT#12;
            EXTRA : INT := INT#2;
        END_VAR

        PROGRAM Main
        VAR
            s : STRING(STRING_LENGTH);
            ws : WSTRING(STRING_LENGTH + EXTRA);
            out_s : STRING(STRING_LENGTH);
            out_ws : WSTRING(STRING_LENGTH + EXTRA);
        END_VAR
        s := 'HELLO';
        ws := "WIDE";
        out_s := s;
        out_ws := ws;
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "runtime execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_s", Value::String("HELLO".into()));
    harness.assert_eq("out_ws", Value::WString("WIDE".to_string()));

    let mut vm = vm_harness(source);
    let cycle = vm.cycle();
    assert!(
        cycle.errors.is_empty(),
        "vm execution failed: {:?}",
        cycle.errors
    );
    vm.assert_eq("out_s", Value::String("HELLO".into()));
    vm.assert_eq("out_ws", Value::WString("WIDE".to_string()));
}

#[test]
fn cross_file_global_constants_drive_string_lengths_in_runtime_and_vm() {
    let globals = r#"
        VAR_GLOBAL CONSTANT
            STRING_LENGTH : INT := INT#12;
            EXTRA : INT := INT#2;
        END_VAR
    "#;

    let program = r#"
        PROGRAM Main
        VAR
            s : STRING(STRING_LENGTH);
            ws : WSTRING(STRING_LENGTH + EXTRA);
            out_s : STRING(STRING_LENGTH);
            out_ws : WSTRING(STRING_LENGTH + EXTRA);
        END_VAR
        s := 'HELLO';
        ws := "WIDE";
        out_s := s;
        out_ws := ws;
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_sources(&[globals, program]).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "runtime execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_s", Value::String("HELLO".into()));
    harness.assert_eq("out_ws", Value::WString("WIDE".to_string()));

    let mut vm = vm_harness_sources(&[globals, program]);
    let cycle = vm.cycle();
    assert!(
        cycle.errors.is_empty(),
        "vm execution failed: {:?}",
        cycle.errors
    );
    vm.assert_eq("out_s", Value::String("HELLO".into()));
    vm.assert_eq("out_ws", Value::WString("WIDE".to_string()));
}
