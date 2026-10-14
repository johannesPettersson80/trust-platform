use super::*;

#[test]
fn a_slope_gives_a_half_line_and_no_slope_gives_all_or_nothing() {
    assert_eq!(
        solve_positive(Affine {
            constant: 1.0,
            slope: 2.0
        }),
        PositiveSolution::Above(-0.5)
    );
    assert_eq!(
        solve_positive(Affine {
            constant: 1.0,
            slope: -2.0
        }),
        PositiveSolution::Below(0.5)
    );
    assert_eq!(
        solve_positive(Affine {
            constant: 1.0,
            slope: 0.0
        }),
        PositiveSolution::All
    );
    assert_eq!(
        solve_positive(Affine {
            constant: 0.0,
            slope: 0.0
        }),
        PositiveSolution::Empty
    );
}

#[test]
fn an_ill_conditioned_slope_or_a_nonfinite_value_is_uncertain() {
    // The slope 1e-15 is below 1e-12 times the larger of one and the constant.
    assert_eq!(
        solve_positive(Affine {
            constant: 1.0,
            slope: 1e-15
        }),
        PositiveSolution::Uncertain
    );
    assert_eq!(
        solve_positive(Affine {
            constant: f64::NAN,
            slope: 1.0
        }),
        PositiveSolution::Uncertain
    );
}

#[test]
fn a_numeric_polynomial_times_an_affine_one_stays_affine() {
    // (2s + 1)(3s + p) = 6s² + (3 + 2p)s + p: the numeric factor [2, 1] and the affine factor [3, p].
    let product = convolve_numeric(
        &[2.0, 1.0],
        &[Affine::constant(3.0), Affine::parameter(1.0)],
    );
    assert_eq!(
        product,
        vec![
            Affine {
                constant: 6.0,
                slope: 0.0
            },
            Affine {
                constant: 3.0,
                slope: 2.0
            },
            Affine {
                constant: 0.0,
                slope: 1.0
            },
        ]
    );
}

#[test]
fn sums_align_at_the_constant_term() {
    let total = sum(
        &[Affine::constant(1.0), Affine::parameter(3.0)],
        &[Affine::constant(2.0)],
    );
    assert_eq!(
        total,
        vec![
            Affine::constant(1.0),
            Affine {
                constant: 2.0,
                slope: 3.0
            }
        ]
    );
}
