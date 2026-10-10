//! Application-size work must depend on touched values, not every declaration per store.
use trust_runtime::bytecode::{BytecodeVersion, SectionData, SectionId};
use trust_runtime::harness::CompileSession;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

#[test]
fn three_thousand_declarations_and_two_thousand_stores_fit_default_work_limit() {
    let mut source = String::from("VAR_GLOBAL\n");
    for index in 0..3000 {
        source.push_str(&format!("cell_{index} : DINT;\n"));
    }
    source.push_str("END_VAR\nPROGRAM Main\n");
    for index in 0..2000 {
        source.push_str(&format!("cell_{index} := cell_{index} + DINT#1;\n"));
    }
    source.push_str("END_PROGRAM\n");
    let module = CompileSession::from_source(&source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let prepared =
        PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default())
            .unwrap();
    let mut state = prepared
        .instantiate(0)
        .expect("ordinary default construction fits default limits");
    for sample in [10, 20] {
        state
            .execute_cycle(Duration::from_millis(sample))
            .expect("2,000 ordinary stores fit the default shared work budget");
        for index in 0..3000 {
            let expected = if index < 2000 {
                (sample / 10) as i32
            } else {
                0
            };
            assert_eq!(
                state.storage().get_global(&format!("cell_{index}")),
                Some(&Value::DInt(expected))
            );
        }
    }
    state.restart(RestartMode::Cold).unwrap();
    state.execute_cycle(Duration::from_millis(30)).unwrap();
    assert_eq!(
        state.storage().get_global("cell_1999"),
        Some(&Value::DInt(1))
    );
    assert!(state.fault().is_none());
}

