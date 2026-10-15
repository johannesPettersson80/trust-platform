//! The stored state-design preferences (specification 35, B4.2 and C4; `stateDesignPreferences.ts`): the design choices an
//! engineer keeps for a state-feedback law (desired poles, weights, covariances and a finite horizon), the draft text of
//! the form that is read into them, and the structural rules a stored preference keeps. A draft is editing state; only
//! [`parse_state_design_draft`] turns it into a preference, and a solver reads nothing else.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use trust_control_math::linalg::DenseMatrix;
use trust_control_math::lqr::FiniteHorizon;

use super::{
    is_symmetric_square, texts, StateControllerFamily, MAXIMUM_HORIZON_STEPS, MAXIMUM_STATES,
};
use crate::json;

/// The most UTF-16 code units one draft field may hold (`decodeStateDesignDraft`).
pub const MAXIMUM_DRAFT_UNITS: usize = 32_768;

/// The stored design choices. A field that is absent keeps the value the law declares (C4).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StateDesignPreferences {
    /// The finite horizon of an LQR or LQG design: its steps and its terminal weight (N5.14).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finite_horizon: Option<FiniteHorizon>,
    /// The desired discrete poles of pole placement, one per state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desired_poles: Option<Vec<f64>>,
    /// The state weight `Q`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_weights: Option<DenseMatrix>,
    /// The input weight `R`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_weights: Option<DenseMatrix>,
    /// The process noise covariance of an LQG estimator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_noise: Option<DenseMatrix>,
    /// The measurement noise covariance of an LQG estimator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measurement_noise: Option<DenseMatrix>,
}

/// The sizes of the model that a preference is checked against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelDimensions {
    /// The number of states.
    pub states: usize,
    /// The number of inputs.
    pub inputs: usize,
    /// The number of outputs.
    pub outputs: usize,
}

/// The fields of the design form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum DraftField {
    /// The desired poles.
    DesiredPoles,
    /// The state weight `Q`.
    StateWeights,
    /// The input weight `R`.
    InputWeights,
    /// The process noise covariance.
    ProcessNoise,
    /// The measurement noise covariance.
    MeasurementNoise,
    /// The solver: finite or infinite horizon.
    SolverMode,
    /// The horizon steps of a finite horizon.
    HorizonSteps,
    /// The terminal weights of a finite horizon.
    TerminalWeights,
}

/// The text of the form's fields: the saved editing state, which only [`parse_state_design_draft`] reads.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StateDesignDraft {
    /// The desired poles, as JSON.
    pub desired_poles: Option<String>,
    /// The state weight, as JSON.
    pub state_weights: Option<String>,
    /// The input weight, as JSON.
    pub input_weights: Option<String>,
    /// The process noise covariance, as JSON.
    pub process_noise: Option<String>,
    /// The measurement noise covariance, as JSON.
    pub measurement_noise: Option<String>,
    /// The solver choice: `finite` or `infinite`.
    pub solver_mode: Option<String>,
    /// The horizon steps, as a number.
    pub horizon_steps: Option<String>,
    /// The terminal weights, as JSON.
    pub terminal_weights: Option<String>,
}

impl StateDesignDraft {
    /// The text of `field`, when the draft holds one.
    #[must_use]
    pub fn text(&self, field: DraftField) -> Option<&str> {
        let text = match field {
            DraftField::DesiredPoles => &self.desired_poles,
            DraftField::StateWeights => &self.state_weights,
            DraftField::InputWeights => &self.input_weights,
            DraftField::ProcessNoise => &self.process_noise,
            DraftField::MeasurementNoise => &self.measurement_noise,
            DraftField::SolverMode => &self.solver_mode,
            DraftField::HorizonSteps => &self.horizon_steps,
            DraftField::TerminalWeights => &self.terminal_weights,
        };
        text.as_deref()
    }
}

/// A draft that cannot be read: the field the sentence concerns, when one does, and the sentence.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct DraftError {
    /// The field the sentence concerns; `None` when the whole design is refused.
    pub field: Option<DraftField>,
    /// The sentence the engineer reads.
    pub message: String,
}

impl DraftError {
    fn new(field: DraftField, message: &str) -> Self {
        Self {
            field: Some(field),
            message: message.to_string(),
        }
    }
}

