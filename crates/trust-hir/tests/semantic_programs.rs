mod common;
use common::*;

// CODESYS program-singleton access (vendor extension): a PROGRAM name
// qualifies its own members from other POUs.

#[test]
fn test_program_singleton_member_read() {
    check_no_errors(
        r#"
PROGRAM Producer
    VAR
        counter : INT;
    END_VAR
END_PROGRAM
PROGRAM Main
    VAR
        seen : INT;
    END_VAR
    seen := Producer.counter;
END_PROGRAM
"#,
    );
}

#[test]
fn test_program_singleton_member_write() {
    check_no_errors(
        r#"
PROGRAM Consumer
    VAR
        setpoint : INT;
    END_VAR
END_PROGRAM
PROGRAM Main
END_PROGRAM
"#,
    );
    // Write form exercised in a separate single-source check below to keep
    // this file's fixtures minimal.
}

#[test]
fn test_program_singleton_member_write_statement() {
    check_no_errors(
        r#"
PROGRAM Consumer
    VAR
        setpoint : INT;
    END_VAR
END_PROGRAM
PROGRAM Main
    VAR
    END_VAR
    Consumer.setpoint := 5;
END_PROGRAM
"#,
    );
}

#[test]
fn test_program_singleton_fb_instance_call() {
    check_no_errors(
        r#"
FUNCTION_BLOCK FBPulse
    VAR_INPUT
        x : BOOL;
    END_VAR
END_FUNCTION_BLOCK
PROGRAM Owner
    VAR
        fb : FBPulse;
    END_VAR
END_PROGRAM
PROGRAM Main
    VAR
    END_VAR
    Owner.fb(x := TRUE);
END_PROGRAM
"#,
    );
}

#[test]
fn test_program_direct_invocation() {
    check_no_errors(
        r#"
PROGRAM I18nConfig
    VAR
        language : INT;
    END_VAR
END_PROGRAM
PROGRAM Main
    VAR
    END_VAR
    I18nConfig();
END_PROGRAM
"#,
    );
}

#[test]
fn test_program_invocation_with_arguments() {
    // CODESYS calling convention (spec 04 §4): program calls accept and
    // check VAR_INPUT bindings like function-block calls.
    check_no_errors(
        r#"
PROGRAM Configured
    VAR_INPUT level : INT; END_VAR
END_PROGRAM
PROGRAM Main
    Configured(5);
    Configured(level := 7);
END_PROGRAM
"#,
    );
}

#[test]
fn test_program_invocation_argument_mismatch_is_checked() {
    check_has_error(
        r#"
PROGRAM Configured
    VAR_INPUT level : INT; END_VAR
END_PROGRAM
PROGRAM Main
    Configured(1, 2, 3);
END_PROGRAM
"#,
        DiagnosticCode::WrongArgumentCount,
    );
}

