use crate::common::*;

#[test]
fn partial_access_accepts_all_byte_bit_reads() {
    check_no_errors(
        r#"
PROGRAM Main
VAR value : BYTE; result : BOOL; END_VAR
result := value.%X0 OR value.%X7 OR value.3;
END_PROGRAM
"#,
    );
}

#[test]
fn partial_access_accepts_all_word_projection_kinds() {
    check_no_errors(
        r#"
PROGRAM Main
VAR value : WORD; bit : BOOL; byte_value : BYTE; END_VAR
bit := value.%X15;
byte_value := value.%B1;
END_PROGRAM
"#,
    );
}

#[test]
fn partial_access_accepts_all_dword_projection_kinds() {
    check_no_errors(
        r#"
PROGRAM Main
VAR value : DWORD; bit : BOOL; byte_value : BYTE; word_value : WORD; END_VAR
bit := value.%X31;
byte_value := value.%B3;
word_value := value.%W1;
END_PROGRAM
"#,
    );
}

#[test]
fn partial_access_accepts_all_lword_projection_kinds() {
    check_no_errors(
        r#"
PROGRAM Main
VAR value : LWORD; bit : BOOL; byte_value : BYTE; word_value : WORD; dword_value : DWORD; END_VAR
bit := value.%X63;
byte_value := value.%B7;
word_value := value.%W3;
dword_value := value.%D1;
END_PROGRAM
"#,
    );
}

#[test]
fn partial_access_accepts_exactly_typed_writes() {
    check_no_errors(
        r#"
PROGRAM Main
VAR value : LWORD; END_VAR
value.%X0 := TRUE;
value.%B0 := BYTE#16#AA;
value.%W0 := WORD#16#1234;
value.%D0 := DWORD#16#89ABCDEF;
END_PROGRAM
"#,
    );
}

#[test]
fn partial_access_accepts_case_insensitive_prefixes_and_digit_separators() {
    check_no_errors(
        r#"
PROGRAM Main
VAR value : LWORD; bit : BOOL; byte_value : BYTE; word_value : WORD; dword_value : DWORD; END_VAR
bit := value.%x0_6_3;
byte_value := value.%b0_7;
word_value := value.%w0_3;
dword_value := value.%d0_1;
END_PROGRAM
"#,
    );
}

#[test]
fn partial_access_accepts_direct_alias_to_bit_string() {
    check_no_errors(
        r#"
TYPE StatusWord : WORD; END_TYPE
PROGRAM Main
VAR status : StatusWord; bit : BOOL; byte_value : BYTE; END_VAR
status.%X15 := TRUE;
bit := status.%X0;
byte_value := status.%B1;
END_PROGRAM
"#,
    );
}

#[test]
fn partial_access_accepts_alias_chain_to_bit_string() {
    check_no_errors(
        r#"
TYPE StatusWord : WORD; MoreStatus : StatusWord; END_TYPE
PROGRAM Main
VAR status : MoreStatus; bit : BOOL; END_VAR
status.%X7 := TRUE;
bit := status.%X7;
END_PROGRAM
"#,
    );
}

#[test]
fn partial_access_accepts_structure_field_base() {
    check_no_errors(
        r#"
TYPE Packet : STRUCT status : DWORD; END_STRUCT; END_TYPE
PROGRAM Main
VAR packet : Packet; word_value : WORD; END_VAR
packet.status.%W1 := WORD#16#1234;
word_value := packet.status.%W1;
END_PROGRAM
"#,
    );
}

#[test]
fn partial_access_accepts_array_element_base() {
    check_no_errors(
        r#"
PROGRAM Main
VAR values : ARRAY[1..2] OF WORD; byte_value : BYTE; END_VAR
values[2].%B1 := BYTE#16#AA;
byte_value := values[1].%B0;
END_PROGRAM
"#,
    );
}

#[test]
fn partial_access_accepts_nested_structure_array_base() {
    check_no_errors(
        r#"
TYPE Packet : STRUCT values : ARRAY[0..1] OF LWORD; END_STRUCT; END_TYPE
PROGRAM Main
VAR packet : Packet; result : DWORD; END_VAR
packet.values[1].%D1 := DWORD#16#89ABCDEF;
result := packet.values[1].%D1;
END_PROGRAM
"#,
    );
}

#[test]
fn partial_access_accepts_function_block_public_field_base() {
    check_no_errors(
        r#"
FUNCTION_BLOCK Device
VAR PUBLIC status : WORD; END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR device : Device; bit : BOOL; END_VAR
device.status.%X0 := TRUE;
bit := device.status.%X0;
END_PROGRAM
"#,
    );
}

#[test]
fn partial_access_accepts_pointer_dereference_base() {
    check_no_errors(
        r#"
PROGRAM Main
VAR value : DWORD; value_ptr : POINTER TO DWORD; word_value : WORD; END_VAR
value_ptr := ADR(value);
value_ptr^.%W1 := WORD#16#1234;
word_value := value_ptr^.%W1;
END_PROGRAM
"#,
    );
}

#[test]
fn partial_access_accepts_reference_dereference_base() {
    check_no_errors(
        r#"
PROGRAM Main
VAR value : WORD; reference : REF_TO WORD; bit : BOOL; END_VAR
reference := REF(value);
reference^.%X1 := TRUE;
bit := reference^.%X1;
END_PROGRAM
"#,
    );
}

#[test]
fn partial_access_accepts_var_in_out_alias_base() {
    check_no_errors(
        r#"
FUNCTION SetHighByte : BOOL
VAR_IN_OUT value : WORD; END_VAR
value.%B1 := BYTE#16#AA;
SetHighByte := TRUE;
END_FUNCTION
PROGRAM Main
VAR status : WORD; END_VAR
SetHighByte(value := status);
END_PROGRAM
"#,
    );
}

#[test]
fn partial_access_accepts_read_from_input_base() {
    check_no_errors(
        r#"
FUNCTION ReadBit : BOOL
VAR_INPUT value : DWORD; END_VAR
ReadBit := value.%X31;
END_FUNCTION
"#,
    );
}
