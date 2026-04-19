//! Runtime-level regression tests for unqualified enum variant references
//! outside `CASE` labels.
//!
//! The v0.18.4 hotfix (commit `8d7f069`) accepts unqualified enum variants
//! as `CASE` labels (see `bytecode_vm_differential::
//! register_and_stack_paths_match_for_unqualified_enum_case_labels`). These
//! tests pin the analogous runtime behavior for three other contexts where
//! the same identifier should resolve to the declared variant value:
//!
//! 1. `VAR` initializer: `state : Phase := IDLE`
//! 2. RHS of assignment: `state := RUNNING`
//! 3. Operand of binary comparison: `state = RUNNING`
//!
//! On v0.18.4 all three silently misbehave:
//! - (1) fails PROGRAM init with `undefined variable 'IDLE'`
//! - (2) compiles but leaves the target at its previous value (no-op)
//! - (3) compiles but the comparison never matches
//!
//! Tests are `#[ignore]`d with a `FIXME` link to the accompanying fix
//! commit so CI stays green until the lowering/runtime fix lands.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

fn enum_variant_name(value: &Option<Value>) -> Option<&str> {
    match value {
        Some(Value::Enum(e)) => Some(e.variant_name.as_str()),
        _ => None,
    }
}

fn enum_numeric(value: &Option<Value>) -> Option<i64> {
    match value {
        Some(Value::Enum(e)) => Some(e.numeric_value),
        _ => None,
    }
}

#[test]
fn unqualified_enum_variant_initializes_var_to_declared_variant() {
    let source = r#"
TYPE Phase : (IDLE, RUNNING, DONE)
END_TYPE

PROGRAM Main
VAR
    state : Phase := IDLE;
END_VAR
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile harness");
    let _ = harness.cycle();

    let got = harness.get_output("state");
    assert_eq!(enum_variant_name(&got), Some("IDLE"), "got {got:?}");
    assert_eq!(enum_numeric(&got), Some(0), "got {got:?}");
}

#[test]
fn unqualified_enum_variant_rvalue_assigns_expected_variant() {
    let source = r#"
TYPE Phase : (IDLE, RUNNING, DONE)
END_TYPE

PROGRAM Main
VAR
    state : Phase := Phase#IDLE;
END_VAR
state := RUNNING;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile harness");
    let _ = harness.cycle();

    let got = harness.get_output("state");
    assert_eq!(enum_variant_name(&got), Some("RUNNING"), "got {got:?}");
    assert_eq!(enum_numeric(&got), Some(1), "got {got:?}");
}

#[test]
#[ignore = "FIXME(enum-unqualified): binary comparison with unqualified \
enum variant silently compiles but never matches at runtime. Unignore \
once the lowering fix for non-CASE-label contexts lands."]
fn unqualified_enum_variant_comparison_matches_when_values_equal() {
    let source = r#"
TYPE Phase : (IDLE, RUNNING, DONE)
END_TYPE

PROGRAM Main
VAR
    state : Phase := Phase#RUNNING;
    flag : DINT := 0;
END_VAR
IF state = RUNNING THEN
    flag := 1;
END_IF;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile harness");
    let _ = harness.cycle();

    assert_eq!(harness.get_output("flag"), Some(Value::DInt(1)));
}
