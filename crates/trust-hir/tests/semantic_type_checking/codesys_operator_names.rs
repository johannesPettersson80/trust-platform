//! Under the CODESYS profile a declared name that is the name of a CODESYS operator, in any case, is
//! warned about (W016, specification 28 §10): CODESYS takes its operator names as keywords, which
//! "cannot be used as variable names" (CODESYS help, "Keywords"), and the CODESYS 3.5.17 compiler
//! refuses an enumeration member `log` with "Identifier expected instead of 'log'" (contributor issue
//! #477). The IEC profile keeps these names, and the warning stays a warning.

use crate::common::*;

/// The type of contributor issue #477: its member `log` is the name of the operator LOG.
const ENTITY_SUB_TYPE: &str = r#"
{attribute 'qualified_only'}
{attribute 'strict'}
TYPE
  eEntitySubType : (door := 0, window := 1, motion := 2, log := 11, power := 12);
END_TYPE
"#;

/// How many W016 warnings `source` gets under `dialect`.
fn operator_names(dialect: Dialect, source: &str) -> usize {
    check_warnings_in(dialect, source)
        .iter()
        .filter(|code| **code == DiagnosticCode::CodesysOperatorName)
        .count()
}

/// The member `log` is warned about once under the CODESYS profile and not under the IEC one; the
/// type has no error under either. Catches a check that missed enumeration members, which CODESYS
/// refuses at parse time, one that reached the IEC profile, and one that refused the name.
#[test]
fn an_enumeration_member_named_like_a_codesys_operator_is_warned_about() {
    assert_eq!(operator_names(Dialect::Codesys, ENTITY_SUB_TYPE), 1);
    assert_eq!(operator_names(Dialect::Iec, ENTITY_SUB_TYPE), 0);
    assert_eq!(check_errors_in(Dialect::Codesys, ENTITY_SUB_TYPE), vec![]);
}

/// A structure member `abs`, an input `Exp`, a variable `Max` and a constant `LIMIT` are each warned
/// about under the CODESYS profile, whatever their case: 4 warnings. `logLevel`, `maxValue` and
/// `Limits` only contain an operator's name and are not. Catches a check that compared the case, one
/// limited to enumeration members, and one that matched part of a name.
#[test]
fn every_declared_name_of_a_codesys_operator_is_warned_about_and_only_those() {
    let source = r#"
TYPE Reading :
STRUCT
    abs : REAL;
    logLevel : INT;
END_STRUCT
END_TYPE

FUNCTION_BLOCK Scaler
VAR_INPUT
    Exp : REAL;
    maxValue : REAL;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Max : INT;
    Limits : INT;
END_VAR
VAR CONSTANT
    LIMIT : INT := 1;
END_VAR
    Max := LIMIT + Limits;
END_PROGRAM
"#;
    assert_eq!(operator_names(Dialect::Codesys, source), 4);
    assert_eq!(operator_names(Dialect::Iec, source), 0);
    assert_eq!(check_errors_in(Dialect::Codesys, source), vec![]);
}
