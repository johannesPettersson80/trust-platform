use crate::error::RuntimeError;
use crate::harness::TestHarness;
use crate::value::Value;

fn run_control_flow_output(source: &str, name: &str) -> Value {
    let mut harness = TestHarness::from_source(source)
        .unwrap_or_else(|error| panic!("control-flow fixture must compile: {error}"));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness
        .try_get_output(name)
        .unwrap_or_else(|error| panic!("output {name} must resolve: {error}"))
}

#[test]
fn statement_control_flow_if_false_without_else_is_noop() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR result : INT := 7; END_VAR
IF FALSE THEN result := 9; END_IF;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(7)
    );
}

#[test]
fn statement_control_flow_if_first_true_skips_later_condition() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR result : INT; END_VAR
IF TRUE THEN
    result := 1;
ELSIF (INT#1 / INT#0) = INT#0 THEN
    result := 2;
END_IF;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(1)
    );
}

#[test]
fn statement_control_flow_elsif_stops_after_first_true_condition() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR result : INT; END_VAR
IF FALSE THEN result := 1;
ELSIF TRUE THEN result := 2;
ELSIF TRUE THEN result := 3;
ELSE result := 4;
END_IF;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(2)
    );
}

#[test]
fn statement_control_flow_case_no_match_without_else_is_noop() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR selector : INT := 9; result : INT := 7; END_VAR
CASE selector OF
    1: result := 1;
    2: result := 2;
END_CASE;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(7)
    );
}

#[test]
fn statement_control_flow_case_range_includes_negative_lower_bound() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR selector : DINT := DINT#-3; result : INT; END_VAR
CASE selector OF
    DINT#-3..DINT#-1: result := 1;
ELSE result := 2;
END_CASE;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(1)
    );
}

#[test]
fn statement_control_flow_case_range_includes_positive_upper_bound() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR selector : DINT := DINT#3; result : INT; END_VAR
CASE selector OF
    DINT#1..DINT#3: result := 1;
ELSE result := 2;
END_CASE;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(1)
    );
}

#[test]
fn statement_control_flow_case_comma_group_executes_once() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR selector : INT := 3; result : INT; END_VAR
CASE selector OF
    1, 3, 5: result := result + 1;
ELSE result := result + 10;
END_CASE;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(1)
    );
}

#[test]
fn statement_control_flow_case_bool_selects_true_label() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR selector : BOOL := TRUE; result : INT; END_VAR
CASE selector OF
    FALSE: result := 1;
    TRUE: result := 2;
END_CASE;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(2)
    );
}

#[test]
fn statement_control_flow_case_string_no_match_uses_else() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR selector : STRING[8] := 'other'; result : INT; END_VAR
CASE selector OF
    'start': result := 1;
    'stop': result := 2;
ELSE result := 3;
END_CASE;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(3)
    );
}

#[test]
fn statement_control_flow_case_enum_selects_typed_label() {
    assert_eq!(
        run_control_flow_output(
            r#"
TYPE Mode : (Idle, Run, Fault); END_TYPE
PROGRAM Main
VAR selector : Mode := Mode#Fault; result : INT; END_VAR
CASE selector OF
    Mode#Idle: result := 1;
    Mode#Run: result := 2;
    Mode#Fault: result := 3;
END_CASE;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(3)
    );
}

#[test]
fn statement_control_flow_for_positive_wrong_direction_runs_zero_times() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR i : INT; result : INT; END_VAR
FOR i := 5 TO 1 BY 1 DO result := result + 1; END_FOR;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(0)
    );
}

#[test]
fn statement_control_flow_for_negative_wrong_direction_runs_zero_times() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR i : INT; result : INT; END_VAR
FOR i := 1 TO 5 BY -1 DO result := result + 1; END_FOR;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(0)
    );
}

#[test]
fn statement_control_flow_for_zero_iteration_leaves_initial_value() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR i : INT := 9; END_VAR
FOR i := 5 TO 1 BY 1 DO ; END_FOR;
END_PROGRAM
"#,
            "i",
        ),
        Value::Int(5)
    );
}

#[test]
fn statement_control_flow_for_normal_positive_completion_leaves_first_value_beyond_bound() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR i : INT; END_VAR
FOR i := 1 TO 3 DO ; END_FOR;
END_PROGRAM
"#,
            "i",
        ),
        Value::Int(4)
    );
}

#[test]
fn statement_control_flow_for_normal_negative_completion_leaves_first_value_beyond_bound() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR i : INT; END_VAR
FOR i := 3 TO 1 BY -1 DO ; END_FOR;
END_PROGRAM
"#,
            "i",
        ),
        Value::Int(0)
    );
}

#[test]
fn statement_control_flow_for_exit_leaves_current_iteration_value() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR i : INT; END_VAR
FOR i := 1 TO 5 DO
    IF i = 3 THEN EXIT; END_IF;
END_FOR;
END_PROGRAM
"#,
            "i",
        ),
        Value::Int(3)
    );
}

#[test]
fn statement_control_flow_for_continue_performs_normal_increment() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR i : INT; result : INT; END_VAR
FOR i := 1 TO 3 DO
    IF i = 2 THEN CONTINUE; END_IF;
    result := result * 10 + i;
END_FOR;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(13)
    );
}

#[test]
fn statement_control_flow_for_captures_step_before_body_mutation() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR i : INT; stepValue : INT := 1; result : INT; END_VAR
FOR i := 1 TO 3 BY stepValue DO
    result := result * 10 + i;
    stepValue := 2;