#[test]
fn two_thousand_distinct_input_bindings_fit_default_work_limit() {
    let mut source = String::from("PROGRAM Main\nVAR\n");
    for index in 0..2000 {
        source.push_str(&format!("input_{index} AT %ID{} : DINT;\n", index * 4));
    }
    source.push_str("END_VAR\nEND_PROGRAM\n");
    let module = CompileSession::from_source(&source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let prepared =
        PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default())
            .unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    for sample in [1i32, 2] {
        for index in 0..2000usize {
            let value = sample * (index as i32 + 1);
            state.inputs_mut()[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        state
            .execute_cycle(Duration::from_millis(i64::from(sample) * 10))
            .expect("input transaction must not do a quadratic destination deduplication scan");
        let Some(Value::Instance(main)) = state.storage().get_global("Main") else {
            panic!("program root")
        };
        for index in 0..2000 {
            assert_eq!(
                state
                    .storage()
                    .get_instance_var(*main, &format!("input_{index}")),
                Some(&Value::DInt(sample * (index + 1)))
            );
        }
        assert!(state.fault().is_none());
    }
}

#[test]
fn two_thousand_reverse_and_mixed_input_bindings_fit_default_work_limit() {
    let mut source = String::from("PROGRAM Main\nVAR\n");
    for index in 0..2000 {
        source.push_str(&format!("input_{index} AT %ID{} : DINT;\n", index * 4));
    }
    source.push_str("END_VAR\nEND_PROGRAM\n");
    let module = CompileSession::from_source(&source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    for mixed in [false, true] {
        let mut reordered = module.clone();
        let Some(SectionData::IoMap(io)) = reordered.section_mut(SectionId::IoMap) else {
            panic!("fixture must contain input bindings")
        };
        assert_eq!(io.bindings.len(), 2000);
        // Permute the artifact's actual binding order, not source declarations.
        // 997 is coprime to 2000, so the mixed permutation visits each binding once.
        let original = io.bindings.clone();
        for (index, binding) in io.bindings.iter_mut().enumerate() {
            let from = if mixed {
                (index * 997) % 2000
            } else {
                1999 - index
            };
            *binding = original[from].clone();
        }
        let prepared =
            PreparedModule::from_bytes(&reordered.encode().unwrap(), PreparationLimits::default())
                .unwrap();
        let mut state = prepared.instantiate(0).unwrap();
        for sample in [1i32, 2] {
            for index in 0..2000usize {
                state.inputs_mut()[index * 4..index * 4 + 4]
                    .copy_from_slice(&(sample * (index as i32 + 1)).to_le_bytes());
            }
            state
                .execute_cycle(Duration::from_millis(i64::from(sample) * 10))
                .expect("arbitrary binding order must fit the default shared work budget");
            let Some(Value::Instance(main)) = state.storage().get_global("Main") else {
                panic!("program root")
            };
            for index in 0..2000 {
                assert_eq!(
                    state
                        .storage()
                        .get_instance_var(*main, &format!("input_{index}")),
                    Some(&Value::DInt(sample * (index + 1)))
                );
            }
            assert!(state.fault().is_none());
        }
    }
}

#[test]
fn compiled_nested_calls_preserve_pending_operands_and_the_admitted_depth() {
    let mut source = String::new();
    for index in 0..32 {
        source.push_str(&format!("FUNCTION Nested_{index} : INT\n"));
        if index == 31 {
            source.push_str(&format!("Nested_{index} := INT#1;\n"));
        } else {
            source.push_str(&format!(
                "Nested_{index} := INT#1 + Nested_{}();\n",
                index + 1
            ));
        }
        source.push_str("END_FUNCTION\n");
    }
    source.push_str(
        "PROGRAM Main\nVAR answer : INT; END_VAR\nanswer := INT#100 + Nested_0();\nEND_PROGRAM\n",
    );
    let module = CompileSession::from_source(&source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let bytes = module.encode().unwrap();
    let prepared = PreparedModule::from_bytes(
        &bytes,
        PreparationLimits {
            max_call_depth: 33,
            ..PreparationLimits::default()
        },
    )
    .unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    let Some(Value::Instance(main)) = state.storage().get_global("Main") else {
        panic!("program root")
    };
    assert_eq!(
        state.storage().get_instance_var(*main, "answer"),
        Some(&Value::Int(132))
    );
    state.restart(RestartMode::Cold).unwrap();
    state.execute_cycle(Duration::from_millis(20)).unwrap();
    assert!(state.fault().is_none());

    let limited = PreparedModule::from_bytes(
        &bytes,
        PreparationLimits {
            max_call_depth: 32,
            ..PreparationLimits::default()
        },
    )
    .unwrap();
    let mut state = limited.instantiate(0).unwrap();
    let error = state.execute_cycle(Duration::from_millis(10)).unwrap_err();
    assert_eq!(
        error,
        trust_runtime_core::vm::VmTrap::CallStackOverflow.into_runtime_error()
    );
    assert_eq!(state.fault(), Some(&error));
}

#[test]
fn deferred_calls_preserve_null_presence_and_suspended_local_copyback() {
    let source = r#"
VAR_GLOBAL target : INT := INT#7; END_VAR
FUNCTION ReadDefault : INT
VAR_INPUT ref_arg : REF_TO INT := REF(target); END_VAR
IF ref_arg = NULL THEN ReadDefault := INT#9; ELSE ReadDefault := ref_arg^; END_IF;
END_FUNCTION
FUNCTION Change : INT
VAR_IN_OUT number : INT; END_VAR
VAR_OUTPUT copied : INT; END_VAR
number := number + INT#2;
copied := number + INT#3;
Change := ReadDefault() + ReadDefault(ref_arg := NULL);
END_FUNCTION
FUNCTION Outer : INT
VAR local : INT := INT#5; copied : INT; result : INT; END_VAR
result := Change(number := local, copied => copied);
Outer := local + copied + result;
END_FUNCTION
PROGRAM Main
VAR answer : INT; END_VAR
answer := INT#100 + Outer();
END_PROGRAM
"#;
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let prepared =
        PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default())
            .unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    let Some(Value::Instance(main)) = state.storage().get_global("Main") else {
        panic!("program root")
    };
    assert_eq!(
        state.storage().get_instance_var(*main, "answer"),
        Some(&Value::Int(133))
    );
}

#[test]
fn deferred_function_block_failure_restores_edge_input_and_withholds_copyback() {
    let source = r#"
VAR_GLOBAL divisor : INT := INT#1; copied : INT := INT#99; END_VAR
FUNCTION_BLOCK Capture
VAR_INPUT pulse : BOOL R_EDGE; END_VAR
VAR_OUTPUT result : INT; END_VAR
IF pulse THEN result := INT#10; ELSE result := INT#20; END_IF;
result := result / divisor;
END_FUNCTION_BLOCK
PROGRAM Main
VAR block : Capture; END_VAR
block(pulse := TRUE, result => copied);
END_PROGRAM
"#;
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let prepared =
        PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default())
            .unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    assert_eq!(state.storage().get_global("copied"), Some(&Value::Int(10)));
    state.write_global("divisor", Value::Int(0)).unwrap();
    assert_eq!(
        state.execute_cycle(Duration::from_millis(20)),
        Err(trust_runtime_core::error::RuntimeError::DivisionByZero)
    );
    let Some(Value::Instance(main)) = state.storage().get_global("Main") else {
        panic!("program root")
    };
    let Some(Value::Instance(block)) = state.storage().get_instance_var(*main, "block") else {
        panic!("block instance")
    };
    assert_eq!(
        state.storage().get_instance_var(*block, "pulse"),
        Some(&Value::Bool(true)),
        "restore raw TRUE after the failed call observed its FALSE edge pulse"
    );
    assert_eq!(
        state.storage().get_global("copied"),
        Some(&Value::Int(10)),
        "failed callee must not publish outputs"
    );
}
