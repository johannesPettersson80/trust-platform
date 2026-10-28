//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use trust_runtime::harness::TestHarness;

#[test]
fn trailing_semicolons() {
    let source = r#"
        PROGRAM Demo
        VAR
            flag: BOOL := FALSE;
            count: DINT := 0;
        END_VAR;
        IF flag THEN
            count := count + 1;
        END_IF;
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.assert_eq("count", 0i32);
    harness.set_input("flag", true);
    harness.cycle();
    harness.assert_eq("count", 1i32);
}
