//! The loops of a project bound to the compiled program (specification 35, B1.2, B1.3 and B1.4; `hostBindingDiscovery.ts`).
//!
//! The engine lists and reads the loop files and the compiled catalog, then calls [`discover_bindings`]. Every rule is decided
//! here: the file bounds, why a file is refused, the duplicate ids, the join to the compiled program, the selectable channels and
//! the selection itself. Nothing here writes a file. A loop that uses channels is refused by name, because the Rust engine does
//! not run channels yet (stage V7).

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::catalog::{CatalogAnswer, CompiledCatalog, CompiledController, CompiledTask};
use super::save::validation_reason_code;
use super::texts::discovery as texts;
use crate::commissioning::adapters::controller_adapter;
use crate::failure::{Cause, Failure, NextAction};
use crate::json::parse_text;
use crate::loop_definition::{
    AuxiliaryRole, LoopDefinition, LoopScope, ManualOutputType, Range, RealType,
};
use crate::tuning::structure::Structure;

/// The most loop files one discovery reads (B1.2; a product limit).
pub const MAXIMUM_LOOP_FILES: usize = 128;

/// The largest loop file the discovery reads, in bytes (B1.2; a product limit).
pub const MAXIMUM_LOOP_BYTES: u64 = 512 * 1024;

/// Two periods are the same when they differ by less than this, in ms (the `TypeScript`'s `1e-6`).
const PERIOD_TOLERANCE_MS: f64 = 1e-6;

/// The most channels one side of a selection may name (`threePageBindingRoute.ts`).
pub const MAXIMUM_SELECTED_CHANNELS: usize = 16;

/// What the engine read of one loop file (B1.2). The engine skips a file that does not exist.
#[derive(Debug, Clone, PartialEq)]
pub enum LoopFileRead {
    /// The file's text.
    Text(String),
    /// The file is larger than [`MAXIMUM_LOOP_BYTES`], so it was not read.
    TooLarge {
        /// The file's size, in bytes.
        bytes: u64,
    },
    /// The file exists but cannot be read.
    Unreadable,
}

/// One loop file as the engine read it. The root file comes first, then `control-loops/*.json` in file-name order (B1.2).
#[derive(Debug, Clone, PartialEq)]
pub struct LoopFileText {
    /// The file's path, as the engine found it.
    pub configuration_path: String,
    /// What was read.
    pub read: LoopFileRead,
}

/// A signal a selector offers for a loop's input or output (B1.3).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SelectableSignal {
    /// The signal's identity in the loop: `manipulatedVariable`, `processVariable`, or an auxiliary signal's id.
    pub id: String,
    /// The variable's path.
    pub symbol: String,
    /// The variable's address; absent for a program variable without one (§17.1).
    pub address: Option<String>,
    /// The variable's IEC type.
    pub iec_type: String,
    /// The signal's unit.
    pub unit: String,
    /// The signal's range.
    pub bounds: Range,
    /// The channel of an auxiliary signal.
    pub channel: Option<String>,
    /// The signal's role: `MV` or `PV`.
    pub role: String,
    /// The signal's place among the offered signals of its side.
    pub order: usize,
}

/// Whether a loop's adapter can realize the loop (C0): not configured, accepted with the structures it realizes, or refused with the
/// reason.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ControllerCompatibility {
    /// The loop declares no parameter record, law or program controller to adapt.
    Unconfigured,
    /// The adapter realizes these structures, such as `P` or `PID`.
    Accepted {
        /// The structures, as the setup page names them.
        families: Vec<String>,
    },
    /// No adapter realizes the loop, and this is why.
    Rejected {
        /// The adapter's whole reason.
        reason: String,
    },
}

/// The controller of a loop as the compiled program has it (B1.2).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredController {
    /// The controller's instance path.
    pub instance: String,
    /// The controller's compiled type.
    #[serde(rename = "type")]
    pub type_name: String,
    /// The tasks that run the controller, with their intervals.
    pub tasks: Vec<CompiledTask>,
}

