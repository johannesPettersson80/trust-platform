use crate::common::*;

fn assert_generic_declaration_rejected(source: &str) {
    check_has_error(source, DiagnosticCode::InvalidOperation);
}

#[test]
fn generic_contract_rejects_any_local_storage() {
    assert_generic_declaration_rejected("PROGRAM Main\nVAR value : ANY; END_VAR\nEND_PROGRAM");
}

#[test]
fn generic_contract_rejects_any_int_input_parameter() {
    assert_generic_declaration_rejected(
        "FUNCTION Read : INT\nVAR_INPUT value : ANY_INT; END_VAR\nRead := 0;\nEND_FUNCTION",
    );
}

#[test]
fn generic_contract_rejects_any_real_output_parameter() {
    assert_generic_declaration_rejected(
        "FUNCTION Read : INT\nVAR_OUTPUT value : ANY_REAL; END_VAR\nRead := 0;\nEND_FUNCTION",
    );
}

#[test]
fn generic_contract_rejects_any_bit_in_out_parameter() {
    assert_generic_declaration_rejected(
        "FUNCTION Mutate : INT\nVAR_IN_OUT value : ANY_BIT; END_VAR\nMutate := 0;\nEND_FUNCTION",
    );
}

#[test]
fn generic_contract_rejects_any_string_temporary_storage() {
    assert_generic_declaration_rejected(
        "FUNCTION Read : INT\nVAR_TEMP value : ANY_STRING; END_VAR\nRead := 0;\nEND_FUNCTION",
    );
}

#[test]
fn generic_contract_rejects_any_date_global_storage() {
    assert_generic_declaration_rejected(
        "VAR_GLOBAL value : ANY_DATE; END_VAR\nPROGRAM Main\nEND_PROGRAM",
    );
}

#[test]
fn generic_contract_rejects_any_elementary_external_storage() {
    assert_generic_declaration_rejected(
        "PROGRAM Main\nVAR_EXTERNAL value : ANY_ELEMENTARY; END_VAR\nEND_PROGRAM",
    );
}

#[test]
fn generic_contract_rejects_any_num_function_result() {
    assert_generic_declaration_rejected("FUNCTION Generic : ANY_NUM\nGeneric := 0;\nEND_FUNCTION");
}

#[test]
fn generic_contract_rejects_alias_to_any_signed() {
    assert_generic_declaration_rejected("TYPE Generic : ANY_SIGNED; END_TYPE");
}

#[test]
fn generic_contract_rejects_any_derived_struct_member() {
    assert_generic_declaration_rejected(
        "TYPE Packet : STRUCT value : ANY_DERIVED; END_STRUCT; END_TYPE",
    );
}

#[test]
fn generic_contract_rejects_any_char_array_element() {
    assert_generic_declaration_rejected("TYPE GenericArray : ARRAY[0..1] OF ANY_CHAR; END_TYPE");
}

#[test]
fn generic_contract_rejects_any_duration_pointer_target() {
    assert_generic_declaration_rejected("TYPE GenericPointer : POINTER TO ANY_DURATION; END_TYPE");
}

#[test]
fn generic_contract_rejects_any_magnitude_reference_target() {
    assert_generic_declaration_rejected("TYPE GenericReference : REF_TO ANY_MAGNITUDE; END_TYPE");
}

#[test]
fn generic_contract_rejects_function_block_generic_state() {
    assert_generic_declaration_rejected(
        "FUNCTION_BLOCK Generic\nVAR value : ANY_CHARS; END_VAR\nEND_FUNCTION_BLOCK",
    );
}

#[test]
fn generic_contract_rejects_class_generic_state() {
    assert_generic_declaration_rejected(
        "CLASS Generic\nVAR value : ANY_UNSIGNED; END_VAR\nEND_CLASS",
    );
}

