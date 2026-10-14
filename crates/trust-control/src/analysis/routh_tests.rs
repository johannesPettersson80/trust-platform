use super::*;
use crate::analysis::declared::{
    DeclaredCoefficient, DeclaredController, DeclaredParameter, DeclaredPlant, DeclaredSweep,
    ParameterCoefficient, PlantKind,
};
use crate::model::{ModelSource, ModelValidation};
use std::collections::BTreeMap;

fn sign_list(analysis: &RouthPolynomialAnalysis) -> Vec<RouthSign> {
    analysis
        .first_column
        .iter()
        .map(|entry| entry.sign)
        .collect()
}

#[allow(clippy::needless_pass_by_value)] // The fixture's lists are written inline at each call.
fn gain_definition(
    numerator: Vec<f64>,
    denominator: Vec<f64>,
    plant_den: Vec<f64>,
    gain_range: (f64, f64),
    values: Vec<f64>,
    selected: f64,
) -> DeclaredControlAnalysis {
    let mut parameters = BTreeMap::new();
    parameters.insert(
        "gain".to_string(),
        DeclaredParameter {
            value: 1.0,
            minimum: gain_range.0,
            maximum: gain_range.1,
            unit: "1".to_string(),
        },
    );
    DeclaredControlAnalysis {
        plant: DeclaredPlant {
            kind: PlantKind::TransferFunction,
            numerator: vec![1.0],
            denominator: plant_den,
            delay_seconds: 0.0,
            input_unit: "V".to_string(),
            output_unit: "m".to_string(),
            revision: "generic-2".to_string(),
            source: ModelSource::Provided,
            validation: ModelValidation::Validated,
        },
        controller: DeclaredController {
            numerator: numerator
                .iter()
                .map(|_| DeclaredCoefficient::Constant(0.0))
                .collect::<Vec<_>>(),
            denominator: denominator
                .iter()
                .map(|value| DeclaredCoefficient::Constant(*value))
                .collect(),
            parameters,
        },
        root_locus: DeclaredSweep {
            parameter: "gain".to_string(),
            values,
            selected_value: selected,
        },
    }
}

