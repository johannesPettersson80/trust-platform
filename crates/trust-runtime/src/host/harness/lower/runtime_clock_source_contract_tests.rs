use std::time::{SystemTime, UNIX_EPOCH};

use crate::harness::{CompileSession, TestHarness};
use crate::value::{Duration, Value};

fn host_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("test host clock must be after the Unix epoch")
        .as_millis()
}

fn dt_ticks(harness: &TestHarness, name: &str) -> i64 {
    match harness.get_output(name) {
        Some(Value::Dt(value)) => value.ticks(),
        other => panic!("{name} must be DT, got {other:?}"),
    }
}

fn compile_error(source: &str) -> String {
    match CompileSession::from_source(source).build_runtime() {
        Ok(_) => panic!("invalid runtime-clock source must fail compilation"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn runtime_clock_source_current_dt_returns_current_host_milliseconds() {
    let before = host_millis();
    let mut harness = TestHarness::from_source(
        "PROGRAM Main\nVAR stamp : DT; END_VAR\nstamp := CURRENT_DT();\nEND_PROGRAM",
    )
    .expect("CURRENT_DT source must compile");
    let cycle = harness.cycle();
    let after = host_millis();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    let ticks = u128::try_from(dt_ticks(&harness, "stamp"))
        .expect("current host milliseconds must be nonnegative");
    assert!(
        (before..=after).contains(&ticks),
        "CURRENT_DT tick {ticks} must be sampled between {before} and {after}"
    );
}

#[test]
fn runtime_clock_source_time_returns_injected_logical_time() {
    let mut harness = TestHarness::from_source(
        "PROGRAM Main\nVAR elapsed : TIME; END_VAR\nelapsed := TIME();\nEND_PROGRAM",
    )
    .expect("TIME source must compile");
    harness.advance_time(Duration::from_millis(42));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    assert_eq!(
        harness.get_output("elapsed"),
        Some(Value::Time(Duration::from_millis(42)))
    );
}

#[test]
fn runtime_clock_source_current_dt_ignores_large_logical_advance() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR elapsed : TIME; stamp : DT; END_VAR
elapsed := TIME();
stamp := CURRENT_DT();
END_PROGRAM
"#,
    )
    .expect("runtime-clock source must compile");
    harness.advance_time(Duration::from_secs(86_400));
    let before = host_millis();
    let cycle = harness.cycle();
    let after = host_millis();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    assert_eq!(
        harness.get_output("elapsed"),
        Some(Value::Time(Duration::from_secs(86_400)))
    );
    let ticks = u128::try_from(dt_ticks(&harness, "stamp"))
        .expect("current host milliseconds must be nonnegative");
    assert!(
        (before..=after).contains(&ticks),
        "logical advance must not offset CURRENT_DT: {ticks} outside {before}..={after}"
    );
}

#[test]
fn runtime_clock_source_repeated_cycles_resample_both_clock_domains() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR elapsed : TIME; stamp : DT; END_VAR
elapsed := TIME();
stamp := CURRENT_DT();
END_PROGRAM
"#,
    )
    .expect("runtime-clock source must compile");
    let first_before = host_millis();
    let first = harness.cycle();
    let first_after = host_millis();
    assert!(first.errors.is_empty(), "{:?}", first.errors);
    let first_host = u128::try_from(dt_ticks(&harness, "stamp"))
        .expect("current host milliseconds must be nonnegative");
    assert!((first_before..=first_after).contains(&first_host));
    harness.advance_time(Duration::from_millis(7));
    let second_before = host_millis();
    let second = harness.cycle();
    let second_after = host_millis();
    assert!(second.errors.is_empty(), "{:?}", second.errors);
    assert_eq!(
        harness.get_output("elapsed"),
        Some(Value::Time(Duration::from_millis(7)))
    );
    let second_host = u128::try_from(dt_ticks(&harness, "stamp"))
        .expect("current host milliseconds must be nonnegative");
    assert!((second_before..=second_after).contains(&second_host));
}

#[test]
fn runtime_clock_source_function_can_return_current_dt() {
    let before = host_millis();
    let mut harness = TestHarness::from_source(
        r#"
FUNCTION ReadClock : DT
ReadClock := CURRENT_DT();
END_FUNCTION
PROGRAM Main
VAR stamp : DT; END_VAR
stamp := ReadClock();
END_PROGRAM
"#,
    )
    .expect("CURRENT_DT function source must compile");
    let cycle = harness.cycle();
    let after = host_millis();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    let ticks = u128::try_from(dt_ticks(&harness, "stamp"))
        .expect("current host milliseconds must be nonnegative");
    assert!((before..=after).contains(&ticks));
}

#[test]
fn runtime_clock_source_names_are_case_insensitive() {
    let before = host_millis();
    let mut harness = TestHarness::from_source(
        "PROGRAM Main\nVAR elapsed : TIME; stamp : DT; END_VAR\nelapsed := time(); stamp := current_dt();\nEND_PROGRAM",
    )
    .expect("case-insensitive clock names must compile");
    let cycle = harness.cycle();
    let after = host_millis();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    assert_eq!(
        harness.get_output("elapsed"),
        Some(Value::Time(Duration::ZERO))
    );
    let ticks = u128::try_from(dt_ticks(&harness, "stamp"))
        .expect("current host milliseconds must be nonnegative");
    assert!((before..=after).contains(&ticks));
}

#[test]
fn runtime_clock_source_rejects_current_dt_arguments() {
    let error = compile_error(
        "PROGRAM Main\nVAR stamp : DT; END_VAR\nstamp := CURRENT_DT(1);\nEND_PROGRAM",
    );
    let normalized = error.to_ascii_lowercase();
    assert!(
        normalized.contains("current_dt"),
        "diagnostic must identify CURRENT_DT: {error}"
    );
    assert!(
        normalized.contains("argument") && normalized.contains('0'),
        "diagnostic must identify the zero-argument contract: {error}"
    );
}

#[test]
fn runtime_clock_source_rejects_time_arguments() {
    let error = compile_error(
        "PROGRAM Main\nVAR elapsed : TIME; END_VAR\nelapsed := TIME(1);\nEND_PROGRAM",
    );
    let normalized = error.to_ascii_lowercase();
    assert!(
        normalized.contains("time"),
        "diagnostic must identify TIME: {error}"
    );
    assert!(
        normalized.contains("argument") && normalized.contains('0'),
        "diagnostic must identify the zero-argument contract: {error}"
    );
}

#[test]
fn runtime_clock_source_rejects_current_dt_result_in_wrong_family() {
    let error = compile_error(
        "PROGRAM Main\nVAR elapsed : TIME; END_VAR\nelapsed := CURRENT_DT();\nEND_PROGRAM",
    );
    let normalized = error.to_ascii_lowercase();
    assert!(
        normalized.contains("assign") || normalized.contains("type"),
        "DT-to-TIME assignment must fail with a type diagnostic: {error}"
    );
}

#[test]
fn runtime_clock_source_emits_bytecode_module_and_bytes() {
    let session = CompileSession::from_source(
        r#"
PROGRAM Main
VAR elapsed : TIME; stamp : DT; END_VAR
elapsed := TIME();
stamp := CURRENT_DT();
END_PROGRAM
"#,
    );
    let module = session
        .build_bytecode_module()
        .expect("runtime-clock source must emit a bytecode module");
    assert!(
        !module.sections.is_empty(),
        "runtime-clock source must retain executable bytecode"
    );
    let bytes = session
        .build_bytecode_bytes()
        .expect("runtime-clock source must emit encoded bytecode");
    assert!(!bytes.is_empty(), "encoded bytecode must not be empty");
}
