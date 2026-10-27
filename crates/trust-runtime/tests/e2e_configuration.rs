//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};

#[test]
fn resource_tasks() {
    let source = r#"
CONFIGURATION Conf
VAR_GLOBAL
    trigger : BOOL := FALSE;
END_VAR
TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
PROGRAM Inst WITH Fast : MainProg;
END_CONFIGURATION

PROGRAM MainProg
VAR
    count : INT := INT#0;
END_VAR
count := count + INT#1;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.cycle();
    assert_eq!(harness.get_output("count"), Some(Value::Int(0)));

    harness.advance_time(Duration::from_millis(10));
    harness.cycle();
    assert_eq!(harness.get_output("count"), Some(Value::Int(1)));
}
