//! The stability interval of a declared analysis's swept parameter, and the Routh–Hurwitz result of its characteristic
//! polynomial at one value of that parameter (N7.1, N7.4): the affine coefficients of the characteristic polynomial, the
//! half-lines of the degree one and two cases, and the exact Hurwitz region of higher degrees. The reading of a table is
//! in `analysis::routh`; the sentences of the result are written here, from the reading and the interval.

// The exact comparisons are deliberate: a zero leading coefficient is the case that N7.4 names, not a tolerance.
#![allow(clippy::float_cmp)]

use trust_control_math::affine::{convolve_numeric, solve_positive, sum, Affine, PositiveSolution};
use trust_control_math::exact_polynomial::ExactError;
use trust_control_math::polynomial::{add_aligned, convolve};
use trust_control_math::routh::regions::{hurwitz_region, HurwitzRegion, RegionKind};

use crate::analysis::declared::{
    check_declared, substitute, DeclaredCoefficient, DeclaredControlAnalysis, DeclaredController,
    FixedParameter,
};
use crate::analysis::routh::{
    analyze_routh_polynomial, rejected, AllowedDomain, DeclaredRouthResult, IntervalKind,
    IntervalStatus, RouthBoundary, RouthInterval, RouthIntervalPiece, RouthPolynomialAnalysis,
    RouthSign,
};
use crate::analysis::{limits, texts};
use crate::failure::{Cause, Failure, NextAction};
use crate::tuning::texts::engineering;

/// The Routh–Hurwitz result of the declared characteristic polynomial at `selected` (or at the definition's
/// selected value), with its parameter interval (N7.1 to N7.4).
///
/// # Errors
/// Returns the declared-invalid sentence for an invalid definition, and the finite-value sentence for a
/// selected value that is not finite.
pub fn calculate_declared_routh(
    input: &DeclaredControlAnalysis,
    selected: Option<f64>,
) -> Result<DeclaredRouthResult, Failure> {
    check_declared(input)?;
    let parameter = input.root_locus.parameter.as_str();
    let selected = selected.unwrap_or(input.root_locus.selected_value);
    if !selected.is_finite() {
        return Err(Failure::new(
            texts::SELECTED_NOT_FINITE,
            Cause::Settings,
            NextAction::None,
        ));
    }
    let controller = &input.controller;
    let configured = &controller.parameters[parameter];
    let fixed_parameters = controller
        .parameters
        .iter()
        .filter(|(name, _)| name.as_str() != parameter)
        .map(|(name, value)| {
            (
                name.clone(),
                FixedParameter {
                    value: value.value,
                    unit: value.unit.clone(),
                },
            )
        })
        .collect();
    let provenance = format!(
        "{} model {} · {}",
        input.plant.source.word(),
        input.plant.revision,
        input.plant.validation.word()
    );
    let allowed_domain = AllowedDomain {
        minimum: configured.minimum,
        maximum: configured.maximum,
    };
    let (polynomial, interval) = if input.plant.delay_seconds > 0.0 {
        (
            rejected(&[], texts::ROUTH_DELAY_UNSUPPORTED),
            unavailable(texts::INTERVAL_DELAY),
        )
    } else {
        let characteristic = characteristic(input, parameter, selected);
        (
            analyze_routh_polynomial(&characteristic),
            affine_interval(input, parameter),
        )
    };
    let (summary, details) =
        routh_words(parameter, selected, &polynomial, &interval, allowed_domain);
    Ok(DeclaredRouthResult {
        parameter: parameter.to_string(),
        parameter_unit: configured.unit.clone(),
        selected_value: selected,
        allowed_domain,
        fixed_parameters,
        polynomial,
        interval,
        provenance,
        scope: texts::ROUTH_SCOPE.to_string(),
        summary,
        details,
    })
}

/// The sentences of a Routh–Hurwitz result (N7.2 to N7.4): the reading and the stable interval for the result area, and
/// the first-column signs, the findings, the allowed domain and the derivation for the details disclosure.
fn routh_words(
    parameter: &str,
    selected: f64,
    polynomial: &RouthPolynomialAnalysis,
    interval: &RouthInterval,
    allowed_domain: AllowedDomain,
) -> (Vec<String>, Vec<String>) {
    let summary = vec![
        format!(
            "{parameter} = {}. {}",
            engineering(selected, None),
            polynomial.interpretation
        ),
        format!("Stable interval: {}.", interval_bounds(interval)),
    ];
    let details = vec![format!(
        "{} Allowed domain [{}, {}]. {}",
        routh_detail_text(polynomial),
        engineering(allowed_domain.minimum, None),
        engineering(allowed_domain.maximum, None),
        interval.derivation
    )];
    (summary, details)
}

