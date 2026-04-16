use trust_runtime::execution_backend::ExecutionBackend;
use trust_runtime::harness::TestHarness;

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
fn split_date_writes_function_local_outputs_in_runtime_and_vm() {
    let source = r#"
        FUNCTION YEAR_OF_DATE_PROBE : INT
        VAR_INPUT
            IDATE : DATE;
        END_VAR
        VAR
            YEAR_VALUE : INT;
            MONTH_VALUE : INT;
            DAY_VALUE : INT;
        END_VAR

        SPLIT_DATE(IDATE, YEAR_VALUE, MONTH_VALUE, DAY_VALUE);
        YEAR_OF_DATE_PROBE := YEAR_VALUE;
        END_FUNCTION

        PROGRAM Main
        VAR
            out_year : INT;
        END_VAR

        out_year := YEAR_OF_DATE_PROBE(IDATE := DATE#2024-06-01);
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "runtime execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_year", 2024i16);

    let mut vm = vm_harness(source);
    let cycle = vm.cycle();
    assert!(
        cycle.errors.is_empty(),
        "vm execution failed: {:?}",
        cycle.errors
    );
    vm.assert_eq("out_year", 2024i16);
}

#[test]
fn function_local_initializer_runs_in_runtime_and_vm() {
    let source = r#"
        FUNCTION LOCAL_INIT_PROBE : INT
        VAR
            BASE : INT := INT#5;
        END_VAR

        LOCAL_INIT_PROBE := BASE;
        END_FUNCTION

        PROGRAM Main
        VAR
            out_value : INT;
        END_VAR

        out_value := LOCAL_INIT_PROBE();
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "runtime execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_value", 5i16);

    let mut vm = vm_harness(source);
    let cycle = vm.cycle();
    assert!(
        cycle.errors.is_empty(),
        "vm execution failed: {:?}",
        cycle.errors
    );
    vm.assert_eq("out_value", 5i16);
}
