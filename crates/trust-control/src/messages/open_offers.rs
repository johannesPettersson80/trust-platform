//! The offers that the open answer makes to the pages (specification 35, B3.6, B4.2, B4.3, C2 and C4): the controller families the
//! selector lists with their labels, the family it shows until the engineer chooses one, the settings each family takes and the rows of
//! the manual editor, the setpoints that a loop without a declared setpoint may take, and the scenario that the page adds to its list.
//! The engine computes each one; the page shows it and keeps no copy of the rule.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::Serialize;

use super::tune_words::{group_setting_symbols, manual_setting_label};
use super::TuneResult;
use crate::loop_definition::{AuxiliaryRole, LoopDefinition};
use crate::setup::discovery::{adapter_compatibility, ControllerCompatibility};
use crate::tuning::settings_pins::ControllerSettingsPins;
use crate::tuning::state_controller::offer::family_name as state_family_name;
use crate::tuning::structure::Structure;
use crate::validation::requirement_record::{RunCheck, Window};
use crate::validation::run_validation::RunRequirement;
use crate::validation::scenario_contract::EngineerScenario;

/// One family that the controller selector lists: its name, as the engine names it, and the label the selector shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FamilyOffer {
    /// The family's name: `P`, `PI`, `PD`, `PID`, `ON/OFF`, `state-feedback`, `LQR`, `LQG` or a program's family.
    pub family: String,
    /// The label the selector shows for the family.
    pub label: String,
}

/// One row of the manual editor (B4.2): a setting of the loop that a family takes, with its label, its unit and its bounds.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SettingRow {
    /// The setting's symbol, as the loop's parameter binding names it.
    pub symbol: String,
    /// The label of the row.
    pub label: String,
    /// The setting's unit, as the parameter binding names it: `1` for a number without a unit.
    pub unit: String,
    /// The smallest value that the engine accepts for the setting.
    pub minimum: f64,
    /// The largest value that the engine accepts for the setting.
    pub maximum: f64,
}

/// The controller that the Tune page offers a loop (C2, C4, B4.2): the families the selector lists, the family it shows by default,
/// the settings each family takes and the rows of the manual editor.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ControllerOffer {
    /// The families the selector lists, in the engine's order.
    pub families: Vec<FamilyOffer>,
    /// The family the selector marks recommended (C4), when the engine names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended: Option<String>,
    /// The family the selector shows until the engineer chooses one: the family of the study's candidate, else the family of its pins
    /// (B4.2). Absent, which is Automatic, when the study holds neither.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_family: Option<String>,
    /// The settings each family takes, by the family's name: the rows the manual editor shows for that family.
    pub family_settings: BTreeMap<String, Vec<String>>,
    /// Every setting that a family takes, in the order of the loop's parameter binding. The page shows the rows of the chosen family.
    pub setting_rows: Vec<SettingRow>,
}

/// A signal that a loop without a declared setpoint may take as its setpoint (B3.6): its identifier, its symbol and its unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetpointChoice {
    /// The signal's identifier in the loop.
    pub id: String,
    /// The signal's symbol.
    pub symbol: String,
    /// The signal's unit, which is the process variable's.
    pub unit: String,
}

/// The controller that the Tune page offers a loop (C2, C4, B4.2), or `None` when the engine adapts no controller of the loop. The
/// families and their settings are the engine's rule (`adapter_compatibility`); a group's settings are its group's (`group_setting_symbols`).
#[must_use]
pub fn controller_offer(
    loop_definition: &LoopDefinition,
    default_family: Option<String>,
) -> Option<ControllerOffer> {
    let ControllerCompatibility::Accepted {
        families,
        family_settings,
        recommended_family,
        ..
    } = adapter_compatibility(loop_definition)
    else {
        return None;
    };
    let family_settings: BTreeMap<String, Vec<String>> =
        match group_setting_symbols(loop_definition) {
            Some(group) => families
                .iter()
                .map(|family| (family.clone(), group.iter().cloned().collect()))
                .collect(),
            None => family_settings,
        };
    let settings: BTreeSet<&str> = family_settings
        .values()
        .flatten()
        .map(String::as_str)
        .collect();
    let setting_rows =
        loop_definition
            .parameter_binding
            .as_ref()
            .map_or_else(Vec::new, |binding| {
                binding
                    .parameters
                    .iter()
                    .filter(|parameter| settings.contains(parameter.symbol.as_str()))
                    .map(|parameter| SettingRow {
                        symbol: parameter.symbol.clone(),
                        label: manual_setting_label(loop_definition, parameter),
                        unit: parameter.unit.clone(),
                        minimum: parameter.minimum,
                        maximum: parameter.maximum,
                    })
                    .collect()
            });
    Some(ControllerOffer {
        families: families
            .iter()
            .map(|family| FamilyOffer {
                family: family.clone(),
                label: family_label(family),
            })
            .collect(),
        recommended: recommended_family,
        default_family,
        family_settings,
        setting_rows,
    })
}

