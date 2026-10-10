//! Application-size work must depend on touched values, not every declaration per store.
use trust_runtime::bytecode::BytecodeVersion;
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
