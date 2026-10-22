//! The loops of a project bound to the compiled program (specification 35, B1.2, B1.3 and B1.4; `hostBindingDiscovery.ts`).
//!
//! The engine lists and reads the loop files and the compiled catalog, then calls [`discover_bindings`]. Every rule is decided
//! here: the file bounds, why a file is refused, the duplicate ids, the join to the compiled program, the selectable channels and
//! the selection itself. Nothing here writes a file. A loop whose channels the engine does not serve is refused by name; the
//! structures that define channels (state feedback, a C12 loop, a feedforward, a cascade, ON/OFF and a PID group) are served.

use std::collections::BTreeMap;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::catalog::{CatalogAnswer, CompiledCatalog, CompiledController, CompiledTask};
use super::declared_symbols::{declared_controller_symbol_types, declared_controller_symbols};
use super::save::validation_reason_code;
use super::texts::discovery as texts;
use super::tune_method_offers::{family_settings_of, method_titles_of, named_methods_of};
use crate::commissioning::adapters::controller_adapter;
use crate::failure::{Cause, Failure, NextAction};
use crate::json::parse_text;
use crate::loop_definition::{AuxiliaryRole, LoopDefinition, LoopScope, Range};
use crate::tuning::structure::Structure;

pub use super::estimator_model_files::{estimator_model_files, EstimatorModelFile, ModelFileText};
pub use super::tune_method_offers::{study_tune_methods, TuneMethodOffer};

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
        /// The parameter symbols that each family's manual settings and pins take, by the family's name (C0, C2). The page asks
        /// for these settings only, and Tune refuses any other.
        #[serde(rename = "familySettings")]
        family_settings: BTreeMap<String, Vec<String>>,
        /// The family the page marks recommended (C4): the family a state law declares, when it offers it. Absent for a PID law.
        #[serde(
            default,
            rename = "recommendedFamily",
            skip_serializing_if = "Option::is_none"
        )]
        recommended_family: Option<String>,
        /// The tuning methods that the controller takes (C4, B4.2): a state law takes the state-space design alone. A finite search and
        /// the loops that Tune designs by Automatic alone (a feedforward, a Smith predictor, a cascade, a gain schedule, a switching
        /// controller and a PID group) take none, an empty list. A `Control.PID`, a proportional law and a program's parallel-PID or
        /// trajectory-feedforward law take SIMC, AMIGO and Manual, in that order. The field is absent when the answer names no method.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        methods: Option<Vec<crate::messages::TuneMethod>>,
        /// The title of each method of `methods`, keyed by the method's word (`SIMC`, `AMIGO`, `manual`, `state-space`), as the engine
        /// gives it (`TuneMethod::title`). Absent when there is no method.
        #[serde(
            default,
            rename = "methodTitles",
            skip_serializing_if = "Option::is_none"
        )]
        method_titles: Option<BTreeMap<String, String>>,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
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
    // The engine's gate refuses a loop with two controller structures; discovery offers no such loop, with the gate's sentence.
    loop_definition
        .require_one_controller_structure()
        .map_err(|failure| failure.message)?;
    let controller = check_controller(&loop_definition, &catalog.control)?;
    check_group_members(&loop_definition, &catalog.control)?;
    check_signals(&loop_definition, catalog)?;
    check_controller_symbols(&loop_definition, &catalog.control)?;
    // Channels the engine serves (state feedback, C12 measured paths, feedforward, cascade, ON/OFF and PID groups) are the loop's own.
    if loop_definition.channels.is_some() && !loop_definition.serves_channels() {
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

/// The loop's controller is in the compiled program, with its type when the loop is PID-designed, and runs in the loop's task when
/// it has tasks at all.
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
    // A PID-designed loop names the compiled block's type (`Control.PID`); a project-designed loop's type describes the controller its
    // program owns (for example "LQR state feedback" on the program `Main`), so only its instance and task are checked (B1.2).
    let project_designed = controller.design == Some(crate::loop_definition::Design::Project);
    if !project_designed && compiled_controller.type_name != controller.type_name {
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
    let period = loop_definition.controller.task_period_ms;
    let task_name = loop_definition
        .parameter_binding
        .as_ref()
        .map(|binding| binding.task_name.as_str());
    for member in &group.members {
        let instance = member.instance.as_str();
        let member_type = Some(member.type_name.as_str());
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
pub(crate) fn adapter_compatibility(loop_definition: &LoopDefinition) -> ControllerCompatibility {
    let controller = &loop_definition.controller;
    if loop_definition.parameter_binding.is_none()
        && controller.law.is_none()
        && controller.project.is_none()
        && controller.on_off.is_none()
        && controller.state_feedback.is_none()
    {
        return ControllerCompatibility::Unconfigured;
    }
    // A served program's own law (C2) is accepted with the families its seed names.
    if let Some(project) = controller
        .project
        .as_ref()
        .filter(|project| project.parked_feature().is_none())
    {
        // A finite search (C5) is tuned by its search alone: its declared family, its coefficient variables, and Automatic as its
        // one method (an empty list of named methods).
        if project.is_search_law() {
            let symbols = project
                .axes
                .iter()
                .map(|axis| axis.symbol.clone())
                .collect();
            return ControllerCompatibility::Accepted {
                families: vec![project.family.clone()],
                family_settings: BTreeMap::from([(project.family.clone(), symbols)]),
                recommended_family: Some(project.family.clone()),
                methods: Some(Vec::new()),
                method_titles: None,
            };
        }
        let families = project.offered_families();
        let methods = named_methods_of(loop_definition);
        return ControllerCompatibility::Accepted {
            families: families
                .iter()
                .map(|structure| family_name(*structure).to_string())
                .collect(),
            family_settings: family_settings_of(loop_definition, &families),
            recommended_family: None,
            method_titles: method_titles_of(&methods),
            methods: Some(methods),
        };
    }
    // A state-feedback law is offered as the families its inputs and estimator take, with the family it declares recommended and
    // the state-space design as its one method (C4).
    if controller.state_feedback.is_some() {
        return match crate::tuning::state_controller::offer::offer(loop_definition) {
            Ok(offer) => ControllerCompatibility::Accepted {
                families: offer.families,
                family_settings: offer.family_settings,
                recommended_family: Some(offer.recommended),
                method_titles: method_titles_of(&[crate::messages::TuneMethod::StateSpace]),
                methods: Some(vec![crate::messages::TuneMethod::StateSpace]),
            },
            Err(Failure {
                message, detail, ..
            }) => ControllerCompatibility::Rejected {
                reason: detail.unwrap_or(message),
            },
        };
    }
    match controller_adapter(loop_definition) {
        Ok(adapter) => {
            let families = adapter.families();
            let methods = named_methods_of(loop_definition);
            ControllerCompatibility::Accepted {
                families: families
                    .iter()
                    .map(|structure| family_name(*structure).to_string())
                    .collect(),
                family_settings: family_settings_of(loop_definition, &families),
                recommended_family: None,
                method_titles: method_titles_of(&methods),
                methods: Some(methods),
            }
        }
        Err(Failure {
            message, detail, ..
        }) => ControllerCompatibility::Rejected {
            reason: detail.unwrap_or(message),
        },
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
pub(super) fn family_name(structure: Structure) -> &'static str {
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
