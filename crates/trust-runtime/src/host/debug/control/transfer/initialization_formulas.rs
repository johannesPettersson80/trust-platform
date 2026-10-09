//! Fixed controller initialization formulas evaluated from the fresh task-boundary snapshot.
use super::*;

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Formula {
    PidIntegral {
        output_symbol: String,
        process_variable: String,
        gain_symbol: String,
        weight_symbol: Option<String>,
        weight: Option<f64>,
    },
    WeightedError {
        process_variable: String,
        weight_symbol: Option<String>,
        weight: Option<f64>,
    },
    NotchHistory {
        process_variable: String,
        denominator_symbols: [String; 2],
    },
    ModelInput {
        output_symbol: String,
    },
    ModelOutput {
        process_variable: String,
    },
    ModelInputIs {
        output_symbol: String,
        value: f64,
    },
    /// A measurement times a controller parameter, such as a feedforward state `K_ff·d`.
    ScaledMeasurement {
        process_variable: String,
        gain_symbol: String,
    },
    /// Row `row` of `D⁻¹·u`: the output a decoupled group member starts at, from the committed outputs `u`.
    DecoupledOutput {
        output_symbols: Vec<String>,
        matrix_symbols: Vec<Vec<String>>,
        row: usize,
    },
}
fn guard<'a>(binding: &'a Binding, symbol: &str) -> Result<&'a NumberMember, String> {
    binding
        .guards
        .iter()
        .find(|member| member.symbol == symbol)
        .ok_or("initialization formula guard is not bound".into())
}
pub(super) fn validate(binding: &Binding, symbol: &str, formula: &Formula) -> Result<(), String> {
    let destination = state(binding, symbol)?;
    let validate_weight = |symbol: &Option<String>, weight: &Option<f64>| -> Result<(), String> {
        if symbol.is_some() && weight.is_some() || weight.is_some_and(|weight| !weight.is_finite())
        {
            return Err("invalid initialization weight".into());
        }
        if let Some(symbol) = symbol {
            if guard(binding, symbol)?.unit != "1" {
                return Err("initialization weights must be dimensionless".into());
            }
        }
        Ok(())
    };
    match formula {
        Formula::ModelInput { output_symbol } => {
            let output = std::iter::once(&binding.output)
                .chain(binding.additional_outputs.iter())
                .find(|member| &member.symbol == output_symbol)
                .ok_or("initialization actuator is not bound")?;
            if destination.unit != output.unit {
                return Err("model-input initialization units do not agree".into());
            }
            if output.iec_type == "BOOL" && (output.minimum != 0.0 || output.maximum != 1.0) {
                return Err("boolean model-input initialization bounds are invalid".into());
            }
        }
        Formula::ModelOutput { process_variable } => {
            if destination.unit != guard(binding, process_variable)?.unit {
                return Err("model-output initialization units do not agree".into());
            }
        }
        Formula::ModelInputIs {
            output_symbol,
            value,
        } => {
            let output = std::iter::once(&binding.output)
                .chain(binding.additional_outputs.iter())
                .find(|member| &member.symbol == output_symbol)
                .ok_or("initialization actuator is not bound")?;
            if destination.unit != "1"
                || !value.is_finite()
                || *value < output.minimum
                || *value > output.maximum
            {
                return Err("model-input-is initialization requires a finite declared actuator level and dimensionless BOOL state".into());
            }
        }
        Formula::PidIntegral {
            output_symbol,
            process_variable,
            gain_symbol,
            weight_symbol,
            weight,
        } => {
            let output = std::iter::once(&binding.output)
                .chain(binding.additional_outputs.iter())
                .find(|member| &member.symbol == output_symbol)
                .ok_or("initialization actuator is not bound")?;
            let process = guard(binding, process_variable)?;
            let gain = guard(binding, gain_symbol)?;
            if destination.unit != output.unit
                || !(gain.unit == format!("{}/{}", output.unit, process.unit)
                    || output.unit == process.unit && gain.unit == "1")
            {
                return Err("integral initialization units do not agree".into());
            }
            validate_weight(weight_symbol, weight)?;
        }
        Formula::WeightedError {
            process_variable,
            weight_symbol,
            weight,
        } => {
            if destination.unit != guard(binding, process_variable)?.unit {
                return Err("error-history units do not agree".into());
            }
            validate_weight(weight_symbol, weight)?;
        }
        Formula::ScaledMeasurement {
            process_variable,
            gain_symbol,
        } => {
            let measurement = guard(binding, process_variable)?;
            if guard(binding, gain_symbol)?.unit
                != format!("{}/{}", destination.unit, measurement.unit)
            {
                return Err("scaled-measurement initialization units do not agree".into());
            }
        }
        Formula::DecoupledOutput {
            output_symbols,
            matrix_symbols,
            row,
        } => {
            let size = output_symbols.len();
            if size == 0
                || size > 8
                || *row >= size
                || matrix_symbols.len() != size
                || matrix_symbols.iter().any(|entries| entries.len() != size)
            {
                return Err("decoupler matrix must be square over the bound outputs".into());
            }
            let outputs = output_symbols
                .iter()
                .map(|symbol| {
                    std::iter::once(&binding.output)
                        .chain(binding.additional_outputs.iter())
                        .find(|member| &member.symbol == symbol)
                        .ok_or_else(|| "decoupled initialization output is not bound".to_string())
                })
                .collect::<Result<Vec<_>, String>>()?;
            if destination.unit != outputs[*row].unit {
                return Err("decoupled initialization units do not agree".into());
            }
            for (input, entries) in matrix_symbols.iter().enumerate() {
                for (member, symbol) in entries.iter().enumerate() {
                    let expected = if outputs[input].unit == outputs[member].unit {
                        "1".to_string()
                    } else {
                        format!("{}/{}", outputs[input].unit, outputs[member].unit)
                    };
                    if guard(binding, symbol)?.unit != expected {
                        return Err("decoupler entry units do not agree".into());
                    }
                }
            }
        }
        Formula::NotchHistory {
            process_variable,
            denominator_symbols,
        } => {
            if destination.unit != guard(binding, process_variable)?.unit
                || denominator_symbols
                    .iter()
                    .any(|symbol| guard(binding, symbol).map_or(true, |member| member.unit != "1"))
            {
                return Err("filter-history units do not agree".into());
            }
        }
    }
    Ok(())
}
pub(super) fn calculate(
    binding: &Binding,
    formula: &Formula,
    storage: &VariableStorage,
    outputs: &BTreeMap<String, f64>,
) -> Result<f64, String> {
    let weight = |symbol: &Option<String>, fixed: &Option<f64>, default: f64| match symbol {
        Some(symbol) => guard_number(binding, storage, symbol),
        None => Ok(fixed.unwrap_or(default)),
    };
    match formula {
        Formula::ModelInput { output_symbol } => outputs
            .get(output_symbol)
            .copied()
            .ok_or("initialization output is missing".into()),
        Formula::ModelOutput { process_variable } => {
            guard_number(binding, storage, process_variable)
        }
        Formula::ModelInputIs {
            output_symbol,
            value,
        } => Ok(f64::from(u8::from(
            outputs
                .get(output_symbol)
                .copied()
                .ok_or("initialization output is missing")?
                == *value,
        ))),
        Formula::PidIntegral {
            output_symbol,
            process_variable,
            gain_symbol,
            weight_symbol,
            weight: fixed,
        } => Ok(outputs
            .get(output_symbol)
            .ok_or("initialization output is missing")?
            - guard_number(binding, storage, gain_symbol)?
                * (weight(weight_symbol, fixed, 1.0)? - 1.0)
                * guard_number(binding, storage, process_variable)?),
        Formula::WeightedError {
            process_variable,
            weight_symbol,
            weight: fixed,
        } => Ok((weight(weight_symbol, fixed, 0.0)? - 1.0)
            * guard_number(binding, storage, process_variable)?),
        Formula::ScaledMeasurement {
            process_variable,
            gain_symbol,
        } => Ok(guard_number(binding, storage, gain_symbol)?
            * guard_number(binding, storage, process_variable)?),
        Formula::DecoupledOutput {
            output_symbols,
            matrix_symbols,
            row,
        } => {
            let mut matrix = matrix_symbols
                .iter()
                .map(|entries| {
                    entries
                        .iter()
                        .map(|symbol| guard_number(binding, storage, symbol))
                        .collect::<Result<Vec<_>, String>>()
                })
                .collect::<Result<Vec<_>, String>>()?;
            let mut held = output_symbols
                .iter()
                .map(|symbol| {
                    outputs
                        .get(symbol)
                        .copied()
                        .ok_or_else(|| "initialization output is missing".to_string())
                })
                .collect::<Result<Vec<_>, String>>()?;
            solve_decoupler(&mut matrix, &mut held)?;
            Ok(held[*row])
        }
        Formula::NotchHistory {
            process_variable,
            denominator_symbols,
        } => {
            let denominator = 1.0
                + guard_number(binding, storage, &denominator_symbols[0])?
                + guard_number(binding, storage, &denominator_symbols[1])?;
            if denominator.abs() <= 1e-12 {
                return Err("notch initialization has a singular DC denominator".into());
            }
            Ok(guard_number(binding, storage, process_variable)? / denominator)
        }
    }
}

