use crate::error::RuntimeError;
use crate::harness::{CompileSession, TestHarness};
use crate::value::Value;

fn run_output(source: &str, name: &str) -> Value {
    let mut harness = TestHarness::from_source(source)
        .unwrap_or_else(|error| panic!("statement fixture must compile: {error}"));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness
        .try_get_output(name)
        .unwrap_or_else(|error| panic!("output {name} must resolve: {error}"))
}

fn compile_error(source: &str) -> String {
    match CompileSession::from_source(source).build_bytecode_module() {
        Ok(_) => panic!("statement fixture must fail"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn statement_lowering_contract_assignments_execute_in_lexical_order() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    result : INT;
END_VAR
result := INT#1;
result := result + INT#2;
result := result * INT#4;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(12)
    );
}

#[test]
fn statement_lowering_contract_if_true_selects_then_only() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    result : INT;
END_VAR
IF TRUE THEN
    result := INT#1;
ELSE
    result := INT#2;
END_IF;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(1)
    );
}

#[test]
fn statement_lowering_contract_elsif_uses_first_matching_branch() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    selector : INT := INT#2;
    result : INT;
END_VAR
IF selector = INT#1 THEN
    result := INT#10;
ELSIF selector = INT#2 THEN
    result := INT#20;
ELSIF TRUE THEN
    result := INT#30;
END_IF;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(20)
    );
}

#[test]
fn statement_lowering_contract_else_runs_when_no_condition_matches() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    result : INT;
END_VAR
IF FALSE THEN
    result := INT#1;
ELSIF FALSE THEN
    result := INT#2;
ELSE
    result := INT#3;
END_IF;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(3)
    );
}

#[test]
fn statement_lowering_contract_nested_if_retains_structure() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    result : INT;
END_VAR
IF TRUE THEN
    IF FALSE THEN
        result := INT#1;
    ELSE
        result := INT#2;
    END_IF;
END_IF;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(2)
    );
}

#[test]
fn statement_lowering_contract_case_single_label_selects_exact_value() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    selector : INT := INT#2;
    result : INT;
END_VAR
CASE selector OF
    INT#1: result := INT#10;
    INT#2: result := INT#20;
END_CASE;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(20)
    );
}

#[test]
fn statement_lowering_contract_case_comma_labels_share_branch() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    selector : INT := INT#3;
    result : INT;
END_VAR
CASE selector OF
    INT#1, INT#3, INT#5: result := INT#15;
ELSE
    result := INT#0;
END_CASE;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(15)
    );
}

#[test]
fn statement_lowering_contract_case_range_includes_lower_boundary() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    selector : INT := INT#2;
    result : BOOL;
END_VAR
CASE selector OF
    INT#2..INT#4: result := TRUE;
ELSE
    result := FALSE;
END_CASE;
END_PROGRAM
"#,
            "result",
        ),
        Value::Bool(true)
    );
}

#[test]
fn statement_lowering_contract_case_range_includes_upper_boundary() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    selector : INT := INT#4;
    result : BOOL;
END_VAR
CASE selector OF
    INT#2..INT#4: result := TRUE;
ELSE
    result := FALSE;
END_CASE;
END_PROGRAM
"#,
            "result",
        ),
        Value::Bool(true)
    );
}

#[test]
fn statement_lowering_contract_case_else_handles_no_match() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    selector : INT := INT#9;
    result : INT;
END_VAR
CASE selector OF
    INT#1: result := INT#1;
ELSE
    result := INT#99;
END_CASE;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(99)
    );
}

#[test]
fn statement_lowering_contract_case_string_label_matches_value() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    selector : STRING[2] := 'B';
    result : INT;
END_VAR
CASE selector OF
    'A': result := INT#1;
    'B': result := INT#2;
ELSE
    result := INT#0;
END_CASE;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(2)
    );
}

#[test]
fn statement_lowering_contract_case_enum_label_retains_enum_identity() {
    assert_eq!(
        run_output(
            r#"
TYPE Mode : (Idle, Run, Fault);
END_TYPE
PROGRAM Main
VAR
    selector : Mode := Mode#Run;
    result : INT;
END_VAR
CASE selector OF
    Mode#Idle: result := INT#1;
    Mode#Run: result := INT#2;
ELSE
    result := INT#3;
END_CASE;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(2)
    );
}

#[test]
fn statement_lowering_contract_for_omitted_by_uses_typed_one() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    i : INT;
    result : INT;
END_VAR
FOR i := INT#1 TO INT#3 DO
    result := result + i;
END_FOR;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(6)
    );
}

#[test]
fn statement_lowering_contract_for_positive_step_is_inclusive() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    i : INT;
    result : INT;
END_VAR
FOR i := INT#1 TO INT#5 BY INT#2 DO
    result := result + i;
END_FOR;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(9)
    );
}

#[test]
fn statement_lowering_contract_for_negative_step_is_inclusive() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    i : INT;
    result : INT;
END_VAR
FOR i := INT#3 TO INT#1 BY INT#-1 DO
    result := result * INT#10 + i;
END_FOR;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(321)
    );
}

