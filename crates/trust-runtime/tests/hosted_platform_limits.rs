//! Spec 34 §4.3 HOST-02: larger hosted applications run with checked execution: 1,000 function
//! block instances, two tasks and a long string.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.
#![cfg(feature = "full")]

use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};

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

/// Spec 34 §4.3 HOST-02 (line 268): a computer runs 1,000 FB instances, two tasks and a 1,500-character STRING[2048] with exact values and no fault.
#[test]
fn larger_strings_instances_and_two_tasks_run_under_default_hosted_limits() {
    let text = "x".repeat(TEXT_LEN);
    let mut runtime = TestHarness::from_source(&scale_source(&text))
        .expect("1,000 instances, two tasks and a long string compile")
        .into_runtime();
    for sample in [10, 20] {
        runtime.set_current_time(Duration::from_millis(sample));
        runtime
            .execute_cycle()
            .expect("every instance call and string store fits the default statement budget");
    }

    assert_eq!(runtime.read_variable("Order"), Ok(Value::DInt(1212)));
    assert_eq!(runtime.read_variable("total"), Ok(Value::DInt(3000)));
    assert_eq!(
        runtime.read_variable("label"),
        Ok(Value::String(text.as_str().into()))
    );
    assert!(!runtime.faulted());
}
