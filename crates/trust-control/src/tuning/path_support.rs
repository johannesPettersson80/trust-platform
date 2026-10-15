//! The production path support (specification 35, C6 to C9 and numerical annex N5.12(c)): the engine's response predictor and pole
//! placement for a reduced path. A path is a plant of its own, so it is made into a synthetic identified model of its form, and the
//! predictor and the placement are the selected model's own (B4.5) on that model. A first-order path becomes a first-order model
//! with its dead time, and an integrating path an integrating model. A second-order path has no synthetic model here: it is not
//! predicted and it has no placement form, as the tuning rules refuse such a path (N5.2).

#![allow(clippy::module_name_repetitions)] // `ReducedPathSupport` and `PathSupport` name the module's subject.

use trust_control_math::identify::{LowOrderModelKind, ParameterInterval};

use crate::identification::plant::IdentifiedModel;
use crate::tuning::placement::{ModelPlacement, PlacementRefusal, PolePlacement};
use crate::tuning::plant::TuningPlant;
use crate::tuning::prediction::{RecordShaping, SelectedModelPredictor};
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::shortfall::{DesignContext, OutputLimits, ResponsePredictor, TimePoint, Trial};
use crate::tuning::structure::{ParallelGains, Structure};
use crate::tuning::structures::path::{LoopFacts, PathSupport};
use crate::tuning::typed_pid::TypedDesign;

/// The production path support: the predictor and the placement of a reduced path, made on its synthetic model.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReducedPathSupport;

impl PathSupport for ReducedPathSupport {
    fn predictor(
        &self,
        path: &TuningPlant,
        facts: &LoopFacts,
        operating: (f64, f64),
    ) -> Box<dyn ResponsePredictor + '_> {
        Box::new(PathPredictor {
            model: path_model(path),
            task_period_ms: facts.task_period_ms,
            operating_input: operating.0,
            operating_output: operating.1,
            output_limits: facts.output_limits,
            rate_limits: facts.rate_limits,
            shaping: facts.shaping,
        })
    }

    fn placement(
        &self,
        path: &TuningPlant,
        requirements: &TuningRequirements,
        context: &DesignContext<'_>,
    ) -> Box<dyn PolePlacement + '_> {
        match path_model(path) {
            Some(model) => Box::new(ModelPlacement::for_identified(
                &model,
                requirements,
                context,
            )),
            None => Box::new(NoForm),
        }
    }
}

/// The synthetic model of a reduced path (B4.5): a first-order model with its gain, its time constant and its dead time, which is the
/// form a two-point reduction gives (N5.15), or an integrating model with its rate gain and dead time. A second-order path has no
/// synthetic model, and `None` is returned for it.
#[must_use]
pub fn path_model(path: &TuningPlant) -> Option<IdentifiedModel> {
    match path {
        TuningPlant::FirstOrder(plant) => {
            let kind = if plant.delay_seconds > 0.0 {
                LowOrderModelKind::FirstOrderDeadTime
            } else {
                LowOrderModelKind::FirstOrder
            };
            Some(form(
                kind,
                plant.gain,
                Some(plant.time_constant_seconds),
                plant.delay_seconds,
            ))
        }
        TuningPlant::Integrating(plant) => Some(form(
            LowOrderModelKind::Integrating,
            plant.rate_gain,
            None,
            plant.delay_seconds,
        )),
        // An integrator with a lag is the integrating kind with its lag as the time constant, as the Tune model reads it (N5.12(c)).
        TuningPlant::IntegratingWithLag(plant) => Some(form(
            LowOrderModelKind::Integrating,
            plant.rate_gain,
            Some(plant.lag_seconds),
            plant.delay_seconds,
        )),
        TuningPlant::SecondOrder(_) => None,
    }
}

/// A stable identified model of a form, with its gain, its first time constant and its dead time. It carries no fit evidence: a
/// synthetic path is not a fit, so it records no error and no residuals.
fn form(
    kind: LowOrderModelKind,
    gain: f64,
    time_constant_seconds: Option<f64>,
    dead_time_seconds: f64,
) -> IdentifiedModel {
    IdentifiedModel {
        kind,
        gain,
        time_constant_seconds,
        second_time_constant_seconds: None,
        dead_time_seconds: Some(dead_time_seconds),
        normalized_error: None,
        normalized_estimation_error: None,
        normalized_validation_error: None,
        estimation_residuals: None,
        validation_residuals: None,
        uncertainty: None,
        stable: true,
        on_search_boundary: false,
    }
}

/// The response predictor of a reduced path: the selected model's predictor (B4.5) on the path's synthetic model, with the loop's
/// period, limits and record shaping, at the path's operating point. A path without a model predicts nothing, so its candidates go
/// unpredicted (N5.12).
struct PathPredictor {
    model: Option<IdentifiedModel>,
    task_period_ms: f64,
    operating_input: f64,
    operating_output: f64,
    output_limits: OutputLimits,
    rate_limits: ParameterInterval,
    shaping: RecordShaping,
}

impl ResponsePredictor for PathPredictor {
    fn predict(
        &self,
        design: &TypedDesign,
        trial: Trial,
        requirements: &TuningRequirements,
    ) -> Option<Vec<TimePoint>> {
        let model = self.model.as_ref()?;
        SelectedModelPredictor {
            model,
            task_period_ms: self.task_period_ms,
            operating_input: self.operating_input,
            operating_output: self.operating_output,
            output_limits: self.output_limits,
            rate_limits: self.rate_limits,
            shaping: self.shaping,
        }
        .predict(design, trial, requirements)
    }
}

/// The placement of a path with no form: every structure is refused, as the model placement refuses a model without one.
struct NoForm;

impl PolePlacement for NoForm {
    fn gains(
        &self,
        _family: Structure,
        _envelope: f64,
        _settling_time_seconds: f64,
    ) -> Result<ParallelGains, PlacementRefusal> {
        Err(PlacementRefusal::NoPlacementForm)
    }
}

#[cfg(test)]
#[path = "path_support_tests.rs"]
mod tests;
