//! Full/development services wrap the same saved-artifact execution and library.
#![cfg(feature = "full")]

use trust_runtime::{
    runtime_core::{
        stdlib::StandardLibrary as PortableLibrary,
        vm::{PreparationLimits, PreparedModule},
    },
    stdlib::StandardLibrary,
    value::{Duration, Value},
    Runtime,
};

const NUMERIC: &[u8] = include_bytes!("fixtures/portability/stbc-2.0/numeric-v2.stbc");

#[test]
fn full_controller_preserves_saved_numeric_state_and_library_identity_with_the_core() {
    let prepared = PreparedModule::from_bytes(NUMERIC, PreparationLimits::default()).unwrap();
    let mut core = prepared.instantiate(0).unwrap();
    let mut full = Runtime::from_bytes(NUMERIC, PreparationLimits::default()).unwrap();
    for ms in (0..=1000).step_by(10) {
        let time = Duration::from_millis(ms);
        core.execute_cycle(time).unwrap();
        full.set_current_time(time);
        full.execute_cycle().unwrap();
        let Some(Value::Instance(core_root)) = core.storage().get_global("Plant") else {
            panic!("portable root")
        };
        let Some(Value::Instance(full_root)) = full.storage().get_global("Plant") else {
            panic!("hosted root")
        };
        assert_eq!(
            core.storage().get_instance(*core_root).unwrap().variables,
            full.storage().get_instance(*full_root).unwrap().variables
        );
        assert_eq!(core.outputs(), full.io().outputs());
        assert_eq!(
            core.task_states()[0].last_run,
            Duration::from_millis(ms - ms % 25)
        );
    }
    // Both compositions preserve even the footprint-heavy text conversions.
    let portable = PortableLibrary::new();
    let hosted = StandardLibrary::new();
    for (name, values) in [
        ("ABS", vec![Value::Int(-7)]),
        ("SQRT", vec![Value::LReal(2.0)]),
        ("STRING_TO_REAL", vec![Value::String("1.5".into())]),
        ("REAL_TO_STRING", vec![Value::Real(1.5)]),
        ("WSTRING_TO_LREAL", vec![Value::WString("1.5".into())]),
        ("LREAL_TO_WSTRING", vec![Value::LReal(1.5)]),
        ("ASSERT_TRUE", vec![Value::Bool(true)]),
    ] {
        assert_eq!(
            portable.call(name, &values),
            hosted.call(name, &values),
            "{name}"
        );
    }
}
