//! The controller adapter of the step-by-step loop setup (specification 35, C0 "Parameter-record adapter" and "Missing
//! adapters"; `configureControllerAdapter.ts`). The engineer picks a JSON mapping of the existing controller. This module
//! reads its `controller` and `parameterBinding` objects, checks every symbol against the compiled catalog with its type,
//! takes the transfer binding of the controller's adapter, and then the loop takes the mapping. The file dialog and the
//! file read stay in the engine, which reads at most [`MAXIMUM_MAPPING_BYTES`] + 1 bytes.

use serde_json::{Map, Value};

use crate::commissioning::adapters::controller_adapter;
use crate::commissioning::transfer::TransferValueType;
use crate::failure::{Cause, Failure, NextAction};
use crate::json;
use crate::loop_definition::{
    Controller, Direction, Execution, Experiment, ExperimentKind, LoopDefinition, ManualOutputType,
    ParameterBinding,
};
use crate::setup::catalog::CompiledCatalog;
use crate::setup::texts;

/// The largest mapping the step accepts, in bytes (`configureControllerAdapter.ts`: "limited to 256 kB").
pub const MAXIMUM_MAPPING_BYTES: usize = 256_000;

/// The controller fields a mapping may set: the ones `configureControllerAdapter.ts` copies from the mapping's `controller`
/// object, and nothing else.
const MAPPED_CONTROLLER_FIELDS: [&str; 9] = [
    "project",
    "stateFeedback",
    "onOff",
    "law",
    "pidInitialization",
    "manualOutputSignal",
    "manualOutputType",
    "manualOutputAddress",
    "manualOutputFollowsOutput",
];

/// Reads a mapping file's bytes as JSON (`configureControllerAdapter.ts`: UTF-8, then `JSON.parse`). A file of more than
/// [`MAXIMUM_MAPPING_BYTES`] bytes is refused before it is read.
///
/// # Errors
/// The file is larger than 256 kB, or its text is not JSON.
pub fn mapping_from_bytes(bytes: &[u8]) -> Result<Value, Failure> {
    if bytes.len() > MAXIMUM_MAPPING_BYTES {
        return Err(refusal(texts::configure::adapter_limit()));
    }
    let text = String::from_utf8_lossy(bytes);
    json::parse_text(&text)
        .map_err(|error| refusal(&texts::configure::adapter_not_json(&error.to_string())))
}

/// Sets the loop's controller, parameter record and mode from a mapping (`configureControllerAdapter.ts`), and makes the
/// loop an observation through the controller's Control Studio experiment.
///
/// The steps are the TypeScript's, in its order: the `controller` and `parameterBinding` objects; the parameter binding,
/// whose `loop_id` is the loop's and whose task must be the selected one; every parameter and the mode as a compiled
/// member of its type; the manual output; the controller's adapter, which refuses the parked features by name; and the
/// transfer binding's states. A controller that has no initialization still accepts a mapping whose states do not
/// compile, because its Auto entry is blocked and Manual staging remains (C0, "Missing adapters").
///
/// # Errors
/// The mapping's objects, binding, symbols or controller do not fit the loop and the catalog, or the controller's adapter
/// cannot realise the loop.
pub fn apply_mapping(
    loop_definition: &mut LoopDefinition,
    mapping: &Value,
    catalog: &CompiledCatalog,
    selected_task: Option<&str>,
) -> Result<(), Failure> {
    let (controller_fields, binding_fields) = mapping_objects(mapping)?;
    let parameter_binding = decode_binding(binding_fields, &loop_definition.id, selected_task)?;
    for parameter in &parameter_binding.parameters {
        check_member(
            catalog,
            &parameter.symbol,
            TransferValueType::from(parameter.iec_type),
        )?;
    }
    check_member(
        catalog,
        &parameter_binding.mode.symbol,
        TransferValueType::Bool,
    )?;
    let controller = merged_controller(
        &loop_definition.controller,
        controller_fields,
        &parameter_binding,
    )?;
    if let (Some(symbol), Some(output_type)) = (
        &controller.manual_output_signal,
        controller.manual_output_type,
    ) {
        check_member(catalog, symbol, manual_type(output_type))?;
    }
    let configured = LoopDefinition {
        controller: controller.clone(),
        parameter_binding: Some(parameter_binding.clone()),
        ..loop_definition.clone()
    };
    let adapter = controller_adapter(&configured)?;
    let states = adapter.transfer_binding().and_then(|transfer| {
        transfer
            .states
            .iter()
            .try_for_each(|state| check_member(catalog, &state.symbol, state.iec_type))
    });
    // `configureControllerAdapter.ts`: a failed state check is kept only for a controller without an initialization. The
    // program's own law outside the served shape (C2), ON/OFF and state feedback are refused by the adapter above, so only the PID
    // initialization is left. A served program's own law is realized by its adapter.
    if let Err(failure) = states
        && controller.pid_initialization.is_some()
    {
        return Err(failure);
    }
    commit(loop_definition, controller, parameter_binding);
    Ok(())
}

