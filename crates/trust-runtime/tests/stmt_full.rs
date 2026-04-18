use trust_runtime::harness::TestHarness;

#[test]
fn function_return_statement_uses_assigned_return_value_in_vm() {
    let source = r#"
        FUNCTION ValueOrDefault : DINT
        VAR_INPUT
            cond : BOOL;
            value : DINT;
        END_VAR
        IF cond THEN
            ValueOrDefault := value;
            RETURN;
        END_IF;
        RETURN DINT#0;
        END_FUNCTION

        PROGRAM Test
        VAR
            out : DINT := DINT#0;
        END_VAR
        out := ValueOrDefault(cond := TRUE, value := DINT#5);
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).expect("compile harness");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "runtime errors: {:?}",
        cycle.errors
    );
    assert_eq!(
        harness.get_output("out"),
        Some(trust_runtime::value::Value::DInt(5))
    );
}

#[test]
fn iec_table72() {
    let source = r#"
        FUNCTION Inc : INT
        VAR_INPUT
            x : INT;
        END_VAR
        Inc := x + INT#1;
        END_FUNCTION

        FUNCTION ReturnIfPositive : INT
        VAR_INPUT
            x : INT;
        END_VAR
        IF x > INT#0 THEN
            RETURN x;
        END_IF;
        RETURN INT#0;
        END_FUNCTION

        PROGRAM Test
        VAR
            x : INT := INT#0;
            y : INT := INT#0;
            i : INT := INT#0;
            tmp : INT := INT#0;
            out : INT := INT#0;
        END_VAR

        x := INT#1;

        IF x = INT#0 THEN
            y := INT#1;
        ELSIF x = INT#1 THEN
            y := INT#2;
        ELSE
            y := INT#3;
        END_IF;

        CASE y OF
            INT#1: x := INT#10;
            INT#2: x := INT#20;
        ELSE
            x := INT#30;
        END_CASE;

        FOR i := INT#0 TO INT#2 BY INT#1 DO
            x := x + INT#1;
        END_FOR;

        WHILE i < INT#2 DO
            i := i + INT#1;
        END_WHILE;

        REPEAT
            i := i - INT#1;
        UNTIL i = INT#0 END_REPEAT;

        FOR i := INT#0 TO INT#3 BY INT#1 DO
            IF i = INT#1 THEN
                CONTINUE;
            END_IF;
            IF i = INT#2 THEN
                EXIT;
            END_IF;
            x := x + INT#1;
        END_FOR;

        label1: x := x + INT#1;
        JMP label2;
        x := x + INT#100;
        label2: x := x + INT#1;

        Inc(x);
        tmp := Inc(x);
        out := ReturnIfPositive(tmp);
        ;
        END_PROGRAM
    "#;

    let err = TestHarness::from_source(source)
        .err()
        .expect("Table72 lowering should fail under current VM constraints");
    assert!(err
        .to_string()
        .contains("unsupported C5 edge-case lowering path"));
}
