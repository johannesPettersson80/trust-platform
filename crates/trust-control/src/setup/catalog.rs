//! The compiled facts that loop setup reads (specification 35, B1.3 and §17.1): symbols with their IEC types and optional
//! addresses, controller instances with their tasks, the code links of each call, what the program writes, and the
//! process-image signals. Ported from `compiledCatalog.ts`, and from `normalizeCatalog` in `hostBindingDiscovery.ts`, which
//! sets the entry limits and refuses conflicting identities. A malformed entry is dropped, never repaired, and nothing is
//! inferred from a name.

#![allow(clippy::module_name_repetitions)] // `CompiledCatalog` and `CatalogAnswer` name the module's subject.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Map, Value};

use super::texts::generators as texts;
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::ModeWhenTrue;

pub use crate::commissioning::pid_measurement::pid_own_measurement as own_measurement;

/// The type of the library PID, compared without regard to case (`PID_TYPE`).
pub const PID_TYPE: &str = "CONTROL.PID";

/// The eleven numeric fields of a `Control.PID` parameter record (C0; `PID_PARAMETERS`).
pub const PID_PARAMETERS: [&str; 11] = [
    "ProportionalGain",
    "IntegralGain",
    "DerivativeGain",
    "ProportionalSetpointWeight",
    "DerivativeSetpointWeight",
    "Bias",
    "OutputMinimum",
    "OutputMaximum",
    "RateMinimum",
    "RateMaximum",
    "TrackingTime",
];

/// The most entries one list of a compiled answer may hold: the discovery's bounded entry limit (`MAX_CATALOG_ENTRIES`).
pub const MAXIMUM_CATALOG_ENTRIES: usize = 16_384;

/// A compiled symbol: its path, its IEC type and its I/O address when it has one (§17.1).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompiledSymbol {
    /// The symbol's path, for example `Main.TIC101.StateValue.ProcessValue`.
    pub symbol: String,
    /// The IEC type name as the compiler states it.
    pub iec_type: String,
    /// The direct address, when the variable has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

/// A task of the compiled program: its name and its interval in ms.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompiledTask {
    /// The task's name.
    pub name: String,
    /// The task's interval, in ms.
    pub interval_ms: f64,
}

/// A controller instance and the tasks that run it.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompiledController {
    /// The instance's path.
    pub instance: String,
    /// The compiled type, for example `Control.PID` or a program's type.
    pub type_name: String,
    /// The tasks that run the instance.
    pub tasks: Vec<CompiledTask>,
}

/// The Auto/Manual condition the code tests to set a PID's mode, and the mode its TRUE value means (B1.4).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModeEvidence {
    /// The BOOL the condition tests.
    pub symbol: String,
    /// The mode the BOOL's TRUE value chooses.
    pub when_true: ModeWhenTrue,
}

/// One call of a controller in the code: `<output> := <instance>.<method>(<argument> := <record>, …)`, with the variables each
/// input field is read from.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CallLink {
    /// The instance called.
    pub instance: String,
    /// The method called, as written.
    pub method: String,
    /// The argument that receives the record.
    pub argument: String,
    /// The record the call passes.
    pub record: String,
    /// The variable that receives the call's output, when the call has one.
    pub output: Option<String>,
    /// For each field of the record, the variables the code assigns to it.
    pub fields: BTreeMap<String, Vec<String>>,
    /// The BOOL of an enclosing `IF NOT <flag> THEN`: the call runs only until its startup is done.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guarded_by: Option<String>,
    /// The condition that sets the record's `Mode`, and the mode it chooses while TRUE.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<ModeEvidence>,
    /// Variables the code assigns the call's output unchanged, such as a manual output that follows it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub copies_of_output: Vec<String>,
}

/// The compiled program: symbols, controller instances, call links and the symbols its code assigns (§17.1).
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledCatalog {
    /// Every compiled symbol, in the order the compiler lists them.
    pub symbols: Vec<CompiledSymbol>,
    /// Every controller instance.
    pub controllers: Vec<CompiledController>,
    /// Every call link of the code.
    pub links: Vec<CallLink>,
    /// Every symbol the program's code assigns, sorted and without repetition.
    pub writes: Vec<String>,
}

/// A process-image signal: a symbol with the address it is bound to (`CompiledControlSignal`).
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledSignal {
    /// The symbol's path.
    pub symbol: String,
    /// The direct address the signal is bound to.
    pub address: String,
    /// The IEC type name.
    pub iec_type: String,
}

