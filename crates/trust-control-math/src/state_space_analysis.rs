//! The structure of a continuous or discrete state-space model (specification 35, C4 and A6; numerical annex N5.13 and
//! N7.11): its eigenvalues, its controllability and observability with their ranks, and whether it is stabilisable and
//! detectable. Stabilisability and detectability use the Hautus test, `rank[λI − A, B] = n` and `rank[λI − A; C] = n`
//! for every unstable eigenvalue `λ` (a stable eigenvalue, with real part below `−absolute`, needs no test).

use nalgebra::Complex;

use crate::linalg::{eigenvalues, ComplexValue, NumericalRejectionReason, NumericalTolerance};
use crate::model::{state_structure, StateSpace, StateStructure};

/// The structure of a state-space model.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // Each flag is one independent property of the model (C4), shown as such.
pub struct StateSpaceAnalysis {
    /// The eigenvalues of `A`.
    pub eigenvalues: Vec<ComplexValue>,
    /// The controllability matrix `[B, AB, …, A^{n−1}B]` and the observability matrix `[C; CA; …; CA^{n−1}]`, each with
    /// its numerical rank.
    pub structure: StateStructure,
    /// Whether the controllability rank is the number of states.
    pub controllable: bool,
    /// Whether the observability rank is the number of states.
    pub observable: bool,
    /// Whether every unstable mode is controllable (the Hautus test).
    pub stabilisable: bool,
    /// Whether every unstable mode is observable (the Hautus test).
    pub detectable: bool,
}

/// Why a state-space model has no structure analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateSpaceRefusal {
    /// The model has no states, no inputs or no outputs, its shapes disagree, or a value is not finite.
    InconsistentDimensions,
    /// The eigenvalues or the structural ranks cannot be computed.
    Numerical(NumericalRejectionReason),
}

/// The eigenvalues, the controllability and observability structure, and the stabilisability and detectability of
/// `system`.
///
/// # Errors
/// Returns [`StateSpaceRefusal::InconsistentDimensions`] when the model has no states, inputs or outputs, its shapes
/// disagree, or a value is not finite, and [`StateSpaceRefusal::Numerical`] when the eigenvalues or the ranks cannot be
/// computed.
pub fn analyze_state_space(
    system: &StateSpace,
    tolerance: &NumericalTolerance,
) -> Result<StateSpaceAnalysis, StateSpaceRefusal> {
    if !is_consistent(system) {
        return Err(StateSpaceRefusal::InconsistentDimensions);
    }
    let spectrum = eigenvalues(&system.a, tolerance)
        .map_err(|error| StateSpaceRefusal::Numerical(error.reason))?;
    let structure = state_structure(system, tolerance)
        .map_err(|_| StateSpaceRefusal::Numerical(NumericalRejectionReason::SolverFailed))?;
    let states = system.states();
    let controllable = structure.controllability.rank == states;
    let observable = structure.observability.rank == states;
    let stabilisable = controllable
        || spectrum.values.iter().all(|value| {
            value.real < -tolerance.absolute || hautus_controllable(system, *value, tolerance)
        });
    let detectable = observable
        || spectrum.values.iter().all(|value| {
            value.real < -tolerance.absolute || hautus_observable(system, *value, tolerance)
        });
    Ok(StateSpaceAnalysis {
        eigenvalues: spectrum.values,
        structure,
        controllable,
        observable,
        stabilisable,
        detectable,
    })
}

/// Whether the shapes are `A: n×n`, `B: n×m`, `C: p×n`, `D: p×m` with `n, m, p ≥ 1`, and every value is finite.
fn is_consistent(system: &StateSpace) -> bool {
    let states = system.states();
    let inputs = system.inputs();
    let outputs = system.outputs();
    let finite = [&system.a, &system.b, &system.c, &system.d]
        .iter()
        .all(|matrix| matrix.iter().flatten().all(|value| value.is_finite()));
    states > 0
        && inputs > 0
        && outputs > 0
        && finite
        && system.a.iter().all(|row| row.len() == states)
        && system.b.len() == states
        && system.b.iter().all(|row| row.len() == inputs)
        && system.c.len() == outputs
        && system.c.iter().all(|row| row.len() == states)
        && system.d.iter().all(|row| row.len() == inputs)
}

/// The Hautus test for controllability at `eigenvalue`: `rank[λI − A, B] = n`.
fn hautus_controllable(
    system: &StateSpace,
    eigenvalue: ComplexValue,
    tolerance: &NumericalTolerance,
) -> bool {
    let mut rows = shifted_rows(system, eigenvalue);
    for (row, input_row) in rows.iter_mut().zip(&system.b) {
        row.extend(input_row.iter().map(|value| Complex::new(*value, 0.0)));
    }
    complex_rank(rows, tolerance.absolute) == system.states()
}

/// The Hautus test for observability at `eigenvalue`: `rank[λI − A; C] = n`.
fn hautus_observable(
    system: &StateSpace,
    eigenvalue: ComplexValue,
    tolerance: &NumericalTolerance,
) -> bool {
    let mut rows = shifted_rows(system, eigenvalue);
    rows.extend(system.c.iter().map(|row| {
        row.iter()
            .map(|value| Complex::new(*value, 0.0))
            .collect::<Vec<_>>()
    }));
    complex_rank(rows, tolerance.absolute) == system.states()
}

/// The rows of `λI − A` as complex numbers.
fn shifted_rows(system: &StateSpace, eigenvalue: ComplexValue) -> Vec<Vec<Complex<f64>>> {
    system
        .a
        .iter()
        .enumerate()
        .map(|(row, values)| {
            values
                .iter()
                .enumerate()
                .map(|(column, value)| {
                    let diagonal = row == column;
                    let shift_real = if diagonal { eigenvalue.real } else { 0.0 };
                    let shift_imaginary = if diagonal { eigenvalue.imaginary } else { 0.0 };
                    Complex::new(shift_real - value, shift_imaginary)
                })
                .collect()
        })
        .collect()
}

/// The rank of a complex matrix by Gaussian elimination with partial pivoting. A column whose largest remaining entry is
/// not above `absolute` is skipped.
fn complex_rank(mut rows: Vec<Vec<Complex<f64>>>, absolute: f64) -> usize {
    let row_count = rows.len();
    let column_count = rows.first().map_or(0, Vec::len);
    let mut rank = 0;
    for column in 0..column_count {
        if rank == row_count {
            break;
        }
        let pivot = (rank..row_count).fold(rank, |best, row| {
            if rows[row][column].norm() > rows[best][column].norm() {
                row
            } else {
                best
            }
        });
        if rows[pivot][column].norm() <= absolute {
            continue;
        }
        rows.swap(rank, pivot);
        let (upper, lower) = rows.split_at_mut(rank + 1);
        let pivot_row = &upper[rank];
        for row in lower.iter_mut() {
            let factor = row[column] / pivot_row[column];
            for (value, pivot_value) in row[column..].iter_mut().zip(&pivot_row[column..]) {
                *value -= factor * *pivot_value;
            }
        }
        rank += 1;
    }
    rank
}

#[cfg(test)]
#[path = "state_space_analysis_tests.rs"]
mod tests;
