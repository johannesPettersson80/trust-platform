use crate::harness::{CompileSession, TestHarness};
use crate::value::Value;

fn run_output(source: &str, name: &str) -> Value {
    let mut harness = TestHarness::from_source(source)
        .unwrap_or_else(|error| panic!("expression fixture must compile: {error}"));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness
        .try_get_output(name)
        .unwrap_or_else(|error| panic!("output {name} must resolve: {error}"))
}

fn compile_error(source: &str) -> String {
    match CompileSession::from_source(source).build_runtime() {
        Ok(_) => panic!("expression fixture must fail"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn expression_lowering_contract_based_integer_literals_preserve_radix() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    result : DINT;
END_VAR
result := 2#1010 + 8#10 + 16#10;
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::DInt(34));
}

#[test]
fn expression_lowering_contract_integer_separators_are_ignored() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    result : LINT;
END_VAR
result := LINT#1_234_567;
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::LInt(1_234_567));
}

#[test]
fn expression_lowering_contract_real_exponent_preserves_fraction() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    result : BOOL;
END_VAR
result := LREAL#1.25E2 = LREAL#125.0;
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::Bool(true));
}

#[test]
fn expression_lowering_contract_parentheses_override_precedence() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    result : INT;
END_VAR
result := (INT#2 + INT#3) * INT#4;
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::Int(20));
}

#[test]
fn expression_lowering_contract_unary_operators_keep_operand_order() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    result : BOOL;
    number : INT;
END_VAR
number := -INT#5 + +INT#2;
result := (number = INT#-3) AND NOT FALSE;
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::Bool(true));
}

#[test]
fn expression_lowering_contract_typed_bit_strings_preserve_width() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    result : BOOL;
END_VAR
result := (BYTE#16#FF = BYTE#255)
    AND (WORD#16#1234 = WORD#4660)
    AND (DWORD#16#89ABCDEF = DWORD#2309737967);
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::Bool(true));
}

#[test]
fn expression_lowering_contract_wide_string_preserves_unicode_scalars() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    result : WSTRING[8];
END_VAR
result := "Ångström";
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::WString("Ångström".into()));
}

#[test]
fn expression_lowering_contract_narrow_string_dollar_escape_is_decoded() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    result : STRING[8];
END_VAR
result := 'A$$B';
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::String("A$B".into()));
}

#[test]
fn expression_lowering_contract_composite_time_literal_is_additive() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    result : BOOL;
END_VAR
result := TIME#1h2m3s4ms = TIME#3723004ms;
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::Bool(true));
}

#[test]
fn expression_lowering_contract_long_time_retains_nanoseconds() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    result : BOOL;
END_VAR
result := LTIME#2s3ns = LTIME#2000000003ns;
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::Bool(true));
}

#[test]
fn expression_lowering_contract_leap_date_orders_before_next_day() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    result : BOOL;
END_VAR
result := DATE#2024-02-29 < DATE#2024-03-01;
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::Bool(true));
}

#[test]
fn expression_lowering_contract_time_of_day_fraction_is_preserved() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    result : BOOL;
END_VAR
result := TOD#12:34:56.125 = TOD#12:34:56.125;
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::Bool(true));
}

#[test]
fn expression_lowering_contract_datetime_combines_date_and_time() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    result : BOOL;
END_VAR
result := DT#2024-02-29-12:34:56 < DT#2024-02-29-12:34:57;
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::Bool(true));
}

#[test]
fn expression_lowering_contract_array_repetition_expands_in_source_order() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    values : ARRAY[1..6] OF INT := [3(INT#1, INT#2)];
    result : INT;
END_VAR
result := values[1] + values[2] + values[3] + values[4] + values[5] + values[6];
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::Int(9));
}

#[test]
fn expression_lowering_contract_struct_initializer_keeps_named_fields() {
    let value = run_output(
        r#"
TYPE Pair :
STRUCT
    left : INT := INT#1;
    right : INT := INT#2;
END_STRUCT
END_TYPE
PROGRAM Main
VAR
    pair : Pair := (right := INT#8);
    result : INT;
END_VAR
result := pair.left + pair.right;
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::Int(9));
}

#[test]
fn expression_lowering_contract_dynamic_array_index_is_evaluated_once() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    values : ARRAY[0..2] OF INT := [INT#3, INT#5, INT#7];
    index : INT := INT#1;
    result : INT;
END_VAR
result := values[index];
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::Int(5));
}

