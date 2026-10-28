//! The form of a project test on the Identify page, and the parameter tool of a project-scripted loop (specification 35, B6.1, C12).
//! The form names the signals the test offers and which start selected, the duration and the baseline the loop declares, and the
//! parameter tool the page shows with the parameters it starts from. [`parameter_tool`] is the one decision of which tool a loop has,
//! for the form and for `controlStudio/parameters`. This module also decides when a finished test releases the experiment settings,
//! and the sentence that says how a finished test ended (C12). The page renders these; the engine decides them, and runs the test
//! through its own review.

use schemars::JsonSchema;
use serde::Serialize;

use crate::identification::policy::project_scripted;
use crate::loop_definition::{AuxiliaryRole, LoopDefinition, ParameterBinding, ParameterPurpose};
use crate::messages::{first_tool_offer, CampaignRestoration, ParameterToolOffer};

/// The duration the project test's form shows when the loop declares none (B6.1): the engine's review then names the duration it runs.
pub const DEFAULT_PROJECT_TEST_SECONDS: f64 = 30.0;

/// One signal the project test's Signals selector offers (B6.1): its identifier, its label without the unit, its unit and whether the
/// test starts with it selected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTestSignal {
    /// The identifier the page sends back: `sp`, `pv`, `mv`, `requestedMv`, or the identifier of an additional signal of the loop.
    pub id: String,
    /// The signal's symbol and its role, as the selector names it.
    pub label: String,
    /// The signal's unit, which the page shows after the label; absent for a signal without one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Whether the test starts with the signal selected.
    pub selected: bool,
}

/// The parameter tool the page shows for a project-scripted loop: the experiment's own settings on the Identify page, or the parameters
/// of a declared analysis on the Tune page (B6.1, C12). `controlStudio/parameters` serves both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ParameterTool {
    /// The experiment's declared settings (`parameterPurpose: "experiment"`), shown with the project test on the Identify page.
    Experiment,
    /// The declared analysis's parameters, shown after the analysis they serve.
    Analysis,
}

/// The form of a project test (B6.1, C12): what the Identify page shows before the engine's review answers.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTestForm {
    /// The signals the test offers, in the order the selector lists them.
    pub signals: Vec<ProjectTestSignal>,
    /// The duration the test shows, in seconds: the loop's declared duration, or the default when it declares none.
    pub duration_seconds: f64,
    /// The baseline the test shows before its activation, in seconds; absent when the loop declares none, or declares none above zero.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_seconds: Option<f64>,
    /// The parameter tool the page shows; absent when the loop has none (see [`parameter_tool`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter_tool: Option<ParameterTool>,
    /// The parameters the tool starts from (B6.1, C12). The experiment's settings are the loop's whole binding. A declared analysis's
    /// tool starts with a binding that lists no parameter, because its parameters are the ones the engine's answers name, with the
    /// bounds the analysis allows. Absent when the form names no tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter_binding: Option<ParameterBinding>,
    /// What the tool offers when the loop opens (C12), before any answer of `controlStudio/parameters`: Inspect, which reads only
    /// (`messages::first_tool_offer`). Absent when the form names no tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter_offer: Option<ParameterToolOffer>,
}

/// The project test's form of a project-scripted loop, or `None` for a loop Control Studio runs itself.
#[must_use]
pub fn project_test_form(loop_definition: &LoopDefinition) -> Option<ProjectTestForm> {
    let experiment = project_scripted(loop_definition)?;
    let tool = parameter_tool(loop_definition);
    Some(ProjectTestForm {
        signals: signals(loop_definition),
        duration_seconds: experiment
            .duration_seconds
            .filter(|seconds| seconds.is_finite())
            .unwrap_or(DEFAULT_PROJECT_TEST_SECONDS),
        baseline_seconds: experiment
            .baseline_seconds
            .filter(|seconds| seconds.is_finite() && *seconds > 0.0),
        parameter_tool: tool,
        parameter_binding: tool_binding(loop_definition, tool),
        parameter_offer: tool.map(|_| first_tool_offer()),
    })
}

