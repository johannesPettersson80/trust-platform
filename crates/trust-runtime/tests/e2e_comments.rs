//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use trust_runtime::harness::TestHarness;

#[test]
fn comment_tolerance() {
    let source = r#"
        (* Block comment before program *)
        PROGRAM Demo
        VAR
            (* init value *)
            count: DINT := 0; // inline comment
        END_VAR
        // increment
        count := count + 1;
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.assert_eq("count", 0i32);
    harness.cycle();
    harness.assert_eq("count", 1i32);
}
