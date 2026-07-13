use trust_runtime::harness::{CompileSession, TestHarness};
use trust_runtime::value::Value;

#[test]
fn function_block_output_copyback_respects_receiving_string_capacity() {
    let source = r#"
FUNCTION_BLOCK Producer
VAR_OUTPUT
    text : STRING[20];
END_VAR
text := 'ABCDEFGHIJKLMNOPQRST';
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    fb : Producer;
    narrow : STRING[5] := 'OLD';
END_VAR
fb(text => narrow);
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "unexpected cycle errors: {:?}",
        cycle.errors
    );
    assert_eq!(
        harness.get_output("narrow"),
        Some(Value::String("ABCDE".into())),
        "VAR_OUTPUT copy-back must not exceed the caller's STRING[5] capacity"
    );
}

#[test]
fn function_output_copyback_respects_receiving_string_capacity() {
    let source = r#"
FUNCTION Produce : BOOL
VAR_OUTPUT
    text : STRING[20];
END_VAR
text := 'ABCDEFGHIJKLMNOPQRST';
Produce := TRUE;
END_FUNCTION

PROGRAM Main
VAR
    narrow : STRING[5] := 'OLD';
END_VAR
Produce(narrow);
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "unexpected cycle errors: {:?}",
        cycle.errors
    );
    assert_eq!(
        harness.get_output("narrow"),
        Some(Value::String("ABCDE".into())),
        "function VAR_OUTPUT copy-back must honor the receiver's STRING[5] capacity"
    );
}

#[test]
fn nested_function_output_copyback_respects_local_string_capacity() {
    let source = r#"
FUNCTION Produce : BOOL
VAR_OUTPUT
    text : STRING[20];
END_VAR
text := 'ABCDEFGHIJKLMNOPQRST';
Produce := TRUE;
END_FUNCTION

FUNCTION CallProduce : DINT
VAR
    narrow : STRING[5] := 'OLD';
END_VAR
Produce(narrow);
CallProduce := LEN(narrow);
END_FUNCTION

PROGRAM Main
VAR
    observed_length : DINT;
END_VAR
observed_length := CallProduce();
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "unexpected cycle errors: {:?}",
        cycle.errors
    );
    assert_eq!(
        harness.get_output("observed_length"),
        Some(Value::DInt(5)),
        "nested VAR_OUTPUT copy-back must honor the caller-local STRING[5] capacity"
    );
}

#[test]
fn nested_function_output_copyback_respects_local_wstring_alias_capacity() {
    let source = r#"
TYPE NarrowWideText : WSTRING[3]; END_TYPE

FUNCTION ProduceWide : BOOL
VAR_OUTPUT
    text : WSTRING[6];
END_VAR
text := "ABCDEF";
ProduceWide := TRUE;
END_FUNCTION

FUNCTION CallProduceWide : DINT
VAR
    narrow : NarrowWideText := "OLD";
END_VAR
ProduceWide(narrow);
CallProduceWide := LEN(narrow);
END_FUNCTION

PROGRAM Main
VAR
    observed_length : DINT;
END_VAR
observed_length := CallProduceWide();
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "unexpected cycle errors: {:?}",
        cycle.errors
    );
    assert_eq!(
        harness.get_output("observed_length"),
        Some(Value::DInt(3)),
        "nested copy-back must resolve WSTRING aliases before applying capacity"
    );
}

#[test]
fn function_block_inout_rejects_mismatched_string_capacity() {
    let source = r#"
FUNCTION_BLOCK Observe
VAR_IN_OUT
    text : STRING[5];
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    fb : Observe;
    caller_text : STRING[20] := 'ABCDEFGHIJKLMNOPQRST';
END_VAR
fb(text := caller_text);
END_PROGRAM
"#;

    let error = CompileSession::from_source(source)
        .build_runtime()
        .expect_err("STRING[20] must not bind implicitly to STRING[5] VAR_IN_OUT");
    assert!(
        error.to_string().contains("error[E205]:"),
        "expected the VAR_IN_OUT type rejection category, got {error}"
    );
}

