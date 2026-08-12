use crate::common::*;

#[test]
fn string_index_contract_accepts_narrow_read_and_write() {
    check_no_errors(
        "PROGRAM Main\nVAR text : STRING[8] := 'AB'; value : CHAR; END_VAR\nvalue := text[1]; text[2] := 'C';\nEND_PROGRAM",
    );
}

#[test]
fn string_index_contract_accepts_wide_read_and_write() {
    check_no_errors(
        "PROGRAM Main\nVAR text : WSTRING[8] := \"AB\"; value : WCHAR; END_VAR\nvalue := text[1]; text[2] := \"C\";\nEND_PROGRAM",
    );
}

#[test]
fn string_index_contract_accepts_every_integer_index_family() {
    check_no_errors(
        r#"
PROGRAM Main
VAR
    text : STRING[8] := 'ABCDEFGH';
    signedIndex : DINT := 1;
    unsignedIndex : UINT := 2;
    narrowSigned : SINT := 3;
    wideUnsigned : ULINT := 4;
    value : CHAR;
END_VAR
value := text[signedIndex];
value := text[unsignedIndex];
value := text[narrowSigned];
value := text[wideUnsigned];
END_PROGRAM
"#,
    );
}

#[test]
fn string_index_contract_accepts_aliases_with_capacity() {
    check_no_errors(
        r#"
TYPE Label : STRING[8]; MoreLabel : Label; WideLabel : WSTRING[8]; END_TYPE
PROGRAM Main
VAR narrow : MoreLabel := 'AB'; wide : WideLabel := "CD"; ch : CHAR; wch : WCHAR; END_VAR
ch := narrow[1];
narrow[2] := 'Z';
wch := wide[1];
wide[2] := "Y";
END_PROGRAM
"#,
    );
}

#[test]
fn string_index_contract_accepts_structure_field_base() {
    check_no_errors(
        r#"
TYPE Packet : STRUCT label : STRING[8]; END_STRUCT; END_TYPE
PROGRAM Main
VAR packet : Packet := (label := 'AB'); ch : CHAR; END_VAR
packet.label[2] := 'C';
ch := packet.label[1];
END_PROGRAM
"#,
    );
}

#[test]
fn string_index_contract_accepts_array_element_base() {
    check_no_errors(
        r#"
PROGRAM Main
VAR labels : ARRAY[0..1] OF STRING[8] := ['AB', 'CD']; ch : CHAR; END_VAR
labels[1][2] := 'Z';
ch := labels[0][1];
END_PROGRAM
"#,
    );
}

#[test]
fn string_index_contract_accepts_pointer_dereference_base() {
    check_no_errors(
        r#"
PROGRAM Main
VAR text : STRING[8] := 'AB'; text_ptr : POINTER TO STRING[8]; ch : CHAR; END_VAR
text_ptr := ADR(text);
text_ptr^[2] := 'Z';
ch := text_ptr^[1];
END_PROGRAM
"#,
    );
}

#[test]
fn string_index_contract_accepts_var_in_out_alias_base() {
    check_no_errors(
        r#"
FUNCTION ReplaceFirst : BOOL
VAR_IN_OUT text : STRING[8]; END_VAR
text[1] := 'Z';
ReplaceFirst := TRUE;
END_FUNCTION
PROGRAM Main
VAR text : STRING[8] := 'AB'; END_VAR
ReplaceFirst(text := text);
END_PROGRAM
"#,
    );
}

#[test]
fn string_index_contract_accepts_read_from_input() {
    check_no_errors(
        "FUNCTION First : CHAR\nVAR_INPUT text : STRING[8]; END_VAR\nFirst := text[1];\nEND_FUNCTION",
    );
}

#[test]
fn string_index_contract_accepts_literal_at_bounded_capacity_edge() {
    check_no_errors(
        "PROGRAM Main\nVAR text : STRING[4] := 'ABCD'; ch : CHAR; END_VAR\nch := text[4];\nEND_PROGRAM",
    );
}

