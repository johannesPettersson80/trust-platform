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
use trust_runtime::value::{EnumValue, Value};

fn enum_variant_name(value: &Option<Value>) -> Option<&str> {
    match value {
        Some(Value::Enum(e)) => Some(e.variant_name().as_str()),
        _ => None,
    }
}

fn enum_numeric(value: &Option<Value>) -> Option<i64> {
    match value {
        Some(Value::Enum(e)) => Some(e.numeric_value()),
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
fn unqualified_enum_initializer_respects_same_named_constant_shadowing() {
    let source = r#"
TYPE Phase : (IDLE, RUNNING, DONE)
END_TYPE

PROGRAM Main
VAR CONSTANT
    IDLE : Phase := Phase#DONE;
END_VAR
VAR
    state : Phase := IDLE;
END_VAR
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile harness");
    let result = harness.cycle();
    assert!(
        result.errors.is_empty(),
        "unexpected runtime errors: {:?}",
        result.errors
    );

    let got = harness.get_output("state");
    assert_eq!(enum_variant_name(&got), Some("DONE"), "got {got:?}");
    assert_eq!(enum_numeric(&got), Some(2), "got {got:?}");
}

#[test]
fn unqualified_enum_assignment_respects_same_named_local_shadowing() {
    let source = r#"
TYPE Phase : (IDLE, RUNNING, DONE)
END_TYPE

PROGRAM Main
VAR
    state : Phase := Phase#IDLE;
    RUNNING : Phase := Phase#DONE;
END_VAR
state := RUNNING;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile harness");
    let _ = harness.cycle();

    let got = harness.get_output("state");
    assert_eq!(enum_variant_name(&got), Some("DONE"), "got {got:?}");
    assert_eq!(enum_numeric(&got), Some(2), "got {got:?}");
}

#[test]
fn unqualified_enum_comparison_respects_same_named_local_shadowing() {
    let source = r#"
TYPE Phase : (IDLE, RUNNING, DONE)
END_TYPE

PROGRAM Main
VAR
    state : Phase := Phase#DONE;
    RUNNING : Phase := Phase#DONE;
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

#[test]
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

#[test]
fn var_initialized_enum_compares_equal_to_its_declared_variant() {
    let source = r#"
TYPE Solo : (S0, S1, S2)
END_TYPE

PROGRAM Main
VAR
    val : Solo := S1;
    cmp_unqualified : BOOL := FALSE;
    cmp_qualified : BOOL := FALSE;
    cmp_literal : BOOL := FALSE;
    cnt : DINT := 0;
END_VAR
cmp_unqualified := val = S1;
cmp_qualified := val = Solo#S1;
cmp_literal := Solo#S1 = Solo#S1;
IF S1 = val THEN
    cnt := 1;
END_IF;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile harness");
    let _ = harness.cycle();

    assert_eq!(
        harness.get_output("val"),
        Some(Value::Enum(Box::new(
            EnumValue::from_serialized_parts(harness.runtime().registry(), "Solo", "S1", 1)
                .expect("Solo#S1 enum value")
        )))
    );
    assert_eq!(
        harness.get_output("cmp_unqualified"),
        Some(Value::Bool(true))
    );
    assert_eq!(harness.get_output("cmp_qualified"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("cmp_literal"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("cnt"), Some(Value::DInt(1)));
}

#[test]
fn enum_alias_and_mixed_case_literals_share_canonical_identity() {
    let source = r#"
TYPE
    Solo : (S0, S1, S2);
    AliasSolo : Solo;
END_TYPE

PROGRAM Main
VAR
    base : Solo := S1;
    alias : AliasSolo := s1;
    cmp_alias : BOOL := FALSE;
    cmp_literal : BOOL := FALSE;
    cmp_left_literal : BOOL := FALSE;
END_VAR
cmp_alias := base = alias;
cmp_literal := alias = AliasSolo#S1;
cmp_left_literal := solo#s1 = alias;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile harness");
    let result = harness.cycle();
    assert!(
        result.errors.is_empty(),
        "unexpected runtime errors: {:?}",
        result.errors
    );

    assert_eq!(harness.get_output("cmp_alias"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("cmp_literal"), Some(Value::Bool(true)));
    assert_eq!(
        harness.get_output("cmp_left_literal"),
        Some(Value::Bool(true))
    );
}

#[test]
fn unqualified_enum_variant_case_label_with_indexed_selector() {
    let source = r#"
TYPE Phase : (IDLE, RUNNING)
END_TYPE

PROGRAM Main
VAR
    arr : ARRAY[1..2] OF Phase;
    hit : DINT := 0;
END_VAR
arr[1] := RUNNING;
CASE arr[1] OF
    IDLE:    hit := 10;
    RUNNING: hit := 20;
END_CASE;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile harness");
    let _ = harness.cycle();

    assert_eq!(harness.get_output("hit"), Some(Value::DInt(20)));
}

#[test]
fn unqualified_enum_variant_case_label_with_field_selector() {
    let source = r#"
TYPE Phase : (IDLE, RUNNING)
END_TYPE
TYPE Box : STRUCT p : Phase; END_STRUCT
END_TYPE

PROGRAM Main
VAR
    c : Box;
    hit : DINT := 0;
END_VAR
c.p := RUNNING;
CASE c.p OF
    IDLE:    hit := 10;
    RUNNING: hit := 20;
END_CASE;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile harness");
    let _ = harness.cycle();

    assert_eq!(harness.get_output("hit"), Some(Value::DInt(20)));
}

#[test]
fn unqualified_enum_variant_case_label_with_indexed_field_selector() {
    let source = r#"
TYPE Phase : (IDLE, RUNNING)
END_TYPE
TYPE Box : STRUCT p : Phase; END_STRUCT
END_TYPE

PROGRAM Main
VAR
    arr : ARRAY[1..2] OF Box;
    hit : DINT := 0;
END_VAR
arr[1].p := RUNNING;
CASE arr[1].p OF
    IDLE:    hit := 10;
    RUNNING: hit := 20;
END_CASE;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile harness");
    let _ = harness.cycle();

    assert_eq!(harness.get_output("hit"), Some(Value::DInt(20)));
}