#[test]
fn statement_lowering_contract_for_zero_step_reports_runtime_error() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR
    i : INT;
END_VAR
FOR i := INT#1 TO INT#3 BY INT#0 DO
END_FOR;
END_PROGRAM
"#,
    )
    .expect("zero step is a runtime failure");
    let cycle = harness.cycle();
    assert_eq!(cycle.errors, [RuntimeError::ForStepZero]);
}

#[test]
fn statement_lowering_contract_while_can_execute_zero_iterations() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    result : INT := INT#5;
END_VAR
WHILE FALSE DO
    result := INT#9;
END_WHILE;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(5)
    );
}

#[test]
fn statement_lowering_contract_while_rechecks_before_each_iteration() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    result : INT;
END_VAR
WHILE result < INT#3 DO
    result := result + INT#1;
END_WHILE;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(3)
    );
}

#[test]
fn statement_lowering_contract_repeat_executes_before_condition() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    result : INT := INT#5;
END_VAR
REPEAT
    result := result + INT#1;
UNTIL TRUE
END_REPEAT;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(6)
    );
}

#[test]
fn statement_lowering_contract_continue_skips_remaining_loop_body() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    i : INT;
    result : INT;
END_VAR
FOR i := INT#1 TO INT#4 DO
    IF i = INT#2 THEN
        CONTINUE;
    END_IF;
    result := result * INT#10 + i;
END_FOR;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(134)
    );
}

#[test]
fn statement_lowering_contract_exit_stops_innermost_loop() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    i : INT;
    result : INT;
END_VAR
FOR i := INT#1 TO INT#5 DO
    IF i = INT#3 THEN
        EXIT;
    END_IF;
    result := result * INT#10 + i;
END_FOR;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(12)
    );
}

#[test]
fn statement_lowering_contract_nested_exit_leaves_outer_loop_running() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    outerIndex : INT;
    innerIndex : INT;
    result : INT;
END_VAR
FOR outerIndex := INT#1 TO INT#2 DO
    FOR innerIndex := INT#1 TO INT#3 DO
        result := result + INT#1;
        EXIT;
    END_FOR;
END_FOR;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(2)
    );
}

#[test]
fn statement_lowering_contract_function_return_expression_stops_body() {
    assert_eq!(
        run_output(
            r#"
FUNCTION Pick : INT
VAR_INPUT
    value : INT;
END_VAR
IF value > INT#0 THEN
    RETURN INT#7;
END_IF;
Pick := INT#9;
END_FUNCTION
PROGRAM Main
VAR
    result : INT;
END_VAR
result := Pick(INT#1);
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(7)
    );
}

#[test]
fn statement_lowering_contract_bare_return_keeps_assigned_function_result() {
    assert_eq!(
        run_output(
            r#"
FUNCTION Pick : INT
Pick := INT#6;
RETURN;
Pick := INT#9;
END_FUNCTION
PROGRAM Main
VAR
    result : INT;
END_VAR
result := Pick();
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(6)
    );
}

#[test]
fn statement_lowering_contract_fb_call_expression_statement_updates_outputs() {
    assert_eq!(
        run_output(
            r#"
FUNCTION_BLOCK Copy
VAR_INPUT
    inputValue : INT;
END_VAR
VAR_OUTPUT
    outputValue : INT;
END_VAR
outputValue := inputValue;
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    copy : Copy;
    result : INT;
END_VAR
copy(inputValue := INT#8);
result := copy.outputValue;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(8)
    );
}

#[test]
fn statement_lowering_contract_empty_statement_is_noop() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    result : INT := INT#4;
END_VAR
;
result := result + INT#1;
;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(5)
    );
}

#[test]
fn statement_lowering_contract_reference_assignment_attempt_copies_or_clears() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    target : INT := INT#5;
    first : REF_TO INT;
    second : REF_TO INT;
    result : INT;
END_VAR
first := REF(target);
second ?= first;
result := second^;
second ?= NULL;
IF second = NULL THEN
    result := result + INT#1;
END_IF;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(6)
    );
}

#[test]
fn statement_lowering_contract_bounded_string_assignment_truncates_suffix() {
    assert_eq!(
        run_output(
            r#"
PROGRAM Main
VAR
    source : STRING[8] := 'abcdef';
    result : STRING[3];
END_VAR
result := source;
END_PROGRAM
"#,
            "result",
        ),
        Value::String("abc".into())
    );
}

#[test]
fn statement_lowering_contract_jmp_is_rejected_at_bytecode_publication() {
    let error = compile_error(
        r#"
PROGRAM Main
VAR
    result : INT;
END_VAR
JMP Done;
result := INT#9;
Done: result := INT#1;
END_PROGRAM
"#,
    );
    assert!(
        error.contains("unsupported C5 edge-case lowering path"),
        "{error}"
    );
}

#[test]
fn statement_lowering_contract_unknown_jmp_label_is_rejected() {
    let error = compile_error(
        r#"
PROGRAM Main
JMP Missing;
END_PROGRAM
"#,
    );
    assert!(
        error.contains("Missing") || error.contains("undefined label"),
        "{error}"
    );
}
