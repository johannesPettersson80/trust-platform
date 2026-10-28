//! Tests of the stability interval of a declared analysis and of its Routh–Hurwitz result at one value (N7.1, N7.4).

use super::*;
use crate::analysis::declared::{
    DeclaredCoefficient, DeclaredController, DeclaredParameter, DeclaredPlant, DeclaredSweep,
    ParameterCoefficient, PlantKind,
};
use crate::analysis::routh::{AllowedDomain, IntervalKind, IntervalStatus, RouthStatus};
use crate::model::{ModelSource, ModelValidation};
use std::collections::BTreeMap;

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

/// The disjoint-regions definition: the plant `1 / (s³ + 3s² + 3s + 10)` with the gain `K` in the controller numerator `K, K, 4K`. Its
/// characteristic polynomial is `s³ + (3 + K)s² + (3 + K)s + (10 + 4K)`, stable for `−2.5 < K < −1 − √2` and for `K > −1 + √2`.
fn disjoint_regions() -> DeclaredControlAnalysis {
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
    input
}

#[test]
fn disjoint_stability_regions_are_both_kept_and_degree_seven_is_solved() {
    let input = disjoint_regions();
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

/// A rational text of an enclosure's end, `p` or `p/q`, as a decimal: its numerator divided by its denominator.
fn decimal(text: &str) -> f64 {
    let (numerator, denominator) = text.split_once('/').unwrap_or((text, "1"));
    numerator.parse::<f64>().expect("a numerator")
        / denominator.parse::<f64>().expect("a denominator")
}

/// A rational text of an enclosure's end as an exact numerator and positive denominator.
fn exact(text: &str) -> (i128, i128) {
    let (numerator, denominator) = text.split_once('/').unwrap_or((text, "1"));
    (
        numerator.parse().expect("a numerator"),
        denominator.parse().expect("a denominator"),
    )
}

/// Whether the rational `left` is at most `right`, both with positive denominators, compared exactly.
fn at_most(left: (i128, i128), right: (i128, i128)) -> bool {
    left.0 * right.1 <= right.0 * left.1
}

#[test]
fn each_stability_boundary_is_a_decimal_with_its_exact_enclosure() {
    // The facts of the removed test "shows each stability boundary as a decimal and keeps its exact enclosure for inspection"
    // (control-studio-routh-hurwitz.test.ts), on the disjoint regions: the boundary polynomial 2, 15, 35, 19, −15, the decimal
    // of each boundary within the tolerance of its root that the algebraic roots above use, and the exact rational enclosure.
    let boundaries = calculate_declared_routh(&disjoint_regions(), None)
        .expect("reading")
        .interval
        .boundaries
        .expect("the boundaries");
    let roots = [
        -3.0,
        -2.5,
        -1.0 - std::f64::consts::SQRT_2,
        -1.0 + std::f64::consts::SQRT_2,
    ];
    assert_eq!(boundaries.len(), roots.len(), "{boundaries:?}");
    for (boundary, root) in boundaries.iter().zip(roots) {
        let coefficients: Vec<f64> = boundary
            .polynomial
            .iter()
            .map(|text| decimal(text))
            .collect();
        assert_eq!(coefficients.len(), 5, "{:?}", boundary.polynomial);
        // Up to a factor, the polynomial is 2, 15, 35, 19, −15.
        for (value, expected) in coefficients.iter().zip([2.0, 15.0, 35.0, 19.0, -15.0]) {
            assert!(
                (value / coefficients[0] - expected / 2.0).abs() < 1e-12,
                "{:?}",
                boundary.polynomial
            );
        }
        let (lower, upper) = (decimal(&boundary.lower), decimal(&boundary.upper));
        assert!(lower <= upper, "{} to {}", boundary.lower, boundary.upper);
        assert!(
            (lower - root).abs() < 1e-12 && (upper - root).abs() < 1e-12,
            "the root {root} has the enclosure {} to {}",
            boundary.lower,
            boundary.upper
        );
    }
    // −3 and −5/2 are rational: the enclosure of −3 holds it exactly, and the enclosure of −5/2 is a single point.
    assert!(
        at_most(exact(&boundaries[0].lower), (-3, 1)),
        "{}",
        boundaries[0].lower
    );
    assert!(
        at_most((-3, 1), exact(&boundaries[0].upper)),
        "{}",
        boundaries[0].upper
    );
    assert_eq!(exact(&boundaries[1].lower), (-5, 2));
    assert_eq!(exact(&boundaries[1].upper), (-5, 2));
}
