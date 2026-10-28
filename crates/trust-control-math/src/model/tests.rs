#![allow(clippy::float_cmp)] // Exact coefficient lists are compared on purpose.

use super::*;

fn assert_close(actual: &[Vec<f64>], expected: &[Vec<f64>], tolerance: f64) {
    assert_eq!(actual.len(), expected.len(), "row count");
    for (row, (actual_row, expected_row)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(
            actual_row.len(),
            expected_row.len(),
            "column count of row {row}"
        );
        for (column, (value, reference)) in actual_row.iter().zip(expected_row).enumerate() {
            assert!(
                (value - reference).abs() <= tolerance,
                "entry ({row}, {column}): {value} differs from {reference}"
            );
        }
    }
}

fn two_tank_structure_model() -> StateSpace {
    // A = [[−0.04, 0], [0.04, −0.04]], B = [2; 0], C = [0, 1], D = 0.
    StateSpace::new(
        vec![vec![-0.04, 0.0], vec![0.04, -0.04]],
        vec![vec![2.0], vec![0.0]],
        vec![vec![0.0, 1.0]],
        vec![vec![0.0]],
    )
    .expect("the two-tank matrices have consistent shapes")
}

fn tolerance() -> NumericalTolerance {
    NumericalTolerance {
        absolute: 1e-12,
        relative: 1e-10,
        condition_limit: 1e12,
    }
}

#[test]
fn companion_form_of_a_second_order_function_matches_its_coefficients() {
    // N/D = (s + 1)/(s² + 3s + 2): A = [[−3, −2], [1, 0]], B = e₁, C = [1, 1], D = 0.
    let companion = transfer_function_state_space(&[1.0, 1.0], &[1.0, 3.0, 2.0])
        .expect("a proper function with a nonzero leading coefficient");
    assert_eq!(companion.order, 2);
    assert_eq!(companion.numerator, vec![0.0, 1.0, 1.0]);
    assert_eq!(companion.denominator, vec![1.0, 3.0, 2.0]);
    assert_close(
        &companion.system.a,
        &[vec![-3.0, -2.0], vec![1.0, 0.0]],
        1e-12,
    );
    assert_close(&companion.system.b, &[vec![1.0], vec![0.0]], 1e-12);
    assert_close(&companion.system.c, &[vec![1.0, 1.0]], 1e-12);
    assert_close(&companion.system.d, &[vec![0.0]], 1e-12);
}

#[test]
fn a_constant_transfer_function_is_a_static_gain_without_states() {
    // N/D = 4/2 = 2: no states, and the gain is the feedthrough.
    let companion = transfer_function_state_space(&[4.0], &[2.0]).expect("a static gain");
    assert_eq!(companion.order, 0);
    assert_eq!(companion.numerator, vec![2.0]);
    assert_eq!(companion.system.states(), 0);
    assert_close(&companion.system.d, &[vec![2.0]], 1e-12);
}

#[test]
fn state_space_converts_to_its_transfer_polynomials_without_cancelling_hidden_modes() {
    // C(sI − A)⁻¹B with A = [[0, 1], [−2, −3]] has denominator s² + 3s + 2 and numerator 1.
    let system = StateSpace::new(
        vec![vec![0.0, 1.0], vec![-2.0, -3.0]],
        vec![vec![0.0], vec![1.0]],
        vec![vec![1.0, 0.0]],
        vec![vec![0.0]],
    )
    .expect("consistent shapes");
    let converted = state_space_transfer_polynomials(&system, 16).expect("SISO and small");
    assert_eq!(converted.denominator, vec![1.0, 3.0, 2.0]);
    assert_eq!(converted.numerator, vec![1.0]);

    // With A = diag(−1, 1), the mode at +1 is hidden from the input: the full denominator
    // (s + 1)(s − 1) is kept, and the numerator (s − 1) is not cancelled.
    let hidden = StateSpace::new(
        vec![vec![-1.0, 0.0], vec![0.0, 1.0]],
        vec![vec![1.0], vec![0.0]],
        vec![vec![1.0, 0.0]],
        vec![vec![0.0]],
    )
    .expect("consistent shapes");
    let converted = state_space_transfer_polynomials(&hidden, 16).expect("SISO and small");
    assert_eq!(converted.denominator, vec![1.0, 0.0, -1.0]);
    assert_eq!(converted.numerator, vec![1.0, -1.0]);
}

#[test]
fn transfer_conversion_refuses_a_multi_input_model_and_an_order_above_the_limit() {
    let two_inputs = StateSpace::new(
        vec![vec![-1.0]],
        vec![vec![1.0, 0.0]],
        vec![vec![1.0]],
        vec![vec![0.0, 0.0]],
    )
    .expect("consistent shapes");
    assert_eq!(
        state_space_transfer_polynomials(&two_inputs, 16),
        Err(ConversionError::NotSiso)
    );
    assert_eq!(
        state_space_transfer_polynomials(&two_tank_structure_model(), 1),
        Err(ConversionError::OrderLimit)
    );
}

