//! The controllability and observability structure (`stateStructure`) and the relative gain array (`relativeGain`) of a state-space
//! model (B9.9; N4.5; draft §2.3). Both answer at once. The structure's rank rules and matrices are the product's (`model/conversion.rs`);
//! the array is `Λ = K ∘ (K⁻¹)ᵀ` of the steady-state gain `K` (N4.5), refused as unavailable when `K` is singular.

use trust_control_math::linalg::NumericalTolerance;
use trust_control_math::multichannel::{relative_gain_array, steady_state_gain};

use crate::analysis::tool::{channel_ref, not_applicable};
use crate::failure::Failure;
use crate::messages::{
    AnalysisTool, NamedMatrix, RelativeGainAnswer, RelativeGainStatus, StateStructureAnswer,
};
use crate::model::{calculate_general_state_structure, GeneralModel, GeneralStateStructure};

/// The tolerance of the steady-state gain and its inverse (the page's tolerance, N4.5).
const TOLERANCE: NumericalTolerance = NumericalTolerance {
    absolute: 1e-12,
    relative: 1e-10,
    condition_limit: 1e12,
};

/// The reason of an unavailable relative gain array (the page's words, N4.5).
pub const RGA_SINGULAR: &str = "Relative gain requires a qualified nonsingular DC gain matrix.";
/// The caption of the relative gain array in the result (the page's words).
pub const RGA_SUMMARY: &str = "Relative gain array (RGA).";

/// The answer of `stateStructure` (B9.9): the structure with its ranks and matrices, the order of the channels and the scope.
///
/// # Errors
/// Returns the not-applicable failure for a transfer function, which has no state structure.
pub fn state_structure_answer(model: &GeneralModel) -> Result<StateStructureAnswer, Failure> {
    let (system, states) = match model {
        GeneralModel::TransferFunction { .. } => {
            return Err(not_applicable(AnalysisTool::StateStructure));
        }
        GeneralModel::ContinuousStateSpace { system, states, .. }
        | GeneralModel::DiscreteStateSpace { system, states, .. } => (system, states),
    };
    let identity = model.identity();
    let order = format!(
        "State order: {}. Input order: {}. Output order: {}.",
        list_of(states.iter().map(|channel| (&channel.id, &channel.unit))),
        list_of(
            identity
                .inputs
                .iter()
                .map(|channel| (&channel.id, &channel.unit))
        ),
        list_of(
            identity
                .outputs
                .iter()
                .map(|channel| (&channel.id, &channel.unit))
        ),
    );
    let structure = calculate_general_state_structure(system);
    let (summary, details, matrices) = match &structure {
        GeneralStateStructure::Unavailable { reason } => (
            vec!["Structural analysis unavailable.".to_owned()],
            vec![order.clone(), reason.clone()],
            Vec::new(),
        ),
        GeneralStateStructure::Qualified {
            controllability,
            observability,
            scope,
        } => {
            let count = system.states();
            let matrices = vec![
                named("A", &system.a),
                named("B", &system.b),
                named("C", &system.c),
                named("D", &system.d),
                named("Controllability [B, AB, …]", &controllability.matrix),
                named("Observability [C; CA; …]", &observability.matrix),
            ];
            (
                vec![format!(
                    "Controllability rank {} of {count}; observability rank {} of {count}.",
                    controllability.rank, observability.rank
                )],
                vec![order.clone(), scope.clone()],
                matrices,
            )
        }
    };
    Ok(StateStructureAnswer {
        structure,
        order,
        matrices,
        summary,
        details,
    })
}

/// The answer of `relativeGain` (B9.9, N4.5): the array of a square state-space model with more than one channel, with its channel
/// names and units; unavailable with its reason when the steady-state gain is singular.
///
/// # Errors
/// Returns the not-applicable failure for a model that is not a state-space model with equal and more than one inputs and outputs.
pub fn relative_gain_answer(model: &GeneralModel) -> Result<RelativeGainAnswer, Failure> {
    let (system, discrete) = match model {
        GeneralModel::TransferFunction { .. } => {
            return Err(not_applicable(AnalysisTool::RelativeGain));
        }
        GeneralModel::ContinuousStateSpace { system, .. } => (system, false),
        GeneralModel::DiscreteStateSpace { system, .. } => (system, true),
    };
    let identity = model.identity();
    let channels = identity.inputs.len();
    if channels < 2 || channels != identity.outputs.len() {
        return Err(not_applicable(AnalysisTool::RelativeGain));
    }
    let gain = steady_state_gain(system, discrete);
    let array = gain
        .as_ref()
        .and_then(|gain| relative_gain_array(gain, &TOLERANCE).ok());
    let (status, summary) = match array {
        Some(matrix) => (
            RelativeGainStatus::Qualified { matrix },
            vec![RGA_SUMMARY.to_owned()],
        ),
        None => (
            RelativeGainStatus::Unavailable {
                reason: RGA_SINGULAR.to_owned(),
            },
            vec![RGA_SINGULAR.to_owned()],
        ),
    };
    Ok(RelativeGainAnswer {
        input_channels: identity
            .inputs
            .iter()
            .map(|channel| channel_ref(&channel.id, &channel.unit))
            .collect(),
        output_channels: identity
            .outputs
            .iter()
            .map(|channel| channel_ref(&channel.id, &channel.unit))
            .collect(),
        status,
        summary,
        details: Vec::new(),
    })
}

/// A matrix with its name, its rows copied.
fn named(name: &str, rows: &[Vec<f64>]) -> NamedMatrix {
    NamedMatrix {
        name: name.to_owned(),
        rows: rows.to_vec(),
    }
}

/// `id (unit)` for each channel, joined by commas, as the page's state order writes them.
fn list_of<'a>(channels: impl Iterator<Item = (&'a String, &'a String)>) -> String {
    channels
        .map(|(id, unit)| format!("{id} ({unit})"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
#[path = "model_views_tests.rs"]
mod tests;
