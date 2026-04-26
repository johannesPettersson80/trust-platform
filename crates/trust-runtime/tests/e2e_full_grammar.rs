use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

#[test]
fn var_and_stmt_coverage() {
    let source = r#"
        PROGRAM Grammar
        VAR
            flag: BOOL := TRUE;
            count: DINT := 0;
            i: DINT := 0;
            a, b: DINT := 1;
            arr: ARRAY[0..1, 0..2] OF DINT;
            msg: STRING[10] := 'Hi$N';
            wmsg: WSTRING[5] := "OK";
            t: TIME := T#10ms;
            d: DATE := DATE#2024-01-02;
            tod_val: TOD := TOD#12:34:56;
            dt_val: DT := DT#2024-01-02-03:04:05;
        END_VAR

        IF flag THEN
            count := count + a + b;
        ELSIF count = 0 THEN
            count := count + 10;
        ELSE
            count := count + 100;
        END_IF;

        CASE count OF
            0: count := count + 1;
            1, 2: count := count + 2;
            3..4: count := count + 3;
        ELSE
            count := count + 4;
        END_CASE;

        arr[0, 1] := count;
        FOR i := 0 TO 1 DO
            arr[1, i] := arr[0, 1] + i;
        END_FOR;

        WHILE count < 5 DO
            count := count + 1;
        END_WHILE;

        REPEAT
            count := count + 1;
        UNTIL count = 7
        END_REPEAT;
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();

    let cycle = harness.cycle();

    assert!(
        cycle.errors.is_empty(),
        "unexpected runtime errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("count", 7i32);

    let arr = harness.get_output("arr").expect("missing arr");
    let Value::Array(array) = arr else {
        panic!("expected array value");
    };
    assert_eq!(array.dimensions(), vec![(0, 1), (0, 2)]);
    assert_eq!(array.elements()[1], Value::DInt(4));
    assert_eq!(array.elements()[3], Value::DInt(4));
    assert_eq!(array.elements()[4], Value::DInt(5));
}
