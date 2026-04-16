use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

#[test]
fn assign_attempt() {
    let source = r#"
        PROGRAM Test
        VAR
            x : INT := INT#5;
            r1 : REF_TO INT;
            r2 : REF_TO INT;
            out : INT := INT#0;
        END_VAR
        r1 := REF(x);
        r2 ?= r1;
        out := r2^;
        r2 ?= NULL;
        IF r2 = NULL THEN
            out := out + INT#1;
        END_IF;
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.cycle();
    harness.assert_eq("out", 6i16);
    assert!(matches!(
        harness.get_output("r2"),
        Some(Value::Reference(None))
    ));
}
