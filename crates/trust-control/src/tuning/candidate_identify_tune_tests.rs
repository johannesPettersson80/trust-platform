// Ported from editors/vscode/src/test/suite/control-studio-identify-tune.test.ts: a complete `Control.PID` record keeps its
// values, its structure follows the nonzero integral and derivative gains, and a formula result is an unapplied candidate.

use super::*;
use crate::tuning::objective::{TuningMethod, TuningObjective};

/// The record of the `TypeScript` test: a proportional gain of 2, an integral gain of 0.5, a derivative gain of 0.1, and
/// the limits and the tracking time of that test.
fn record() -> ControlPidParameters {
    ControlPidParameters {
        proportional_gain: 2.0,
        integral_gain: 0.5,
        derivative_gain: 0.1,
        proportional_setpoint_weight: 0.5,
        derivative_setpoint_weight: 0.0,
        bias: 1.0,
        output_minimum: -10.0,
        output_maximum: 10.0,
        rate_minimum: -20.0,
        rate_maximum: 20.0,
        tracking_time: 1.0,
        anti_windup: AntiWindup::BackCalculation,
    }
}

// control-studio-identify-tune: "preserves manual and imported Control.PID settings without conversion"
#[test]
fn manual_and_imported_records_keep_every_value_and_the_structure_follows_the_nonzero_terms() {
    let parameters = record();
    let manual = candidate_from_control_pid(parameters, CandidateSource::Manual)
        .expect("the record is valid");
    let imported = candidate_from_control_pid(parameters, CandidateSource::Imported)
        .expect("the record is valid");
    assert_eq!(manual.structure, Structure::PID);
    assert!(!manual.applied);
    assert_eq!(manual.parameters, parameters);
    assert_eq!(imported.parameters, parameters);

    let without_integral = ControlPidParameters {
        integral_gain: 0.0,
        ..parameters
    };
    let derivative = candidate_from_control_pid(without_integral, CandidateSource::Manual)
        .expect("the record is valid");
    assert_eq!(derivative.structure, Structure::PD);

    let without_derivative = ControlPidParameters {
        derivative_gain: 0.0,
        ..parameters
    };
    let integral = candidate_from_control_pid(without_derivative, CandidateSource::Manual)
        .expect("the record is valid");
    assert_eq!(integral.structure, Structure::PI);
}

// control-studio-identify-tune: "preserves manual and imported Control.PID settings without conversion"
#[test]
fn a_tracking_time_of_zero_is_refused_in_the_record_sentence() {
    let refusal = candidate_from_control_pid(
        ControlPidParameters {
            tracking_time: 0.0,
            ..record()
        },
        CandidateSource::Manual,
    )
    .expect_err("a tracking time of zero is refused");
    assert_eq!(refusal.0, "Control.PID tracking time must be positive.");
}

// control-studio-identify-tune: "generates unapplied SIMC and AMIGO PI candidates with model-backed margins"
// A partial port: the AMIGO values for K = T = L = 1 (Kp = 0.25, the oracle at TypeScript lines 556-558, N5.5), with
// Ki = Kp / Ti (N5.6). The SIMC candidate and the margins are in candidate_margins_identify_tune_tests.rs.
#[test]
fn a_formula_result_becomes_an_unapplied_pi_candidate_with_its_integral_gain() {
    let tuning = PiTuning {
        method: TuningMethod::Amigo,
        objective: TuningObjective::Balanced,
        proportional_gain: 0.25,
        integral_time_seconds: 1.0,
        closed_loop_time_seconds: None,
    };
    let candidate =
        generated_candidate(&tuning, CandidateLimits::default()).expect("the result is valid");
    assert!(!candidate.applied);
    assert_eq!(candidate.structure, Structure::PI);
    assert!(
        (candidate.parameters.integral_gain - 0.25).abs() <= 1e-12,
        "{}",
        candidate.parameters.integral_gain
    );
}
