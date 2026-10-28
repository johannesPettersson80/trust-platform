#![allow(clippy::float_cmp)] // The margins are compared as values read back unchanged, and the finite ones with tolerances.

use std::f64::consts::PI;

use super::*;

fn margins(gain: MarginValue, phase: MarginValue, crossover: Option<f64>) -> CandidateMargins {
    CandidateMargins {
        gain_margin_db: gain,
        phase_margin_degrees: phase,
        gain_crossover_rad_per_second: crossover,
        phase_crossover_rad_per_second: None,
        peak_sensitivity: 10f64.powf(3.0 / 20.0),
        peak_sensitivity_rad_per_second: 1.0,
        unavailable: None,
    }
}

#[test]
fn the_delay_margin_is_the_phase_margin_in_radians_over_the_gain_crossover() {
    // identify-tune and candidate-robustness: a phase margin of 60° over a crossover of 0.5 rad/s gives
    // (60·π/180)/0.5 seconds (N7.7), and the peak sensitivity is the largest value of the sweep, 10^(3/20).
    let robustness = candidate_robustness(&margins(
        MarginValue::Finite(12.0),
        MarginValue::Finite(60.0),
        Some(0.5),
    ));
    assert_eq!(robustness.gain_margin_db, MarginValue::Finite(12.0));
    assert_eq!(robustness.phase_margin_degrees, MarginValue::Finite(60.0));
    match robustness.delay_margin_seconds {
        MarginValue::Finite(delay) => {
            assert!(
                (delay - (60.0 * PI / 180.0) / 0.5).abs() <= 1e-12,
                "{delay}"
            );
        }
        other => panic!("the delay margin is finite, not {other:?}"),
    }
    assert!((robustness.peak_sensitivity - 10f64.powf(3.0 / 20.0)).abs() <= 1e-12);
}

#[test]
fn a_loop_without_a_gain_crossover_has_no_phase_or_delay_margin() {
    // candidate-robustness: a loop whose gain stays below one has no crossover, so no phase or delay limit.
    let robustness = candidate_robustness(&margins(
        MarginValue::Unbounded,
        MarginValue::Unbounded,
        None,
    ));
    assert_eq!(robustness.gain_margin_db, MarginValue::Unbounded);
    assert_eq!(robustness.phase_margin_degrees, MarginValue::Unbounded);
    assert_eq!(robustness.delay_margin_seconds, MarginValue::Unbounded);
}

#[test]
fn a_margin_that_the_grid_does_not_show_is_never_reported_as_infinite() {
    let robustness = candidate_robustness(&margins(
        MarginValue::NotObservedInRange,
        MarginValue::NotObservedInRange,
        None,
    ));
    assert_eq!(
        robustness.delay_margin_seconds,
        MarginValue::NotObservedInRange
    );
}

#[test]
fn the_reason_for_unavailable_margins_is_carried_to_the_robustness() {
    let mut unavailable = margins(
        MarginValue::NotObservedInRange,
        MarginValue::NotObservedInRange,
        None,
    );
    unavailable.unavailable = Some(
        "The loop gain has magnitude one at every frequency, so its crossovers are undefined."
            .to_string(),
    );
    let robustness = candidate_robustness(&unavailable);
    assert_eq!(
        robustness.unavailable.as_deref(),
        Some(
            "The loop gain has magnitude one at every frequency, so its crossovers are undefined."
        )
    );
}
