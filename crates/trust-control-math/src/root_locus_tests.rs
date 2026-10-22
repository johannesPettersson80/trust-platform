#![allow(clippy::float_cmp)] // The tests compare exact values on purpose.

use super::*;

fn sorted_real_parts(values: &[ComplexValue]) -> Vec<f64> {
    let mut parts: Vec<f64> = values.iter().map(|value| value.real).collect();
    parts.sort_by(f64::total_cmp);
    parts
}

#[test]
fn the_root_locus_gives_the_closed_loop_poles_at_each_gain() {
    // Ported from control-studio-numerics.test.ts: 1/(s² + s), gains 0 and 2.
    let transfer = Transfer {
        numerator: vec![1.0],
        denominator: vec![1.0, 1.0, 0.0],
        delay_seconds: 0.0,
    };
    let branches = root_locus(&transfer, &[0.0, 2.0]).expect("a root locus");
    let close = |actual: Vec<f64>, expected: [f64; 2]| {
        actual.len() == 2
            && actual
                .iter()
                .zip(expected)
                .all(|(a, e)| (a - e).abs() <= 1e-12)
    };
    assert!(close(sorted_real_parts(&branches[0].poles), [-1.0, 0.0]));
    assert!(close(sorted_real_parts(&branches[1].poles), [-0.5, -0.5]));
    let imaginary: Vec<f64> = branches[1]
        .poles
        .iter()
        .map(|pole| pole.imaginary.abs())
        .collect();
    assert!(imaginary
        .iter()
        .all(|value| (value - 7.0_f64.sqrt() / 2.0).abs() < 1e-9));
}

#[test]
fn a_zero_numerator_is_the_open_loop_starting_point_of_a_sweep() {
    // Ported: num [0] over den [1, 2, 0] at gain 0 gives the poles −2 and 0.
    let transfer = Transfer {
        numerator: vec![0.0],
        denominator: vec![1.0, 2.0, 0.0],
        delay_seconds: 0.0,
    };
    let branches = root_locus(&transfer, &[0.0]).expect("a root locus");
    assert_eq!(sorted_real_parts(&branches[0].poles), vec![-2.0, 0.0]);
}

#[test]
fn a_delay_a_negative_gain_and_an_empty_list_are_refused() {
    let delayed = Transfer {
        numerator: vec![1.0],
        denominator: vec![1.0, 1.0],
        delay_seconds: 0.1,
    };
    assert_eq!(
        root_locus(&delayed, &[1.0]),
        Err(ClassicalError::InvalidInput)
    );
    let plain = Transfer {
        numerator: vec![1.0],
        denominator: vec![1.0, 1.0],
        delay_seconds: 0.0,
    };
    assert_eq!(
        root_locus(&plain, &[-1.0]),
        Err(ClassicalError::InvalidInput)
    );
    assert_eq!(root_locus(&plain, &[]), Err(ClassicalError::InvalidInput));
}

#[test]
fn the_poles_of_a_sampled_loop_at_each_gain_are_the_roots_of_its_polynomial_in_z() {
    // Ported from control-studio-numerics.test.ts, "qualifies four discretizations and z-plane root-locus stability" (N7.5): the sampled
    // plant 1/(z² − 0.5z) at the gains 0 and 0.5. The solver does not know the domain: the roots of z² − 0.5z + k are 0 and 0.5 at
    // k = 0 and 0.25 ± √0.4375·j at k = 0.5, whose modulus √0.5 is inside the unit circle.
    let transfer = Transfer {
        numerator: vec![1.0],
        denominator: vec![1.0, -0.5, 0.0],
        delay_seconds: 0.0,
    };
    let branches = root_locus(&transfer, &[0.0, 0.5]).expect("a root locus");
    let open_loop = sorted_real_parts(&branches[0].poles);
    assert!(
        open_loop.len() == 2 && open_loop[0].abs() <= 1e-10 && (open_loop[1] - 0.5).abs() <= 1e-10,
        "{open_loop:?}"
    );
    assert_eq!(branches[1].poles.len(), 2);
    for pole in &branches[1].poles {
        assert!((pole.real - 0.25).abs() <= 1e-10, "{pole:?}");
        assert!(
            (pole.imaginary.abs() - 0.4375_f64.sqrt()).abs() <= 1e-9,
            "{pole:?}"
        );
        assert!(
            pole.real.hypot(pole.imaginary) < 1.0,
            "inside the unit circle: {pole:?}"
        );
    }
}
