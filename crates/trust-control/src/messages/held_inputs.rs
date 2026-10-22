//! What the study's recorded inputs mean for the page's editors (specification 35, §0.1a `open`, A8.6, B3.2 and B4.2). The open
//! answer carries the recorded inputs as the study stores them (`inputs`), and the page's editors take the study's choices once, when
//! the loop opens. The page decodes no record: the engine names the entry of the Method list that holds the study's method, and the
//! controller family and the pinned settings that apply to the tuning method the study holds.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use super::TuneMethod;
use crate::identification::method_offers::method_word;
use crate::study::inputs::InputsSection;

/// The study's recorded inputs as the page's editors show them (A8.6): resolved by the engine, each absent when the study records no
/// such choice.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HeldInputs {
    /// The entry of the Identify page's Method list that holds the method the study records (B3.2), by the word of its offer
    /// (`identifyMethods`). The method the loop declares has no entry of its own and is shown as Automatic. Absent when the study
    /// records no method: the list then stands on its first entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identify_method: Option<String>,
    /// The controller family the study holds (B4.4): the family of the state law the engineer chose (`state-feedback`, `LQR` or
    /// `LQG`), else the structure the engineer fixed (`P`, `PI`, `PD` or `PID`). Absent leaves the choice to Automatic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    /// The pinned settings that apply to the tuning method the study holds (B4.2, C4), by symbol: the state design's pins for the
    /// state-space method, the structure's pins for any other. Absent when the study pins none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pins: Option<BTreeMap<String, f64>>,
}

impl HeldInputs {
    /// What the recorded `inputs` mean for the editors.
    #[must_use]
    pub fn of(inputs: &InputsSection) -> Self {
        let design = inputs.state_design.as_ref();
        let state_space = inputs.method == Some(TuneMethod::StateSpace);
        Self {
            identify_method: inputs
                .acquisition
                .as_ref()
                .map(|record| method_word(record.method).to_owned()),
            family: design
                .and_then(|design| design.family)
                .and_then(|family| word_of(&family))
                .or_else(|| inputs.structure.and_then(|structure| word_of(&structure))),
            pins: if state_space {
                design.and_then(|design| design.pins.clone())
            } else {
                inputs.pins.as_ref().map(|pins| pins.values.clone())
            },
        }
    }
}

/// The word a value serializes as, when it serializes as a word.
fn word_of(value: &impl Serialize) -> Option<String> {
    match serde_json::to_value(value) {
        Ok(Value::String(word)) => Some(word),
        _ => None,
    }
}

#[cfg(test)]
#[path = "held_inputs_tests.rs"]
mod tests;