/// The affine second-order definition of control-studio-routh-hurwitz.test.ts: (s² + 3s + 2) with the controller gain.
fn affine_second_order() -> DeclaredControlAnalysis {
    let mut definition = gain_definition(
        vec![1.0],
        vec![1.0],
        vec![1.0, 3.0, 2.0],
        (-10.0, 10.0),
        vec![-1.0, 1.0, 4.0],
        1.0,
    );
    definition.controller.numerator = vec![DeclaredCoefficient::Parameter(ParameterCoefficient {
        parameter: "gain".to_string(),
        scale: None,
    })];
    definition
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

#[test]
fn the_declared_affine_intervals_follow_the_hurwitz_rules() {
    let input = affine_second_order();
    let before = serde_json::to_string(&input).expect("json");
    let second = calculate_declared_routh(&input, Some(-1.0)).expect("the reading");
    assert_eq!(second.polynomial.coefficients, vec![1.0, 3.0, 1.0]);
    assert_eq!(second.interval.status, IntervalStatus::Qualified);
    assert_eq!(second.interval.kind, IntervalKind::Open);
    assert_eq!(second.interval.lower, Some(-2.0));
    assert_eq!(second.interval.upper, None);
    assert_eq!(
        second.allowed_domain,
        AllowedDomain {
            minimum: -10.0,
            maximum: 10.0
        }
    );
    assert!(
        second.provenance.contains("provided")
            && second.provenance.contains("generic-2")
            && second.provenance.contains("validated")
    );
    assert_eq!(
        serde_json::to_string(&input).expect("json"),
        before,
        "the analysis changes no definition"
    );
    let mut first_order = affine_second_order();
    first_order.plant.denominator = vec![1.0, 2.0];
    assert_eq!(
        calculate_declared_routh(&first_order, None)
            .expect("reading")
            .interval
            .lower,
        Some(-2.0)
    );
    let mut tiny = affine_second_order();
    tiny.plant.denominator = vec![1.0, 1.0];
    tiny.controller.numerator = vec![DeclaredCoefficient::Parameter(ParameterCoefficient {
        parameter: "gain".to_string(),
        scale: Some(1e-15),
    })];
    assert_eq!(
        calculate_declared_routh(&tiny, None)
            .expect("reading")
            .interval
            .status,
        IntervalStatus::Uncertain
    );
    let mut impossible = affine_second_order();
    impossible.plant.denominator = vec![1.0, 0.0];
    impossible.controller.numerator = vec![DeclaredCoefficient::Constant(0.0)];
    assert_eq!(
        calculate_declared_routh(&impossible, None)
            .expect("reading")
            .interval
            .kind,
        IntervalKind::Empty
    );
}

#[test]
fn a_delay_is_refused_and_a_variable_leading_coefficient_gives_its_half_line() {
    let mut delayed = affine_second_order();
    delayed.plant.delay_seconds = 0.1;
    assert_eq!(
        calculate_declared_routh(&delayed, None)
            .expect("reading")
            .polynomial
            .status,
        RouthStatus::Rejected
    );
    let mut variable = affine_second_order();
    variable.plant.denominator = vec![1.0, 0.0];
    variable.controller.denominator = vec![DeclaredCoefficient::Parameter(ParameterCoefficient {
        parameter: "gain".to_string(),
        scale: None,
    })];
    variable.controller.numerator = vec![DeclaredCoefficient::Constant(1.0)];
    variable.root_locus.values = vec![1.0, 2.0];
    variable.root_locus.selected_value = 1.0;
    variable
        .controller
        .parameters
        .get_mut("gain")
        .expect("gain")
        .minimum = 1.0;
    let interval = calculate_declared_routh(&variable, None)
        .expect("reading")
        .interval;
    assert_eq!(interval.status, IntervalStatus::Qualified);
    assert_eq!(interval.lower, Some(0.0));
    assert_eq!(interval.upper, None);
}

#[test]
fn a_cubic_has_one_bounded_stability_interval_and_its_boundaries_are_not_stable() {
    let mut input = affine_second_order();
    input.plant.denominator = vec![1.0, 2.0, 3.0, 0.0];
    let result = calculate_declared_routh(&input, Some(1.0)).expect("reading");
    assert_eq!(result.interval.status, IntervalStatus::Qualified);
    assert_eq!(result.interval.kind, IntervalKind::Open);
    assert_eq!(result.interval.lower, Some(0.0));
    assert_eq!(result.interval.upper, Some(6.0));
    assert_eq!(
        calculate_declared_routh(&input, Some(0.0))
            .expect("reading")
            .polynomial
            .stable,
        Some(false)
    );
    assert_eq!(
        calculate_declared_routh(&input, Some(6.0))
            .expect("reading")
            .polynomial
            .stable,
        Some(false)
    );
}

#[test]
fn disjoint_stability_regions_are_both_kept_and_degree_seven_is_solved() {
    let mut input = affine_second_order();
    input.plant.denominator = vec![1.0, 3.0, 3.0, 10.0];
    input.controller.numerator = vec![
        DeclaredCoefficient::Parameter(ParameterCoefficient {
            parameter: "gain".to_string(),
            scale: None,
        }),
        DeclaredCoefficient::Parameter(ParameterCoefficient {
            parameter: "gain".to_string(),
            scale: None,
        }),
        DeclaredCoefficient::Parameter(ParameterCoefficient {
            parameter: "gain".to_string(),
            scale: Some(4.0),
        }),
    ];
    let interval = calculate_declared_routh(&input, None)
        .expect("reading")
        .interval;
    assert_eq!(interval.kind, IntervalKind::Union);
    let pieces = interval.intervals.expect("the union's pieces");
    assert_eq!(pieces.len(), 2);
    assert!((pieces[0].lower.expect("bounded") + 2.5).abs() < 1e-12);
    assert!((pieces[0].upper.expect("bounded") - (-1.0 - std::f64::consts::SQRT_2)).abs() < 1e-12);
    assert!((pieces[1].lower.expect("bounded") - (-1.0 + std::f64::consts::SQRT_2)).abs() < 1e-12);
    assert!(interval
        .boundaries
        .expect("boundaries")
        .iter()
        .all(|boundary| !boundary.polynomial.is_empty()));
    let mut seventh = affine_second_order();
    seventh.plant.denominator = vec![1.0, 7.0, 21.0, 35.0, 35.0, 21.0, 7.0, 1.0];
    seventh.controller.numerator = vec![DeclaredCoefficient::Parameter(ParameterCoefficient {
        parameter: "gain".to_string(),
        scale: None,
    })];
    let interval = calculate_declared_routh(&seventh, None)
        .expect("reading")
        .interval;
    assert_eq!(
        interval.status,
        IntervalStatus::Qualified,
        "the solver bounds a degree-seven polynomial"
    );
    assert!(interval.lower.expect("lower") < 0.0 && interval.upper.expect("upper") > 0.0);
}
