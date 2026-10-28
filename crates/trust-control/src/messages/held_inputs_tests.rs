//! Tests of what the recorded inputs mean for the editors (B3.2, B4.2, B4.4, A8.6): the Method list's entry, the controller family and
//! the pinned settings that apply to the tuning method the study holds.

use std::collections::BTreeMap;

use serde_json::json;

use super::{HeldInputs, TuneMethod};
use crate::identification::acquisition_settings::AcquisitionSettings;
use crate::study::inputs::InputsSection;
use crate::study::tune::state::StateDesignInput;
use crate::tuning::settings_pins::ControllerSettingsPins;
use crate::tuning::state_controller::StateControllerFamily;
use crate::tuning::structure::Structure;

fn pins(values: &[(&str, f64)]) -> BTreeMap<String, f64> {
    values
        .iter()
        .map(|(symbol, value)| ((*symbol).to_owned(), *value))
        .collect()
}

/// The inputs of a state-feedback loop whose engineer once chose a PI structure: the state design holds the LQR family and the pin
/// `K1`, and the structure holds the PI pins `Kp`.
fn both_kinds(method: Option<TuneMethod>) -> InputsSection {
    InputsSection {
        method,
        structure: Some(Structure::PI),
        pins: Some(ControllerSettingsPins {
            loop_revision: "sha256:loop".to_owned(),
            family: Structure::PI,
            values: pins(&[("Kp", 3.0)]),
        }),
        state_design: Some(StateDesignInput {
            family: Some(StateControllerFamily::Lqr),
            draft: None,
            pins: Some(pins(&[("K1", 2.0)])),
        }),
        ..InputsSection::default()
    }
}

fn acquisition(method: &str) -> AcquisitionSettings {
    serde_json::from_value(json!({
        "method": method, "durationSeconds": 60, "amplitude": 0
    }))
    .expect("the record decodes")
}

/// A8.6, B4.2, C4: the editors take the pins of the method the study holds. The state-space method's pins are the state design's;
/// every other method's are the structure's. Catches a page that picks between the two records itself and shows the other method's
/// pins when the study holds both.
#[test]
fn inputs_answer_resolves_the_pins_and_family_of_the_held_method() {
    let state = HeldInputs::of(&both_kinds(Some(TuneMethod::StateSpace)));
    assert_eq!(state.pins, Some(pins(&[("K1", 2.0)])));
    let formula = HeldInputs::of(&both_kinds(Some(TuneMethod::Simc)));
    assert_eq!(formula.pins, Some(pins(&[("Kp", 3.0)])));
    let automatic = HeldInputs::of(&both_kinds(None));
    assert_eq!(
        automatic.pins,
        Some(pins(&[("Kp", 3.0)])),
        "Automatic pins the structure's settings"
    );
    // The family is the state law's when the engineer chose one, in the words the state families serialize as; the structure's
    // otherwise.
    assert_eq!(state.family.as_deref(), Some("LQR"));
    let structure_only = InputsSection {
        structure: Some(Structure::PID),
        ..InputsSection::default()
    };
    assert_eq!(
        HeldInputs::of(&structure_only).family.as_deref(),
        Some("PID")
    );
    let law = InputsSection {
        state_design: Some(StateDesignInput {
            family: Some(StateControllerFamily::StateFeedback),
            ..StateDesignInput::default()
        }),
        ..InputsSection::default()
    };
    assert_eq!(
        HeldInputs::of(&law).family.as_deref(),
        Some("state-feedback")
    );
}

/// A study that records nothing resolves to nothing: the answer omits every field, so the editors keep Automatic and no pins.
#[test]
fn a_study_that_records_no_choice_resolves_to_no_choice() {
    let held = HeldInputs::of(&InputsSection::default());
    assert_eq!(
        held,
        HeldInputs {
            identify_method: None,
            family: None,
            pins: None
        }
    );
    assert_eq!(
        serde_json::to_value(&held).expect("the answer serializes"),
        json!({})
    );
}

/// B3.2: the Method list shows the method the study records under the word of its offer, and the method the loop declares as
/// Automatic. Catches a page-side name table (`configured` shown as Automatic by the page) and a study that records a method but
/// shows none.
#[test]
fn the_held_method_is_the_word_of_its_offer() {
    let held = |method: &str| {
        HeldInputs::of(&InputsSection {
            acquisition: Some(acquisition(method)),
            ..InputsSection::default()
        })
        .identify_method
    };
    assert_eq!(held("relay").as_deref(), Some("relay"));
    assert_eq!(held("mv_step").as_deref(), Some("mv_step"));
    assert_eq!(held("setpoint_step").as_deref(), Some("setpoint_step"));
    assert_eq!(held("configured").as_deref(), Some("automatic"));
    assert_eq!(held("automatic").as_deref(), Some("automatic"));
}
