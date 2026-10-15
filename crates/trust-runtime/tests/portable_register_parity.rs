//! Actual current STBC construction/execution parity between prepared register plans
//! and the shared stack path used while a debugger is active.
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::{DateTimeValue, Duration, Value};
use trust_runtime_core::vm::register::RegisterExecutionConfiguration;
use trust_runtime_core::vm::{ExecutionServices, PreparationLimits, PreparedModule, RuntimeState};

struct Services {
    stack: bool,
}
impl ExecutionServices for Services {
    fn debug_active(&self) -> bool {
        self.stack
    }
    fn deadline_exceeded(&self) -> bool {
        false
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
}

#[test]
fn source_free_string_element_reads_and_comparisons_match_shared_stack() {
    let source = r#"
PROGRAM Main
VAR
    narrow : STRING[8] := 'AB';
    wide : WSTRING[8] := "αβ";
    narrow_char : CHAR;
    wide_char : WCHAR;
    equal : BOOL;
    cycles : DINT;
END_VAR
narrow_char := narrow[2];
wide_char := wide[2];
equal := (narrow_char = narrow[2]) AND (wide_char = wide[2]);
cycles := cycles + 1;
END_PROGRAM
"#;
    let bytes = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted source")
        .encode()
        .expect("serialize supplied application");
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default())
        .expect("admitted saved STBC");
    let register_services = Services { stack: false };
    let stack_services = Services { stack: true };
    let mut register = prepared
        .instantiate_with_services(0, &register_services)
        .unwrap();
    let mut stack = prepared
        .instantiate_with_services(0, &stack_services)
        .unwrap();
    register.set_register_profile_enabled(true);
    register.set_tier1_enabled(true);
    stack.set_register_profile_enabled(true);
    let preparation = register.tier1_snapshot();
    for cycle in 1..=80 {
        let time = Duration::from_millis(cycle * 10);
        register.execute_cycle(time).unwrap();
        stack.execute_cycle(time).unwrap();
        let Some(Value::Instance(register_root)) = register.storage().get_global("Main") else {
            panic!("program root")
        };
        let Some(Value::Instance(stack_root)) = stack.storage().get_global("Main") else {
            panic!("program root")
        };
        for (name, expected) in [
            ("narrow_char", Value::Char(b'B')),
            ("wide_char", Value::WChar(0x03b2)),
            ("equal", Value::Bool(true)),
            ("cycles", Value::DInt(cycle as i32)),
        ] {
            assert_eq!(
                register.storage().get_instance_var(*register_root, name),
                Some(&expected)
            );
            assert_eq!(
                stack.storage().get_instance_var(*stack_root, name),
                Some(&expected)
            );
        }
        assert_eq!(register.outputs(), stack.outputs());
    }
    let optimized = register.register_profile_snapshot();
    assert!(
        optimized.register_programs_executed > 0,
        "actual register body execution: {optimized:?}"
    );
    assert!(
        optimized.fallback_reasons.is_empty(),
        "tested program must be eligible: {optimized:?}"
    );
    let debug = stack.register_profile_snapshot();
    assert_eq!(debug.register_programs_executed, 0);
    assert!(debug
        .fallback_reasons
        .iter()
        .any(|reason| reason.reason == "debug_mode" && reason.count > 0));
    let running = register.tier1_snapshot();
    assert!(
        running.block_executions > 0,
        "precompiled tier selected after default warmup: {running:?}"
    );
    assert_eq!(
        running.compile_attempts, preparation.compile_attempts,
        "RUN never compiles"
    );
    assert_eq!(running.compile_successes, preparation.compile_successes);
}

