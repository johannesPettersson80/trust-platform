use crate::common::*;

// WAGO s4 measured (2026-10-19, re-probed 2026-10-28 on the PR #11 line):
// METHOD parameters declared in a `VAR_IN_OUT` section are by-reference
// slots — writes through them are the parameter's purpose and must not
// fire the invalid-assignment-target diagnostic, while the same write to
// a `VAR_INPUT` method parameter stays rejected.

#[test]
fn method_in_out_parameter_write_is_accepted() {
    check_no_errors(
        r#"
FUNCTION_BLOCK FB_Probe
    VAR
        xState : BOOL;
    END_VAR

    METHOD PUBLIC mProbe : BOOL
        VAR_INPUT
            iTarget : INT;
        END_VAR
        VAR_IN_OUT
            xPresent : BOOL;
        END_VAR
        mProbe := TRUE;
        xPresent := TRUE;
    END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
    VAR
        fbProbe : FB_Probe;
        xFlag : BOOL;
    END_VAR
    fbProbe.mProbe(iTarget := INT#1, xPresent := xFlag);
END_PROGRAM
"#,
    );
}

#[test]
fn method_input_parameter_write_stays_rejected() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
FUNCTION_BLOCK FB_Probe
    VAR
        xState : BOOL;
    END_VAR

    METHOD PUBLIC mProbe : BOOL
        VAR_INPUT
            iTarget : INT;
            xPresent : BOOL;
        END_VAR
        mProbe := TRUE;
        xPresent := TRUE;
    END_METHOD
END_FUNCTION_BLOCK
"#
        .to_string(),
    );

    assert!(db.diagnostics(file).iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::InvalidAssignmentTarget
            && diagnostic
                .message
                .contains("cannot assign to input parameter 'xPresent'")
    }));
}