/// Solves `D·v = u` in place by Gaussian elimination with partial pivoting; `u` becomes `v`.
fn solve_decoupler(matrix: &mut [Vec<f64>], values: &mut [f64]) -> Result<(), String> {
    let size = values.len();
    let scale = matrix
        .iter()
        .flatten()
        .fold(0.0_f64, |largest, value| largest.max(value.abs()));
    for column in 0..size {
        let pivot = (column..size)
            .max_by(|&left, &right| {
                matrix[left][column]
                    .abs()
                    .total_cmp(&matrix[right][column].abs())
            })
            .ok_or("decoupler matrix is empty")?;
        // Not greater, including a pivot that is not a number, means singular.
        let threshold = 1e-12 * scale.max(1e-300);
        if matrix[pivot][column].abs().partial_cmp(&threshold) != Some(std::cmp::Ordering::Greater)
        {
            return Err("decoupler matrix is singular".into());
        }
        matrix.swap(column, pivot);
        values.swap(column, pivot);
        for row in column + 1..size {
            let factor = matrix[row][column] / matrix[column][column];
            // The pivot row lies above `row`, so the two rows can be borrowed together.
            let (above, below) = matrix.split_at_mut(row);
            let pivot_row = &above[column];
            for (cell, pivot_cell) in below[0][column..size]
                .iter_mut()
                .zip(&pivot_row[column..size])
            {
                *cell -= factor * pivot_cell;
            }
            values[row] -= factor * values[column];
        }
    }
    for row in (0..size).rev() {
        let sum: f64 = (row + 1..size).map(|k| matrix[row][k] * values[k]).sum();
        values[row] = (values[row] - sum) / matrix[row][row];
    }
    if values.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err("decoupled initialization is not finite".into())
    }
}

