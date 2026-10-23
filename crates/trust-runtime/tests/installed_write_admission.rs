#![cfg(feature = "full")]
//! Installed-state admission: a refused artifact never yields an executable controller.

use trust_runtime::{
    error::{RuntimeError, StableErrorCode},
    runtime_core::vm::PreparationLimits,
    Runtime,
};

const GPIO: &[u8] = include_bytes!("fixtures/portability/f401/gpio.stbc");

/// Spec 34 §4.3.1 HOST-09 (non-executable construction): installation transfers ownership only after admission, so a module with invalid magic produces no controller.
#[test]
fn installed_write_bad_magic_produces_no_installed_controller() {
    let mut bytes = GPIO.to_vec();
    bytes[0] = 0x00;
    let error = match Runtime::from_bytes(&bytes, Default::default()) {
        Ok(_) => panic!("a module with invalid magic must not construct a controller"),
        Err(error) => error,
    };
    assert_eq!(error.stable_code(), StableErrorCode::BytecodeInvalidMagic);
}

/// Spec 34 §4.3.1 HOST-09 (non-executable construction): an artifact larger than the admission limit produces no controller.
#[test]
fn installed_write_oversized_artifact_produces_no_installed_controller() {
    let limits = PreparationLimits {
        max_artifact_bytes: GPIO.len() - 1,
        ..Default::default()
    };
    let error = match Runtime::from_bytes(GPIO, limits) {
        Ok(_) => panic!("an artifact over the admission limit must not construct a controller"),
        Err(error) => error,
    };
    assert_eq!(error, RuntimeError::PreparationLimit);
}

/// Spec 34 §4.3.1 HOST-09 (code and layout row): an isolated candidate is built outside the bounded RUN budget, so a live scan limit of one unit does not refuse candidate construction.
#[test]
fn installed_write_candidate_construction_is_not_charged_to_the_live_scan_budget() {
    let limits = PreparationLimits {
        max_work: 1,
        ..Default::default()
    };
    let mut runtime = Runtime::from_bytes(GPIO, limits).unwrap();
    runtime
        .apply_bytecode_bytes(GPIO, None)
        .expect("candidate construction runs outside the bounded RUN budget");
    assert_eq!(runtime.prepared().limits().max_work, 1);
    assert!(!runtime.faulted());
}
