//! TIME-08 TEXT: writing one character charges 3 units per UTF-8 byte of the whole text plus
//! 12 (that is 3 × (B + 4)) before the commit. A refused write leaves the text unchanged.
//! A character write is also cut to the string's declared capacity without splitting a character
//! (spec 02 §6; IEC 61131-3 Ed.3 §6.4.2.2 Example 1 d)).
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::{ExecutionLimitKind, RuntimeError};
use trust_runtime_core::value::{Duration, RefSegment, Value};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

const SOURCE: &str = "VAR_GLOBAL
    text : STRING[6];
    wide : WSTRING[6];
    marker : CHAR;
    wide_marker : WCHAR;
END_VAR
PROGRAM Main END_PROGRAM
";

fn artifact() -> Vec<u8> {
    CompileSession::from_source(SOURCE)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted source")
        .encode()
        .unwrap()
}

/// Seeds `name` with `seed`, then writes `value` at the 1-based character `index`. Each of the
/// two writes is an engineering operation with its own allowance of `max_work`. Returns the
/// element write's outcome and the destination after it.
fn write_element(
    name: &str,
    seed: Value,
    index: i64,
    value: Value,
    max_work: usize,
) -> (Result<(), RuntimeError>, Value) {
    let limits = PreparationLimits {
        max_work,
        ..Default::default()
    };
    let prepared = PreparedModule::from_bytes(&artifact(), limits).expect("admit the artifact");
    let mut state = prepared
        .instantiate(0)
        .expect("the default construction allowance admits the artifact");
    state
        .write_global(name, seed)
        .expect("the seed fits the allowance");
    let outcome = state.write_named_path(name, &[RefSegment::Index(vec![index])], value);
    let after = state
        .storage()
        .get_global(name)
        .expect("declared global")
        .clone();
    (outcome, after)
}

#[test]
fn char_write_is_refused_one_unit_below_three_units_per_padded_byte() {
    // "åäö" is 6 UTF-8 bytes, so the write costs 3 × 6 + 12 = 30 units. Nothing before it
    // charges work, so the refusal reports 0 used. The text must stay "åäö".
    let (outcome, after) = write_element(
        "text",
        Value::String("åäö".into()),
        2,
        Value::Char(b'x'),
        29,
    );
    assert!(matches!(
        outcome,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 30,
            limit: 29,
        })
    ));
    assert_eq!(after, Value::String("åäö".into()));
}

#[test]
fn char_write_is_admitted_at_three_units_per_padded_byte() {
    let (outcome, after) = write_element(
        "text",
        Value::String("åäö".into()),
        2,
        Value::Char(b'x'),
        30,
    );
    outcome.unwrap();
    assert_eq!(after, Value::String("åxö".into()));
}

/// A shorter set of full strings, so a character write exceeds the declared capacity. The
/// element write is cut to that capacity (spec 02 §6; IEC 61131-3 Ed.3 §6.4.2.2 Example 1 d)).
const CAPACITY_SOURCE: &str = "VAR_GLOBAL
    text : STRING[3];
    wide : WSTRING[3];
    marker : CHAR;
    wide_marker : WCHAR;
END_VAR
PROGRAM Main END_PROGRAM
";

fn capacity_artifact() -> Vec<u8> {
    CompileSession::from_source(CAPACITY_SOURCE)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted source")
        .encode()
        .unwrap()
}

/// Seeds the full `name` with `seed`, then writes the 1-based character `index` with `value` through
/// the engineering boundary that resolves the declared capacity. Returns the destination after the
/// write.
fn write_capacity_element(name: &str, seed: Value, index: i64, value: Value) -> Value {
    let prepared = PreparedModule::from_bytes(&capacity_artifact(), PreparationLimits::default())
        .expect("admit the artifact");
    let mut state = prepared.instantiate(0).expect("instantiate the artifact");
    state.write_global(name, seed).expect("the seed fits");
    state
        .write_named_path(name, &[RefSegment::Index(vec![index])], value)
        .expect("the element write is admitted");
    state
        .storage()
        .get_global(name)
        .expect("declared global")
        .clone()
}

#[test]
fn a_char_write_cuts_a_full_narrow_string_to_its_declared_capacity() {
    // A full STRING[3] = 'abc'. CHAR#16#C4 is 'Ä', two UTF-8 bytes, so 'Äbc' is four bytes and is
    // cut to 'Äb' (three bytes) without splitting the character.
    let after = write_capacity_element("text", Value::String("abc".into()), 1, Value::Char(0xC4));
    assert_eq!(after, Value::String("Äb".into()));
}

#[test]
fn a_char_write_keeps_a_full_wide_string_within_its_declared_unit_capacity() {
    // A full WSTRING[3] = 'abc'. WCHAR#16#C4 is 'Ä', one UTF-16 code unit, so 'Äbc' is three units,
    // the whole WSTRING[3], and is not cut.
    let after = write_capacity_element(
        "wide",
        Value::WString("abc".into()),
        1,
        Value::WChar(0x00C4),
    );
    assert_eq!(after, Value::WString("Äbc".into()));
}

