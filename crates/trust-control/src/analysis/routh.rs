//! Routh–Hurwitz analysis (N7.1 to N7.4): the table of a polynomial and its reading, the stability of the
//! declared characteristic polynomial at one value of the swept parameter, and the parameter intervals on
//! which it is stable. The mathematics is in `trust_control_math`; the words of each finding are in
//! `analysis::texts`; this module decides what a table means.

// The exact comparisons are deliberate: a zero leading or constant coefficient, a zero entry and a zero
// denominator are the cases the criterion names, not values to be compared within a tolerance.
#![allow(clippy::float_cmp)]

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::affine::{convolve_numeric, solve_positive, sum, Affine, PositiveSolution};
use trust_control_math::exact_polynomial::ExactError;
use trust_control_math::polynomial::{add_aligned, convolve};
use trust_control_math::routh::exact_table::{exact_routh_signs, ExactTable};
use trust_control_math::routh::regions::{hurwitz_region, HurwitzRegion, RegionKind};
use trust_control_math::routh::{build, Stop, Table, ZeroRow};

use crate::analysis::declared::{
    check_declared, substitute, DeclaredCoefficient, DeclaredControlAnalysis, DeclaredController,
    FixedParameter,
};
use crate::analysis::{limits, texts};
use crate::failure::{Cause, Failure, NextAction};

/// The relative size of a pivot, against the largest coefficient, below which it counts as zero.
const RELATIVE_PIVOT_TOLERANCE: f64 = 1e-12;

/// The status of a Routh reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RouthStatus {
    /// The sign changes are established.
    Qualified,
    /// The polynomial has roots on the imaginary axis or at the origin.
    Boundary,
    /// A numerical or special case prevents a definitive count.
    Uncertain,
    /// The polynomial is not a valid input.
    Rejected,
}

/// The sign of a first-column entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RouthSign {
    /// Positive.
    Positive,
    /// Negative.
    Negative,
    /// Zero.
    Zero,
}

/// One calculation of a Routh row: `result = (b0·a_next − a0·b_next) / b0`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouthCalculation {
    /// The column of the entry.
    pub column: usize,
    /// The first entry of the row two above.
    pub a0: f64,
    /// The entry of the row two above, in the next column.
    pub a_next: f64,
    /// The pivot.
    pub b0: f64,
    /// The entry of the row above, in the next column.
    pub b_next: f64,
    /// The entry that the calculation produces.
    pub result: f64,
}

/// A row of a Routh table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouthRow {
    /// The power of `s` of the row.
    pub power: usize,
    /// The entries of the row.
    pub entries: Vec<f64>,
    /// The calculations that produced the entries.
    pub calculations: Vec<RouthCalculation>,
}

/// A first-column entry and its sign.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FirstColumnEntry {
    /// The value. In an exact table it is the limit sign, −1, 0 or 1.
    pub value: f64,
    /// The sign of the value.
    pub sign: RouthSign,
}

/// What a finding of a Routh table is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RouthIssueKind {
    /// The polynomial is not valid.
    InvalidPolynomial,
    /// The constant coefficient is zero: a pole at the origin.
    OriginPole,
    /// A pivot is exactly zero.
    ZeroPivot,
    /// A row is all zero.
    AllZeroRow,
    /// A pivot is too small against the scale of the polynomial.
    NearZeroPivot,
    /// A first-column entry is too small against the scale of the polynomial.
    NearZeroEntry,
    /// A calculation overflowed.
    NumericalOverflow,
}

/// A finding of a Routh table and its explanation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouthIssue {
    /// What the finding is about.
    pub kind: RouthIssueKind,
    /// The explanation, naming the row by its power of `s`.
    pub explanation: String,
}

/// A row of an exact table, with its formal entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SymbolicRow {
    /// The power of `s` of the row.
    pub power: usize,
    /// The formal entries.
    pub entries: Vec<String>,
}

/// The reading of one polynomial's Routh table (N7.2, N7.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouthPolynomialAnalysis {
    /// The status of the reading.
    pub status: RouthStatus,
    /// The coefficients, from the highest power down.
    pub coefficients: Vec<f64>,
    /// The rows of the float table.
    pub rows: Vec<RouthRow>,
    /// The first column with the sign of each entry.
    pub first_column: Vec<FirstColumnEntry>,
    /// The number of right-half-plane poles established, when the reading qualifies.
    pub sign_changes: Option<usize>,
    /// Whether the polynomial is established as asymptotically stable.
    pub stable: Option<bool>,
    /// The interpretation, in words.
    pub interpretation: String,
    /// The findings of the table.
    pub issues: Vec<RouthIssue>,
    /// The rows of the exact table, when it resolved a special case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbolic_rows: Option<Vec<SymbolicRow>>,
}

