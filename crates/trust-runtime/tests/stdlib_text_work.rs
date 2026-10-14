//! TIME-02 and TIME-08: standard-library text work in real Structured Text programs. A STRING or
//! WSTRING argument costs one unit per UTF-8 byte before the body runs, and a text result costs
//! one unit as a new value after it. Totals are the smallest scan allowance that admits the
//! first scan, as in `work_cost_model.rs`.
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::{ExecutionLimitKind, RuntimeError};
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

fn artifact(source: &str) -> Vec<u8> {
    CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted source")
        .encode()
        .unwrap()
}

fn scheduled(body: &str) -> String {
    format!(
        "{body}\nCONFIGURATION Conf RESOURCE Controller ON PLC TASK Periodic (INTERVAL := T#10ms, \
         PRIORITY := 0); PROGRAM Plant WITH Periodic : Main; END_RESOURCE END_CONFIGURATION"
    )
}

/// A program whose globals are declared in `globals` and whose main body is `body`.
fn program(globals: &str, body: &str) -> String {
    format!("VAR_GLOBAL {globals} END_VAR\nPROGRAM Main\n{body}\nEND_PROGRAM\n")
}

/// Runs the first scan under `max_work`. Only a work refusal of that scan may return false.
fn first_scan_admitted(bytes: &[u8], max_work: usize) -> bool {
    let limits = PreparationLimits {
        max_work,
        ..Default::default()
    };
    let prepared = PreparedModule::from_bytes(bytes, limits).expect("admit the artifact");
    let mut state = prepared
        .instantiate(0)
        .expect("the default construction allowance admits the artifact");
    match state.execute_cycle(Duration::from_millis(10)) {
        Ok(()) => true,
        Err(error) => {
            assert!(
                matches!(
                    error,
                    RuntimeError::ExecutionLimit {
                        kind: ExecutionLimitKind::Work,
                        ..
                    }
                ),
                "a refused first scan must be a work refusal, got {error:?}"
            );
            false
        }
    }
}

/// The smallest scan allowance that admits the first scan, which is the work that scan charges.
fn first_scan_work(source: &str) -> usize {
    let bytes = artifact(&scheduled(source));
    let (mut low, mut high) = (1usize, 1_000_000usize);
    assert!(
        first_scan_admitted(&bytes, high),
        "the upper probe must admit the first scan"
    );
    while low < high {
        let middle = low + (high - low) / 2;
        if first_scan_admitted(&bytes, middle) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

#[test]
fn concat_charges_one_unit_per_argument_byte_and_one_unit_for_its_result() {
    let eight = program(
        "a : STRING := 'abcdefgh'; b : STRING := 'ijklmnop'; r : STRING;",
        "r := CONCAT(a, b);",
    );
    let empty = program(
        "e : STRING := ''; f : STRING := ''; r : STRING;",
        "r := CONCAT(e, f);",
    );
    let numbers = program("i : INT := 1; j : INT := 2; n : INT;", "n := MAX(i, j);");
    // The 16 argument bytes are the only difference between the two CONCAT scans: the result
    // costs one unit whatever its length.
    assert_eq!(first_scan_work(&eight) - first_scan_work(&empty), 16);
    // Both CONCAT scans pay one unit for their text result, which MAX (no text) does not.
    assert_eq!(first_scan_work(&empty) - first_scan_work(&numbers), 1);
}

#[test]
fn length_charges_its_text_bytes_and_no_result_unit() {
    let text = program("s : STRING := 'abcdefgh'; n : INT;", "n := LEN(s);");
    let numbers = program("i : INT := 3; n : INT;", "n := ABS(i);");
    // LEN returns an INT, so the 8 bytes of its argument are the whole difference.
    assert_eq!(first_scan_work(&text) - first_scan_work(&numbers), 8);
}

#[test]
fn a_non_text_call_charges_what_a_zero_byte_text_call_charges() {
    let empty = program("s : STRING := ''; n : INT;", "n := LEN(s);");
    let numbers = program("i : INT := 3; n : INT;", "n := ABS(i);");
    assert_eq!(first_scan_work(&numbers), first_scan_work(&empty));
}

#[test]
fn conversions_charge_their_text_argument_and_their_text_result() {
    let five = program("s : STRING := '12345'; x : INT;", "x := STRING_TO_INT(s);");
    let one = program("s : STRING := '1'; x : INT;", "x := STRING_TO_INT(s);");
    assert_eq!(first_scan_work(&five) - first_scan_work(&one), 4);

    let to_text = program("i : INT := 3; s : STRING;", "s := INT_TO_STRING(i);");
    let numbers = program("i : INT := 3; n : INT;", "n := ABS(i);");
    assert_eq!(first_scan_work(&to_text) - first_scan_work(&numbers), 1);
}

#[test]
fn one_unit_short_of_the_argument_charge_refuses_before_the_body_and_keeps_the_destination() {
    let source = program(
        "a : STRING := 'abcdefgh'; b : STRING := 'ijklmnop'; r : STRING := 'init';",
        "r := CONCAT(a, b);",
    );
    let bytes = artifact(&scheduled(&source));
    let total = first_scan_work(&source);
    // Every allowance that refuses the 16-unit argument charge reports requested 16, because
    // no other charge is 16 units. The largest of them leaves 15 units, one short of that charge,
    // after the instructions that precede it.
    let mut refused_at_text = None;
    for max_work in 1..=total {
        let limits = PreparationLimits {
            max_work,
            ..Default::default()
        };
        let prepared = PreparedModule::from_bytes(&bytes, limits).expect("admit the artifact");
        let mut state = prepared
            .instantiate(0)
            .expect("the default construction allowance admits the artifact");
        if let Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used,
            requested: 16,
            limit,
        }) = state.execute_cycle(Duration::from_millis(10))
        {
            assert_eq!(limit, max_work);
            assert_eq!(
                state.storage().get_global("r"),
                Some(&Value::String("init".into())),
                "a refused argument charge must leave the destination unchanged"
            );
            refused_at_text = Some((max_work, used));
        }
    }
    let (max_work, used) =
        refused_at_text.expect("some allowance refuses the 16-unit argument charge");
    assert_eq!(used, max_work - 15, "one unit short of the 16-unit charge");
}

#[test]
fn unbounded_string_doubling_stops_at_the_work_allowance() {
    let source = scheduled(&program(
        "s : STRING := 'ab'; i : INT;",
        "FOR i := 1 TO 64 DO s := CONCAT(s, s); END_FOR;",
    ));
    let bytes = artifact(&source);
    let limits = PreparationLimits {
        max_work: 100_000,
        ..Default::default()
    };
    let prepared = PreparedModule::from_bytes(&bytes, limits).expect("admit the artifact");
    let mut state = prepared
        .instantiate(0)
        .expect("the default construction allowance admits the artifact");
    let error = state
        .execute_cycle(Duration::from_millis(10))
        .expect_err("doubling cannot finish 64 rounds within 100,000 units");
    assert!(
        matches!(
            error,
            RuntimeError::ExecutionLimit {
                kind: ExecutionLimitKind::Work,
                ..
            }
        ),
        "unbounded doubling must stop at the work allowance, got {error:?}"
    );
}
