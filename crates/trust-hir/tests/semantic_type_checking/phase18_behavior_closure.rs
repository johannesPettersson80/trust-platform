use crate::common::*;

#[test]
fn modifiable_storage_assignment_targets_are_accepted() {
    check_no_errors(
        r#"
FUNCTION_BLOCK Box
    VAR_OUTPUT
        OutputValue : INT;
    END_VAR
    PROPERTY Value : INT
    SET
    END_SET
    END_PROPERTY

    METHOD Update
        VAR_IN_OUT
            LinkedValue : INT;
        END_VAR
        VAR_TEMP
            Scratch : INT;
        END_VAR
        Scratch := INT#1;
        LinkedValue := Scratch;
        OutputValue := LinkedValue;
        Value := OutputValue;
    END_METHOD
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn valid_task_and_program_binding_is_accepted() {
    check_no_errors(
        r#"
PROGRAM Main
END_PROGRAM

CONFIGURATION Conf
    RESOURCE R ON CPU
        TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH Fast : Main;
    END_RESOURCE
END_CONFIGURATION
"#,
    );
}

#[test]
fn ambiguous_target_suppresses_dependent_type_cascades() {
    let errors = check_errors(
        r#"
NAMESPACE A
VAR_GLOBAL
    Shared : INT;
END_VAR
END_NAMESPACE

NAMESPACE B
VAR_GLOBAL
    Shared : BOOL;
END_VAR
END_NAMESPACE

USING A;
USING B;

PROGRAM Main
VAR
    Result : DINT;
END_VAR
Result := Shared + DINT#1;
END_PROGRAM
"#,
    );

    assert!(
        errors.contains(&DiagnosticCode::CannotResolve),
        "expected the ambiguous target's primary diagnostic, got {errors:?}"
    );
    for cascade in [
        DiagnosticCode::TypeMismatch,
        DiagnosticCode::InvalidOperation,
        DiagnosticCode::IncompatibleAssignment,
    ] {
        assert!(
            !errors.contains(&cascade),
            "ambiguous target emitted dependent {cascade:?}: {errors:?}"
        );
    }
}

#[test]
fn integer_for_control_bounds_and_by_are_accepted() {
    check_no_errors(
        r#"
PROGRAM Main
VAR
    Index : INT;
    First : INT := INT#1;
    Last : INT := INT#10;
    Step : INT := INT#2;
END_VAR
FOR Index := First TO Last BY Step DO
END_FOR;
END_PROGRAM
"#,
    );
}

#[test]
fn explicit_time_timer_function_blocks_are_accepted() {
    check_no_errors(
        r#"
PROGRAM Main
VAR
    Pulse : TP_TIME;
    OnDelay : TON_TIME;
    OffDelay : TOF_TIME;
    PulseQ : BOOL;
    OnDelayQ : BOOL;
    OffDelayQ : BOOL;
    PulseElapsed : TIME;
    OnDelayElapsed : TIME;
    OffDelayElapsed : TIME;
END_VAR
Pulse(IN := TRUE, PT := T#1s, Q => PulseQ, ET => PulseElapsed);
OnDelay(IN := TRUE, PT := T#2s, Q => OnDelayQ, ET => OnDelayElapsed);
OffDelay(IN := FALSE, PT := T#3s, Q => OffDelayQ, ET => OffDelayElapsed);
END_PROGRAM
"#,
    );
}
