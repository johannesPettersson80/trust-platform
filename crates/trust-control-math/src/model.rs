//! Linear models and their exact conversions: transfer functions and state-space matrices, the
//! controllable companion realization, the transfer polynomials of a state-space model, the
//! held-input transfer equivalent of a sampled one-state model, and the controllability and
//! observability structure with its ranks. Each function states the formula it computes.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::linalg::{
    eigenvalues, identity, multiply, numerical_rank, DenseMatrix, NumericalTolerance,
};

/// The inputs and outputs that a model is linearised about. The model's deviations are measured
/// from them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OperatingPoint {
    /// The input values at the operating point, one per model input.
    pub inputs: Vec<f64>,
    /// The output values at the operating point, one per model output.
    pub outputs: Vec<f64>,
}

/// Why coefficients do not form a valid model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConstructionError {
    /// A polynomial or a list of matrix rows is empty where a value is needed.
    #[error("empty polynomial")]
    Empty,
    /// A coefficient is not finite.
    #[error("nonfinite coefficient")]
    Nonfinite,
    /// The leading denominator coefficient is zero.
    #[error("zero leading denominator coefficient")]
    ZeroLeadingDenominator,
    /// The numerator has more coefficients than the denominator, so the function is improper.
    #[error("improper transfer function")]
    Improper,
    /// The matrices do not have the shapes `A: n×n`, `B: n×m`, `C: p×n`, `D: p×m` with `m ≥ 1`
    /// and `p ≥ 1`.
    #[error("matrix shapes disagree")]
    Shape,
    /// A sample period is not finite and positive.
    #[error("sample period is not finite and positive")]
    SampleTime,
}

/// A transfer function `N(s)/D(s)` (or `N(z)/D(z)`) with coefficients from the highest power down.
/// Both polynomials are non-empty and finite, `D` has a nonzero leading coefficient, and `N` is no
/// longer than `D` (the function is proper).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct TransferFunction {
    /// The numerator coefficients `N`.
    pub numerator: Vec<f64>,
    /// The denominator coefficients `D`.
    pub denominator: Vec<f64>,
}

impl TransferFunction {
    /// Checks the coefficients and builds the transfer function.
    ///
    /// # Errors
    /// Returns [`ConstructionError`] when a polynomial is empty or has a coefficient that is not
    /// finite, the leading denominator coefficient is zero, or the numerator is longer than the
    /// denominator.
    pub fn new(numerator: Vec<f64>, denominator: Vec<f64>) -> Result<Self, ConstructionError> {
        if numerator.is_empty() || denominator.is_empty() {
            return Err(ConstructionError::Empty);
        }
        if !numerator
            .iter()
            .chain(&denominator)
            .all(|value| value.is_finite())
        {
            return Err(ConstructionError::Nonfinite);
        }
        if denominator[0] == 0.0 {
            return Err(ConstructionError::ZeroLeadingDenominator);
        }
        if numerator.len() > denominator.len() {
            return Err(ConstructionError::Improper);
        }
        Ok(Self {
            numerator,
            denominator,
        })
    }
}

/// The matrices of a linear state-space model `ẋ = Ax + Bu`, `y = Cx + Du`, or its discrete form
/// `x(k+1) = Ax(k) + Bu(k)`, `y(k) = Cx(k) + Du(k)`. `A` is `n×n`, `B` is `n×m`, `C` is `p×n` and
/// `D` is `p×m`, with `m ≥ 1` and `p ≥ 1`. `n` may be zero for a static gain, whose `A` and `B`
/// are empty.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct StateSpace {
    /// The state matrix `A`.
    pub a: DenseMatrix,
    /// The input matrix `B`.
    pub b: DenseMatrix,
    /// The output matrix `C`.
    pub c: DenseMatrix,
    /// The feedthrough matrix `D`.
    pub d: DenseMatrix,
}

