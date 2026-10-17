//! TIME-02 and TIME-08: standard-library text work in real Structured Text programs. A STRING or
//! WSTRING argument costs one unit per UTF-8 byte before the body runs, and a text result costs
//! one unit as a new value after it. Totals are the smallest scan allowance that admits the
//! first scan, as in `work_cost_model.rs`. Every STRING is bounded (80 without a length) and a
//! text result above 65,535 bytes is an error (docs/IEC_DECISIONS.md 2026-10-17).
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
    // The 16 bytes are the only difference between the two CONCAT scans, and three charges
    // count them: CONCAT's arguments (16), the argument of the assignment helper that bounds
    // the result to r's capacity of 80 (16), and the store's normalization (2 × 16). The
    // results cost one unit each whatever their length.
    assert_eq!(first_scan_work(&eight) - first_scan_work(&empty), 64);
    // Against MAX (no text): CONCAT's text result (1), and the helper's capacity constant and
    // call (2 instructions) and its text result (1).
    assert_eq!(first_scan_work(&empty) - first_scan_work(&numbers), 4);
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
    // The text result "3" (1), the assignment helper's capacity constant and call (2), its
    // one-byte argument (1) and text result (1), and the store's normalization (2 × 1).
    assert_eq!(first_scan_work(&to_text) - first_scan_work(&numbers), 7);
}

#[test]
fn one_unit_short_of_the_argument_charge_refuses_before_the_body_and_keeps_the_destination() {
    let source = program(
        "a : STRING := 'abcdefgh'; b : STRING := 'ijklmnop'; r : STRING := 'init';",
        "r := CONCAT(a, b);",
    );
    let bytes = artifact(&scheduled(&source));
    let total = first_scan_work(&source);
    // Two charges are 16 units: CONCAT's arguments and the argument of the assignment helper
    // that bounds the result to r's capacity. Every allowance that refuses one of them reports
    // requested 16. The largest leaves 15 units, one short of that charge, after the
    // instructions that precede it, and neither refusal reaches the store.
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

/// Runs the first scan of `source` under `max_work` and returns its outcome and the global `s`.
fn doubling_scan(source: &str, max_work: usize) -> (Result<(), RuntimeError>, Value) {
    let bytes = artifact(&scheduled(source));
    let limits = PreparationLimits {
        max_work,
        ..Default::default()
    };
    let prepared = PreparedModule::from_bytes(&bytes, limits).expect("admit the artifact");
    let mut state = prepared
        .instantiate(0)
        .expect("the default construction allowance admits the artifact");
    let outcome = state.execute_cycle(Duration::from_millis(10));
    let text = state.storage().get_global("s").expect("declared s").clone();
    (outcome, text)
}

#[test]
fn long_string_doubling_stops_at_the_work_allowance() {
    // TIME-02: text grows by doubling inside STRING[65535], so the work allowance stops it.
    let (outcome, _) = doubling_scan(
        &program(
            "s : STRING[65535] := 'ab'; i : INT;",
            "FOR i := 1 TO 64 DO s := CONCAT(s, s); END_FOR;",
        ),
        100_000,
    );
    let error = outcome.expect_err("doubling cannot finish 64 rounds within 100,000 units");
    assert!(
        matches!(
            error,
            RuntimeError::ExecutionLimit {
                kind: ExecutionLimitKind::Work,
                ..
            }
        ),
        "doubling must stop at the work allowance, got {error:?}"
    );
}

#[test]
fn string_doubling_past_the_implementer_maximum_is_an_error() {
    // IEC 61131-3 Ed.3 §6.6.2.5.11, Table 34: CONCAT of two 32,768-byte texts would produce
    // 65,536 bytes, one more than the implementer maximum, and fails instead of returning it.
    let (outcome, text) = doubling_scan(
        &program(
            "s : STRING[65535] := 'ab'; i : INT;",
            "FOR i := 1 TO 64 DO s := CONCAT(s, s); END_FOR;",
        ),
        10_000_000,
    );
    assert_eq!(outcome, Err(RuntimeError::StringCapacityExceeded));
    assert_eq!(text, Value::String("ab".repeat(16_384).into()));
}

#[test]
fn doubling_a_string_without_a_length_stays_at_80_bytes() {
    // docs/IEC_DECISIONS.md 2026-10-17: STRING is STRING[80], so every store cuts the doubled
    // text back to 80 bytes and the 64 rounds finish within the allowance.
    let (outcome, text) = doubling_scan(
        &program(
            "s : STRING := 'ab'; i : INT;",
            "FOR i := 1 TO 64 DO s := CONCAT(s, s); END_FOR;",
        ),
        100_000,
    );
    assert_eq!(outcome, Ok(()));
    assert_eq!(text, Value::String("ab".repeat(40).into()));
}
