//! The values that a state-feedback law or a finite search writes through the loop's parameter record (specification 35, C4 and C5, C0):
//! each coefficient must be a parameter of the loop's binding, inside its declared bounds and representable in its type, as the design
//! checks it. A `Control.PID` or a program's law writes its gains through `ControllerAdapter::materialize`, not through this module.

use std::collections::BTreeMap;

use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{LoopDefinition, RealType};
use crate::tuning::settings_pins::single_precision;

/// The refusal of a loop with no parameter binding, which its coefficients are written through.
const NO_BINDING: &str =
    "The loop declares no parameter binding, so its coefficients have nowhere to be written (C0).";

/// The values that a set of coefficients writes (C4, C5): each coefficient, rounded to single precision where its parameter is `REAL`,
/// checked against the parameter's declared bounds (signed, as the design checks them), with the observed values kept for every other
/// parameter.
///
/// # Errors
/// The loop has no parameter binding; a coefficient is not a parameter of it; or a coefficient is not finite, or lies outside its bounds
/// once it is rounded to its type.
pub fn coefficient_values(
    loop_definition: &LoopDefinition,
    coefficients: &BTreeMap<String, f64>,
    observed: &BTreeMap<String, f64>,
) -> Result<BTreeMap<String, f64>, Failure> {
    let binding = loop_definition.parameter_binding.as_ref().ok_or_else(|| {
        Failure::new(NO_BINDING, Cause::Implementation, NextAction::OpenLoopSetup)
    })?;
    let mut values = observed.clone();
    for (symbol, value) in coefficients {
        let parameter = binding
            .parameters
            .iter()
            .find(|parameter| parameter.symbol == *symbol)
            .ok_or_else(|| {
                Failure::new(
                    &format!("The coefficient {symbol} is not a parameter of the loop's record."),
                    Cause::Settings,
                    NextAction::OpenLoopSetup,
                )
            })?;
        let realized = match parameter.iec_type {
            RealType::Real => single_precision(*value),
            RealType::Lreal => *value,
        };
        if !realized.is_finite() || realized < parameter.minimum || realized > parameter.maximum {
            return Err(Failure::new(
                &format!(
                    "The coefficient {symbol} = {realized} lies outside its declared bounds {} to {} {}; Tune again.",
                    parameter.minimum, parameter.maximum, parameter.unit
                ),
                Cause::Settings,
                NextAction::Tune,
            ));
        }
        values.insert(symbol.clone(), realized);
    }
    Ok(values)
}
