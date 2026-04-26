use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

#[test]
fn iec_table15() {
    let source = r#"
FUNCTION IncFirst : INT
VAR_IN_OUT
    arr : ARRAY[*] OF INT;
END_VAR
arr[0] := arr[0] + INT#1;
IncFirst := arr[0];
END_FUNCTION

PROGRAM Main
VAR
    a1 : ARRAY[0..1] OF INT;
    a2 : ARRAY[0..3] OF INT;
END_VAR
a1[0] := INT#1;
a2[0] := INT#5;
a1[0] := IncFirst(arr := a1);
a2[0] := IncFirst(arr := a2);
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.cycle();

    let a1 = harness.get_output("a1").unwrap();
    let Value::Array(arr1) = a1 else {
        panic!("expected array");
    };
    assert_eq!(arr1.elements()[0], Value::Int(2));

    let a2 = harness.get_output("a2").unwrap();
    let Value::Array(arr2) = a2 else {
        panic!("expected array");
    };
    assert_eq!(arr2.elements()[0], Value::Int(6));
}
