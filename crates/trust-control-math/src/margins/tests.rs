use std::f64::consts::PI;

use super::*;
use crate::frequency::logarithmic_grid;
use crate::model::TransferFunction;

fn transfer(numerator: &[f64], denominator: &[f64]) -> TransferFunction {
    TransferFunction::new(numerator.to_vec(), denominator.to_vec()).expect("valid test polynomial")
}

fn continuous(numerator: &[f64], denominator: &[f64], delay_seconds: f64) -> FrequencyModel {
    FrequencyModel::Transfer {
        transfer: transfer(numerator, denominator),
        timing: Timing::Continuous,
        delay_seconds,
    }
}

fn sampled(numerator: &[f64], denominator: &[f64], sample_time_seconds: f64) -> FrequencyModel {
    FrequencyModel::Transfer {
        transfer: transfer(numerator, denominator),
        timing: Timing::Discrete {
            sample_time_seconds,
        },
        delay_seconds: 0.0,
    }
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual}"
    );
}

fn finite(value: MarginValue) -> f64 {
    match value {
        MarginValue::Finite(number) => number,
        other => panic!("expected a finite margin, got {other:?}"),
    }
}

fn wide_grid() -> Vec<f64> {
    logarithmic_grid(1e-3, 1e3, 1201).expect("grid")
}

#[test]
fn ten_over_s_times_s_plus_one_has_closed_form_crossover_and_phase_margin() {
    // L = 10/(s(s+1)): |L| = 1 where ω²(ω² + 1) = 100, so ωc² = (√401 − 1)/2, and ∠L = −90° − atan ω.
    // The phase margin is 90° − atan ωc, and the delay margin is that angle in radians over ωc.
    // Its phase never reaches −180°, so the gain margin is unbounded (N7.7).
    let report =
        analyze_loop(&continuous(&[10.0], &[1.0, 1.0, 0.0], 0.0), &wide_grid()).expect("margins");
    let omega_c = ((401.0_f64.sqrt() - 1.0) / 2.0).sqrt();
    let phase_margin = 90.0 - omega_c.atan().to_degrees();
    assert_close(
        report
            .gain_crossover_rad_per_second
            .expect("gain crossover"),
        omega_c,
        1e-9,
    );
    assert_close(finite(report.phase_margin_degrees), phase_margin, 1e-7);
    assert_close(
        finite(report.delay_margin_seconds),
        phase_margin.to_radians() / omega_c,
        1e-7,
    );
    assert_eq!(report.gain_margin_db, MarginValue::Unbounded);
    assert_eq!(report.phase_crossover_rad_per_second, None);
    // The test suite's published phase margin is 17.96423592° (ωc 3.0842328377 rad/s, DM 0.10165 s).
    assert_close(phase_margin, 17.964_235_92, 1e-5);
    assert_close(phase_margin.to_radians() / omega_c, 0.10165, 1e-4);
    match report.closed_loop {
        ClosedLoopStability::Stable { poles } => assert_eq!(poles.len(), 2),
        other => panic!("expected a stable loop, got {other:?}"),
    }
}

#[test]
fn one_over_s_times_s_plus_one_has_phase_margin_near_51_8_degrees() {
    // L = 1/(s(s+1)): ωc² = (√5 − 1)/2 ≈ 0.618, PM = 90° − atan ωc ≈ 51.83°. Gain margin unbounded.
    let report =
        analyze_loop(&continuous(&[1.0], &[1.0, 1.0, 0.0], 0.0), &wide_grid()).expect("margins");
    let omega_c = ((5.0_f64.sqrt() - 1.0) / 2.0).sqrt();
    let phase_margin = 90.0 - omega_c.atan().to_degrees();
    assert_close(finite(report.phase_margin_degrees), phase_margin, 1e-7);
    assert_close(phase_margin, 51.8269, 1e-3);
    assert_close(
        finite(report.delay_margin_seconds),
        phase_margin.to_radians() / omega_c,
        1e-7,
    );
    assert_eq!(report.gain_margin_db, MarginValue::Unbounded);
}

