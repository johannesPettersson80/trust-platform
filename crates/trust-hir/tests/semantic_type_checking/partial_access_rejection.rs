use crate::common::*;

fn assert_out_of_range(source: &str) {
    check_has_error(source, DiagnosticCode::OutOfRange);
}

fn assert_wrong_base(source: &str) {
    let errors = check_errors(source);
    assert!(
        errors.contains(&DiagnosticCode::TypeMismatch)
            || errors.contains(&DiagnosticCode::CannotResolve),
        "partial access on a non-bit-string base must be rejected: {errors:?}"
    );
}

#[test]
fn partial_access_rejects_byte_bit_above_range() {
    assert_out_of_range(
        "PROGRAM Main\nVAR value : BYTE; bit : BOOL; END_VAR\nbit := value.%X8;\nEND_PROGRAM",
    );
}

#[test]
fn partial_access_rejects_word_bit_and_byte_above_range() {
    assert_out_of_range(
        "PROGRAM Main\nVAR value : WORD; bit : BOOL; END_VAR\nbit := value.%X16;\nEND_PROGRAM",
    );
    assert_out_of_range(
        "PROGRAM Main\nVAR value : WORD; byte_value : BYTE; END_VAR\nbyte_value := value.%B2;\nEND_PROGRAM",
    );
}

#[test]
fn partial_access_rejects_dword_selectors_above_range() {
    assert_out_of_range(
        "PROGRAM Main\nVAR value : DWORD; bit : BOOL; END_VAR\nbit := value.%X32;\nEND_PROGRAM",
    );
    assert_out_of_range(
        "PROGRAM Main\nVAR value : DWORD; byte_value : BYTE; END_VAR\nbyte_value := value.%B4;\nEND_PROGRAM",
    );
    assert_out_of_range(
        "PROGRAM Main\nVAR value : DWORD; word_value : WORD; END_VAR\nword_value := value.%W2;\nEND_PROGRAM",
    );
}

#[test]
fn partial_access_rejects_lword_selectors_above_range() {
    assert_out_of_range(
        "PROGRAM Main\nVAR value : LWORD; bit : BOOL; END_VAR\nbit := value.%X64;\nEND_PROGRAM",
    );
    assert_out_of_range(
        "PROGRAM Main\nVAR value : LWORD; byte_value : BYTE; END_VAR\nbyte_value := value.%B8;\nEND_PROGRAM",
    );
    assert_out_of_range(
        "PROGRAM Main\nVAR value : LWORD; word_value : WORD; END_VAR\nword_value := value.%W4;\nEND_PROGRAM",
    );
    assert_out_of_range(
        "PROGRAM Main\nVAR value : LWORD; dword_value : DWORD; END_VAR\ndword_value := value.%D2;\nEND_PROGRAM",
    );
}

#[test]
fn partial_access_rejects_byte_projection_on_byte() {
    assert_wrong_base(
        "PROGRAM Main\nVAR value : BYTE; result : BYTE; END_VAR\nresult := value.%B0;\nEND_PROGRAM",
    );
}

#[test]
fn partial_access_rejects_word_projection_on_word() {
    assert_wrong_base(
        "PROGRAM Main\nVAR value : WORD; result : WORD; END_VAR\nresult := value.%W0;\nEND_PROGRAM",
    );
}

#[test]
fn partial_access_rejects_dword_projection_on_dword() {
    assert_wrong_base(
        "PROGRAM Main\nVAR value : DWORD; result : DWORD; END_VAR\nresult := value.%D0;\nEND_PROGRAM",
    );
}

#[test]
fn partial_access_rejects_bool_base() {
    assert_wrong_base(
        "PROGRAM Main\nVAR value : BOOL; result : BOOL; END_VAR\nresult := value.%X0;\nEND_PROGRAM",
    );
}

#[test]
fn partial_access_rejects_signed_integer_base() {
    assert_wrong_base(
        "PROGRAM Main\nVAR value : DINT; result : BOOL; END_VAR\nresult := value.%X0;\nEND_PROGRAM",
    );
}

#[test]
fn partial_access_rejects_unsigned_integer_base() {
    assert_wrong_base(
        "PROGRAM Main\nVAR value : UDINT; result : BYTE; END_VAR\nresult := value.%B0;\nEND_PROGRAM",
    );
}

#[test]
fn partial_access_rejects_enum_base() {
    assert_wrong_base(
        "TYPE State : (Idle, Run); END_TYPE\nPROGRAM Main\nVAR value : State; result : BOOL; END_VAR\nresult := value.%X0;\nEND_PROGRAM",
    );
}

#[test]
fn partial_access_rejects_array_without_element_selection() {
    assert_wrong_base(
        "PROGRAM Main\nVAR value : ARRAY[0..1] OF WORD; result : BOOL; END_VAR\nresult := value.%X0;\nEND_PROGRAM",
    );
}

#[test]
fn partial_access_rejects_struct_without_field_selection() {
    assert_wrong_base(
        "TYPE Packet : STRUCT status : WORD; END_STRUCT; END_TYPE\nPROGRAM Main\nVAR value : Packet; result : BOOL; END_VAR\nresult := value.%X0;\nEND_PROGRAM",
    );
}

#[test]
fn partial_access_rejects_wrong_bit_write_type() {
    check_has_error(
        "PROGRAM Main\nVAR value : WORD; END_VAR\nvalue.%X0 := BYTE#1;\nEND_PROGRAM",
        DiagnosticCode::IncompatibleAssignment,
    );
}

#[test]
fn partial_access_rejects_wrong_byte_write_type() {
    check_has_error(
        "PROGRAM Main\nVAR value : DWORD; END_VAR\nvalue.%B0 := WORD#1;\nEND_PROGRAM",
        DiagnosticCode::IncompatibleAssignment,
    );
}

#[test]
fn partial_access_rejects_wrong_word_write_type() {
    check_has_error(
        "PROGRAM Main\nVAR value : LWORD; END_VAR\nvalue.%W0 := DWORD#1;\nEND_PROGRAM",
        DiagnosticCode::IncompatibleAssignment,
    );
}

#[test]
fn partial_access_rejects_wrong_dword_write_type() {
    check_has_error(
        "PROGRAM Main\nVAR value : LWORD; END_VAR\nvalue.%D0 := LWORD#1;\nEND_PROGRAM",
        DiagnosticCode::IncompatibleAssignment,
    );
}

#[test]
fn partial_access_rejects_write_through_constant_base() {
    check_has_error(
        "PROGRAM Main\nVAR CONSTANT value : WORD := WORD#1; END_VAR\nvalue.%X0 := TRUE;\nEND_PROGRAM",
        DiagnosticCode::ConstantModification,
    );
}

#[test]
fn partial_access_rejects_write_through_input_base() {
    check_has_error(
        "FUNCTION Mutate : BOOL\nVAR_INPUT value : WORD; END_VAR\nvalue.%X0 := TRUE;\nMutate := FALSE;\nEND_FUNCTION",
        DiagnosticCode::InvalidAssignmentTarget,
    );
}

#[test]
fn partial_access_rejects_directly_represented_base() {
    let errors = check_errors(
        "PROGRAM Main\nVAR input AT %IW0 : WORD; result : BOOL; END_VAR\nresult := %IW0.%X0;\nEND_PROGRAM",
    );
    assert!(
        !errors.is_empty(),
        "directly represented variables must not expose partial access"
    );
}