/// The first-column signs of the table that the reading came from, then each finding that shaped the count.
fn routh_detail_text(polynomial: &RouthPolynomialAnalysis) -> String {
    let exact = polynomial
        .symbolic_rows
        .as_ref()
        .is_some_and(|rows| !rows.is_empty());
    let table = if exact {
        "the exact table"
    } else {
        "the table"
    };
    let signs: Vec<&str> = polynomial
        .first_column
        .iter()
        .map(|entry| sign_word(entry.sign))
        .collect();
    let mut sentences = vec![format!(
        "First-column signs of {table}: {}.",
        signs.join(", ")
    )];
    sentences.extend(
        polynomial
            .issues
            .iter()
            .map(|issue| issue.explanation.clone()),
    );
    sentences.join(" ")
}

fn sign_word(sign: RouthSign) -> &'static str {
    match sign {
        RouthSign::Positive => "positive",
        RouthSign::Negative => "negative",
        RouthSign::Zero => "zero",
    }
}

/// The stable interval in words: its bounds, with −∞ and ∞ at an open end, or its kind when it is no single interval.
fn interval_bounds(interval: &RouthInterval) -> String {
    let bound = |value: Option<f64>, open_end: &str| {
        value.map_or_else(|| open_end.to_string(), |value| engineering(value, None))
    };
    match (&interval.intervals, interval.kind) {
        (Some(pieces), _) => pieces
            .iter()
            .map(|piece| {
                format!(
                    "({}, {})",
                    bound(piece.lower, "−∞"),
                    bound(piece.upper, "∞")
                )
            })
            .collect::<Vec<_>>()
            .join(" ∪ "),
        (None, IntervalKind::Open) => format!(
            "({}, {})",
            bound(interval.lower, "−∞"),
            bound(interval.upper, "∞")
        ),
        (None, kind) => kind_word(kind).to_string(),
    }
}

fn kind_word(kind: IntervalKind) -> &'static str {
    match kind {
        IntervalKind::Open => "open",
        IntervalKind::Empty => "empty",
        IntervalKind::AllReal => "all-real",
        IntervalKind::Unavailable => "unavailable",
        IntervalKind::Union => "union",
    }
}

/// The characteristic polynomial `Dp·Dc + Np·Nc` with the parameter at `selected` (N7.1).
fn characteristic(input: &DeclaredControlAnalysis, parameter: &str, selected: f64) -> Vec<f64> {
    let controller = &input.controller;
    let denominator = substitute(&controller.denominator, parameter, selected, controller);
    let numerator = substitute(&controller.numerator, parameter, selected, controller);
    add_aligned(
        &convolve(&input.plant.denominator, &denominator),
        &convolve(&input.plant.numerator, &numerator),
    )
}