/// The label that the selector shows for a family (C2, C4): the state and program families by their words, and each other family by
/// its name.
fn family_label(family: &str) -> String {
    let label = match family {
        "state-feedback" => "State feedback",
        "cascade" => "Cascade",
        "feedforward" => "Feedforward",
        "ratio" => "Ratio",
        "split-range" => "Split range",
        "override" => "Override",
        "Smith" => "Smith predictor",
        "paired-PI" => "Paired PI",
        "decoupled-PI" => "Decoupled PI",
        "gain-scheduled" => "Gain schedule",
        "adaptive" => "Adaptive",
        "robust" => "Robust",
        "feedback-linearization" => "Feedback linearization",
        "observer-feedback" => "Observer feedback",
        "lead-lag" => "Lead/lag",
        other => other,
    };
    label.to_owned()
}

/// The name of a `Control.PID` or program structure as the selector names it: `P`, `PI`, `PD` or `PID`.
fn structure_family(structure: Structure) -> String {
    let name = match structure {
        Structure::P => "P",
        Structure::PI => "PI",
        Structure::PD => "PD",
        Structure::PID => "PID",
    };
    name.to_owned()
}

/// The family that the Tune page shows until the engineer chooses one (B4.2): the family of the study's candidate, else the family of
/// its pins. `None` is Automatic, which the page shows when the study holds neither.
#[must_use]
pub fn default_family(
    tune: Option<&TuneResult>,
    pins: Option<&ControllerSettingsPins>,
) -> Option<String> {
    tune.and_then(candidate_family)
        .or_else(|| pins.map(|pins| structure_family(pins.family)))
}

/// The family of a Tune answer's candidate: its state law's family, else the structure of its `Control.PID` or program's candidate.
fn candidate_family(tune: &TuneResult) -> Option<String> {
    tune.state
        .as_ref()
        .map(|state| state_family_name(state.family).to_owned())
        .or_else(|| {
            tune.candidate
                .as_ref()
                .map(|candidate| structure_family(candidate.structure))
        })
        .or_else(|| {
            tune.project_candidate
                .as_ref()
                .map(|project| structure_family(project.structure))
        })
}

/// The setpoints that a loop without a declared setpoint may take (B3.6): the additional signals whose role is the setpoint and whose
/// unit is the process variable's. `None` when the loop declares its setpoint, so that the page offers no choice.
#[must_use]
pub fn setpoint_choices(loop_definition: &LoopDefinition) -> Option<Vec<SetpointChoice>> {
    if loop_definition.signals.setpoint.is_some() {
        return None;
    }
    let unit = loop_definition.signals.process_variable.unit.as_str();
    Some(
        loop_definition
            .signals
            .additional
            .iter()
            .flatten()
            .filter(|signal| matches!(signal.role, AuxiliaryRole::Setpoint) && signal.unit == unit)
            .map(|signal| SetpointChoice {
                id: signal.id.clone(),
                symbol: signal.symbol.clone(),
                unit: signal.unit.clone(),
            })
            .collect(),
    )
}

/// The scenario that the page adds to its list (B4.3): one bounds check of the process variable over the validation run, with no name, no
/// initial values and no events. The page gives it no identity: a scenario whose name is blank takes the first identity `scenario-N`
/// that no other scenario of the list holds when the list changes (`validation::scenario_form`). `duration_seconds` is the study's
/// validation duration, or zero when the study holds no requirements.
#[must_use]
pub fn scenario_template(
    loop_definition: &LoopDefinition,
    duration_seconds: f64,
) -> EngineerScenario {
    let process = &loop_definition.signals.process_variable;
    EngineerScenario {
        id: String::new(),
        model: None,
        operating_outputs: None,
        effects: None,
        initial_values: BTreeMap::new(),
        events: Vec::new(),
        requirements: vec![RunRequirement {
            id: "requirement-1".to_owned(),
            channel: process.symbol.clone(),
            window: Window {
                start: 0.0,
                end: duration_seconds,
            },
            check: RunCheck::Bounds {
                minimum: process.range.minimum,
                maximum: process.range.maximum,
            },
        }],
    }
}

/// The signal that a new initial value or timed change of a scenario starts at (B4.3): the loop's setpoint. `None` when the loop declares
/// no setpoint, which leaves the new row's signal blank for the engine to refuse.
#[must_use]
pub fn scenario_row_signal(loop_definition: &LoopDefinition) -> Option<String> {
    loop_definition
        .signals
        .setpoint
        .as_ref()
        .map(|setpoint| setpoint.symbol.clone())
}

#[cfg(test)]
#[path = "open_offers_tests.rs"]
mod tests;
