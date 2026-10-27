//! Writable connections that coincide only at run time commit in formal declaration order.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.
use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};

/// Runs `source` for two cycles and returns the elements of `global` (a scalar is a one-element
/// list) after each cycle.
fn global_after_each_cycle(source: &str, global: &str) -> Vec<Vec<Value>> {
    let mut harness = TestHarness::from_source(source).unwrap();
    (1..=2i64)
        .map(|tick| {
            let runtime = harness.runtime_mut();
            runtime.set_current_time(Duration::from_millis(tick * 10));
            runtime
                .execute_cycle()
                .expect("coinciding writable connections are not an error");
            match runtime.read_variable(global) {
                Ok(Value::Array(array)) => array.elements().to_vec(),
                Ok(value) => vec![value],
                Err(error) => panic!("global {global}: {error}"),
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
    let after = global_after_each_cycle(&source, "cells");
    let expected = vec![Value::Int(0), Value::Int(12), Value::Int(0), Value::Int(0)];
    assert_eq!(after, vec![expected.clone(), expected]);
}

/// Spec 10 §7.5: in-outs that coincide at run time are one variable: `first` makes cell 1 equal to 1,
/// `second` reads that 1 and makes it 11; the next cycle starts from 11 and ends at 22. A snapshot
/// per in-out would end each cycle at the second's 10 and 20.
#[test]
fn call_path_coinciding_in_outs_are_one_variable() {
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
    let after = global_after_each_cycle(&source, "cells");
    let cycle_one = vec![Value::Int(0), Value::Int(11), Value::Int(0), Value::Int(0)];
    let cycle_two = vec![Value::Int(0), Value::Int(22), Value::Int(0), Value::Int(0)];
    assert_eq!(after, vec![cycle_one, cycle_two]);
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
    let after = global_after_each_cycle(&source, "cells");
    let expected = vec![Value::Int(0), Value::Int(12), Value::Int(0), Value::Int(0)];
    assert_eq!(after, vec![expected.clone(), expected]);
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
    let after = global_after_each_cycle(&source, "counter");
    assert_eq!(after, vec![vec![Value::Int(12)], vec![Value::Int(12)]]);
}