#[test]
fn function_block_wstring_output_copyback_respects_receiving_capacity() {
    let source = r#"
FUNCTION_BLOCK Producer
VAR_OUTPUT
    text : WSTRING[6];
END_VAR
text := "ABCDEF";
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    fb : Producer;
    narrow : WSTRING[3] := "OLD";
END_VAR
fb(text => narrow);
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "unexpected cycle errors: {:?}",
        cycle.errors
    );
    assert_eq!(
        harness.get_output("narrow"),
        Some(Value::WString("ABC".into())),
        "VAR_OUTPUT copy-back must count Unicode scalar values for WSTRING[3]"
    );
}

#[test]
fn function_block_inout_rejects_mismatched_wstring_capacity() {
    let source = r#"
FUNCTION_BLOCK Observe
VAR_IN_OUT
    text : WSTRING[3];
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    fb : Observe;
    caller_text : WSTRING[6] := "ABCDEF";
END_VAR
fb(text := caller_text);
END_PROGRAM
"#;

    let error = CompileSession::from_source(source)
        .build_runtime()
        .expect_err("WSTRING[6] must not bind implicitly to WSTRING[3] VAR_IN_OUT");
    assert!(
        error.to_string().contains("error[E205]:"),
        "expected the VAR_IN_OUT type rejection category, got {error}"
    );
}

#[test]
fn bounded_string_initializers_count_unicode_scalar_values() {
    let source = r#"
PROGRAM Main
VAR
    narrow : STRING[1] := 'Å';
    wide : WSTRING[1] := "Å";
END_VAR
END_PROGRAM
"#;

    let harness = TestHarness::from_source(source)
        .expect("one Unicode scalar must fit STRING[1] and WSTRING[1]");
    assert_eq!(
        harness.get_output("narrow"),
        Some(Value::String("Å".into()))
    );
    assert_eq!(harness.get_output("wide"), Some(Value::WString("Å".into())));
}

#[test]
fn bounded_string_assignment_truncates_at_unicode_scalar_boundary() {
    let source = r#"
PROGRAM Main
VAR
    source : STRING[3] := 'ÅBC';
    wide_source : WSTRING[3] := "ÅBC";
    narrow : STRING[2];
    wide_narrow : WSTRING[2];
END_VAR
narrow := source;
wide_narrow := wide_source;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "unexpected cycle errors: {:?}",
        cycle.errors
    );
    assert_eq!(
        harness.get_output("narrow"),
        Some(Value::String("ÅB".into()))
    );
    assert_eq!(
        harness.get_output("wide_narrow"),
        Some(Value::WString("ÅB".into()))
    );
}

#[test]
fn bounded_function_result_truncates_at_receiving_assignment() {
    let source = r#"
FUNCTION ProduceText : STRING[20]
ProduceText := 'ABCDEFGHIJKLMNOPQRST';
END_FUNCTION

PROGRAM Main
VAR
    narrow : STRING[5];
END_VAR
narrow := ProduceText();
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "unexpected cycle errors: {:?}",
        cycle.errors
    );
    assert_eq!(
        harness.get_output("narrow"),
        Some(Value::String("ABCDE".into()))
    );
}

#[test]
fn bounded_function_result_respects_its_declared_string_capacity() {
    let source = r#"
FUNCTION BoundText : STRING[5]
VAR_INPUT
    source : STRING[20];
END_VAR
BoundText := source;
END_FUNCTION

FUNCTION BoundWideText : WSTRING[3]
VAR_INPUT
    source : WSTRING[6];
END_VAR
BoundWideText := source;
END_FUNCTION

PROGRAM Main
VAR
    source : STRING[20] := 'ABCDEFGHIJKLMNOPQRST';
    wide_source : WSTRING[6] := "ABCDEF";
    result : STRING[20];
    wide_result : WSTRING[6];
END_VAR
result := BoundText(source);
wide_result := BoundWideText(wide_source);
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "unexpected cycle errors: {:?}",
        cycle.errors
    );
    assert_eq!(
        harness.get_output("result"),
        Some(Value::String("ABCDE".into()))
    );
    assert_eq!(
        harness.get_output("wide_result"),
        Some(Value::WString("ABC".into()))
    );
}

