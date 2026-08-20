use crate::error::RuntimeError;
use crate::harness::{CompileSession, TestHarness};
use crate::value::Value;

fn run(source: &str) -> TestHarness {
    let mut harness = TestHarness::from_source(source)
        .unwrap_or_else(|error| panic!("string-index fixture must compile: {error}"));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness
}

fn run_fault(source: &str) -> (TestHarness, Vec<RuntimeError>) {
    let mut harness = TestHarness::from_source(source)
        .unwrap_or_else(|error| panic!("string-index fault fixture must compile: {error}"));
    let cycle = harness.cycle();
    (harness, cycle.errors)
}

#[test]
fn string_index_runtime_reads_narrow_and_wide_elements() {
    let harness = run(r#"
PROGRAM Main
VAR
    narrow : STRING[8] := 'AB';
    wide : WSTRING[8] := "CD";
    narrowResult : CHAR;
    wideResult : WCHAR;
END_VAR
narrowResult := narrow[2];
wideResult := wide[1];
END_PROGRAM
"#);
    assert_eq!(harness.get_output("narrowResult"), Some(Value::Char(b'B')));
    assert_eq!(
        harness.get_output("wideResult"),
        Some(Value::WChar('C' as u16))
    );
}

#[test]
fn string_index_runtime_writes_replace_exactly_one_element() {
    let harness = run(r#"
PROGRAM Main
VAR narrow : STRING[8] := 'ABCD'; wide : WSTRING[8] := "WXYZ"; END_VAR
narrow[2] := 'Q';
wide[3] := "R";
END_PROGRAM
"#);
    assert_eq!(
        harness.get_output("narrow"),
        Some(Value::String("AQCD".into()))
    );
    assert_eq!(
        harness.get_output("wide"),
        Some(Value::WString("WXRZ".to_string()))
    );
}

#[test]
fn string_index_runtime_uses_one_based_positions() {
    let harness = run(r#"
PROGRAM Main
VAR text : STRING[4] := 'ABCD'; first : CHAR; last : CHAR; END_VAR
first := text[1];
last := text[4];
END_PROGRAM
"#);
    assert_eq!(harness.get_output("first"), Some(Value::Char(b'A')));
    assert_eq!(harness.get_output("last"), Some(Value::Char(b'D')));
}

#[test]
fn string_index_runtime_counts_multibyte_text_as_elements() {
    let harness = run(r#"
PROGRAM Main
VAR text : WSTRING[4] := "ÅB"; first : WCHAR; second : WCHAR; END_VAR
first := text[1];
second := text[2];
END_PROGRAM
"#);
    assert_eq!(harness.get_output("first"), Some(Value::WChar('Å' as u16)));
    assert_eq!(harness.get_output("second"), Some(Value::WChar('B' as u16)));
}

#[test]
fn string_index_runtime_alias_chain_retains_family_and_capacity() {
    let harness = run(r#"
TYPE Label : STRING[4]; MoreLabel : Label; END_TYPE
PROGRAM Main
VAR text : MoreLabel := 'AB'; result : CHAR; END_VAR
text[2] := 'Z';
result := text[2];
END_PROGRAM
"#);
    assert_eq!(harness.get_output("text"), Some(Value::String("AZ".into())));
    assert_eq!(harness.get_output("result"), Some(Value::Char(b'Z')));
}

#[test]
fn string_index_runtime_nested_structure_array_write_updates_owner() {
    let harness = run(r#"
TYPE Packet : STRUCT labels : ARRAY[0..1] OF STRING[4]; END_STRUCT; END_TYPE
PROGRAM Main
VAR packet : Packet; result : STRING[4]; END_VAR
packet.labels[1] := 'CD';
packet.labels[1][2] := 'Z';
result := packet.labels[1];
END_PROGRAM
"#);
    assert_eq!(
        harness.get_output("result"),
        Some(Value::String("CZ".into()))
    );
}

#[test]
fn string_index_runtime_pointer_write_updates_referent() {
    let harness = run(r#"
PROGRAM Main
VAR text : STRING[4] := 'AB'; text_ptr : POINTER TO STRING[4]; result : STRING[4]; END_VAR
text_ptr := ADR(text);
text_ptr^[1] := 'Z';
result := text;
END_PROGRAM
"#);
    assert_eq!(
        harness.get_output("result"),
        Some(Value::String("ZB".into()))
    );
}

#[test]
fn string_index_runtime_var_in_out_write_updates_caller() {
    let harness = run(r#"
FUNCTION ReplaceSecond : BOOL
VAR_IN_OUT text : STRING[4]; END_VAR
text[2] := 'Z';
ReplaceSecond := TRUE;
END_FUNCTION
PROGRAM Main
VAR text : STRING[4] := 'AB'; END_VAR
ReplaceSecond(text := text);
END_PROGRAM
"#);
    assert_eq!(harness.get_output("text"), Some(Value::String("AZ".into())));
}

#[test]
fn string_index_runtime_dynamic_zero_fault_is_atomic() {
    let (harness, errors) = run_fault(
        r#"
PROGRAM Main
VAR text : STRING[4] := 'AB'; index : INT; sentinel : INT := 7; END_VAR
text[index] := 'Z';
sentinel := 9;
END_PROGRAM
"#,
    );
    assert_eq!(
        errors,
        [RuntimeError::IndexOutOfBounds {
            index: 0,
            lower: 1,
            upper: i64::MAX
        }]
    );
    assert_eq!(harness.get_output("text"), Some(Value::String("AB".into())));
    assert_eq!(harness.get_output("sentinel"), Some(Value::Int(7)));
}

#[test]
fn string_index_runtime_dynamic_index_beyond_current_length_faults() {
    let (harness, errors) = run_fault(
        r#"
PROGRAM Main
VAR text : STRING[8] := 'AB'; index : INT := 3; sentinel : BOOL := TRUE; END_VAR
text[index] := 'Z';
sentinel := FALSE;
END_PROGRAM
"#,
    );
    assert_eq!(
        errors,
        [RuntimeError::IndexOutOfBounds {
            index: 3,
            lower: 1,
            upper: 2
        }]
    );
    assert_eq!(harness.get_output("text"), Some(Value::String("AB".into())));
    assert_eq!(harness.get_output("sentinel"), Some(Value::Bool(true)));
}

#[test]
fn string_index_runtime_failed_read_preserves_assignment_target() {
    let (harness, errors) = run_fault(
        r#"
PROGRAM Main
VAR text : WSTRING[8] := "AB"; index : INT := 3; result : WCHAR; END_VAR
result := text[index];
END_PROGRAM
"#,
    );
    assert_eq!(
        errors,
        [RuntimeError::IndexOutOfBounds {
            index: 3,
            lower: 1,
            upper: 2
        }]
    );
    assert_eq!(harness.get_output("result"), Some(Value::WChar(0)));
}

#[test]
fn string_index_runtime_source_emits_bytecode_module_and_bytes() {
    let session = CompileSession::from_source(
        r#"
PROGRAM Main
VAR text : STRING[4] := 'AB'; result : CHAR; END_VAR
text[2] := 'Z';
result := text[2];
END_PROGRAM
"#,
    );
    let module = session
        .build_bytecode_module()
        .expect("string indexing must emit a bytecode module");
    assert!(
        !module.sections.is_empty(),
        "string-index source must retain executable bytecode"
    );
    let bytes = session
        .build_bytecode_bytes()
        .expect("string indexing must emit encoded bytecode");
    assert!(!bytes.is_empty(), "encoded bytecode must not be empty");
}