/// One `trust-lsp.hmiBindings` answer: its control symbols and its process-image signals.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogAnswer {
    /// The control symbols, links and writes that loop setup reads.
    pub control: CompiledCatalog,
    /// The process-image signals, one per symbol, sorted by symbol.
    pub signals: Vec<CompiledSignal>,
}

/// What the compiled program already says about one library PID; every entry is optional evidence (B1.3, C1).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PidCandidate {
    /// The PID's instance path.
    pub instance: String,
    /// The task that runs the PID, when it runs in exactly one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<CompiledTask>,
    /// The record its `Configure` receives.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_record: Option<String>,
    /// The variable its `Update` output is written to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// The variable its `Update` receives as the setpoint, when only one is given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setpoint: Option<String>,
    /// The variable its `Update` receives as the process value, when only one is given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_value: Option<String>,
    /// The variable its `Update` receives as the manual output, when only one is given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manual_output: Option<String>,
    /// The measurement the PID holds itself: `<instance>.StateValue.ProcessValue`, when the compiled program has it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub own_measurement: Option<String>,
    /// The condition that sets the PID's mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<ModeEvidence>,
    /// The flag the code tests in `IF NOT <flag>` around the PID's `Initialize`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub startup_flag: Option<String>,
    /// The code copies the PID's output into its manual output, so Manual starts where Auto left off.
    #[serde(skip_serializing_if = "is_false")]
    pub manual_output_follows_output: bool,
}

/// A pair of PIDs in which the outer PID's output is the only source of the inner PID's setpoint (B1.3).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CascadePair {
    /// The outer PID.
    pub outer: String,
    /// The inner PID.
    pub inner: String,
}

/// Decodes one `trust-lsp.hmiBindings` answer (`normalizeCatalog`, then `decodeCompiledCatalog`). The entry limits are checked
/// on the raw lists, before any malformed entry is dropped; the identities of the signals, symbols and controllers must agree.
///
/// # Errors
/// The answer is not ok; a list holds more than [`MAXIMUM_CATALOG_ENTRIES`] entries; one symbol, signal or instance has two
/// identities or types; or the answer carries no control-symbol lists.
pub fn decode_catalog_answer(answer: &Value) -> Result<CatalogAnswer, Failure> {
    let Some(answer) = answer.as_object() else {
        return Err(refusal(texts::catalog_unavailable()));
    };
    if answer.get("ok").is_some_and(|ok| *ok != Value::Bool(true)) {
        let reason = answer
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or(texts::catalog_unavailable());
        return Err(refusal(reason));
    }
    let image = answer.get("processImage").and_then(Value::as_object);
    let image_lists = [
        items(image, "commands"),
        items(image, "feedbacks"),
        items(image, "memory"),
        items(image, "booleans"),
    ];
    if image_lists
        .iter()
        .flatten()
        .any(|list| list.len() > MAXIMUM_CATALOG_ENTRIES)
    {
        return Err(Failure::new(
            texts::process_image_limit(),
            Cause::Settings,
            NextAction::None,
        ));
    }
    let control_symbols = answer.get("controlSymbols").and_then(Value::as_object);
    let symbol_list = items(control_symbols, "symbols");
    let controller_list = items(control_symbols, "controllers");
    if [symbol_list, controller_list]
        .into_iter()
        .flatten()
        .any(|list| list.len() > MAXIMUM_CATALOG_ENTRIES)
    {
        return Err(Failure::new(
            texts::control_symbol_limit(),
            Cause::Settings,
            NextAction::None,
        ));
    }
    let (Some(symbols), Some(controllers)) = (symbol_list, controller_list) else {
        return Err(refusal(texts::catalog_not_compiled()));
    };
    let signals = image_signals(&image_lists)?;
    let control = CompiledCatalog {
        symbols: compiled_symbols(symbols)?,
        controllers: compiled_controllers(controllers)?,
        links: call_links(control_symbols),
        writes: writes(control_symbols),
    };
    Ok(CatalogAnswer { control, signals })
}

/// Every compiled library PID with the evidence its own calls provide. Nothing is inferred from names (`pidCandidates`).
#[must_use]
pub fn pid_candidates(catalog: &CompiledCatalog) -> Vec<PidCandidate> {
    catalog
        .controllers
        .iter()
        .filter(|controller| controller.type_name.to_uppercase() == PID_TYPE)
        .map(|controller| candidate(catalog, controller))
        .collect()
}

