//! Writable connections that coincide only at run time commit in formal declaration order.
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

/// Runs `source` for two cycles, with the default configuration or with register profiling and tier-1 on, and
/// returns the elements of `global` (a scalar is a one-element list) after each cycle.
fn global_after_each_cycle(source: &str, global: &str, configured: bool) -> Vec<Vec<Value>> {
    let bytes = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap()
        .encode()
        .unwrap();
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    if configured {
        state.set_register_profile_enabled(true);
        state.set_tier1_enabled(true);
    }
    (1..=2i64)
        .map(|tick| {
            state
                .execute_cycle(Duration::from_millis(tick * 10))
                .expect("coinciding writable connections are not an error");
            match state.storage().get_global(global) {
                Some(Value::Array(array)) => array.elements().to_vec(),
                Some(value) => vec![value.clone()],
                None => panic!("global {global}"),
            }
        })
        .collect()
}

const CELLS: &str = r#"
TYPE CellRow : ARRAY[0..3] OF INT; END_TYPE
VAR_GLOBAL cells : CellRow; counter : INT; END_VAR
"#;

const SPLIT_FUNCTION: &str = r#"
FUNCTION Split : INT
VAR_INPUT seed : INT; END_VAR
VAR_OUTPUT first : INT; second : INT; END_VAR
first := seed + INT#1;
second := seed + INT#2;
Split := INT#0;
END_FUNCTION
"#;

/// Spec 10 §7.5: `A[i]` and `A[j]` with `i = j` at run time are not an error; the last declared output's value
/// remains (function outputs).
#[test]
fn call_path_coinciding_function_outputs_keep_the_last_declared_value() {
    let source = format!(
        "{CELLS}{SPLIT_FUNCTION}
PROGRAM Main
VAR_EXTERNAL cells : CellRow; END_VAR
VAR i : INT := INT#1; j : INT := INT#1; result : INT; END_VAR
result := Split(seed := INT#10, first => cells[i], second => cells[j]);
END_PROGRAM
"
    );
    for configured in [false, true] {
        let after = global_after_each_cycle(&source, "cells", configured);
        let expected = vec![Value::Int(0), Value::Int(12), Value::Int(0), Value::Int(0)];
        assert_eq!(
            after,
            vec![expected.clone(), expected],
            "configured {configured}"
        );
    }
}

/// Spec 10 §7.5: coinciding in-out connections commit in declaration order; the second in-out's value remains,
/// and the next cycle snapshots that committed value.
#[test]
fn call_path_coinciding_in_outs_keep_the_last_declared_value() {
    let source = format!(
        "{CELLS}
FUNCTION Bump : INT
VAR_IN_OUT first : INT; second : INT; END_VAR
first := first + INT#1;
second := second + INT#10;
Bump := INT#0;
END_FUNCTION
PROGRAM Main
VAR_EXTERNAL cells : CellRow; END_VAR
VAR i : INT := INT#1; j : INT := INT#1; result : INT; END_VAR
result := Bump(first := cells[i], second := cells[j]);
END_PROGRAM
"
    );
    for configured in [false, true] {
        let after = global_after_each_cycle(&source, "cells", configured);
        let cycle_one = vec![Value::Int(0), Value::Int(10), Value::Int(0), Value::Int(0)];
        let cycle_two = vec![Value::Int(0), Value::Int(20), Value::Int(0), Value::Int(0)];
        assert_eq!(after, vec![cycle_one, cycle_two], "configured {configured}");
    }
}

/// Spec 10 §7.5: the same rule holds for function-block outputs, which commit after the body returns.
#[test]
fn call_path_coinciding_function_block_outputs_keep_the_last_declared_value() {
    let source = format!(
        "{CELLS}
FUNCTION_BLOCK Split
VAR_INPUT seed : INT; END_VAR
VAR_OUTPUT first : INT; second : INT; END_VAR
first := seed + INT#1;
second := seed + INT#2;
END_FUNCTION_BLOCK
PROGRAM Main
VAR_EXTERNAL cells : CellRow; END_VAR
VAR worker : Split; i : INT := INT#1; j : INT := INT#1; END_VAR
worker(seed := INT#10, first => cells[i], second => cells[j]);
END_PROGRAM
"
    );
    for configured in [false, true] {
        let after = global_after_each_cycle(&source, "cells", configured);
        let expected = vec![Value::Int(0), Value::Int(12), Value::Int(0), Value::Int(0)];
        assert_eq!(
            after,
            vec![expected.clone(), expected],
            "configured {configured}"
        );
    }
}

/// Spec 10 §7.5: a dereferenced reference and the variable it designates coincide only at run time; the last
/// declared formal's value remains.
#[test]
fn call_path_reference_and_designated_variable_keep_the_last_declared_value() {
    let source = format!(
        "{CELLS}{SPLIT_FUNCTION}
PROGRAM Main
VAR_EXTERNAL counter : INT; END_VAR
VAR p : REF_TO INT; result : INT; END_VAR
p := REF(counter);
result := Split(seed := INT#10, first => p^, second => counter);
END_PROGRAM
"
    );
    for configured in [false, true] {
        let after = global_after_each_cycle(&source, "counter", configured);
        assert_eq!(
            after,
            vec![vec![Value::Int(12)], vec![Value::Int(12)]],
            "configured {configured}"
        );
    }
}
