//! The plant an isolated Validate run runs against (`configuredPlantVerification.ts`; specification 35, B5.3 and A9). The
//! current and the candidate settings run against the project's configured simulated plant when the target is simulated
//! and that plant drives the loop's signals. Otherwise they run against the qualified model. The evidence names the choice:
//! a configured plant by its id, or the model.
//!
//! Left out: the cascade branches of the `TypeScript` (a cascade is stage V7); `configuredPlantRequest`, which changes the
//! verification request (stage V3.2); and the `plant.read` request of `readConfiguredPlantBinding`, which is I/O. Its answer
//! is decoded by [`decode_configured_plant_binding`].

use serde::Deserialize;
use serde_json::Value;

use crate::loop_definition::LoopDefinition;
use crate::tuning::project_candidate::served_feedforward;
use crate::validation::scenario_contract::EngineerScenario;
use crate::validation::texts;

/// The ids of the model-variation cases that the campaign generates (`engineerVerificationCampaign`). A generated case keeps
/// its model, so it does not run on the configured plant.
const GENERATED_MODEL_CASES: [&str; 6] = [
    "load-rejection",
    "measurement-noise",
    "lower-gain",
    "upper-gain",
    "constraint-recovery",
    "state-recovery",
];

/// A configured simulated plant's bindings, as the project's saved configuration states them (`plant.read`, the
/// `[[plants]]` tables of the simulation file).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfiguredPlantBinding {
    /// The plant's identifier, which the loop names in its `plant_id`.
    pub id: String,
    /// The addresses the plant reads its inputs from: the loop's plant input must be one of them.
    pub command_sources: Vec<String>,
    /// The addresses the plant writes its measurements to: the loop's measured output must be one of them.
    pub feedback_targets: Vec<String>,
}

/// The plant an isolated run runs against (B5.3). Its evidence names the choice (A9): the configured plant by its id, or
/// the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationPlant {
    /// The project's configured simulated plant, by its id.
    Configured {
        /// The plant's identifier.
        plant_id: String,
    },
    /// The qualified model.
    Model,
}

/// Whether the configured plant drives the loop (`configuredPlantDrivesLoop`). The loop must name the plant; the plant's
/// feedback must reach the loop's measured output, and the loop's plant input must reach the plant's commands. Addresses
/// compare trimmed and in upper case, and an empty address is absent. Only direct bindings count: couplings between plants
/// are not followed.
#[must_use]
pub fn configured_plant_drives_loop(
    loop_definition: &LoopDefinition,
    plant: Option<&ConfiguredPlantBinding>,
) -> bool {
    let Some(plant) = plant else {
        return false;
    };
    let Some(plant_id) = loop_definition
        .plant_id
        .as_deref()
        .filter(|id| !id.is_empty())
    else {
        return false;
    };
    if plant.id != plant_id {
        return false;
    }
    let measured = loop_definition
        .signals
        .process_variable
        .address
        .as_deref()
        .and_then(normalized_address);
    let input = loop_definition
        .signals
        .manipulated_variable
        .address
        .as_deref()
        .and_then(normalized_address);
    let (Some(measured), Some(input)) = (measured, input) else {
        return false;
    };
    plant
        .feedback_targets
        .iter()
        .any(|target| normalized_address(target).as_deref() == Some(measured.as_str()))
        && plant
            .command_sources
            .iter()
            .any(|source| normalized_address(source).as_deref() == Some(input.as_str()))
}

/// The plant an isolated run of a target runs against (`verificationPlant`, B5.3): the configured plant when the target is
/// simulated and that plant drives the loop's signals, otherwise the model. A physical target never runs on the configured
/// plant.
#[must_use]
pub fn verification_plant(
    loop_definition: &LoopDefinition,
    simulated_target: bool,
    plant: Option<&ConfiguredPlantBinding>,
) -> VerificationPlant {
    match plant {
        // A measured-disturbance feedforward always validates on the model (C8 Validate).
        Some(plant)
            if simulated_target
                && served_feedforward(loop_definition).is_none()
                && configured_plant_drives_loop(loop_definition, Some(plant)) =>
        {
            VerificationPlant::Configured {
                plant_id: plant.id.clone(),
            }
        }
        _ => VerificationPlant::Model,
    }
}

