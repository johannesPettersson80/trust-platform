//! Routh–Hurwitz analysis (N7.1 to N7.3): the table of a polynomial and its reading, and the sign-change count of its
//! first column. The mathematics is in `trust_control_math`; the words of each finding are in `analysis::texts`; this
//! module decides what a table means. The stability interval of a declared analysis, and its result at one value of the
//! swept parameter, are in `routh_interval` (N7.4).

// The exact comparisons are deliberate: a zero leading or constant coefficient, a zero entry and a zero
// denominator are the cases the criterion names, not values to be compared within a tolerance.
#![allow(clippy::float_cmp)]

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::routh::exact_table::{exact_routh_signs, ExactTable};
use trust_control_math::routh::{build, Stop, Table, ZeroRow};

use crate::analysis::declared::FixedParameter;
use crate::analysis::{limits, texts};

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
    /// The stability interval of the parameter.
    pub interval: RouthInterval,
    /// The model's source, revision and validation.
    pub provenance: String,
    /// The scope of the result.
    pub scope: String,
    /// The sentences of the result area: the reading and the stable interval, warnings first (B9.9).
    pub summary: Vec<String>,
    /// The sentences of the details disclosure: the first-column signs, the findings, the allowed domain and the derivation.
    pub details: Vec<String>,
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
pub(crate) fn rejected(coefficients: &[f64], reason: &str) -> RouthPolynomialAnalysis {
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

#[cfg(test)]
#[path = "routh_tests.rs"]
mod tests;
