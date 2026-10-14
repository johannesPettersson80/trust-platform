use super::*;
use crate::linalg::DenseMatrix;

fn tolerance() -> NumericalTolerance {
    NumericalTolerance {
        absolute: 1e-12,
        relative: 1e-10,
        condition_limit: 1e8,
    }
}

fn system(a: DenseMatrix, b: DenseMatrix, c: DenseMatrix) -> StateSpace {
    let outputs = c.len();
    StateSpace {
        a,
        b,
        c,
        d: vec![vec![0.0]; outputs],
    }
}

fn assert_eigenvalues(actual: &[ComplexValue], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len(), "eigenvalue count");
    for value in expected {
        assert!(
            actual
                .iter()
                .any(|pole| (pole.real - value).abs() <= 1e-10 && pole.imaginary.abs() <= 1e-10),
            "eigenvalue {value} is missing from {actual:?}"
        );
    }
}

#[test]
fn a_controllable_and_observable_second_order_model_has_full_ranks() {
    // A = [[0, 1], [−2, −3]], B = [0, 1]ᵀ, C = [1, 0]: [B, AB] = [[0, 1], [1, −3]] and [C; CA] = I, so both ranks are two.
    let analysis = analyze_state_space(
        &system(
            vec![vec![0.0, 1.0], vec![-2.0, -3.0]],
            vec![vec![0.0], vec![1.0]],
            vec![vec![1.0, 0.0]],
        ),
        &tolerance(),
    )
    .expect("the model qualifies");
    assert_eigenvalues(&analysis.eigenvalues, &[-1.0, -2.0]);
    assert_eq!(analysis.structure.controllability.rank, 2);
    assert_eq!(
        analysis.structure.controllability.matrix,
        vec![vec![0.0, 1.0], vec![1.0, -3.0]]
    );
    assert_eq!(analysis.structure.observability.rank, 2);
    assert_eq!(
        analysis.structure.observability.matrix,
        vec![vec![1.0, 0.0], vec![0.0, 1.0]]
    );
    assert!(analysis.controllable && analysis.observable);
    assert!(analysis.stabilisable && analysis.detectable);
}

#[test]
fn stable_modes_that_cannot_be_reached_or_seen_are_still_stabilisable_and_detectable() {
    // A = diag(−1, −2) with the input and the output reaching only the first mode: the second mode is hidden but stable.
    let analysis = analyze_state_space(
        &system(
            vec![vec![-1.0, 0.0], vec![0.0, -2.0]],
            vec![vec![1.0], vec![0.0]],
            vec![vec![1.0, 0.0]],
        ),
        &tolerance(),
    )
    .expect("the model qualifies");
    assert!(!analysis.controllable);
    assert!(!analysis.observable);
    assert!(analysis.stabilisable);
    assert!(analysis.detectable);
}

#[test]
fn an_unstable_hidden_mode_makes_the_model_neither_stabilisable_nor_detectable() {
    // The unstable mode +0.5 is hidden from the input and from the output: the Hautus rank is one, below two.
    let analysis = analyze_state_space(
        &system(
            vec![vec![-1.0, 0.0], vec![0.0, 0.5]],
            vec![vec![1.0], vec![0.0]],
            vec![vec![1.0, 0.0]],
        ),
        &tolerance(),
    )
    .expect("the model qualifies");
    assert!(!analysis.stabilisable);
    assert!(!analysis.detectable);
}

#[test]
fn a_model_without_states_or_with_a_nonfinite_value_is_inconsistent() {
    let static_gain = StateSpace {
        a: vec![],
        b: vec![],
        c: vec![vec![]],
        d: vec![vec![2.0]],
    };
    assert_eq!(
        analyze_state_space(&static_gain, &tolerance()).map(|_| ()),
        Err(StateSpaceRefusal::InconsistentDimensions)
    );
    let nonfinite = system(vec![vec![f64::NAN]], vec![vec![1.0]], vec![vec![1.0]]);
    assert_eq!(
        analyze_state_space(&nonfinite, &tolerance()).map(|_| ()),
        Err(StateSpaceRefusal::InconsistentDimensions)
    );
}
