//! A function block instance that a task runs on its own (IEC 61131-3 §6.8.2, `fb WITH T`).
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

#[test]
fn fb_instance_runs_under_task_control() {
    let source = r#"
FUNCTION_BLOCK FB
VAR_INPUT
    IN : BOOL;
END_VAR
VAR_OUTPUT
    OUT : BOOL;
END_VAR
OUT := IN;
END_FUNCTION_BLOCK

PROGRAM P
VAR
    fb : FB;
END_VAR
END_PROGRAM

CONFIGURATION C
RESOURCE R ON CPU
VAR_GLOBAL
    trigger : BOOL;
END_VAR
TASK T (SINGLE := trigger, INTERVAL := T#0ms, PRIORITY := 0);
PROGRAM P1 WITH T : P (fb WITH T);
END_RESOURCE
END_CONFIGURATION
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    {
        let runtime = harness.runtime_mut();
        runtime
            .write_variable("P1.fb.IN", &Value::Bool(true))
            .unwrap();
        runtime
            .write_variable("trigger", &Value::Bool(true))
            .unwrap();
    }

    harness.cycle();

    assert_eq!(
        harness.runtime().read_variable("P1.fb.OUT"),
        Ok(Value::Bool(true))
    );
}
