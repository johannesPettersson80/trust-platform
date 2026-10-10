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