impl StateSpace {
    /// Checks the shapes and values of the four matrices and builds the model.
    ///
    /// # Errors
    /// Returns [`ConstructionError::Shape`] when the shapes disagree or there is no input or output,
    /// and [`ConstructionError::Nonfinite`] when a value is not finite.
    pub fn new(
        a: DenseMatrix,
        b: DenseMatrix,
        c: DenseMatrix,
        d: DenseMatrix,
    ) -> Result<Self, ConstructionError> {
        let states = a.len();
        let inputs = d.first().map_or(0, Vec::len);
        let outputs = d.len();
        let shaped = a.iter().all(|row| row.len() == states)
            && b.len() == states
            && b.iter().all(|row| row.len() == inputs)
            && c.len() == outputs
            && c.iter().all(|row| row.len() == states)
            && d.iter().all(|row| row.len() == inputs);
        if inputs == 0 || outputs == 0 || !shaped {
            return Err(ConstructionError::Shape);
        }
        if !a
            .iter()
            .chain(&b)
            .chain(&c)
            .chain(&d)
            .flatten()
            .all(|value| value.is_finite())
        {
            return Err(ConstructionError::Nonfinite);
        }
        Ok(Self { a, b, c, d })
    }

    /// The number of states `n`.
    #[must_use]
    pub fn states(&self) -> usize {
        self.a.len()
    }

    /// The number of inputs `m`.
    #[must_use]
    pub fn inputs(&self) -> usize {
        self.d.first().map_or(0, Vec::len)
    }

    /// The number of outputs `p`.
    #[must_use]
    pub fn outputs(&self) -> usize {
        self.d.len()
    }
}

/// Deserialises a transfer function through [`TransferFunction::new`], so a stored function is
/// checked exactly like a new one.
impl<'de> Deserialize<'de> for TransferFunction {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Parts {
            numerator: Vec<f64>,
            denominator: Vec<f64>,
        }
        let parts = Parts::deserialize(deserializer)?;
        Self::new(parts.numerator, parts.denominator).map_err(serde::de::Error::custom)
    }
}

/// Deserialises a state-space model through [`StateSpace::new`], so a stored model is checked
/// exactly like a new one and its matrices always have the shapes the functions here index.
impl<'de> Deserialize<'de> for StateSpace {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Parts {
            a: DenseMatrix,
            b: DenseMatrix,
            c: DenseMatrix,
            d: DenseMatrix,
        }
        let parts = Parts::deserialize(deserializer)?;
        Self::new(parts.a, parts.b, parts.c, parts.d).map_err(serde::de::Error::custom)
    }
}

/// Whether a model runs in continuous time or at a sample period.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum Timing {
    /// Continuous time.
    Continuous,
    /// Discrete time, with the sample period of the model.
    Discrete {
        /// The sample period `T` in seconds: finite and positive.
        sample_time_seconds: f64,
    },
}

impl Timing {
    /// A discrete timing with the given sample period.
    ///
    /// # Errors
    /// Returns [`ConstructionError::SampleTime`] unless the period is finite and positive.
    pub fn discrete(sample_time_seconds: f64) -> Result<Self, ConstructionError> {
        if sample_time_seconds.is_finite() && sample_time_seconds > 0.0 {
            Ok(Self::Discrete {
                sample_time_seconds,
            })
        } else {
            Err(ConstructionError::SampleTime)
        }
    }
}

/// The controllable companion realization of a transfer function, with its normalised
/// coefficients.
#[derive(Debug, Clone, PartialEq)]
pub struct Companion {
    /// The number of states: the denominator order.
    pub order: usize,
    /// The numerator divided by the leading denominator coefficient and padded with leading zeros
    /// to `order + 1` entries.
    pub numerator: Vec<f64>,
    /// The denominator divided by its leading coefficient, so its first entry is one.
    pub denominator: Vec<f64>,
    /// The realization `(A, B, C, D)`.
    pub system: StateSpace,
}

