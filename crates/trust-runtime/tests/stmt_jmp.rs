use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

#[test]
fn jmp_flow() {
    let source = r#"
        PROGRAM Demo
        VAR
            x: INT := 0;
        END_VAR
        x := INT#1;
        JMP L1;
        x := INT#2;
        L1: x := x + INT#3;
        END_PROGRAM
    "#;

    // Specification 06 §13: the jump skips `x := 2`, so x = 1 + 3 = 4; a jump that ran the skipped
    // statement would give 2 + 3 = 5.
    let mut harness = TestHarness::from_source(source).expect("a program with JMP compiles");
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    assert_eq!(harness.get_output("x"), Some(Value::Int(4)));
}

#[test]
fn jmp_to_empty_label() {
    let source = r#"
        PROGRAM Demo
        VAR
            x: INT := 0;
        END_VAR
        JMP L1;
        x := INT#1;
        L1: ;
        x := INT#2;
        END_PROGRAM
    "#;

    // The jump skips `x := 1` to the empty statement, and the body goes on: x = 2.
    let mut harness = TestHarness::from_source(source).expect("a program with JMP compiles");
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    assert_eq!(harness.get_output("x"), Some(Value::Int(2)));
}
