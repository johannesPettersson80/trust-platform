//! The Tune of a Smith predictor (specification 35, C9): the PI for the model without its delay, designed for the requirements through the
//! production path support (B4.4, C7), and the model's gain, time constant and delay from the two-point reduction of its step response.
//! The candidate holds the PI and those three values, unapplied, and the answer shows both.
//!
//! With a correct model the predictor makes the loop respond as the delay-free loop (C9), so the answer's prediction and margins are
//! those of the PI on the delay-free first-order model. The model has one input and one output, which the design checks.

use std::collections::BTreeMap;

use trust_control_math::model::{OperatingPoint, TransferFunction};
use trust_control_math::tuning::FirstOrderPlusDelay;

use super::{
    answer, declared_response, defect, ready_reason, refused, Design, Evaluation, LoopSetup,
};
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::plant::IdentifiedModel;
use crate::messages::{TuneRequest, TuneResult};
use crate::model::GeneralModel;
use crate::study::tune::model::tuning_model;
use crate::study::tune::prediction::{current_prediction, TraceNames};
use crate::study::tune::requirements::requirements_for;
use crate::study::tune::{
    check_settings, current_for, stage_evidence, TuneInputs, TuneOutcome, TuneSection,
};
use crate::tuning::candidate::{CandidateSource, LawCandidate};
use crate::tuning::path_support::{path_model, ReducedPathSupport};
use crate::tuning::plant::TuningPlant;
use crate::tuning::prediction::RecordShaping;
use crate::tuning::project_candidate::{served_smith, smith_candidate};
use crate::tuning::project_seed::{SmithPidSeed, SmithTerms};
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::settings_pins::mapped_parameters;
use crate::tuning::structure::{ParallelGains, Structure};
use crate::tuning::structures::path::{operating_point, operating_value, LoopFacts};
use crate::tuning::structures::smith_predictor::{design_smith_predictor, SmithDesign};

/// The refusal of a named method: the Smith predictor's PI is designed by Automatic.
const SMITH_AUTOMATIC: &str =
    "The Smith predictor's PI is designed by Automatic. A named method does not apply to it.";
/// The refusal of a structure other than PI: the Smith predictor's feedback law is a PI (C0, C9).
const SMITH_STRUCTURE: &str =
    "The Smith predictor's feedback law is a PI. Choose PI, or leave the structure to Automatic.";
/// The refusal of pinned settings: the Smith predictor's settings are designed, not pinned.
const SMITH_PINS: &str = "The Smith predictor's settings are designed, so they cannot be pinned.";

/// The Tune of a Smith predictor (C9) on the study's model: the candidate and the answer (B4.1 to B4.6).
///
/// # Errors
/// The loop's own checks (`require_controller_law`, `rust_engine_support`) and the request's (settings, Automatic only, a PI at most,
/// no pins); requirements that cannot be formed; a model that is not one input and one output; a settling time no longer than the
/// model's delay; or a PI that cannot meet the requirements on the delay-free model.
pub(super) fn run(
    inputs: &TuneInputs<'_>,
    held: Option<&TuningRequirements>,
    running: Option<&BTreeMap<String, f64>>,
) -> Result<TuneOutcome, Failure> {
    let loop_definition = inputs.loop_definition;
    let request = inputs.request;
    let Some(seed) = served_smith(loop_definition) else {
        return Err(defect(
            "Tune reached the Smith predictor design without a Smith predictor seed.",
        ));
    };
    loop_definition.require_controller_law()?;
    let current = current_for(inputs.model, loop_definition)?;
    loop_definition.rust_engine_support()?;
    check_settings(request)?;
    check_request(request)?;
    let model = tuning_model(current).map_err(refused(Cause::ModelOrData, NextAction::Identify))?;
    let requirements = requirements_for(
        loop_definition,
        &model,
        &current.model,
        inputs.previous,
        held,
        request,
    )
    .map_err(refused(Cause::Settings, NextAction::OpenRequirements))?;
    requirements
        .validate()
        .map_err(refused(Cause::Settings, NextAction::OpenRequirements))?;

    let setup = LoopSetup::of(loop_definition, &current.model, false);
    let design = design_smith_predictor(
        &current.model,
        &facts_of(&setup),
        &requirements,
        &ReducedPathSupport,
    )
    .map_err(refused(Cause::TaskInfeasible, NextAction::OpenRequirements))?;
    let (operating_inputs, operating_outputs) = operating_point(&current.model);
    let operating = (
        operating_value(&operating_inputs, 0),
        operating_value(&operating_outputs, 0),
    );
    let (designed, candidate) = design_of(seed, &design, &requirements, operating);

    let (stored, stored_project) = candidate.clone().into_parts();
    let section = TuneSection {
        method: None,
        model_revision: current.model.identity().revision.clone(),
        requirements: requirements.clone(),
        candidate: stored,
        project_candidate: stored_project,
        state_candidate: None,
        search_candidate: None,
        on_off_candidate: None,
        group_candidate: None,
        reason: designed.reason.clone(),
        closed_loop_time_seconds: None,
        attempts: Vec::new(),
        pins: None,
    };
    let evidence = stage_evidence(
        loop_definition,
        Some(current),
        Some(&section),
        current.identify.is_some(),
    );
    let view = delay_free_view(&current.model, &design)?;
    let view_setup = LoopSetup::of(loop_definition, &view.general, false);
    let predictor = view_setup.predictor(&view.model, RecordShaping::default());
    let context = view_setup.context(&predictor);
    let mapped = mapped_parameters(loop_definition);
    let running_prediction = running
        .and_then(|running| current_prediction(&predictor, loop_definition, &mapped, running));
    let evaluation = Evaluation {
        model: &view.model,
        requirements: &requirements,
        predictor: &predictor,
        context: &context,
        names: TraceNames::of(loop_definition),
        parameters: &mapped,
        current: running_prediction,
    };
    let result: TuneResult = answer(None, designed, candidate, Vec::new(), evidence, &evaluation);
    Ok(TuneOutcome { section, result })
}