#[test]
fn generic_contract_direct_alias_to_integer_matches_numeric_formals() {
    check_no_errors(
        r#"
TYPE Count : DINT; END_TYPE
PROGRAM Main
VAR value : Count; absolute : Count; remainder : Count; END_VAR
absolute := ABS(value);
remainder := MOD(value, Count#2);
END_PROGRAM
"#,
    );
}

#[test]
fn generic_contract_alias_chain_to_integer_matches_signed_and_integer_formals() {
    check_no_errors(
        r#"
TYPE Count : DINT; MoreCount : Count; END_TYPE
PROGRAM Main
VAR value : MoreCount; absolute : MoreCount; dateValue : DATE; END_VAR
absolute := ABS(value);
dateValue := CONCAT_DATE(value, MoreCount#1, MoreCount#1);
END_PROGRAM
"#,
    );
}

#[test]
fn generic_contract_signed_subrange_matches_integer_formals() {
    check_no_errors(
        r#"
TYPE Count : INT (-10..10); END_TYPE
PROGRAM Main
VAR value : Count; absolute : INT; dateValue : DATE; END_VAR
absolute := ABS(value);
dateValue := CONCAT_DATE(value, value, value);
END_PROGRAM
"#,
    );
}

#[test]
fn generic_contract_unsigned_subrange_matches_unsigned_and_integer_formals() {
    check_no_errors(
        r#"
TYPE Count : UINT (1..10); END_TYPE
PROGRAM Main
VAR value : Count; shifted : WORD; END_VAR
shifted := SHL(WORD#1, value);
END_PROGRAM
"#,
    );
}

#[test]
fn generic_contract_bounded_strings_match_string_formals() {
    check_no_errors(
        r#"
TYPE Label : STRING[12]; WideLabel : WSTRING[12]; END_TYPE
PROGRAM Main
VAR narrow : Label; wide : WideLabel; length : INT; END_VAR
length := LEN(narrow);
length := LEN(wide);
END_PROGRAM
"#,
    );
}

#[test]
fn generic_contract_character_alias_does_not_match_string_formal() {
    check_has_error(
        r#"
TYPE Letter : CHAR; Text : STRING[8]; END_TYPE
PROGRAM Main
VAR letter : Letter; text : Text; END_VAR
text := CONCAT(letter, text);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn generic_contract_date_alias_matches_date_formal() {
    check_no_errors(
        r#"
TYPE ProductionDate : DATE; END_TYPE
PROGRAM Main
VAR production : ProductionDate; weekday : INT; END_VAR
weekday := DAY_OF_WEEK(production);
END_PROGRAM
"#,
    );
}

#[test]
fn generic_contract_bit_alias_matches_bit_formals() {
    check_no_errors(
        r#"
TYPE Flags : WORD; END_TYPE
PROGRAM Main
VAR input : Flags; output : Flags; END_VAR
output := NOT(input);
output := AND(input, Flags#16#00FF);
END_PROGRAM
"#,
    );
}

#[test]
fn generic_contract_enum_does_not_match_any_int_formal() {
    check_has_error(
        r#"
TYPE State : (Idle, Running); END_TYPE
PROGRAM Main
VAR state : State; result : INT; END_VAR
result := ABS(state);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn generic_contract_integer_based_enum_does_not_match_any_unsigned_formal() {
    check_has_error(
        r#"
TYPE State : UDINT (Idle := 0, Running := 1); END_TYPE
PROGRAM Main
VAR state : State; shifted : WORD; END_VAR
shifted := SHL(WORD#1, state);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn generic_contract_alias_to_enum_does_not_match_any_int_formal() {
    check_has_error(
        r#"
TYPE State : (Idle, Running); StateAlias : State; END_TYPE
PROGRAM Main
VAR state : StateAlias; result : INT; END_VAR
result := ABS(state);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn generic_contract_bool_does_not_match_any_int_formal() {
    check_has_error(
        r#"
PROGRAM Main
VAR flag : BOOL; dateValue : DATE; END_VAR
dateValue := CONCAT_DATE(flag, 1, 1);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn generic_contract_duration_does_not_match_any_num_formal() {
    check_has_error(
        r#"
PROGRAM Main
VAR elapsed : TIME; result : DINT; END_VAR
result := ABS(elapsed);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn generic_contract_struct_does_not_match_any_elementary_formal() {
    check_has_error(
        r#"
TYPE Packet : STRUCT value : INT; END_STRUCT; END_TYPE
PROGRAM Main
VAR left : Packet; right : Packet; same : BOOL; END_VAR
same := EQ(left, right);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}
