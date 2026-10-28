//! The structure comparison of a sensor-lag model with the first-order reference (N2.10, N3.3; `sensorLagComparison.ts`):
//! the adequacy of each structure's residuals, the recommendation between them, and the engineer's choice, which is
//! accepted only when it is the recommended structure. `sensor_lag_review` builds both candidates from the estimation
//! record and checks the chosen structure on the independent recording.
//!
//! The comparison keeps every result provisional. The additional lag's physical origin is unresolved, and a chosen
//! structure is never a design acceptance. The sensor-time grid and the residual requirements are optional held-step
//! settings (`sensorTimeConstant`, `modelAdequacy`), admitted by [`sensor_lag_settings_problem`].

#![allow(clippy::module_name_repetitions)] // The comparison's records keep the names of the `TypeScript` records.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::identify::step_response::StepResponseWindow;

use crate::identification::evidence::{ModelFitEvidence, ModelFitPoint};
use crate::identification::plant::IdentifiedModel;
use crate::identification::refusal::Refusal;
use crate::identification::sensor_lag::SensorLagStepAnalysis;
use crate::identification::sensor_lag_texts::{
    CALCULATE_FIRST, FIRST_ORDER_ADEQUATE, STRUCTURE_NOT_MET,
};
use crate::identification::step_response::{
    SearchGrid, SettingsProblem, StepResponseAnalysis, StepResponseSettings,
};

/// The tolerance of the sensor-time grid's admission, in grid steps (`EPSILON` of `stepResponseIdentification.ts`).
const EPSILON: f64 = 1e-9;

/// The most intervals a sensor-time grid may span (`MAX_GRID_POINTS` of `stepResponseIdentification.ts`).
const MAXIMUM_SENSOR_INTERVALS: f64 = 1_000_000.0;

/// The residual requirements a structure must meet to be adequate (`modelAdequacy`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelAdequacy {
    /// The largest absolute residual over the fitted samples, in output units.
    pub maximum_absolute_residual: f64,
    /// The largest absolute mean residual over the final window, in output units.
    pub maximum_final_error: f64,
}

/// The requirements used when the settings declare none: a residual of 0.2 and a final error of 0.1.
pub const DEFAULT_MODEL_ADEQUACY: ModelAdequacy = ModelAdequacy {
    maximum_absolute_residual: 0.2,
    maximum_final_error: 0.1,
};

/// How one structure fits the retained estimation record.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StructureAssessment {
    /// The largest absolute residual over the fitted samples.
    pub maximum_absolute_residual: f64,
    /// The absolute mean residual over the final window.
    pub final_error: f64,
    /// The root-mean-square residual over the fitted samples.
    pub root_mean_square_error: f64,
    /// Whether both residual requirements hold (with a tolerance of `10⁻⁹`).
    pub adequate: bool,
}

/// The structure the comparison recommends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StructureRecommendation {
    /// The first-order model is adequate, so the extra state is not needed.
    FirstOrder,
    /// The first-order model is not adequate, and the additional lag is adequate and improves the fit beyond the measurement bound.
    AdditionalLag,
    /// Neither structure meets the requirements.
    Neither,
}

/// A structure the engineer may choose (`structureChoice`: `first_order` or `additional_lag`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StructureChoice {
    /// The first-order model.
    FirstOrder,
    /// The first-order model with an additional lag.
    AdditionalLag,
}

/// One structure of a comparison (`candidateA`, `candidateB`): its residual assessment, the analysis it was fitted with,
/// its model, its fit evidence on the estimation record, and the model's equation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StructureCandidate<A> {
    /// The largest absolute residual over the fitted samples.
    pub maximum_absolute_residual: f64,
    /// The absolute mean residual over the final window.
    pub final_error: f64,
    /// The root-mean-square residual over the fitted samples.
    pub root_mean_square_error: f64,
    /// Whether both residual requirements hold.
    pub adequate: bool,
    /// The analysis the structure was fitted with: the first-order reference's, or the sensor-lag search's.
    pub analysis: A,
    /// The model of the structure.
    pub model: IdentifiedModel,
    /// The fit evidence of the model on the estimation record.
    pub fit: ModelFitEvidence,
    /// The model's equation with its coefficients.
    pub equation: String,
}

