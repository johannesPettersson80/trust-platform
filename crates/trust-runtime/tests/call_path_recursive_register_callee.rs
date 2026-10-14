//! A distinct eligible callee keeps register execution while a recursive POU occupies the stack path.
#![cfg(feature = "register-ir")]
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

/// `Recur` adds `term` to the result of the level below it, so each level above zero adds one `term`.
fn recursion_source(term: &str) -> String {
    format!(
        r#"
NAMESPACE Demo
FUNCTION Helper : INT
Helper := INT#5;
END_FUNCTION
FUNCTION Recur : INT
VAR_INPUT level : INT; END_VAR
IF level = INT#0 THEN
    Recur := INT#1;
ELSE
    Recur := Demo.Recur(level := level - INT#1) + {term};
END_IF;
END_FUNCTION
END_NAMESPACE
PROGRAM Main
VAR answer : INT; END_VAR
answer := Demo.Recur(level := INT#2);
END_PROGRAM
"#
    )
}

struct Run {
    answer: Value,
    register_programs_executed: u64,
}

fn run(term: &str) -> Run {
    let module = CompileSession::from_source(recursion_source(term))
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let limits = PreparationLimits {
        max_call_depth: 8,
        ..PreparationLimits::default()
    };
    let prepared = PreparedModule::from_bytes(&module.encode().unwrap(), limits).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.set_register_profile_enabled(true);
    state
        .execute_cycle(Duration::from_millis(10))
        .expect("the recursive program runs to completion");
    let Some(Value::Instance(main)) = state.storage().get_global("Main") else {
        panic!("program root")
    };
    let answer = state
        .storage()
        .get_instance_var(*main, "answer")
        .cloned()
        .expect("answer");
    Run {
        answer,
        register_programs_executed: state.register_profile_snapshot().register_programs_executed,
    }
}

/// Spec 34 §4.3 HOST-04: calls to a distinct eligible POU retain register execution, while recursive re-entry into
/// the occupied POU may use the shared stack fallback.
///
/// Counted register executions are the activations that complete in registers (`record_executed`):
/// - Main, the program root, is an eligible register activation: 1.
/// - Recur at level 2 finds Recur's scratch free, so it is a register activation: 1.
/// - Recur at levels 1 and 0 re-enter the occupied Recur and run on the stack fallback: 0.
/// - Helper is called from level 2 and from level 1, each time with its own scratch free: 2.
///
/// Total 4. With the Helper calls replaced by a constant, only Main and Recur at level 2 run in registers: 2.
/// The difference of 2 is the register executions of Helper.
#[test]
fn call_path_distinct_eligible_callee_runs_in_registers_during_recursion() {
    let with_helper = run("Helper()");
    let without_helper = run("INT#5");
    assert_eq!(with_helper.answer, Value::Int(11));
    assert_eq!(without_helper.answer, Value::Int(11));
    assert_eq!(with_helper.register_programs_executed, 4);
    assert_eq!(without_helper.register_programs_executed, 2);
}
