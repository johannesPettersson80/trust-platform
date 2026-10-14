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
