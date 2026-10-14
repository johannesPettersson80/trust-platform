use super::*;

fn sign_list(analysis: &RouthPolynomialAnalysis) -> Vec<RouthSign> {
    analysis
        .first_column
        .iter()
        .map(|entry| entry.sign)
        .collect()
}

#[test]
fn a_generic_table_counts_the_established_right_half_plane_poles() {
    // Ported: s² + 5s + 6 is stable; s³ + 2s² + s + 4 has two sign changes in its first column.
    let stable = analyze_routh_polynomial(&[1.0, 5.0, 6.0]);
    assert_eq!(stable.status, RouthStatus::Qualified);
    let rows: Vec<(usize, Vec<f64>)> = stable
        .rows
        .iter()
        .map(|row| (row.power, row.entries.clone()))
        .collect();
    assert_eq!(
        rows,
        vec![
            (2, vec![1.0, 6.0]),
            (1, vec![5.0, 0.0]),
            (0, vec![6.0, 0.0])
        ]
    );
    assert_eq!(stable.sign_changes, Some(0));
    assert_eq!(stable.stable, Some(true));
    assert!(stable.interpretation.contains("asymptotically stable"));
    let unstable = analyze_routh_polynomial(&[1.0, 2.0, 1.0, 4.0]);
    assert_eq!(
        sign_list(&unstable),
        vec![
            RouthSign::Positive,
            RouthSign::Positive,
            RouthSign::Negative,
            RouthSign::Positive
        ]
    );
    assert_eq!(unstable.sign_changes, Some(2));
    assert_eq!(unstable.stable, Some(false));
}

#[test]
fn rows_are_named_with_superscript_powers_of_s() {
    let zero_pivot = analyze_routh_polynomial(&[1.0, 0.0, 1.0, 2.0, 3.0]);
    let explanations: Vec<&str> = zero_pivot
        .issues
        .iter()
        .map(|issue| issue.explanation.as_str())
        .collect();
    assert!(
        explanations
            .iter()
            .any(|text| text.contains("s³") || text.contains("s²") || text.contains("s⁴")),
        "{explanations:?}"
    );
    assert!(
        explanations.iter().all(|text| !text.contains('^')),
        "{explanations:?}"
    );
}

#[test]
fn invalid_polynomials_are_rejected_and_boundary_cases_stay_explicit() {
    assert_eq!(
        analyze_routh_polynomial(&[0.0, 1.0]).status,
        RouthStatus::Rejected
    );
    assert_eq!(
        analyze_routh_polynomial(&[0.0, 0.0]).status,
        RouthStatus::Rejected
    );
    assert_eq!(
        analyze_routh_polynomial(&[1.0, f64::NAN]).status,
        RouthStatus::Rejected
    );
    let origin = analyze_routh_polynomial(&[1.0, 1.0, 0.0]);
    assert_eq!(
        origin.sign_changes,
        Some(0),
        "the exact auxiliary handling counts the right-half-plane poles"
    );
    assert_eq!(origin.stable, Some(false));
    assert!(origin
        .issues
        .iter()
        .any(|issue| issue.kind == RouthIssueKind::OriginPole));
    assert!(origin
        .issues
        .iter()
        .any(|issue| issue.kind == RouthIssueKind::AllZeroRow));
    assert!(origin.interpretation.contains("origin pole"));
    assert!(origin.interpretation.contains("auxiliary polynomial"));
    assert!(!origin.interpretation.contains("formal"));
    let zero_row = origin
        .issues
        .iter()
        .find(|issue| issue.kind == RouthIssueKind::AllZeroRow)
        .expect("the zero row");
    assert!(zero_row.explanation.contains("derivative"));
    assert!(!zero_row.explanation.contains("requires separate analysis"));
    let zero_pivot = analyze_routh_polynomial(&[1.0, 0.0, 1.0, 2.0, 3.0]);
    assert_eq!(
        zero_pivot.status,
        RouthStatus::Qualified,
        "an exact zero pivot uses the symbolic rule"
    );
    assert_eq!(zero_pivot.sign_changes, Some(2));
    assert!(zero_pivot
        .issues
        .iter()
        .any(|issue| issue.kind == RouthIssueKind::ZeroPivot));
    let later = analyze_routh_polynomial(&[1.0, -3.0, -1.0, 3.0, -3.0]);
    assert_eq!(later.status, RouthStatus::Qualified);
    assert_eq!(later.sign_changes, Some(3));
    let real_symmetric = analyze_routh_polynomial(&[1.0, 0.0, -1.0]);
    assert_eq!(real_symmetric.status, RouthStatus::Qualified);
    assert_eq!(real_symmetric.stable, Some(false));
    let near_zero = analyze_routh_polynomial(&[1.0, 1.0, 1e-13]);
    assert_eq!(near_zero.status, RouthStatus::Uncertain);
    assert!(near_zero
        .issues
        .iter()
        .any(|issue| issue.kind == RouthIssueKind::NearZeroEntry));
}

#[test]
fn an_auxiliary_row_is_a_boundary_with_its_derivative_named() {
    let auxiliary = analyze_routh_polynomial(&[1.0, 0.0, 2.0, 0.0, 1.0]);
    assert_eq!(auxiliary.status, RouthStatus::Boundary);
    assert_eq!(auxiliary.stable, Some(false));
    assert!(auxiliary
        .issues
        .iter()
        .any(|issue| issue.kind == RouthIssueKind::AllZeroRow));
    assert!(auxiliary.interpretation.contains("auxiliary"));
}

#[test]
fn known_factored_polynomials_count_their_poles_across_scales_and_singular_cases() {
    // Ported loop of control-studio-routh-hurwitz.test.ts: products of real and conjugate roots at three scales.
    let product = |a: &[f64], b: &[f64]| -> Vec<f64> {
        let mut result = vec![0.0; a.len() + b.len() - 1];
        for (i, x) in a.iter().enumerate() {
            for (j, y) in b.iter().enumerate() {
                result[i + j] += x * y;
            }
        }
        result
    };
    for scale in [1.0, 1e-9, 1e9] {
        for real_roots in [
            vec![-1.0, -1.0],
            vec![-1.0, 1.0],
            vec![-2.0, 0.0, 2.0],
            vec![-3.0, -2.0, -1.0, 1.0, 2.0, 3.0],
        ] {
            for conjugate_real in [-1.0, 0.0, 1.0] {
                let base = real_roots.iter().fold(vec![1.0], |polynomial, root| {
                    product(&polynomial, &[1.0, -root])
                });
                let quadratic = [
                    1.0,
                    -2.0 * conjugate_real,
                    conjugate_real * conjugate_real + 1.0,
                ];
                let combined: Vec<f64> = product(&base, &quadratic)
                    .into_iter()
                    .map(|value| {
                        if scale == 1.0 {
                            value
                        } else {
                            let exponent = if scale < 1.0 { -9 } else { 9 };
                            format!("{value}e{exponent}")
                                .parse::<f64>()
                                .expect("a number")
                        }
                    })
                    .collect();
                let result = analyze_routh_polynomial(&combined);
                let boundary = real_roots.contains(&0.0) || conjugate_real == 0.0;
                let expected_status = if boundary {
                    RouthStatus::Boundary
                } else {
                    RouthStatus::Qualified
                };
                assert_eq!(result.status, expected_status, "{combined:?}");
                let positive = real_roots.iter().filter(|root| **root > 0.0).count();
                let complex = if conjugate_real > 0.0 { 2 } else { 0 };
                assert_eq!(
                    result.sign_changes,
                    Some(positive + complex),
                    "{combined:?}"
                );
            }
        }
    }
}