/// The end of a stable interval in the swept parameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouthIntervalPiece {
    /// The lower end, when bounded.
    pub lower: Option<f64>,
    /// The upper end, when bounded.
    pub upper: Option<f64>,
    /// The boundary index of the lower end.
    pub lower_boundary: Option<usize>,
    /// The boundary index of the upper end.
    pub upper_boundary: Option<usize>,
}

/// An exact boundary root of a stable region, as text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouthBoundary {
    /// The coefficients of the boundary polynomial, as exact rationals.
    pub polynomial: Vec<String>,
    /// The exact lower end of the root's enclosure.
    pub lower: String,
    /// The exact upper end of the root's enclosure.
    pub upper: String,
}

/// The status of a parameter interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum IntervalStatus {
    /// The interval is established.
    Qualified,
    /// The analysis does not support the polynomial.
    Unsupported,
    /// The analysis cannot classify the interval with confidence.
    Uncertain,
}

/// The shape of a parameter interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum IntervalKind {
    /// One bounded or half-bounded interval.
    Open,
    /// No parameter value.
    Empty,
    /// Every parameter value.
    AllReal,
    /// Not available.
    Unavailable,
    /// Several disjoint intervals.
    Union,
}

/// The stability interval of the swept parameter (N7.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouthInterval {
    /// The status of the interval.
    pub status: IntervalStatus,
    /// The shape of the interval.
    pub kind: IntervalKind,
    /// The lower end of a single interval, when bounded.
    pub lower: Option<f64>,
    /// The upper end of a single interval, when bounded.
    pub upper: Option<f64>,
    /// The intervals of a union, from the exact region.
    pub intervals: Option<Vec<RouthIntervalPiece>>,
    /// The exact boundary roots of the region.
    pub boundaries: Option<Vec<RouthBoundary>>,
    /// The strict inequalities that define the interval.
    pub inequalities: Vec<String>,
    /// The derivation, in words.
    pub derivation: String,
}

/// The allowed domain of the swept parameter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AllowedDomain {
    /// The smallest allowed value.
    pub minimum: f64,
    /// The largest allowed value.
    pub maximum: f64,
}

/// The Routh–Hurwitz result of a declared analysis at one value of the swept parameter (N7.1 to N7.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeclaredRouthResult {
    /// The swept parameter.
    pub parameter: String,
    /// The unit of the swept parameter.
    pub parameter_unit: String,
    /// The value at which the polynomial was read.
    pub selected_value: f64,
    /// The allowed domain of the parameter.
    pub allowed_domain: AllowedDomain,
    /// The parameters that stay fixed.
    pub fixed_parameters: BTreeMap<String, FixedParameter>,
    /// The reading of the characteristic polynomial.
    pub polynomial: RouthPolynomialAnalysis,
    /// The rows of the table, the same as the reading's.
    pub table: Vec<RouthRow>,
    /// The stability interval of the parameter.
    pub interval: RouthInterval,
    /// The model's source, revision and validation.
    pub provenance: String,
    /// The scope of the result.
    pub scope: String,
}