/// The records that hold every numeric `Control.PID` parameter, so that one can be bound as a loop's parameter record, sorted
/// (`pidParameterRecords`). A PID's own copy `<instance>.ParametersValue` is not one when the code configures that PID from a
/// record of its own: Configure would overwrite whatever Apply wrote there on the next scan (C1 Setup).
#[must_use]
pub fn pid_parameter_records(catalog: &CompiledCatalog) -> Vec<String> {
    let internal: BTreeSet<String> = catalog
        .links
        .iter()
        .filter(|link| {
            link.method.to_uppercase() == "CONFIGURE"
                && link.record.to_uppercase() != parameters_value(&link.instance)
        })
        .map(|link| parameters_value(&link.instance))
        .collect();
    let mut fields: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for entry in &catalog.symbols {
        if !matches!(entry.iec_type.as_str(), "REAL" | "LREAL") {
            continue;
        }
        let Some(cut) = entry.symbol.rfind('.').filter(|cut| *cut > 0) else {
            continue;
        };
        let (record, field) = (&entry.symbol[..cut], &entry.symbol[cut + 1..]);
        if PID_PARAMETERS.contains(&field) {
            fields.entry(record).or_default().insert(field);
        }
    }
    fields
        .into_iter()
        .filter(|(record, present)| {
            present.len() == PID_PARAMETERS.len() && !internal.contains(&record.to_uppercase())
        })
        .map(|(record, _)| record.to_string())
        .collect()
}

/// Pairs in which one PID's output (or call) is the only source of another PID's setpoint, in inner-then-outer order
/// (`cascadePairs`).
#[must_use]
pub fn cascade_pairs(candidates: &[PidCandidate]) -> Vec<CascadePair> {
    candidates
        .iter()
        .enumerate()
        .flat_map(|(inner_index, inner)| {
            candidates
                .iter()
                .enumerate()
                .filter(move |(outer_index, outer)| {
                    *outer_index != inner_index && feeds(outer, inner)
                })
                .map(move |(_, outer)| CascadePair {
                    outer: outer.instance.clone(),
                    inner: inner.instance.clone(),
                })
        })
        .collect()
}

/// A refusal of the compiled answer that the implementation or its compile causes; nothing to do from the setup.
fn refusal(text: &str) -> Failure {
    Failure::new(text, Cause::Implementation, NextAction::None)
}

/// The array under `key` of a JSON object, when the key holds an array.
fn items<'a>(object: Option<&'a Map<String, Value>>, key: &str) -> Option<&'a [Value]> {
    object?.get(key)?.as_array().map(Vec::as_slice)
}