/// One loop file that is safe to present to the selectors (B1.2).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BindingTarget {
    /// The file the loop was read from.
    pub configuration_path: String,
    /// The loop, as checked.
    pub loop_definition: LoopDefinition,
    /// The controller the loop runs in the compiled program.
    pub controller: DiscoveredController,
    /// The offered inputs, in declared order.
    pub inputs: Vec<SelectableSignal>,
    /// The offered outputs, in declared order.
    pub outputs: Vec<SelectableSignal>,
    /// Whether the loop's adapter can realize it.
    pub controller_compatibility: ControllerCompatibility,
}

/// A loop file, or the whole project, that the discovery refuses, and why (B1.2: a missing or invalid definition is explained).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Rejection {
    /// The file (or, for the whole project, the project's folder).
    pub configuration_path: String,
    /// Why it is refused.
    pub reason: String,
}

/// The result of one discovery: the selectable loops and the refusals (B1.2).
#[derive(Debug, Clone, PartialEq, Default, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Discovery {
    /// The loops a selector can offer.
    pub targets: Vec<BindingTarget>,
    /// The refused files and the reasons.
    pub rejected: Vec<Rejection>,
}

/// The choice a selector sends: one loop, its controller and every channel of both sides (B1.3).
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BindingSelection {
    /// The loop's id.
    pub loop_id: String,
    /// The controller instance the loop runs in.
    pub controller_instance: String,
    /// The input identities, every declared input once.
    pub input_ids: Vec<String>,
    /// The output identities, every declared output once.
    pub output_ids: Vec<String>,
}

/// Discovers the loops of one project and joins each to the compiled program (B1.2 and B1.4).
///
/// The whole project is refused, with one rejection at `project_root`, when more than [`MAXIMUM_LOOP_FILES`] files are given. Each
/// file is read and checked in the order given: a file that is too large, not JSON, not a complete loop, of another runtime,
/// invalid, or not in the compiled program is refused with its reason. A loop id that more than one file declares makes every one
/// of those files ambiguous, and no target with that id is selectable.
#[must_use]
pub fn discover_bindings(
    project_root: &str,
    catalog: &CatalogAnswer,
    runtime_id: &str,
    files: &[LoopFileText],
) -> Discovery {
    if files.len() > MAXIMUM_LOOP_FILES {
        return refused(
            project_root,
            &texts::too_many_loop_files(MAXIMUM_LOOP_FILES),
        );
    }
    let mut discovery = Discovery::default();
    let mut paths_by_id: Vec<(String, Vec<String>)> = Vec::new();
    for file in files {
        let Some(loop_definition) = read_or_refuse(file, &mut discovery) else {
            continue;
        };
        record_id(
            &mut paths_by_id,
            &loop_definition.id,
            &file.configuration_path,
        );
        if loop_definition.runtime_id != runtime_id {
            let reason = texts::runtime_mismatch(&loop_definition.runtime_id, runtime_id);
            discovery.rejected.push(rejection_for(file, reason));
            continue;
        }
        let scope = LoopScope {
            project_id: loop_definition.project_id.clone(),
            runtime_id: runtime_id.to_string(),
        };
        if let Err(rejection) = loop_definition.validate(&scope) {
            let reason = texts::invalid_loop(validation_reason_code(&rejection));
            discovery.rejected.push(rejection_for(file, reason));
            continue;
        }
        match target_for_loop(&file.configuration_path, loop_definition, catalog) {
            Ok(target) => discovery.targets.push(target),
            Err(reason) => discovery.rejected.push(rejection_for(file, reason)),
        }
    }
    for (id, paths) in paths_by_id.iter().filter(|(_, paths)| paths.len() > 1) {
        discovery
            .targets
            .retain(|target| target.loop_definition.id != *id);
        for path in paths {
            discovery.rejected.push(Rejection {
                configuration_path: path.clone(),
                reason: texts::duplicate_loop_id(id),
            });
        }
    }
    discovery
}