/// The reading of a polynomial's Routh table (N7.2, N7.3). A polynomial that is not valid is rejected.
#[must_use]
#[allow(clippy::too_many_lines)] // The table and its reading (N7.2, N7.3) in the order the numerical annex states them.
pub fn analyze_routh_polynomial(coefficients: &[f64]) -> RouthPolynomialAnalysis {
    if coefficients.len() < 2
        || coefficients.len() > limits::ROUTH_MAXIMUM_COEFFICIENTS
        || !coefficients.iter().all(|value| value.is_finite())
        || coefficients[0] == 0.0
        || coefficients.iter().all(|value| *value == 0.0)
    {
        return rejected(coefficients, texts::ROUTH_INVALID);
    }
    let degree = coefficients.len() - 1;
    let scale = coefficients
        .iter()
        .fold(0.0_f64, |largest, value| largest.max(value.abs()));
    let table = build(coefficients, RELATIVE_PIVOT_TOLERANCE);
    let mut findings = Vec::new();
    if coefficients[degree] == 0.0 {
        findings.push(Finding::new(
            RouthIssueKind::OriginPole,
            None,
            texts::ORIGIN_POLE,
        ));
    }
    for zero_row in &table.zero_rows {
        let power = match zero_row {
            ZeroRow::Replaced { power } | ZeroRow::Computed { power } => *power,
        };
        findings.push(Finding::new(
            RouthIssueKind::AllZeroRow,
            Some(power),
            &texts::all_zero_row_pending(power),
        ));
    }
    if let Some(stop) = table.stop {
        findings.push(stop_finding(stop));
    }
    let rows = table_rows(&table);
    let float_first_column: Vec<f64> = table.rows.iter().map(|row| row.entries[0]).collect();
    let special = if findings.iter().any(|finding| {
        matches!(
            finding.kind,
            RouthIssueKind::ZeroPivot | RouthIssueKind::AllZeroRow | RouthIssueKind::NearZeroPivot
        )
    }) {
        exact_routh_signs(coefficients)
    } else {
        None
    };
    if let Some(exact) = special.filter(|exact| {
        !exact.uncertain
            && (exact.zero_pivot || exact.all_zero_row)
            && exact.signs.iter().all(|sign| *sign != 0)
    }) {
        return exact_reading(coefficients, rows, findings, &exact);
    }
    if !findings.iter().any(|finding| {
        matches!(
            finding.kind,
            RouthIssueKind::NearZeroPivot | RouthIssueKind::NumericalOverflow
        )
    }) && float_first_column
        .iter()
        .any(|value| *value != 0.0 && value.abs() <= scale * RELATIVE_PIVOT_TOLERANCE)
    {
        findings.push(Finding::new(
            RouthIssueKind::NearZeroEntry,
            None,
            texts::NEAR_ZERO_ENTRY,
        ));
    }
    let first_column: Vec<FirstColumnEntry> = float_first_column
        .iter()
        .map(|value| entry(*value))
        .collect();
    let has_origin = findings
        .iter()
        .any(|finding| finding.kind == RouthIssueKind::OriginPole);
    let status = if findings.iter().any(|finding| {
        matches!(
            finding.kind,
            RouthIssueKind::NearZeroPivot
                | RouthIssueKind::NearZeroEntry
                | RouthIssueKind::NumericalOverflow
        )
    }) {
        RouthStatus::Uncertain
    } else if has_origin {
        RouthStatus::Boundary
    } else if !findings.is_empty() {
        RouthStatus::Uncertain
    } else {
        RouthStatus::Qualified
    };
    let issues = into_issues(findings);
    if status != RouthStatus::Qualified {
        let interpretation = issues
            .iter()
            .map(|issue| issue.explanation.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        return RouthPolynomialAnalysis {
            status,
            coefficients: coefficients.to_vec(),
            rows,
            first_column,
            sign_changes: None,
            stable: has_origin.then_some(false),
            interpretation,
            issues,
            symbolic_rows: None,
        };
    }
    let changes = sign_changes(&first_column);
    RouthPolynomialAnalysis {
        status,
        coefficients: coefficients.to_vec(),
        rows,
        first_column,
        sign_changes: Some(changes),
        stable: Some(changes == 0),
        interpretation: if changes == 0 {
            texts::INTERPRET_STABLE.to_string()
        } else {
            texts::interpret_float_count(i64::try_from(changes).unwrap_or(i64::MAX))
        },
        issues,
        symbolic_rows: None,
    }
}

/// The reading of a table resolved by the exact arithmetic: the auxiliary polynomial and the formal `ε`.
fn exact_reading(
    coefficients: &[f64],
    rows: Vec<RouthRow>,
    mut findings: Vec<Finding>,
    exact: &ExactTable,
) -> RouthPolynomialAnalysis {
    if exact.all_zero_row
        && !findings
            .iter()
            .any(|finding| finding.kind == RouthIssueKind::AllZeroRow)
    {
        findings.push(Finding::new(
            RouthIssueKind::AllZeroRow,
            None,
            texts::EXACT_ALL_ZERO_RESOLVED,
        ));
    } else if exact.zero_pivot
        && !findings
            .iter()
            .any(|finding| finding.kind == RouthIssueKind::ZeroPivot)
    {
        findings.push(Finding::new(
            RouthIssueKind::ZeroPivot,
            None,
            texts::EXACT_ZERO_PIVOT_RESOLVED,
        ));
    }
    let first_column: Vec<FirstColumnEntry> = exact
        .signs
        .iter()
        .map(|sign| entry(f64::from(*sign)))
        .collect();
    let changes = sign_changes(&first_column);
    let has_origin = findings
        .iter()
        .any(|finding| finding.kind == RouthIssueKind::OriginPole);
    let boundary_words = if has_origin {
        texts::INTERPRET_ORIGIN
    } else if exact.boundary {
        texts::INTERPRET_IMAGINARY_AXIS
    } else {
        ""
    };
    let interpretation = format!(
        "{boundary_words}{}",
        texts::interpret_exact_count(i64::try_from(changes).unwrap_or(i64::MAX), method(exact))
    );
    let status = if exact.boundary {
        RouthStatus::Boundary
    } else {
        RouthStatus::Qualified
    };
    let issues = resolved_issues(findings);
    RouthPolynomialAnalysis {
        status,
        coefficients: coefficients.to_vec(),
        rows,
        first_column,
        sign_changes: Some(changes),
        stable: Some(status == RouthStatus::Qualified && changes == 0),
        interpretation,
        issues,
        symbolic_rows: Some(
            exact
                .rows
                .iter()
                .map(|row| SymbolicRow {
                    power: row.power,
                    entries: row.entries.clone(),
                })
                .collect(),
        ),
    }
}

/// The method words of an exact reading.
fn method(exact: &ExactTable) -> &'static str {
    match (exact.all_zero_row, exact.zero_pivot) {
        (true, true) => texts::METHOD_AUXILIARY_AND_EPSILON,
        (true, false) => texts::METHOD_AUXILIARY,
        _ => texts::METHOD_EPSILON,
    }
}

