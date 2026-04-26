use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

#[test]
fn iec_table11() {
    let source = r#"
TYPE
    Color : (Red, Green, Blue);
    Range : INT(0..10);
    Point : STRUCT
        x : INT;
        y : INT;
    END_STRUCT;
    IntArray : ARRAY[1..3] OF INT;
END_TYPE

PROGRAM Main
VAR
    c : Color;
    r : Range := INT#5;
    p : Point;
    a : IntArray;
END_VAR
p.x := INT#1;
p.y := INT#2;
a[1] := INT#10;
a[2] := INT#20;
a[3] := INT#30;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.cycle();

    let c = harness.get_output("c").unwrap();
    match c {
        Value::Enum(enum_value) => {
            assert_eq!(enum_value.variant_name().as_str(), "Red");
        }
        _ => panic!("expected enum value"),
    }

    assert_eq!(harness.get_output("r"), Some(Value::Int(5)));

    let p = harness.get_output("p").unwrap();
    let Value::Struct(struct_value) = p else {
        panic!("expected struct value");
    };
    assert_eq!(struct_value.fields().get("x"), Some(&Value::Int(1)));
    assert_eq!(struct_value.fields().get("y"), Some(&Value::Int(2)));

    let a = harness.get_output("a").unwrap();
    let Value::Array(array_value) = a else {
        panic!("expected array value");
    };
    assert_eq!(array_value.dimensions(), vec![(1, 3)]);
    assert_eq!(array_value.elements()[0], Value::Int(10));
    assert_eq!(array_value.elements()[1], Value::Int(20));
    assert_eq!(array_value.elements()[2], Value::Int(30));
}
