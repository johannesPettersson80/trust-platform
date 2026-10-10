#![allow(clippy::float_cmp)] // The products and sums of these small integers are exact, so they are compared exactly.

use super::*;

fn sorted_by_real_part(mut values: Vec<ComplexValue>) -> Vec<ComplexValue> {
    values.sort_by(|left, right| left.real.total_cmp(&right.real));
    values
}

#[test]
fn convolution_multiplies_the_polynomials() {
    // (s + 1)(s − 1) = s² − 1, and 2·(3s² + s) = 6s² + 2s.
    assert_eq!(convolve(&[1.0, 1.0], &[1.0, -1.0]), vec![1.0, 0.0, -1.0]);
    assert_eq!(convolve(&[2.0], &[3.0, 1.0, 0.0]), vec![6.0, 2.0, 0.0]);
    assert_eq!(convolve(&[], &[1.0]), [] as [f64; 0]);
}

#[test]
fn sums_align_the_polynomials_at_their_constant_term() {
    // (s² + 2s + 3) + (4s + 5) = s² + 6s + 8.
    assert_eq!(
        add_aligned(&[1.0, 2.0, 3.0], &[4.0, 5.0]),
        vec![1.0, 6.0, 8.0]
    );
}

#[test]
fn trimming_removes_leading_zeros_and_keeps_one_coefficient_when_all_are_zero() {
    assert_eq!(trim_leading_zeros(&[0.0, 0.0, 2.0, 1.0]), vec![2.0, 1.0]);
    assert_eq!(trim_leading_zeros(&[0.0, 0.0]), vec![0.0]);
}

#[test]
fn roots_of_a_real_quadratic_are_its_zeros() {
    // s² + 8s + 12 = (s + 2)(s + 6).
    let roots_found = sorted_by_real_part(roots(&[1.0, 8.0, 12.0]).expect("the solver converges"));
    assert_eq!(roots_found.len(), 2);
    assert!((roots_found[0].real + 6.0).abs() <= 1e-9, "{roots_found:?}");
    assert!((roots_found[1].real + 2.0).abs() <= 1e-9, "{roots_found:?}");
    assert!(roots_found.iter().all(|root| root.imaginary.abs() <= 1e-9));
}

#[test]
fn roots_of_a_complex_pair_are_conjugate() {
    // s² + 8s + 20 has the roots −4 ± 2j (the shaft's root locus at Kp = 20).
    let roots_found = roots(&[1.0, 8.0, 20.0]).expect("the solver converges");
    assert_eq!(roots_found.len(), 2);
    assert!(roots_found
        .iter()
        .all(|root| (root.real + 4.0).abs() <= 1e-9 && (root.imaginary.abs() - 2.0).abs() <= 1e-9));
    assert!((roots_found[0].imaginary + roots_found[1].imaginary).abs() <= 1e-9);
}

#[test]
fn leading_zeros_do_not_change_the_roots() {
    // 0·s² + s + 2 has the single root −2.
    let roots_found = roots(&[0.0, 1.0, 2.0]).expect("the solver converges");
    assert_eq!(roots_found.len(), 1);
    assert!((roots_found[0].real + 2.0).abs() <= 1e-9);
}

#[test]
fn a_constant_has_no_roots() {
    assert_eq!(
        roots(&[5.0]).expect("the solver converges"),
        [] as [ComplexValue; 0]
    );
}
