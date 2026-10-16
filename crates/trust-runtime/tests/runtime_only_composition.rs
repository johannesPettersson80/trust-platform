//! BUILD-07/HOST-07: consume saved artifacts without compiler/dev product edges.
#![cfg(not(feature = "full"))]

use std::{
    path::Path,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use trust_runtime::{
    error::RuntimeError,
    io::IoAddress,
    runtime_core::vm::PreparationLimits,
    stdlib::StandardLibrary,
    value::{Duration, Value},
    Runtime,
};

const PROGRAM: &[u8] = include_bytes!("fixtures/portability/stbc-2.0/program-v2.stbc");
const NUMERIC: &[u8] = include_bytes!("fixtures/portability/stbc-2.0/numeric-v2.stbc");
const GPIO: &[u8] = include_bytes!("fixtures/portability/f401/gpio.stbc");

fn member(runtime: &Runtime, root: &str, name: &str) -> Value {
    let Some(Value::Instance(instance)) = runtime.storage().get_global(root) else {
        panic!("admitted root")
    };
    runtime
        .storage()
        .get_instance_var(*instance, name)
        .unwrap()
        .clone()
}

#[test]
fn saved_application_keeps_scheduling_references_and_checked_writes_without_authoring() {
    let mut runtime = Runtime::from_bytes(PROGRAM, PreparationLimits::default()).unwrap();
    for ms in (0..=1000).step_by(10) {
        runtime.set_current_time(Duration::from_millis(ms));
        runtime.execute_cycle().unwrap();
    }
    assert_eq!(member(&runtime, "Plant", "activations"), Value::DInt(40));
    assert_eq!(
        runtime.task_states()[0].last_run,
        Duration::from_millis(1000)
    );
    assert_eq!(runtime.task_states()[0].overrun_count, 0);
    assert!(!runtime.faulted());
    let mut gpio = Runtime::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    gpio.write_global("probe_depth", Value::Int(3)).unwrap();
    let reference = gpio.storage().ref_for_global("probe_depth").unwrap();
    assert_eq!(
        gpio.write_reference(&reference, Value::Bool(false)),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(gpio.read_reference(&reference).unwrap(), Value::Int(3));
    gpio.write_io(&IoAddress::parse("%IX0.0").unwrap(), Value::Bool(true))
        .unwrap();
    gpio.advance_time(Duration::from_millis(10));
    gpio.execute_cycle().unwrap();
    assert_eq!(member(&gpio, "Plant", "depth_result"), Value::Int(3));
    assert_eq!(gpio.outputs()[0] & 1, 1);
    assert!(!gpio.faulted());
}

#[test]
fn runtime_only_keeps_float_text_and_all_numeric_function_families() {
    let mut runtime = Runtime::from_bytes(NUMERIC, PreparationLimits::default()).unwrap();
    #[cfg(feature = "register-ir")]
    {
        runtime.set_vm_register_profile_enabled(true);
        runtime.set_vm_tier1_specialized_executor_enabled(true);
    }
    let cases = [
        ("v_sqrt", core::f64::consts::SQRT_2),
        ("v_ln", core::f64::consts::LN_2),
        ("v_log", 1.0),
        ("v_exp", core::f64::consts::E),
        ("v_sin", 0.5f64.sin()),
        ("v_cos", 0.5f64.cos()),
        ("v_tan", 0.5f64.tan()),
        ("v_asin", core::f64::consts::FRAC_PI_6),
        ("v_acos", core::f64::consts::FRAC_PI_3),
        ("v_atan", core::f64::consts::FRAC_PI_4),
    ];
    for ms in (0..=1000).step_by(10) {
        runtime.set_current_time(Duration::from_millis(ms));
        runtime.execute_cycle().unwrap();
        if ms < 30 {
            continue;
        }
        for (name, expected) in cases {
            let Value::LReal(actual) = member(&runtime, "Plant", name) else {
                panic!("LREAL {name}")
            };
            assert!(
                (actual - expected).abs() <= 8.0 * f64::EPSILON * expected.abs().max(1.0),
                "{name}"
            );
        }
        assert_eq!(member(&runtime, "Plant", "exact_root"), Value::Real(3.0));
        assert_eq!(member(&runtime, "Plant", "exact_power"), Value::LReal(8.0));
        assert_eq!(member(&runtime, "Plant", "sum"), Value::Real(3.75));
        assert_eq!(
            member(&runtime, "Plant", "scaled"),
            Value::Time(Duration::from_millis(15))
        );
        assert_eq!(member(&runtime, "Plant", "command"), Value::Bool(true));
    }
    assert_eq!(member(&runtime, "Plant", "activations"), Value::DInt(40));
    let library = StandardLibrary::new();
    assert_eq!(
        library
            .call("STRING_TO_REAL", &[Value::String("1.5".into())])
            .unwrap(),
        Value::Real(1.5)
    );
    assert_eq!(
        library.call("REAL_TO_STRING", &[Value::Real(1.5)]).unwrap(),
        Value::String("1.5".into())
    );
    assert_eq!(
        library
            .call("WSTRING_TO_LREAL", &[Value::WString("1.5".into())])
            .unwrap(),
        Value::LReal(1.5)
    );
    assert_eq!(
        library
            .call("LREAL_TO_WSTRING", &[Value::LReal(1.5)])
            .unwrap(),
        Value::WString("1.5".into())
    );
    for name in [
        "ABS",
        "MIN",
        "MAX",
        "LIMIT",
        "MUX",
        "LEN",
        "LEFT",
        "RIGHT",
        "MID",
        "CONCAT",
        "ASSERT_TRUE",
        "ASSERT_EQUAL",
    ] {
        assert!(
            library.get(name).is_some(),
            "full shared descriptor set: {name}"
        );
    }
    #[cfg(feature = "register-ir")]
    {
        // EXPT (0x4C) is an inherited whole-POU register fallback. The full
        // numeric contract above still executes through the common stack path.
        let profile = runtime.vm_register_profile_snapshot();
        assert_eq!(profile.register_programs_executed, 0);
        assert_eq!(profile.register_program_fallbacks, 40);
        assert!(profile
            .fallback_reasons
            .iter()
            .any(|reason| reason.reason == "unsupported_opcode_0x4C" && reason.count == 40));
    }
}

#[test]
fn saved_clock_artifact_uses_host_utc_and_logical_time_without_hir() {
    // The existing fixture producer generates this in the consolidated batch,
    // before the isolated runtime-only consumer. It never runs in this graph.
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/portability/stbc-2.0/clock-v2.stbc");
    let bytes = std::fs::read(path).expect("clock fixture produced before isolated consumer");
    let mut runtime = Runtime::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    #[cfg(feature = "register-ir")]
    {
        runtime.set_vm_register_profile_enabled(true);
        runtime.set_vm_tier1_specialized_executor_enabled(true);
    }
    runtime.set_current_time(Duration::from_millis(37));
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    runtime.execute_cycle().unwrap();
    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let Value::Dt(now) = member(&runtime, "Clock", "now") else {
        panic!("DT")
    };
    assert!((before..=after).contains(&(now.ticks() as u128)));
    assert_eq!(
        member(&runtime, "Clock", "elapsed"),
        Value::Time(Duration::from_millis(37))
    );
    #[cfg(feature = "register-ir")]
    {
        let profile = runtime.vm_register_profile_snapshot();
        assert_eq!(
            profile.register_programs_executed, 1,
            "eligible saved clock artifact must execute the shared register plan: {profile:?}"
        );
        assert_eq!(profile.register_program_fallbacks, 0);
        assert!(profile.fallback_reasons.is_empty());
    }
}

#[test]
fn physical_deadline_fault_is_shared_and_cannot_be_reset_by_a_value_write() {
    let mut runtime = Runtime::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    runtime.set_execution_deadline(Some(Instant::now()));
    assert_eq!(
        runtime.write_global("probe_depth", Value::Int(8)),
        Err(RuntimeError::ExecutionTimeout)
    );
    assert_eq!(
        runtime.storage().get_global("probe_depth"),
        Some(&Value::Int(1))
    );
    assert!(
        !runtime.faulted(),
        "owner request rejection is not a PLC fault"
    );
    runtime.advance_time(Duration::from_millis(10));
    assert_eq!(runtime.execute_cycle(), Err(RuntimeError::ExecutionTimeout));
    assert!(runtime.faulted());
    runtime.set_execution_deadline(None);
    runtime.clear_fault();
    runtime.execute_cycle().unwrap();
    assert!(!runtime.faulted());
}
