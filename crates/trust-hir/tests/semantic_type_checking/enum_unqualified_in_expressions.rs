//! Regression tests for unqualified enum variant references outside `CASE`
//! labels.
//!
//! The v0.18.4 hotfix (commit `8d7f069`) accepts unqualified enum variant
//! names as `CASE` labels. The tests below pin the analogous behavior for
//! three other contexts in which the same identifier should resolve to the
//! same runtime variant value as the qualified `EnumType#Variant` form:
//!
//! 1. `VAR` initializer (`state : Phase := IDLE`)
//! 2. Right-hand side of assignment (`state := RUNNING`)
//! 3. Operand of a binary comparison (`state = RUNNING`)
//!
//! HIR type-check already accepts all three forms (mirrors
//! `test_case_enum_label_ok`). These tests exist as a stable anchor so the
//! runtime/bytecode-layer fix that follows can rely on HIR-level contracts
//! not regressing.

use crate::common::*;

#[test]
fn test_unqualified_enum_variant_in_var_initializer_type_checks() {
    check_no_errors(
        r#"
TYPE Phase : (IDLE, RUNNING, DONE)
END_TYPE

PROGRAM Test
    VAR
        state : Phase := IDLE;
    END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn test_unqualified_enum_variant_in_assignment_rvalue_type_checks() {
    check_no_errors(
        r#"
TYPE Phase : (IDLE, RUNNING, DONE)
END_TYPE

PROGRAM Test
    VAR
        state : Phase;
    END_VAR
    state := RUNNING;
END_PROGRAM
"#,
    );
}

#[test]
fn test_unqualified_enum_variant_in_binary_comparison_type_checks() {
    check_no_errors(
        r#"
TYPE Phase : (IDLE, RUNNING, DONE)
END_TYPE

PROGRAM Test
    VAR
        state : Phase;
        flag : BOOL;
    END_VAR
    flag := state = RUNNING;
END_PROGRAM
"#,
    );
}

#[test]
fn test_ambiguous_unqualified_enum_variant_in_constant_initializer_is_rejected() {
    check_has_error(
        r#"
TYPE
NAMESPACE Paint
TYPE PaintColor : (RED, BLUE)
END_TYPE
END_NAMESPACE

NAMESPACE Alarm
TYPE AlarmColor : (RED, GREEN)
END_TYPE
END_NAMESPACE

PROGRAM Test
    VAR CONSTANT
        Selected : INT := RED;
    END_VAR
END_PROGRAM
"#,
        DiagnosticCode::CannotResolve,
    );
}
