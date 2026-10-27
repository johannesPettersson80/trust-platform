//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

#[test]
fn sr_rs() {
    let source = r#"
        PROGRAM Test
        VAR
            sr : SR;
            rs : RS;
            set_in : BOOL;
            reset_in : BOOL;
            q_sr : BOOL;
            q_rs : BOOL;
        END_VAR
        sr(S1 := set_in, R := reset_in, Q1 => q_sr);
        rs(S := set_in, R1 := reset_in, Q1 => q_rs);
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();

    harness.set_input("set_in", Value::Bool(false));
    harness.set_input("reset_in", Value::Bool(false));
    harness.cycle();
    harness.assert_eq("q_sr", Value::Bool(false));
    harness.assert_eq("q_rs", Value::Bool(false));

    harness.set_input("set_in", Value::Bool(true));
    harness.set_input("reset_in", Value::Bool(false));
    harness.cycle();
    harness.assert_eq("q_sr", Value::Bool(true));
    harness.assert_eq("q_rs", Value::Bool(true));

    harness.set_input("set_in", Value::Bool(false));
    harness.set_input("reset_in", Value::Bool(false));
    harness.cycle();
    harness.assert_eq("q_sr", Value::Bool(true));
    harness.assert_eq("q_rs", Value::Bool(true));

    harness.set_input("set_in", Value::Bool(false));
    harness.set_input("reset_in", Value::Bool(true));
    harness.cycle();
    harness.assert_eq("q_sr", Value::Bool(false));
    harness.assert_eq("q_rs", Value::Bool(false));

    harness.set_input("set_in", Value::Bool(true));
    harness.set_input("reset_in", Value::Bool(true));
    harness.cycle();
    harness.assert_eq("q_sr", Value::Bool(true));
    harness.assert_eq("q_rs", Value::Bool(false));
}
