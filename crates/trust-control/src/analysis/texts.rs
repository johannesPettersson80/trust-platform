//! The sentences of Control Studio's analysis tools (A11.1). Each sentence names the values it reports.

use trust_control_math::classical::ClassicalError;

/// The Routh input is invalid: a coefficient, the degree or the leading coefficient.
pub const ROUTH_INVALID: &str = "The polynomial must have real finite descending coefficients, degree 1–16, and a nonzero leading coefficient.";
/// The Routh table is refused for a declared delay.
pub const ROUTH_DELAY_UNSUPPORTED: &str =
    "Routh-Hurwitz analysis does not approximate declared delay models.";
/// The declared Routh scope.
pub const ROUTH_SCOPE: &str = "Declared continuous-time, delay-free scalar negative-feedback model; nonlinear limits, sample/hold, and the running controller are not changed.";
/// The declared model or sweep is invalid.
pub const DECLARED_INVALID: &str = "The declared model or parameter sweep is invalid.";
/// The selected parameter value must be finite.
pub const SELECTED_NOT_FINITE: &str = "Enter a finite parameter value.";
/// The declared root locus could not qualify a gain.
pub const ROOT_SOLVER_FAILED: &str = "Root solver could not qualify this model.";
/// The declared root locus approximation, without delay.
pub const DECLARED_APPROXIMATION: &str =
    "Declared continuous linear model; nonlinear limits and task sample/hold are not represented.";
/// The declared root locus approximation, with delay.
pub const DECLARED_DELAY_PADE: &str = " Delay uses first-order Padé approximation.";
/// The declared root locus approximation, without delay.
pub const DECLARED_NO_DELAY: &str = " No model delay.";
/// The declared controller denominator vanishes at a gain.
#[must_use]
pub fn declared_denominator_zero(parameter: &str, gain: f64) -> String {
    format!("Controller denominator is zero at {parameter} = {gain}.")
}

/// The general feedback's model, channels or units are invalid.
pub const FEEDBACK_INVALID: &str = "invalid-feedback-model-channels-units-or-domain";
/// The general feedback's selected model has no supported shape for a classical margin.
pub const MARGIN_NEEDS_TRANSFER: &str =
    "Classical margin estimates currently require continuous transfer-function models.";
/// The frequency range cannot support a bounded crossover estimate.
pub const MARGIN_RANGE_UNSUPPORTED: &str =
    "The frequency range cannot support a bounded crossover estimate.";
/// A pure delay makes the phase change too fast for the grid.
pub const MARGIN_DELAY_TOO_FAST: &str = "Pure-delay phase change exceeds the bounded grid resolution; narrow the frequency range before estimating margins.";
/// No gain crossover on the finite range.
pub const MARGIN_NO_CROSSOVER: &str =
    "No qualified gain crossover was found in the finite requested range.";
/// The margin estimate's scope.
pub const MARGIN_SCOPE: &str = "Sampling-limited first-crossover estimate in the requested range (at most 512 samples). Other crossovers may exist. No phase crossover means gain margin is unavailable, not infinite. Margins alone do not qualify stability.";
/// The feedback closed loop has a delay, so its finite poles are unavailable.
pub const FEEDBACK_DELAY_POLES: &str = "Pure delay is exact in frequency response; finite closed-loop poles require an explicit delay approximation, which was not supplied.";
/// The scope of the closed-loop poles of a model feedback.
pub const FEEDBACK_CLOSED_LOOP_SCOPE: &str = "Exact poles of the declared selected-channel negative-feedback realization. Other inputs held fixed. This does not establish full MIMO feedback stability; undeclared nonlinear limits, filters and continuous-model sample/hold are not represented.";
/// The scope of the closed-loop poles of a declared controller: its first sentence.
pub const FEEDBACK_DECLARED_POLES_SCOPE: &str = "Exact roots of the selected plant denominator times the declared controller denominator plus their numerator product";
/// The part of the declared poles' scope that a state-space plant adds.
pub const FEEDBACK_DECLARED_STATE_SUFFIX: &str =
    "; the full SISO state denominator is retained through the existing linear transfer conversion";
