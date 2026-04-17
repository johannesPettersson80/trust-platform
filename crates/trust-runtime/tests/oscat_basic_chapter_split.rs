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
const OSCAT_BASIC_MATHEMATICS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/05_mathematics/oscat_basic_mathematics.st"
));
const OSCAT_BASIC_DOUBLE_PRECISION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/08_arithmetics_with_double_precision/oscat_basic_double_precision.st"
));
const OSCAT_BASIC_ARITHMETIC_FUNCTIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/09_arithmetic_functions/oscat_basic_arithmetic_functions.st"
));
const OSCAT_BASIC_GEOMETRIC_FUNCTIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/10_geometric_functions/oscat_basic_geometric_functions.st"
));
const OSCAT_BASIC_TIME_AND_DATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/12_time_and_date/oscat_basic_time_and_date.st"
));
const OSCAT_BASIC_STRING_FUNCTIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/13_string_functions/oscat_basic_string_functions.st"
));
const OSCAT_BASIC_MEMORY_MODULES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/14_memory_modules/oscat_basic_memory_modules.st"
));
const OSCAT_BASIC_PULSE_GENERATORS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/15_pulse_generators/oscat_basic_pulse_generators.st"
));
const OSCAT_BASIC_LOGIC_MODULES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/16_logic_modules/oscat_basic_logic_modules.st"
));
const OSCAT_BASIC_LATCHES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/17_latches_flip_flop_and_shift_register/oscat_basic_latches_flip_flop_and_shift_register.st"
));
const OSCAT_BASIC_SIGNAL_PROCESSING: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/19_signal_processing/oscat_basic_signal_processing.st"
));
const OSCAT_BASIC_MEASURING_MODULES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/21_measuring_modules/oscat_basic_measuring_modules.st"
));
const OSCAT_BASIC_CALCULATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/22_calculations/oscat_basic_calculations.st"
));
const OSCAT_BASIC_BUFFER_MANAGEMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/25_buffer_management/oscat_basic_buffer_management.st"
));
const OSCAT_BASIC_LIST_PROCESSING: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/26_list_processing/oscat_basic_list_processing.st"
));

const OSCAT_BASIC_SHIPPED_SOURCES: &[&str] = &[
    OSCAT_BASIC_DATA_TYPES,
    OSCAT_BASIC_OTHER_FUNCTIONS,
    OSCAT_BASIC_MATHEMATICS,
    OSCAT_BASIC_DOUBLE_PRECISION,
    OSCAT_BASIC_ARITHMETIC_FUNCTIONS,
    OSCAT_BASIC_GEOMETRIC_FUNCTIONS,
    OSCAT_BASIC_TIME_AND_DATE,
    OSCAT_BASIC_STRING_FUNCTIONS,
    OSCAT_BASIC_MEMORY_MODULES,
    OSCAT_BASIC_PULSE_GENERATORS,
    OSCAT_BASIC_LOGIC_MODULES,
    OSCAT_BASIC_LATCHES,
    OSCAT_BASIC_SIGNAL_PROCESSING,
    OSCAT_BASIC_MEASURING_MODULES,
    OSCAT_BASIC_CALCULATIONS,
    OSCAT_BASIC_BUFFER_MANAGEMENT,
    OSCAT_BASIC_LIST_PROCESSING,
    OSCAT_BASIC_CARRIER_SPLIT_PROGRAM,
];

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
    let mut harness =
        TestHarness::from_sources(OSCAT_BASIC_SHIPPED_SOURCES).expect("compile harness");
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