#[test]
fn wide_char_write_is_refused_one_unit_below_three_units_per_padded_byte() {
    // "αβγ" is 6 UTF-8 bytes, so the WSTRING element write costs 3 × 6 + 12 = 30 units.
    let (outcome, after) = write_element(
        "wide",
        Value::WString("αβγ".into()),
        2,
        Value::WChar(u16::from(b'x')),
        29,
    );
    assert!(matches!(
        outcome,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 30,
            limit: 29,
        })
    ));
    assert_eq!(after, Value::WString("αβγ".into()));

    let (outcome, after) = write_element(
        "wide",
        Value::WString("αβγ".into()),
        2,
        Value::WChar(u16::from(b'x')),
        30,
    );
    outcome.unwrap();
    assert_eq!(after, Value::WString("αxγ".into()));
}

/// A normal program scan copies the seeded `ch` into the full `STRING[3]` `text`, so the store
/// through the program's reference path must cut the text to its declared capacity. The authoring
/// coercion refuses a `CHAR` literal outside ASCII (`CHAR#16#C4` lowers to an integer, and
/// `CHAR#'$C4'` to a non-ASCII scalar), so `ch` is seeded with `Value::Char(0xC4)` here; the store
/// path under test is the same.
const PROGRAM_CAPACITY_SOURCE: &str = "VAR_GLOBAL
    text : STRING[3];
    ch : CHAR;
END_VAR
PROGRAM Main
    text[1] := ch;
END_PROGRAM
";

fn program_capacity_artifact() -> Vec<u8> {
    CompileSession::from_source(PROGRAM_CAPACITY_SOURCE)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted source")
        .encode()
        .unwrap()
}

#[test]
fn a_program_char_write_cuts_a_full_string_on_the_stack_vm() {
    // A full STRING[3] = 'abc'. ch is 'Ä' (0xC4), two UTF-8 bytes, so 'Äbc' is four bytes and the
    // program's element store cuts it to 'Äb' (three whole bytes) at the declared capacity.
    let prepared =
        PreparedModule::from_bytes(&program_capacity_artifact(), PreparationLimits::default())
            .expect("admit the artifact");
    let mut state = prepared.instantiate(0).expect("instantiate the artifact");
    state
        .write_global("text", Value::String("abc".into()))
        .expect("the seed fits");
    state
        .write_global("ch", Value::Char(0xC4))
        .expect("the seed fits");
    state
        .execute_cycle(Duration::from_millis(10))
        .expect("the program scans");
    let after = state
        .storage()
        .get_global("text")
        .expect("declared global")
        .clone();
    assert_eq!(after, Value::String("Äb".into()));
}

#[cfg(feature = "register-ir")]
mod register_tier_capacity {
    use trust_runtime_core::error::RuntimeError;
    use trust_runtime_core::value::{DateTimeValue, Duration, Value};
    use trust_runtime_core::vm::register::RegisterExecutionConfiguration;
    use trust_runtime_core::vm::{ExecutionServices, PreparationLimits, PreparedModule};

    use super::program_capacity_artifact;

    /// Platform services that enable tier-1: each block is selected at its first hit.
    struct Tier1;

    impl ExecutionServices for Tier1 {
        fn register_configuration(&self) -> RegisterExecutionConfiguration {
            RegisterExecutionConfiguration {
                tier1_enabled: true,
                tier1_hot_threshold: 1,
                ..RegisterExecutionConfiguration::default()
            }
        }
        fn deadline_exceeded(&self) -> bool {
            false
        }
        fn has_wall_clock(&self) -> bool {
            false
        }
        fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
            Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
        }
    }

    #[test]
    fn a_program_char_write_cuts_a_full_string_on_the_register_tier() {
        // The same program on the register IR with tier-1: the element store still resolves the
        // declared STRING[3] capacity, so 'Äbc' is cut to 'Äb'.
        let prepared =
            PreparedModule::from_bytes(&program_capacity_artifact(), PreparationLimits::default())
                .expect("admit the artifact");
        let mut state = prepared
            .instantiate_with_services(0, &Tier1)
            .expect("instantiate the artifact");
        state.set_register_profile_enabled(true);
        state
            .write_global("text", Value::String("abc".into()))
            .expect("the seed fits");
        state
            .write_global("ch", Value::Char(0xC4))
            .expect("the seed fits");
        state
            .execute_cycle(Duration::from_millis(10))
            .expect("the program scans");
        let after = state
            .storage()
            .get_global("text")
            .expect("declared global")
            .clone();
        assert_eq!(after, Value::String("Äb".into()));
        assert!(
            state.register_profile_snapshot().register_programs_executed >= 1,
            "the register IR runs the program"
        );
        assert!(
            state.tier1_snapshot().block_executions >= 1,
            "tier-1 runs the block at its first hit"
        );
    }
}