#[test]
fn integrator_with_delay_has_closed_form_margins() {
    // L = e^(−s)/s: |L| = 1/ω, so ωc = 1. ∠L = −90° − ω·1 rad, so PM = 90° − 180°/π ≈ 32.70°
    // and DM = π/2 − 1 s. The phase reaches −180° where ω = π/2, with |L| = 2/π, so GM = 20·log10(π/2) dB.
    // Next crossings at 5π/2 and beyond have larger gain margins, and the smallest is reported.
    let grid = logarithmic_grid(1e-3, 100.0, 2000).expect("grid");
    let report = analyze_loop(&continuous(&[1.0], &[1.0, 0.0], 1.0), &grid).expect("margins");
    assert_close(
        report
            .gain_crossover_rad_per_second
            .expect("gain crossover"),
        1.0,
        1e-9,
    );
    assert_close(finite(report.phase_margin_degrees), 90.0 - 180.0 / PI, 1e-6);
    assert_close(finite(report.delay_margin_seconds), PI / 2.0 - 1.0, 1e-6);
    assert_close(
        report
            .phase_crossover_rad_per_second
            .expect("phase crossover"),
        PI / 2.0,
        1e-8,
    );
    assert_close(
        finite(report.gain_margin_db),
        20.0 * (PI / 2.0).log10(),
        1e-6,
    );
    // A pure delay keeps the closed loop out of exact reach (N7.12).
    assert_eq!(
        report.closed_loop,
        ClosedLoopStability::Unavailable {
            reason: ClosedLoopUnavailable::PureDelay
        }
    );
}

#[test]
fn unstable_loop_reports_negative_margins_and_an_unstable_closed_loop() {
    // L = 10/(s + 1)³: the phase reaches −180° at ω = √3 with |L| = 10/8, so GM = −20·log10(1.25) dB,
    // which is negative. The gain crossover is ωc² = 100^(1/3) − 1. The characteristic polynomial
    // s³ + 3s² + 3s + 11 fails the Routh test (3·3 < 11), so the closed loop is unstable.
    let report = analyze_loop(
        &continuous(&[10.0], &[1.0, 3.0, 3.0, 1.0], 0.0),
        &wide_grid(),
    )
    .expect("margins");
    let omega_c = (100.0_f64.cbrt() - 1.0).sqrt();
    assert_close(
        report
            .gain_crossover_rad_per_second
            .expect("gain crossover"),
        omega_c,
        1e-9,
    );
    assert_close(
        report
            .phase_crossover_rad_per_second
            .expect("phase crossover"),
        3.0_f64.sqrt(),
        1e-8,
    );
    assert_close(
        finite(report.gain_margin_db),
        -20.0 * 1.25_f64.log10(),
        1e-8,
    );
    let phase_margin = 180.0 - 3.0 * omega_c.atan().to_degrees();
    assert!(phase_margin < 0.0);
    assert_close(finite(report.phase_margin_degrees), phase_margin, 1e-7);
    assert!(finite(report.delay_margin_seconds) < 0.0);
    match report.closed_loop {
        ClosedLoopStability::Unstable { poles } => assert_eq!(poles.len(), 3),
        other => panic!("expected an unstable loop, got {other:?}"),
    }
}

#[test]
fn a_delayed_loop_with_a_large_gain_has_a_negative_gain_margin() {
    // L = 100·e^(−5s)/(s + 1): the phase reaches −180° near ω ≈ 0.52 with |L| ≈ 89, so GM < 0 dB.
    let grid = logarithmic_grid(1e-3, 10.0, 2000).expect("grid");
    let report = analyze_loop(&continuous(&[100.0], &[1.0, 1.0], 5.0), &grid).expect("margins");
    assert!(finite(report.gain_margin_db) < 0.0);
}

