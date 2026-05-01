use trust_runtime::error::RuntimeError;
use trust_runtime::execution_backend::{ExecutionBackend, VmRegisterProfileSnapshot};
use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

fn vm_harness(source: &str, force_stack_via_debug: bool) -> TestHarness {
    let mut harness = TestHarness::from_source(source).expect("compile harness");
    let runtime = harness.runtime_mut();
    runtime
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("select vm backend");
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();
    if force_stack_via_debug {
        let _ = runtime.enable_debug();
    }
    harness
}

fn assert_register_path(profile: &VmRegisterProfileSnapshot) {
    assert!(
        profile.register_programs_executed >= 1,
        "expected register execution, got profile {profile:?}"
    );
    assert_eq!(
        profile.register_program_fallbacks, 0,
        "unexpected register fallback profile {profile:?}"
    );
}

fn assert_stack_fallback(profile: &VmRegisterProfileSnapshot) {
    assert_eq!(
        profile.register_programs_executed, 0,
        "expected stack-only execution, got profile {profile:?}"
    );
    assert!(
        profile.register_program_fallbacks >= 1,
        "expected at least one stack fallback, got profile {profile:?}"
    );
    assert!(
        profile
            .fallback_reasons
            .iter()
            .any(|reason| reason.reason == "debug_mode"),
        "expected debug_mode fallback reason, got profile {profile:?}"
    );
}

fn assert_register_started_without_fallback(profile: &VmRegisterProfileSnapshot) {
    assert_eq!(
        profile.register_program_fallbacks, 0,
        "unexpected register fallback profile {profile:?}"
    );
    assert!(
        !profile.hot_blocks.is_empty(),
        "expected register block activity before trap, got profile {profile:?}"
    );
}

#[test]
fn register_and_stack_paths_match_for_composite_value_program() {
    let source = r#"
FUNCTION_BLOCK Bump
VAR_INPUT
    IN : DINT;
END_VAR
VAR_OUTPUT
    OUT : DINT;
END_VAR
OUT := IN + 1;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    grid : ARRAY[0..1, 0..1] OF DINT;
    total : DINT := 0;
    second : CHAR := ' ';
    text : STRING[8] := 'ABC';
    fb : Bump;
END_VAR
grid[0,0] := 1;
grid[0,1] := 2;
grid[1,0] := 3;
grid[1,1] := 4;
fb(IN := grid[0,1] + grid[1,0]);
total := fb.OUT;
second := text[2];
END_PROGRAM
"#;

    let mut register = vm_harness(source, false);
    let mut stack = vm_harness(source, true);

    let register_cycle = register.cycle();
    let stack_cycle = stack.cycle();

    assert_eq!(register_cycle.errors, stack_cycle.errors);
    assert_eq!(register.get_output("total"), stack.get_output("total"));
    assert_eq!(register.get_output("second"), stack.get_output("second"));
    assert_eq!(register.get_output("total"), Some(Value::DInt(6)));
    assert_eq!(register.get_output("second"), Some(Value::Char(b'B')));

    assert_register_path(&register.runtime().vm_register_profile_snapshot());
    assert_stack_fallback(&stack.runtime().vm_register_profile_snapshot());
}

#[test]
fn register_and_stack_paths_match_for_deep_ref_chain_field_index_parity() {
    let source = r#"
TYPE Cell :
STRUCT
    value : DINT;
END_STRUCT
END_TYPE

TYPE Row :
STRUCT
    cells : ARRAY[0..1] OF Cell;
END_STRUCT
END_TYPE

TYPE Matrix :
STRUCT
    rows : ARRAY[0..1] OF Row;
END_STRUCT
END_TYPE

PROGRAM Main
VAR
    matrix : Matrix;
    row_idx : DINT := 1;
    cell_idx : DINT := 0;
    outv : DINT := 0;
END_VAR
matrix.rows[1].cells[0].value := 41;
matrix.rows[row_idx].cells[cell_idx].value := matrix.rows[row_idx].cells[cell_idx].value + 1;
outv := matrix.rows[1].cells[0].value;
END_PROGRAM
"#;

    let mut register = vm_harness(source, false);
    let mut stack = vm_harness(source, true);

    let register_cycle = register.cycle();
    let stack_cycle = stack.cycle();

    assert_eq!(register_cycle.errors, stack_cycle.errors);
    assert!(
        register_cycle.errors.is_empty(),
        "register errors: {:?}",
        register_cycle.errors
    );
    assert_eq!(register.get_output("outv"), stack.get_output("outv"));
    assert_eq!(register.get_output("outv"), Some(Value::DInt(42)));

    assert_register_path(&register.runtime().vm_register_profile_snapshot());
    assert_stack_fallback(&stack.runtime().vm_register_profile_snapshot());
}