/// The last sentence of the declared poles' scope.
pub const FEEDBACK_DECLARED_POLES_TAIL: &str = ". This is an analytic declared model result; nonlinear limits, sample/hold and runtime state are not represented.";
/// The Nyquist scope of a model feedback with transfer functions.
pub const FEEDBACK_NYQUIST_TRANSFER_SCOPE: &str = "The shared Nyquist stability owner evaluated a bounded closed contour for the selected continuous transfer models with their declared delay; encirclement and pole counts are reported with its numerical limits.";
/// The Nyquist scope of a sampled feedback.
pub const FEEDBACK_NYQUIST_SAMPLED_SCOPE: &str = "Finite sampled positive-frequency curve; conjugate negative-frequency points are shown for the real model. A full contour is unavailable for state-space selections without a compatible transfer conversion.";
/// The Nyquist scope of a declared feedback.
pub const FEEDBACK_NYQUIST_DECLARED_SCOPE: &str = "The shared Nyquist stability owner evaluated the declared controller loop with its exact plant delay; encirclement and pole counts are reported with its numerical limits.";
/// The declared controller's frequency response could not be qualified.
pub const DECLARED_CONTROLLER_RESPONSE: &str =
    "Declared controller frequency response could not be qualified.";
/// The declared plant is not a supported SISO model.
pub const DECLARED_PLANT_UNSUPPORTED: &str =
    "Declared rational feedback requires a supported SISO plant.";
/// The declared feedback channels do not share the plant's frequency grid.
pub const DECLARED_GRID_MISMATCH: &str =
    "Declared feedback channels do not share a frequency grid.";
/// The declared closed-loop characteristic polynomial could not be solved.
pub const DECLARED_CHARACTERISTIC_UNSOLVED: &str =
    "Declared closed-loop characteristic polynomial could not be solved.";
/// The response is not finite.
pub const FEEDBACK_NONFINITE: &str = "nonfinite-feedback-response";
/// A feedthrough of the algebraic loop is singular.
pub const FEEDBACK_SINGULAR_LOOP: &str = "singular-algebraic-loop";
/// The frequency grid of a feedback is invalid.
pub const FEEDBACK_GRID: &str = "invalid-frequency-grid";
/// A sampled plant's grid reaches the Nyquist frequency.
pub const FEEDBACK_ALIAS: &str = "alias-risk";

/// The lead/lag request is incomplete or out of range.
pub const LEAD_LAG_INPUT: &str = "Lead/lag design requires a selected model and bounded settings.";
/// The lead/lag design is refused.
#[must_use]
pub fn lead_lag_rejected(reason: &str) -> String {
    format!("Lead/lag design was rejected: {reason}.")
}
/// The saved lead/lag analysis belongs to another model.
pub const LEAD_LAG_OTHER_MODEL: &str = "Saved lead/lag analysis belongs to another selected model.";
/// The saved lead/lag analysis is invalid.
pub const LEAD_LAG_SAVED_INVALID: &str = "Saved lead/lag analysis is invalid.";

/// The power of `s` as a student reads it: `s`, `s²`, `s¹⁰` (the Control Studio power notation).
#[must_use]
pub fn power_of_s(power: usize) -> String {
    const SUPERSCRIPT_DIGITS: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
    if power == 1 {
        return "s".to_string();
    }
    let digits: String = power
        .to_string()
        .chars()
        .map(|digit| {
            let index = usize::try_from(digit.to_digit(10).unwrap_or(0)).unwrap_or(0);
            SUPERSCRIPT_DIGITS[index]
        })
        .collect();
    format!("s{digits}")
}

/// The sentences of the Routh issue kinds (N7.3), naming the row by its power of `s`.
#[must_use]
pub fn all_zero_row_pending(power: usize) -> String {
    format!(
        "The {} row is all zero; its auxiliary-polynomial case requires separate analysis.",
        power_of_s(power)
    )
}

/// The all-zero row resolved by the auxiliary polynomial's derivative.
#[must_use]
pub fn all_zero_row_resolved(power: usize) -> String {
    format!("The {} row is all zero. The row above it defines an auxiliary polynomial whose roots are symmetric about the origin; the exact table replaces the zero row with that polynomial's derivative.", power_of_s(power))
}

/// A zero first-column pivot.
#[must_use]
pub fn zero_pivot(power: usize) -> String {
    format!(
        "The {} row has a zero first-column pivot; no epsilon substitution was made.",
        power_of_s(power)
    )
}

/// A numerically small first-column pivot.
#[must_use]
pub fn near_zero_pivot(power: usize) -> String {
    format!("The {} first-column pivot is too small relative to the polynomial scale for a confident count.", power_of_s(power))
}

/// A row that overflowed.
#[must_use]
pub fn overflow(power: usize) -> String {
    format!(
        "The {} row overflowed or lost finite numerical representation.",
        power_of_s(power)
    )
}

