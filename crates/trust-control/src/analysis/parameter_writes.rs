//! The parameter writes of a declared analysis (specification 35, C0 and C12, §0.1a; annex E7). A loop whose declared analysis maps
//! its parameters to program symbols (`analysisParameterSymbols`) lets Control Studio write those symbols, and no others, each
//! within the analysis parameter's own bounds, through the runtime's parameter transaction. This module holds the rules: which
//! symbols a write may set, the bounds of each value after the IEC rounding of its type (C0), the whole record that a write sets
//! (E7.3), and the identity of the review of a write, which is identified as an Apply's review is (A2.5, E11.8). The mapping is
//! checked as the `TypeScript` route checked it (`validAnalysisParameterSymbols`, E7.8). The status read, the transaction and its
//! journal belong to `trust-control-studio`.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::analysis::declared::{check_declared, DeclaredControlAnalysis, DeclaredParameter};
use crate::commissioning::application_records::review_identity;
use crate::commissioning::parameter_status::ParameterStatus;
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{LoopDefinition, Parameter, ParameterBinding, RealType};

/// The refusal of a loop with no declared analysis, or with no mapping of its parameters.
const NO_MAPPING: &str = "This loop declares no parameter mapping for its analysis, so Control Studio writes none of its parameters.";
/// The refusal of a mapping that does not name each analysis parameter once, with its own parameter of the binding.
const MAPPING_INVALID: &str =
    "The parameter mapping names each analysis parameter once, with a different parameter of the loop's binding.";
/// The refusal of a values record that names other symbols than the mapped ones.
const NOT_WRITABLE: &str =
    "A write sets exactly the parameters that the analysis maps, and no others.";

/// What a write does: an Apply sets the reviewed values (B6.1), and a Restore sets back the values the runtime retains from before
/// (E7.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ParameterWriteAction {
    /// The reviewed values are set.
    Apply,
    /// The retained previous values are set back.
    Restore,
}

/// A parameter that the declared analysis maps to a program symbol: the name the analysis gives it, the symbol, the IEC type that
/// rounds its values, and the bounds and the selected value that the analysis declares.
#[derive(Debug, Clone, PartialEq)]
pub struct WritableParameter {
    /// The name in the analysis (`Kp`).
    pub name: String,
    /// The program symbol that the mapping names (`P1.Kp`).
    pub symbol: String,
    /// The IEC type of the symbol.
    pub iec_type: RealType,
    /// The unit that the analysis and the binding both declare.
    pub unit: String,
    /// The smallest value that the analysis allows.
    pub minimum: f64,
    /// The largest value that the analysis allows.
    pub maximum: f64,
    /// The value the analysis selects: the root locus's selected value for its swept parameter, the declared value otherwise.
    pub selected: f64,
}

/// One value that a write changes, as its review shows it (A2.5): the symbol, its unit, the value in force and the value written.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WrittenValue {
    /// The program symbol.
    pub symbol: String,
    /// The unit of the symbol.
    pub unit: String,
    /// The value in force, as the runtime reports it.
    pub running: f64,
    /// The value that the write sets.
    pub next: f64,
}

/// The parameters that the loop's declared analysis lets a write set, in the order of the analysis's names.
///
/// # Errors
/// The loop declares no analysis or no mapping, its analysis is invalid, the mapping does not name each analysis parameter exactly
/// once with a unique symbol of the loop's parameter binding, or a mapped symbol's unit or bounds do not hold the analysis parameter's.
pub fn writable_parameters(
    loop_definition: &LoopDefinition,
) -> Result<Vec<WritableParameter>, Failure> {
    let analysis = declared_analysis(loop_definition)?;
    check_declared(&analysis)?;
    let mapping = symbol_mapping(loop_definition)?;
    let parameters = &analysis.controller.parameters;
    let symbols: BTreeSet<&str> = mapping.values().map(String::as_str).collect();
    if mapping.len() != parameters.len()
        || symbols.len() != mapping.len()
        || parameters.keys().any(|name| !mapping.contains_key(name))
    {
        return Err(Failure::new(
            MAPPING_INVALID,
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ));
    }
    parameters
        .iter()
        .map(|(name, declared)| {
            let symbol = mapping.get(name).ok_or_else(|| {
                Failure::new(MAPPING_INVALID, Cause::Settings, NextAction::OpenLoopSetup)
            })?;
            writable_of(loop_definition, &analysis, name, declared, symbol)
        })
        .collect()
}

