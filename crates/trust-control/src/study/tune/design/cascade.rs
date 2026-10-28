//! The Tune of a cascade (specification 35, C6): the inner PI on its path from the plant input, then the outer PI on the outer
//! measurement's response through the closed inner loop, both designed through the production path support (B4.4, C6). The
//! requirements are those of the outer measurement, the loop's controlled output. Tune runs on the model only, and the candidate is
//! unapplied: it holds both PIs. The answer gives no predicted outcome and no margins, since the specification gives the cascade none
//! (C0 names the linear candidates that have margins).

use trust_control_math::tuning::two_point::TwoPointReduction;

use super::feedforward::output_view;
use super::{declared_response, defect, ready_reason, refused};
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::plant::IdentifiedModel;
use crate::identification::recorded_model::check_gain_sign;
use crate::identification::recording_import::display_engineering_unit;
use crate::loop_definition::{
    channel_signal, ChannelOutput, ChannelRequirements as DeclaredRequirements, LoopDefinition,
};
use crate::messages::{CandidateOutcome, TuneRequest, TuneResult};
use crate::model::GeneralModel;
use crate::study::current_model::CurrentModel;
use crate::study::tune::prediction::CandidateResponse;
use crate::study::tune::record::LoopLimits;
use crate::study::tune::requirements::requirements_for;
use crate::study::tune::result::{tune_result, ResultInputs};
use crate::study::tune::{
    check_settings, controller_action, current_for, stage_evidence, TuneInputs, TuneOutcome,
    TuneSection,
};
use crate::tuning::candidate::{CandidateSource, LawCandidate};
use crate::tuning::cascade_candidate::{cascade_candidate, served_cascade};
use crate::tuning::path_support::ReducedPathSupport;
use crate::tuning::project_seed::{CascadeGains, CascadePidSeed};
use crate::tuning::proportional::GainBounds;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::shortfall::{GainLimit, GainLimits, OutputLimits};
use crate::tuning::structure::Structure;
use crate::tuning::structures::cascade::{
    design_cascade, CascadeDesign, CascadeLoop, CascadeMeasurement, CascadeRequirements,
};
use crate::tuning::structures::channel::ChannelRequirements;
use crate::tuning::structures::feedforward::FeedforwardChannels;
use crate::tuning::structures::path::{
    delayed_plant, open_loop_step_response, operating_point, operating_value, two_point_path,
};

/// The answer's description of a cascade's design (C6), in place of the formula the other structures name.
const DESCRIPTION: &str = "The cascade is designed together (C6): the inner PI on the path from the plant input to the inner measurement, then the outer PI on the outer measurement's response through the closed inner loop. Both controllers are PI.";
/// The answer's reason that no outcome is predicted for a cascade (C6, C0).
const UNPREDICTED: &str = "A cascade's response is not predicted on the model; Validate runs both controllers on the model.";
/// The answer's reason that a cascade has no margins (C0).
const MARGINS_UNAVAILABLE: &str =
    "A cascade has two controllers, so no single loop has margins to show (C0).";
/// The refusal of a named method for a cascade: Automatic designs both PIs (C6).
const AUTOMATIC: &str =
    "A cascade is tuned by Automatic: both of its PIs are designed on their paths.";
/// The refusal of a structure other than PI for a cascade, whose controllers are both PI (C0, C6).
const STRUCTURE: &str =
    "A cascade's two controllers are both PI, so no other structure can be chosen.";
/// The refusal of pinned settings for a cascade, whose two PIs are designed on their paths (C6).
const PINS: &str = "A cascade takes no pinned settings: its two PIs are designed on their paths.";
/// The refusal of a model whose channels are not the cascade's plant input and two measurements (C6).
const CHANNELS: &str =
    "The model's channels do not match the cascade's plant input and measurements.";

/// The Tune of a cascade (C6) on the study's model: the candidate and the answer (B4.1 to B4.6).
///
/// # Errors
/// The loop's own checks (`require_controller_law`, `rust_engine_support`), the request's (settings, Automatic only, a PI only, no
/// pins); a model whose channels are not the cascade's; a gain whose sign contradicts the controller action; requirements that cannot be
/// formed; or a PI that cannot meet them on its path.
pub(super) fn run(
    inputs: &TuneInputs<'_>,
    held: Option<&TuningRequirements>,
) -> Result<TuneOutcome, Failure> {
    let loop_definition = inputs.loop_definition;
    let request = inputs.request;
    let Some(seed) = served_cascade(loop_definition) else {
        return Err(defect(
            "Tune reached the cascade design without a cascade seed.",
        ));
    };
    loop_definition.require_controller_law()?;
    let current = current_for(inputs.model, loop_definition)?;
    loop_definition.rust_engine_support()?;
    check_settings(request)?;
    check_request(request)?;
    let shape = shape_of(loop_definition, &current.model)?;
    let paths = cascade_paths(&current.model, &shape)?;
    check_gain_sign(loop_definition, paths.inner.gain, false)
        .map_err(refused(Cause::ModelOrData, NextAction::OpenLoopSetup))?;
    let (operating_inputs, operating_outputs) = operating_point(&current.model);
    let operating = (
        operating_value(&operating_inputs, shape.plant_input),
        operating_value(&operating_outputs, shape.outer.model_output),
    );
    let view = output_view(
        &current.model,
        &FeedforwardChannels {
            controller_input: shape.plant_input,
            disturbance_input: shape.plant_input,
            measurement: shape.outer.model_output,
        },
        &paths.outer,
        operating,
    )?;
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
    let design = design_cascade(
        &current.model,
        &cascade_loop(loop_definition, seed),
        &shape.inner.measurement(),
        &shape.outer.measurement(),
        &CascadeRequirements {
            task: requirements.clone(),
            inner: shape.inner.own.clone(),
            outer: shape.outer.own.clone(),
        },
        &ReducedPathSupport,
    )
    .map_err(refused(Cause::TaskInfeasible, NextAction::OpenRequirements))?;
    Ok(outcome(
        loop_definition,
        current,
        &view.model,
        requirements,
        design,
        seed,
    ))
}

