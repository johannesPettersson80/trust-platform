use trust_runtime::harness::TestHarness;
use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::{DateTimeValue, DateValue, Duration, TimeOfDayValue, Value};

#[test]
fn numeric_full() {
    let lib = StandardLibrary::new();

    assert_eq!(lib.call("ABS", &[Value::Int(-5)]).unwrap(), Value::Int(5));

    let sqrt = lib.call("SQRT", &[Value::Real(9.0)]).unwrap();
    match sqrt {
        Value::Real(value) => assert!((value - 3.0).abs() < 1e-6),
        _ => panic!("expected REAL result"),
    }

    let log = lib.call("LOG", &[Value::Real(100.0)]).unwrap();
    match log {
        Value::Real(value) => assert!((value - 2.0).abs() < 1e-6),
        _ => panic!("expected REAL result"),
    }

    let atan2 = lib
        .call("ATAN2", &[Value::Real(0.0), Value::Real(1.0)])
        .unwrap();
    match atan2 {
        Value::Real(value) => assert!(value.abs() < 1e-6),
        _ => panic!("expected REAL result"),
    }

    assert_eq!(
        lib.call("ADD", &[Value::Int(1), Value::Int(2), Value::Int(3)])
            .unwrap(),
        Value::Int(6)
    );

    assert_eq!(
        lib.call("MUL", &[Value::Int(2), Value::Int(3), Value::Int(4)])
            .unwrap(),
        Value::Int(24)
    );

    assert_eq!(
        lib.call("SUB", &[Value::Int(7), Value::Int(2)]).unwrap(),
        Value::Int(5)
    );

    assert_eq!(
        lib.call("DIV", &[Value::Int(7), Value::Int(2)]).unwrap(),
        Value::Int(3)
    );

    assert_eq!(
        lib.call("MOD", &[Value::Int(7), Value::Int(2)]).unwrap(),
        Value::Int(1)
    );

    let expt = lib
        .call("EXPT", &[Value::Real(2.0), Value::Real(3.0)])
        .unwrap();
    match expt {
        Value::Real(value) => assert!((value - 8.0).abs() < 1e-6),
        _ => panic!("expected REAL result"),
    }

    assert_eq!(lib.call("MOVE", &[Value::Int(42)]).unwrap(), Value::Int(42));

    let add_time = lib
        .call(
            "ADD_TIME",
            &[
                Value::Time(Duration::from_secs(1)),
                Value::Time(Duration::from_secs(2)),
            ],
        )
        .unwrap();
    assert_eq!(add_time, Value::Time(Duration::from_secs(3)));

    let date1 = Value::Date(DateValue::new(0));
    let date2 = Value::Date(DateValue::new(86_400_000));
    let diff = lib.call("SUB_DATE_DATE", &[date2, date1]).unwrap();
    assert_eq!(diff, Value::Time(Duration::from_secs(86_400)));

    let dt = lib
        .call(
            "CONCAT_DATE_TOD",
            &[
                Value::Date(DateValue::new(0)),
                Value::Tod(TimeOfDayValue::new(3_600_000)),
            ],
        )
        .unwrap();
    assert_eq!(dt, Value::Dt(DateTimeValue::new(3_600_000)));

    let date = lib
        .call(
            "CONCAT_DATE",
            &[Value::Int(1970), Value::Int(1), Value::Int(1)],
        )
        .unwrap();
    assert_eq!(date, Value::Date(DateValue::new(0)));

    let tod = lib
        .call(
            "CONCAT_TOD",
            &[Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4)],
        )
        .unwrap();
    assert_eq!(tod, Value::Tod(TimeOfDayValue::new(3_723_004)));

    let dt2 = lib
        .call(
            "CONCAT_DT",
            &[
                Value::Int(1970),
                Value::Int(1),
                Value::Int(1),
                Value::Int(1),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
            ],
        )
        .unwrap();
    assert_eq!(dt2, Value::Dt(DateTimeValue::new(3_600_000)));

    assert_eq!(
        lib.call("DAY_OF_WEEK", &[Value::Date(DateValue::new(0))])
            .unwrap(),
        Value::Int(4)
    );
}

#[test]
fn split_functions() {
    let source = r#"
        PROGRAM Test
        VAR
            date_a : DATE := DATE#1970-01-02;
            tod_a : TOD := TOD#01:02:03.004;
            dt_a : DT := DT#1970-01-02-01:02:03.004;

            year_a : INT; month_a : INT; day_a : INT;
            hour_a : INT; minute_a : INT; second_a : INT; msec_a : INT;

            year_b : INT; month_b : INT; day_b : INT;
            hour_b : INT; minute_b : INT; second_b : INT; msec_b : INT;
        END_VAR
        SPLIT_DATE(date_a, year_a, month_a, day_a);
        SPLIT_TOD(tod_a, hour_a, minute_a, second_a, msec_a);
        SPLIT_DT(dt_a, year_b, month_b, day_b, hour_b, minute_b, second_b, msec_b);
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.cycle();

    harness.assert_eq("year_a", 1970i16);
    harness.assert_eq("month_a", 1i16);
    harness.assert_eq("day_a", 2i16);
    harness.assert_eq("hour_a", 1i16);
    harness.assert_eq("minute_a", 2i16);
    harness.assert_eq("second_a", 3i16);
    harness.assert_eq("msec_a", 4i16);
    harness.assert_eq("year_b", 1970i16);
    harness.assert_eq("month_b", 1i16);
    harness.assert_eq("day_b", 2i16);
    harness.assert_eq("hour_b", 1i16);
    harness.assert_eq("minute_b", 2i16);
    harness.assert_eq("second_b", 3i16);
    harness.assert_eq("msec_b", 4i16);
}
