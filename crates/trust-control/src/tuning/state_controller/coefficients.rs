//! The mapping of designed gains onto the declared parameters (C0 and C4; specification 35). Each gain takes the symbol that
//! the declaration names for its position. Its unit must be the unit of its model coordinates, a `REAL` coefficient is
//! stored rounded to single precision (C0), and the value must lie within the parameter's bounds.

use std::collections::BTreeMap;

use trust_control_math::linalg::DenseMatrix;

use super::texts;
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{Parameter, RealType};
use crate::tuning::settings_pins::single_precision;

/// The unit of a coefficient that maps a quantity in the denominator's unit to one in the numerator's: `1` when the two
/// units are the same, otherwise `numerator/denominator` (`ratioUnit`).
#[must_use]
pub fn ratio_unit(numerator: &str, denominator: &str) -> String {
    if numerator == denominator {
        "1".to_string()
    } else {
        format!("{numerator}/{denominator}")
    }
}

/// Writes each coefficient of `values` into `target` under the symbol that `symbols` names at the same position. The
/// coefficient's unit, given by `unit_of(row, column)`, must be the parameter's unit.
///
/// # Errors
/// Returns a failure when the symbols and the values have different layouts, when a symbol is not a parameter, when a
/// unit differs, or when a realized value is not finite or lies outside the parameter's bounds.
pub fn map_coefficients(
    symbols: &[Vec<String>],
    values: &DenseMatrix,
    unit_of: impl Fn(usize, usize) -> String,
    parameters: &[Parameter],
    target: &mut BTreeMap<String, f64>,
) -> Result<(), Failure> {
    let layout_fits = symbols.len() == values.len()
        && symbols
            .iter()
            .zip(values)
            .all(|(symbol_row, value_row)| symbol_row.len() == value_row.len());
    if !layout_fits {
        return Err(Failure::new(
            texts::LAYOUT_MISMATCH,
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        ));
    }
    for (row, (symbol_row, value_row)) in symbols.iter().zip(values).enumerate() {
        for (column, (symbol, value)) in symbol_row.iter().zip(value_row).enumerate() {
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.symbol == *symbol)
                .ok_or_else(|| {
                    Failure::new(
                        texts::LAYOUT_MISMATCH,
                        Cause::Implementation,
                        NextAction::OpenLoopSetup,
                    )
                })?;
            if parameter.unit != unit_of(row, column) {
                return Err(Failure::new(
                    &texts::unit_mismatch(symbol),
                    Cause::Implementation,
                    NextAction::OpenLoopSetup,
                ));
            }
            let realized = match parameter.iec_type {
                RealType::Real => single_precision(*value),
                RealType::Lreal => *value,
            };
            if !realized.is_finite() || realized < parameter.minimum || realized > parameter.maximum
            {
                return Err(Failure::new(
                    &texts::outside_representable_bounds(symbol),
                    Cause::Settings,
                    NextAction::OpenLoopSetup,
                ));
            }
            target.insert(symbol.clone(), realized);
        }
    }
    Ok(())
}
