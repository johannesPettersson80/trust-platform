//! Saved numeric/control artifact: no compiler or HIR in the consumer.
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

#[test]
fn saved_numeric_artifact_preserves_accuracy_control_and_nominal_periods() {
    // The authorized batch generates this artifact once on the builder before
    // this dependent suite, then retains its exact bytes for board replay.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../trust-runtime/tests/fixtures/portability/stbc-2.0/numeric-v2.stbc");
    let bytes = std::fs::read(path).expect("numeric artifact generated during batch preparation");
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let cases = [
        ("v_sqrt", core::f64::consts::SQRT_2),
        ("v_ln", core::f64::consts::LN_2),
        ("v_log", 1.0),
        ("v_exp", core::f64::consts::E),
        ("v_sin", 0.479_425_538_604_203),
        ("v_cos", 0.877_582_561_890_372_8),
        ("v_tan", 0.546_302_489_843_790_5),
        ("v_asin", core::f64::consts::FRAC_PI_6),
        ("v_acos", core::f64::consts::FRAC_PI_3),
        ("v_atan", core::f64::consts::FRAC_PI_4),
    ];
    for ms in (0..=1000).step_by(10) {
        state.execute_cycle(Duration::from_millis(ms)).unwrap();
        let Some(Value::Instance(plant)) = state.storage().get_global("Plant") else {
            panic!("program root")
        };
        assert_eq!(
            state.storage().get_instance_var(*plant, "activations"),
            Some(&Value::DInt((ms / 25) as i32))
        );
        assert_eq!(state.task_states()[0].overrun_count, 0);
        if ms < 30 {
            continue;
        }
        for (name, reference) in cases {
            let Some(Value::LReal(value)) = state.storage().get_instance_var(*plant, name) else {
                panic!("LREAL {name}")
            };
            let tolerance = 8.0 * f64::EPSILON * reference.abs().max(1.0);
            assert!((value - reference).abs() <= tolerance, "{name} at {ms}");
            if ms == 1000 {
                println!("numeric,{ms},{name},{:016x}", value.to_bits());
            }
            assert!(*value > reference - 2.0 * tolerance && *value < reference + 2.0 * tolerance);
        }
        for (name, expected) in [
            ("exact_root", Value::Real(3.0)),
            ("exact_power", Value::LReal(8.0)),
            ("sum", Value::Real(3.75)),
            ("scaled", Value::Time(Duration::from_millis(15))),
            ("command", Value::Bool(true)),
        ] {
            assert_eq!(
                state.storage().get_instance_var(*plant, name),
                Some(&expected),
                "{name} at {ms}"
            );
        }
    }
}