/// A first-column entry near zero.
pub const NEAR_ZERO_ENTRY: &str = "A first-column entry is near zero relative to the polynomial scale, so the sign-change count is numerically uncertain.";
/// The origin pole of a zero constant coefficient.
pub const ORIGIN_POLE: &str = "The zero constant coefficient establishes an origin pole, so asymptotic stability is excluded.";
/// The exact table resolved a numerically small pivot as an all-zero row.
pub const EXACT_ALL_ZERO_RESOLVED: &str = "Exact arithmetic resolves the numerically small pivot as an auxiliary-polynomial all-zero row.";
/// The exact table resolved a numerically small pivot as a zero pivot.
pub const EXACT_ZERO_PIVOT_RESOLVED: &str = "Exact arithmetic resolves the numerically small pivot as a zero pivot; the formal positive infinitesimal determines its limiting sign.";
/// The interpretation of an origin pole.
pub const INTERPRET_ORIGIN: &str = "An origin pole excludes strict stability. ";
/// The interpretation of imaginary-axis poles from the auxiliary polynomial.
pub const INTERPRET_IMAGINARY_AXIS: &str =
    "The auxiliary polynomial establishes imaginary-axis poles and excludes strict stability. ";
/// The interpretation of a stable first column.
pub const INTERPRET_STABLE: &str = "No right-half-plane poles were established by the Routh first column; the declared polynomial is asymptotically stable.";
/// The count of right-half-plane poles in the exact table.
#[must_use]
pub fn interpret_exact_count(count: i64, method: &str) -> String {
    format!("{count} right-half-plane poles established by {method}.")
}
/// The count of right-half-plane poles from the float first column.
#[must_use]
pub fn interpret_float_count(count: i64) -> String {
    let plural = if count == 1 { "" } else { "s" };
    format!("{count} right-half-plane pole{plural} established by first-column sign changes.")
}
/// The method words of the exact table, by the kind of its special cases.
pub const METHOD_AUXILIARY_AND_EPSILON: &str =
    "the exact Routh recurrence with the auxiliary polynomial and a formal ε";
/// See [`METHOD_AUXILIARY_AND_EPSILON`].
pub const METHOD_AUXILIARY: &str = "the exact Routh recurrence with the auxiliary polynomial";
/// See [`METHOD_AUXILIARY_AND_EPSILON`].
pub const METHOD_EPSILON: &str = "the exact formal-ε Routh recurrence";

/// The interval sentences of N7.4.
pub const INTERVAL_DELAY: &str = "Declared delay is unsupported for this criterion.";
/// The characteristic leading coefficient is zero.
pub const INTERVAL_LEADING_ZERO: &str = "The characteristic leading coefficient is zero.";
/// The affine coefficient calculation is not finite.
pub const INTERVAL_NOT_FINITE: &str = "The affine coefficient calculation is not finite.";
/// A strict coefficient inequality is impossible.
pub const INTERVAL_IMPOSSIBLE: &str =
    "At least one strict Routh coefficient inequality is impossible.";
/// The strict coefficient half-lines do not intersect.
pub const INTERVAL_NO_INTERSECTION: &str = "The strict coefficient half-lines do not intersect.";
/// Every strict coefficient inequality holds over the axis.
pub const INTERVAL_ALL_REAL: &str =
    "Every strict coefficient inequality holds over the real parameter axis.";
/// The intersection of strict half-lines.
pub const INTERVAL_INTERSECTED: &str = "Intersected strict coefficient-sign half-lines after normalizing the leading coefficient positive.";
/// A coefficient inequality is ill-conditioned.
pub const INTERVAL_ILL_CONDITIONED: &str =
    "A coefficient inequality is too ill-conditioned to classify confidently.";
/// The degree of a Hurwitz region is out of range.
pub const INTERVAL_DEGREE: &str = "Characteristic degree must be between 1 and 16.";
/// A Hurwitz determinant inequality is identically zero.
pub const INTERVAL_DEGENERATE: &str =
    "A strict Hurwitz determinant inequality is identically zero.";
/// The exact region's derivation.
pub const INTERVAL_EXACT: &str = "Exact rational Bareiss determinants and Sturm root isolation establish every open stable component. Decimal bounds are display approximations; the retained polynomial and rational enclosures identify the exact boundaries. Leading-coefficient zeros and determinant zeros are excluded.";

/// The reason code of a refused classical calculation, in the words the analysis rules report.
#[must_use]
pub fn classical_reason(error: ClassicalError) -> &'static str {
    match error {
        ClassicalError::InvalidInput => "nonfinite-input",
        ClassicalError::InvalidPolynomial => "invalid-polynomial",
        ClassicalError::SolverFailed => "solver-failed",
        ClassicalError::NonfiniteResponse => "nonfinite-response",
        ClassicalError::PoleOnContour => "pole-on-contour",
        ClassicalError::InsufficientContourClosure => "insufficient-contour-closure",
        ClassicalError::ContourNearCriticalPoint => "contour-near-critical-point",
    }
}