#[cfg(test)]
mod decoupler_tests {
    //! Behavior lock for `solve_decoupler` (Specification 35 §17.15): the solution `D⁻¹·u`, and
    //! the singular cases, including a pivot that is not a number.
    use super::solve_decoupler;

    fn solve(rows: &[&[f64]], values: &[f64]) -> Result<Vec<f64>, String> {
        let mut matrix: Vec<Vec<f64>> = rows.iter().map(|row| row.to_vec()).collect();
        let mut values = values.to_vec();
        solve_decoupler(&mut matrix, &mut values).map(|()| values)
    }

    #[test]
    fn a_regular_matrix_gives_d_inverse_times_u() {
        let solved = solve(
            &[&[2.0, 1.0, 0.0], &[1.0, 3.0, 1.0], &[0.0, 1.0, 4.0]],
            &[3.0, 6.0, 9.0],
        )
        .expect("a regular matrix");
        for (value, expected) in solved.iter().zip([1.0, 1.0, 2.0]) {
            assert!((value - expected).abs() < 1e-12, "{solved:?}");
        }
        let swapped = solve(&[&[0.0, 1.0], &[1.0, 0.0]], &[2.0, 3.0]).expect("pivoting swaps rows");
        assert_eq!(swapped, vec![3.0, 2.0]);
    }

    #[test]
    fn a_singular_matrix_or_a_pivot_that_is_not_a_number_is_refused() {
        assert_eq!(
            solve(&[&[1.0, 2.0], &[2.0, 4.0]], &[1.0, 2.0]),
            Err("decoupler matrix is singular".to_string())
        );
        assert_eq!(
            solve(&[&[f64::NAN, 0.0], &[0.0, f64::NAN]], &[1.0, 1.0]),
            Err("decoupler matrix is singular".to_string())
        );
        assert_eq!(
            solve(&[&[0.0, 0.0], &[0.0, 0.0]], &[1.0, 1.0]),
            Err("decoupler matrix is singular".to_string())
        );
    }
}
