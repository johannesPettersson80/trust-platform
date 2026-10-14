//! The structure comparison of a sensor-lag model with the first-order reference (N3.2, N3.3; `sensorLagComparison.ts`):
//! the adequacy of each structure's residuals, the recommendation between them, and the engineer's choice, which is
//! accepted only when it is the recommended structure.
//!
//! The comparison keeps every result provisional. The additional lag's physical origin is unresolved, and a chosen
//! structure is never a design acceptance. Building the two candidate models from the retained estimation record is the
//! engine's work and follows this rule set.

use trust_control_math::identify::step_response::StepResponseWindow;

use crate::identification::evidence::ModelFitPoint;
use crate::identification::refusal::Refusal;
use crate::identification::sensor_lag_texts::{
    CALCULATE_FIRST, FIRST_ORDER_ADEQUATE, STRUCTURE_NOT_MET,
};

/// The residual requirements a structure must meet to be adequate (`modelAdequacy`).
#[derive(Debug, Clone, Copy, PartialEq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructureRecommendation {
    /// The first-order model is adequate, so the extra state is not needed.
    FirstOrder,
    /// The first-order model is not adequate, and the additional lag is adequate and improves the fit beyond the measurement bound.
    AdditionalLag,
    /// Neither structure meets the requirements.
    Neither,
}

/// A structure the engineer may choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructureChoice {
    /// The first-order model.
    FirstOrder,
    /// The first-order model with an additional lag.
    AdditionalLag,
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