END_FOR;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(123)
    );
}

#[test]
fn statement_control_flow_for_implicit_step_uses_unsigned_one() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR i : UINT; result : UINT; END_VAR
FOR i := UINT#1 TO UINT#3 DO result := result + i; END_FOR;
END_PROGRAM
"#,
            "result",
        ),
        Value::UInt(6)
    );
}

#[test]
fn statement_control_flow_for_zero_step_faults_before_control_or_body_mutation() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR i : INT := 7; result : INT; END_VAR
FOR i := 1 TO 3 BY 0 DO result := 9; END_FOR;
END_PROGRAM
"#,
    )
    .expect("zero step is a runtime contract");
    let cycle = harness.cycle();
    assert_eq!(cycle.errors, [RuntimeError::ForStepZero]);
    assert_eq!(harness.try_get_output("i").unwrap(), Value::Int(7));
    assert_eq!(harness.try_get_output("result").unwrap(), Value::Int(0));
}

#[test]
fn statement_control_flow_for_dynamic_zero_step_has_same_no_mutation_fault() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR i : INT := 7; stepValue : INT; result : INT; END_VAR
FOR i := 1 TO 3 BY stepValue DO result := 9; END_FOR;
END_PROGRAM
"#,
    )
    .expect("dynamic zero step is a runtime contract");
    let cycle = harness.cycle();
    assert_eq!(cycle.errors, [RuntimeError::ForStepZero]);
    assert_eq!(harness.try_get_output("i").unwrap(), Value::Int(7));
    assert_eq!(harness.try_get_output("result").unwrap(), Value::Int(0));
}

#[test]
fn statement_control_flow_for_increment_overflow_preserves_last_valid_value() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR i : SINT; result : INT; END_VAR
FOR i := SINT#126 TO SINT#127 DO result := result + 1; END_FOR;
END_PROGRAM
"#,
    )
    .expect("overflow boundary must compile");
    let cycle = harness.cycle();
    assert_eq!(cycle.errors, [RuntimeError::Overflow]);
    assert_eq!(harness.try_get_output("i").unwrap(), Value::SInt(127));
    assert_eq!(harness.try_get_output("result").unwrap(), Value::Int(2));
}

#[test]
fn statement_control_flow_while_false_runs_zero_times() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR result : INT := 7; END_VAR
WHILE FALSE DO result := 9; END_WHILE;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(7)
    );
}

#[test]
fn statement_control_flow_while_continue_rechecks_condition() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR i : INT; END_VAR
WHILE i < 3 DO
    i := i + 1;
    CONTINUE;
    i := 99;
END_WHILE;
END_PROGRAM
"#,
            "i",
        ),
        Value::Int(3)
    );
}

#[test]
fn statement_control_flow_while_exit_stops_before_remaining_body() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR result : INT; END_VAR
WHILE TRUE DO
    result := 7;
    EXIT;
    result := 9;
END_WHILE;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(7)
    );
}

#[test]
fn statement_control_flow_repeat_executes_once_when_condition_initially_true() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR result : INT; done : BOOL := TRUE; END_VAR
REPEAT result := result + 1; UNTIL done END_REPEAT;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(1)
    );
}

#[test]
fn statement_control_flow_repeat_continue_still_evaluates_until() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR i : INT; END_VAR
REPEAT
    i := i + 1;
    CONTINUE;
    i := 99;
UNTIL i >= 3
END_REPEAT;
END_PROGRAM
"#,
            "i",
        ),
        Value::Int(3)
    );
}

#[test]
fn statement_control_flow_repeat_exit_skips_remaining_body() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR result : INT; END_VAR
REPEAT
    result := 7;
    EXIT;
    result := 9;
UNTIL FALSE
END_REPEAT;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(7)
    );
}

#[test]
fn statement_control_flow_nested_exit_only_stops_innermost_loop() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR outerIndex : INT; innerIndex : INT; result : INT; END_VAR
FOR outerIndex := 1 TO 3 DO
    FOR innerIndex := 1 TO 3 DO
        result := result * 10 + outerIndex;
        EXIT;
    END_FOR;
END_FOR;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(123)
    );
}

#[test]
fn statement_control_flow_bare_program_return_stops_remaining_statements() {
    assert_eq!(
        run_control_flow_output(
            r#"
PROGRAM Main
VAR result : INT; END_VAR
result := 7;
RETURN;
result := 9;
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(7)
    );
}

#[test]
fn statement_control_flow_value_return_extension_escapes_nested_loop() {
    assert_eq!(
        run_control_flow_output(
            r#"
FUNCTION Pick : INT
VAR i : INT; END_VAR
FOR i := 1 TO 5 DO
    IF i = 3 THEN RETURN i; END_IF;
END_FOR;
RETURN 0;
END_FUNCTION
PROGRAM Main
VAR result : INT; END_VAR
result := Pick();
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(3)
    );
}

#[test]
fn statement_control_flow_bare_function_return_preserves_assigned_result() {
    assert_eq!(
        run_control_flow_output(
            r#"
FUNCTION Pick : INT
Pick := 7;
RETURN;
Pick := 9;
END_FUNCTION
PROGRAM Main
VAR result : INT; END_VAR
result := Pick();
END_PROGRAM
"#,
            "result",
        ),
        Value::Int(7)
    );
}