/// The stored preferences, when they keep the rules of the form (`decodeStateDesignPreferences`): a finite horizon of 1 to
/// 512 steps with a symmetric terminal weight that is not negative on its diagonal; desired poles of one to 32 values;
/// symmetric weights and covariances whose diagonal is not negative, or positive for `R` and the measurement noise. With
/// `dimensions`, each matrix and the poles must have the size of its model coordinate.
#[must_use]
pub fn decode_state_design_preferences(
    value: &Value,
    dimensions: Option<ModelDimensions>,
) -> Option<StateDesignPreferences> {
    let preferences: StateDesignPreferences = json::decode_value(value.clone()).ok()?;
    preferences_fit(&preferences, dimensions).then_some(preferences)
}

fn preferences_fit(
    preferences: &StateDesignPreferences,
    dimensions: Option<ModelDimensions>,
) -> bool {
    let horizon_fits = preferences.finite_horizon.as_ref().is_none_or(|horizon| {
        (1..=MAXIMUM_HORIZON_STEPS).contains(&horizon.steps)
            && is_square_symmetric(&horizon.terminal_weights)
            && diagonal_fits(&horizon.terminal_weights, false)
            && dimensions.is_none_or(|sizes| horizon.terminal_weights.len() == sizes.states)
    });
    let poles_fit = preferences.desired_poles.as_ref().is_none_or(|poles| {
        (1..=MAXIMUM_STATES).contains(&poles.len())
            && dimensions.is_none_or(|sizes| poles.len() == sizes.states)
    });
    let weights_fit = weight_fits(
        preferences.state_weights.as_ref(),
        dimensions.map(|sizes| sizes.states),
        false,
    ) && weight_fits(
        preferences.input_weights.as_ref(),
        dimensions.map(|sizes| sizes.inputs),
        true,
    ) && weight_fits(
        preferences.process_noise.as_ref(),
        dimensions.map(|sizes| sizes.states),
        false,
    ) && weight_fits(
        preferences.measurement_noise.as_ref(),
        dimensions.map(|sizes| sizes.outputs),
        true,
    );
    horizon_fits && poles_fit && weights_fit
}

/// A weight or covariance: square and symmetric, of the model's size when it is known, with its diagonal checked by
/// `strict`.
fn weight_fits(matrix: Option<&DenseMatrix>, size: Option<usize>, strict: bool) -> bool {
    matrix.is_none_or(|matrix| {
        is_square_symmetric(matrix)
            && size.is_none_or(|size| matrix.len() == size)
            && diagonal_fits(matrix, strict)
    })
}

/// Square with one to 32 rows, and symmetric to the tolerance of the design; the shape is checked before any entry is read.
fn is_square_symmetric(matrix: &DenseMatrix) -> bool {
    (1..=MAXIMUM_STATES).contains(&matrix.len()) && is_symmetric_square(matrix, matrix.len())
}

/// Whether every diagonal entry is positive (`strict`) or not negative. The matrix must be square.
fn diagonal_fits(matrix: &DenseMatrix, strict: bool) -> bool {
    matrix.iter().enumerate().all(|(index, row)| {
        if strict {
            row[index] > 0.0
        } else {
            row[index] >= 0.0
        }
    })
}

/// The draft of a form, when it is an object whose every field is text of at most [`MAXIMUM_DRAFT_UNITS`] UTF-16 code
/// units and whose field names are the form's (`decodeStateDesignDraft`).
#[must_use]
pub fn decode_state_design_draft(value: &Value) -> Option<StateDesignDraft> {
    let fields = value.as_object()?;
    let texts_fit = fields.values().all(|text| {
        text.as_str()
            .is_some_and(|text| text.encode_utf16().count() <= MAXIMUM_DRAFT_UNITS)
    });
    if !texts_fit {
        return None;
    }
    json::decode_value(value.clone()).ok()
}

/// The fields a design of `family` reads from its draft, in the order the form lists them (`STATE_DESIGN_FIELDS`).
fn applicable_fields(family: StateControllerFamily) -> &'static [DraftField] {
    match family {
        StateControllerFamily::StateFeedback => &[DraftField::DesiredPoles],
        StateControllerFamily::Lqr => &[DraftField::StateWeights, DraftField::InputWeights],
        StateControllerFamily::Lqg => &[
            DraftField::StateWeights,
            DraftField::InputWeights,
            DraftField::ProcessNoise,
            DraftField::MeasurementNoise,
        ],
    }
}

/// One field of the design form that a family shows, with the label the page shows for it (the page's words, see `draft_label`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FormField {
    /// The field.
    pub field: DraftField,
    /// The label the form shows.
    pub label: String,
}

