//! The Tune of a measured-disturbance feedforward (specification 35, C8): the feedback PI on the path from the controller output to the
//! measurement, designed for the requirements through the production path support (B4.4, C7), and the feedforward gain, lead and lag
//! from both reduced paths. The answer shows both paths, the three values and the largest deviation on the model, with the PI alone and
//! with the feedforward. Tune runs on the model only, and the candidate is unapplied.
//!
//! The feedforward acts outside the loop, so the margins, the prediction and the outcome of the answer are those of the PI on the output
//! path (C0, C8). The model has two inputs, so its tuning model is the output path's first-order model, and the requirements are filled on
//! that path as a one-input model of the loop.

use std::collections::BTreeMap;

use trust_control_math::model::{OperatingPoint, TransferFunction};
use trust_control_math::tuning::two_point::TwoPointReduction;

use super::{
    answer, declared_response, defect, ready_reason, refused, Design, Evaluation, LoopSetup,
};
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::plant::IdentifiedModel;
use crate::identification::recorded_model::check_gain_sign;
use crate::loop_definition::LoopDefinition;
use crate::messages::{FeedforwardAnswer, FeedforwardPath, TuneRequest, TuneResult};
use crate::model::GeneralModel;
use crate::study::tune::prediction::{current_prediction, TraceNames};
use crate::study::tune::requirements::requirements_for;
use crate::study::tune::{
    check_settings, current_for, stage_evidence, TuneInputs, TuneOutcome, TuneSection,
};
use crate::tuning::candidate::{CandidateSource, LawCandidate};
use crate::tuning::path_support::{path_model, ReducedPathSupport};
use crate::tuning::prediction::RecordShaping;
use crate::tuning::project_candidate::{feedforward_candidate, served_feedforward};
use crate::tuning::project_seed::{FeedforwardPidSeed, FeedforwardTerms};
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::settings_pins::mapped_parameters;
use crate::tuning::state_controller::coefficients::ratio_unit;
use crate::tuning::structure::Structure;
use crate::tuning::structures::feedforward::{
    design_feedforward, reduce_paths, FeedforwardChannels, FeedforwardDesign,
};
use crate::tuning::structures::path::{operating_point, operating_value, reduced_plant, LoopFacts};
use crate::tuning::structures::texts;

/// The Tune of a measured-disturbance feedforward (C8) on the study's model: the candidate and the answer (B4.1 to B4.6).
///
/// # Errors
/// The loop's own checks (`require_controller_law`, `rust_engine_support`) and the request's (settings, Automatic only, a PI at most,
/// no pins); a model whose channels are not the feedforward's, or whose disturbance has no identification step; a gain whose sign
/// contradicts the controller action; requirements that cannot be formed; or a PI that cannot meet them on its path.
pub(super) fn run(
    inputs: &TuneInputs<'_>,
    held: Option<&TuningRequirements>,
    running: Option<&BTreeMap<String, f64>>,
) -> Result<TuneOutcome, Failure> {
    let loop_definition = inputs.loop_definition;
    let request = inputs.request;
    let Some(seed) = served_feedforward(loop_definition) else {
        return Err(defect(
            "Tune reached the feedforward design without a feedforward seed.",
        ));
    };
    loop_definition.require_controller_law()?;
    let current = current_for(inputs.model, loop_definition)?;
    loop_definition.rust_engine_support()?;
    check_settings(request)?;
    check_request(request)?;
    let shape = shape_of(loop_definition, &current.model)?;
    let paths = reduce_paths(&current.model, &shape.channels)
        .map_err(refused(Cause::ModelOrData, NextAction::Identify))?;
    let (operating_inputs, operating_outputs) = operating_point(&current.model);
    let operating = (
        operating_value(&operating_inputs, shape.channels.controller_input),
        operating_value(&operating_outputs, shape.channels.measurement),
    );
    let view = output_view(&current.model, &shape.channels, &paths.output, operating)?;
    check_gain_sign(loop_definition, paths.output.gain, false)
        .map_err(refused(Cause::ModelOrData, NextAction::OpenLoopSetup))?;
    let requirements = requirements_for(
        loop_definition,
        &view.model,
        &view.general,
        inputs.previous,
        held,
        request,
    )
    .map_err(refused(Cause::Settings, NextAction::OpenRequirements))?;
    requirements
        .validate()
        .map_err(refused(Cause::Settings, NextAction::OpenRequirements))?;

    let setup = LoopSetup::of(loop_definition, &view.general, false);
    let design = design_feedforward(
        &current.model,
        &facts_of(&setup),
        &shape.channels,
        Some(shape.disturbance_step),
        seed.lead_lag.is_some(),
        &requirements,
        &ReducedPathSupport,
    )
    .map_err(refused(Cause::TaskInfeasible, NextAction::OpenRequirements))?;
    let answer_section = feedforward_answer(&design, &current.model, &shape.channels);
    let (designed, candidate) = design_of(seed, &design, &requirements);

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
    let mapped = mapped_parameters(loop_definition);
    let predictor = setup.predictor(&view.model, RecordShaping::default());
    let context = setup.context(&predictor);
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
    let mut result: TuneResult =
        answer(None, designed, candidate, Vec::new(), evidence, &evaluation);
    result.feedforward = Some(answer_section);
    Ok(TuneOutcome { section, result })
}

