//! Control Studio's model rules: the general model and its decoder, the linear and structural
//! conversions, the open-loop time response, the frozen prediction, and the simulated plant that
//! Verify runs. The mathematics is in `trust-control-math`; this module adds the product rules, the
//! product limits, and Control Studio's sentences.

// Control Studio's names (GeneralModel, ModelRefusal, SelectedModelSimulation) and its doc comments
// that quote the TypeScript and JSON words follow the records they port.
#![allow(clippy::module_name_repetitions, clippy::doc_markdown)]

pub mod conversion;
pub mod general;
pub mod limits;
pub mod prediction;
pub mod refusal;
pub mod response;
pub mod simulation;

#[cfg(test)]
mod tests;

pub use conversion::{
    calculate_general_state_structure, is_general_state_structure, linear_transfer_model,
    GeneralStateStructure,
};
pub use general::{
    decode_general_model, GeneralModel, ModelChannel, ModelIdentity, ModelSource, ModelValidation,
    OperatingRange,
};
pub use prediction::{delayed_model_realization, frozen_model_prediction};
pub use refusal::ModelRefusal;
pub use response::{
    analyze_general_model_time, is_model_time_response, model_time_settings, ModelTimeResponse,
    ModelTimeSettings, ResponseConnection,
};
pub use simulation::{
    materialize_selected_model_simulation, valid_actuator, ActuatorCapabilities, ActuatorKind,
    SelectedModelSimulation, SelectedModelSimulationBinding, SimulationOverride,
};
