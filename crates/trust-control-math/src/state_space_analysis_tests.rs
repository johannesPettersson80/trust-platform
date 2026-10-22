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

/// One model of the positioning axis: its state matrix, its three poles (real and imaginary parts), and whether they all lie inside
/// the unit circle.
struct AxisModel {
    name: &'static str,
    matrix: [[f64; 3]; 3],
    poles: [(f64, f64); 3],
    stable: bool,
}

/// Whether each expected eigenvalue (its real and imaginary parts) is among `actual` within `tolerance`, and no other is.
fn assert_spectrum(actual: &[ComplexValue], expected: &[(f64, f64)], tolerance: f64) {
    assert_eq!(actual.len(), expected.len(), "eigenvalue count");
    for (real, imaginary) in expected {
        let found = actual.iter().any(|value| {
            (value.real - real).abs() <= tolerance
                && (value.imaginary - imaginary).abs() <= tolerance
        });
        assert!(found, "{real} + {imaginary}i is missing from {actual:?}");
    }
}

#[test]
fn the_positioning_axis_models_have_the_poles_that_decide_their_stability_at_their_sample_period() {
    // Ported from control-studio-numerics.test.ts, "qualifies the executed positioning-axis sample, hold, and delay models" (N4.6): three
    // zero-order-hold models of the positioning axis with its delayed input held in a third state. The 10 ms model and the corrected
    // 100 ms model have every pole inside the unit circle; the 100 ms model that keeps the 10 ms gain has a pair outside it.
    let models = [
        AxisModel {
            name: "fine",
            matrix: [
                [1.0, 0.009_900_663_346_622_374, 0.000_496_683_266_888_134_4],
                [0.0, 0.980_198_673_306_755_3, 0.099_006_633_466_223_74],
                [-4.0, -0.4, 0.0],
            ],
            poles: [
                (0.039_841_00, 0.0),
                (0.970_178_83, -0.056_879_62),
                (0.970_178_83, 0.056_879_62),
            ],
            stable: true,
        },
        AxisModel {
            name: "coarse unchanged",
            matrix: [
                [1.0, 0.090_634_623_461_009_09, 0.046_826_882_694_954_55],
                [0.0, 0.818_730_753_077_981_8, 0.906_346_234_610_090_9],
                [-4.0, -0.4, 0.0],
            ],
            poles: [
                (0.172_757_44, 0.0),
                (0.822_986_65, -0.637_899_37),
                (0.822_986_65, 0.637_899_37),
            ],
            stable: false,
        },
        AxisModel {
            name: "coarse corrected",
            matrix: [
                [1.0, 0.090_634_623_461_009_09, 0.046_826_882_694_954_55],
                [0.0, 0.818_730_753_077_981_8, 0.906_346_234_610_090_9],
                [-1.0, -0.4, 0.0],
            ],
            poles: [
                (0.743_612_64, 0.0),
                (0.537_559_06, -0.373_704_39),
                (0.537_559_06, 0.373_704_39),
            ],
            stable: true,
        },
    ];
    for model in models {
        let rows: Vec<Vec<f64>> = model.matrix.iter().map(|row| row.to_vec()).collect();
        let spectrum = eigenvalues(&rows, &tolerance()).expect(model.name);
        assert_spectrum(&spectrum.values, &model.poles, 1e-8);
        let inside = spectrum
            .values
            .iter()
            .all(|value| value.real.hypot(value.imaginary) < 1.0);
        assert_eq!(inside, model.stable, "{}", model.name);
    }
}
