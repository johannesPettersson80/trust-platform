#![allow(clippy::float_cmp)] // The tests compare exact values on purpose.

use super::*;

fn plant() -> Transfer {
    Transfer {
        numerator: vec![1.0],
        denominator: vec![1.0, 1.0, 0.0],
        delay_seconds: 0.0,
    }
}

#[test]
fn a_lead_adds_the_requested_phase_at_the_crossover_with_unit_loop_gain() {
    // Ported from control-studio-numerics.test.ts: lead at ω = 1 rad/s adding 30°.
    let design = design_lead(&plant(), 1.0, 30.0).expect("a lead");
    assert!((design.phase_contribution_degrees - 30.0).abs() <= 1e-9);
    assert!((design.open_loop_magnitude_at_crossover - 1.0).abs() <= 1e-9);
    assert!(design.zero_rad_per_second < design.pole_rad_per_second);
    assert_eq!(design.low_frequency_gain_ratio, None);
}

#[test]
fn a_lag_raises_the_low_frequency_gain_by_the_ratio_and_keeps_the_crossover_gain() {
    // Ported from control-studio-numerics.test.ts: lag at ω = 1 rad/s with a ratio of 5.
    let design = design_lag(&plant(), 1.0, 5.0).expect("a lag");
    assert!(design.pole_rad_per_second < design.zero_rad_per_second);
    assert!((design.open_loop_magnitude_at_crossover - 1.0).abs() <= 1e-9);
    assert!(design.low_frequency_gain_ratio.expect("the ratio") > 4.9);
    assert_eq!(design.low_frequency_gain_ratio, Some(5.0));
}

#[test]
fn the_lead_and_lag_are_refused_outside_their_ranges() {
    assert_eq!(
        design_lead(&plant(), 1.0, 85.0),
        Err(ClassicalError::InvalidInput)
    );
    assert_eq!(
        design_lead(&plant(), 1.0, 0.0),
        Err(ClassicalError::InvalidInput)
    );
    assert_eq!(
        design_lead(&plant(), 0.0, 30.0),
        Err(ClassicalError::InvalidInput)
    );
    assert_eq!(
        design_lag(&plant(), 1.0, 1.0),
        Err(ClassicalError::InvalidInput)
    );
    let zero_plant = Transfer {
        numerator: vec![0.0],
        denominator: vec![1.0, 1.0],
        delay_seconds: 0.0,
    };
    assert_eq!(
        design_lag(&zero_plant, 1.0, 2.0),
        Err(ClassicalError::InvalidPolynomial)
    );
}

#[test]
fn the_mode_dispatches_to_its_own_design() {
    let lead = design_lead_lag(&plant(), LeadLagMode::Lead, 1.0, 30.0).expect("a lead");
    let direct = design_lead(&plant(), 1.0, 30.0).expect("a lead");
    assert_eq!(lead, direct);
    let lag = design_lead_lag(&plant(), LeadLagMode::Lag, 1.0, 5.0).expect("a lag");
    assert_eq!(lag.low_frequency_gain_ratio, Some(5.0));
}