/// The structure comparison of a held step (`SensorLagComparison`; N2.10): both candidates, the requirements they were
/// judged by, the root-mean-square improvement of the additional lag, the recommendation and, once the engineer chose,
/// the structure. The comparison is provisional, and the additional lag's physical origin is unresolved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SensorLagComparison {
    /// The first-order model with dead time, the reference.
    pub candidate_a: StructureCandidate<StepResponseAnalysis>,
    /// The second-order model with dead time, the additional lag.
    pub candidate_b: StructureCandidate<SensorLagStepAnalysis>,
    /// The residual requirements both candidates were judged by.
    pub requirements: ModelAdequacy,
    /// The first-order root-mean-square residual less the additional lag's.
    pub rms_improvement: f64,
    /// The recommended structure.
    pub recommended: StructureRecommendation,
    /// The structure the engineer chose, which must be the recommended one; absent until a choice is made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<StructureChoice>,
}

/// The first problem of the sensor-lag inputs of held-step settings (`stepResponseSettingsProblem`, its sensor-time and
/// model-adequacy parts): the sensor-time grid, when given, has bounds and a step above zero, a maximum at least its minimum,
/// and a step that divides its span exactly within the grid limit; the residual requirements, when given, are above zero.
/// `None` when the inputs are admissible or absent.
#[must_use]
pub fn sensor_lag_settings_problem(settings: &StepResponseSettings) -> Option<SettingsProblem> {
    settings
        .sensor_time_constant
        .as_ref()
        .and_then(sensor_grid_problem)
        .or_else(|| settings.model_adequacy.and_then(adequacy_problem))
}

/// The first problem of a sensor-time grid (see [`sensor_lag_settings_problem`]).
fn sensor_grid_problem(grid: &SearchGrid) -> Option<SettingsProblem> {
    for (field, value) in [
        ("minimum", grid.minimum),
        ("maximum", grid.maximum),
        ("step", grid.step),
    ] {
        if !is_positive(value) {
            return Some(problem(
                format!("sensorTimeConstant.{field}"),
                "Enter a finite value greater than zero.",
            ));
        }
    }
    if grid.maximum < grid.minimum {
        return Some(problem(
            "sensorTimeConstant.maximum",
            "Enter a maximum at least as large as the minimum.",
        ));
    }
    let intervals = (grid.maximum - grid.minimum) / grid.step;
    let exact = (intervals - intervals.round()).abs() <= EPSILON;
    if !intervals.is_finite() || !exact || intervals > MAXIMUM_SENSOR_INTERVALS {
        return Some(problem(
            "sensorTimeConstant.step",
            "Choose a bounded grid step that divides the sensor-time span exactly.",
        ));
    }
    None
}

/// The first problem of the residual requirements (see [`sensor_lag_settings_problem`]).
fn adequacy_problem(adequacy: ModelAdequacy) -> Option<SettingsProblem> {
    [
        (
            "maximumAbsoluteResidual",
            adequacy.maximum_absolute_residual,
        ),
        ("maximumFinalError", adequacy.maximum_final_error),
    ]
    .into_iter()
    .find(|(_, value)| !is_positive(*value))
    .map(|(field, _)| {
        problem(
            format!("modelAdequacy.{field}"),
            "Enter a finite model-error limit greater than zero.",
        )
    })
}

/// Whether a number is finite and above zero.
fn is_positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

/// A settings problem with its field and its sentence.
fn problem(field: impl Into<String>, message: &str) -> SettingsProblem {
    SettingsProblem {
        field: field.into(),
        message: message.to_string(),
    }
}