/// The controllable companion realization of `N(s)/D(s)`, with the normalised coefficients
/// `d̂ = D/D₀` and `n̂ = N/D₀` padded to the denominator order. Then `A` has `−d̂₁ … −d̂ₙ` in its
/// first row and ones on its subdiagonal, `B = e₁`, `C = [n̂₁ − d̂₁n̂₀, …, n̂ₙ − d̂ₙn̂₀]` and
/// `D = [n̂₀]`. A constant `N/D` (order zero) has empty `A` and `B`, and `D` is the gain.
///
/// # Errors
/// Returns [`ConstructionError`] when a polynomial is empty, the leading denominator coefficient is
/// zero, the numerator is longer than the denominator, or a normalised coefficient is not finite.
pub fn transfer_function_state_space(
    numerator: &[f64],
    denominator: &[f64],
) -> Result<Companion, ConstructionError> {
    let Some(&leading) = denominator.first() else {
        return Err(ConstructionError::Empty);
    };
    if numerator.is_empty() {
        return Err(ConstructionError::Empty);
    }
    if leading == 0.0 {
        return Err(ConstructionError::ZeroLeadingDenominator);
    }
    if numerator.len() > denominator.len() {
        return Err(ConstructionError::Improper);
    }
    let order = denominator.len() - 1;
    let normalized_denominator: Vec<f64> =
        denominator.iter().map(|value| value / leading).collect();
    let mut normalized_numerator = vec![0.0; order + 1 - numerator.len()];
    normalized_numerator.extend(numerator.iter().map(|value| value / leading));
    if !normalized_denominator
        .iter()
        .chain(&normalized_numerator)
        .all(|value| value.is_finite())
    {
        return Err(ConstructionError::Nonfinite);
    }
    let a: DenseMatrix = (0..order)
        .map(|row| {
            (0..order)
                .map(|column| {
                    if row == 0 {
                        -normalized_denominator[column + 1]
                    } else {
                        f64::from(u8::from(column + 1 == row))
                    }
                })
                .collect()
        })
        .collect();
    let b: DenseMatrix = (0..order)
        .map(|row| vec![f64::from(u8::from(row == 0))])
        .collect();
    let c: DenseMatrix = vec![(0..order)
        .map(|index| {
            normalized_numerator[index + 1]
                - normalized_denominator[index + 1] * normalized_numerator[0]
        })
        .collect()];
    let d: DenseMatrix = vec![vec![normalized_numerator[0]]];
    Ok(Companion {
        order,
        numerator: normalized_numerator,
        denominator: normalized_denominator,
        system: StateSpace::new(a, b, c, d)?,
    })
}

/// Why a model cannot be converted to a transfer function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConversionError {
    /// The model is not single input and single output.
    #[error("not single input and single output")]
    NotSiso,
    /// The model has more states than the caller's order limit.
    #[error("order above the limit")]
    OrderLimit,
    /// The conversion's coefficients are not finite, or its final residual is not small compared
    /// with the coefficients.
    #[error("numerical uncertainty")]
    NumericalUncertainty,
    /// The sampled model is not a positive one-state model without direct feedthrough.
    #[error("unsupported sampled model")]
    UnsupportedSampledModel,
    /// The held-input equivalent of the sampled model is not finite.
    #[error("no finite equivalent")]
    NoFiniteEquivalent,
}

