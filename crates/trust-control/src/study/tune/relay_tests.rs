//! Tests of the relay rule's design (numerical annex N5.R and N6.4).

use super::*;
use crate::failure::{Cause, NextAction};
use crate::tuning::candidate::CandidateSource;
use crate::tuning::structure::Structure;

/// A qualified record of `Ku = 2` and `Pu = 4` s: `Kp = 0.45·Ku = 0.9`, `Ti = Pu/1.2`, and so `Ki = 0.9/(4/1.2) = 0.27`.
fn qualified() -> RelayCycleEvidence {
    RelayCycleEvidence::Qualified {
        cycles: 4,
        output_amplitude: 1.0,
        response_amplitude: 1.0,
        period_seconds: 4.0,
        ultimate_gain: 2.0,
        proportional_gain: 0.9,
        integral_time_seconds: 4.0 / 1.2,
        integral_gain: 0.27,
    }
}

#[test]
fn the_relay_rule_gives_the_pi_the_sign_of_the_controller_action() {
    let direct = design(
        Some(&qualified()),
        ControllerAction::Direct,
        TuningObjective::Robust,
    )
    .expect("a qualified record designs a PI");
    assert_eq!(direct.source, CandidateSource::RelayRule);
    assert_eq!(direct.structure, Structure::PI);
    assert!((direct.gains.proportional_gain - 0.9).abs() <= 1e-12);
    assert!((direct.gains.integral_gain - 0.27).abs() <= 1e-12);
    assert!(direct.gains.derivative_gain.abs() <= 1e-12);
    assert_eq!(
        direct.reason,
        "Relay rule from Ku = 2 and Pu = 4 s: Kp = 0.45·Ku, Ti = Pu/1.2. Validate before applying."
    );
    assert!(direct
        .description
        .as_deref()
        .is_some_and(|text| text.contains("Ku") && text.contains("Pu")));

    let reverse = design(
        Some(&qualified()),
        ControllerAction::Reverse,
        TuningObjective::Robust,
    )
    .expect("a qualified record designs a PI");
    assert_eq!(reverse.structure, Structure::PI);
    assert!((reverse.gains.proportional_gain + 0.9).abs() <= 1e-12);
    assert!((reverse.gains.integral_gain + 0.27).abs() <= 1e-12);
}

#[test]
fn a_missing_or_unqualified_record_is_refused_with_its_reason_and_a_method_choice() {
    let missing = design(None, ControllerAction::Direct, TuningObjective::Robust)
        .expect_err("no relay record");
    assert_eq!(missing.message, texts::RELAY_NOT_IDENTIFIED);
    assert_eq!(missing.cause, Cause::ModelOrData);
    assert_eq!(missing.next_action, NextAction::ChooseMethod);

    let reason = "At least three complete feedback relay cycles are required.";
    let unqualified = RelayCycleEvidence::NotQualified {
        reason: reason.to_owned(),
    };
    let refused = design(
        Some(&unqualified),
        ControllerAction::Direct,
        TuningObjective::Robust,
    )
    .expect_err("the record does not qualify");
    assert_eq!(refused.message, reason);
    assert_eq!(
        refusal_of(Some(&unqualified)).map(|failure| failure.message),
        Some(reason.to_owned())
    );
    assert!(refusal_of(Some(&qualified())).is_none());
}
