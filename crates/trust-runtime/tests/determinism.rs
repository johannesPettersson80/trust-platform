//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use trust_runtime::harness::TestHarness;

#[test]
fn ordered_execution() {
    let source = r#"
        PROGRAM Determinism
        VAR
            a: DINT := 0;
            b: DINT := 0;
        END_VAR
        a := a + 1;
        b := b + a;
        END_PROGRAM
    "#;

    let mut first = TestHarness::from_source(source).unwrap();
    let mut second = TestHarness::from_source(source).unwrap();

    first.run_cycles(5);
    second.run_cycles(5);

    assert_eq!(first.get_output("a"), second.get_output("a"));
    assert_eq!(first.get_output("b"), second.get_output("b"));
}