/// The request of a Smith predictor (C9): Automatic designs it, the feedback law is a PI, and no setting is pinned.
///
/// # Errors
/// A named method, a structure other than PI, or pinned settings.
fn check_request(request: &TuneRequest) -> Result<(), Failure> {
    if request.method.is_some() {
        return Err(Failure::new(
            SMITH_AUTOMATIC,
            Cause::Settings,
            NextAction::ChooseMethod,
        ));
    }
    if request
        .structure
        .is_some_and(|structure| structure != Structure::PI)
    {
        return Err(Failure::new(
            SMITH_STRUCTURE,
            Cause::Settings,
            NextAction::Tune,
        ));
    }
    if request.pins.is_some() {
        return Err(Failure::new(SMITH_PINS, Cause::Settings, NextAction::Tune));
    }
    Ok(())
}

/// What a run reads from the loop for the design (B4.2, B9.6): the controller action, the task period, the bounds, the limits and the
/// units, as Tune reads them for a `Control.PID`.
fn facts_of(setup: &LoopSetup) -> LoopFacts {
    LoopFacts {
        action: setup.action,
        task_period_ms: setup.task_period_ms,
        gain_limits: setup.gain_limits.clone(),
        output_limits: setup.output_limits,
        rate_limits: setup.rate_limits,
        shaping: RecordShaping::default(),
        process_unit: setup.process_unit.clone(),
        manipulated_unit: setup.manipulated_unit.clone(),
    }
}

/// The candidate of a Smith predictor (C9): the PI the design chose, and the model's gain, time constant and delay with the operating
/// point the model was identified at (D-18). Its source is the constrained comparison of a design to requirements, as the other
/// project designs' are (B4.4).
fn design_of(
    seed: &SmithPidSeed,
    design: &SmithDesign,
    requirements: &TuningRequirements,
    operating: (f64, f64),
) -> (Design, LawCandidate) {
    let gains: ParallelGains = design.pi;
    let terms = SmithTerms {
        gain: design.model_gain,
        time_constant_seconds: design.model_time_constant_seconds,
        delay_seconds: design.model_delay_seconds,
        operating,
    };
    let source = CandidateSource::ConstrainedComparison;
    let candidate = LawCandidate::Project(smith_candidate(seed, source, gains, terms));
    let designed = Design {
        source,
        structure: Structure::PI,
        gains,
        objective: declared_response(requirements),
        closed_loop_time_seconds: None,
        attempts: Vec::new(),
        reason: ready_reason(Structure::PI),
        description: None,
    };
    (designed, candidate)
}

/// The delay-free model of the Smith predictor's design, as a model of the loop's one input and one output (C9): the first-order model
/// `K/(T·s + 1)` with no delay, at the model's operating point. The PI is designed and predicted on it.
struct DelayFreeView {
    general: GeneralModel,
    model: IdentifiedModel,
}

/// The delay-free view of the design (C9).
///
/// # Errors
/// The first-order model has no identified form, which a finite gain and time constant do not give.
fn delay_free_view(model: &GeneralModel, design: &SmithDesign) -> Result<DelayFreeView, Failure> {
    let identity = model.identity();
    let (operating_inputs, operating_outputs) = operating_point(model);
    let mut view_identity = identity.clone();
    view_identity.inputs = identity.inputs.iter().take(1).cloned().collect();
    view_identity.outputs = identity.outputs.iter().take(1).cloned().collect();
    view_identity.delay_seconds = 0.0;
    view_identity.operating_point = Some(OperatingPoint {
        inputs: vec![operating_value(&operating_inputs, 0)],
        outputs: vec![operating_value(&operating_outputs, 0)],
    });
    let general = GeneralModel::TransferFunction {
        identity: view_identity,
        transfer: TransferFunction {
            numerator: vec![design.model_gain],
            denominator: vec![design.model_time_constant_seconds, 1.0],
        },
    };
    let plant = TuningPlant::FirstOrder(FirstOrderPlusDelay {
        gain: design.model_gain,
        time_constant_seconds: design.model_time_constant_seconds,
        delay_seconds: 0.0,
    });
    let model = path_model(&plant).ok_or_else(|| {
        defect("The delay-free model of the Smith predictor has no identified form.")
    })?;
    Ok(DelayFreeView { general, model })
}

#[cfg(test)]
#[path = "smith_tests.rs"]
mod tests;
