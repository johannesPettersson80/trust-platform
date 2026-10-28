//! The Routh table of a real polynomial in floating point (N7.2 and N7.3).
//!
//! The table holds the coefficients, every computed row with its calculations, and the place where
//! the construction stops. A row that is all zero is replaced by the derivative of the auxiliary
//! polynomial of the row above (N7.3), so the construction goes on. Its interpretation (the signs,
//! the count of sign changes and the words of each finding) belongs to `trust-control`.
//!
//! Exact tables with a formal infinitesimal for the singular cases are in [`exact_table`], and the
//! parameter regions of the stability of a polynomial with affine coefficients are in [`regions`].

pub mod exact_table;
pub mod regions;

/// One calculation of a Routh row: `result = (b0·a_next − a0·b_next) / b0` (N7.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Calculation {
    /// The column of the entry that the calculation produces.
    pub column: usize,
    /// The first entry of the row two above (`A[0]`).
    pub a0: f64,
    /// The entry of the row two above, in the next column (`A[j+1]`).
    pub a_next: f64,
    /// The first entry of the row above, the pivot (`B[0]`).
    pub b0: f64,
    /// The entry of the row above, in the next column (`B[j+1]`).
    pub b_next: f64,
    /// The entry that the calculation produces.
    pub result: f64,
}

/// A computed row of the table and its power of `s`.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// The power of `s` of the row: the first row holds the highest power.
    pub power: usize,
    /// The entries, padded with zeros.
    pub entries: Vec<f64>,
    /// The calculations that produced the entries, in column order. Empty for the first two rows.
    pub calculations: Vec<Calculation>,
}

/// Why the float construction stopped before its last row (N7.3). Each stop names the power of `s`
/// of the row at fault.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// The pivot of the row at `power` is exactly zero. No epsilon is substituted in the float table.
    ZeroPivot {
        /// The power of `s` of the row whose pivot is zero.
        power: usize,
    },
    /// The pivot of the row at `power` is too small against the polynomial's scale for a confident count.
    NearZeroPivot {
        /// The power of `s` of the row whose pivot is small.
        power: usize,
    },
    /// A calculation of the row at `power` is not finite.
    Overflow {
        /// The power of `s` of the row being computed.
        power: usize,
    },
}

/// A row that is all zero (N7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZeroRow {
    /// The row at `power` was all zero before its pivot was used. Its auxiliary-polynomial derivative
    /// replaced it, and the construction went on.
    Replaced {
        /// The power of `s` of the replaced row.
        power: usize,
    },
    /// The row at `power` was computed and is all zero. The construction ends there.
    Computed {
        /// The power of `s` of the all-zero row.
        power: usize,
    },
}

/// The float table of a polynomial and the place where its construction stopped.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    /// The computed rows, from the highest power down.
    pub rows: Vec<Row>,
    /// The all-zero rows that were met, in order.
    pub zero_rows: Vec<ZeroRow>,
    /// Where the construction stopped before its last row, if it did.
    pub stop: Option<Stop>,
}

/// Builds the float table of `coefficients`: real, descending, of degree at least one, with a nonzero
/// leading coefficient. A pivot whose size is at most `relative_tolerance` times the largest coefficient
/// counts as near zero, and the construction stops there (N7.3).
///
/// A polynomial of fewer than two coefficients has no table.
#[must_use]
pub fn build(coefficients: &[f64], relative_tolerance: f64) -> Table {
    let degree = coefficients.len().saturating_sub(1);
    if degree == 0 {
        return Table {
            rows: Vec::new(),
            zero_rows: Vec::new(),
            stop: None,
        };
    }
    let columns = degree / 2 + 1;
    let mut table = vec![vec![0.0; columns]; degree + 1];
    let coefficient = |index: usize| coefficients.get(index).copied().unwrap_or(0.0);
    table[0] = (0..columns).map(|column| coefficient(2 * column)).collect();
    table[1] = (0..columns)
        .map(|column| coefficient(2 * column + 1))
        .collect();
    let scale = coefficients
        .iter()
        .fold(0.0_f64, |largest, value| largest.max(value.abs()));
    let mut calculations: Vec<Vec<Calculation>> = vec![Vec::new(); degree + 1];
    let mut zero_rows = Vec::new();
    let mut stop = None;
    let mut computed = 2;
    for row in 2..=degree {
        let prior = table[row - 2].clone();
        if table[row - 1].iter().all(|value| is_exact_zero(*value)) {
            zero_rows.push(ZeroRow::Replaced {
                power: degree - row + 1,
            });
            table[row - 1] = auxiliary_derivative_row(&prior, degree - row + 2, columns);
        }
        let pivot_row = table[row - 1].clone();
        let pivot = pivot_row[0];
        if is_exact_zero(pivot) {
            stop = Some(Stop::ZeroPivot {
                power: degree - row + 1,
            });
            break;
        }
        if pivot.abs() <= scale * relative_tolerance {
            stop = Some(Stop::NearZeroPivot {
                power: degree - row + 1,
            });
            break;
        }
        let mut overflow = false;
        for column in 0..columns - 1 {
            let result = (pivot * prior[column + 1] - prior[0] * pivot_row[column + 1]) / pivot;
            calculations[row].push(Calculation {
                column,
                a0: prior[0],
                a_next: prior[column + 1],
                b0: pivot,
                b_next: pivot_row[column + 1],
                result,
            });
            if !result.is_finite() {
                overflow = true;
                break;
            }
            table[row][column] = result;
        }
        if overflow {
            stop = Some(Stop::Overflow {
                power: degree - row,
            });
            break;
        }
        computed = row + 1;
        if table[row].iter().all(|value| is_exact_zero(*value)) {
            zero_rows.push(ZeroRow::Computed {
                power: degree - row,
            });
            break;
        }
    }
    let rows = (0..computed)
        .map(|index| Row {
            power: degree - index,
            entries: table[index].clone(),
            calculations: calculations[index].clone(),
        })
        .collect();
    Table {
        rows,
        zero_rows,
        stop,
    }
}

/// The derivative row of the auxiliary polynomial of the row above (N7.3). With the row above of power
/// `above_power`, entry `j` becomes `row_above[j] · (above_power − 2j)` where that power is positive, and
/// zero otherwise. The result has `width` entries.
#[must_use]
pub fn auxiliary_derivative_row(row_above: &[f64], above_power: usize, width: usize) -> Vec<f64> {
    (0..width)
        .map(|index| {
            match above_power
                .checked_sub(2 * index)
                .filter(|power| *power > 0)
            {
                Some(power) => row_above.get(index).copied().unwrap_or(0.0) * as_f64(power),
                None => 0.0,
            }
        })
        .collect()
}

/// A count of rows or powers as a floating-point number. The counts of a Routh table are small.
fn as_f64(value: usize) -> f64 {
    f64::from(u32::try_from(value).unwrap_or(u32::MAX))
}

/// Whether a value is exactly zero, `+0` or `-0`. A pivot is exactly zero only when it is.
#[allow(clippy::float_cmp)] // An exact zero is the table's rule for a zero pivot, not a tolerance.
fn is_exact_zero(value: f64) -> bool {
    value == 0.0
}

#[cfg(test)]
#[path = "routh_tests.rs"]
mod tests;
