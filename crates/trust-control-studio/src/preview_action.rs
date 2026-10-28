//! The preview method `controlStudio/preview` (specification 35, §0.1a and B4.5; the Tune page's tuning controls, where the
//! predicted response follows each edit).
//!
//! `preview` answers at once with the result that `controlStudio/tune` with the manual method would finish with: the controller
//! settings the engineer entered, predicted on the study's current model, with their margins and outcome. It starts no operation,
//! claims no loop and keeps no candidate. With a connection it reads the running controller's settings for the comparison, and
//! writes nothing (B4.5). Every rule is trust-control's (`study::tune::preview`); this module reads the loop, its study and the
//! running settings, and forwards the request.

use std::path::Path;

use serde_json::Value;
use trust_control::failure::Failure;
use trust_control::messages::{PreviewRequest, TuneMethod, TuneRequest};
use trust_control::study::current_model::current_model;
use trust_control::study::inputs::InputsSection;
use trust_control::study::tune::structure_choice::structure_of_request;
use trust_control::study::tune::{preview, preview_settings, TuneInputs, TuneSection};
use trust_control::tuning::settings_pins::PinnedSettings;

use crate::commands::to_value;
use crate::tune_action::running_settings;
use crate::{loops, storage};

/// Answers `controlStudio/preview` (B4.5): the result that Tune with the manual method would finish with for the settings the
/// request enters, answered at once. The loop and its study are read as `controlStudio/open` and Tune read them, so the study's
/// Identify result gives the model, its Tune section supplies the requirements it holds, and its inputs give the structure and the
/// pins the settings are predicted with (B4.2). With the request's connection, the running controller's settings are read for the
/// comparison; a read that fails leaves it out. Nothing is kept: no operation starts, no loop is claimed and no candidate is stored.
///
/// # Errors
/// The loop cannot be opened, or its study or one of its sections cannot be read; the settings are missing or are not numbers; or
/// trust-control refuses the preview, for want of a current model or for the entered settings, with the next action it names.
pub fn answer(request: &PreviewRequest) -> Result<Value, Failure> {
    let project_root = Path::new(&request.project_root);
    let loop_definition = loops::open(project_root, &request.runtime_id, &request.loop_id)?;
    let study = storage::open_study(project_root, &loop_definition.id, &loop_definition.revision)?;
    let model = current_model(&study)?;
    let previous = study.section::<TuneSection>()?;
    let recorded = study.section::<InputsSection>()?.unwrap_or_default();
    let settings = preview_settings(request.settings.as_ref())?;
    // The request that the preview evaluates is the manual method with these settings, the structure and the pins the study holds,
    // and no journal (B4.5). The pins keep the family the study recorded them with. The structure is the one the study's inputs record,
    // else the family the Controller selector shows (`structure_of_request`), as the manual Tune takes it.
    let pins = recorded.pins.clone();
    let structure = structure_of_request(
        None,
        Some(TuneMethod::Manual),
        recorded.structure,
        previous.as_ref().and_then(TuneSection::candidate_structure),
        pins.as_ref(),
    );
    let tune_request = TuneRequest {
        project_root: request.project_root.clone(),
        runtime_id: request.runtime_id.clone(),
        loop_id: request.loop_id.clone(),
        storage_folder: String::new(),
        method: Some(TuneMethod::Manual),
        choice: None,
        structure,
        pins: pins.as_ref().map(PinnedSettings::from),
        settings: Some(settings),
        connection: request.connection.clone(),
    };
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: model.as_ref(),
        previous: previous.as_ref(),
        relay: None,
        request: &tune_request,
        pins: pins.as_ref(),
    };
    let running = running_settings(request.connection.as_ref(), &loop_definition);
    to_value(&preview(&inputs, running.as_ref())?)
}

#[cfg(test)]
#[path = "preview_action_tests.rs"]
mod tests;
