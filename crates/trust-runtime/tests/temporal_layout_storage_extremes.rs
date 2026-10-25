//! Storage round trip of every temporal type at the signed 64-bit boundaries (spec 10 §2.5 and
//! spec 12 §4.1: logical temporal values keep the signed 64-bit domain). Values are written,
//! copied by an assignment in the cycle, and read back exactly.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.
#![cfg(feature = "full")]
use trust_runtime::{
    harness::TestHarness,
    value::{
        DateTimeValue, DateValue, Duration, LDateTimeValue, LDateValue, LTimeOfDayValue,
        TimeOfDayValue, Value,
    },
};

const SOURCE: &str = r#"
VAR_GLOBAL
    time_in : TIME; time_out : TIME;
    ltime_in : LTIME; ltime_out : LTIME;
    date_in : DATE; date_out : DATE;
    ldate_in : LDATE; ldate_out : LDATE;
    tod_in : TOD; tod_out : TOD;
    ltod_in : LTOD; ltod_out : LTOD;
    dt_in : DT; dt_out : DT;
    ldt_in : LDT; ldt_out : LDT;
END_VAR
PROGRAM Main
    time_out := time_in;
    ltime_out := ltime_in;
    date_out := date_in;
    ldate_out := ldate_in;
    tod_out := tod_in;
    ltod_out := ltod_in;
    dt_out := dt_in;
    ldt_out := ldt_in;
END_PROGRAM
"#;

/// Input and output globals of each temporal type, in the order of `typed`.
const NAMES: [(&str, &str); 8] = [
    ("time_in", "time_out"),
    ("ltime_in", "ltime_out"),
    ("date_in", "date_out"),
    ("ldate_in", "ldate_out"),
    ("tod_in", "tod_out"),
    ("ltod_in", "ltod_out"),
    ("dt_in", "dt_out"),
    ("ldt_in", "ldt_out"),
];

fn typed(kind: usize, value: i64) -> Value {
    match kind {
        0 => Value::Time(Duration::from_nanos(value)),
        1 => Value::LTime(Duration::from_nanos(value)),
        2 => Value::Date(DateValue::new(value)),
        3 => Value::LDate(LDateValue::new(value)),
        4 => Value::Tod(TimeOfDayValue::new(value)),
        5 => Value::LTod(LTimeOfDayValue::new(value)),
        6 => Value::Dt(DateTimeValue::new(value)),
        _ => Value::Ldt(LDateTimeValue::new(value)),
    }
}

#[test]
fn every_temporal_type_keeps_the_extreme_64_bit_values_through_storage() {
    let mut harness = TestHarness::from_source(SOURCE).unwrap();
    for value in [i64::MIN, -1, 0, 1, i64::MAX] {
        for (kind, (input, _)) in NAMES.iter().enumerate() {
            harness
                .runtime_mut()
                .write_variable(input, &typed(kind, value))
                .unwrap();
        }
        let result = harness.cycle();
        assert!(result.errors.is_empty(), "{value}: {:?}", result.errors);
        for (kind, (_, output)) in NAMES.iter().enumerate() {
            assert_eq!(
                harness.runtime().read_variable(output),
                Ok(typed(kind, value)),
                "{output} {value}"
            );
        }
    }
}
