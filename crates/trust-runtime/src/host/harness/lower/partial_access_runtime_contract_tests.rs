use crate::harness::{CompileSession, TestHarness};
use crate::value::Value;

fn run(source: &str) -> TestHarness {
    let mut harness = TestHarness::from_source(source)
        .unwrap_or_else(|error| panic!("partial-access fixture must compile: {error}"));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness
}

#[test]
fn partial_access_runtime_reads_all_bit_string_projection_kinds() {
    let harness = run(r#"
PROGRAM Main
VAR
    byteValue : BYTE := BYTE#16#81;
    wordValue : WORD := WORD#16#ABCD;
    dwordValue : DWORD := DWORD#16#89ABCDEF;
    lwordValue : LWORD := LWORD#16#0123456789ABCDEF;
    byteLowBit : BOOL;
    byteHighBit : BOOL;
    wordHighByte : BYTE;
    dwordHighWord : WORD;
    lwordHighDword : DWORD;
END_VAR
byteLowBit := byteValue.%X0;
byteHighBit := byteValue.7;
wordHighByte := wordValue.%B1;
dwordHighWord := dwordValue.%W1;
lwordHighDword := lwordValue.%D1;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("byteLowBit"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("byteHighBit"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("wordHighByte"), Some(Value::Byte(0xAB)));
    assert_eq!(
        harness.get_output("dwordHighWord"),
        Some(Value::Word(0x89AB))
    );
    assert_eq!(
        harness.get_output("lwordHighDword"),
        Some(Value::DWord(0x0123_4567))
    );
}

#[test]
fn partial_access_runtime_bit_write_changes_only_selected_bit() {
    let harness = run(
        "PROGRAM Main\nVAR value : BYTE := BYTE#16#A0; END_VAR\nvalue.%X1 := TRUE;\nEND_PROGRAM",
    );
    assert_eq!(harness.get_output("value"), Some(Value::Byte(0xA2)));
}

#[test]
fn partial_access_runtime_byte_write_changes_only_selected_byte() {
    let harness = run(
        "PROGRAM Main\nVAR value : DWORD := DWORD#16#12345678; END_VAR\nvalue.%B2 := BYTE#16#AB;\nEND_PROGRAM",
    );
    assert_eq!(harness.get_output("value"), Some(Value::DWord(0x12AB_5678)));
}

#[test]
fn partial_access_runtime_word_write_changes_only_selected_word() {
    let harness = run(
        "PROGRAM Main\nVAR value : LWORD := LWORD#16#0123456789ABCDEF; END_VAR\nvalue.%W1 := WORD#16#1357;\nEND_PROGRAM",
    );
    assert_eq!(
        harness.get_output("value"),
        Some(Value::LWord(0x0123_4567_1357_CDEF))
    );
}

#[test]
fn partial_access_runtime_dword_write_changes_only_selected_dword() {
    let harness = run(
        "PROGRAM Main\nVAR value : LWORD := LWORD#16#0123456789ABCDEF; END_VAR\nvalue.%D0 := DWORD#16#76543210;\nEND_PROGRAM",
    );
    assert_eq!(
        harness.get_output("value"),
        Some(Value::LWord(0x0123_4567_7654_3210))
    );
}

#[test]
fn partial_access_runtime_alias_chain_retains_projection_layout() {
    let harness = run(r#"
TYPE Status : WORD; MoreStatus : Status; END_TYPE
PROGRAM Main
VAR value : MoreStatus := MoreStatus#16#8000; high : BOOL; END_VAR
high := value.%X15;
value.%B0 := BYTE#16#AA;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("high"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("value"), Some(Value::Word(0x80AA)));
}

#[test]
fn partial_access_runtime_nested_structure_array_write_updates_owner() {
    let harness = run(r#"
TYPE Packet : STRUCT values : ARRAY[0..1] OF DWORD; END_STRUCT; END_TYPE
PROGRAM Main
VAR packet : Packet := (values := [DWORD#0, DWORD#16#12345678]); result : DWORD; END_VAR
packet.values[1].%W1 := WORD#16#ABCD;
result := packet.values[1];
END_PROGRAM
"#);
    assert_eq!(
        harness.get_output("result"),
        Some(Value::DWord(0xABCD_5678))
    );
}

#[test]
fn partial_access_runtime_pointer_projection_writes_referent() {
    let harness = run(r#"
PROGRAM Main
VAR value : WORD := WORD#16#1200; value_ptr : POINTER TO WORD; result : WORD; END_VAR
value_ptr := ADR(value);
value_ptr^.%B0 := BYTE#16#34;
result := value;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::Word(0x1234)));
}

#[test]
fn partial_access_runtime_var_in_out_projection_writes_caller() {
    let harness = run(r#"
FUNCTION SetHighByte : BOOL
VAR_IN_OUT value : WORD; END_VAR
value.%B1 := BYTE#16#AB;
SetHighByte := TRUE;
END_FUNCTION
PROGRAM Main
VAR value : WORD := WORD#16#1234; result : WORD; END_VAR
SetHighByte(value := value);
result := value;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::Word(0xAB34)));
}

#[test]
fn partial_access_runtime_case_insensitive_and_unprefixed_bit_forms_match() {
    let harness = run(r#"
PROGRAM Main
VAR value : WORD; same : BOOL; END_VAR
value.%x0 := TRUE;
value.15 := TRUE;
same := value.%X0 AND value.%X15 AND (value = WORD#16#8001);
END_PROGRAM
"#);
    assert_eq!(harness.get_output("same"), Some(Value::Bool(true)));
}

#[test]
fn partial_access_runtime_source_emits_bytecode_module_and_bytes() {
    let session = CompileSession::from_source(
        r#"
PROGRAM Main
VAR value : DWORD := DWORD#16#12345678; result : WORD; END_VAR
value.%B0 := BYTE#16#AA;
result := value.%W1;
END_PROGRAM
"#,
    );
    let module = session
        .build_bytecode_module()
        .expect("partial access must emit a bytecode module");
    assert!(
        !module.sections.is_empty(),
        "partial-access source must retain executable bytecode"
    );
    let bytes = session
        .build_bytecode_bytes()
        .expect("partial access must emit encoded bytecode");
    assert!(!bytes.is_empty(), "encoded bytecode must not be empty");
}
