//! The setup form's model (specification 35, B1.3 and C0; `loopSetup/setupFormModel.ts`): what the compiled program shows, and only
//! the choices a loop can use. The model is read from the compiled catalog alone; no name is guessed from the program's text.
//!
//! The `TypeScript` model also lists the model files an LQG estimator can implement (`estimatorModels`). They belong to the LQG
//! structure (C4), which is parked until stage V7, so they are not ported here.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::Serialize;

use super::catalog::{
    cascade_pairs, pid_candidates, pid_parameter_records, CascadePair, CompiledCatalog,
    CompiledSymbol, PidCandidate, PID_TYPE,
};
use crate::loop_definition::ModeWhenTrue;

/// What the program states for one library PID, offered as the form's preselection (B1.3). Only the signals the form offers are
/// named, so the engineer confirms every prefilled binding.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PidPrefill {
    /// The PID's instance.
    pub instance: String,
    /// The parameter record the PID's Configure call reads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_record: Option<String>,
    /// The output the PID's Update call writes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manipulated_variable: Option<String>,
    /// The setpoint the PID's Update call reads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setpoint: Option<String>,
    /// The measurement: the PID's own, or a signal the program reads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_value: Option<String>,
    /// The manual output the program steps by name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manual_output: Option<String>,
    /// The Auto/Manual signal the code tests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// What the mode signal's TRUE value means.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode_when_true: Option<ModeWhenTrue>,
    /// The flag the code tests in `IF NOT <flag>` around the PID's Initialize.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub startup_flag: Option<String>,
    /// A measurement the program computes, which a simulated measurement could not reach.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computed: Option<ComputedPrefill>,
}

/// A measurement the program computes for a PID (B1.3).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ComputedPrefill {
    /// The computed signal's symbol.
    pub process_value: String,
}

/// The setup form's model of one compiled program (B1.3).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetupFormModel {
    /// The structure the program shows: `cascade`, `group`, `pid` or `identify`.
    pub structure: String,
    /// The library PIDs of the program, in the compiled order.
    pub pids: Vec<PidCandidate>,
    /// The outer and inner PID of a cascade: the first pair the program shows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cascade: Option<CascadePair>,
    /// The REAL and LREAL signals with an I/O address.
    pub addressed_real: Vec<String>,
    /// The REAL and LREAL signals a loop can use as an output or a setpoint, addressed or not; a PID's own records and states are
    /// not signals.
    pub signals: Vec<String>,
    /// The REAL and LREAL signals a simulated plant can write as a measurement: inputs, program variables without an address that
    /// the program only reads, and the measurement each library PID holds.
    pub measurements: Vec<String>,
    /// The BOOL symbols.
    pub booleans: Vec<String>,
    /// The BOOL symbols without an I/O address: the only ones the controller transfer switches.
    pub modes: Vec<String>,
    /// The parameter records of complete library PIDs.
    pub records: Vec<String>,
    /// The program and function block instances, other than library PIDs, that run in one cyclic task.
    pub instances: Vec<String>,
    /// Every REAL and LREAL symbol: Studio writes controller settings directly, so they need no address.
    pub reals: Vec<String>,
    /// The symbols the program assigns, as the compiler reports them, in the order it reports them.
    pub writes: Vec<String>,
    /// The library PIDs whose Initialize the program calls.
    pub initializes: Vec<String>,
    /// What each library PID's own calls say, for the controller the engineer chooses.
    pub prefills: Vec<PidPrefill>,
    /// The only PID's prefill, when the structure is `pid`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefill: Option<PidPrefill>,
}