/// Writes the checked mapping into the loop. The experiment keeps the duration and the seed it has; an absent duration stays
/// absent, because Identify and Limits choose it (B3.2, C1 Setup), and the TypeScript's silent 10 s is not carried over.
fn commit(loop_definition: &mut LoopDefinition, controller: Controller, binding: ParameterBinding) {
    let duration_seconds = loop_definition
        .experiment
        .as_ref()
        .and_then(|experiment| experiment.duration_seconds);
    let seed = loop_definition
        .experiment
        .as_ref()
        .map_or(1, |experiment| experiment.seed);
    loop_definition.controller = controller;
    loop_definition.parameter_binding = Some(binding);
    loop_definition.signals.manipulated_variable.direction = Direction::Write;
    loop_definition.experiment = Some(Experiment {
        execution: Execution::ControlStudio,
        kind: ExperimentKind::Observation,
        amplitude: 0.0,
        relay_direction: None,
        duration_seconds,
        baseline_seconds: None,
        hold_parameter: None,
        seed,
        activation: None,
        abort: None,
    });
}

/// The `controller` and `parameterBinding` objects of an adapter mapping.
type MappingObjects<'a> = (&'a Map<String, Value>, &'a Map<String, Value>);

/// The mapping's `controller` and `parameterBinding` objects (`record` in `configureControllerAdapter.ts`).
fn mapping_objects(mapping: &Value) -> Result<MappingObjects<'_>, Failure> {
    let controller = mapping.get("controller").and_then(Value::as_object);
    let binding = mapping.get("parameterBinding").and_then(Value::as_object);
    match (controller, binding) {
        (Some(controller), Some(binding)) => Ok((controller, binding)),
        _ => Err(refusal(texts::configure::adapter_objects())),
    }
}

/// The parameter binding as the mapping states it, with the loop's own `loop_id`
/// (`decodeParameterBinding({ ...value.parameterBinding, loop_id: loop.id }, loop.id)`).
fn decode_binding(
    fields: &Map<String, Value>,
    loop_id: &str,
    selected_task: Option<&str>,
) -> Result<ParameterBinding, Failure> {
    let mut fields = fields.clone();
    fields.insert("loop_id".into(), Value::String(loop_id.to_owned()));
    let binding: ParameterBinding = json::decode_value(Value::Object(fields))
        .map_err(|_| refusal(texts::configure::adapter_task_mismatch()))?;
    if selected_task.is_some_and(|task| binding.task_name != task) {
        return Err(refusal(texts::configure::adapter_task_mismatch()));
    }
    Ok(binding)
}

/// The loop's controller with the mapped fields copied over it; the design is the project's, and the mode is the mapping's.
fn merged_controller(
    base: &Controller,
    fields: &Map<String, Value>,
    binding: &ParameterBinding,
) -> Result<Controller, Failure> {
    let Value::Object(mut merged) =
        serde_json::to_value(base).map_err(|error| refusal(&error.to_string()))?
    else {
        return Err(refusal(texts::configure::adapter_objects()));
    };
    for field in MAPPED_CONTROLLER_FIELDS {
        if let Some(value) = fields.get(field) {
            merged.insert(field.to_owned(), value.clone());
        }
    }
    let mode_when_true = if binding.mode.manual_value {
        "Manual"
    } else {
        "Auto"
    };
    merged.insert("design".into(), Value::String("project".into()));
    merged.insert(
        "modeSignal".into(),
        Value::String(binding.mode.symbol.clone()),
    );
    merged.insert("modeWhenTrue".into(), Value::String(mode_when_true.into()));
    json::decode_value(Value::Object(merged))
        .map_err(|error| refusal(&texts::configure::adapter_controller(&error.to_string())))
}

/// Checks that `symbol` is a compiled member path of the expected type (`check` in `configureControllerAdapter.ts`): a dotted
/// path of identifiers, declared in the catalog with that type.
fn check_member(
    catalog: &CompiledCatalog,
    symbol: &str,
    expected: TransferValueType,
) -> Result<(), Failure> {
    let declared = catalog.symbols.iter().any(|candidate| {
        candidate.symbol == symbol
            && TransferValueType::from_iec(&candidate.iec_type) == Some(expected)
    });
    if is_member_path(symbol) && declared {
        return Ok(());
    }
    Err(Failure::new(
        &texts::configure::adapter_symbol(symbol, type_word(expected)),
        Cause::Implementation,
        NextAction::OpenLoopSetup,
    ))
}

/// A dotted path of at least two identifiers (`configureControllerAdapter.ts`, the path pattern). The identifier classes are
/// Rust's `alphabetic` and `numeric`, which are the letter and number categories plus the derived alphabetic characters.
fn is_member_path(symbol: &str) -> bool {
    let segments: Vec<&str> = symbol.split('.').collect();
    segments.len() >= 2 && segments.iter().all(|segment| is_identifier(segment))
}

fn is_identifier(segment: &str) -> bool {
    let mut characters = segment.chars();
    matches!(characters.next(), Some(first) if first.is_alphabetic() || first == '_')
        && characters.all(|character| {
            character.is_alphabetic() || character.is_numeric() || character == '_'
        })
}

const fn type_word(value: TransferValueType) -> &'static str {
    match value {
        TransferValueType::Real => "REAL",
        TransferValueType::Lreal => "LREAL",
        TransferValueType::Bool => "BOOL",
    }
}

const fn manual_type(value: ManualOutputType) -> TransferValueType {
    match value {
        ManualOutputType::Real => TransferValueType::Real,
        ManualOutputType::Lreal => TransferValueType::Lreal,
        ManualOutputType::Bool => TransferValueType::Bool,
    }
}

/// A refusal of the mapping, of the kind that the setup shows with its own next action (A11.2).
fn refusal(text: &str) -> Failure {
    Failure::new(text, Cause::Settings, NextAction::OpenLoopSetup)
}

#[cfg(test)]
#[path = "configure_adapter_tests.rs"]
mod tests;
