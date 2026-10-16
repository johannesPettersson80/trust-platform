//! Spec 34 §4.3 HOST-02: hosted preparation limits are chosen by the caller, and
//! larger hosted applications are admitted with checked execution.
#![cfg(feature = "full")]

use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime::Runtime;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::error_code::StableErrorCode;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

const PERIODIC: &[u8] = include_bytes!("fixtures/portability/stbc-2.0/program-v2.stbc");

const CELL_COUNT: usize = 1000;
const TEXT_LEN: usize = 1500;

/// One FB call per cell and each cell's tick is summed into `total`.
/// `Fast` and `Slow` both run on 10 ms tasks, so the two cycles at 10 ms and
/// 20 ms run each program exactly twice: `Order` is 1, 12, 121, 1212.
fn scale_source(text: &str) -> String {
    let mut source = String::from(
        r"VAR_GLOBAL
    Order : DINT;
    total : DINT;
    label : STRING[2048];
END_VAR
FUNCTION_BLOCK Cell
VAR_OUTPUT
    ticks : DINT;
END_VAR
ticks := ticks + DINT#1;
END_FUNCTION_BLOCK
PROGRAM Fast
VAR_EXTERNAL
    Order : DINT;
    total : DINT;
END_VAR
VAR
",
    );
    for index in 0..CELL_COUNT {
        source.push_str(&format!("    cell_{index} : Cell;\n"));
    }
    source.push_str("END_VAR\n");
    for index in 0..CELL_COUNT {
        source.push_str(&format!("cell_{index}();\n"));
    }
    for index in 0..CELL_COUNT {
        source.push_str(&format!("total := total + cell_{index}.ticks;\n"));
    }
    source.push_str(
        r"Order := Order * 10 + 1;
END_PROGRAM
PROGRAM Slow
VAR_EXTERNAL
    Order : DINT;
    label : STRING[2048];
END_VAR
Order := Order * 10 + 2;
",
    );
    source.push_str(&format!("label := '{text}';\n"));
    source.push_str(
        r"END_PROGRAM
CONFIGURATION Hosted
TASK FastTask (INTERVAL := T#10ms, PRIORITY := 0);
TASK SlowTask (INTERVAL := T#10ms, PRIORITY := 1);
PROGRAM FastRun WITH FastTask : Fast;
PROGRAM SlowRun WITH SlowTask : Slow;
END_CONFIGURATION
",
    );
    source
}

/// Spec 34 §4.3 HOST-02 (line 268): the host entry applies the caller-selected preparation limits, not a fixed hosted profile.
#[test]
fn host_entry_applies_the_caller_selected_artifact_limit() {
    let refused = PreparationLimits {
        max_artifact_bytes: PERIODIC.len() - 1,
        ..Default::default()
    };
    let error = match Runtime::from_bytes(PERIODIC, refused) {
        Ok(_) => panic!("an artifact one byte over the caller's limit must be refused"),
        Err(error) => error,
    };
    assert_eq!(
        error.stable_code(),
        StableErrorCode::RuntimePreparationLimit
    );

    let mut runtime = Runtime::from_bytes(PERIODIC, PreparationLimits::default()).unwrap();
    runtime.set_current_time(Duration::from_millis(10));
    runtime.execute_cycle().unwrap();
    assert!(!runtime.faulted());
}

/// Spec 34 §4.3 HOST-02 (line 268): default hosted limits admit 1,000 FB instances, two tasks and a 1,500-character STRING[2048] with exact values and no fault.
#[test]
fn larger_strings_instances_and_two_tasks_run_under_default_hosted_limits() {
    let text = "x".repeat(TEXT_LEN);
    let module = CompileSession::from_source(scale_source(&text))
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let bytes = module.encode().unwrap();
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared
        .instantiate(0)
        .expect("default hosted limits admit 1,000 instances, two tasks and a long string");
    for sample in [10, 20] {
        state
            .execute_cycle(Duration::from_millis(sample))
            .expect("every instance call and string store fits the default work budget");
    }

    assert_eq!(
        state.storage().get_global("Order"),
        Some(&Value::DInt(1212))
    );
    assert_eq!(
        state.storage().get_global("total"),
        Some(&Value::DInt(3000))
    );
    assert_eq!(
        state.storage().get_global("label"),
        Some(&Value::String(text.as_str().into()))
    );
    assert!(state.fault().is_none());
}

/// Spec 34 §4.3 HOST-02 (line 268): the same artifact is refused when the caller's construction-values limit is below its demand.
#[test]
fn construction_demand_above_the_callers_value_limit_is_refused() {
    let module = CompileSession::from_source(scale_source("x"))
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let bytes = module.encode().unwrap();
    let error = PreparedModule::from_bytes(
        &bytes,
        PreparationLimits {
            max_construction_values: 1,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(error, RuntimeError::PreparationLimit);
}
