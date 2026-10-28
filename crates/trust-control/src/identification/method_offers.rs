//! The identification methods the Identify page offers for a loop (specification 35, B3.2, B3.3 and §0.1a `open`). The page builds
//! its Method list from these offers, in their order, with the words the engine gives each one. A method the loop cannot run is
//! offered with the sentence that says why (`method_applicability`). Each offer says whether the study records the method as an
//! acquisition setting (`records`), so the page sends a chosen method only when the engine records it. Nothing here starts an
//! operation.

use schemars::JsonSchema;
use serde::Serialize;

use crate::identification::acquisition_settings::AcquisitionMethod;
use crate::identification::method_applicability::{method_applicability, Applicability};
use crate::identification::policy::project_scripted;
use crate::identification::texts::method_refusal_text;
use crate::loop_definition::{AuxiliaryRole, LoopDefinition, LoopSignal};

/// One method of the Identify page's Method list (B3.2, B3.3): its word, which the page sends as the Method, its label, whether the
/// loop can run it, the sentence that says why it cannot, and whether the study records it as an acquisition setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IdentifyMethodOffer {
    /// The method's word: `automatic`, `mv_step`, `mv_pulse`, `setpoint_step`, `relay`, `prbs`, `multisine`, `recorded-fit`,
    /// `supplied-model` or `project`.
    pub value: String,
    /// The label the list shows.
    pub label: String,
    /// Whether the loop can run the method.
    pub available: bool,
    /// The sentence that says why the loop cannot run the method; absent when it can.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Whether the study records the method as an acquisition setting (A8.1, `controlStudio/inputs`): every live method does. The
    /// recorded-data fit, the supplied model and the project test start no campaign of the study's inputs, so the engine records no
    /// setting for them and the page keeps them as its view (`pageView`). Absent when the study records none.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub records: bool,
}

/// The methods the Identify page offers for the loop, in the page's order. The recorded-data fit and the supplied model start no
/// campaign, so every loop offers them. The project test is offered for a project-scripted loop only (B6.1, C12).
#[must_use]
pub fn identify_method_offers(loop_definition: &LoopDefinition) -> Vec<IdentifyMethodOffer> {
    let reference = reference_loop(loop_definition);
    let checked = reference.as_ref().unwrap_or(loop_definition);
    let mut offers = vec![
        method_offer(checked, "Automatic", AcquisitionMethod::Automatic),
        method_offer(checked, "Step response", AcquisitionMethod::MvStep),
        method_offer(checked, "Pulse response", AcquisitionMethod::MvPulse),
        method_offer(checked, "Setpoint step", AcquisitionMethod::SetpointStep),
        method_offer(checked, "Feedback relay", AcquisitionMethod::Relay),
        method_offer(
            checked,
            "Pseudo-random binary sequence (PRBS)",
            AcquisitionMethod::Prbs,
        ),
        method_offer(checked, "Multisine", AcquisitionMethod::Multisine),
        view_offer("recorded-fit", "Recorded-data fit"),
        view_offer("supplied-model", "Supplied plant model"),
    ];
    if project_scripted(loop_definition).is_some() {
        offers.push(view_offer("project", "Project experiment"));
    }
    offers
}

/// The word of the Method list's entry for an acquisition method (B3.2, B3.3): the word the offer carries and the page sends back as
/// the Method. The method the loop declares (`Configured`) has no entry of its own: a study that records it shows Automatic.
#[must_use]
pub fn method_word(method: AcquisitionMethod) -> &'static str {
    match method {
        AcquisitionMethod::Configured | AcquisitionMethod::Automatic => "automatic",
        AcquisitionMethod::MvStep => "mv_step",
        AcquisitionMethod::MvPulse => "mv_pulse",
        AcquisitionMethod::SetpointStep => "setpoint_step",
        AcquisitionMethod::Relay => "relay",
        AcquisitionMethod::Prbs => "prbs",
        AcquisitionMethod::Multisine => "multisine",
    }
}

/// The offer of one live method, which the study records as an acquisition setting: available when its rules pass, otherwise the
/// sentence of the first rule it fails.
fn method_offer(
    loop_definition: &LoopDefinition,
    label: &str,
    method: AcquisitionMethod,
) -> IdentifyMethodOffer {
    let value = method_word(method);
    match method_applicability(loop_definition, method) {
        Applicability::Available => offer(value, label, None, true),
        Applicability::Unavailable(refusal) => {
            offer(value, label, Some(method_refusal_text(refusal)), true)
        }
    }
}

/// The offer of a method that is the page's view: every loop can run it, and the study records no setting for it.
fn view_offer(value: &str, label: &str) -> IdentifyMethodOffer {
    offer(value, label, None, false)
}

fn offer(value: &str, label: &str, reason: Option<&str>, records: bool) -> IdentifyMethodOffer {
    IdentifyMethodOffer {
        value: value.to_owned(),
        label: label.to_owned(),
        available: reason.is_none(),
        reason: reason.map(str::to_owned),
        records,
    }
}

/// The loop with a setpoint taken from its first setpoint-role signal in the process unit, when the loop has no setpoint of its own.
/// That is the reference the page's Setpoint choice names (B3.2). The rules of the methods look only at whether a setpoint exists, so
/// the first candidate stands for the choice; Identify checks the chosen signal when it starts.
fn reference_loop(loop_definition: &LoopDefinition) -> Option<LoopDefinition> {
    if loop_definition.signals.setpoint.is_some() {
        return None;
    }
    let candidate = loop_definition
        .signals
        .additional
        .iter()
        .flatten()
        .find(|signal| {
            signal.role == AuxiliaryRole::Setpoint
                && signal.unit == loop_definition.signals.process_variable.unit
        })?;
    let mut with_reference = loop_definition.clone();
    with_reference.signals.setpoint = Some(LoopSignal {
        symbol: candidate.symbol.clone(),
        address: candidate.address.clone(),
        iec_type: candidate.iec_type.clone(),
        direction: candidate.direction,
        unit: candidate.unit.clone(),
        range: candidate.range,
    });
    Some(with_reference)
}

#[cfg(test)]
#[path = "method_offers_tests.rs"]
mod tests;