/// A non-empty string under `key` of a JSON object (`text` in `compiledCatalog.ts`).
fn text(object: &Map<String, Value>, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// Every non-empty string of a JSON array, or `None` when the value is not an array of such strings.
fn all_texts(value: &Value) -> Option<Vec<String>> {
    value
        .as_array()?
        .iter()
        .map(|item| {
            item.as_str()
                .filter(|item| !item.is_empty())
                .map(str::to_string)
        })
        .collect()
}

/// The process-image signals of the lists (the plant lists and `booleans`), one per symbol. A signal listed twice must have one address
/// and one type (`normalizeCatalog`).
fn image_signals(lists: &[Option<&[Value]>]) -> Result<Vec<CompiledSignal>, Failure> {
    let mut by_symbol: BTreeMap<String, CompiledSignal> = BTreeMap::new();
    for entry in lists.iter().flatten().flat_map(|list| list.iter()) {
        let Some(signal) = image_signal(entry) else {
            continue;
        };
        if let Some(previous) = by_symbol.get(&signal.symbol)
            && (previous.address != signal.address
                || previous.iec_type.to_uppercase() != signal.iec_type.to_uppercase())
        {
            return Err(refusal(&texts::signal_identity_conflict(&signal.symbol)));
        }
        by_symbol.insert(signal.symbol.clone(), signal);
    }
    Ok(by_symbol.into_values().collect())
}

/// A process-image entry with its symbol, address and type, each non-empty (`isCompiledSignal`).
fn image_signal(entry: &Value) -> Option<CompiledSignal> {
    let object = entry.as_object()?;
    Some(CompiledSignal {
        symbol: text(object, "symbol")?,
        address: text(object, "address")?,
        iec_type: text(object, "type")?,
    })
}

/// The control symbols, in order. A symbol listed twice must have one type (`normalizeCatalog`).
fn compiled_symbols(entries: &[Value]) -> Result<Vec<CompiledSymbol>, Failure> {
    let mut types: BTreeMap<String, String> = BTreeMap::new();
    let mut symbols = Vec::new();
    for symbol in entries.iter().filter_map(compiled_symbol) {
        if let Some(previous) = types.get(&symbol.symbol)
            && previous.to_uppercase() != symbol.iec_type.to_uppercase()
        {
            return Err(refusal(&texts::symbol_type_conflict(&symbol.symbol)));
        }
        types.insert(symbol.symbol.clone(), symbol.iec_type.clone());
        symbols.push(symbol);
    }
    Ok(symbols)
}

/// A control symbol with its path and IEC type, and its address when it has one (`decodeCompiledCatalog`).
fn compiled_symbol(entry: &Value) -> Option<CompiledSymbol> {
    let object = entry.as_object()?;
    Some(CompiledSymbol {
        symbol: text(object, "symbol")?,
        iec_type: text(object, "type")?,
        address: text(object, "address"),
    })
}

/// The controller instances, in order. An instance listed twice must have one type; an instance whose task has no name or no
/// finite interval is refused from the identity check only, and kept with its valid tasks (`normalizeCatalog`,
/// `decodeCompiledCatalog`).
fn compiled_controllers(entries: &[Value]) -> Result<Vec<CompiledController>, Failure> {
    let mut types: BTreeMap<String, String> = BTreeMap::new();
    let mut controllers = Vec::new();
    for entry in entries {
        if let Some((instance, type_name)) = checked_identity(entry) {
            let first = types
                .entry(instance.clone())
                .or_insert_with(|| type_name.clone());
            if *first != type_name {
                return Err(refusal(&texts::controller_type_conflict(&instance)));
            }
        }
        if let Some(controller) = compiled_controller(entry) {
            controllers.push(controller);
        }
    }
    Ok(controllers)
}

/// The instance and type of a controller whose every task has a name and a finite interval (`isCompiledController`).
fn checked_identity(entry: &Value) -> Option<(String, String)> {
    let object = entry.as_object()?;
    let instance = text(object, "instance")?;
    let type_name = text(object, "type")?;
    let tasks = object.get("tasks")?.as_array()?;
    let every_task_valid = tasks.iter().all(|task| {
        task.as_object().is_some_and(|task| {
            task.get("name").is_some_and(Value::is_string)
                && task
                    .get("intervalMs")
                    .and_then(Value::as_f64)
                    .is_some_and(f64::is_finite)
        })
    });
    every_task_valid.then_some((instance, type_name))
}

/// A controller with its instance, type and the tasks that have a name and a number for their interval; the others are
/// dropped (`decodeCompiledCatalog`).
fn compiled_controller(entry: &Value) -> Option<CompiledController> {
    let object = entry.as_object()?;
    let tasks = object.get("tasks")?.as_array()?;
    Some(CompiledController {
        instance: text(object, "instance")?,
        type_name: text(object, "type")?,
        tasks: tasks.iter().filter_map(compiled_task).collect(),
    })
}

/// A task with a name and an interval, or nothing (`decodeCompiledCatalog`).
fn compiled_task(entry: &Value) -> Option<CompiledTask> {
    let object = entry.as_object()?;
    Some(CompiledTask {
        name: text(object, "name")?,
        interval_ms: object.get("intervalMs").and_then(Value::as_f64)?,
    })
}

/// The call links of the control symbols; a link that breaks a rule is dropped, and a part of it that does is left out
/// (`decodeCompiledCatalog`).
fn call_links(control: Option<&Map<String, Value>>) -> Vec<CallLink> {
    items(control, "links")
        .unwrap_or_default()
        .iter()
        .filter_map(call_link)
        .collect()
}

/// One call link: its five texts, its output (null or text), its fields and its optional parts.
fn call_link(entry: &Value) -> Option<CallLink> {
    let object = entry.as_object()?;
    let output = match object.get("output")? {
        Value::Null => None,
        output => Some(output.as_str().filter(|text| !text.is_empty())?.to_string()),
    };
    let fields = object.get("fields")?.as_object()?;
    Some(CallLink {
        instance: text(object, "instance")?,
        method: text(object, "method")?,
        argument: text(object, "argument")?,
        record: text(object, "record")?,
        output,
        fields: fields
            .iter()
            .filter_map(|(name, sources)| Some((name.clone(), all_texts(sources)?)))
            .collect(),
        guarded_by: text(object, "guardedBy"),
        mode: mode_evidence(object.get("mode")),
        copies_of_output: object
            .get("copiesOfOutput")
            .and_then(Value::as_array)
            .map(|copies| {
                copies
                    .iter()
                    .filter_map(|copy| copy.as_str().filter(|copy| !copy.is_empty()))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// The Auto/Manual condition of a call: a symbol and a mode that is `Auto` or `Manual` (`mode` in `decodeCompiledCatalog`).
fn mode_evidence(value: Option<&Value>) -> Option<ModeEvidence> {
    let object = value?.as_object()?;
    let when_true = match object.get("whenTrue")?.as_str()? {
        "Auto" => ModeWhenTrue::Auto,
        "Manual" => ModeWhenTrue::Manual,
        _ => return None,
    };
    Some(ModeEvidence {
        symbol: text(object, "symbol")?,
        when_true,
    })
}

/// The symbols the program's code assigns: non-empty, without repetition, sorted (`writes` in `decodeCompiledCatalog`).
fn writes(control: Option<&Map<String, Value>>) -> Vec<String> {
    let written: BTreeSet<String> = items(control, "writes")
        .unwrap_or_default()
        .iter()
        .filter_map(|item| item.as_str().filter(|item| !item.is_empty()))
        .map(str::to_string)
        .collect();
    written.into_iter().collect()
}

/// The candidate of one `Control.PID` controller, from its own calls (`pidCandidates`).
fn candidate(catalog: &CompiledCatalog, controller: &CompiledController) -> PidCandidate {
    let calls: Vec<&CallLink> = catalog
        .links
        .iter()
        .filter(|link| link.instance == controller.instance)
        .collect();
    let update = call(&calls, "UPDATE", "INPUT");
    let only_source = |field: &str| {
        update
            .and_then(|link| link.fields.get(field))
            .and_then(|sources| match sources.as_slice() {
                [only] if !only.is_empty() => Some(only.clone()),
                _ => None,
            })
    };
    let manual_output = only_source("ManualOutput");
    let manual_output_follows_output = manual_output
        .as_ref()
        .is_some_and(|manual| update.is_some_and(|link| link.copies_of_output.contains(manual)));
    let own = own_measurement(&controller.instance);
    PidCandidate {
        instance: controller.instance.clone(),
        task: match controller.tasks.as_slice() {
            [task] => Some(task.clone()),
            _ => None,
        },
        parameter_record: call(&calls, "CONFIGURE", "PARAMETERS")
            .map(|link| link.record.clone())
            .filter(|record| !record.is_empty()),
        output: update
            .and_then(|link| link.output.clone())
            .filter(|output| !output.is_empty()),
        setpoint: only_source("Setpoint"),
        process_value: only_source("ProcessValue"),
        manual_output,
        own_measurement: catalog
            .symbols
            .iter()
            .any(|entry| entry.symbol == own)
            .then_some(own),
        mode: update.and_then(|link| link.mode.clone()),
        startup_flag: calls
            .iter()
            .copied()
            .find(|link| link.method.to_uppercase() == "INITIALIZE" && link.guarded_by.is_some())
            .and_then(|link| link.guarded_by.clone()),
        manual_output_follows_output,
    }
}

/// The call of `method` with `argument` (both compared without regard to case).
fn call<'a>(calls: &[&'a CallLink], method: &str, argument: &str) -> Option<&'a CallLink> {
    calls.iter().copied().find(|link| {
        link.method.to_uppercase() == method && link.argument.to_uppercase() == argument
    })
}

/// Whether `outer`'s output, or its instance, is the only source of `inner`'s setpoint.
fn feeds(outer: &PidCandidate, inner: &PidCandidate) -> bool {
    inner.setpoint.as_ref().is_some_and(|setpoint| {
        outer.output.as_ref() == Some(setpoint) || *setpoint == outer.instance
    })
}

/// The PID's own copy of its parameter record, upper case, as the record-name comparison needs it.
fn parameters_value(instance: &str) -> String {
    format!("{instance}.ParametersValue").to_uppercase()
}

/// The serde skip predicate for a flag that is written only when it is set.
#[allow(clippy::trivially_copy_pass_by_ref)] // serde's `skip_serializing_if` passes the field by reference.
fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
