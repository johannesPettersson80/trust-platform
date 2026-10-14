use super::*;

fn tolerance() -> NumericalTolerance {
    NumericalTolerance {
        absolute: 1e-12,
        relative: 1e-10,
        condition_limit: 1e8,
    }
}

#[test]
fn the_characteristic_polynomial_of_two_real_roots_is_their_product() {
    // (s + 4)(s + 5) = s² + 9s + 20.
    assert_eq!(real_root_polynomial(&[-4.0, -5.0]), vec![1.0, 9.0, 20.0]);
}

#[test]
fn a_second_order_plant_places_both_poles_at_the_desired_values() {
    // A = [[0, 1], [−2, −3]], B = [0, 1]ᵀ, desired poles −4 and −5: p(A) = A² + 9A + 20I = [[18, 6], [−12, 0]] and the
    // last row of C⁻¹ is [1, 0], so K = [18, 6]. Then A − B·K = [[0, 1], [−20, −9]], whose poles are −4 and −5.
    let placement = place_state_feedback_poles(
        &vec![vec![0.0, 1.0], vec![-2.0, -3.0]],
        &vec![vec![0.0], vec![1.0]],
        &[-4.0, -5.0],
        &tolerance(),
    )
    .expect("the plant is controllable");
    assert!((placement.gain[0][0] - 18.0).abs() <= 1e-10);
    assert!((placement.gain[0][1] - 6.0).abs() <= 1e-10);
    assert!(placement.pole_residual <= 1e-9);
    assert_eq!(placement.desired_poles.len(), 2);
    assert!(placement
        .achieved_poles
        .iter()
        .all(|pole| pole.imaginary.abs() <= 1e-9));
}

#[test]
fn the_double_integrator_places_its_poles_with_the_coefficients_of_the_characteristic_polynomial() {
    // A = [[0, 1], [0, 0]], B = [0, 1]ᵀ. A − B·K = [[0, 1], [−k₁, −k₂]] has the characteristic polynomial s² + k₂s + k₁,
    // which is (s + 1)(s + 2) = s² + 3s + 2 for the poles −1 and −2: K = [2, 3].
    let placement = place_state_feedback_poles(
        &vec![vec![0.0, 1.0], vec![0.0, 0.0]],
        &vec![vec![0.0], vec![1.0]],
        &[-1.0, -2.0],
        &tolerance(),
    )
    .expect("the double integrator is controllable");
    assert!((placement.gain[0][0] - 2.0).abs() <= 1e-10);
    assert!((placement.gain[0][1] - 3.0).abs() <= 1e-10);
}

#[test]
fn an_uncontrollable_plant_is_refused() {
    // The input reaches only the first state, and the second state is not coupled to it, so [B, AB] has rank one.
    let result = place_state_feedback_poles(
        &vec![vec![1.0, 0.0], vec![0.0, 2.0]],
        &vec![vec![1.0], vec![0.0]],
        &[-1.0, -2.0],
        &tolerance(),
    );
    assert_eq!(result, Err(PlacementRefusal::Uncontrollable));
}

#[test]
fn shapes_and_nonfinite_poles_are_refused_as_inconsistent_dimensions() {
    let a = vec![vec![0.0, 1.0], vec![-2.0, -3.0]];
    let b = vec![vec![0.0], vec![1.0]];
    let wrong_pole_count = place_state_feedback_poles(&a, &b, &[-4.0], &tolerance());
    assert_eq!(
        wrong_pole_count,
        Err(PlacementRefusal::InconsistentDimensions)
    );
    let two_inputs = place_state_feedback_poles(
        &a,
        &vec![vec![0.0, 1.0], vec![1.0, 0.0]],
        &[-4.0, -5.0],
        &tolerance(),
    );
    assert_eq!(two_inputs, Err(PlacementRefusal::InconsistentDimensions));
    let nonfinite = place_state_feedback_poles(&a, &b, &[-4.0, f64::NAN], &tolerance());
    assert_eq!(nonfinite, Err(PlacementRefusal::InconsistentDimensions));
}