/// The declared analysis that the loop file holds.
fn declared_analysis(loop_definition: &LoopDefinition) -> Result<DeclaredControlAnalysis, Failure> {
    let value = loop_definition
        .analysis
        .clone()
        .ok_or_else(|| Failure::new(NO_MAPPING, Cause::Settings, NextAction::OpenLoopSetup))?;
    crate::json::decode_value::<DeclaredControlAnalysis>(value).map_err(|error| {
        Failure::new(
            &format!("The loop's declared analysis cannot be read: {error}."),
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        )
    })
}

/// The mapping of the analysis parameters to program symbols, as the loop file declares it.
fn symbol_mapping(loop_definition: &LoopDefinition) -> Result<BTreeMap<String, String>, Failure> {
    let value = loop_definition
        .analysis_parameter_symbols
        .clone()
        .ok_or_else(|| Failure::new(NO_MAPPING, Cause::Settings, NextAction::OpenLoopSetup))?;
    crate::json::decode_value::<BTreeMap<String, String>>(value)
        .map_err(|_| Failure::new(MAPPING_INVALID, Cause::Settings, NextAction::OpenLoopSetup))
}

/// One writable parameter: the binding's symbol must be the analysis parameter's unit, and its range must hold the analysis range.
fn writable_of(
    loop_definition: &LoopDefinition,
    analysis: &DeclaredControlAnalysis,
    name: &str,
    declared: &DeclaredParameter,
    symbol: &str,
) -> Result<WritableParameter, Failure> {
    let parameter = loop_definition
        .parameter_binding
        .as_ref()
        .and_then(|binding| {
            binding
                .parameters
                .iter()
                .find(|parameter| parameter.symbol == symbol)
        })
        .ok_or_else(|| Failure::new(MAPPING_INVALID, Cause::Settings, NextAction::OpenLoopSetup))?;
    if parameter.unit != declared.unit
        || parameter.minimum > declared.minimum
        || parameter.maximum < declared.maximum
    {
        return Err(Failure::new(
            &format!(
                "{symbol} does not have the unit or the range of its analysis parameter {name}."
            ),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ));
    }
    let selected = if name == analysis.root_locus.parameter {
        analysis.root_locus.selected_value
    } else {
        declared.value
    };
    Ok(WritableParameter {
        name: name.to_owned(),
        symbol: symbol.to_owned(),
        iec_type: parameter.iec_type,
        unit: declared.unit.clone(),
        minimum: declared.minimum,
        maximum: declared.maximum,
        selected,
    })
}

/// The parameter binding the page shows for the declared analysis (C12): the loop's binding with only the writable parameters, each
/// with the bounds the analysis allows rather than the binding's. `None` for a loop without a parameter binding.
#[must_use]
pub fn writable_binding(
    loop_definition: &LoopDefinition,
    writable: &[WritableParameter],
) -> Option<ParameterBinding> {
    let binding = loop_definition.parameter_binding.as_ref()?;
    let convention = |symbol: &str| {
        binding
            .parameters
            .iter()
            .find(|parameter| parameter.symbol == symbol)
            .map(|parameter| parameter.convention.clone())
            .unwrap_or_default()
    };
    Some(ParameterBinding {
        loop_id: binding.loop_id.clone(),
        task_name: binding.task_name.clone(),
        parameters: writable
            .iter()
            .map(|parameter| Parameter {
                symbol: parameter.symbol.clone(),
                iec_type: parameter.iec_type,
                unit: parameter.unit.clone(),
                convention: convention(&parameter.symbol),
                minimum: parameter.minimum,
                maximum: parameter.maximum,
            })
            .collect(),
        mode: binding.mode.clone(),
    })
}

/// The values that the declared analysis selects, by symbol: the values that a review of the analysis's own choice starts from
/// (`selected-analysis`).
#[must_use]
pub fn selected_values(writable: &[WritableParameter]) -> BTreeMap<String, f64> {
    writable
        .iter()
        .map(|parameter| (parameter.symbol.clone(), parameter.selected))
        .collect()
}

