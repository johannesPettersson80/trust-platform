use trust_runtime::harness::TestHarness;

#[test]
fn class_instances() {
    let source = r#"
CLASS Base
VAR PUBLIC
    base_val : INT := INT#1;
END_VAR
END_CLASS

CLASS Derived EXTENDS Base
VAR PUBLIC
    child_val : INT := INT#2;
END_VAR
END_CLASS

PROGRAM Main
VAR
    obj : Derived;
    out_base : INT := INT#0;
    out_child : INT := INT#0;
END_VAR
obj.base_val := INT#10;
obj.child_val := INT#20;
out_base := obj.base_val;
out_child := obj.child_val;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    harness.assert_eq("out_base", 10i16);
    harness.assert_eq("out_child", 20i16);
}
