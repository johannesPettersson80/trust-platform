//! Spec 34 Scope B: the board probe uses the real source-free process-image path.
//! Native execution does not claim PC13/PA5 electrical or MSP/IRQ evidence.
use trust_runtime::{
    bytecode::BytecodeVersion,
    harness::{CompileSession, SourceFile},
};
use trust_runtime_core::{
    bytecode::ValidationLimits,
    value::{Duration, Value},
    vm::{PreparationLimits, PreparedModule},
};

#[test]
fn f401_gpio_probe_executes_both_input_levels_at_all_four_call_depths() {
    let source = include_str!("fixtures/portability/f401/gpio.st");
    let bytecode = CompileSession::from_sources(vec![SourceFile::with_path(
        "portability/f401/gpio.st",
        source,
    )])
    .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
    .expect("board GPIO probe must be accepted source");
    let bytes = bytecode
        .encode()
        .expect("serialize the independent board artifact");
    // These logical admission limits match the firmware profile. They do not
    // stand in for its independently measured 72 KiB heap or native stack fit.
    let prepared = PreparedModule::from_bytes(
        &bytes,
        PreparationLimits {
            max_artifact_bytes: 16 * 1024,
            max_preparation_bytes: 512 * 1024,
            max_preparation_work: 1_000_000,
            validation: ValidationLimits {
                max_scratch_bytes: 48 * 1024,
                max_work: 500_000,
            },
            max_construction_values: 4096,
            max_construction_bytes: 128 * 1024,
            max_process_image_bytes: 64,
            max_work: 50_000,
            max_call_depth: 4,
        },
    )
    .expect("board profile must admit the authored GPIO probe");
    let mut state = prepared
        .instantiate(0)
        .expect("construct a fresh compiler-free engine");
    let Some(Value::Instance(plant)) = state.storage().get_global("Plant") else {
        panic!("the saved board contract names the program root Plant");
    };
    let plant = *plant;
    assert_eq!(state.inputs_mut().len(), 1);
    assert_eq!(state.outputs(), &[0]);
    assert_eq!(state.task_states().len(), 1);
    let mut milliseconds = 0;
    for depth in 1..=4i16 {
        state
            .write_global("probe_depth", Value::Int(depth))
            .unwrap();
        for high in [false, true] {
            milliseconds += 10;
            state.inputs_mut()[0] = u8::from(high);
            state
                .execute_cycle(Duration::from_millis(milliseconds))
                .expect("the admitted nested call path must execute");
            assert_eq!(
                state.storage().get_instance_var(plant, "button"),
                Some(&Value::Bool(high))
            );
            assert_eq!(
                state.storage().get_instance_var(plant, "led"),
                Some(&Value::Bool(high))
            );
            assert_eq!(
                state.outputs(),
                &[u8::from(high)],
                "input level at depth {depth}"
            );
            assert_eq!(
                state.storage().get_instance_var(plant, "depth_result"),
                Some(&Value::Int(depth)),
                "the selected nested POU chain must actually execute"
            );
            assert_eq!(
                state.task_states()[0].last_run,
                Duration::from_millis(milliseconds)
            );
            assert_eq!(state.task_states()[0].overrun_count, 0);
            assert!(state.fault().is_none());
        }
    }
}
