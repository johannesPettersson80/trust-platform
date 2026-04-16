use std::f64::consts::TAU;

use trust_runtime::execution_backend::ExecutionBackend;
use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

const OSCAT_BASIC_DATA_TYPES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/03_data_types/oscat_basic_data_types.st"
));
const OSCAT_BASIC_OTHER_FUNCTIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/04_other_functions/oscat_basic_other_functions.st"
));

const OSCAT_BASIC_CARRIER_SPLIT_PROGRAM: &str = r#"
PROGRAM Main
VAR
    InitOk : BOOL := FALSE;
    Pi2 : REAL := REAL#0.0;
    T0 : REAL := REAL#0.0;
    DefaultLanguage : INT := INT#0;
    AprilName : STRING[16];
END_VAR

InitOk := OSCAT_BASIC_Constants();
Pi2 := MATH.PI2;
T0 := PHYS.T0;
DefaultLanguage := LANGUAGE.DEFAULT;
AprilName := LANGUAGE.MONTHS[DefaultLanguage, INT#4];
END_PROGRAM
"#;

fn harness(bytecode_vm: bool) -> TestHarness {
    let mut harness = TestHarness::from_sources(&[
        OSCAT_BASIC_DATA_TYPES,
        OSCAT_BASIC_OTHER_FUNCTIONS,
        OSCAT_BASIC_CARRIER_SPLIT_PROGRAM,
    ])
    .expect("compile harness");
    if bytecode_vm {
        harness
            .runtime_mut()
            .set_execution_backend(ExecutionBackend::BytecodeVm)
            .expect("select vm backend");
        harness
            .runtime_mut()
            .restart(trust_runtime::RestartMode::Cold)
            .expect("restart runtime");
    }
    harness
}

fn assert_real_near(actual: Value, expected: f64, delta: f64) {
    match actual {
        Value::Real(value) => {
            assert!(
                ((value as f64) - expected).abs() <= delta,
                "expected {expected} +/- {delta}, got {value}"
            );
        }
        Value::LReal(value) => {
            assert!(
                (value - expected).abs() <= delta,
                "expected {expected} +/- {delta}, got {value}"
            );
        }
        other => panic!("expected REAL value, got {other:?}"),
    }
}

fn assert_carrier_split_behavior(harness: &mut TestHarness) {
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);

    harness.assert_eq("InitOk", true);
    assert_real_near(harness.get_output("Pi2").expect("Pi2 output"), TAU, 0.0001);
    assert_real_near(
        harness.get_output("T0").expect("T0 output"),
        -273.15,
        0.0001,
    );
    harness.assert_eq("DefaultLanguage", 1i32);
    harness.assert_eq("AprilName", Value::String("April".into()));
}

#[test]
fn oscat_basic_chapter_split_globals_resolve_in_runtime() {
    let mut harness = harness(false);
    assert_carrier_split_behavior(&mut harness);
}

#[test]
fn oscat_basic_chapter_split_globals_resolve_in_vm() {
    let mut harness = harness(true);
    assert_carrier_split_behavior(&mut harness);
}