/// The signals the test offers: the setpoint, the process value and the applied output (selected), the requested output of an actuator
/// (not selected), then the loop's additional signals.
fn signals(loop_definition: &LoopDefinition) -> Vec<ProjectTestSignal> {
    let loop_signals = &loop_definition.signals;
    let manipulated = &loop_signals.manipulated_variable;
    let mut offered = Vec::new();
    if let Some(setpoint) = &loop_signals.setpoint {
        offered.push(offer_signal(
            "sp",
            format!("{} · Setpoint", setpoint.symbol),
            Some(setpoint.unit.clone()),
            true,
        ));
    }
    let process = &loop_signals.process_variable;
    offered.push(offer_signal(
        "pv",
        format!("{} · Process value", process.symbol),
        Some(process.unit.clone()),
        true,
    ));
    let applied = loop_signals.actuator.as_ref().unwrap_or(manipulated);
    offered.push(offer_signal(
        "mv",
        format!("{} · Applied output", applied.symbol),
        Some(manipulated.unit.clone()),
        true,
    ));
    if loop_signals.actuator.is_some() {
        offered.push(offer_signal(
            "requestedMv",
            format!("{} · Requested output", manipulated.symbol),
            Some(manipulated.unit.clone()),
            false,
        ));
    }
    for auxiliary in loop_signals.additional.iter().flatten() {
        offered.push(ProjectTestSignal {
            id: auxiliary.id.clone(),
            label: format!("{} · {}", auxiliary.symbol, role_word(auxiliary.role)),
            unit: (!auxiliary.unit.is_empty()).then(|| auxiliary.unit.clone()),
            selected: auxiliary.role == AuxiliaryRole::Setpoint && loop_signals.setpoint.is_none(),
        });
    }
    offered
}

fn offer_signal(
    id: &str,
    label: String,
    unit: Option<String>,
    selected: bool,
) -> ProjectTestSignal {
    ProjectTestSignal {
        id: id.to_owned(),
        label,
        unit,
        selected,
    }
}

/// The word of a signal's role as the loop file writes it (`SP`, `PV`, `MV`, `actuator`, `disturbance`, ...).
fn role_word(role: AuxiliaryRole) -> String {
    serde_json::to_value(role)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// The parameter tool of the loop (§0.1a `controlStudio/parameters`, B6.1, C12): the parameters of its declared analysis when it maps
/// any, else the experiment's declared settings when the loop declares `parameterPurpose: "experiment"`. A loop with neither, and a
/// loop without a parameter binding, has none: the page shows no parameter controls for it, and the engine refuses its parameter
/// requests. The form and `controlStudio/parameters` both decide with this function.
#[must_use]
pub fn parameter_tool(loop_definition: &LoopDefinition) -> Option<ParameterTool> {
    loop_definition.parameter_binding.as_ref()?;
    if loop_definition.analysis_parameter_symbols.is_some() {
        return Some(ParameterTool::Analysis);
    }
    (loop_definition.parameter_purpose == Some(ParameterPurpose::Experiment))
        .then_some(ParameterTool::Experiment)
}

/// The parameters the tool starts from (B6.1, C12): the loop's whole binding for the experiment's settings, and the binding with no
/// parameter for a declared analysis, whose parameters the engine's answers name.
fn tool_binding(
    loop_definition: &LoopDefinition,
    tool: Option<ParameterTool>,
) -> Option<ParameterBinding> {
    let binding = loop_definition.parameter_binding.clone()?;
    match tool? {
        ParameterTool::Experiment => Some(binding),
        ParameterTool::Analysis => Some(ParameterBinding {
            parameters: Vec::new(),
            ..binding
        }),
    }
}

/// The sentence a finished project test carries when it releases the experiment settings (C12).
pub const SETTINGS_RELEASED: &str =
    "The previous project test restored its declared values. Edit and review them before the next trial.";

/// The sentence that says a finished project test releases the experiment settings, or `None` when it does not (C12). The settings'
/// ownership is released only after the terminal restoration: a campaign whose restoration the runtime confirmed, or that wrote
/// nothing to restore, releases them, and a campaign that kept no restoration releases nothing. The page returns the parameter tool to
/// its inspected state when the answer carries the sentence.
#[must_use]
pub fn settings_released(restoration: Option<CampaignRestoration>) -> Option<String> {
    restoration.map(|_| SETTINGS_RELEASED.to_owned())
}

/// The sentence of a finished project test (C12).
pub const PROJECT_TEST_COMPLETED: &str = "Project test completed.";

/// The sentence a finished project test adds when the runtime confirmed the restoration of its outputs (C12, A7).
pub const OUTPUTS_RESTORED: &str = "Outputs restored.";

/// The sentence that says how a finished project test ended (C12): it completed, and its outputs are restored when the runtime confirmed
/// the restoration of what it wrote. A test that wrote nothing to restore, or kept no restoration, says only that it completed. The page
/// shows the sentence as the test's status.
#[must_use]
pub fn project_test_outcome(restoration: Option<CampaignRestoration>) -> String {
    match restoration {
        Some(CampaignRestoration::Restored) => {
            format!("{PROJECT_TEST_COMPLETED} {OUTPUTS_RESTORED}")
        }
        Some(CampaignRestoration::NotRequired) | None => PROJECT_TEST_COMPLETED.to_owned(),
    }
}

#[cfg(test)]
#[path = "project_test_tests.rs"]
mod tests;