/// The stability interval of the parameter, from the affine coefficients of the characteristic polynomial
/// (N7.4): the degree one and two cases by their half-lines, the others by the exact region.
#[allow(clippy::too_many_lines)] // The three cases of N7.4 side by side, so their bounds can be compared.
fn affine_interval(input: &DeclaredControlAnalysis, parameter: &str) -> RouthInterval {
    let controller = &input.controller;
    let denominator = affine_coefficients(&controller.denominator, parameter, controller);
    let numerator = affine_coefficients(&controller.numerator, parameter, controller);
    let coefficients = sum(
        &convolve_numeric(&input.plant.denominator, &denominator),
        &convolve_numeric(&input.plant.numerator, &numerator),
    );
    if !coefficients
        .iter()
        .all(|value| value.constant.is_finite() && value.slope.is_finite())
    {
        return RouthInterval {
            status: IntervalStatus::Uncertain,
            kind: IntervalKind::Unavailable,
            lower: None,
            upper: None,
            intervals: None,
            boundaries: None,
            inequalities: Vec::new(),
            derivation: texts::INTERVAL_NOT_FINITE.to_string(),
        };
    }
    let degree = coefficients.len() - 1;
    let leading = coefficients[0];
    if degree > 2 || leading.slope != 0.0 {
        return hurwitz_interval(&coefficients);
    }
    if leading.constant == 0.0 {
        return unavailable(texts::INTERVAL_LEADING_ZERO);
    }
    let factor = if leading.constant < 0.0 { -1.0 } else { 1.0 };
    let constraints: Vec<(Affine, String)> = coefficients[1..]
        .iter()
        .enumerate()
        .map(|(index, coefficient)| {
            (
                Affine {
                    constant: coefficient.constant * factor,
                    slope: coefficient.slope * factor,
                },
                format!("a{}", degree - index - 1),
            )
        })
        .collect();
    let solved: Vec<(PositiveSolution, String)> = constraints
        .iter()
        .map(|(value, label)| {
            let solution = solve_positive(*value);
            (solution, inequality(*value, label, solution))
        })
        .collect();
    let inequalities: Vec<String> = solved.iter().map(|(_, text)| text.clone()).collect();
    if solved
        .iter()
        .any(|(solution, _)| *solution == PositiveSolution::Uncertain)
    {
        return RouthInterval {
            status: IntervalStatus::Uncertain,
            kind: IntervalKind::Unavailable,
            lower: None,
            upper: None,
            intervals: None,
            boundaries: None,
            inequalities,
            derivation: texts::INTERVAL_ILL_CONDITIONED.to_string(),
        };
    }
    if solved
        .iter()
        .any(|(solution, _)| *solution == PositiveSolution::Empty)
    {
        return half_line_result(
            IntervalKind::Empty,
            None,
            None,
            inequalities,
            texts::INTERVAL_IMPOSSIBLE,
        );
    }
    let mut lower = f64::NEG_INFINITY;
    let mut upper = f64::INFINITY;
    for (solution, _) in &solved {
        match solution {
            PositiveSolution::Above(bound) => lower = lower.max(*bound),
            PositiveSolution::Below(bound) => upper = upper.min(*bound),
            _ => {}
        }
    }
    if lower >= upper {
        return half_line_result(
            IntervalKind::Empty,
            None,
            None,
            inequalities,
            texts::INTERVAL_NO_INTERSECTION,
        );
    }
    if lower == f64::NEG_INFINITY && upper == f64::INFINITY {
        return half_line_result(
            IntervalKind::AllReal,
            None,
            None,
            inequalities,
            texts::INTERVAL_ALL_REAL,
        );
    }
    let finite = |value: f64| (value.is_finite()).then_some(value);
    half_line_result(
        IntervalKind::Open,
        finite(lower),
        finite(upper),
        inequalities,
        texts::INTERVAL_INTERSECTED,
    )
}

/// The interval of a degree-one or degree-two characteristic polynomial, or of a parameter-dependent leading
/// coefficient, from the exact Hurwitz region (N7.4).
fn hurwitz_interval(coefficients: &[Affine]) -> RouthInterval {
    let degree = coefficients.len() - 1;
    let inequalities: Vec<String> = (1..=degree)
        .map(|index| format!("Δ{index}(p) / a₀(p)^{index} > 0"))
        .collect();
    if !(1..=limits::INTERVAL_MAXIMUM_DEGREE).contains(&degree) {
        return unavailable_with(IntervalStatus::Unsupported, texts::INTERVAL_DEGREE);
    }
    match hurwitz_region(coefficients) {
        Ok(HurwitzRegion::Degenerate) => RouthInterval {
            status: IntervalStatus::Qualified,
            kind: IntervalKind::Empty,
            lower: None,
            upper: None,
            intervals: None,
            boundaries: None,
            inequalities,
            derivation: texts::INTERVAL_DEGENERATE.to_string(),
        },
        Ok(HurwitzRegion::Regions {
            kind,
            intervals,
            boundaries,
        }) => {
            let single = match intervals.as_slice() {
                [only] => Some((only.lower, only.upper)),
                _ => None,
            };
            RouthInterval {
                status: IntervalStatus::Qualified,
                kind: interval_kind(kind),
                lower: single.and_then(|(lower, _)| lower),
                upper: single.and_then(|(_, upper)| upper),
                intervals: Some(
                    intervals
                        .into_iter()
                        .map(|piece| RouthIntervalPiece {
                            lower: piece.lower,
                            upper: piece.upper,
                            lower_boundary: piece.lower_boundary,
                            upper_boundary: piece.upper_boundary,
                        })
                        .collect(),
                ),
                boundaries: Some(
                    boundaries
                        .into_iter()
                        .map(|boundary| RouthBoundary {
                            polynomial: boundary
                                .polynomial
                                .iter()
                                .map(ToString::to_string)
                                .collect(),
                            lower: boundary.lower.to_string(),
                            upper: boundary.upper.to_string(),
                        })
                        .collect(),
                ),
                inequalities,
                derivation: texts::INTERVAL_EXACT.to_string(),
            }
        }
        Err(error) => RouthInterval {
            status: IntervalStatus::Uncertain,
            kind: IntervalKind::Unavailable,
            lower: None,
            upper: None,
            intervals: None,
            boundaries: None,
            inequalities,
            derivation: refusal_text(error),
        },
    }
}

