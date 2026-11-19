//! The driver's tests: what it prints for each type, how it reads inputs, a run of records through
//! the test harness, and its refusals.

use super::*;

#[test]
fn each_type_prints_in_the_trace_syntax() {
    // The suite's tracefmt compares every value but a real by its text: a bit string as 16#
    // with the hex digits of its width, a time as T#<n>ms, a string as #<hex bytes> that the
    // harness reads back, and a real as a number its float() reads. Catches a WORD printed in
    // decimal, a time in another unit, and a NaN spelled as Rust spells it.
    assert_eq!(render(&Value::Bool(true)).as_deref(), Some("TRUE"));
    assert_eq!(render(&Value::Int(-5)).as_deref(), Some("-5"));
    assert_eq!(render(&Value::Byte(0x0F)).as_deref(), Some("16#0F"));
    assert_eq!(render(&Value::Word(0x00FF)).as_deref(), Some("16#00FF"));
    assert_eq!(render(&Value::DWord(0xFF)).as_deref(), Some("16#000000FF"));
    assert_eq!(
        render(&Value::Time(Duration::from_millis(1500))).as_deref(),
        Some("T#1500ms")
    );
    assert_eq!(
        render(&Value::String("a b'$".into())).as_deref(),
        Some("#6120622724")
    );
    assert_eq!(render(&Value::Real(0.5)).as_deref(), Some("0.5"));
    assert_eq!(render(&Value::LReal(f64::NAN)).as_deref(), Some("nan"));
    assert_eq!(
        render(&Value::LReal(f64::NEG_INFINITY)).as_deref(),
        Some("-inf")
    );
}

#[test]
fn an_input_takes_the_variable_s_type() {
    // A trace's input text in the type the variable has: a bit string from 16#, a time from
    // T#<n>ms, a string through its escapes. Catches a value out of its type's range written
    // anyway, and an escape read as plain text.
    assert_eq!(input_value(&Value::Byte(0), "16#A5"), Ok(Value::Byte(0xA5)));
    assert_eq!(input_value(&Value::Int(0), "-3"), Ok(Value::Int(-3)));
    assert_eq!(
        input_value(&Value::Bool(false), "TRUE"),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        input_value(&Value::Time(Duration::from_millis(0)), "T#20ms"),
        Ok(Value::Time(Duration::from_millis(20)))
    );
    assert_eq!(
        input_value(&Value::String("".into()), "'a$20b$''"),
        Ok(Value::String("a b'".into()))
    );
    assert_eq!(
        input_value(&Value::Byte(0), "300"),
        Err("300 is out of range".to_string())
    );
}

#[test]
fn each_record_runs_one_scan_at_its_time() {
    // Records 0 and 1 at a period of 100 ms: the inputs of each record before its scan, the
    // scan of record n at PLC time n * 100 ms, so TIME() reads T#0ms and then T#100ms, and the
    // types line first. A division by zero in record 2 prints a trap and ends the run, so
    // record 3 is never read. Catches inputs written after the scan, a scan before the first
    // record or two per record, and a run that goes on after a fault.
    let program = "PROGRAM main\nVAR_INPUT a : INT; d : INT := 1; END_VAR\n\
                   VAR_OUTPUT b : INT; t : TIME; END_VAR\n\
                   b := a * 2 / d;\nt := TIME();\nEND_PROGRAM\n";
    let records = "0 a=1\n1 a=5\n2 d=0\n3 a=7\n";
    let mut printed = Vec::new();
    drive(
        program,
        "100",
        &["b".to_string(), "t".to_string()],
        records.as_bytes(),
        &mut printed,
    )
    .expect("the run");
    let printed = String::from_utf8(printed).expect("text");
    let mut lines = printed.lines();
    assert_eq!(lines.next(), Some("types b=INT t=TIME"));
    assert_eq!(lines.next(), Some("0 => b=2 t=T#0ms"));
    assert_eq!(lines.next(), Some("1 => b=10 t=T#100ms"));
    assert!(
        lines
            .next()
            .is_some_and(|line| line.starts_with("2 => trap: ")),
        "{printed}"
    );
    assert_eq!(lines.next(), None, "{printed}");
}

#[test]
fn a_program_that_does_not_compile_and_an_unknown_variable_are_refused() {
    // Exit status 1 for a compile error, 2 for a name the program does not have. Catches a
    // compile error reported as a failed record, which the harness would count as a runtime's
    // wrong output.
    let refused = drive(
        "PROGRAM main\nx := ;\nEND_PROGRAM\n",
        "10",
        &[],
        &b""[..],
        Vec::<u8>::new(),
    );
    assert!(matches!(refused, Err(Refusal::Compile(_))), "{refused:?}");
    let refused = drive(
        "PROGRAM main\nVAR_OUTPUT b : INT; END_VAR\nb := 1;\nEND_PROGRAM\n",
        "10",
        &["nothing".to_string()],
        &b""[..],
        Vec::<u8>::new(),
    );
    assert!(matches!(refused, Err(Refusal::Other(_))), "{refused:?}");
}
