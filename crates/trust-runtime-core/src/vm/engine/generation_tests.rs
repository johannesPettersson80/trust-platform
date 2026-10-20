//! Spec 34 §11.3: the installation generation never wraps. At the last generation a restart or
//! a replacement is refused with `Overflow` before anything changes, and the installed
//! generation's references keep working.
use super::*;
use crate::memory::view::StoreAccess;
use crate::retain::RestartMode;

const GPIO: &[u8] =
    include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");

fn prepared() -> PreparedModule {
    PreparedModule::from_bytes(GPIO, crate::vm::PreparationLimits::default()).unwrap()
}

#[test]
fn a_restart_at_the_last_generation_is_refused_and_changes_nothing() {
    let prepared = prepared();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    // Reaching the limit otherwise takes 2^32 - 1 restarts of one installation.
    data.set_generation_for_test(Generation(u32::MAX - 1));
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    state.restart(RestartMode::Warm).unwrap();
    assert_eq!(StoreAccess::generation(&state), Generation(u32::MAX));

    let held = StoreAccess::ref_for_global(&state, "probe_depth").unwrap();
    let globals = super::test_store::projection(&state).globals().clone();
    for mode in [RestartMode::Warm, RestartMode::Cold] {
        assert_eq!(state.restart(mode), Err(RuntimeError::Overflow));
        assert_eq!(StoreAccess::generation(&state), Generation(u32::MAX));
        assert_eq!(super::test_store::projection(&state).globals(), &globals);
        assert_eq!(super::test_store::read(&state, &held), Some(Value::Int(1)));
        assert!(state.data.fault.is_none());
    }
    state.execute_cycle(Duration::from_millis(20)).unwrap();
    assert!(state.data.fault.is_none());
}

#[cfg(feature = "std")]
#[test]
fn a_replacement_at_the_last_generation_is_refused_and_changes_nothing() {
    use alloc::sync::Arc;
    let prepared = Arc::new(prepared());
    let mut state = Arc::clone(&prepared).instantiate_owned(0).unwrap();
    state.data.set_generation_for_test(Generation(u32::MAX));
    let held = state.storage().ref_for_global("probe_depth").unwrap();
    let globals = state.storage().globals().clone();
    let result =
        state.prepare_replacement(Arc::clone(&prepared), 0, services::LogicalExecutionServices);
    assert!(matches!(result, Err(RuntimeError::Overflow)));
    assert_eq!(state.storage().generation(), Generation(u32::MAX));
    assert_eq!(state.storage().globals(), &globals);
    assert_eq!(state.read_reference(&held), Ok(Value::Int(1)));
    assert!(state.fault().is_none());
}
