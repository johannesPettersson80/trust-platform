//! Independent-oracle cases for TIME, LTIME, TOD and DATE arithmetic, conversions and
//! DAY_OF_WEEK.
//!
//! Source: A13/A27 numeric and standard-library audit, section 6 (prioritized missing cases)
//! and family H (date and time) and C22 (ADD_TIME overflow). Authorities: IEC 61131-3 Ed.3
//! Tables 35-36 (date and time functions), specification 07 sections 2.4, 2.8 and 9,
//! specification 10 section 6.13, and the DAY_OF_WEEK convention of spec 07 section 9
//! (0 = Sunday .. 6 = Saturday).
//!
//! Durations are signed 64-bit nanoseconds; TOD and DT ticks use the default 1 ms resolution.
//! Expected values come from the audit oracle (python3 `datetime` and exact integers), not
//! from the implementation. A failing case is a finding: report it, do not change the
//! expectation. Where a typed time literal is needed at the i64 boundary, an ST form is kept
//! as the audit gives it and a native companion asserts the same function directly.

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::{Duration, TimeOfDayValue, Value};

fn compile(source: &str) -> TestHarness {
    TestHarness::from_source(source).unwrap_or_else(|error| panic!("fixture must compile: {error}"))
}

fn output_after_cycle(source: &str, name: &str) -> Value {
    let mut harness = compile(source);
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness.try_get_output(name).unwrap()
}

fn assert_fault_keeps(source: &str, error: RuntimeError, name: &str, kept: Value) {
    let mut harness = compile(source);
    let cycle = harness.cycle();
    assert_eq!(cycle.errors, [error]);
    assert_eq!(harness.try_get_output(name).unwrap(), kept);
}

#[test]
fn oracle_c22_add_time_i64_maximum_plus_one_nanosecond_overflows_st() {
    // C22 (ST form as given by the audit): spec 07 section 9, checked signed nanoseconds.
    // The TIME literal for i64::MAX ns is parsed through f64 in truST, so this form depends
    // on the literal lowering being exact.
    assert_fault_keeps(
        "PROGRAM Main\nVAR t : TIME := T#5s; \
         a : TIME := TIME#9223372036s854ms775us807ns; END_VAR\n\
         t := ADD_TIME(a, T#1ns);\nEND_PROGRAM",
        RuntimeError::Overflow,
        "t",
        Value::Time(Duration::from_secs(5)),
    );
}

#[test]
fn oracle_c22_add_time_i64_maximum_plus_one_nanosecond_overflows_native() {
    // C22 (native form): i64::MAX nanoseconds plus one nanosecond.
    assert_eq!(
        StandardLibrary::new().call(
            "ADD_TIME",
            &[
                Value::Time(Duration::from_nanos(i64::MAX)),
                Value::Time(Duration::from_nanos(1)),
            ],
        ),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn oracle_h18_add_ltime_i64_maximum_plus_one_nanosecond_overflows_st() {
    // H18 (ST form as given by the audit): spec 07 section 9.
    assert_fault_keeps(
        "PROGRAM Main\nVAR t : LTIME := LTIME#5s; \
         a : LTIME := LTIME#9223372036s854ms775us807ns; END_VAR\n\
         t := ADD_LTIME(a, LTIME#1ns);\nEND_PROGRAM",
        RuntimeError::Overflow,
        "t",
        Value::LTime(Duration::from_secs(5)),
    );
}

#[test]
fn oracle_h18_add_ltime_i64_maximum_plus_one_nanosecond_overflows_native() {
    // H18 (native form): i64::MAX nanoseconds plus one nanosecond.
    assert_eq!(
        StandardLibrary::new().call(
            "ADD_LTIME",
            &[
                Value::LTime(Duration::from_nanos(i64::MAX)),
                Value::LTime(Duration::from_nanos(1)),
            ],
        ),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn oracle_h3_add_tod_time_does_not_wrap_at_midnight() {
    // H3: spec 07 section 9 and spec 10 section 6.13. 23:59:59.999 plus 1 ms is 86400000 ms
    // since midnight, one past the day; the result does not wrap to zero.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR t : TOD := TOD#00:00:00; END_VAR\n\
             t := ADD_TOD_TIME(TOD#23:59:59.999, T#1ms);\nEND_PROGRAM",
            "t",
        ),
        Value::Tod(TimeOfDayValue::new(86_400_000))
    );
}

#[test]
fn oracle_h4_sub_tod_time_does_not_wrap_below_midnight() {
    // H4: spec 10 section 6.13. 00:00:00 minus 1 ms is -1 ms in the same value family.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR t : TOD := TOD#00:00:01; END_VAR\n\
             t := SUB_TOD_TIME(TOD#00:00:00, T#1ms);\nEND_PROGRAM",
            "t",
        ),
        Value::Tod(TimeOfDayValue::new(-1))
    );
}

#[test]
fn oracle_h2_sub_tod_tod_negative_difference_is_signed() {
    // H2: spec 07 section 9 ("signed tick difference"). 1 s minus 3 s is -2 s.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR t : TIME := T#0s; END_VAR\n\
             t := SUB_TOD_TOD(TOD#00:00:01, TOD#00:00:03);\nEND_PROGRAM",
            "t",
        ),
        Value::Time(Duration::from_secs(-2))
    );
}

#[test]
fn oracle_h10_day_of_week_before_epoch() {
    // H10: spec 07 section 9 (0 = Sunday .. 6 = Saturday). 1969-12-31 was a Wednesday.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR d : INT := 0; END_VAR\nd := DAY_OF_WEEK(DATE#1969-12-31);\n\
             END_PROGRAM",
            "d",
        ),
        Value::Int(3)
    );
}

#[test]
fn oracle_h11_day_of_week_across_leap_day() {
    // H11: spec 07 section 9. 2000-01-01 was a Saturday (6); 2000-02-29 a Tuesday (2).
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR d : INT := 0; END_VAR\nd := DAY_OF_WEEK(DATE#2000-01-01);\n\
             END_PROGRAM",
            "d",
        ),
        Value::Int(6)
    );
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR d : INT := 0; END_VAR\nd := DAY_OF_WEEK(DATE#2000-02-29);\n\
             END_PROGRAM",
            "d",
        ),
        Value::Int(2)
    );
}

#[test]
fn oracle_h16_time_to_dword_negative_duration_overflows_and_keeps_destination() {
    // H16: spec 07 section 2.8 (rejects negative values). T#-1ns is not a DWORD millisecond
    // count.
    assert_fault_keeps(
        "PROGRAM Main\nVAR d : DWORD := DWORD#0; END_VAR\nd := TIME_TO_DWORD(T#-1ns);\n\
         END_PROGRAM",
        RuntimeError::Overflow,
        "d",
        Value::DWord(0),
    );
}

#[test]
fn oracle_h17_time_to_dword_upper_bound_is_exact() {
    // H17: spec 07 section 2.8. 4294967295 ms is exactly the DWORD maximum.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR d : DWORD := DWORD#0; END_VAR\nd := TIME_TO_DWORD(T#4294967295ms);\n\
             END_PROGRAM",
            "d",
        ),
        Value::DWord(u32::MAX)
    );
}