#[test]
fn function_input_and_equal_width_inout_preserve_string_bounds() {
    let source = r#"
FUNCTION EchoText : DINT
VAR_INPUT
    input_text : STRING[5];
END_VAR
VAR_IN_OUT
    shared_text : STRING[5];
END_VAR
shared_text := CONCAT(shared_text, 'Z');
EchoText := LEN(input_text);
END_FUNCTION

PROGRAM Main
VAR
    long_text : STRING[20] := 'ABCDEFGHIJKLMNOPQRST';
    shared : STRING[5] := 'AB';
    observed_length : DINT;
END_VAR
observed_length := EchoText(input_text := long_text, shared_text := shared);
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "unexpected cycle errors: {:?}",
        cycle.errors
    );
    assert_eq!(harness.get_output("observed_length"), Some(Value::DInt(5)));
    assert_eq!(
        harness.get_output("shared"),
        Some(Value::String("ABZ".into()))
    );
}

#[test]
fn function_inout_rejects_mismatched_string_capacity() {
    let source = r#"
FUNCTION ObserveText : BOOL
VAR_IN_OUT
    text : STRING[5];
END_VAR
ObserveText := TRUE;
END_FUNCTION

PROGRAM Main
VAR
    caller_text : STRING[20] := 'ABCDEFGHIJKLMNOPQRST';
    accepted : BOOL;
END_VAR
accepted := ObserveText(caller_text);
END_PROGRAM
"#;

    let error = CompileSession::from_source(source)
        .build_runtime()
        .expect_err("STRING[20] must not bind implicitly to function STRING[5] VAR_IN_OUT");
    assert!(
        error.to_string().contains("error[E205]:"),
        "expected the VAR_IN_OUT type rejection category, got {error}"
    );
}

#[test]
fn function_output_copyback_respects_nested_string_field_capacity() {
    let source = r#"
TYPE
    TextHolder : STRUCT
        text : STRING[5];
    END_STRUCT
END_TYPE

FUNCTION ProduceText : BOOL
VAR_OUTPUT
    text : STRING[20];
END_VAR
text := 'ABCDEFGHIJKLMNOPQRST';
ProduceText := TRUE;
END_FUNCTION

PROGRAM Main
VAR
    holder : TextHolder;
    observed : STRING[5];
END_VAR
ProduceText(holder.text);
observed := holder.text;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "unexpected cycle errors: {:?}",
        cycle.errors
    );
    assert_eq!(
        harness.get_output("observed"),
        Some(Value::String("ABCDE".into()))
    );
}

#[test]
fn function_block_input_and_equal_width_inout_preserve_string_bounds() {
    let source = r#"
FUNCTION_BLOCK Echo
VAR_INPUT
    input_text : STRING[5];
END_VAR
VAR_IN_OUT
    shared_text : STRING[5];
END_VAR
VAR_OUTPUT
    observed_input : STRING[5];
END_VAR
observed_input := input_text;
shared_text := CONCAT(shared_text, 'Z');
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    fb : Echo;
    long_text : STRING[20] := 'ABCDEFGHIJKLMNOPQRST';
    shared : STRING[5] := 'AB';
    observed : STRING[5];
END_VAR
fb(input_text := long_text, shared_text := shared, observed_input => observed);
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "unexpected cycle errors: {:?}",
        cycle.errors
    );
    assert_eq!(
        harness.get_output("observed"),
        Some(Value::String("ABCDE".into()))
    );
    assert_eq!(
        harness.get_output("shared"),
        Some(Value::String("ABZ".into()))
    );
}