/// The request of a feedforward (C8): Automatic designs it, the feedback law is a PI, and no setting is pinned yet.
///
/// # Errors
/// A named method, a structure other than PI, or pinned settings.
fn check_request(request: &TuneRequest) -> Result<(), Failure> {
    if request.method.is_some() {
        return Err(Failure::new(
            texts::FEEDFORWARD_AUTOMATIC,
            Cause::Settings,
            NextAction::ChooseMethod,
        ));
    }
    if request
        .structure
        .is_some_and(|structure| structure != Structure::PI)
    {
        return Err(Failure::new(
            texts::FEEDFORWARD_STRUCTURE,
            Cause::Settings,
            NextAction::Tune,
        ));
    }
    if request.pins.is_some() {
        return Err(Failure::new(
            texts::FEEDFORWARD_PINS,
            Cause::Settings,
            NextAction::Tune,
        ));
    }
    Ok(())
}

/// The channels of a feedforward on the model and the disturbance's identification step (C8): the model's controller output,
/// measured disturbance and measurement, bound to the loop's channels by their identifiers.
pub(crate) struct Shape {
    pub(crate) channels: FeedforwardChannels,
    pub(crate) disturbance_step: f64,
}

/// The feedforward's channels on the model, from the loop's two inputs and its output (C8).
///
/// # Errors
/// The loop has no channels of the feedforward's shape, a channel names no input or output of the model, or the disturbance has no
/// identification step above zero.
pub(crate) fn shape_of(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
) -> Result<Shape, Failure> {
    let loop_channels = loop_definition
        .channels
        .as_ref()
        .ok_or_else(channels_refused)?;
    let [controller, disturbance] = loop_channels.inputs.as_slice() else {
        return Err(channels_refused());
    };
    let identity = model.identity();
    let input_index = |id: &str| identity.inputs.iter().position(|input| input.id == id);
    let output_index = |id: &str| identity.outputs.iter().position(|output| output.id == id);
    let controller_input =
        input_index(controller.model_channel_id.as_str()).ok_or_else(channels_refused)?;
    let disturbance_input =
        input_index(disturbance.model_channel_id.as_str()).ok_or_else(channels_refused)?;
    let measurement = loop_channels
        .outputs
        .first()
        .and_then(|output| output_index(output.model_channel_id.as_str()))
        .ok_or_else(channels_refused)?;
    let disturbance_step = disturbance
        .experiment_step
        .filter(|step| *step > 0.0)
        .ok_or_else(|| {
            Failure::new(
                texts::FEEDFORWARD_STEP,
                Cause::Settings,
                NextAction::OpenLoopSetup,
            )
        })?;
    Ok(Shape {
        channels: FeedforwardChannels {
            controller_input,
            disturbance_input,
            measurement,
        },
        disturbance_step,
    })
}

/// The refusal of a model that does not have the feedforward's channels: the model is not the one the loop's channels name.
fn channels_refused() -> Failure {
    Failure::new(
        texts::FEEDFORWARD_CHANNELS,
        Cause::ModelOrData,
        NextAction::Identify,
    )
}

/// The output path as a model of the loop's one plant input and one output (C8): the model's identity with the controller output and
/// the measurement as its only channels, its operating point at the two operating values, and the output path's first-order form with
/// its delay. The requirements are filled and the predictor and the placement are made on this view, as the feedback PI is designed on it.
pub(crate) struct OutputView {
    pub(crate) general: GeneralModel,
    pub(crate) model: IdentifiedModel,
}

