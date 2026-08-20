mod common;
use common::*;

#[test]
fn action_semantics_share_program_variable_scope() {
    check_no_errors(
        r#"
PROGRAM Main
VAR
    value : INT;
END_VAR
ACTION Reset:
    value := INT#0;
END_ACTION
END_PROGRAM
"#,
    );
}

#[test]
fn action_semantics_share_global_variable_scope() {
    check_no_errors(
        r#"
VAR_GLOBAL
    shared : INT;
END_VAR
PROGRAM Main
ACTION Update:
    shared := shared + INT#1;
END_ACTION
END_PROGRAM
"#,
    );
}

#[test]
fn action_semantics_share_function_block_receiver_scope() {
    check_no_errors(
        r#"
FUNCTION_BLOCK Controller
VAR
    value : INT;
END_VAR
ACTION Reset:
    THIS.value := INT#0;
END_ACTION
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn action_semantics_share_inherited_function_block_scope() {
    check_no_errors(
        r#"
FUNCTION_BLOCK Base
VAR PROTECTED
    value : INT;
END_VAR
END_FUNCTION_BLOCK
FUNCTION_BLOCK Controller EXTENDS Base
ACTION Reset:
    SUPER.value := INT#0;
END_ACTION
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn action_semantics_share_owner_using_directives() {
    check_no_errors(
        r#"
NAMESPACE Plant
TYPE Counter : DINT; END_TYPE
END_NAMESPACE
PROGRAM Main
USING Plant;
VAR
    value : Counter;
END_VAR
ACTION Reset:
    value := DINT#0;
END_ACTION
END_PROGRAM
"#,
    );
}

#[test]
fn action_semantics_check_function_and_function_block_calls() {
    check_no_errors(
        r#"
FUNCTION Increment : INT
VAR_INPUT value : INT; END_VAR
Increment := value + INT#1;
END_FUNCTION
FUNCTION_BLOCK Latch
VAR_INPUT trigger : BOOL; END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    value : INT;
    latch : Latch;
END_VAR
ACTION Work:
    value := Increment(value);
    latch(trigger := TRUE);
END_ACTION
END_PROGRAM
"#,
    );
}

#[test]
fn action_semantics_allow_boolean_variable_with_same_name() {
    check_no_errors(
        r#"
PROGRAM Main
VAR
    Reset : BOOL;
END_VAR
ACTION Reset:
    Reset := FALSE;
END_ACTION
END_PROGRAM
"#,
    );
}

#[test]
fn action_semantics_allow_same_name_in_different_owners() {
    check_no_errors(
        r#"
PROGRAM First
ACTION Reset:
END_ACTION
END_PROGRAM
PROGRAM Second
ACTION Reset:
END_ACTION
END_PROGRAM
"#,
    );
}

#[test]
fn action_semantics_accept_nested_control_flow() {
    check_no_errors(
        r#"
PROGRAM Main
VAR
    i : INT;
    enabled : BOOL;
END_VAR
ACTION Work:
    IF enabled THEN
        FOR i := 0 TO 3 DO
            IF i = 2 THEN CONTINUE; END_IF;
        END_FOR;
    ELSE
        REPEAT
            i := i - INT#1;
            IF i = 0 THEN EXIT; END_IF;
        UNTIL i <= 0
        END_REPEAT;
    END_IF;
END_ACTION
END_PROGRAM
"#,
    );
}

#[test]
fn action_semantics_accept_forward_and_backward_local_jumps() {
    check_no_errors(
        r#"
PROGRAM Main
VAR value : INT; END_VAR
ACTION Work:
    JMP Later;
Earlier:
    value := INT#1;
    JMP Earlier;
Later:
    value := INT#2;
END_ACTION
END_PROGRAM
"#,
    );
}

#[test]
fn action_semantics_accept_bare_return_in_program_action() {
    check_no_errors(
        r#"
PROGRAM Main
ACTION Stop:
    RETURN;
END_ACTION
END_PROGRAM
"#,
    );
}

#[test]
fn action_semantics_accept_bare_return_in_function_block_action() {
    check_no_errors(
        r#"
FUNCTION_BLOCK Controller
ACTION Stop:
    RETURN;
END_ACTION
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn action_semantics_reject_incompatible_assignment() {
    check_has_error(
        r#"
PROGRAM Main
VAR value : INT; END_VAR
ACTION Reset:
    value := 'wrong';
END_ACTION
END_PROGRAM
"#,
        DiagnosticCode::IncompatibleAssignment,
    );
}

#[test]
fn action_semantics_reject_unresolved_name() {
    check_has_error(
        r#"
PROGRAM Main
ACTION Reset:
    missing := INT#0;
END_ACTION
END_PROGRAM
"#,
        DiagnosticCode::UndefinedVariable,
    );
}

#[test]
fn action_semantics_reject_non_boolean_condition() {
    check_has_error(
        r#"
PROGRAM Main
ACTION Work:
    IF INT#1 THEN RETURN; END_IF;
END_ACTION
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn action_semantics_reject_invalid_call_argument() {
    check_has_error(
        r#"
FUNCTION Consume : INT
VAR_INPUT value : INT; END_VAR
Consume := value;
END_FUNCTION
PROGRAM Main
ACTION Work:
    Consume('wrong');
END_ACTION
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn action_semantics_reject_constant_modification() {
    check_has_error(
        r#"
PROGRAM Main
VAR CONSTANT
    Limit : INT := INT#5;
END_VAR
ACTION Work:
    Limit := INT#6;
END_ACTION
END_PROGRAM
"#,
        DiagnosticCode::ConstantModification,
    );
}

#[test]
fn action_semantics_reject_exit_outside_action_loop() {
    check_has_error(
        "PROGRAM Main\nACTION Work:\nEXIT;\nEND_ACTION\nEND_PROGRAM",
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn action_semantics_reject_continue_outside_action_loop() {
    check_has_error(
        "PROGRAM Main\nACTION Work:\nCONTINUE;\nEND_ACTION\nEND_PROGRAM",
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn action_semantics_reject_return_value() {
    check_has_error(
        "PROGRAM Main\nACTION Work:\nRETURN INT#1;\nEND_ACTION\nEND_PROGRAM",
        DiagnosticCode::InvalidReturnType,
    );
}

#[test]
fn action_semantics_reject_duplicate_names_case_insensitively_in_one_owner() {
    check_has_error(
        r#"
PROGRAM Main
ACTION Reset:
END_ACTION
ACTION rEsEt:
END_ACTION
END_PROGRAM
"#,
        DiagnosticCode::DuplicateDeclaration,
    );
}

#[test]
fn action_semantics_reject_jump_from_action_to_owner_label() {
    check_has_error(
        r#"
PROGRAM Main
OwnerLabel:
    ;
ACTION Work:
    JMP OwnerLabel;
END_ACTION
END_PROGRAM
"#,
        DiagnosticCode::CannotResolve,
    );
}

#[test]
fn action_semantics_reject_jump_from_owner_to_action_label() {
    check_has_error(
        r#"
PROGRAM Main
JMP ActionLabel;
ACTION Work:
ActionLabel:
    ;
END_ACTION
END_PROGRAM
"#,
        DiagnosticCode::CannotResolve,
    );
}

#[test]
fn action_semantics_reject_jump_between_actions() {
    check_has_error(
        r#"
PROGRAM Main
ACTION First:
    JMP SecondLabel;
END_ACTION
ACTION Second:
SecondLabel:
    ;
END_ACTION
END_PROGRAM
"#,
        DiagnosticCode::CannotResolve,
    );
}

#[test]
fn action_semantics_reject_duplicate_labels_case_insensitively() {
    check_has_error(
        r#"
PROGRAM Main
ACTION Work:
Again:
    ;
aGaIn:
    ;
END_ACTION
END_PROGRAM
"#,
        DiagnosticCode::DuplicateDeclaration,
    );
}

#[test]
fn action_semantics_do_not_make_action_name_callable() {
    check_has_error(
        r#"
PROGRAM Main
ACTION Reset:
END_ACTION
Reset();
END_PROGRAM
"#,
        DiagnosticCode::UndefinedFunction,
    );
}

#[test]
fn action_semantics_do_not_cross_owner_local_scope() {
    check_has_error(
        r#"
PROGRAM First
VAR privateValue : INT; END_VAR
END_PROGRAM
PROGRAM Second
ACTION Work:
    privateValue := INT#1;
END_ACTION
END_PROGRAM
"#,
        DiagnosticCode::UndefinedVariable,
    );
}

#[test]
fn action_semantics_reject_this_in_program_action() {
    check_has_error(
        r#"
PROGRAM Main
ACTION Work:
    THIS.value := INT#1;
END_ACTION
END_PROGRAM
"#,
        DiagnosticCode::CannotResolve,
    );
}

#[test]
fn action_semantics_reject_super_without_base_owner() {
    check_has_error(
        r#"
FUNCTION_BLOCK Controller
VAR value : INT; END_VAR
ACTION Work:
    SUPER.value := INT#1;
END_ACTION
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::CannotResolve,
    );
}