#[test]
fn nested_source_free_functions_and_function_blocks_keep_prepared_register_execution() {
    let source = r#"
FUNCTION Increment : INT
VAR_INPUT input : INT; END_VAR
Increment := input + 1;
END_FUNCTION
FUNCTION_BLOCK Counter
VAR_INPUT delta : INT; END_VAR
VAR_OUTPUT total : INT; END_VAR
total := total + Increment(delta);
END_FUNCTION_BLOCK
PROGRAM Main
VAR counter : Counter; result : INT; cycles : INT; END_VAR
counter(delta := 2);
result := Increment(counter.total);
cycles := cycles + 1;
END_PROGRAM
"#;
    let bytes = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap()
        .encode()
        .unwrap();
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let register_services = Services { stack: false };
    let stack_services = Services { stack: true };
    let mut register = prepared
        .instantiate_with_services(0, &register_services)
        .unwrap();
    let mut stack = prepared
        .instantiate_with_services(0, &stack_services)
        .unwrap();
    register.set_register_profile_enabled(true);
    register.set_tier1_enabled(true);
    let preparation = register.tier1_snapshot();
    for cycle in 1..=80 {
        let time = Duration::from_millis(cycle * 10);
        register.execute_cycle(time).unwrap();
        stack.execute_cycle(time).unwrap();
        let Some(Value::Instance(root)) = register.storage().get_global("Main") else {
            panic!("program root")
        };
        let Some(Value::Instance(stack_root)) = stack.storage().get_global("Main") else {
            panic!("stack program root")
        };
        for (name, expected) in [
            ("result", Value::Int((cycle * 3 + 1) as i16)),
            ("cycles", Value::Int(cycle as i16)),
        ] {
            assert_eq!(
                register.storage().get_instance_var(*root, name),
                Some(&expected)
            );
            assert_eq!(
                stack.storage().get_instance_var(*stack_root, name),
                Some(&expected)
            );
        }
        assert_eq!(register.outputs(), stack.outputs());
    }
    let profile = register.register_profile_snapshot();
    assert_eq!(
        profile.register_programs_executed,
        80 * 4,
        "program, FB, and both function calls: {profile:?}"
    );
    assert!(
        profile.fallback_reasons.is_empty(),
        "eligible nested bodies stay optimized: {profile:?}"
    );
    let running = register.tier1_snapshot();
    assert!(running.block_executions > 0);
    assert_eq!(
        running.compile_attempts, preparation.compile_attempts,
        "RUN never compiles"
    );
    assert_eq!(running.compile_successes, preparation.compile_successes);
}

// Executor differential replay. Each comparison runs the same saved program and the same
// per-cycle inputs on the shared stack (`debug_active`), the register interpreter (Tier-1 off)
// and Tier-1, then compares variables, outputs, faults and task state on every cycle.

/// The program of `source_free_string_element_reads_and_comparisons_match_shared_stack`.
const STRING_ELEMENT_SOURCE: &str = r#"
PROGRAM Main
VAR
    narrow : STRING[8] := 'AB';
    wide : WSTRING[8] := "αβ";
    narrow_char : CHAR;
    wide_char : WCHAR;
    equal : BOOL;
    cycles : DINT;
END_VAR
narrow_char := narrow[2];
wide_char := wide[2];
equal := (narrow_char = narrow[2]) AND (wide_char = wide[2]);
cycles := cycles + 1;
END_PROGRAM
"#;

/// Global inputs written between cycles; `divisor` is zero once, at cycle 40.
const INPUT_SOURCE: &str = r#"
VAR_GLOBAL
    sample : DINT := 0;
    divisor : DINT := 1;
END_VAR
PROGRAM Main
VAR
    acc : DINT := 0;
    remainder : DINT := 0;
    cycles : DINT := 0;
END_VAR
acc := acc + sample;
remainder := DINT#100 MOD divisor;
cycles := cycles + 1;
END_PROGRAM
"#;

/// An IF with a toggle that alternates every cycle, and a CASE with an ELSE arm.
const BRANCH_SOURCE: &str = r#"
PROGRAM Main
VAR
    cycles : DINT := 0;
    toggle : BOOL := FALSE;
    selector : DINT := 0;
    then_count : DINT := 0;
    else_count : DINT := 0;
    arm_zero : DINT := 0;
    arm_one : DINT := 0;
    arm_else : DINT := 0;