/// The section to keep and the answer of a cascade's design (C6): the unapplied candidate that holds both PIs, the reason, the requirements
/// and the stage evidence, with no predicted outcome and no margins.
fn outcome(
    loop_definition: &LoopDefinition,
    current: &CurrentModel,
    model: &IdentifiedModel,
    requirements: TuningRequirements,
    design: CascadeDesign,
    seed: &CascadePidSeed,
) -> TuneOutcome {
    let plant_unit = display_engineering_unit(&loop_definition.signals.manipulated_variable.unit);
    let process_unit = display_engineering_unit(&loop_definition.signals.process_variable.unit);
    let candidate = cascade_candidate(seed, CandidateSource::ConstrainedComparison, design);
    let settings = candidate.values.clone();
    let reason = ready_reason(Structure::PI);
    let law = LawCandidate::Project(candidate);
    let (stored, stored_project) = law.clone().into_parts();
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
        reason: reason.clone(),
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
    let result: TuneResult = tune_result(ResultInputs {
        model,
        method: None,
        description: Some(DESCRIPTION.to_owned()),
        action: controller_action(loop_definition),
        process_unit: &process_unit,
        manipulated_unit: &plant_unit,
        candidate: law,
        settings,
        objective: declared_response(&requirements),
        closed_loop_time_seconds: None,
        attempts: Vec::new(),
        reason,
        requirements,
        outcome: CandidateOutcome::Unpredicted {
            reason: UNPREDICTED.to_owned(),
        },
        prediction: CandidateResponse {
            response: None,
            unavailable: Some(UNPREDICTED.to_owned()),
            settling_time_seconds: None,
            overshoot_percent: None,
        },
        margins: (None, Some(MARGINS_UNAVAILABLE.to_owned())),
        comparison: Vec::new(),
        stage_evidence: evidence,
    });
    TuneOutcome { section, result }
}

/// The request of a cascade (C6): Automatic designs both PIs, a PI is the only structure, and no setting is pinned.
///
/// # Errors
/// A named method, a structure other than PI, or pinned settings.
fn check_request(request: &TuneRequest) -> Result<(), Failure> {
    if request.method.is_some() {
        return Err(Failure::new(
            AUTOMATIC,
            Cause::Settings,
            NextAction::ChooseMethod,
        ));
    }
    if request
        .structure
        .is_some_and(|structure| structure != Structure::PI)
    {
        return Err(Failure::new(STRUCTURE, Cause::Settings, NextAction::Tune));
    }
    if request.pins.is_some() {
        return Err(Failure::new(PINS, Cause::Settings, NextAction::Tune));
    }
    Ok(())
}

/// One measured output of a cascade on the model (C6): its index among the model's outputs, its unit and range, and the requirements
/// it declares, which are the loop's own for the outer measurement and none for the inner.
struct Measured {
    model_output: usize,
    unit: String,
    range: OutputLimits,
    own: Option<ChannelRequirements>,
}

impl Measured {
    /// The measurement as the design reads it.
    fn measurement(&self) -> CascadeMeasurement {
        CascadeMeasurement {
            model_output: self.model_output,
            unit: self.unit.clone(),
            range: self.range,
        }
    }
}

/// The cascade's channels on the model (C6): the plant input's index, and the inner and the outer measurements. The outer measurement is
/// the output with a setpoint; the inner one has none.
struct Shape {
    plant_input: usize,
    inner: Measured,
    outer: Measured,
}

/// The paths of a cascade's model that its requirements and gain check read (C6): the inner path from the plant input and the outer one.
struct Paths {
    inner: TwoPointReduction,
    outer: TwoPointReduction,
}

