//! The controller symbols and their types that a loop declares (specification 35, C0; `declaredControllerSymbols` and
//! `declaredControllerSymbolTypes` of `hostBindingDiscovery.ts`): the walk over the parameter binding and the controller whose results
//! `discovery` checks against the compiled catalog.

use serde::Serialize;
use serde_json::Value;

use crate::loop_definition::{LoopDefinition, ManualOutputType, RealType};

/// The controller symbols a loop declares, as the `TypeScript` walk finds them (`declaredControllerSymbols`): every text under a key
/// named `…symbol`, `…symbols` or `…Signal` that is dotted and not an address, except the record's own name and revision. The walk
/// reads the parameter binding and the controller as the file declares them.
pub(super) fn declared_controller_symbols(loop_definition: &LoopDefinition) -> Vec<String> {
    let mut symbols = Vec::new();
    if let Some(binding) = &loop_definition.parameter_binding {
        collect_symbols(&value_of(binding), "", &mut symbols);
    }
    collect_symbols(&value_of(&loop_definition.controller), "", &mut symbols);
    symbols
}

fn collect_symbols(value: &Value, key: &str, symbols: &mut Vec<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_symbols(item, key, symbols);
            }
        }
        Value::Object(fields) => {
            for (child_key, child) in fields {
                collect_symbols(child, child_key, symbols);
            }
        }
        Value::String(text) => {
            let record_name = matches!(key, "parameterRecordSymbol" | "parameterRevisionSymbol");
            if !record_name
                && names_a_symbol(key)
                && text.contains('.')
                && !text.starts_with('%')
                && !symbols.contains(text)
            {
                symbols.push(text.clone());
            }
        }
        _ => {}
    }
}

/// Whether a key names a symbol: `symbol`, `symbols` (in any case) or a `…Signal`.
fn names_a_symbol(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower.ends_with("symbol") || lower.ends_with("symbols") || key.ends_with("Signal")
}

/// The types the loop declares for its controller symbols (`declaredControllerSymbolTypes`). A later declaration of a symbol replaces
/// the type of an earlier one and keeps its place: the parameters, the mode (BOOL), the manual output, then the states the
/// initialization and the controller declare.
pub(super) fn declared_controller_symbol_types(
    loop_definition: &LoopDefinition,
) -> Vec<(String, String)> {
    let mut types: Vec<(String, String)> = Vec::new();
    if let Some(binding) = &loop_definition.parameter_binding {
        for parameter in &binding.parameters {
            set_type(&mut types, &parameter.symbol, real_name(parameter.iec_type));
        }
        set_type(&mut types, &binding.mode.symbol, "BOOL");
    }
    let controller = &loop_definition.controller;
    if let (Some(symbol), Some(manual_type)) = (
        &controller.manual_output_signal,
        controller.manual_output_type,
    ) {
        set_type(&mut types, symbol, manual_output_name(manual_type));
    }
    walk_types(&value_of(&controller.pid_initialization), &mut types);
    if let Some(state_feedback) = &controller.state_feedback {
        walk_types(state_feedback, &mut types);
    }
    if let Some(on_off) = &controller.on_off {
        walk_types(&on_off.to_value(), &mut types);
    }
    types
}

/// Reads every record of a declared structure that names a symbol and its IEC type, in either spelling.
fn walk_types(value: &Value, types: &mut Vec<(String, String)>) {
    match value {
        Value::Array(items) => {
            for item in items {
                walk_types(item, types);
            }
        }
        Value::Object(fields) => {
            if let Some(symbol) = fields.get("symbol").and_then(Value::as_str) {
                for key in ["iecType", "iec_type"] {
                    if let Some(iec_type) = fields.get(key).and_then(Value::as_str) {
                        set_type(types, symbol, iec_type);
                    }
                }
            }
            for child in fields.values() {
                walk_types(child, types);
            }
        }
        _ => {}
    }
}

/// Sets the type of a symbol: a later declaration replaces an earlier one and keeps its place.
fn set_type(types: &mut Vec<(String, String)>, symbol: &str, iec_type: &str) {
    match types.iter_mut().find(|entry| entry.0 == symbol) {
        Some(entry) => entry.1 = iec_type.to_string(),
        None => types.push((symbol.to_string(), iec_type.to_string())),
    }
}

/// The JSON of a loop part. Every part here is a record of serde types, so it serializes.
fn value_of<T: Serialize>(part: &T) -> Value {
    serde_json::to_value(part).unwrap_or(Value::Null)
}

/// The IEC name of a real type.
fn real_name(real: RealType) -> &'static str {
    match real {
        RealType::Real => "REAL",
        RealType::Lreal => "LREAL",
    }
}

/// The IEC name of a manual output's type.
fn manual_output_name(manual: ManualOutputType) -> &'static str {
    match manual {
        ManualOutputType::Real => "REAL",
        ManualOutputType::Lreal => "LREAL",
        ManualOutputType::Bool => "BOOL",
    }
}