#[test]
fn string_index_contract_rejects_zero_literal_index() {
    check_has_error(
        "PROGRAM Main\nVAR text : STRING[4]; ch : CHAR; END_VAR\nch := text[0];\nEND_PROGRAM",
        DiagnosticCode::OutOfRange,
    );
}

#[test]
fn string_index_contract_rejects_negative_literal_index() {
    check_has_error(
        "PROGRAM Main\nVAR text : STRING[4]; ch : CHAR; END_VAR\nch := text[-1];\nEND_PROGRAM",
        DiagnosticCode::OutOfRange,
    );
}

#[test]
fn string_index_contract_rejects_literal_above_declared_capacity() {
    check_has_error(
        "PROGRAM Main\nVAR text : STRING[4]; ch : CHAR; END_VAR\nch := text[5];\nEND_PROGRAM",
        DiagnosticCode::OutOfRange,
    );
}

#[test]
fn string_index_contract_rejects_multiple_indexes() {
    check_has_error(
        "PROGRAM Main\nVAR text : STRING[4]; ch : CHAR; END_VAR\nch := text[1, 2];\nEND_PROGRAM",
        DiagnosticCode::InvalidArrayIndex,
    );
}

#[test]
fn string_index_contract_rejects_real_index() {
    check_has_error(
        "PROGRAM Main\nVAR text : STRING[4]; index : REAL; ch : CHAR; END_VAR\nch := text[index];\nEND_PROGRAM",
        DiagnosticCode::InvalidArrayIndex,
    );
}

#[test]
fn string_index_contract_rejects_bool_index() {
    check_has_error(
        "PROGRAM Main\nVAR text : STRING[4]; index : BOOL; ch : CHAR; END_VAR\nch := text[index];\nEND_PROGRAM",
        DiagnosticCode::InvalidArrayIndex,
    );
}

#[test]
fn string_index_contract_rejects_bit_string_index() {
    check_has_error(
        "PROGRAM Main\nVAR text : STRING[4]; index : WORD; ch : CHAR; END_VAR\nch := text[index];\nEND_PROGRAM",
        DiagnosticCode::InvalidArrayIndex,
    );
}

#[test]
fn string_index_contract_rejects_wchar_write_to_string() {
    check_has_error(
        "PROGRAM Main\nVAR text : STRING[4] := 'AB'; wide : WCHAR; END_VAR\ntext[1] := wide;\nEND_PROGRAM",
        DiagnosticCode::IncompatibleAssignment,
    );
}

#[test]
fn string_index_contract_rejects_char_write_to_wstring() {
    check_has_error(
        "PROGRAM Main\nVAR text : WSTRING[4] := \"AB\"; narrow : CHAR; END_VAR\ntext[1] := narrow;\nEND_PROGRAM",
        DiagnosticCode::IncompatibleAssignment,
    );
}

#[test]
fn string_index_contract_rejects_string_value_as_character_write() {
    check_has_error(
        "PROGRAM Main\nVAR text : STRING[4] := 'AB'; replacement : STRING[1] := 'Z'; END_VAR\ntext[1] := replacement;\nEND_PROGRAM",
        DiagnosticCode::IncompatibleAssignment,
    );
}

#[test]
fn string_index_contract_rejects_write_through_constant() {
    check_has_error(
        "PROGRAM Main\nVAR CONSTANT text : STRING[4] := 'AB'; END_VAR\ntext[1] := 'Z';\nEND_PROGRAM",
        DiagnosticCode::ConstantModification,
    );
}

#[test]
fn string_index_contract_rejects_write_through_input() {
    check_has_error(
        "FUNCTION Mutate : BOOL\nVAR_INPUT text : STRING[4]; END_VAR\ntext[1] := 'Z';\nMutate := FALSE;\nEND_FUNCTION",
        DiagnosticCode::InvalidAssignmentTarget,
    );
}