/// The discovery of a project whose `control-loops` folder cannot be listed: nothing is selectable (B1.2).
#[must_use]
pub fn control_loops_unreadable(project_root: &str) -> Discovery {
    let folder = Path::new(project_root).join("control-loops");
    refused(
        &folder.display().to_string(),
        texts::control_loops_unreadable(),
    )
}

/// A discovery with no target and one rejection, at `configuration_path`: a whole-project refusal, such as a compiled catalog that
/// could not be read.
#[must_use]
pub fn refused(configuration_path: &str, reason: &str) -> Discovery {
    Discovery {
        targets: Vec::new(),
        rejected: vec![Rejection {
            configuration_path: configuration_path.to_string(),
            reason: reason.to_string(),
        }],
    }
}

/// The discovery restricted to the loops of one project and runtime, as the selectors list them: the rejections stay whole (B1.2,
/// `hostBindingSession.ts`, scoped discovery).
#[must_use]
pub fn in_scope(discovery: Discovery, scope: &LoopScope) -> Discovery {
    let targets = discovery
        .targets
        .into_iter()
        .filter(|target| {
            target.loop_definition.project_id == scope.project_id
                && target.loop_definition.runtime_id == scope.runtime_id
        })
        .collect();
    Discovery {
        targets,
        rejected: discovery.rejected,
    }
}

/// The target a selection names (B1.3, `validateControlStudioBindingSelection`). The selection names the loop's controller and every
/// declared input and output once, so the selected channels are the loop's own, in declared order. A selection never changes the
/// loop; the caller opens a copy of the target.
///
/// # Errors
/// No target has the loop's id, the controller is another one, or a side names no channel, a partial set, an undeclared channel or
/// one channel twice.
pub fn select_binding<'a>(
    discovery: &'a Discovery,
    selection: &BindingSelection,
) -> Result<&'a BindingTarget, Failure> {
    let Some(target) = discovery
        .targets
        .iter()
        .find(|target| target.loop_definition.id == selection.loop_id)
    else {
        return Err(Failure::new(
            &texts::no_loop_with_id(&selection.loop_id),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ));
    };
    if selection.controller_instance != target.controller.instance {
        return Err(Failure::new(
            &texts::controller_not_declared(
                &selection.controller_instance,
                &target.controller.instance,
            ),
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        ));
    }
    check_channels(&selection.input_ids, &target.inputs, "input")?;
    check_channels(&selection.output_ids, &target.outputs, "output")?;
    Ok(target)
}

/// The definition of one loop file, or its refusal recorded in the discovery.
fn read_or_refuse(file: &LoopFileText, discovery: &mut Discovery) -> Option<LoopDefinition> {
    match read_loop_definition(file) {
        Ok(loop_definition) => Some(loop_definition),
        Err(reason) => {
            discovery.rejected.push(rejection_for(file, reason));
            None
        }
    }
}

/// Reads and checks one loop file: its size, its JSON, its definition (B1.2). The JSON is checked first, so that a syntax error keeps
/// its own reason.
fn read_loop_definition(file: &LoopFileText) -> Result<LoopDefinition, String> {
    let name = base_name(&file.configuration_path);
    let text = match &file.read {
        LoopFileRead::Text(text) => text,
        LoopFileRead::TooLarge { .. } => {
            return Err(texts::exceeds_byte_bound(&name, MAXIMUM_LOOP_BYTES));
        }
        LoopFileRead::Unreadable => return Err(texts::not_valid_json(&name)),
    };
    if parse_text(text).is_err() {
        return Err(texts::not_valid_json(&name));
    }
    LoopDefinition::decode(text).map_err(|_| texts::not_loop_definition(&name))
}

/// Records that `path` declares `id`, in the order the ids first appear (the duplicate rejections follow that order).
fn record_id(paths_by_id: &mut Vec<(String, Vec<String>)>, id: &str, path: &str) {
    match paths_by_id.iter_mut().find(|entry| entry.0 == id) {
        Some(entry) => entry.1.push(path.to_string()),
        None => paths_by_id.push((id.to_string(), vec![path.to_string()])),
    }
}