#[test]
fn register_and_stack_paths_surface_same_deep_ref_chain_index_trap() {
    let source = r#"
TYPE Cell :
STRUCT
    value : DINT;
END_STRUCT
END_TYPE

TYPE Row :
STRUCT
    cells : ARRAY[0..1] OF Cell;
END_STRUCT
END_TYPE

TYPE Matrix :
STRUCT
    rows : ARRAY[0..1] OF Row;
END_STRUCT
END_TYPE

PROGRAM Main
VAR
    matrix : Matrix;
    row_idx : DINT := 1;
    cell_idx : DINT := 2;
    outv : DINT := 0;
END_VAR
outv := matrix.rows[row_idx].cells[cell_idx].value;
END_PROGRAM
"#;

    let mut register = vm_harness(source, false);
    let mut stack = vm_harness(source, true);

    let register_cycle = register.cycle();
    let stack_cycle = stack.cycle();

    assert_eq!(register_cycle.errors, stack_cycle.errors);
    assert_eq!(
        register_cycle.errors,
        vec![RuntimeError::IndexOutOfBounds {
            index: 2,
            lower: 0,
            upper: 1,
        }]
    );

    assert_register_started_without_fallback(&register.runtime().vm_register_profile_snapshot());
    assert_stack_fallback(&stack.runtime().vm_register_profile_snapshot());
}

#[test]
fn register_and_stack_paths_match_for_string_wstring_edge_indices() {
    let source = r#"
PROGRAM Main
VAR
    text_value : STRING[8] := 'ÄBC';
    wide_value : WSTRING[8] := "ÅZ";
    text_first : CHAR;
    text_last : CHAR;
    wide_first : WCHAR;
    wide_last : WCHAR;
END_VAR
text_first := text_value[1];
text_last := text_value[3];
wide_first := wide_value[1];
wide_last := wide_value[2];
END_PROGRAM
"#;

    let mut register = vm_harness(source, false);
    let mut stack = vm_harness(source, true);

    let register_cycle = register.cycle();
    let stack_cycle = stack.cycle();

    assert_eq!(register_cycle.errors, stack_cycle.errors);
    assert!(
        register_cycle.errors.is_empty(),
        "register errors: {:?}",
        register_cycle.errors
    );
    assert_eq!(
        register.get_output("text_first"),
        stack.get_output("text_first")
    );
    assert_eq!(
        register.get_output("text_last"),
        stack.get_output("text_last")
    );
    assert_eq!(
        register.get_output("wide_first"),
        stack.get_output("wide_first")
    );
    assert_eq!(
        register.get_output("wide_last"),
        stack.get_output("wide_last")
    );
    assert_eq!(register.get_output("text_first"), Some(Value::Char(0xC4)));
    assert_eq!(register.get_output("text_last"), Some(Value::Char(b'C')));
    assert_eq!(
        register.get_output("wide_first"),
        Some(Value::WChar(0x00C5))
    );
    assert_eq!(
        register.get_output("wide_last"),
        Some(Value::WChar(b'Z' as u16))
    );

    assert_register_path(&register.runtime().vm_register_profile_snapshot());
    assert_stack_fallback(&stack.runtime().vm_register_profile_snapshot());
}

#[test]
fn register_and_stack_paths_surface_same_string_wstring_index_traps() {
    let cases = [
        (
            r#"
PROGRAM Main
VAR
    text_value : STRING[8] := 'ABC';
    idx : DINT := 0;
    out_char : CHAR;
END_VAR
out_char := text_value[idx];
END_PROGRAM
"#,
            RuntimeError::IndexOutOfBounds {
                index: 0,
                lower: 1,
                upper: i64::MAX,
            },
        ),
        (
            r#"
PROGRAM Main
VAR
    wide_value : WSTRING[8] := "AZ";
    idx : DINT := 3;
    out_wchar : WCHAR;
END_VAR
out_wchar := wide_value[idx];
END_PROGRAM
"#,
            RuntimeError::IndexOutOfBounds {
                index: 3,
                lower: 1,
                upper: 2,
            },
        ),
    ];

    for (source, expected) in cases {
        let mut register = vm_harness(source, false);
        let mut stack = vm_harness(source, true);

        let register_cycle = register.cycle();
        let stack_cycle = stack.cycle();

        assert_eq!(register_cycle.errors, stack_cycle.errors);
        assert_eq!(register_cycle.errors, vec![expected]);

        assert_register_started_without_fallback(
            &register.runtime().vm_register_profile_snapshot(),
        );
        assert_stack_fallback(&stack.runtime().vm_register_profile_snapshot());
    }
}