/// The derivation text of an exact refusal: the sentence of the exact arithmetic.
fn refusal_text(error: ExactError) -> String {
    error.to_string()
}

fn interval_kind(kind: RegionKind) -> IntervalKind {
    match kind {
        RegionKind::Empty => IntervalKind::Empty,
        RegionKind::AllReal => IntervalKind::AllReal,
        RegionKind::Open => IntervalKind::Open,
        RegionKind::Union => IntervalKind::Union,
    }
}

/// A qualified interval of one or no half-line.
fn half_line_result(
    kind: IntervalKind,
    lower: Option<f64>,
    upper: Option<f64>,
    inequalities: Vec<String>,
    derivation: &str,
) -> RouthInterval {
    RouthInterval {
        status: IntervalStatus::Qualified,
        kind,
        lower,
        upper,
        intervals: None,
        boundaries: None,
        inequalities,
        derivation: derivation.to_string(),
    }
}

/// An interval that cannot be established for this reason.
fn unavailable(derivation: &str) -> RouthInterval {
    RouthInterval {
        status: IntervalStatus::Unsupported,
        kind: IntervalKind::Unavailable,
        lower: None,
        upper: None,
        intervals: None,
        boundaries: None,
        inequalities: Vec::new(),
        derivation: derivation.to_string(),
    }
}

/// The affine coefficients of a controller polynomial: a fixed number, or a parameter's slope when it is the
/// swept one and its stored value otherwise, times its scale (N7.4).
fn affine_coefficients(
    coefficients: &[DeclaredCoefficient],
    parameter: &str,
    controller: &DeclaredController,
) -> Vec<Affine> {
    coefficients
        .iter()
        .map(|coefficient| match coefficient {
            DeclaredCoefficient::Constant(value) => Affine::constant(*value),
            DeclaredCoefficient::Parameter(term) => {
                let scale = term.scale.unwrap_or(1.0);
                if term.parameter == parameter {
                    Affine::parameter(scale)
                } else {
                    Affine::constant(
                        scale
                            * controller
                                .parameters
                                .get(&term.parameter)
                                .map_or(0.0, |value| value.value),
                    )
                }
            }
        })
        .collect()
}

/// The text of a strict inequality `a(p) > 0` and how it is solved.
fn inequality(value: Affine, label: &str, solution: PositiveSolution) -> String {
    if !value.constant.is_finite() || !value.slope.is_finite() {
        return format!("{label}(p) > 0 is not finite");
    }
    if value.slope == 0.0 {
        return if value.constant > 0.0 {
            format!("{label}(p) = {} > 0", value.constant)
        } else {
            format!("{label}(p) = {} > 0 is impossible", value.constant)
        };
    }
    let bound = -value.constant / value.slope;
    match solution {
        PositiveSolution::Uncertain if !bound.is_finite() => {
            format!("{label}(p) > 0 has a non-finite bound")
        }
        PositiveSolution::Uncertain => {
            format!("{label}(p) > 0 has an ill-conditioned nonzero parameter slope")
        }
        _ if value.slope > 0.0 => format!("{label}(p) > 0 requires p > {bound}"),
        _ => format!("{label}(p) > 0 requires p < {bound}"),
    }
}

/// An interval with no bounds and no inequalities, for a reason.
fn unavailable_with(status: IntervalStatus, derivation: &str) -> RouthInterval {
    RouthInterval {
        status,
        kind: IntervalKind::Unavailable,
        lower: None,
        upper: None,
        intervals: None,
        boundaries: None,
        inequalities: Vec::new(),
        derivation: derivation.to_string(),
    }
}

#[cfg(test)]
#[path = "routh_interval_tests.rs"]
mod tests;
