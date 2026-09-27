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
