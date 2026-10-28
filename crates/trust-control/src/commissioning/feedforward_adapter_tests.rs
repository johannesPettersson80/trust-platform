//! The values that Apply writes for a measured-disturbance feedforward (specification 35, C8): the PI's three gains with the derivative
//! written as zero, the feedforward gain, and the lead and lag times when the program has both, each to its own variable, through the
//! project adapter. A candidate that lacks one of them is refused, and a law without a feedforward holds no feedforward value.

#![allow(clippy::float_cmp)] // The values are the literals the gains carry, compared exactly on purpose.

use std::collections::BTreeMap;

use super::*;

/// Lesson 84 as the shipped file declares it: the heater whose draw-off is fed forward through a lead and a lag.
fn lesson_84() -> LoopDefinition {
    LoopDefinition::decode(include_str!(
        "../../../../editors/vscode/media/examples/control/14-integrated-capstones/84-independent/control-loop.json"
    ))
    .expect("lesson 84 decodes")
}

/// Lesson 90 as the shipped file declares it: a program's PID with no feedforward variable.
fn lesson_90() -> LoopDefinition {
    LoopDefinition::decode(include_str!(
        "../../../../editors/vscode/media/examples/control/09-industrial-control-structures/90-trajectory-feedforward/control-loop.json"
    ))
    .expect("lesson 90 decodes")
}

/// The whole sentence a refusal shows: its detail when it says more than its first sentence.
fn full_text(failure: &Failure) -> String {
    failure
        .detail
        .clone()
        .unwrap_or_else(|| failure.message.clone())
}

/// The gains of a PI candidate with its feedforward: the gain, and the lead and lag when given.
fn gains(feedforward: Option<f64>, lead_lag: Option<(f64, f64)>) -> ControllerGains {
    ControllerGains {
        proportional_gain: Some(2.5),
        integral_gain: Some(0.5),
        derivative_gain: Some(0.0),
        feedforward_gain: feedforward,
        lead_seconds: lead_lag.map(|(lead, _)| lead),
        lag_seconds: lead_lag.map(|(_, lag)| lag),
        ..ControllerGains::default()
    }
}

#[test]
fn a_feedforward_writes_its_pi_gains_and_its_gain_lead_and_lag_through_the_project_adapter() {
    let loop_definition = lesson_84();
    let adapter = controller_adapter(&loop_definition).expect("the engine serves lesson 84's law");
    assert!(
        matches!(adapter.kind(), AdapterKind::ProjectPid { .. }),
        "the project adapter applies a feedforward"
    );
    let values = adapter
        .materialize(&gains(Some(0.5), Some((8.0, 3.0))), &BTreeMap::new())
        .expect("the values are within their bounds");
    assert_eq!(
        values,
        BTreeMap::from([
            ("P1.Parameters.ProportionalGain".to_owned(), 2.5),
            ("P1.Parameters.IntegralGain".to_owned(), 0.5),
            ("P1.Parameters.DerivativeGain".to_owned(), 0.0),
            ("P1.FeedforwardGain".to_owned(), 0.5),
            ("P1.LeadS".to_owned(), 8.0),
            ("P1.LagS".to_owned(), 3.0),
        ])
    );
}

#[test]
fn a_feedforward_candidate_without_its_gain_or_with_one_of_its_lead_and_lag_is_refused() {
    let loop_definition = lesson_84();
    let adapter = controller_adapter(&loop_definition).expect("the engine serves lesson 84's law");
    let missing_gain = adapter
        .materialize(&gains(None, Some((8.0, 3.0))), &BTreeMap::new())
        .expect_err("the feedforward gain is needed");
    assert_eq!(missing_gain.cause, Cause::Settings);
    assert_eq!(missing_gain.next_action, NextAction::Tune);
    assert_eq!(
        full_text(&missing_gain),
        texts::project_feedforward_gain_required()
    );

    let lead_only = ControllerGains {
        lead_seconds: Some(8.0),
        ..gains(Some(0.5), None)
    };
    let refusal = adapter
        .materialize(&lead_only, &BTreeMap::new())
        .expect_err("the lead and the lag come together");
    assert_eq!(
        full_text(&refusal),
        texts::project_feedforward_gain_required()
    );
}

#[test]
fn a_law_without_a_feedforward_holds_no_feedforward_value_and_a_zero_one_is_neutral() {
    // Lesson 90's law is a trajectory law, so its own two feedforward gains are given; the gain of C8 has nowhere to be written.
    let loop_definition = lesson_90();
    let adapter = controller_adapter(&loop_definition).expect("the engine serves lesson 90's law");
    let trajectory = |feedforward: Option<f64>| ControllerGains {
        rate_gain: Some(1.0),
        acceleration_gain: Some(0.75),
        ..gains(feedforward, None)
    };
    let refusal = adapter
        .materialize(&trajectory(Some(0.5)), &BTreeMap::new())
        .expect_err("a law without a feedforward has nowhere to write a nonzero gain");
    assert_eq!(refusal.cause, Cause::ControllerCapability);
    let neutral = adapter
        .materialize(&trajectory(Some(0.0)), &BTreeMap::new())
        .expect("a zero feedforward gain is neutral");
    assert!(
        neutral.keys().all(|symbol| !symbol.contains("Feedforward")),
        "{neutral:?}"
    );
}