#[test]
fn discrete_loop_margins_approach_the_continuous_limit_as_the_sample_time_shrinks() {
    // Continuous L = 2/(s + 1): |L| = 1 at ωc = √3, PM = 180° − 60° = 120°, and the phase never reaches
    // −180°, so its gain margin is unbounded. The zero-order-hold equivalent at Ts = 1 ms is
    // L(z) = 2(1 − a)/(z − a) with a = e^(−Ts). Its PM must be within half a degree of 120°.
    // Its gain margin is not observed in range: a sampled loop has no proof of absence over the axis.
    let sample_time: f64 = 1e-3;
    let a = (-sample_time).exp();
    let model = sampled(&[2.0 * (1.0 - a)], &[1.0, -a], sample_time);
    let grid = logarithmic_grid(1e-3, 1000.0, 4000).expect("grid");
    let report = analyze_loop(&model, &grid).expect("margins");
    assert_close(
        report
            .gain_crossover_rad_per_second
            .expect("gain crossover"),
        3.0_f64.sqrt(),
        1e-2,
    );
    assert_close(finite(report.phase_margin_degrees), 120.0, 0.5);
    assert_eq!(report.gain_margin_db, MarginValue::NotObservedInRange);
    // The closed-loop pole is z = 3a − 2, inside the unit circle for a < 1.
    match report.closed_loop {
        ClosedLoopStability::Stable { poles } => {
            assert_eq!(poles.len(), 1);
            assert_close(poles[0].real, 3.0 * a - 2.0, 1e-9);
            assert_close(poles[0].imaginary, 0.0, 1e-12);
        }
        other => panic!("expected a stable discrete loop, got {other:?}"),
    }
}

#[test]
fn state_space_loop_gives_the_same_phase_margin_but_no_unbounded_claim() {
    // The state-space realization of 1/(s² + s) has the same margins as the transfer function. A
    // state-space loop has no polynomial proof of absence, so its gain margin is not observed in range,
    // and it has no closed-loop poles from a transfer function.
    use crate::model::StateSpace;
    let system = StateSpace::new(
        vec![vec![0.0, 1.0], vec![0.0, -1.0]],
        vec![vec![0.0], vec![1.0]],
        vec![vec![1.0, 0.0]],
        vec![vec![0.0]],
    )
    .expect("valid system");
    let model = FrequencyModel::StateSpace {
        system,
        timing: Timing::Continuous,
        delay_seconds: 0.0,
    };
    let report = analyze_loop(&model, &wide_grid()).expect("margins");
    let omega_c = ((5.0_f64.sqrt() - 1.0) / 2.0).sqrt();
    assert_close(
        finite(report.phase_margin_degrees),
        90.0 - omega_c.atan().to_degrees(),
        1e-7,
    );
    assert_eq!(report.gain_margin_db, MarginValue::NotObservedInRange);
    assert_eq!(
        report.closed_loop,
        ClosedLoopStability::Unavailable {
            reason: ClosedLoopUnavailable::StateSpaceLoop
        }
    );
}

#[test]
fn a_loop_whose_gain_stays_below_one_has_unbounded_gain_phase_and_delay_margins() {
    // L = 0.5/(s + 1): |L| < 1 at every frequency and its phase is above −90°. Every margin the loop
    // does not have is unbounded. The closed-loop pole is −1.5.
    let report =
        analyze_loop(&continuous(&[0.5], &[1.0, 1.0], 0.0), &wide_grid()).expect("margins");
    assert_eq!(report.gain_crossover_rad_per_second, None);
    assert_eq!(report.phase_margin_degrees, MarginValue::Unbounded);
    assert_eq!(report.delay_margin_seconds, MarginValue::Unbounded);
    assert_eq!(report.gain_margin_db, MarginValue::Unbounded);
    match report.closed_loop {
        ClosedLoopStability::Stable { poles } => {
            assert_eq!(poles.len(), 1);
            assert_close(poles[0].real, -1.5, 1e-12);
        }
        other => panic!("expected a stable loop, got {other:?}"),
    }
}

