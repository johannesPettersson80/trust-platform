//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use trust_runtime::harness::TestHarness;

#[test]
fn from_source_full_pipeline() {
    let source = r#"
        PROGRAM Demo
        VAR
            count: DINT := 0;
        END_VAR
        count := count + 1;
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.assert_eq("count", 0i32);
    harness.cycle();
    harness.assert_eq("count", 1i32);
}
