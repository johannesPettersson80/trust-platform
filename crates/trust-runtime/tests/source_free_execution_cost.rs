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
    #[cfg(feature = "register-ir")]
    state.set_register_profile_enabled(true);
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    let Some(Value::Instance(main)) = state.storage().get_global("Main") else {
        panic!("program root")
    };
    assert_eq!(
        state.storage().get_instance_var(*main, "answer"),
        Some(&Value::Int(132))
    );
    #[cfg(feature = "register-ir")]
    assert!(
        state.register_profile_snapshot().register_programs_executed >= 32,
        "the admitted depth result must include distinct register-IR callees"
    );
    state.restart(RestartMode::Cold).unwrap();
    state.execute_cycle(Duration::from_millis(20)).unwrap();
    assert!(state.fault().is_none());
    let Some(Value::Instance(main)) = state.storage().get_global("Main") else {
        panic!("program root")
    };
    assert_eq!(
        state.storage().get_instance_var(*main, "answer"),
        Some(&Value::Int(132)),
        "the cold restart re-runs the nested chain on the next cycle"
    );

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

#[cfg(feature = "register-ir")]
#[test]
fn compiled_nested_calls_resume_inside_tier1_blocks_at_admitted_depth() {
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
    let prepared = PreparedModule::from_bytes(
        &module.encode().unwrap(),
        PreparationLimits {
            max_call_depth: 33,
            ..PreparationLimits::default()
        },
    )
    .unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.set_register_profile_enabled(true);
    state.set_tier1_enabled(true);
    for tick in 1..=64 {
        state
            .execute_cycle(Duration::from_millis(tick * 10))
            .expect("nested register calls must resume within their selected tier-1 block");
    }
    let Some(Value::Instance(main)) = state.storage().get_global("Main") else {
        panic!("program root")
    };
    assert_eq!(
        state.storage().get_instance_var(*main, "answer"),
        Some(&Value::Int(132))
    );
    assert!(state.register_profile_snapshot().register_programs_executed >= 32);
    assert!(
        state.tier1_snapshot().block_executions > 0,
        "the nested call chain must actually resume a tier-1 block"
    );
}