/// The exact transfer polynomials of a single-input, single-output state-space model, by the
/// `Faddeev–LeVerrier` recursion. With `M₀ = I`, the coefficients are `cₖ = −tr(AMₖ₋₁)/k` and
/// `Mₖ = AMₖ₋₁ + cₖI`, so `N(s)/D(s) = C(sI − A)⁻¹B + D` with `D(s) = [1, c₁, …, cₙ]` and
/// `N = [D, D·cₖ + CMₖ₋₁B]`. The leading zeros of `N` are removed. The recursion ends with
/// `Mₙ = 0` (Cayley–Hamilton), which is checked against the coefficient scale.
///
/// # Errors
/// Returns [`ConversionError::NotSiso`] for more than one input or output,
/// [`ConversionError::OrderLimit`] when there are more than `maximum_order` states, and
/// [`ConversionError::NumericalUncertainty`] when a coefficient is not finite or `‖Mₙ‖∞` exceeds
/// `10⁻⁸` times the largest coefficient magnitude.
pub fn state_space_transfer_polynomials(
    system: &StateSpace,
    maximum_order: usize,
) -> Result<TransferFunction, ConversionError> {
    if system.inputs() != 1 || system.outputs() != 1 {
        return Err(ConversionError::NotSiso);
    }
    let order = system.states();
    if order > maximum_order {
        return Err(ConversionError::OrderLimit);
    }
    let direct = system.d[0][0];
    let mut adjugate = identity(order);
    let mut denominator = vec![1.0];
    let mut numerator = vec![direct];
    let mut scale = 1.0_f64;
    for step in 1..=order {
        let next = multiply(&system.a, &adjugate);
        let trace: f64 = (0..order).map(|index| next[index][index]).sum();
        // `step` is at most the order limit, far below 2^53, so the conversion is exact.
        #[allow(clippy::cast_precision_loss)]
        let coefficient = -trace / step as f64;
        denominator.push(if coefficient == 0.0 { 0.0 } else { coefficient });
        let markov = markov_term(system, &adjugate);
        numerator.push(direct * coefficient + markov);
        scale = scale.max(coefficient.abs()).max(largest_magnitude(&next));
        adjugate = next;
        for (index, row) in adjugate.iter_mut().enumerate() {
            row[index] += coefficient;
        }
    }
    let finite = scale.is_finite()
        && denominator
            .iter()
            .chain(&numerator)
            .chain(adjugate.iter().flatten())
            .all(|value| value.is_finite());
    if !finite || largest_magnitude(&adjugate) > 1e-8 * scale {
        return Err(ConversionError::NumericalUncertainty);
    }
    while numerator.len() > 1 && numerator[0] == 0.0 {
        numerator.remove(0);
    }
    Ok(TransferFunction {
        numerator,
        denominator,
    })
}

/// The held-input transfer equivalent of a positive one-state sampled model
/// `x(k+1) = a·x(k) + b·u(k)`, `y(k) = c·x(k)` with sample period `T`. Its pole is `p = ln(a)/T`
/// and its input gain is `g = b·p/(a − 1)` (`g = b/T` when `a = 1`), so `G(s) = c·g/(s − p)`. The
/// model must have one state, one input, one output, `a > 0` and no direct feedthrough (`d = 0`).
///
/// # Errors
/// Returns [`ConversionError::UnsupportedSampledModel`] when a condition above fails, and
/// [`ConversionError::NoFiniteEquivalent`] when the pole or the gain is not finite.
pub fn sampled_one_state_transfer(
    system: &StateSpace,
    sample_time_seconds: f64,
) -> Result<TransferFunction, ConversionError> {
    let supported = system.states() == 1
        && system.inputs() == 1
        && system.outputs() == 1
        && system.a[0][0] > 0.0
        && system.d[0][0] == 0.0;
    if !supported {
        return Err(ConversionError::UnsupportedSampledModel);
    }
    let transition = system.a[0][0];
    let pole = transition.ln() / sample_time_seconds;
    let input_gain = if transition - 1.0 == 0.0 {
        1.0 / sample_time_seconds
    } else {
        pole / (transition - 1.0)
    };
    let gain = system.c[0][0] * (system.b[0][0] * input_gain);
    if !pole.is_finite() || !gain.is_finite() {
        return Err(ConversionError::NoFiniteEquivalent);
    }
    Ok(TransferFunction {
        numerator: vec![gain],
        denominator: vec![1.0, if pole == 0.0 { 0.0 } else { -pole }],
    })
}

/// A structural matrix and its numerical rank.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RankedMatrix {
    /// The numerical rank (see [`numerical_rank`]).
    pub rank: usize,
    /// The matrix.
    pub matrix: DenseMatrix,
}

/// The controllability and observability matrices of a state-space model, each with its rank.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StateStructure {
    /// The controllability matrix `[B, AB, …, A^{n−1}B]`, `n × n·m`.
    pub controllability: RankedMatrix,
    /// The observability matrix `[C; CA; …; CA^{n−1}]`, `n·p × n`.
    pub observability: RankedMatrix,
}