END_VAR
toggle := NOT toggle;
IF toggle THEN
    then_count := then_count + 1;
ELSE
    else_count := else_count + 1;
END_IF;
selector := cycles MOD DINT#3;
CASE selector OF
    0: arm_zero := arm_zero + 1;
    1: arm_one := arm_one + 1;
ELSE
    arm_else := arm_else + 1;
END_CASE;
cycles := cycles + 1;
END_PROGRAM
"#;

/// One RETAIN and one non-retained counter, so restarts show both kinds of value.
const RESTART_SOURCE: &str = r#"
PROGRAM Main
VAR RETAIN
    retained : DINT := 0;
END_VAR
VAR
    plain : DINT := 0;
END_VAR
retained := retained + 1;
plain := plain + 1;
END_PROGRAM
"#;

/// Arm services with an explicit register configuration; `stack` forces the canonical path.
struct ConfiguredServices {
    stack: bool,
    configuration: RegisterExecutionConfiguration,
}

impl ExecutionServices for ConfiguredServices {
    fn register_configuration(&self) -> RegisterExecutionConfiguration {
        self.configuration
    }
    fn debug_active(&self) -> bool {
        self.stack
    }
    fn deadline_exceeded(&self) -> bool {
        false
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
}

/// Telemetry on with the default Tier-1 settings (disabled).
fn profiled() -> RegisterExecutionConfiguration {
    RegisterExecutionConfiguration {
        profile_enabled: true,
        ..RegisterExecutionConfiguration::default()
    }
}

/// Telemetry on and Tier-1 enabled, selecting a block once it has `threshold` hits.
fn tier1_arm(threshold: u64) -> RegisterExecutionConfiguration {
    RegisterExecutionConfiguration {
        tier1_enabled: true,
        tier1_hot_threshold: threshold,
        ..profiled()
    }
}

fn prepare(source: &str) -> PreparedModule {
    let bytes = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted source")
        .encode()
        .expect("serialize supplied application");
    PreparedModule::from_bytes(&bytes, PreparationLimits::default()).expect("admitted saved STBC")
}

/// One variable of a program instance, read through the storage accessors.
fn program_var(state: &RuntimeState<'_>, instance: &str, name: &str) -> Value {
    let Some(Value::Instance(root)) = state.storage().get_global(instance) else {
        panic!("program instance {instance}")
    };
    state
        .storage()
        .get_instance_var(*root, name)
        .unwrap_or_else(|| panic!("program variable {name}"))
        .clone()
}

/// `TaskState` has no equality, so compare its scheduling fields.
fn task_fields(state: &RuntimeState<'_>) -> Vec<(bool, Duration, u64)> {
    state
        .task_states()
        .iter()
        .map(|task| (task.last_single, task.last_run, task.overrun_count))
        .collect()
}

/// The string-element values after `cycle` executions of `STRING_ELEMENT_SOURCE`.
fn assert_string_program(state: &RuntimeState<'_>, instance: &str, cycle: i64) {
    assert_eq!(
        program_var(state, instance, "narrow_char"),
        Value::Char(b'B')
    );
    assert_eq!(
        program_var(state, instance, "wide_char"),
        Value::WChar(0x03b2)
    );
    assert_eq!(program_var(state, instance, "equal"), Value::Bool(true));
    assert_eq!(
        program_var(state, instance, "cycles"),
        Value::DInt(cycle as i32)
    );
}

/// Per-cycle `(sample, divisor)` for `INPUT_SOURCE`.
fn input_table(cycle: i64) -> (i32, i32) {
    let sample = (cycle * 3 - 7) as i32;
    let divisor = if cycle == 40 {
        0
    } else {
        (cycle % 7 + 1) as i32
    };
    (sample, divisor)
}

#[test]
fn default_threshold_tier1_boundary_is_exact_per_cycle() {
    let prepared = prepare(STRING_ELEMENT_SOURCE);
    // Profiling only records block hits; it does not take part in Tier-1 selection.
    let register_services = ConfiguredServices {
        stack: false,
        configuration: RegisterExecutionConfiguration {
            tier1_enabled: true,
            ..profiled()
        },
    };
    let stack_services = ConfiguredServices {
        stack: true,
        configuration: profiled(),
    };
    let mut register = prepared
        .instantiate_with_services(0, &register_services)
        .unwrap();
    let mut stack = prepared
        .instantiate_with_services(0, &stack_services)
        .unwrap();
    let preparation = register.tier1_snapshot();
    assert_eq!(preparation.hot_block_threshold, 64);
    for cycle in 1..=80_i64 {
        let time = Duration::from_millis(cycle * 10);
        register.execute_cycle(time).unwrap();
        stack.execute_cycle(time).unwrap();
        assert_string_program(&register, "Main", cycle);
        assert_string_program(&stack, "Main", cycle);
        assert_eq!(register.outputs(), stack.outputs(), "cycle {cycle}");
        // Every block of the program is entered once per cycle, so each block's 64th hit falls
        // on cycle 64 and from then on every block runs as Tier-1 once per cycle.
        let profile = register.register_profile_snapshot();
        assert!(
            !profile.hot_blocks.is_empty()
                && profile
                    .hot_blocks
                    .iter()
                    .all(|block| block.hits == cycle as u64),
            "cycle {cycle}: every block entered once per cycle: {profile:?}"
        );
        let blocks = profile.hot_blocks.len() as u64;
        let tier1 = register.tier1_snapshot();
        let expected: u64 = if cycle < 64 {
            0
        } else {
            blocks * (cycle - 63) as u64
        };
        assert_eq!(tier1.hot_block_threshold, 64);
        assert_eq!(tier1.block_executions, expected, "cycle {cycle}");
        assert_eq!(
            tier1.compile_attempts, preparation.compile_attempts,
            "RUN never compiles"
        );
    }
}

#[test]
fn tier1_threshold_one_matches_interpreter_per_cycle_with_inputs_and_fault() {
    let prepared = prepare(INPUT_SOURCE);
    let interpreter_services = ConfiguredServices {
        stack: false,
        configuration: RegisterExecutionConfiguration::default(),
    };
    let tier1_services = ConfiguredServices {
        stack: false,
        configuration: tier1_arm(1),
    };
    let mut interpreter = prepared
        .instantiate_with_services(0, &interpreter_services)
        .unwrap();
    let mut tier1 = prepared
        .instantiate_with_services(0, &tier1_services)
        .unwrap();
    assert_eq!(
        tier1.tier1_snapshot().compile_failures,
        0,
        "every block is Tier-1 eligible"
    );
    let mut completed = 0_i32;
    for cycle in 1..=80_i64 {
        let time = Duration::from_millis(cycle * 10);
        let (sample, divisor) = input_table(cycle);
        interpreter.clear_fault();
        tier1.clear_fault();
        interpreter
            .write_global("sample", Value::DInt(sample))
            .unwrap();
        interpreter
            .write_global("divisor", Value::DInt(divisor))
            .unwrap();
        tier1.write_global("sample", Value::DInt(sample)).unwrap();
        tier1.write_global("divisor", Value::DInt(divisor)).unwrap();
        let interpreter_result = interpreter.execute_cycle(time);
        let tier1_result = tier1.execute_cycle(time);
        let expected: Result<(), RuntimeError> = if divisor == 0 {
            Err(RuntimeError::ModuloByZero)
        } else {
            Ok(())
        };
        assert_eq!(interpreter_result, expected, "cycle {cycle}: interpreter");
        assert_eq!(tier1_result, interpreter_result, "cycle {cycle}: Tier-1");
        assert_eq!(tier1.fault(), interpreter.fault(), "cycle {cycle}: fault");
        for name in ["acc", "remainder", "cycles"] {
            assert_eq!(
                program_var(&tier1, "Main", name),
                program_var(&interpreter, "Main", name),
                "cycle {cycle}: {name}"
            );
        }
        assert_eq!(tier1.outputs(), interpreter.outputs(), "cycle {cycle}");
        if divisor != 0 {
            completed += 1;
            assert_eq!(
                program_var(&interpreter, "Main", "remainder"),
                Value::DInt(100 % divisor),
                "cycle {cycle}"
            );
            assert_eq!(
                program_var(&interpreter, "Main", "cycles"),
                Value::DInt(completed),
                "cycle {cycle}"
            );
        }
    }
    // One call-free block per cycle; the faulting cycle does not complete its block.
    let completed_cycles = (1..=80_i64)
        .filter(|cycle| input_table(*cycle).1 != 0)
        .count() as u64;
    assert_eq!(
        tier1.tier1_snapshot().block_executions,
        completed_cycles,
        "Tier-1 block executions"
    );
    assert_eq!(interpreter.tier1_snapshot().block_executions, 0);
}

#[test]
fn rare_if_and_case_branches_match_shared_stack_under_tier1() {
    let prepared = prepare(BRANCH_SOURCE);
    let stack_services = ConfiguredServices {
        stack: true,
        configuration: profiled(),
    };
    let tier1_services = ConfiguredServices {
        stack: false,
        configuration: tier1_arm(1),
    };
    let mut stack = prepared
        .instantiate_with_services(0, &stack_services)
        .unwrap();
    let mut tier1 = prepared
        .instantiate_with_services(0, &tier1_services)
        .unwrap();
    for cycle in 1..=80_i64 {
        let time = Duration::from_millis(cycle * 10);
        let stack_result = stack.execute_cycle(time);
        let tier1_result = tier1.execute_cycle(time);
        assert_eq!(tier1_result, stack_result, "cycle {cycle}");
        for name in [
            "cycles",
            "then_count",
            "else_count",
            "arm_zero",
            "arm_one",
            "arm_else",
        ] {
            assert_eq!(
                program_var(&tier1, "Main", name),
                program_var(&stack, "Main", name),
                "cycle {cycle}: {name}"
            );
        }
        assert_eq!(tier1.outputs(), stack.outputs(), "cycle {cycle}");
    }
    // toggle is TRUE on the 40 odd cycles and FALSE on the 40 even ones; selector is 0, 1 or 2.
    assert_eq!(program_var(&tier1, "Main", "then_count"), Value::DInt(40));
    assert_eq!(program_var(&tier1, "Main", "else_count"), Value::DInt(40));
    assert_eq!(program_var(&tier1, "Main", "arm_zero"), Value::DInt(27));
    assert_eq!(program_var(&tier1, "Main", "arm_one"), Value::DInt(27));
    assert_eq!(program_var(&tier1, "Main", "arm_else"), Value::DInt(26));
    let profile = tier1.register_profile_snapshot();
    assert_eq!(profile.register_program_fallbacks, 0, "{profile:?}");
    // The THEN and ELSE blocks are the two hot blocks entered on exactly 40 cycles each.
    let branch_blocks = profile
        .hot_blocks
        .iter()
        .filter(|block| block.hits == 40)
        .count();
    assert!(
        branch_blocks >= 2,
        "THEN and ELSE blocks entered: {:?}",
        profile.hot_blocks
    );
}

#[test]
fn warm_and_cold_restarts_keep_arms_equal_and_tier1_observations() {
    let prepared = prepare(RESTART_SOURCE);
    let stack_services = ConfiguredServices {
        stack: true,
        configuration: profiled(),
    };
    let tier1_services = ConfiguredServices {
        stack: false,
        configuration: tier1_arm(1),
    };
    let mut stack = prepared
        .instantiate_with_services(0, &stack_services)
        .unwrap();
    let mut tier1 = prepared
        .instantiate_with_services(0, &tier1_services)
        .unwrap();
    let preparation = tier1.tier1_snapshot();
    let (mut retained, mut plain) = (0_i32, 0_i32);
    for cycle in 1..=40_i64 {
        if cycle == 21 || cycle == 31 {
            let mode = if cycle == 21 {
                RestartMode::Warm
            } else {
                RestartMode::Cold
            };
            let before = tier1.tier1_snapshot();
            assert_eq!(
                before.block_executions,
                (cycle - 1) as u64,
                "cycle {cycle}: one Tier-1 execution per cycle"
            );
            stack.restart(mode).unwrap();
            tier1.restart(mode).unwrap();
            assert_eq!(
                tier1.tier1_snapshot(),
                before,
                "cycle {cycle}: restart keeps Tier-1 selection and counters"
            );
            plain = 0;
            if mode == RestartMode::Cold {
                retained = 0;
            }
        }
        let time = Duration::from_millis(cycle * 10);
        let stack_result = stack.execute_cycle(time);
        let tier1_result = tier1.execute_cycle(time);
        assert_eq!(tier1_result, stack_result, "cycle {cycle}");
        retained += 1;
        plain += 1;
        assert_eq!(
            program_var(&stack, "Main", "retained"),
            Value::DInt(retained),
            "cycle {cycle}"
        );
        assert_eq!(
            program_var(&tier1, "Main", "retained"),
            Value::DInt(retained),
            "cycle {cycle}"
        );
        assert_eq!(
            program_var(&stack, "Main", "plain"),
            Value::DInt(plain),
            "cycle {cycle}"
        );
        assert_eq!(
            program_var(&tier1, "Main", "plain"),
            Value::DInt(plain),
            "cycle {cycle}"
        );
        assert_eq!(tier1.outputs(), stack.outputs(), "cycle {cycle}");
    }
    let after = tier1.tier1_snapshot();
    assert_eq!(after.block_executions, 40);
    assert_eq!(
        after.compile_attempts, preparation.compile_attempts,
        "RUN never compiles"
    );
    assert_eq!(
        tier1.register_profile_snapshot().register_programs_executed,
        40
    );
}

#[test]
fn periodic_task_state_matches_shared_stack_per_cycle() {
    let source = format!(
        "{}\nCONFIGURATION Conf RESOURCE Controller ON PLC TASK Periodic (INTERVAL := T#10ms, PRIORITY := 0); PROGRAM Plant WITH Periodic : Main; END_RESOURCE END_CONFIGURATION",
        STRING_ELEMENT_SOURCE
    );
    let prepared = prepare(&source);
    let register_services = ConfiguredServices {
        stack: false,
        configuration: RegisterExecutionConfiguration::default(),
    };
    let stack_services = ConfiguredServices {
        stack: true,
        configuration: RegisterExecutionConfiguration::default(),
    };
    let mut register = prepared
        .instantiate_with_services(0, &register_services)
        .unwrap();
    let mut stack = prepared
        .instantiate_with_services(0, &stack_services)
        .unwrap();
    for cycle in 1..=80_i64 {
        let time = Duration::from_millis(cycle * 10);
        register.execute_cycle(time).unwrap();
        stack.execute_cycle(time).unwrap();
        assert_eq!(
            task_fields(&register),
            task_fields(&stack),
            "cycle {cycle}: task state"
        );
        // The 10 ms task is due on every cycle, and its deadline advances by one interval.
        assert_eq!(
            task_fields(&register),
            vec![(false, Duration::from_millis(cycle * 10), 0_u64)],
            "cycle {cycle}"
        );
        assert_string_program(&register, "Plant", cycle);
        assert_string_program(&stack, "Plant", cycle);
        assert_eq!(register.outputs(), stack.outputs(), "cycle {cycle}");
    }
}

#[test]
fn call_free_profile_hits_split_into_interpreted_and_tier1_entries() {
    let prepared = prepare(STRING_ELEMENT_SOURCE);
    for threshold in [64_u64, 1] {
        let register_services = ConfiguredServices {
            stack: false,
            configuration: tier1_arm(threshold),
        };
        let stack_services = ConfiguredServices {
            stack: true,
            configuration: profiled(),
        };
        let mut register = prepared
            .instantiate_with_services(0, &register_services)
            .unwrap();
        let mut stack = prepared
            .instantiate_with_services(0, &stack_services)
            .unwrap();
        for cycle in 1..=80_i64 {
            let time = Duration::from_millis(cycle * 10);
            register.execute_cycle(time).unwrap();
            stack.execute_cycle(time).unwrap();
        }
        let profile = register.register_profile_snapshot();
        let tier1 = register.tier1_snapshot();
        let blocks = profile.hot_blocks.len() as u64;
        assert!(
            blocks > 0 && profile.hot_blocks.iter().all(|block| block.hits == 80),
            "every call-free block entered once per cycle: {profile:?}"
        );
        let entries: u64 = profile.hot_blocks.iter().map(|block| block.hits).sum();
        assert_eq!(entries, blocks * 80, "{profile:?}");
        assert_eq!(profile.register_programs_executed, 80);
        assert_eq!(profile.register_program_fallbacks, 0);
        // Each block's entries before its threshold-th hit are interpreted; the rest run as
        // Tier-1 blocks.
        let interpreted = entries
            .checked_sub(tier1.block_executions)
            .expect("Tier-1 executions are block entries");
        assert_eq!(
            interpreted,
            blocks * (threshold - 1),
            "threshold {threshold}: {tier1:?}"
        );
        // The register profile records interpreted entries itself, not as the hits less Tier-1.
        assert_eq!(
            profile.interpreted_block_entries, interpreted,
            "threshold {threshold}: {profile:?} {tier1:?}"
        );
        let stack_profile = stack.register_profile_snapshot();
        assert_eq!(
            stack_profile.register_programs_executed, 0,
            "stack arm: {stack_profile:?}"
        );
        assert_eq!(
            stack_profile.interpreted_block_entries, 0,
            "stack arm: {stack_profile:?}"
        );
    }
}

/// A FOR loop whose zero-step guard is a HALT; the literal step is 1, so the loop runs.
const FOR_LOOP_SOURCE: &str = r#"
PROGRAM Main
VAR
    i : DINT := 0;
    s : DINT := 0;
    cycles : DINT := 0;
END_VAR
FOR i := 1 TO 3 DO s := s + i; END_FOR;
cycles := cycles + 1;
END_PROGRAM
"#;

/// A FOR loop whose step is a global that the host writes between cycles.
const FOR_STEP_SOURCE: &str = r#"
VAR_GLOBAL
    step_size : DINT := 1;
END_VAR
PROGRAM Main
VAR
    i : DINT := 0;
    total : DINT := 0;
    cycles : DINT := 0;
END_VAR
FOR i := 1 TO 3 BY step_size DO
    total := total + i;
END_FOR;
cycles := cycles + 1;
END_PROGRAM
"#;

/// The control variable, body accumulator and cycle counter of `FOR_STEP_SOURCE`.
fn for_step_vars(state: &RuntimeState<'_>) -> [Value; 3] {
    ["i", "total", "cycles"].map(|name| program_var(state, "Main", name))
}

#[test]
fn for_loop_runs_on_the_register_path_without_a_fallback_and_matches_the_stack_arm() {
    let prepared = prepare(FOR_LOOP_SOURCE);
    let register_services = ConfiguredServices {
        stack: false,
        configuration: profiled(),
    };
    let stack_services = ConfiguredServices {
        stack: true,
        configuration: profiled(),
    };
    let mut register = prepared
        .instantiate_with_services(0, &register_services)
        .unwrap();
    let mut stack = prepared
        .instantiate_with_services(0, &stack_services)
        .unwrap();
    for cycle in 1..=20_i64 {
        let time = Duration::from_millis(cycle * 10);
        register.execute_cycle(time).unwrap();
        stack.execute_cycle(time).unwrap();
        for name in ["i", "s", "cycles"] {
            assert_eq!(
                program_var(&register, "Main", name),
                program_var(&stack, "Main", name),
                "cycle {cycle}: {name}"
            );
        }
        // Each cycle adds 1 + 2 + 3 to the running sum.
        assert_eq!(
            program_var(&register, "Main", "s"),
            Value::DInt(6 * cycle as i32),
            "cycle {cycle}"
        );
        assert_eq!(register.outputs(), stack.outputs(), "cycle {cycle}");
    }
    let profile = register.register_profile_snapshot();
    assert!(profile.register_programs_executed > 0, "{profile:?}");
    assert!(profile.fallback_reasons.is_empty(), "{profile:?}");
}

#[test]
fn for_step_zero_faults_identically_on_stack_interpreter_and_tier1_threshold_one() {
    let prepared = prepare(FOR_STEP_SOURCE);
    let stack_services = ConfiguredServices {
        stack: true,
        configuration: profiled(),
    };
    let interpreter_services = ConfiguredServices {
        stack: false,
        configuration: profiled(),
    };
    let tier1_services = ConfiguredServices {
        stack: false,
        configuration: tier1_arm(1),
    };
    let mut stack = prepared
        .instantiate_with_services(0, &stack_services)
        .unwrap();
    let mut interpreter = prepared
        .instantiate_with_services(0, &interpreter_services)
        .unwrap();
    let mut tier1 = prepared
        .instantiate_with_services(0, &tier1_services)
        .unwrap();
    for cycle in 1..=6_i64 {
        let time = Duration::from_millis(cycle * 10);
        // The step is zero on cycle 3 only, so the guard faults once.
        let step: i32 = if cycle == 3 { 0 } else { 1 };
        let before = for_step_vars(&stack);
        stack.clear_fault();
        interpreter.clear_fault();
        tier1.clear_fault();
        stack.write_global("step_size", Value::DInt(step)).unwrap();
        interpreter
            .write_global("step_size", Value::DInt(step))
            .unwrap();
        tier1.write_global("step_size", Value::DInt(step)).unwrap();
        let stack_result = stack.execute_cycle(time);
        let interpreter_result = interpreter.execute_cycle(time);
        let tier1_result = tier1.execute_cycle(time);
        let expected: Result<(), RuntimeError> = if step == 0 {
            Err(RuntimeError::ForStepZero)
        } else {
            Ok(())
        };
        assert_eq!(stack_result, expected, "cycle {cycle}: stack");
        assert_eq!(
            interpreter_result, stack_result,
            "cycle {cycle}: interpreter"
        );
        assert_eq!(tier1_result, stack_result, "cycle {cycle}: Tier-1");
        assert_eq!(interpreter.fault(), stack.fault(), "cycle {cycle}: fault");
        assert_eq!(tier1.fault(), stack.fault(), "cycle {cycle}: Tier-1 fault");
        assert_eq!(
            for_step_vars(&interpreter),
            for_step_vars(&stack),
            "cycle {cycle}: interpreter values"
        );
        assert_eq!(
            for_step_vars(&tier1),
            for_step_vars(&stack),
            "cycle {cycle}: Tier-1 values"
        );
        if step == 0 {
            // The guard runs before the control variable is stored or the body is entered.
            assert_eq!(
                for_step_vars(&stack),
                before,
                "cycle {cycle}: destination unchanged"
            );
        }
        assert_eq!(tier1.outputs(), stack.outputs(), "cycle {cycle}");
    }
    // The faulting trap came from the register interpreter, not from a POU fallback.
    let profile = interpreter.register_profile_snapshot();
    assert!(profile.register_programs_executed > 0, "{profile:?}");
    assert!(profile.fallback_reasons.is_empty(), "{profile:?}");
    assert!(
        tier1.tier1_snapshot().block_executions > 0,
        "{:?}",
        tier1.tier1_snapshot()
    );
}
