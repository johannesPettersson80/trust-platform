//! The model files an LQG estimator can implement (specification 35, C4 Setup; `estimatorModelFiles.ts`): the project's model files as the
//! engine read them, and the state-space models among them that fit the estimator. `discovery` re-exports the public items.

use schemars::JsonSchema;
use serde::Serialize;

use super::discovery::LoopFileRead;
use super::lqg_loop::MAXIMUM_ESTIMATOR_STATES;
use crate::json::parse_text;
use crate::model::{decode_general_model, GeneralModel};
use crate::tuning::state_controller::sampling::state_channels;

/// A model file of the project as the engine read it: its project-relative name and what was read, under the bounds of a loop file.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelFileText {
    /// The file's path relative to the project, with forward slashes: `lqg-model.json`, or `models/tanks.json`.
    pub file_name: String,
    /// What was read.
    pub read: LoopFileRead,
}

/// A model file an LQG estimator can implement, with its number of states (C4 Setup; the form's `estimatorModels`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EstimatorModelFile {
    /// The file's path relative to the project, with forward slashes.
    pub file: String,
    /// The model's number of states: at most three.
    pub states: usize,
}

/// The model files an LQG estimator can implement, in the order they are given (C4 Setup; `estimatorModelFiles.ts`): a state-space
/// model with one input, one output and at most three states. A file that is not read, or is not such a model, is left out.
#[must_use]
pub fn estimator_model_files(files: &[ModelFileText]) -> Vec<EstimatorModelFile> {
    files.iter().filter_map(estimator_model_file).collect()
}

/// The entry of one model file, or `None` when the file cannot be an estimator's model (`estimatorModelFiles.ts`).
fn estimator_model_file(file: &ModelFileText) -> Option<EstimatorModelFile> {
    let LoopFileRead::Text(text) = &file.read else {
        return None;
    };
    let value = parse_text(text).ok()?;
    let model = decode_general_model(&value)?;
    let identity = model.identity();
    let state_space = !matches!(model, GeneralModel::TransferFunction { .. });
    let states = state_channels(&model).len();
    let fits = state_space
        && identity.inputs.len() == 1
        && identity.outputs.len() == 1
        && states <= MAXIMUM_ESTIMATOR_STATES;
    fits.then(|| EstimatorModelFile {
        file: file.file_name.clone(),
        states,
    })
}

#[cfg(test)]
#[path = "estimator_model_files_tests.rs"]
mod tests;