/// Why the structure of a model is not available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StructureError {
    /// The state matrix does not pass the eigenvalue calculation.
    #[error("structure did not qualify")]
    NotQualified,
    /// A structural matrix has a value that is not finite, or its rank cannot be computed.
    #[error("structure is inconsistent")]
    Inconsistent,
}

/// The controllability matrix `[B, AB, …, A^{n−1}B]` and the observability matrix
/// `[C; CA; …; CA^{n−1}]`, each with its numerical rank. The state matrix must pass the eigenvalue
/// calculation of [`eigenvalues`] first.
///
/// # Errors
/// Returns [`StructureError::NotQualified`] when the eigenvalue calculation refuses `A`, and
/// [`StructureError::Inconsistent`] when a matrix is not finite or its rank cannot be computed.
pub fn state_structure(
    system: &StateSpace,
    tolerance: &NumericalTolerance,
) -> Result<StateStructure, StructureError> {
    eigenvalues(&system.a, tolerance).map_err(|_| StructureError::NotQualified)?;
    let controllability = controllability_matrix(system);
    let observability = observability_matrix(system);
    let controllability_rank =
        numerical_rank(&controllability, tolerance).map_err(|_| StructureError::Inconsistent)?;
    let observability_rank =
        numerical_rank(&observability, tolerance).map_err(|_| StructureError::Inconsistent)?;
    if !controllability
        .iter()
        .chain(&observability)
        .flatten()
        .all(|value| value.is_finite())
    {
        return Err(StructureError::Inconsistent);
    }
    Ok(StateStructure {
        controllability: RankedMatrix {
            rank: controllability_rank,
            matrix: controllability,
        },
        observability: RankedMatrix {
            rank: observability_rank,
            matrix: observability,
        },
    })
}

impl StateStructure {
    /// Whether the structure has the shapes of a model with `states` states, `inputs` inputs and
    /// `outputs` outputs: controllability `n × n·m`, observability `n·p × n`, both ranks at most
    /// `n`, and every value finite.
    #[must_use]
    pub fn fits(&self, states: usize, inputs: usize, outputs: usize) -> bool {
        ranked_fits(&self.controllability, states, states * inputs, states)
            && ranked_fits(&self.observability, states * outputs, states, states)
    }
}

fn ranked_fits(ranked: &RankedMatrix, rows: usize, columns: usize, states: usize) -> bool {
    ranked.rank <= states
        && ranked.matrix.len() == rows
        && ranked
            .matrix
            .iter()
            .all(|row| row.len() == columns && row.iter().all(|value| value.is_finite()))
}

fn controllability_matrix(system: &StateSpace) -> DenseMatrix {
    let mut power = system.b.clone();
    let mut blocks = Vec::with_capacity(system.states());
    for _ in 0..system.states() {
        blocks.push(power.clone());
        power = multiply(&system.a, &power);
    }
    (0..system.states())
        .map(|row| {
            blocks
                .iter()
                .flat_map(|block| block[row].iter().copied())
                .collect()
        })
        .collect()
}

fn observability_matrix(system: &StateSpace) -> DenseMatrix {
    let mut power = system.c.clone();
    let mut blocks = Vec::with_capacity(system.states());
    for _ in 0..system.states() {
        blocks.push(power.clone());
        power = multiply(&power, &system.a);
    }
    blocks.into_iter().flatten().collect()
}

/// `C · M · B` for the single input and output: `Σᵢ cᵢ Σⱼ Mᵢⱼ bⱼ`.
fn markov_term(system: &StateSpace, matrix: &[Vec<f64>]) -> f64 {
    system.c[0]
        .iter()
        .enumerate()
        .map(|(row, entry)| {
            entry
                * matrix[row]
                    .iter()
                    .zip(&system.b)
                    .map(|(value, b_row)| value * b_row[0])
                    .sum::<f64>()
        })
        .sum::<f64>()
}

fn largest_magnitude(matrix: &[Vec<f64>]) -> f64 {
    matrix
        .iter()
        .flatten()
        .fold(0.0, |largest, value| largest.max(value.abs()))
}

#[cfg(test)]
mod tests;
