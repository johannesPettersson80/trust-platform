#![allow(clippy::float_cmp)] // The values are the gains themselves, not computed sums.

use std::collections::BTreeMap;

use super::*;

fn seed(family: Structure, integral: bool, derivative: bool) -> ParallelPidSeed {
    ParallelPidSeed {
        family,
        proportional_symbol: "Main.Kp".to_owned(),
        integral_symbol: integral.then(|| "Main.Ki".to_owned()),
        derivative_symbol: derivative.then(|| "Main.Kd".to_owned()),
    }
}

fn gains(proportional: f64, integral: f64, derivative: f64) -> ParallelGains {
    ParallelGains {
        proportional_gain: proportional,
        integral_gain: integral,
        derivative_gain: derivative,
    }
}

#[test]
fn a_pid_design_writes_each_gain_to_the_variable_the_seed_names() {
    let candidate = project_candidate(
        &seed(Structure::PID, true, true),
        CandidateSource::Simc,
        gains(2.0, 0.5, 0.25),
    );
    assert!(!candidate.applied);
    assert_eq!(candidate.structure, Structure::PID);
    assert_eq!(
        candidate.values,
        BTreeMap::from([
            ("Main.Kp".to_owned(), 2.0),
            ("Main.Ki".to_owned(), 0.5),
            ("Main.Kd".to_owned(), 0.25),
        ])
    );
}

#[test]
fn a_proportional_design_on_a_pid_seed_holds_only_the_proportional_variable() {
    // Apply still writes an explicit zero to the integral and derivative variables (C2); the candidate's values do not hold them.
    let candidate = project_candidate(
        &seed(Structure::PID, true, true),
        CandidateSource::ConstrainedComparison,
        gains(1.5, 0.0, 0.0),
    );
    assert_eq!(candidate.structure, Structure::P);
    assert_eq!(candidate.gains, gains(1.5, 0.0, 0.0));
    assert_eq!(
        candidate.values,
        BTreeMap::from([("Main.Kp".to_owned(), 1.5)])
    );
}

#[test]
fn a_pi_design_on_a_pid_seed_holds_the_proportional_and_integral_variables_only() {
    let candidate = project_candidate(
        &seed(Structure::PID, true, true),
        CandidateSource::Simc,
        gains(2.0, 0.5, 0.0),
    );
    assert_eq!(candidate.structure, Structure::PI);
    assert_eq!(
        candidate.values,
        BTreeMap::from([("Main.Kp".to_owned(), 2.0), ("Main.Ki".to_owned(), 0.5)])
    );
}

#[test]
fn a_proportional_seed_writes_only_its_proportional_variable() {
    let candidate = project_candidate(
        &seed(Structure::P, false, false),
        CandidateSource::ConstrainedComparison,
        gains(3.0, 0.0, 0.0),
    );
    assert_eq!(candidate.structure, Structure::P);
    assert_eq!(
        candidate.values,
        BTreeMap::from([("Main.Kp".to_owned(), 3.0)])
    );
}

/// The trajectory seed of a PID law: its PID variables, and the variables of its rate and acceleration gains (C11).
fn trajectory(family: Structure) -> TrajectoryFeedforwardSeed {
    TrajectoryFeedforwardSeed {
        pid: seed(family, true, true),
        rate_symbol: "Main.Kv".to_owned(),
        acceleration_symbol: "Main.Ka".to_owned(),
    }
}

#[test]
fn a_trajectory_candidate_holds_the_feedforward_gains_beside_the_pid_gains() {
    let candidate = trajectory_candidate(
        &trajectory(Structure::PID),
        CandidateSource::Simc,
        gains(2.0, 0.5, 0.25),
        (1.0, 0.75),
    );
    assert_eq!(candidate.structure, Structure::PID);
    assert_eq!(
        candidate.values,
        BTreeMap::from([
            ("Main.Kp".to_owned(), 2.0),
            ("Main.Ki".to_owned(), 0.5),
            ("Main.Kd".to_owned(), 0.25),
            ("Main.Kv".to_owned(), 1.0),
            ("Main.Ka".to_owned(), 0.75),
        ])
    );
    assert!(candidate.is_finite());
}

#[test]
fn a_proportional_trajectory_candidate_holds_its_gain_and_both_feedforward_gains() {
    // The PID variables of a P design are its proportional variable only; the feedforward gains are the trajectory's own.
    let candidate = trajectory_candidate(
        &trajectory(Structure::P),
        CandidateSource::ConstrainedComparison,
        gains(3.0, 0.0, 0.0),
        (1.0, 0.0),
    );
    assert_eq!(candidate.structure, Structure::P);
    assert_eq!(
        candidate.values,
        BTreeMap::from([
            ("Main.Kp".to_owned(), 3.0),
            ("Main.Kv".to_owned(), 1.0),
            ("Main.Ka".to_owned(), 0.0),
        ])
    );
}