/// The cascade's channels on the model, from the loop's one plant input and its two outputs (C6).
///
/// # Errors
/// The loop has no channels of the cascade's shape, or a channel names no input or output of the model.
fn shape_of(loop_definition: &LoopDefinition, model: &GeneralModel) -> Result<Shape, Failure> {
    let channels = loop_definition
        .channels
        .as_ref()
        .ok_or_else(channels_refused)?;
    let [plant] = channels.inputs.as_slice() else {
        return Err(channels_refused());
    };
    let [first, second] = channels.outputs.as_slice() else {
        return Err(channels_refused());
    };
    let (inner, outer) = match (
        first.setpoint_signal_id.is_some(),
        second.setpoint_signal_id.is_some(),
    ) {
        (false, true) => (first, second),
        (true, false) => (second, first),
        _ => return Err(channels_refused()),
    };
    let plant_input = model
        .identity()
        .inputs
        .iter()
        .position(|input| input.id == plant.model_channel_id)
        .ok_or_else(channels_refused)?;
    Ok(Shape {
        plant_input,
        inner: measured(loop_definition, model, inner)?,
        outer: measured(loop_definition, model, outer)?,
    })
}

/// One measured output of the cascade, bound to its model output and its loop signal.
fn measured(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    output: &ChannelOutput,
) -> Result<Measured, Failure> {
    let model_output = model
        .identity()
        .outputs
        .iter()
        .position(|channel| channel.id == output.model_channel_id)
        .ok_or_else(channels_refused)?;
    let signal = channel_signal(loop_definition, &output.signal_id).ok_or_else(channels_refused)?;
    Ok(Measured {
        model_output,
        unit: display_engineering_unit(signal.unit),
        range: OutputLimits {
            minimum: signal.range.minimum,
            maximum: signal.range.maximum,
        },
        own: output.requirements.map(declared_channel),
    })
}

/// A measurement's own requirements, as the design reads them (C6).
fn declared_channel(own: DeclaredRequirements) -> ChannelRequirements {
    ChannelRequirements {
        reference_change: own.reference_change,
        maximum_error: own.maximum_error,
        settling_band: own.settling_band,
        settling_time_seconds: own.settling_time_seconds,
    }
}

/// The paths of the cascade's model: the plant input's two-point path to the inner measurement, and to the outer one, both from the
/// model's open-loop step response (C6, C7).
///
/// # Errors
/// The model has no delayed plant, its response does not settle, or a path has no steady change.
fn cascade_paths(model: &GeneralModel, shape: &Shape) -> Result<Paths, Failure> {
    let plant = delayed_plant(model).map_err(refused(Cause::ModelOrData, NextAction::Identify))?;
    let response = open_loop_step_response(&plant, shape.plant_input, "cascade")
        .map_err(refused(Cause::ModelOrData, NextAction::Identify))?;
    let reduce = |model_output: usize, subject: &str| {
        two_point_path(&response.times, &response.outputs[model_output], subject)
            .map_err(refused(Cause::ModelOrData, NextAction::Identify))
    };
    Ok(Paths {
        inner: reduce(shape.inner.model_output, "cascade inner")?,
        outer: reduce(shape.outer.model_output, "cascade outer")?,
    })
}

/// The cascade's loop facts for the design (C6): the action, the period, each controller's gain bounds, the loop's output rates, and the
/// plant input's unit and range, which are the loop's manipulated variable's.
fn cascade_loop(loop_definition: &LoopDefinition, seed: &CascadePidSeed) -> CascadeLoop {
    let limits = LoopLimits::of(loop_definition);
    CascadeLoop {
        action: controller_action(loop_definition),
        task_period_ms: limits.task_period_ms,
        inner_gain_limits: gain_limits(loop_definition, &seed.inner),
        outer_gain_limits: gain_limits(loop_definition, &seed.outer),
        rate_limits: limits.rate,
        plant_input_unit: display_engineering_unit(
            &loop_definition.signals.manipulated_variable.unit,
        ),
        plant_input_limits: limits.output,
    }
}

/// The bounds of one controller's gains, each with its unit, as the loop's parameter binding declares them (N6.4).
fn gain_limits(loop_definition: &LoopDefinition, gains: &CascadeGains) -> GainLimits {
    let limit = |symbol: &str| {
        loop_definition
            .parameter_binding
            .as_ref()?
            .parameters
            .iter()
            .find(|parameter| parameter.symbol == symbol)
            .map(|parameter| GainLimit {
                bounds: GainBounds {
                    minimum: parameter.minimum,
                    maximum: parameter.maximum,
                },
                unit: display_engineering_unit(&parameter.unit),
            })
    };
    GainLimits {
        proportional: limit(gains.proportional_symbol.as_str()),
        integral: limit(gains.integral_symbol.as_str()),
        derivative: limit(gains.derivative_symbol.as_str()),
    }
}

/// The refusal of a loop whose channels are not the cascade's.
fn channels_refused() -> Failure {
    Failure::new(CHANNELS, Cause::ModelOrData, NextAction::Identify)
}

#[cfg(test)]
#[path = "cascade_tests.rs"]
mod tests;
