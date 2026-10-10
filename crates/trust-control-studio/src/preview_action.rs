//! The preview method `controlStudio/preview` (specification 35, §0.1a and B4.5; the Tune page's tuning controls, where the
//! predicted response follows each edit).
//!
//! `preview` answers at once with the result that `controlStudio/tune` with the manual method would finish with: the controller
//! settings the engineer entered, predicted on the study's current model, with their margins and outcome. It starts no operation,
//! claims no loop and keeps no candidate. It names no connection either, because Tune never reaches the runtime (B4.1). Every rule
//! is trust-control's (`study::tune::preview`); this module reads the loop and its study, and forwards the request.

use std::path::Path;

use serde_json::Value;
use trust_control::failure::Failure;
use trust_control::messages::PreviewRequest;
use trust_control::study::identify::IdentifySection;
use trust_control::study::tune::{preview, TuneInputs, TuneSection};

use crate::commands::to_value;
use crate::{loops, storage};

/// Answers `controlStudio/preview` (B4.5): the result that Tune with the manual method would finish with for the settings the
/// request enters, answered at once. The loop and its study are read as `controlStudio/open` and Tune read them, so the study's
/// Identify result gives the model and its Tune section supplies the requirements it holds. Nothing is kept: no operation starts,
/// no loop is claimed and no candidate is stored.
///
/// # Errors
/// The loop cannot be opened, or its study or one of its sections cannot be read; or trust-control refuses the preview, for want
/// of a current model or for the entered settings, with the next action it names.
pub fn answer(request: &PreviewRequest) -> Result<Value, Failure> {
    let project_root = Path::new(&request.project_root);
    let loop_definition = loops::open(project_root, &request.runtime_id, &request.loop_id)?;
    let study = storage::open_study(project_root, &loop_definition.id, &loop_definition.revision)?;
    let identify = study.section::<IdentifySection>()?;
    let previous = study.section::<TuneSection>()?;
    // The request that the preview evaluates is the manual method with these settings; it names no journal (B4.5).
    let tune_request = request.tune_request();
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        identify: identify.as_ref(),
        previous: previous.as_ref(),
        relay: None,
        request: &tune_request,
    };
    to_value(&preview(&inputs)?)
}

#[cfg(test)]
#[path = "preview_action_tests.rs"]
mod tests;