#[test]
fn test_program_bare_name_still_not_a_value() {
    check_has_error(
        r#"
PROGRAM OtherPrg
    VAR
        counter : INT;
    END_VAR
END_PROGRAM
PROGRAM Main
    VAR
        x : INT;
    END_VAR
    x := OtherPrg;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_program_missing_member_reports_diagnostic() {
    check_has_error(
        r#"
PROGRAM OtherPrg
    VAR
        counter : INT;
    END_VAR
END_PROGRAM
PROGRAM Main
    VAR
        x : INT;
    END_VAR
    x := OtherPrg.nope;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_program_singleton_fb_member_chain() {
    // Deep singleton chains resolve: ProgramName.fbInstance.member walks the
    // program child and then the FB instance's members. External access is
    // exercised on public (VAR_INPUT/VAR_OUTPUT) members; plain VAR members
    // stay PROTECTED and keep rejecting external access.
    check_no_errors(
        r#"
FUNCTION_BLOCK Worker
    VAR_INPUT
        level : INT;
    END_VAR
    VAR_OUTPUT
        done : BOOL;
    END_VAR
END_FUNCTION_BLOCK
PROGRAM OtherPrg
    VAR
        fb : Worker;
    END_VAR
END_PROGRAM
PROGRAM Main
    VAR
        ok : BOOL;
    END_VAR
    OtherPrg.fb.level := 5;
    ok := OtherPrg.fb.done;
END_PROGRAM
"#,
    );
}

#[test]
fn test_program_singleton_fb_plain_var_is_accessible() {
    // Classic IEC: FB members are reachable through the instance name, so
    // the deep singleton chain reads plain VAR members too (s4 T7).
    check_no_errors(
        r#"
FUNCTION_BLOCK Worker
    VAR
        state : INT;
    END_VAR
END_FUNCTION_BLOCK
PROGRAM OtherPrg
    VAR
        fb : Worker;
    END_VAR
END_PROGRAM
PROGRAM Main
    VAR
        ok : INT;
    END_VAR
    ok := OtherPrg.fb.state;
END_PROGRAM
"#,
    );
}

// e!COCKPIT dialect: program-scoped METHODs (spec 04 §4). The raw imported
// form nests `METHOD ... END_METHOD` inside the program body; the program
// body calls them by bare name without a THIS qualifier.

#[test]
fn test_program_method_bare_call_resolves() {
    check_no_errors(
        r#"
PROGRAM MqttSubscribeMulti
VAR
    flag : BOOL;
END_VAR
    flag := StateToBool('1');
METHOD PUBLIC StateToBool : BOOL
VAR_INPUT
    sInput : STRING;
END_VAR
    StateToBool := sInput = '1';
END_METHOD
END_PROGRAM
"#,
    );
}

#[test]
fn test_program_method_reads_owner_variables() {
    check_no_errors(
        r#"
PROGRAM Main
VAR
    count : INT;
    done : BOOL;
END_VAR
    count := count + 1;
    done := IsDone();
METHOD IsDone : BOOL
    IsDone := count >= 3;
END_METHOD
END_PROGRAM
"#,
    );
}

#[test]
fn test_program_method_is_callable_across_programs() {
    check_no_errors(
        r#"
PROGRAM Owner
METHOD PUBLIC Helper : INT
VAR_INPUT value : INT; END_VAR
    Helper := value * 2;
END_METHOD
END_PROGRAM
PROGRAM Main
VAR
    doubled : INT;
END_VAR
    doubled := Owner.Helper(21);
END_PROGRAM
"#,
    );
}

#[test]
fn test_program_method_call_checks_argument_type() {
    check_has_error(
        r#"
PROGRAM Main
VAR
    flag : BOOL;
END_VAR
    flag := StateToBool(3);
METHOD PUBLIC StateToBool : BOOL
VAR_INPUT
    sInput : STRING;
END_VAR
    StateToBool := sInput = '1';
END_METHOD
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn test_program_method_call_checks_argument_count() {
    check_has_error(
        r#"
PROGRAM Main
VAR
    flag : BOOL;
END_VAR
    flag := StateToBool('1', '2');
METHOD PUBLIC StateToBool : BOOL
VAR_INPUT
    sInput : STRING;
END_VAR
    StateToBool := sInput = '1';
END_METHOD
END_PROGRAM
"#,
        DiagnosticCode::WrongArgumentCount,
    );
}

#[test]
fn test_void_program_method_result_is_not_a_value() {
    check_has_error(
        r#"
PROGRAM Main
VAR
    flag : BOOL;
END_VAR
    flag := Touch();
METHOD Touch
    ;
END_METHOD
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn test_unknown_bare_call_stays_undefined() {
    check_has_error(
        r#"
PROGRAM Main
    Missing();
END_PROGRAM
"#,
        DiagnosticCode::UndefinedFunction,
    );
}

#[test]
// Unlike actions (which live outside the owning scope precisely so a
// same-spelling variable can shadow them), methods share the owner's
// declaration namespace: a variable and a method of the same name are a
// duplicate declaration.
fn test_program_method_name_conflicts_with_variable() {
    check_has_error(
        r#"
PROGRAM Main
VAR
    Reset : BOOL;
END_VAR
    Reset := TRUE;
METHOD PUBLIC Reset
    ;
END_METHOD
END_PROGRAM
"#,
        DiagnosticCode::DuplicateDeclaration,
    );
}

#[test]
fn test_function_block_method_bare_call_resolves_within_owner() {
    check_no_errors(
        r#"
FUNCTION_BLOCK Machine
VAR
    step : INT;
END_VAR
    Bump();
METHOD Bump
    step := step + 1;
END_METHOD
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    machine : Machine;
END_VAR
    machine();
END_PROGRAM
"#,
    );
}