#[test]
fn an_algebraic_loop_keeps_its_margins_and_reports_no_closed_loop_poles() {
    // L = −s/(s + 1): 1 + L(∞) = 0, so the feedback is algebraically singular (N7.11). The magnitude is
    // always below one and the phase is always above −180°, so the margins are unbounded.
    let report =
        analyze_loop(&continuous(&[-1.0, 0.0], &[1.0, 1.0], 0.0), &wide_grid()).expect("margins");
    assert_eq!(report.phase_margin_degrees, MarginValue::Unbounded);
    assert_eq!(report.gain_margin_db, MarginValue::Unbounded);
    assert_eq!(
        report.closed_loop,
        ClosedLoopStability::Unavailable {
            reason: ClosedLoopUnavailable::AlgebraicLoop
        }
    );
}

#[test]
fn invalid_grids_and_coarse_pure_delays_are_refused() {
    let model = continuous(&[1.0], &[1.0, 1.0], 0.0);
    assert_eq!(analyze_loop(&model, &[1.0]), Err(MarginError::InvalidGrid));
    assert_eq!(
        analyze_loop(&model, &[1.0, 0.5]),
        Err(MarginError::InvalidGrid)
    );
    // A delay of 10 s on a grid with spacing near 1 rad/s makes the phase advance per step exceed π/2.
    let coarse = continuous(&[1.0], &[1.0, 1.0], 10.0);
    assert_eq!(
        analyze_loop(&coarse, &[1.0, 2.0, 3.0]),
        Err(MarginError::DelayPhaseTooFast)
    );
}

#[test]
fn the_precision_drive_loop_keeps_the_test_suites_crossover_margin_and_peak_sensitivity() {
    // Ported from control-studio-numerics.test.ts, "qualifies precision-drive sensitivity and loop-shaping trade-offs" (N7.7, N7.8): the
    // plant 10/(s² + 0.6s + 1) under the gain 0.4, and under a shaped controller that adds a lead and a roll-off. The grid is the suite's,
    // 1e-3 to 100 rad/s at 400 points per decade. The suite's peak complementary sensitivity, bandwidth and input-disturbance gain are not
    // part of a margin report.
    let grid = logarithmic_grid(1e-3, 100.0, 2001).expect("the grid");
    let plant_denominator = [1.0, 0.6, 1.0];
    let baseline = analyze_loop(&continuous(&[4.0], &plant_denominator, 0.0), &grid)
        .expect("the baseline loop is analysed");
    assert_close(baseline.peak_sensitivity, 3.1967, 0.003);
    assert_close(
        baseline
            .gain_crossover_rad_per_second
            .expect("a gain crossover"),
        2.187,
        0.01,
    );
    assert_close(finite(baseline.phase_margin_degrees), 19.13, 0.08);

    // The shaped controller (5/3·s² + 11/6·s + 0.4) / (25/48·s³ + 277/48·s² + 311/24·s + 1) times the plant's gain of 10.
    let numerator: Vec<f64> = [5.0 / 3.0, 11.0 / 6.0, 0.4]
        .iter()
        .map(|coefficient| 10.0 * coefficient)
        .collect();
    let denominator = crate::polynomial::convolve(
        &[25.0 / 48.0, 277.0 / 48.0, 311.0 / 24.0, 1.0],
        &plant_denominator,
    );
    let shaped = analyze_loop(&continuous(&numerator, &denominator, 0.0), &grid)
        .expect("the shaped loop is analysed");
    assert_close(shaped.peak_sensitivity, 1.7235, 0.003);
    assert_close(
        shaped
            .gain_crossover_rad_per_second
            .expect("a gain crossover"),
        1.706,
        0.01,
    );
    assert_close(finite(shaped.phase_margin_degrees), 44.11, 0.08);
}