/// The refusal of one file, with the file's path.
fn rejection_for(file: &LoopFileText, reason: String) -> Rejection {
    Rejection {
        configuration_path: file.configuration_path.clone(),
        reason,
    }
}

/// The file's name, as the discovery's sentences show it.
fn base_name(path: &str) -> String {
    Path::new(path).file_name().map_or_else(
        || path.to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// The target of one loop: its controller, its task, its signals, its symbols and its selectable channels (B1.2, B1.4), in the order
/// of `targetForLoop`.
fn target_for_loop(
    configuration_path: &str,
    loop_definition: LoopDefinition,
    catalog: &CatalogAnswer,
) -> Result<BindingTarget, String> {
    let controller = check_controller(&loop_definition, &catalog.control)?;
    check_group_members(&loop_definition, &catalog.control)?;
    check_signals(&loop_definition, catalog)?;
    check_controller_symbols(&loop_definition, &catalog.control)?;
    if loop_definition.channels.is_some() {
        return Err(texts::channels_not_supported().to_string());
    }
    let inputs = selectable_signals(&loop_definition, Side::Input);
    let outputs = selectable_signals(&loop_definition, Side::Output);
    if inputs.is_empty() || outputs.is_empty() {
        return Err(texts::needs_input_and_output().to_string());
    }
    let controller_compatibility = adapter_compatibility(&loop_definition);
    Ok(BindingTarget {
        configuration_path: configuration_path.to_string(),
        controller: DiscoveredController {
            instance: controller.instance.clone(),
            type_name: controller.type_name.clone(),
            tasks: controller.tasks.clone(),
        },
        loop_definition,
        inputs,
        outputs,
        controller_compatibility,
    })
}

/// The loop's controller is in the compiled program with its type, and runs in the loop's task when it has tasks at all.
fn check_controller<'a>(
    loop_definition: &LoopDefinition,
    compiled: &'a CompiledCatalog,
) -> Result<&'a CompiledController, String> {
    let controller = &loop_definition.controller;
    let Some(compiled_controller) = compiled
        .controllers
        .iter()
        .find(|candidate| candidate.instance == controller.instance)
    else {
        return Err(texts::controller_unavailable(&controller.instance));
    };
    if compiled_controller.type_name != controller.type_name {
        return Err(texts::controller_type(
            &controller.instance,
            &compiled_controller.type_name,
            &controller.type_name,
        ));
    }
    let period = controller.task_period_ms;
    let in_task = compiled_controller
        .tasks
        .iter()
        .any(|task| (task.interval_ms - period).abs() < PERIOD_TOLERANCE_MS);
    if !compiled_controller.tasks.is_empty() && !in_task {
        return Err(texts::controller_task(&controller.instance, period));
    }
    Ok(compiled_controller)
}

/// Every member of a PID group is in the compiled program with its type, in the loop's task (C7). The group is read as the file
/// declares it: the Rust engine does not run groups yet.
fn check_group_members(
    loop_definition: &LoopDefinition,
    compiled: &CompiledCatalog,
) -> Result<(), String> {
    let Some(group) = &loop_definition.controller.group else {
        return Ok(());
    };
    let Some(members) = group.get("members").and_then(Value::as_array) else {
        return Ok(());
    };
    let period = loop_definition.controller.task_period_ms;
    let task_name = loop_definition
        .parameter_binding
        .as_ref()
        .map(|binding| binding.task_name.as_str());
    for member in members {
        let instance = member
            .get("instance")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let member_type = member.get("type").and_then(Value::as_str);
        let available = compiled
            .controllers
            .iter()
            .find(|candidate| candidate.instance == instance)
            .is_some_and(|actual| {
                member_type == Some(actual.type_name.as_str())
                    && actual.tasks.iter().any(|task| {
                        task_name == Some(task.name.as_str())
                            && (task.interval_ms - period).abs() < PERIOD_TOLERANCE_MS
                    })
            });
        if !available {
            return Err(texts::group_member_unavailable(instance));
        }
    }
    Ok(())
}

