use super::*;

const LIMITS: HorizonLimits = HorizonLimits {
    maximum_steps: 512,
    maximum_states: 32,
    maximum_inputs: 16,
};

fn tolerance() -> NumericalTolerance {
    NumericalTolerance {
        absolute: 1e-12,
        relative: 1e-10,
        condition_limit: 1e12,
    }
}

fn scalar(value: f64) -> DenseMatrix {
    vec![vec![value]]
}

#[test]
fn the_scalar_kalman_gain_matches_its_closed_form() {
    // a = c = 1, process noise q and measurement noise r. The settled prediction P⁻ satisfies P⁻² − qP⁻ − qr = 0, so
    // P⁻ = (q + √(q² + 4qr))/2. The gain is P⁻/(P⁻ + r) and the settled covariance is P⁻·r/(P⁻ + r).
    let (q, r) = (0.1_f64, 0.4_f64);
    let predicted = f64::midpoint(q, (q * q + 4.0 * q * r).sqrt());
    let gain = predicted / (predicted + r);
    let covariance = predicted * r / (predicted + r);
    let estimate = steady_state_kalman(
        &scalar(1.0),
        &scalar(1.0),
        &scalar(q),
        &scalar(r),
        &tolerance(),
    )
    .expect("the scalar estimator settles");
    assert!((estimate.gain[0][0] - gain).abs() <= 1e-9);
    assert!((estimate.covariance[0][0] - covariance).abs() <= 1e-9);
    assert!(estimate.riccati_residual <= 1e-10);
}

#[test]
fn the_scalar_lqg_design_separates_into_the_lqr_pole_and_the_estimator_pole() {
    // With a = b = c = q = r = 1, the LQR gain is (√5 − 1)/2, so the controller pole is (3 − √5)/2; the estimator pole is
    // 1 − L, with L the steady-state Kalman gain of the closed form above.
    let (process, measurement) = (0.1_f64, 0.4_f64);
    let predicted = f64::midpoint(
        process,
        (process * process + 4.0 * process * measurement).sqrt(),
    );
    let estimator_pole = 1.0 - predicted / (predicted + measurement);
    let problem = LqgProblem {
        a: &scalar(1.0),
        b: &scalar(1.0),
        c: &scalar(1.0),
        q: &scalar(1.0),
        r: &scalar(1.0),
        process_noise: &scalar(process),
        measurement_noise: &scalar(measurement),
        horizon: None,
        limits: LIMITS,
    };
    let design = design_discrete_lqg(&problem, &tolerance()).expect("the LQG design is qualified");
    assert!(design
        .lqr
        .riccati_residual
        .is_some_and(|residual| residual <= 1e-10));
    assert!(design.kalman.riccati_residual <= 1e-10);
    let root_five = 5.0_f64.sqrt();
    assert!((design.controller_poles[0].real - (3.0 - root_five) / 2.0).abs() <= 1e-9);
    assert!((design.estimator_poles[0].real - estimator_pole).abs() <= 1e-9);
    assert_eq!(design.separation_poles.len(), 2);
    assert_eq!(design.separation_poles[0], design.controller_poles[0]);
    assert_eq!(design.separation_poles[1], design.estimator_poles[0]);
}

#[test]
fn an_estimator_that_does_not_settle_inside_the_unit_circle_is_refused() {
    // With no process noise the estimator gain is zero, so the estimator is the model itself: a model with a = 1.5 has
    // an estimator pole of 1.5, outside the unit circle, while its LQR is stabilising.
    let problem = LqgProblem {
        a: &scalar(1.5),
        b: &scalar(1.0),
        c: &scalar(1.0),
        q: &scalar(1.0),
        r: &scalar(1.0),
        process_noise: &scalar(0.0),
        measurement_noise: &scalar(1.0),
        horizon: None,
        limits: LIMITS,
    };
    assert_eq!(
        design_discrete_lqg(&problem, &tolerance()),
        Err(LqgRefusal::NonstabilisingEstimator)
    );
}

#[test]
fn an_invalid_horizon_of_the_lqr_refuses_the_whole_lqg_design() {
    let horizon = FiniteHorizon {
        steps: 0,
        terminal_weights: scalar(1.0),
    };
    let problem = LqgProblem {
        a: &scalar(1.0),
        b: &scalar(1.0),
        c: &scalar(1.0),
        q: &scalar(1.0),
        r: &scalar(1.0),
        process_noise: &scalar(0.1),
        measurement_noise: &scalar(0.4),
        horizon: Some(&horizon),
        limits: LIMITS,
    };
    assert_eq!(
        design_discrete_lqg(&problem, &tolerance()),
        Err(LqgRefusal::Lqr(LqrRefusal::InvalidFiniteHorizon))
    );
}