/// The findings of an exact reading: an all-zero row is resolved by the auxiliary polynomial's derivative, and
/// the zero pivots are resolved by the formal positive infinitesimal, so each says how it was resolved.
fn resolved_issues(findings: Vec<Finding>) -> Vec<RouthIssue> {
    findings
        .into_iter()
        .map(|finding| {
            if finding.kind == RouthIssueKind::AllZeroRow {
                if let Some(power) = finding.power {
                    return RouthIssue {
                        kind: finding.kind,
                        explanation: texts::all_zero_row_resolved(power),
                    };
                }
            }
            into_issue(finding)
        })
        .collect()
}

/// The reading of a polynomial that is not valid, or that the criterion does not take, with its reason.
fn rejected(coefficients: &[f64], reason: &str) -> RouthPolynomialAnalysis {
    RouthPolynomialAnalysis {
        status: RouthStatus::Rejected,
        coefficients: coefficients.to_vec(),
        rows: Vec::new(),
        first_column: Vec::new(),
        sign_changes: None,
        stable: None,
        interpretation: reason.to_string(),
        issues: vec![RouthIssue {
            kind: RouthIssueKind::InvalidPolynomial,
            explanation: reason.to_string(),
        }],
        symbolic_rows: None,
    }
}

/// A finding before its explanation is final. Its power of `s` lets the explanation name the row.
struct Finding {
    kind: RouthIssueKind,
    power: Option<usize>,
    explanation: String,
}

impl Finding {
    fn new(kind: RouthIssueKind, power: Option<usize>, explanation: &str) -> Self {
        Self {
            kind,
            power,
            explanation: explanation.to_string(),
        }
    }
}

/// The finding of a float construction that stopped (N7.3).
fn stop_finding(stop: Stop) -> Finding {
    match stop {
        Stop::ZeroPivot { power } => Finding::new(
            RouthIssueKind::ZeroPivot,
            Some(power),
            &texts::zero_pivot(power),
        ),
        Stop::NearZeroPivot { power } => Finding::new(
            RouthIssueKind::NearZeroPivot,
            Some(power),
            &texts::near_zero_pivot(power),
        ),
        Stop::Overflow { power } => Finding::new(
            RouthIssueKind::NumericalOverflow,
            Some(power),
            &texts::overflow(power),
        ),
    }
}

fn into_issue(finding: Finding) -> RouthIssue {
    RouthIssue {
        kind: finding.kind,
        explanation: finding.explanation,
    }
}

fn into_issues(findings: Vec<Finding>) -> Vec<RouthIssue> {
    findings.into_iter().map(into_issue).collect()
}

/// The rows of a float table, with their calculations.
fn table_rows(table: &Table) -> Vec<RouthRow> {
    table
        .rows
        .iter()
        .map(|row| RouthRow {
            power: row.power,
            entries: row.entries.clone(),
            calculations: row
                .calculations
                .iter()
                .map(|calculation| RouthCalculation {
                    column: calculation.column,
                    a0: calculation.a0,
                    a_next: calculation.a_next,
                    b0: calculation.b0,
                    b_next: calculation.b_next,
                    result: calculation.result,
                })
                .collect(),
        })
        .collect()
}

/// The sign of a first-column value.
fn entry(value: f64) -> FirstColumnEntry {
    FirstColumnEntry {
        value,
        sign: if value > 0.0 {
            RouthSign::Positive
        } else if value < 0.0 {
            RouthSign::Negative
        } else {
            RouthSign::Zero
        },
    }
}

/// The number of sign changes along a first column.
fn sign_changes(column: &[FirstColumnEntry]) -> usize {
    column
        .windows(2)
        .filter(|pair| pair[0].sign != pair[1].sign)
        .count()
}

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
    let table = polynomial.rows.clone();
    Ok(DeclaredRouthResult {
        parameter: parameter.to_string(),
        parameter_unit: configured.unit.clone(),
        selected_value: selected,
        allowed_domain,
        fixed_parameters,
        polynomial,
        table,
        interval,
        provenance,
        scope: texts::ROUTH_SCOPE.to_string(),
    })
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
#[path = "routh_tests.rs"]
mod tests;
