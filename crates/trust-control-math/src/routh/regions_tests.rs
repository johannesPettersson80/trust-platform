use super::*;
use crate::affine::Affine;

fn parameter_polynomial(constants_and_slopes: &[(f64, f64)]) -> Vec<Affine> {
    constants_and_slopes
        .iter()
        .map(|&(constant, slope)| Affine { constant, slope })
        .collect()
}

#[test]
fn disjoint_algebraic_stability_regions_are_both_kept_with_exact_boundaries() {
    // Ported from control-studio-routh-hurwitz.test.ts: s³ + (3 + p)s² + (3 + p)s + (10 + 4p). The Hurwitz
    // inequalities give 10 + 4p > 0, 3 + p > 0 and (3 + p)² > 10 + 4p, so the stable set is
    // (−2.5, −1 − √2) together with (−1 + √2, ∞).
    let coefficients = parameter_polynomial(&[(1.0, 0.0), (3.0, 1.0), (3.0, 1.0), (10.0, 4.0)]);
    let HurwitzRegion::Regions {
        kind,
        intervals,
        boundaries,
    } = hurwitz_region(&coefficients).expect("a region")
    else {
        panic!("the polynomial is not degenerate");
    };
    assert_eq!(kind, RegionKind::Union);
    assert_eq!(intervals.len(), 2);
    assert!((intervals[0].lower.expect("bounded") + 2.5).abs() < 1e-12);
    assert!(
        (intervals[0].upper.expect("bounded") - (-1.0 - std::f64::consts::SQRT_2)).abs() < 1e-12
    );
    assert!(
        (intervals[1].lower.expect("bounded") - (-1.0 + std::f64::consts::SQRT_2)).abs() < 1e-12
    );
    assert_eq!(intervals[1].upper, None);
    assert!(
        boundaries
            .iter()
            .all(|boundary| !boundary.polynomial.is_empty()),
        "every boundary keeps its certificate"
    );
}

#[test]
fn a_degree_seven_polynomial_has_a_bounded_stability_interval() {
    // Ported: the characteristic polynomial (s + 1)⁷ with its constant coefficient 1 + p. The stable
    // interval contains zero.
    let coefficients = parameter_polynomial(&[
        (1.0, 0.0),
        (7.0, 0.0),
        (21.0, 0.0),
        (35.0, 0.0),
        (35.0, 0.0),
        (21.0, 0.0),
        (7.0, 0.0),
        (1.0, 1.0),
    ]);
    let HurwitzRegion::Regions { intervals, .. } = hurwitz_region(&coefficients).expect("a region")
    else {
        panic!("the polynomial is not degenerate");
    };
    assert_eq!(intervals.len(), 1);
    let lower = intervals[0].lower.expect("bounded below");
    let upper = intervals[0].upper.expect("bounded above");
    assert!(lower < 0.0 && upper > 0.0);
}

#[test]
fn a_leading_coefficient_that_is_identically_zero_is_refused() {
    let coefficients = parameter_polynomial(&[(0.0, 0.0), (1.0, 0.0)]);
    assert_eq!(
        hurwitz_region(&coefficients),
        Err(ExactError::ZeroLeadingCoefficient)
    );
}

#[test]
fn a_constant_polynomial_with_positive_coefficients_is_stable_for_every_parameter() {
    // s + (2 + 0p): the Hurwitz inequality a0 > 0 holds on the whole axis.
    let coefficients = parameter_polynomial(&[(1.0, 0.0), (2.0, 0.0)]);
    let HurwitzRegion::Regions {
        kind, intervals, ..
    } = hurwitz_region(&coefficients).expect("a region")
    else {
        panic!("the polynomial is not degenerate");
    };
    assert_eq!(kind, RegionKind::AllReal);
    assert_eq!(intervals.len(), 1);
}