/// The whole record that a write sets. Each writable symbol takes its proposed value, rounded as its IEC type rounds it and within the
/// analysis bounds; every other symbol keeps the value the runtime reports. The runtime writes the whole record (E7.3), so the
/// parameters that the analysis does not map keep their values.
///
/// # Errors
/// `proposed` does not name exactly the writable symbols, or one of its values lies outside the analysis bounds after rounding.
pub fn written_record(
    writable: &[WritableParameter],
    observed: &BTreeMap<String, f64>,
    proposed: &BTreeMap<String, f64>,
) -> Result<BTreeMap<String, f64>, Failure> {
    let symbols: BTreeSet<&str> = writable
        .iter()
        .map(|parameter| parameter.symbol.as_str())
        .collect();
    if proposed.len() != symbols.len()
        || proposed
            .keys()
            .any(|symbol| !symbols.contains(symbol.as_str()))
    {
        return Err(Failure::new(
            NOT_WRITABLE,
            Cause::Settings,
            NextAction::None,
        ));
    }
    let mut record = observed.clone();
    for parameter in writable {
        let value = proposed.get(&parameter.symbol).copied().unwrap_or(f64::NAN);
        let rounded = to_iec(parameter.iec_type, value);
        if !rounded.is_finite() || rounded < parameter.minimum || rounded > parameter.maximum {
            return Err(Failure::new(
                &outside_range(parameter, rounded),
                Cause::Settings,
                NextAction::None,
            ));
        }
        record.insert(parameter.symbol.clone(), rounded);
    }
    Ok(record)
}

/// The refusal of a value outside the range of its analysis parameter.
fn outside_range(parameter: &WritableParameter, value: f64) -> String {
    format!(
        "{} = {value} {} is outside the range {} to {} that its analysis parameter {} allows.",
        parameter.symbol, parameter.unit, parameter.minimum, parameter.maximum, parameter.name
    )
}

/// The value as its IEC type holds it: a REAL is rounded to single precision, as `Math.fround` rounds it (C0).
#[allow(clippy::cast_possible_truncation)] // Single precision is what the IEC type REAL holds.
#[must_use]
fn to_iec(iec_type: RealType, value: f64) -> f64 {
    match iec_type {
        RealType::Real => f64::from(value as f32),
        RealType::Lreal => value,
    }
}

/// The values that a write changes, in the order of the writable parameters: the value in force and the value the write sets.
#[must_use]
pub fn written_changes(
    writable: &[WritableParameter],
    observed: &BTreeMap<String, f64>,
    record: &BTreeMap<String, f64>,
) -> Vec<WrittenValue> {
    writable
        .iter()
        .map(|parameter| WrittenValue {
            symbol: parameter.symbol.clone(),
            unit: parameter.unit.clone(),
            running: observed.get(&parameter.symbol).copied().unwrap_or(f64::NAN),
            next: record.get(&parameter.symbol).copied().unwrap_or(f64::NAN),
        })
        .collect()
}

/// The identity of the review of a write (A2.5, E11.8), identified as an Apply's review is: `sha256:` of the canonical JSON of the
/// loop's revision, the revisions of the record the review was made against, the action, the revision a Restore returns to, the symbols
/// the write sets, and the whole record it sets. A confirmation names this identity, and the engine recomputes it from the runtime before
/// the write: a record or a loop that changed since the review gives another identity, and the write is refused.
#[must_use]
pub fn write_identity(
    loop_definition: &LoopDefinition,
    action: ParameterWriteAction,
    observed: &ParameterStatus,
    previous_revision: Option<&str>,
    writable: &[WritableParameter],
    record: &BTreeMap<String, f64>,
) -> String {
    let symbols: BTreeSet<&str> = writable
        .iter()
        .map(|parameter| parameter.symbol.as_str())
        .collect();
    review_identity(&json!({
        "action": action,
        "loopRevision": loop_definition.revision,
        "programRevision": observed.program_revision,
        "bindingRevision": observed.binding_revision,
        "activeRevision": observed.active_revision,
        "previousRevision": previous_revision,
        "symbols": symbols,
        "values": record,
    }))
}

#[cfg(test)]
#[path = "parameter_writes_tests.rs"]
mod tests;
