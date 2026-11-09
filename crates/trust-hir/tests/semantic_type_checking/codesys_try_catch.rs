//! Specification 06 §15: CODESYS's `__TRY`, under every profile, with the system type
//! `__SYSTEM.ExceptionCode`.
use crate::common::*;

/// A complete `__TRY` with a catch variable of `__SYSTEM.ExceptionCode` and a comparison with one of
/// its values checks clean under the IEC profile as under CODESYS's. Wrong behaviour caught: the type
/// or its value unknown, or the construct refused outside the CODESYS profile.
#[test]
fn a_try_with_an_exception_code_variable_checks_under_every_profile() {
    let source = "PROGRAM P\nVAR x : INT; e : __SYSTEM.ExceptionCode; divided : BOOL; END_VAR\n\
                  __TRY\n    x := 10 / x;\n__CATCH(e)\n    divided := e = __SYSTEM.ExceptionCode.RTSEXCPT_DIVIDEBYZERO;\n\
                  __FINALLY\n    x := x + 1;\n__ENDTRY;\nEND_PROGRAM";
    check_no_errors_in(Dialect::Iec, source);
    check_no_errors_in(Dialect::Codesys, source);
}

/// The catch variable must be a variable of `__SYSTEM.ExceptionCode` (E203), and a type statement of
/// the try statements is checked as any other. Wrong behaviour caught: a catch variable of another type,
/// which the build would fill with a code it cannot hold, and statements inside the construct that
/// escape the type check.
#[test]
fn a_catch_variable_of_another_type_and_a_wrong_statement_inside_are_refused() {
    let catch_dint =
        "PROGRAM P\nVAR e : DINT; END_VAR\n__TRY ; __CATCH(e) ; __ENDTRY;\nEND_PROGRAM";
    assert_eq!(check_errors(catch_dint), vec![DiagnosticCode::TypeMismatch]);
    let wrong_inside = "PROGRAM P\nVAR x : INT; s : STRING; END_VAR\n__TRY\n    x := s;\n__FINALLY ;\n__ENDTRY;\nEND_PROGRAM";
    assert!(
        !check_errors(wrong_inside).is_empty(),
        "the assignment inside is checked"
    );
}

/// `RETURN`, and an `EXIT` of a loop around the construct, may not leave the `__FINALLY` statements; an
/// `EXIT` of a loop inside them may; a `JMP` and a label inside the construct are refused. Wrong
/// behaviour caught: a jump out of `__FINALLY`, which would skip the rest of it.
#[test]
fn leaving_finally_and_jumping_inside_try_are_refused() {
    let return_in_finally =
        "FUNCTION F : INT\nF := 0;\n__TRY ; __FINALLY RETURN; __ENDTRY;\nEND_FUNCTION";
    assert_eq!(
        check_errors(return_in_finally),
        vec![DiagnosticCode::InvalidOperation]
    );
    let exit_outer = "PROGRAM P\nVAR i : INT; END_VAR\nFOR i := 1 TO 3 DO\n__TRY ; __FINALLY EXIT; __ENDTRY;\nEND_FOR;\nEND_PROGRAM";
    assert_eq!(
        check_errors(exit_outer),
        vec![DiagnosticCode::InvalidOperation]
    );
    let exit_inner = "PROGRAM P\nVAR i : INT; END_VAR\n__TRY ; __FINALLY FOR i := 1 TO 3 DO EXIT; END_FOR; __ENDTRY;\nEND_PROGRAM";
    check_no_errors(exit_inner);
    let exit_from_try = "PROGRAM P\nVAR i : INT; END_VAR\nFOR i := 1 TO 3 DO\n__TRY EXIT; __FINALLY ; __ENDTRY;\nEND_FOR;\nEND_PROGRAM";
    check_no_errors(exit_from_try);
    let jump = "PROGRAM P\n__TRY JMP L; L: ; __FINALLY ; __ENDTRY;\nEND_PROGRAM";
    assert_eq!(
        check_errors(jump),
        vec![
            DiagnosticCode::InvalidOperation,
            DiagnosticCode::InvalidOperation
        ]
    );
}

/// Another `__SYSTEM` type is unknown (E-code of an undefined type).
#[test]
fn an_unknown_system_type_is_refused() {
    let source = "PROGRAM P\nVAR e : __SYSTEM.Nothing; END_VAR\nEND_PROGRAM";
    assert!(check_errors(source).contains(&DiagnosticCode::UndefinedType));
}
