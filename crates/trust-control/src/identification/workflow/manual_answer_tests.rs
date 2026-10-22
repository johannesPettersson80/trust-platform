//! Tests of the model choices of the page's view of the manual identification (`model_choices_of`, `modelChoices`): a held step
//! enables only the models that allow it, without a held step every model is enabled, and every choice carries its words.

use serde_json::json;

use crate::identification::form_draft::{blank_form, IdentificationFormDraft};
use crate::identification::review::IdentificationModelSelection;
use crate::identification::workflow::fixtures::test_loop;
use crate::identification::workflow::IdentificationWorkflow;
use crate::messages::ManualModelChoice;

/// A choice of `model`, enabled or not, as the page's view of the choices lists it: the model and whether the page may enable it.
fn choice(
    model: IdentificationModelSelection,
    enabled: bool,
) -> (IdentificationModelSelection, bool) {
    (model, enabled)
}

/// The models of a list of choices with whether each is enabled.
fn enabled_models(choices: Vec<ManualModelChoice>) -> Vec<(IdentificationModelSelection, bool)> {
    choices
        .into_iter()
        .map(|choice| (choice.model, choice.enabled))
        .collect()
}

/// The form a page holds with the held-step measurement on and the automatic model.
fn held_form() -> IdentificationFormDraft {
    IdentificationFormDraft {
        held_step: true,
        ..blank_form()
    }
}

/// The model choices of the answer to `form`, from a workflow that holds no recording.
fn choices_of(form: Option<IdentificationFormDraft>) -> Vec<ManualModelChoice> {
    IdentificationWorkflow::new(0.1)
        .manual_answer(&test_loop(), form)
        .model_choices
}

#[test]
fn a_held_step_enables_the_first_order_with_dead_time_the_additional_lag_and_the_integrating_models(
) {
    // N2.3a: an integrating form may be selected for a held step; its fit is checked by the integrator rule.
    assert_eq!(
        enabled_models(choices_of(Some(held_form()))),
        vec![
            choice(IdentificationModelSelection::Automatic, false),
            choice(IdentificationModelSelection::Integrating, true),
            choice(IdentificationModelSelection::FirstOrder, false),
            choice(IdentificationModelSelection::Fopdt, true),
            choice(IdentificationModelSelection::SecondOrder, false),
            choice(IdentificationModelSelection::Sopdt, true),
        ]
    );
}

#[test]
fn without_a_held_step_every_model_is_enabled() {
    let all_enabled = vec![
        choice(IdentificationModelSelection::Automatic, true),
        choice(IdentificationModelSelection::Integrating, true),
        choice(IdentificationModelSelection::FirstOrder, true),
        choice(IdentificationModelSelection::Fopdt, true),
        choice(IdentificationModelSelection::SecondOrder, true),
        choice(IdentificationModelSelection::Sopdt, true),
    ];
    assert_eq!(enabled_models(choices_of(Some(blank_form()))), all_enabled);
    assert_eq!(enabled_models(choices_of(None)), all_enabled);
}

#[test]
fn a_model_choice_is_written_with_the_option_value_of_the_page_select() {
    let choices = serde_json::to_value(choices_of(Some(held_form()))).expect("the choices");
    assert_eq!(
        choices[2],
        json!({ "model": "first_order", "enabled": false, "label": "First order" })
    );
    assert_eq!(
        choices[5],
        json!({ "model": "sopdt", "enabled": true, "label": "Compare additional lag" })
    );
}

#[test]
fn every_offered_choice_carries_its_words() {
    // B3.3: the page lists the model options from the engine's choices, so each choice must say what it is called. The words are the ones
    // the page's select showed before the engine offered them; a choice without words would be listed blank, and the page keeps no
    // table of its own to fill it in.
    assert_eq!(
        serde_json::to_value(choices_of(None)).expect("the choices"),
        json!([
            { "model": "automatic", "enabled": true, "label": "Choose automatically" },
            { "model": "integrating", "enabled": true, "label": "Integrating" },
            { "model": "first_order", "enabled": true, "label": "First order" },
            { "model": "fopdt", "enabled": true, "label": "First order with dead time" },
            { "model": "second_order", "enabled": true, "label": "Second order" },
            { "model": "sopdt", "enabled": true, "label": "Compare additional lag" },
        ])
    );
}