/// The output path's view of the feedforward's model (C8).
///
/// # Errors
/// The model does not have the feedforward's channels, or the path has no first-order model.
pub(crate) fn output_view(
    model: &GeneralModel,
    channels: &FeedforwardChannels,
    output: &TwoPointReduction,
    operating: (f64, f64),
) -> Result<OutputView, Failure> {
    let identity = model.identity();
    let controller = identity
        .inputs
        .get(channels.controller_input)
        .cloned()
        .ok_or_else(channels_refused)?;
    let measurement = identity
        .outputs
        .get(channels.measurement)
        .cloned()
        .ok_or_else(channels_refused)?;
    let mut view_identity = identity.clone();
    view_identity.inputs = vec![controller];
    view_identity.outputs = vec![measurement];
    view_identity.delay_seconds = output.delay_seconds;
    view_identity.operating_point = Some(OperatingPoint {
        inputs: vec![operating.0],
        outputs: vec![operating.1],
    });
    let general = GeneralModel::TransferFunction {
        identity: view_identity,
        transfer: TransferFunction {
            numerator: vec![output.gain],
            denominator: vec![output.time_constant_seconds, 1.0],
        },
    };
    let plant_model =
        path_model(&reduced_plant(output, output.delay_seconds)).ok_or_else(channels_refused)?;
    Ok(OutputView {
        general,
        model: plant_model,
    })
}

/// The loop's facts for the design, read from the setup of the output path's view (B4.4, C7).
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

/// The design of the feedforward's candidate (C8): the PI on the output path with its derivative term zero, and the feedforward gain
/// with its lead and lag when the program has them. Its source is the constrained comparison of a design to requirements, as a
/// pole placement or a proportional design is (B4.4).
fn design_of(
    seed: &FeedforwardPidSeed,
    design: &FeedforwardDesign,
    requirements: &TuningRequirements,
) -> (Design, LawCandidate) {
    let gains = design.pi;
    let terms = FeedforwardTerms {
        gain: design.gain,
        lead_seconds: design.lead_seconds,
        lag_seconds: design.lag_seconds,
    };
    let source = CandidateSource::ConstrainedComparison;
    let candidate = LawCandidate::Project(feedforward_candidate(seed, source, gains, terms));
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

/// The answer's feedforward section (C8): both paths, the three values, the disturbance step and the two largest deviations, each gain
/// with its unit. The units are the model's channels': a gain is the unit of what it gives per the unit of what it takes (`ratio_unit`),
/// so the page composes none.
fn feedforward_answer(
    design: &FeedforwardDesign,
    model: &GeneralModel,
    channels: &FeedforwardChannels,
) -> FeedforwardAnswer {
    let identity = model.identity();
    let input_unit = |index: usize| {
        identity
            .inputs
            .get(index)
            .map(|channel| channel.unit.as_str())
    };
    let controller = input_unit(channels.controller_input);
    let disturbance = input_unit(channels.disturbance_input);
    let measurement = identity
        .outputs
        .get(channels.measurement)
        .map(|channel| channel.unit.as_str());
    let per = |numerator: Option<&str>, denominator: Option<&str>| {
        numerator
            .zip(denominator)
            .map(|(numerator, denominator)| ratio_unit(numerator, denominator))
    };
    FeedforwardAnswer {
        output_path: path_answer(&design.output_path, per(measurement, controller)),
        disturbance_path: path_answer(&design.disturbance_path, per(measurement, disturbance)),
        gain: design.gain,
        gain_unit: per(controller, disturbance),
        disturbance_unit: disturbance.map(str::to_owned),
        measurement_unit: measurement.map(str::to_owned),
        lead_seconds: design.lead_seconds,
        lag_seconds: design.lag_seconds,
        disturbance_step: design.disturbance_step,
        feedback_deviation: design.largest_deviation.feedback,
        feedforward_deviation: design.largest_deviation.feedforward,
    }
}

/// A reduced path as the answer shows it, with the unit of its gain.
fn path_answer(path: &TwoPointReduction, gain_unit: Option<String>) -> FeedforwardPath {
    FeedforwardPath {
        gain: path.gain,
        gain_unit,
        time_constant_seconds: path.time_constant_seconds,
        delay_seconds: path.delay_seconds,
    }
}

#[cfg(test)]
#[path = "feedforward_tests.rs"]
mod tests;
