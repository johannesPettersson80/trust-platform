use crate::common::*;

#[test]
fn control_flow_accepts_bool_if_without_else() {
    check_no_errors(
        r#"
PROGRAM Main
VAR result : INT; END_VAR
IF TRUE THEN result := INT#1; END_IF;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_ordered_elsif_chain() {
    check_no_errors(
        r#"
PROGRAM Main
VAR first : BOOL; second : BOOL; result : INT; END_VAR
IF first THEN result := 1;
ELSIF second THEN result := 2;
ELSE result := 3;
END_IF;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_nested_if_conditions() {
    check_no_errors(
        r#"
PROGRAM Main
VAR outer : BOOL; inner : BOOL; result : BOOL; END_VAR
IF outer THEN
    IF inner THEN result := TRUE; END_IF;
END_IF;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_bool_case_labels() {
    check_no_errors(
        r#"
PROGRAM Main
VAR selector : BOOL; result : INT; END_VAR
CASE selector OF
    FALSE: result := 0;
    TRUE: result := 1;
END_CASE;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_signed_integer_case_range() {
    check_no_errors(
        r#"
PROGRAM Main
VAR selector : DINT; result : INT; END_VAR
CASE selector OF
    DINT#-10..DINT#-1: result := 1;
    DINT#0: result := 2;
    DINT#1..DINT#10: result := 3;
ELSE result := 4;
END_CASE;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_unsigned_integer_case_labels() {
    check_no_errors(
        r#"
PROGRAM Main
VAR selector : UDINT; result : INT; END_VAR
CASE selector OF
    UDINT#0, UDINT#2, UDINT#4: result := 1;
ELSE result := 2;
END_CASE;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_bit_string_case_labels() {
    check_no_errors(
        r#"
PROGRAM Main
VAR selector : WORD; result : INT; END_VAR
CASE selector OF
    WORD#0: result := 0;
    WORD#1..WORD#3: result := 1;
ELSE result := 2;
END_CASE;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_string_case_labels() {
    check_no_errors(
        r#"
PROGRAM Main
VAR selector : STRING[8]; result : INT; END_VAR
CASE selector OF
    'start': result := 1;
    'stop': result := 2;
ELSE result := 3;
END_CASE;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_wstring_case_labels() {
    check_no_errors(
        r#"
PROGRAM Main
VAR selector : WSTRING[8]; result : INT; END_VAR
CASE selector OF
    "start": result := 1;
    "stop": result := 2;
ELSE result := 3;
END_CASE;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_unqualified_enum_case_labels() {
    check_no_errors(
        r#"
TYPE Mode : (Idle, Run, Fault); END_TYPE
PROGRAM Main
VAR selector : Mode; result : INT; END_VAR
CASE selector OF
    Idle: result := 1;
    Run: result := 2;
    Fault: result := 3;
END_CASE;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_typed_enum_case_labels() {
    check_no_errors(
        r#"
TYPE Mode : (Idle, Run, Fault); END_TYPE
PROGRAM Main
VAR selector : Mode; result : INT; END_VAR
CASE selector OF
    Mode#Idle: result := 1;
    Mode#Run: result := 2;
    Mode#Fault: result := 3;
END_CASE;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_case_constant_labels() {
    check_no_errors(
        r#"
PROGRAM Main
VAR CONSTANT
    StartCode : INT := 10;
    StopCode : INT := 20;
END_VAR
VAR selector : INT; result : INT; END_VAR
CASE selector OF
    StartCode: result := 1;
    StopCode: result := 2;
ELSE result := 3;
END_CASE;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_unmatched_case_without_else() {
    check_no_errors(
        r#"
PROGRAM Main
VAR selector : INT; result : INT; END_VAR
CASE selector OF
    1: result := 1;
END_CASE;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_sint_for_with_contextual_literals() {
    check_no_errors(
        r#"
PROGRAM Main
VAR i : SINT; result : SINT; END_VAR
FOR i := 1 TO 5 BY 2 DO result := result + i; END_FOR;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_lint_for_with_typed_bounds() {
    check_no_errors(
        r#"
PROGRAM Main
VAR i : LINT; result : LINT; END_VAR
FOR i := LINT#1 TO LINT#5 BY LINT#2 DO result := result + i; END_FOR;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_uint_for_with_implicit_step() {
    check_no_errors(
        r#"
PROGRAM Main
VAR i : UINT; result : UINT; END_VAR
FOR i := UINT#1 TO UINT#5 DO result := result + i; END_FOR;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_negative_for_step() {
    check_no_errors(
        r#"
PROGRAM Main
VAR i : DINT; result : DINT; END_VAR
FOR i := DINT#5 TO DINT#1 BY DINT#-1 DO result := result + i; END_FOR;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_step_source_mutation_after_capture() {
    check_no_errors(
        r#"
PROGRAM Main
VAR i : INT; stepValue : INT := 1; result : INT; END_VAR
FOR i := 1 TO 3 BY stepValue DO
    stepValue := 2;
    result := result + i;
END_FOR;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_while_continue_and_exit() {
    check_no_errors(
        r#"
PROGRAM Main
VAR keepGoing : BOOL; END_VAR
WHILE keepGoing DO
    IF keepGoing THEN CONTINUE; END_IF;
    EXIT;
END_WHILE;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_repeat_continue_and_exit() {
    check_no_errors(
        r#"
PROGRAM Main
VAR done : BOOL; END_VAR
REPEAT
    IF done THEN EXIT; END_IF;
    CONTINUE;
UNTIL done
END_REPEAT;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_exit_in_innermost_nested_loop() {
    check_no_errors(
        r#"
PROGRAM Main
VAR i : INT; j : INT; END_VAR
FOR i := 1 TO 2 DO
    FOR j := 1 TO 2 DO EXIT; END_FOR;
END_FOR;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_continue_in_innermost_nested_loop() {
    check_no_errors(
        r#"
PROGRAM Main
VAR i : INT; j : INT; END_VAR
FOR i := 1 TO 2 DO
    WHILE TRUE DO CONTINUE; END_WHILE;
END_FOR;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_value_return_extension_with_exact_type() {
    check_no_errors(
        r#"
FUNCTION Pick : INT
VAR_INPUT value : INT; END_VAR
IF value > 0 THEN RETURN INT#1; END_IF;
RETURN INT#2;
END_FUNCTION
"#,
    );
}

#[test]
fn control_flow_accepts_bare_return_after_result_assignment() {
    check_no_errors(
        r#"
FUNCTION Pick : INT
Pick := INT#7;
RETURN;
END_FUNCTION
"#,
    );
}

#[test]
fn control_flow_accepts_bare_return_from_program() {
    check_no_errors(
        r#"
PROGRAM Main
VAR stop : BOOL; result : INT; END_VAR
IF stop THEN RETURN; END_IF;
result := 1;
END_PROGRAM
"#,
    );
}

#[test]
fn control_flow_accepts_return_from_nested_selection_in_loop() {
    check_no_errors(
        r#"
FUNCTION Find : INT
VAR i : INT; END_VAR
FOR i := 1 TO 5 DO
    CASE i OF
        3: RETURN i;
    ELSE
        ;
    END_CASE;
END_FOR;
RETURN 0;
END_FUNCTION
"#,
    );
}
