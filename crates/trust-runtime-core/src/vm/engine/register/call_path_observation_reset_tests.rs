use super::*;
use crate::value::DateTimeValue;
use crate::vm::{
    register_ir::RegisterExecutionConfiguration, ExecutionServices, PreparationLimits,
    PreparedModule,
};

/// Platform services with register profiling enabled.
struct Profiled;

impl ExecutionServices for Profiled {
    fn register_configuration(&self) -> RegisterExecutionConfiguration {
        RegisterExecutionConfiguration {
            profile_enabled: true,
            ..Default::default()
        }
    }
    fn deadline_exceeded(&self) -> bool {
        false
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
}

const FIXTURE: &[u8] = include_bytes!(
    "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
);

/// Spec 34 §4.3 "preparation observations and resets": a register profiling reset clears instrumentation only. It
/// keeps the observation facility enabled and allocates or releases no observation storage.
#[test]
fn call_path_profile_reset_allocates_and_releases_no_observation_storage() {
    let prepared = PreparedModule::from_bytes(FIXTURE, PreparationLimits::default()).unwrap();
    let services = Profiled;
    let mut data = EngineState::construct_data(&prepared, 0, &services).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, &services);
    let before = state.data.resources.construction_bytes.get();
    assert!(state.data.resources.register.profile.is_enabled());
    state.data.resources.register.reset_profile();
    assert_eq!(state.data.resources.construction_bytes.get(), before);
    assert!(
        state.data.resources.register.profile.is_enabled(),
        "the reset keeps the observation facility enabled"
    );
}