#[test]
fn expression_lowering_contract_multidimensional_indices_keep_order() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    values : ARRAY[1..2, 3..4] OF INT;
    result : INT;
END_VAR
values[2, 3] := INT#23;
values[1, 4] := INT#14;
result := values[2, 3] * INT#10 + values[1, 4];
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::Int(244));
}

#[test]
fn expression_lowering_contract_reference_and_dereference_share_storage() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    target : INT := INT#4;
    reference : REF_TO INT;
    result : INT;
END_VAR
reference := REF(target);
reference^ := INT#12;
result := target + reference^;
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::Int(24));
}

#[test]
fn expression_lowering_contract_sizeof_type_is_lowered() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    result : DINT;
END_VAR
result := SIZEOF(WORD);
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::DInt(2));
}

#[test]
fn expression_lowering_contract_positional_call_preserves_argument_order() {
    let value = run_output(
        r#"
FUNCTION Combine : INT
VAR_INPUT
    left : INT;
    right : INT;
END_VAR
Combine := left * INT#10 + right;
END_FUNCTION
PROGRAM Main
VAR
    result : INT;
END_VAR
result := Combine(INT#3, INT#2);
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::Int(32));
}

#[test]
fn expression_lowering_contract_named_call_arguments_bind_by_name() {
    let value = run_output(
        r#"
FUNCTION Combine : INT
VAR_INPUT
    left : INT;
    right : INT;
END_VAR
Combine := left * INT#10 + right;
END_FUNCTION
PROGRAM Main
VAR
    result : INT;
END_VAR
result := Combine(right := INT#2, left := INT#3);
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::Int(32));
}

#[test]
fn expression_lowering_contract_boolean_and_short_circuits_rhs() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    guard : BOOL := FALSE;
    result : BOOL;
END_VAR
result := guard AND ((INT#1 / INT#0) = INT#0);
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::Bool(false));
}

#[test]
fn expression_lowering_contract_context_materializes_untyped_literal_as_sint() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    small : SINT;
    result : BOOL;
END_VAR
small := 127;
result := small = SINT#127;
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::Bool(true));
}

#[test]
fn expression_lowering_contract_out_of_range_contextual_literal_is_rejected() {
    let error = compile_error(
        r#"
PROGRAM Main
VAR
    small : SINT;
END_VAR
small := 128;
END_PROGRAM
"#,
    );
    assert!(error.contains("cannot assign 'INT' to 'SINT'"), "{error}");
}

#[test]
fn expression_lowering_contract_boolean_or_short_circuits_rhs() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    guard : BOOL := TRUE;
    result : BOOL;
END_VAR
result := guard OR ((INT#1 / INT#0) = INT#0);
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::Bool(true));
}

#[test]
fn expression_lowering_contract_bitwise_and_or_remain_eager_value_operators() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    result : WORD;
END_VAR
result := (WORD#16#F0F0 AND WORD#16#0FF0) OR WORD#16#0003;
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::Word(0x00F3));
}

#[test]
fn expression_lowering_contract_enum_member_retains_type_identity() {
    let value = run_output(
        r#"
TYPE Mode : (Idle, Run, Fault);
END_TYPE
PROGRAM Main
VAR
    mode : Mode := Mode#Run;
    result : BOOL;
END_VAR
result := mode = Mode#Run;
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::Bool(true));
}

#[test]
fn expression_lowering_contract_conversion_call_materializes_target_type() {
    let value = run_output(
        r#"
PROGRAM Main
VAR
    result : BOOL;
END_VAR
result := DINT_TO_LREAL(DINT#7) = LREAL#7.0;
END_PROGRAM
"#,
        "result",
    );
    assert_eq!(value, Value::Bool(true));
}

#[test]
fn expression_lowering_contract_malformed_literal_never_builds_runtime() {
    let error = compile_error(
        r#"
PROGRAM Main
VAR
    result : DINT;
END_VAR
result := 16#GG;
END_PROGRAM
"#,
    );
    assert!(!error.is_empty());
}

#[test]
fn expression_lowering_contract_unknown_call_never_builds_runtime() {
    let error = compile_error(
        r#"
PROGRAM Main
VAR
    result : INT;
END_VAR
result := MissingFunction(INT#1);
END_PROGRAM
"#,
    );
    assert!(error.contains("MissingFunction"), "{error}");
}
