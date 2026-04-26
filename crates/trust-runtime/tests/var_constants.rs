use trust_runtime::execution_backend::ExecutionBackend;
use trust_runtime::harness::TestHarness;
use trust_runtime::value::{ArrayValue, Value};

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

#[test]
fn iec_6_5_4() {
    let source = r#"
PROGRAM Main
VAR CONSTANT
    c : INT := 1;
END_VAR
c := 2;
END_PROGRAM
"#;

    let err = TestHarness::from_source(source)
        .err()
        .expect("expected constant modification error");
    let _ = err;
}

#[test]
fn parameter_constant_runtime_call_end_to_end() {
    let source = r#"
FUNCTION_BLOCK Worker
    VAR_INPUT CONSTANT
        A : INT;
    END_VAR
    VAR_IN_OUT CONSTANT
        B : ARRAY[0..2] OF INT;
    END_VAR
    VAR_OUTPUT
        C : INT;
    END_VAR
    VAR_TEMP CONSTANT
        T : INT := INT#7;
    END_VAR
    C := A + B[0] + T;
END_FUNCTION_BLOCK

PROGRAM Main
    VAR
        fb : Worker;
        arr : ARRAY[0..2] OF INT;
        out_c : INT;
    END_VAR
    arr[0] := INT#5;
    arr[1] := INT#6;
    arr[2] := INT#7;
    fb(A := INT#3, B := arr, C => out_c);
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "runtime execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_c", 15i16);
    harness.assert_eq(
        "arr",
        Value::Array(Box::new(
            ArrayValue::from_untyped_parts(
                vec![5i16.into(), 6i16.into(), 7i16.into()],
                vec![(0, 2)],
            )
            .expect("valid expected array"),
        )),
    );

    let mut vm = vm_harness(source);
    let cycle = vm.cycle();
    assert!(
        cycle.errors.is_empty(),
        "vm execution failed: {:?}",
        cycle.errors
    );
    vm.assert_eq("out_c", 15i16);
    vm.assert_eq(
        "arr",
        Value::Array(Box::new(
            ArrayValue::from_untyped_parts(
                vec![5i16.into(), 6i16.into(), 7i16.into()],
                vec![(0, 2)],
            )
            .expect("valid expected array"),
        )),
    );
}