/// Every declared signal is the compiled one: a signal without an address is a compiled variable of its type, and a signal with an
/// address matches the compiled process image in address and type (B1.2).
fn check_signals(loop_definition: &LoopDefinition, catalog: &CatalogAnswer) -> Result<(), String> {
    for (symbol, address, iec_type) in declared_signals(loop_definition) {
        match address {
            None => match catalog
                .control
                .symbols
                .iter()
                .find(|entry| entry.symbol == symbol)
            {
                Some(entry) if entry.iec_type.eq_ignore_ascii_case(iec_type) => {}
                _ => return Err(texts::signal_not_variable(symbol, iec_type)),
            },
            Some(address) => match catalog.signals.iter().find(|entry| entry.symbol == symbol) {
                Some(entry)
                    if entry.address == address
                        && entry.iec_type.eq_ignore_ascii_case(iec_type) => {}
                _ => return Err(texts::signal_identity(symbol)),
            },
        }
    }
    Ok(())
}

/// Every controller symbol the loop declares is in the compiled catalog, and has the type the loop declares for it (C0).
fn check_controller_symbols(
    loop_definition: &LoopDefinition,
    compiled: &CompiledCatalog,
) -> Result<(), String> {
    for symbol in declared_controller_symbols(loop_definition) {
        if !compiled.symbols.iter().any(|entry| entry.symbol == symbol) {
            return Err(texts::controller_symbol_unavailable(&symbol));
        }
    }
    for (symbol, expected) in declared_controller_symbol_types(loop_definition) {
        if let Some(entry) = compiled.symbols.iter().find(|entry| entry.symbol == symbol) {
            if !entry.iec_type.eq_ignore_ascii_case(&expected) {
                return Err(texts::controller_symbol_type(
                    &symbol,
                    &entry.iec_type,
                    &expected,
                ));
            }
        }
    }
    Ok(())
}

/// The signals a loop declares, as (symbol, address, IEC type): the setpoint, the measurement, the output, the actuator and the
/// auxiliary signals, in that order (`allDeclaredSignals`).
fn declared_signals(loop_definition: &LoopDefinition) -> Vec<(&str, Option<&str>, &str)> {
    let signals = &loop_definition.signals;
    let fixed = [
        signals.setpoint.as_ref(),
        Some(&signals.process_variable),
        Some(&signals.manipulated_variable),
        signals.actuator.as_ref(),
    ];
    let mut declared: Vec<(&str, Option<&str>, &str)> = fixed
        .into_iter()
        .flatten()
        .map(|signal| {
            (
                signal.symbol.as_str(),
                signal.address.as_deref(),
                signal.iec_type.as_str(),
            )
        })
        .collect();
    declared.extend(signals.additional.iter().flatten().map(|signal| {
        (
            signal.symbol.as_str(),
            signal.address.as_deref(),
            signal.iec_type.as_str(),
        )
    }));
    declared
}

/// The selectable inputs or outputs of a loop without channels (`selectableSignals`): the primary signal of the side first, then the
/// auxiliary signals of its role in declared order.
fn selectable_signals(loop_definition: &LoopDefinition, side: Side) -> Vec<SelectableSignal> {
    let signals = &loop_definition.signals;
    let (id, primary, role, wanted) = match side {
        Side::Input => (
            "manipulatedVariable",
            &signals.manipulated_variable,
            "MV",
            AuxiliaryRole::ManipulatedVariable,
        ),
        Side::Output => (
            "processVariable",
            &signals.process_variable,
            "PV",
            AuxiliaryRole::ProcessVariable,
        ),
    };
    let mut selectable = vec![SelectableSignal {
        id: id.to_string(),
        symbol: primary.symbol.clone(),
        address: primary.address.clone(),
        iec_type: primary.iec_type.clone(),
        unit: primary.unit.clone(),
        bounds: primary.range,
        channel: None,
        role: role.to_string(),
        order: 0,
    }];
    let auxiliary = signals
        .additional
        .iter()
        .flatten()
        .filter(|signal| signal.role == wanted);
    for (index, signal) in auxiliary.enumerate() {
        selectable.push(SelectableSignal {
            id: signal.id.clone(),
            symbol: signal.symbol.clone(),
            address: signal.address.clone(),
            iec_type: signal.iec_type.clone(),
            unit: signal.unit.clone(),
            bounds: signal.range,
            channel: Some(signal.channel.clone()),
            role: role_name(signal.role).to_string(),
            order: index + 1,
        });
    }
    selectable
}

