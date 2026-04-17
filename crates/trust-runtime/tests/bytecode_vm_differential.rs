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