/// The fields the design form shows for `family` under its solver policy (D-B12), in the order the page lists them: for LQR and
/// LQG the solver policy first, then the weights and, for LQG, the covariances; the horizon steps and the terminal weights only
/// when the policy is the finite horizon. State feedback shows its desired poles alone.
#[must_use]
pub fn form_fields(family: StateControllerFamily, finite_horizon: bool) -> Vec<FormField> {
    let mut fields = Vec::new();
    if family != StateControllerFamily::StateFeedback {
        fields.push(DraftField::SolverMode);
    }
    fields.extend_from_slice(applicable_fields(family));
    if family != StateControllerFamily::StateFeedback && finite_horizon {
        fields.extend([DraftField::HorizonSteps, DraftField::TerminalWeights]);
    }
    fields
        .into_iter()
        .map(|field| FormField {
            field,
            label: texts::draft_label(field).to_string(),
        })
        .collect()
}

/// The key that a stored preference uses for a matrix or pole field.
#[must_use]
pub fn field_key(field: DraftField) -> &'static str {
    match field {
        DraftField::DesiredPoles => "desiredPoles",
        DraftField::StateWeights => "stateWeights",
        DraftField::InputWeights => "inputWeights",
        DraftField::ProcessNoise => "processNoise",
        DraftField::MeasurementNoise => "measurementNoise",
        DraftField::SolverMode => "solverMode",
        DraftField::HorizonSteps => "horizonSteps",
        DraftField::TerminalWeights => "terminalWeights",
    }
}

/// The preferences that a draft states for `family` (`parseStateDesignDraft`). Each field of the family is read as JSON,
/// the horizon is read when the solver is finite (LQR and LQG), and the result must keep the structural rules.
///
/// # Errors
/// Returns a [`DraftError`] that names the first field that cannot be read or that breaks its rule. The whole design is
/// named, without a field, when the read preferences break a structural rule.
pub fn parse_state_design_draft(
    draft: &StateDesignDraft,
    family: StateControllerFamily,
) -> Result<StateDesignPreferences, DraftError> {
    let mut result = Map::new();
    for &field in applicable_fields(family) {
        if let Some(text) = draft.text(field).filter(|text| !text.trim().is_empty()) {
            result.insert(field_key(field).to_string(), parse_json(field, text)?);
        }
    }
    let solver = draft.solver_mode.as_deref();
    if family != StateControllerFamily::StateFeedback && solver == Some("finite") {
        let steps = horizon_steps(draft)?;
        let terminal_text = draft
            .terminal_weights
            .as_deref()
            .filter(|text| !text.trim().is_empty())
            .ok_or_else(|| {
                DraftError::new(DraftField::TerminalWeights, texts::TERMINAL_WEIGHTS_MISSING)
            })?;
        let terminal = parse_json(DraftField::TerminalWeights, terminal_text)?;
        result.insert(
            "finiteHorizon".to_string(),
            json!({ "steps": steps, "terminalWeights": terminal }),
        );
    } else if solver.is_some_and(|mode| !mode.is_empty() && mode != "finite" && mode != "infinite")
    {
        return Err(DraftError::new(
            DraftField::SolverMode,
            texts::SOLVER_MODE_CHOICE,
        ));
    }
    decode_state_design_preferences(&Value::Object(result), None).ok_or_else(|| DraftError {
        field: None,
        message: texts::PREFERENCES_INVALID.to_string(),
    })
}

/// The JSON value of a field's text.
fn parse_json(field: DraftField, text: &str) -> Result<Value, DraftError> {
    json::parse_text(text)
        .map_err(|_| DraftError::new(field, &texts::not_valid_json(texts::draft_label(field))))
}

/// The horizon steps of a draft: a whole number from 1 to 512 (the form's number, read as the form reads it).
fn horizon_steps(draft: &StateDesignDraft) -> Result<usize, DraftError> {
    let refused = || DraftError::new(DraftField::HorizonSteps, texts::HORIZON_STEPS_RANGE);
    let number = draft
        .horizon_steps
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .and_then(|text| text.parse::<f64>().ok())
        .ok_or_else(refused)?;
    whole_steps(number)
        .filter(|steps| (1..=MAXIMUM_HORIZON_STEPS).contains(steps))
        .ok_or_else(refused)
}

/// The whole number that `number` denotes, when it is integral, nonnegative and within the range of a `u32`.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_cmp
)] // Checked to be integral and below 2³², so the cast is exact.
fn whole_steps(number: f64) -> Option<usize> {
    (number.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(&number))
        .then_some(number as usize)
}

#[cfg(test)]
#[path = "preferences_tests.rs"]
mod tests;