#[cfg(feature = "register-ir")]
#[test]
fn recursive_stack_fallback_can_enter_a_distinct_register_callee() {
    let source = r#"
NAMESPACE Demo
FUNCTION Helper : INT
Helper := INT#5;
END_FUNCTION
FUNCTION Recur : INT
VAR_INPUT level : INT; END_VAR
IF level = INT#0 THEN
    Recur := INT#1;
ELSE
    Recur := Demo.Recur(level := level - INT#1) + Helper();
END_IF;
END_FUNCTION
END_NAMESPACE
PROGRAM Main
VAR answer : INT; END_VAR
answer := Demo.Recur(level := INT#2);
END_PROGRAM
"#;
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let prepared = PreparedModule::from_bytes(
        &module.encode().unwrap(),
        PreparationLimits {
            max_call_depth: 8,
            ..PreparationLimits::default()
        },
    )
    .unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.set_register_profile_enabled(true);
    state
        .execute_cycle(Duration::from_millis(10))
        .expect("recursive occupancy should select stack, then resume the helper in registers");

    let Some(Value::Instance(main)) = state.storage().get_global("Main") else {
        panic!("program root")
    };
    assert_eq!(
        state.storage().get_instance_var(*main, "answer"),
        Some(&Value::Int(11))
    );
    let profile = state.register_profile_snapshot();
    assert!(profile.register_programs_executed >= 4);
    assert!(profile
        .fallback_reasons
        .iter()
        .any(|reason| reason.reason == "nested_scratch" && reason.count >= 1));
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
    // `Capture` makes no user call, so register builds now run it in place;
    // `in_place_leaf_calls_match_the_pump` compares that with the pump.
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

#[test]
fn failed_leaf_function_withholds_in_out_copyback() {
    // A callee that makes no user call runs in place of a pump suspension. Its
    // failure must leave the caller as the pump does: the original fault and no
    // IN_OUT copy-back. The function-block edge case is covered above.
    let source = r#"
VAR_GLOBAL divisor : INT := INT#1; END_VAR
FUNCTION Scale : INT
VAR_IN_OUT value : INT; END_VAR
value := value + INT#1;
Scale := value / divisor;
END_FUNCTION
PROGRAM Main
VAR count : INT := INT#5; scaled : INT; END_VAR
scaled := Scale(value := count);
END_PROGRAM
"#;
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let prepared =
        PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default())
            .unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    #[cfg(feature = "register-ir")]
    state.set_register_profile_enabled(true);
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    let Some(Value::Instance(main)) = state.storage().get_global("Main") else {
        panic!("program root")
    };
    let main = *main;
    assert_eq!(
        state.storage().get_instance_var(main, "count"),
        Some(&Value::Int(6))
    );
    assert_eq!(
        state.storage().get_instance_var(main, "scaled"),
        Some(&Value::Int(6))
    );
    #[cfg(feature = "register-ir")]
    assert!(
        state.register_profile_snapshot().register_programs_executed >= 2,
        "the program and its leaf callee must run as register activations"
    );

    state.write_global("divisor", Value::Int(0)).unwrap();
    assert_eq!(
        state.execute_cycle(Duration::from_millis(20)),
        Err(trust_runtime_core::error::RuntimeError::DivisionByZero)
    );
    assert_eq!(
        state.storage().get_instance_var(main, "count"),
        Some(&Value::Int(6)),
        "a failed callee must not copy its IN_OUT value back"
    );
    assert_eq!(
        state.storage().get_instance_var(main, "scaled"),
        Some(&Value::Int(6))
    );
}

/// One cycle's result and the watched program variables after it.
#[cfg(feature = "register-ir")]
type CycleObservation = (
    Result<(), trust_runtime_core::error::RuntimeError>,
    Vec<Option<Value>>,
);

/// Everything a caller can observe from one run, for pump/in-place parity.
#[cfg(feature = "register-ir")]
#[derive(Debug, PartialEq)]
struct CallObservation {
    cycles: Vec<CycleObservation>,
    fault: Option<trust_runtime_core::error::RuntimeError>,
    profile: String,
    tier1: String,
}

/// Runs `source` for 24 cycles, zeroing `failing` before cycle 21, with leaf calls
/// in place or through the pump. Returns the observation and the in-place count.
#[cfg(feature = "register-ir")]
fn observe_calls(
    source: &str,
    max_call_depth: usize,
    failing: Option<&str>,
    in_place: bool,
) -> (CallObservation, u64) {
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let prepared = PreparedModule::from_bytes(
        &module.encode().unwrap(),
        PreparationLimits {
            max_call_depth,
            ..PreparationLimits::default()
        },
    )
    .unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.set_register_profile_enabled(true);
    state.set_tier1_enabled(true);
    state.set_in_place_calls_enabled(in_place);
    let mut cycles = Vec::new();
    for tick in 1..=24i64 {
        if tick == 21
            && let Some(name) = failing
        {
            state.write_global(name, Value::Int(0)).unwrap();
        }
        let result = state.execute_cycle(Duration::from_millis(tick * 10));
        let Some(Value::Instance(main)) = state.storage().get_global("Main").cloned() else {
            panic!("program root")
        };
        let values = ["count", "scaled", "seen", "answer"]
            .iter()
            .map(|name| state.storage().get_instance_var(main, name).cloned())
            .collect();
        let failed = result.is_err();
        cycles.push((result, values));
        if failed {
            break;
        }
    }
    let observation = CallObservation {
        cycles,
        fault: state.fault().cloned(),
        profile: format!("{:?}", state.register_profile_snapshot()),
        tier1: format!("{:?}", state.tier1_snapshot()),
    };
    (observation, state.in_place_call_count())
}

#[cfg(feature = "register-ir")]
const LEAF_PARITY_SOURCE: &str = r#"
VAR_GLOBAL in_divisor : INT := INT#1; fb_divisor : INT := INT#1; END_VAR
FUNCTION Scale : INT
VAR_IN_OUT value : INT; END_VAR
VAR_INPUT increment : INT := INT#1; END_VAR
value := value + increment;
Scale := value / in_divisor;
END_FUNCTION
FUNCTION_BLOCK Edge
VAR_INPUT pulse : BOOL R_EDGE; END_VAR
VAR_OUTPUT seen : INT; END_VAR
IF pulse THEN seen := seen + INT#1; END_IF;
seen := seen / fb_divisor;
END_FUNCTION_BLOCK
FUNCTION Outer : INT
VAR local : INT := INT#2; END_VAR
Outer := Scale(value := local) + local;
END_FUNCTION
PROGRAM Main
VAR count : INT := INT#5; scaled : INT; edge : Edge; seen : INT; answer : INT; END_VAR
scaled := Scale(value := count);
edge(pulse := TRUE, seen => seen);
answer := Outer();
END_PROGRAM
"#;

#[cfg(feature = "register-ir")]
#[test]
fn in_place_leaf_calls_match_the_pump() {
    // Success over tier-1 warmup, a fault in a leaf function (IN_OUT withheld), a
    // fault in a leaf function block (edge input and output withheld), and a
    // depth refusal at the leaf call. Each runs both ways and must agree on
    // results, faults, values, profile and tier-1 counters.
    let cases = [
        (64, None),
        (64, Some("in_divisor")),
        (64, Some("fb_divisor")),
        (2, None),
    ];
    for (depth, failing) in cases {
        let (in_place, in_place_count) = observe_calls(LEAF_PARITY_SOURCE, depth, failing, true);
        let (pumped, pumped_count) = observe_calls(LEAF_PARITY_SOURCE, depth, failing, false);
        assert_eq!(in_place, pumped, "depth {depth}, failing {failing:?}");
        let expected_fault = match (depth, failing) {
            (2, _) => Some(trust_runtime_core::vm::VmTrap::CallStackOverflow.into_runtime_error()),
            (_, Some(_)) => Some(trust_runtime_core::error::RuntimeError::DivisionByZero),
            _ => None,
        };
        assert_eq!(
            in_place.fault, expected_fault,
            "depth {depth}, failing {failing:?}"
        );
        assert!(in_place_count > 0, "leaf calls must run in place");
        assert_eq!(
            pumped_count, 0,
            "the switch must route every call through the pump"
        );
    }
}

#[cfg(feature = "register-ir")]
#[test]
fn in_place_leaf_calls_charge_the_pump_work() {
    // TIME-08: the in-place path charges exactly the pump's work for the same scan.
    let bytes = CompileSession::from_source(LEAF_PARITY_SOURCE)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap()
        .encode()
        .unwrap();
    let admitted = |max_work: usize, in_place: bool| {
        let prepared = PreparedModule::from_bytes(
            &bytes,
            PreparationLimits {
                max_work,
                ..PreparationLimits::default()
            },
        )
        .unwrap();
        let mut state = prepared.instantiate(0).unwrap();
        state.set_in_place_calls_enabled(in_place);
        state.execute_cycle(Duration::from_millis(10)).is_ok()
    };
    let minimum = |in_place: bool| {
        let (mut low, mut high) = (1usize, 100_000usize);
        assert!(admitted(high, in_place));
        while low < high {
            let middle = low + (high - low) / 2;
            if admitted(middle, in_place) {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        low
    };
    assert_eq!(minimum(true), minimum(false));
}
