mod common;
use common::*;

// Program-singleton access under the CODESYS profile (IEC_DEVIATIONS DEV-030): a PROGRAM name
// qualifies its own members from other POUs, and calling it runs its body. IEC 61131-3 Ed.3
// §6.6.4 instantiates programs only in resources, so the IEC profile rejects both.

#[test]
fn test_program_singleton_member_read() {
    check_no_errors_in(
        Dialect::Codesys,
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
    check_no_errors_in(
        Dialect::Codesys,
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
    check_no_errors_in(
        Dialect::Codesys,
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
    check_no_errors_in(
        Dialect::Codesys,
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
    check_no_errors_in(
        Dialect::Codesys,
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
    check_no_errors_in(
        Dialect::Codesys,
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
    check_has_error_in(
        Dialect::Codesys,
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
    check_has_error_in(
        Dialect::Codesys,
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
    check_has_error_in(
        Dialect::Codesys,
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
    check_no_errors_in(
        Dialect::Codesys,
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
    check_no_errors_in(
        Dialect::Codesys,
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

/// Under the IEC profile a program's name neither reads another program's variable nor runs its
/// body (IEC 61131-3 Ed.3 §6.6.4 items 2 and 3). Catches the CODESYS profile's program access
/// accepted in an IEC project.
#[test]
fn test_program_member_access_and_invocation_are_iec_errors() {
    let read = r#"
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
"#;
    let call = r#"
PROGRAM Worker
END_PROGRAM
PROGRAM Main
    Worker();
END_PROGRAM
"#;
    for source in [read, call] {
        assert!(!check_errors(source).is_empty(), "IEC rejects {source}");
        check_no_errors_in(Dialect::Codesys, source);
    }
}
