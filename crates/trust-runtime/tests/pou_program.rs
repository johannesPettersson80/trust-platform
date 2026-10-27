//! A program instance run by its task: an event task runs it on a rising edge of its SINGLE input.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

/// `TestProg` counts in an event task that a rising edge of `trigger` starts.
const COUNTING: &str = r#"
CONFIGURATION C
VAR_GLOBAL
    trigger : BOOL := FALSE;
    count : INT := 0;
END_VAR
RESOURCE R ON PLC
    TASK T (SINGLE := trigger, PRIORITY := 0);
    PROGRAM P WITH T : TestProg;
END_RESOURCE
END_CONFIGURATION

PROGRAM TestProg
count := count + 1;
END_PROGRAM
"#;

#[test]
fn program_cycle() {
    let mut runtime = TestHarness::from_source(COUNTING)
        .expect("the program compiles")
        .into_runtime();
    runtime
        .write_variable("trigger", &Value::Bool(true))
        .unwrap();
    runtime.execute_cycle().unwrap();
    assert_eq!(runtime.read_variable("count"), Ok(Value::Int(1)));
}
