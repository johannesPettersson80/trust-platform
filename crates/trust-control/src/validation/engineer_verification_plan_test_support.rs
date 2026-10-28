//! The fixtures that the verification plan tests and the task record tests share (specification 35, B4.3): the whole message of a failure
//! and the task of the `TypeScript` tests' `BASE`. Declared under `cfg(test)` by `engineer_verification_plan.rs`.

use super::EngineerRequirements;
use crate::failure::Failure;

/// The message of a failure, whole: its detail when the sentence has more than one sentence.
pub(super) fn text(failure: &Failure) -> String {
    failure
        .detail
        .clone()
        .unwrap_or_else(|| failure.message.clone())
}

/// The task of the `TypeScript` tests' `BASE`: a 1-unit change, a 0.2 error limit, a 0.1 settling band, a 15 s response
/// and a 30 s run.
pub(super) fn base_task() -> EngineerRequirements {
    EngineerRequirements {
        channel_requirements: None,
        reference_change: 1.0,
        maximum_error: 0.2,
        settling_band: 0.1,
        settling_time_seconds: 15.0,
        duration_seconds: 30.0,
        constant_input_disturbance: None,
        measurement_noise_band: None,
        gain_uncertainty_fraction: None,
        transfer_tolerance: None,
        maximum_overshoot_percent: None,
        tuning_method: None,
        tuning_objective: None,
        automatic_criteria: None,
    }
}