/// Whether the loop's adapter can realize it (C0, `adapterCompatibility`). A loop that declares nothing to adapt is unconfigured; an
/// adapter's refusal is the loop's reason, kept whole.
fn adapter_compatibility(loop_definition: &LoopDefinition) -> ControllerCompatibility {
    let controller = &loop_definition.controller;
    if loop_definition.parameter_binding.is_none()
        && controller.law.is_none()
        && controller.project.is_none()
        && controller.on_off.is_none()
        && controller.state_feedback.is_none()
    {
        return ControllerCompatibility::Unconfigured;
    }
    match controller_adapter(loop_definition) {
        Ok(adapter) => ControllerCompatibility::Accepted {
            families: adapter
                .families()
                .iter()
                .map(|structure| family_name(*structure).to_string())
                .collect(),
        },
        Err(Failure {
            message, detail, ..
        }) => ControllerCompatibility::Rejected {
            reason: detail.unwrap_or(message),
        },
    }
}

/// The controller symbols a loop declares, as the `TypeScript` walk finds them (`declaredControllerSymbols`): every text under a key
/// named `…symbol`, `…symbols` or `…Signal` that is dotted and not an address, except the record's own name and revision. The walk
/// reads the parameter binding and the controller as the file declares them.
fn declared_controller_symbols(loop_definition: &LoopDefinition) -> Vec<String> {
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
fn declared_controller_symbol_types(loop_definition: &LoopDefinition) -> Vec<(String, String)> {
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
        walk_types(on_off, &mut types);
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

/// The checked channel identities of one side: at least one, every declared channel once, each a declared identity.
fn check_channels(
    requested: &[String],
    available: &[SelectableSignal],
    label: &str,
) -> Result<(), Failure> {
    if requested.is_empty() {
        return Err(Failure::new(
            &texts::choose_at_least_one(label),
            Cause::Settings,
            NextAction::None,
        ));
    }
    if requested.len() != available.len() {
        return Err(Failure::new(
            &texts::select_every(label),
            Cause::Settings,
            NextAction::None,
        ));
    }
    let mut seen: Vec<&str> = Vec::new();
    for id in requested {
        if seen.contains(&id.as_str()) || !available.iter().any(|signal| signal.id == *id) {
            return Err(Failure::new(
                &texts::channel_not_declared(label),
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            ));
        }
        seen.push(id.as_str());
    }
    Ok(())
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

/// The name of an auxiliary signal's role, as the file writes it.
fn role_name(role: AuxiliaryRole) -> &'static str {
    match role {
        AuxiliaryRole::Setpoint => "SP",
        AuxiliaryRole::ProcessVariable => "PV",
        AuxiliaryRole::ManipulatedVariable => "MV",
        AuxiliaryRole::Actuator => "actuator",
        AuxiliaryRole::Disturbance => "disturbance",
        AuxiliaryRole::State => "state",
        AuxiliaryRole::Quality => "quality",
        AuxiliaryRole::Schedule => "schedule",
    }
}

/// The name of a structure as the setup page shows it.
fn family_name(structure: Structure) -> &'static str {
    match structure {
        Structure::P => "P",
        Structure::PI => "PI",
        Structure::PD => "PD",
        Structure::PID => "PID",
    }
}

/// The side of a loop a selector offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Input,
    Output,
}

#[cfg(test)]
#[path = "discovery_tests.rs"]
mod tests;
