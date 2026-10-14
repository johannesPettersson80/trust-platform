//! The check of a saved Control Studio result or recording file, answered by `controlStudio/identify` with `importRecording`
//! (specification 35, §0.1a, B3.6 and B10.4). The rules are trust-control's (`identification::recording_import::check_import`);
//! this module answers them and holds none of its own. It starts nothing and keeps nothing: fitting an imported file and
//! keeping it in the study are not ported yet.

use serde_json::Value;

use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::recording_import::check_import;
use trust_control::identification::refusal::Refusal;
use trust_control::loop_definition::LoopDefinition;

use crate::commands::to_value;

/// Answers `importRecording`: the file checked against the loop. The answer is the checked identity of the saved result, and a
/// refusal is the sentence the engineer reads.
///
/// # Errors
/// The file is refused by the check (B10.4, B3.6, B3.9, A9), with the sentence the check gives.
pub fn check(loop_definition: &LoopDefinition, json: &str) -> Result<Value, Failure> {
    let answer = check_import(json, loop_definition).map_err(|refusal| refused(&refusal))?;
    to_value(&answer)
}

/// The failure of a refused file: its sentence, with the data as its cause and no next action, because the engineer chooses
/// another file.
fn refused(refusal: &Refusal) -> Failure {
    Failure::new(&refusal.0, Cause::ModelOrData, NextAction::None)
}