/// The setup form's model of a compiled program (B1.3, `setupFormModel`). The structure is `cascade` when one PID's output is the
/// other's setpoint, `group` for several PIDs, `pid` for one, and `identify` when the program has no library PID.
#[must_use]
#[allow(clippy::too_many_lines)] // One form model, built field by field in the order setupFormModel.ts builds it.
pub fn setup_form_model(catalog: &CompiledCatalog) -> SetupFormModel {
    let pids = pid_candidates(catalog);
    let pairs = cascade_pairs(&pids);
    let records = pid_parameter_records(catalog);
    let written: BTreeSet<String> = catalog
        .writes
        .iter()
        .map(|symbol| symbol.to_ascii_uppercase())
        .collect();
    let owners: Vec<&str> = records
        .iter()
        .map(String::as_str)
        .chain(pids.iter().map(|pid| pid.instance.as_str()))
        .collect();
    let internal = |symbol: &str| {
        owners.iter().any(|owner| {
            symbol
                .strip_prefix(*owner)
                .is_some_and(|rest| rest.starts_with('.'))
        })
    };
    let addressed_real = sorted(
        catalog
            .symbols
            .iter()
            .filter(|entry| is_real(entry) && has_address(entry))
            .map(symbol_of)
            .collect(),
    );
    let booleans = sorted(
        catalog
            .symbols
            .iter()
            .filter(|entry| entry.iec_type == "BOOL")
            .map(symbol_of)
            .collect(),
    );
    let modes = sorted(
        catalog
            .symbols
            .iter()
            .filter(|entry| entry.iec_type == "BOOL" && !has_address(entry))
            .map(symbol_of)
            .collect(),
    );
    let reals = sorted(
        catalog
            .symbols
            .iter()
            .filter(|entry| is_real(entry))
            .map(symbol_of)
            .collect(),
    );
    let signals = sorted(
        catalog
            .symbols
            .iter()
            .filter(|entry| is_real(entry) && !internal(&entry.symbol))
            .map(symbol_of)
            .collect(),
    );
    // A PID's own measurement is what its Update received, wherever the program computed it from (a sensor block, a filter).
    let mut measurements: Vec<String> = catalog
        .symbols
        .iter()
        .filter(|entry| {
            is_real(entry) && !internal(&entry.symbol) && is_measurement(entry, &written)
        })
        .map(symbol_of)
        .collect();
    measurements.extend(pids.iter().filter_map(|pid| pid.own_measurement.clone()));
    let measurements = sorted(measurements);
    let instances = sorted(
        catalog
            .controllers
            .iter()
            .filter(|controller| {
                controller.type_name.to_ascii_uppercase() != PID_TYPE
                    && matches!(controller.tasks.as_slice(), [task] if task.interval_ms > 0.0)
            })
            .map(|controller| controller.instance.clone())
            .collect(),
    );
    let initializes = sorted(
        pids.iter()
            .filter(|pid| {
                catalog.links.iter().any(|link| {
                    link.instance == pid.instance && link.method.eq_ignore_ascii_case("INITIALIZE")
                })
            })
            .map(|pid| pid.instance.clone())
            .collect(),
    );
    let structure = if !pairs.is_empty() {
        "cascade"
    } else if pids.len() > 1 {
        "group"
    } else if pids.len() == 1 {
        "pid"
    } else {
        "identify"
    };
    let lists = Lists {
        records: &records,
        signals: &signals,
        measurements: &measurements,
        modes: &modes,
        booleans: &booleans,
    };
    let prefills: Vec<PidPrefill> = pids.iter().map(|pid| prefill(pid, &lists)).collect();
    let single = if structure == "pid" {
        prefills.first().cloned()
    } else {
        None
    };
    SetupFormModel {
        structure: structure.to_string(),
        pids,
        cascade: pairs.first().cloned(),
        addressed_real,
        signals,
        measurements,
        booleans,
        modes,
        records,
        instances,
        reals,
        writes: catalog.writes.clone(),
        initializes,
        prefills,
        prefill: single,
    }
}

/// The lists a prefill checks its names against: only what the form offers is preselected.
struct Lists<'a> {
    records: &'a [String],
    signals: &'a [String],
    measurements: &'a [String],
    modes: &'a [String],
    booleans: &'a [String],
}

/// What a library PID's own calls say, restricted to what the form offers (`setupFormModel`, prefills).
fn prefill(pid: &PidCandidate, lists: &Lists<'_>) -> PidPrefill {
    let mut prefill = PidPrefill {
        instance: pid.instance.clone(),
        parameter_record: offered(pid.parameter_record.as_ref(), lists.records),
        manipulated_variable: offered(pid.output.as_ref(), lists.signals),
        setpoint: offered(pid.setpoint.as_ref(), lists.signals),
        process_value: None,
        manual_output: offered(pid.manual_output.as_ref(), lists.signals),
        mode: None,
        mode_when_true: None,
        startup_flag: offered(pid.startup_flag.as_ref(), lists.booleans),
        computed: None,
    };
    if let Some(own) = &pid.own_measurement {
        prefill.process_value = Some(own.clone());
    } else if let Some(value) = offered(pid.process_value.as_ref(), lists.measurements) {
        prefill.process_value = Some(value);
    } else if let Some(value) = offered(pid.process_value.as_ref(), lists.signals) {
        prefill.computed = Some(ComputedPrefill {
            process_value: value,
        });
    }
    if let Some(mode) = pid
        .mode
        .as_ref()
        .filter(|mode| lists.modes.contains(&mode.symbol))
    {
        prefill.mode = Some(mode.symbol.clone());
        prefill.mode_when_true = Some(mode.when_true);
    }
    prefill
}

/// The choice when the offered list names it.
fn offered(choice: Option<&String>, list: &[String]) -> Option<String> {
    choice.filter(|name| list.contains(*name)).cloned()
}

/// Whether a symbol is a REAL or LREAL (`setupFormModel`: the type is compared as the compiler writes it).
fn is_real(entry: &CompiledSymbol) -> bool {
    entry.iec_type == "REAL" || entry.iec_type == "LREAL"
}

/// Whether a symbol has an I/O address (an empty address is no address).
fn has_address(entry: &CompiledSymbol) -> bool {
    entry
        .address
        .as_deref()
        .is_some_and(|address| !address.is_empty())
}

/// Whether a REAL can be a simulated measurement: an input (an address starting `%I`), or a program variable without an address
/// that the program only reads.
fn is_measurement(entry: &CompiledSymbol, written: &BTreeSet<String>) -> bool {
    match entry.address.as_deref() {
        Some(address) if !address.is_empty() => address.to_ascii_uppercase().starts_with("%I"),
        _ => !written.contains(&entry.symbol.to_ascii_uppercase()),
    }
}

/// The symbol of a compiled entry.
fn symbol_of(entry: &CompiledSymbol) -> String {
    entry.symbol.clone()
}

/// The names in order.
fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names
}

#[cfg(test)]
#[path = "form_tests.rs"]
mod tests;