#[test]
fn register_and_stack_paths_surface_same_modulo_by_zero_error() {
    let source = r#"
PROGRAM Main
VAR
    left : DINT := 10;
    right : DINT := 0;
    outv : DINT := 0;
END_VAR
outv := left MOD right;
END_PROGRAM
"#;

    let mut register = vm_harness(source, false);
    let mut stack = vm_harness(source, true);

    let register_cycle = register.cycle();
    let stack_cycle = stack.cycle();

    assert_eq!(register_cycle.errors, stack_cycle.errors);
    assert_eq!(
        register_cycle.errors,
        vec![trust_runtime::error::RuntimeError::ModuloByZero]
    );

    assert_register_started_without_fallback(&register.runtime().vm_register_profile_snapshot());
    assert_stack_fallback(&stack.runtime().vm_register_profile_snapshot());
}

#[test]
fn register_and_stack_paths_match_for_case_insensitive_oscat_style_calls() {
    let source = r#"
TYPE CONSTANTS_PHYS :
STRUCT
    T0 : REAL := -273.15;
END_STRUCT
END_TYPE

VAR_GLOBAL
    PHYS : CONSTANTS_PHYS;
END_VAR

FUNCTION EXP10 : REAL
VAR_INPUT
    X : REAL;
END_VAR
EXP10 := EXP(X * 2.30258509299405);
END_FUNCTION

FUNCTION DEW_TEMP : REAL
VAR_INPUT
    RH : REAL;
    T : REAL;
END_VAR
VAR CONSTANT
    a : REAL := 7.5;
    b : REAL := 237.3;
END_VAR
VAR
    V : REAL;
    SaturationTerm : REAL;
END_VAR
IF rh > 0.0 THEN
    SaturationTerm := EXP10((a * T) / (b + T));
    V := LOG(RH * 0.01 * SaturationTerm);
    DEW_TEMP := b * V / (a - V);
ELSE
    DEW_TEMP := phys.T0;
END_IF;
END_FUNCTION

PROGRAM Main
VAR
    outv : REAL := REAL#0.0;
END_VAR
outv := DEW_TEMP(RH := REAL#50.0, T := REAL#20.0);
END_PROGRAM
"#;

    let mut register = vm_harness(source, false);
    let mut stack = vm_harness(source, true);

    let register_cycle = register.cycle();
    let stack_cycle = stack.cycle();

    assert_eq!(register_cycle.errors, stack_cycle.errors);
    assert!(
        register_cycle.errors.is_empty(),
        "register errors: {:?}",
        register_cycle.errors
    );
    assert_eq!(register.get_output("outv"), stack.get_output("outv"));

    assert_register_path(&register.runtime().vm_register_profile_snapshot());
    assert_stack_fallback(&stack.runtime().vm_register_profile_snapshot());
}

#[test]
fn register_and_stack_paths_match_for_unqualified_enum_case_labels() {
    let source = r#"
TYPE Axis : (X, Z, G)
END_TYPE

PROGRAM Main
VAR
    axis : Axis := Axis#Z;
    outv : DINT := 0;
END_VAR
CASE axis OF
    X: outv := 1;
    Z: outv := 2;
    G: outv := 3;
END_CASE;
END_PROGRAM
"#;

    let mut register = vm_harness(source, false);
    let mut stack = vm_harness(source, true);

    let register_cycle = register.cycle();
    let stack_cycle = stack.cycle();

    assert_eq!(register_cycle.errors, stack_cycle.errors);
    assert!(
        register_cycle.errors.is_empty(),
        "register errors: {:?}",
        register_cycle.errors
    );
    assert_eq!(register.get_output("outv"), stack.get_output("outv"));
    assert_eq!(register.get_output("outv"), Some(Value::DInt(2)));

    assert_register_path(&register.runtime().vm_register_profile_snapshot());
    assert_stack_fallback(&stack.runtime().vm_register_profile_snapshot());
}