#[test]
fn a_sampled_one_state_model_becomes_its_held_input_transfer_function() {
    // a = e^{−T/2} with T = 0.02 is a pole at −0.5; b = 3(1 − a) gives gain 3(1 − a)·(−0.5)/(a − 1) = 1.5.
    let step = 0.02_f64;
    let pole = (-step / 2.0).exp();
    let system = StateSpace::new(
        vec![vec![pole]],
        vec![vec![3.0 * (1.0 - pole)]],
        vec![vec![1.0]],
        vec![vec![0.0]],
    )
    .expect("consistent shapes");
    let transfer = sampled_one_state_transfer(&system, step).expect("a positive one-state model");
    assert!((transfer.denominator[1] - 0.5).abs() < 1e-12);
    assert!((transfer.numerator[0] - 1.5).abs() < 1e-12);

    // a = 1 is an integrator: the input gain is b/T = 0.04/0.02 = 2, and the pole is zero.
    let integrator = StateSpace::new(
        vec![vec![1.0]],
        vec![vec![2.0 * step]],
        vec![vec![1.0]],
        vec![vec![0.0]],
    )
    .expect("consistent shapes");
    let transfer = sampled_one_state_transfer(&integrator, step).expect("an integrator");
    assert_eq!(transfer.denominator, vec![1.0, 0.0]);
    assert!((transfer.numerator[0] - 2.0).abs() < 1e-12);
}

#[test]
fn a_sampled_model_with_a_nonpositive_transition_or_feedthrough_is_refused() {
    let negative = StateSpace::new(
        vec![vec![-0.5]],
        vec![vec![1.0]],
        vec![vec![1.0]],
        vec![vec![0.0]],
    )
    .expect("consistent shapes");
    assert_eq!(
        sampled_one_state_transfer(&negative, 0.1),
        Err(ConversionError::UnsupportedSampledModel)
    );
    let feedthrough = StateSpace::new(
        vec![vec![0.5]],
        vec![vec![1.0]],
        vec![vec![1.0]],
        vec![vec![1.0]],
    )
    .expect("consistent shapes");
    assert_eq!(
        sampled_one_state_transfer(&feedthrough, 0.1),
        Err(ConversionError::UnsupportedSampledModel)
    );
}

#[test]
fn the_two_tank_structure_has_full_rank_matrices_and_hidden_observability_when_c_changes() {
    // Controllability [B, AB] = [[2, −0.08], [0, 0.08]]; observability [C; CA] = [[0, 1], [0.04, −0.04]].
    let structure = state_structure(&two_tank_structure_model(), &tolerance())
        .expect("the structure qualifies");
    assert_eq!(structure.controllability.rank, 2);
    assert_close(
        &structure.controllability.matrix,
        &[vec![2.0, -0.08], vec![0.0, 0.08]],
        1e-12,
    );
    assert_eq!(structure.observability.rank, 2);
    assert_close(
        &structure.observability.matrix,
        &[vec![0.0, 1.0], vec![0.04, -0.04]],
        1e-12,
    );

    // With C = [1, 0] the observability matrix is [[1, 0], [−0.04, 0]], which has rank one.
    let hidden = StateSpace::new(
        vec![vec![-0.04, 0.0], vec![0.04, -0.04]],
        vec![vec![2.0], vec![0.0]],
        vec![vec![1.0, 0.0]],
        vec![vec![0.0]],
    )
    .expect("consistent shapes");
    let structure = state_structure(&hidden, &tolerance()).expect("the structure qualifies");
    assert_eq!(structure.controllability.rank, 2);
    assert_eq!(structure.observability.rank, 1);
    assert!(structure.fits(2, 1, 1));
    assert!(!structure.fits(2, 2, 1));
}

#[test]
fn constructors_refuse_invalid_coefficients_with_typed_reasons() {
    assert_eq!(
        TransferFunction::new(vec![1.0], vec![0.0, 1.0]),
        Err(ConstructionError::ZeroLeadingDenominator)
    );
    assert_eq!(
        TransferFunction::new(vec![1.0, 2.0, 3.0], vec![1.0, 1.0]),
        Err(ConstructionError::Improper)
    );
    assert_eq!(
        TransferFunction::new(Vec::new(), vec![1.0]),
        Err(ConstructionError::Empty)
    );
    assert_eq!(
        TransferFunction::new(vec![f64::NAN], vec![1.0]),
        Err(ConstructionError::Nonfinite)
    );
    assert_eq!(Timing::discrete(0.0), Err(ConstructionError::SampleTime));
    assert_eq!(
        StateSpace::new(
            vec![vec![1.0]],
            vec![vec![1.0], vec![2.0]],
            vec![vec![1.0]],
            vec![vec![0.0]]
        ),
        Err(ConstructionError::Shape)
    );
}
