//! The product wrappers of linearTransferModel.ts and generalStateStructure.ts. The library computes
//! the mathematics; these functions choose Control Studio's sentences and keep the model's identity.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use trust_control_math::linalg::NumericalTolerance;
use trust_control_math::model::{
    sampled_one_state_transfer, state_space_transfer_polynomials, state_structure, RankedMatrix,
    StateSpace, StateStructure,
};

use super::general::{decode_general_model, GeneralModel};
use super::limits::MAXIMUM_CONVERSION_ORDER;
use super::refusal::{conversion_refusal, structure_refusal, ModelRefusal};

/// The tolerance of the structure calculation (generalStateStructure.ts).
const STRUCTURE_TOLERANCE: NumericalTolerance = NumericalTolerance {
    absolute: 1e-12,
    relative: 1e-10,
    condition_limit: 1e12,
};

/// The scope of a qualified structure: what its ranks do and do not establish.
pub const STRUCTURE_SCOPE: &str = "Ranks describe the supplied local unconstrained model in declared state/input/output order. They do not establish bounded physical reachability, arbitrary steady targets or useful estimation in noise.";

/// The transfer function of a model, with the same identity. A transfer function is returned as it
/// is. A continuous state-space model uses its exact polynomials (order up to 16). A discrete model
/// with one state uses its held-input equivalent.
///
/// # Errors
/// Returns the sentence of the failed rule: the model is not valid, or the conversion is refused.
pub fn linear_transfer_model(value: &Value) -> Result<GeneralModel, ModelRefusal> {
    let model = decode_general_model(value).ok_or(ModelRefusal::SelectValidLinearModel)?;
    let converted = match &model {
        GeneralModel::TransferFunction { .. } => None,
        GeneralModel::ContinuousStateSpace {
            identity, system, ..
        } => Some((
            identity.clone(),
            state_space_transfer_polynomials(system, MAXIMUM_CONVERSION_ORDER)
                .map_err(conversion_refusal)?,
        )),
        GeneralModel::DiscreteStateSpace {
            identity,
            system,
            sample_time_seconds,
            ..
        } => Some((
            identity.clone(),
            sampled_one_state_transfer(system, *sample_time_seconds).map_err(conversion_refusal)?,
        )),
    };
    match converted {
        None => Ok(model),
        Some((identity, transfer)) => Ok(GeneralModel::TransferFunction { identity, transfer }),
    }
}

/// The structure of a state-space model, as the product's result: its ranks and matrices and the
/// scope sentence, or the reason it is not available.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum GeneralStateStructure {
    /// The structure is not available, for the reason given.
    Unavailable {
        /// The sentence that says why.
        reason: String,
    },
    /// The structure qualifies.
    Qualified {
        /// The controllability matrix and its rank.
        controllability: RankedMatrix,
        /// The observability matrix and its rank.
        observability: RankedMatrix,
        /// The scope sentence.
        scope: String,
    },
}

/// The controllability and observability structure of a state-space model, with the product's scope
/// sentence. A structure that fails the shape check is reported as inconsistent.
#[must_use]
pub fn calculate_general_state_structure(system: &StateSpace) -> GeneralStateStructure {
    let structure = match state_structure(system, &STRUCTURE_TOLERANCE) {
        Ok(structure) => structure,
        Err(error) => return unavailable(&structure_refusal(error)),
    };
    if !structure.fits(system.states(), system.inputs(), system.outputs()) {
        return unavailable(&ModelRefusal::StructureInconsistent);
    }
    GeneralStateStructure::Qualified {
        controllability: structure.controllability,
        observability: structure.observability,
        scope: STRUCTURE_SCOPE.to_owned(),
    }
}

/// Whether a stored structure has the shape of the model's structure: an unavailable structure with
/// its reason, or a qualified structure whose matrices and ranks fit the model's states, inputs and
/// outputs. A transfer function has no structure, so no stored structure fits it.
#[must_use]
pub fn is_general_state_structure(value: &Value, model: &GeneralModel) -> bool {
    let Ok(structure) = crate::json::decode_value::<GeneralStateStructure>(value.clone()) else {
        return false;
    };
    match (structure, model) {
        (GeneralStateStructure::Unavailable { .. }, _) => true,
        (
            GeneralStateStructure::Qualified {
                controllability,
                observability,
                ..
            },
            GeneralModel::ContinuousStateSpace { system, .. }
            | GeneralModel::DiscreteStateSpace { system, .. },
        ) => StateStructure {
            controllability,
            observability,
        }
        .fits(system.states(), system.inputs(), system.outputs()),
        (GeneralStateStructure::Qualified { .. }, GeneralModel::TransferFunction { .. }) => false,
    }
}

fn unavailable(refusal: &ModelRefusal) -> GeneralStateStructure {
    GeneralStateStructure::Unavailable {
        reason: refusal.to_string(),
    }
}
