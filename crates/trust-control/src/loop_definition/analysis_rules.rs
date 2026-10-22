//! The declared analysis of a loop and its parameter map, checked when the loop opens as the page's loop validator checks them
//! (`loopDefinition.ts`, `declaredAnalysis.ts` `decodeDeclaredAnalysis` and `parameterBinding.ts` `validAnalysisParameterSymbols`;
//! specification 35, B6.1 and C12). The analysis's own rules are those of [`check_declared`]; this module adds the parameter map,
//! which names one bound parameter for each analysis parameter.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::analysis::declared::{check_declared, DeclaredControlAnalysis};

use super::{LoopDefinition, ParameterBinding};

/// Checks the loop's declared analysis, and its parameter map when the loop has one.
///
/// # Errors
/// The analysis does not decode or breaks the declared-analysis rules, or the parameter map does not name one bound parameter for
/// each analysis parameter, with the analysis's unit and a bound that contains the analysis's bound.
pub(super) fn check(loop_definition: &LoopDefinition) -> Result<(), String> {
    let analysis = loop_definition.analysis.as_ref().map(decode).transpose()?;
    let Some(symbols) = &loop_definition.analysis_parameter_symbols else {
        return Ok(());
    };
    let (Some(analysis), Some(binding)) = (&analysis, &loop_definition.parameter_binding) else {
        return Err(
            "the analysis parameter symbols need a declared analysis and a parameter binding"
                .into(),
        );
    };
    check_symbols(symbols, binding, analysis)
}

/// The analysis as the engine reads it, with its rules (`decodeDeclaredAnalysis`).
fn decode(value: &Value) -> Result<DeclaredControlAnalysis, String> {
    let analysis: DeclaredControlAnalysis = serde_json::from_value(value.clone())
        .map_err(|error| format!("the declared analysis is malformed: {error}"))?;
    check_declared(&analysis).map_err(|failure| failure.message)?;
    Ok(analysis)
}

/// The parameter map names exactly the analysis's parameters: each maps to a bound parameter of the same unit whose range contains the
/// analysis's range, and no bound parameter serves two of them (`validAnalysisParameterSymbols`).
fn check_symbols(
    symbols: &Value,
    binding: &ParameterBinding,
    analysis: &DeclaredControlAnalysis,
) -> Result<(), String> {
    let map = symbols
        .as_object()
        .ok_or("the analysis parameter symbols are a record")?;
    let declared = &analysis.controller.parameters;
    if map.len() != declared.len() {
        return Err("the analysis parameter symbols name each analysis parameter once".into());
    }
    let mut used = BTreeSet::new();
    for (name, parameter) in declared {
        let symbol = map
            .get(name)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("the analysis parameter {name} has no symbol"))?;
        let bound = binding
            .parameters
            .iter()
            .find(|bound| bound.symbol == symbol)
            .ok_or_else(|| format!("{symbol} is not a bound parameter"))?;
        if bound.unit != parameter.unit
            || bound.minimum > parameter.minimum
            || bound.maximum < parameter.maximum
            || !used.insert(symbol)
        {
            return Err(format!(
                "{symbol} does not bind the analysis parameter {name} in its unit and range, or binds it twice"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "analysis_rules_tests.rs"]
mod tests;