/// Assesses a structure's residuals against the requirements (`assess` in `sensorLagComparison.ts`).
///
/// The final error is the absolute mean residual over the samples in the final window `[start, end)`. Over no samples it is
/// not a number, and the structure is then not adequate.
#[must_use]
pub fn assess_structure(
    points: &[ModelFitPoint],
    final_window: &StepResponseWindow,
    requirements: ModelAdequacy,
) -> StructureAssessment {
    let maximum_absolute_residual = points.iter().fold(f64::NEG_INFINITY, |largest, point| {
        largest.max(point.residual.abs())
    });
    let final_residuals: Vec<f64> = points
        .iter()
        .filter(|point| {
            point.time_seconds >= final_window.start_seconds
                && point.time_seconds < final_window.end_seconds
        })
        .map(|point| point.residual)
        .collect();
    let final_error =
        (final_residuals.iter().sum::<f64>() / count_f64(final_residuals.len())).abs();
    let root_mean_square_error = (points
        .iter()
        .map(|point| point.residual * point.residual)
        .sum::<f64>()
        / count_f64(points.len()))
    .sqrt();
    StructureAssessment {
        maximum_absolute_residual,
        final_error,
        root_mean_square_error,
        adequate: maximum_absolute_residual <= requirements.maximum_absolute_residual + 1e-9
            && final_error <= requirements.maximum_final_error + 1e-9,
    }
}

/// The improvement of the additional lag in root-mean-square error over the first-order model (`rmsImprovement`).
#[must_use]
pub fn rms_improvement(
    first_order: &StructureAssessment,
    additional_lag: &StructureAssessment,
) -> f64 {
    first_order.root_mean_square_error - additional_lag.root_mean_square_error
}

/// The recommended structure (`recommended` in `sensorLagComparison.ts`): the first-order model when it is adequate;
/// otherwise the additional lag when it is adequate and improves the root-mean-square error by more than the measurement
/// bound; otherwise neither.
#[must_use]
pub fn recommend_structure(
    first_order: &StructureAssessment,
    additional_lag: &StructureAssessment,
    rms_improvement: f64,
    measurement_bound: f64,
) -> StructureRecommendation {
    if first_order.adequate {
        StructureRecommendation::FirstOrder
    } else if additional_lag.adequate && rms_improvement > measurement_bound {
        StructureRecommendation::AdditionalLag
    } else {
        StructureRecommendation::Neither
    }
}

/// Accepts the engineer's structure choice only when it is the recommended structure (`chooseSensorLagModel`).
///
/// `recommended` is `None` when no comparison has been calculated.
///
/// # Errors
/// Returns a [`Refusal`] when no comparison has been calculated, and otherwise a refusal naming why the choice is not
/// supported: the first-order model is adequate, or the structure does not meet the requirements.
pub fn choose_structure(
    recommended: Option<StructureRecommendation>,
    choice: StructureChoice,
) -> Result<StructureChoice, Refusal> {
    let Some(recommended) = recommended else {
        return Err(Refusal::new(CALCULATE_FIRST));
    };
    let supported = match recommended {
        StructureRecommendation::FirstOrder => StructureChoice::FirstOrder,
        StructureRecommendation::AdditionalLag => StructureChoice::AdditionalLag,
        StructureRecommendation::Neither => {
            return Err(Refusal::new(STRUCTURE_NOT_MET));
        }
    };
    if choice == supported {
        return Ok(choice);
    }
    Err(Refusal::new(
        if recommended == StructureRecommendation::FirstOrder {
            FIRST_ORDER_ADEQUATE
        } else {
            STRUCTURE_NOT_MET
        },
    ))
}

/// A count as a float. A count of samples is far below 2⁵³, so the conversion is exact.
#[allow(clippy::cast_precision_loss)]
fn count_f64(count: usize) -> f64 {
    count as f64
}

#[cfg(test)]
#[path = "sensor_lag_comparison_tests.rs"]
pub(crate) mod tests;