/// The identifier of one entry of a `plant.read` answer, read on its own.
#[derive(Deserialize)]
struct PlantId {
    id: String,
}

/// One entry of a `plant.read` answer in full, under the runtime's own field names.
#[derive(Deserialize)]
struct PlantAnswer {
    id: String,
    command_sources: Vec<String>,
    feedback_targets: Vec<String>,
}

/// The `plants` list of a `plant.read` answer. Its entries stay as values until one of them is read.
#[derive(Deserialize)]
struct PlantsAnswer {
    plants: Vec<Value>,
}

/// The configured plant `plant_id` in a `plant.read` answer: the pure part of `readConfiguredPlantBinding`. The answer lists
/// its plants under `plants`. The first object whose id is the plant's is the plant, even when its lists are invalid: a later
/// entry is not read. An answer that is not an object, a list without the plant, or a plant whose `command_sources` or
/// `feedback_targets` are not lists of text gives no binding, so the model is the plant that runs. An empty plant id names no
/// plant, and nothing is asked for it.
#[must_use]
pub fn decode_configured_plant_binding(
    response: &Value,
    plant_id: &str,
) -> Option<ConfiguredPlantBinding> {
    // Both levels must be JSON objects (`isRecord` in the TypeScript), because serde would also read an array by position.
    if plant_id.is_empty() || !response.is_object() {
        return None;
    }
    let answer: PlantsAnswer = crate::json::decode_value(response.clone()).ok()?;
    let entry = answer
        .plants
        .iter()
        .find(|entry| entry.is_object() && has_plant_id(entry, plant_id))?;
    let plant: PlantAnswer = crate::json::decode_value(entry.clone()).ok()?;
    Some(ConfiguredPlantBinding {
        id: plant.id,
        command_sources: plant.command_sources,
        feedback_targets: plant.feedback_targets,
    })
}

/// Whether the entry's own `id` decodes to `plant_id`. An entry whose id does not decode does not match.
fn has_plant_id(entry: &Value, plant_id: &str) -> bool {
    crate::json::decode_value::<PlantId>(entry.clone()).is_ok_and(|found| found.id == plant_id)
}

/// Whether a scenario runs on the chosen plant (`usesConfiguredPlant`, B5.3). A model choice runs every scenario on the
/// model. A declared scenario runs on the plant unless it supplies its own model, operating outputs or effects, and a
/// declared effects list that is empty does not count. A scenario without a declaration runs on the plant unless its id is
/// one of the generated model-variation cases, which keep their model.
///
/// The cascade branch of the `TypeScript` (an outer cascade loop keeps its model, stage V7) is left out, so the loop is not
/// read here. The parameter keeps the signature the contract names.
#[must_use]
pub fn uses_configured_plant(
    _loop_definition: &LoopDefinition,
    scenario_id: &str,
    declared: Option<&EngineerScenario>,
    plant: &VerificationPlant,
) -> bool {
    if matches!(plant, VerificationPlant::Model) {
        return false;
    }
    match declared {
        Some(scenario) => {
            !(scenario.model.is_some()
                || scenario.operating_outputs.is_some()
                || scenario
                    .effects
                    .as_ref()
                    .is_some_and(|effects| !effects.is_empty()))
        }
        None => !GENERATED_MODEL_CASES.contains(&scenario_id),
    }
}

/// The evidence name of an isolated run's plant (A9): the configured plant's id, or the model with its revision.
#[must_use]
pub fn isolated_plant_name(plant: &VerificationPlant, model_revision: &str) -> String {
    match plant {
        VerificationPlant::Configured { plant_id } => texts::configured_plant_name(plant_id),
        VerificationPlant::Model => texts::model_name(model_revision),
    }
}

/// An address as the comparisons read it: trimmed and in upper case. An empty address is absent.
fn normalized_address(address: &str) -> Option<String> {
    let text = address.trim().to_uppercase();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
#[path = "configured_plant_verification_tests.rs"]
mod tests;
