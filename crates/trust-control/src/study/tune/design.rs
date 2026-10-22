//! The design of a Tune run (specification 35, B4.1 to B4.7 and C1; numerical annex N5, N5.R, N6.4 and N8.9): the tuning model
//! and its plant, the requirements, the design of the request, the unapplied candidate record, and the section and the
//! answer of the run. Every rule is one of `tuning`'s; this module decides which applies and in what order. Nothing here
//! reaches a runtime (B4.1).
//!
//! A named method runs its formula unchanged for the structure asked, PI when none is asked (N5.12, last paragraph): the
//! formula PI, its constrained PD or PID (N5.10), or its P, the formula PI without its integral action. Automatic runs the
//! recommendation (N5.11, N5.12) with the model's pole placement; a structure the engineer fixed is never replaced (B4.4).
//! The relay rule ([`super::relay`]) and manual settings ([`super::manual`]) give their own candidates. The pins of the request
//! apply to every design before its record is written (B4.2): the structure is the pinned family, and the pinned setpoint
//! weights and measurement filter shape the prediction of the run (C0). The record itself is [`super::record`]'s.

use std::collections::BTreeMap;

use trust_control_math::identify::gain::ControllerAction;
use trust_control_math::identify::{LowOrderModelKind, ParameterInterval};
use trust_control_math::tuning::integrating_proportional_gain;

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::plant::IdentifiedModel;
use crate::identification::recording_import::display_engineering_unit;
use crate::identification::refusal::Refusal;
use crate::loop_definition::{
    ControllerLaw, InitialSource, InitialValue, LoopDefinition, Parameter,
};
use crate::messages::{ComparisonEntry, TuneMethod, TuneRequest, TuneResult};
use crate::model::GeneralModel;
use crate::stages::StageEvidence;
use crate::study::current_model::CurrentModel;
use crate::study::tune::comparison::simc_comparison;
use crate::study::tune::manual;
use crate::study::tune::model::tuning_model;
use crate::study::tune::prediction::{
    candidate_margins, candidate_outcome, candidate_prediction, current_prediction, TraceNames,
};
use crate::study::tune::record::{
    candidate_record, candidate_settings, gains_map, gains_of_map, pinned_settings, record_shaping,
    shaping_of_candidate, LoopLimits,
};
use crate::study::tune::relay;
use crate::study::tune::requirements::requirements_for;
use crate::study::tune::result::{tune_result, ResultInputs};
use crate::study::tune::state;
use crate::study::tune::{
    check_before_start, controller_action, gain_unit, settings_refused, stage_evidence,
    CandidateFields, TuneInputs, TuneOutcome, TuneSection,
};
use crate::tuning::candidate::{CandidateSource, LawCandidate};
use crate::tuning::cascade_candidate::served_cascade;
use crate::tuning::design_to_requirements::{
    design_to_requirements, nearest_responses, structure_word,
};
use crate::tuning::formula::{action_sign, tune_pi, PiTuning};
use crate::tuning::objective::{TuningMethod, TuningObjective, AUTOMATIC_OBJECTIVE};
use crate::tuning::placement::{ModelPlacement, PolePlacement};
use crate::tuning::plant::{check_normalized_error, check_plant, from_identified, TuningPlant};
use crate::tuning::prediction::{PredictedGains, RecordShaping, SelectedModelPredictor};
use crate::tuning::project_candidate::{
    project_candidate, served_feedforward, served_seed, served_smith, served_trajectory,
    trajectory_candidate,
};
use crate::tuning::project_seed::trajectory_feedforward_of_model;
use crate::tuning::proportional::GainBounds;
use crate::tuning::recommendation::{
    recommend, Attempt, AttemptStatus, RecommendationError, RecommendationInputs,
};
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::settings_pins::{
    apply_pins, mapped_parameters, ControllerSettingsPins, GainKey, MappedParameter,
};
use crate::tuning::shortfall::{
    predicted_outcome, DesignContext, GainLimit, GainLimits, OutputLimits, ResponsePredictor,
};
use crate::tuning::structure::{ParallelGains, Structure};
use crate::tuning::typed_pid::{constrained_comparison, ComparisonLabel, TypedDesign};

/// The refusal of a loop with several channels, which Tune does not design yet (checklist stage V7).
const MULTICHANNEL: &str = "Multichannel tuning is not ported yet (stage V7).";
/// The reason of a P that a named method or a fixed structure designed (`controllerTuning.ts`).
const P_READY: &str = "Candidate ready. Validate it before applying.";
/// The defect of a run that reaches its design without the study's current model, which the checks before the start refuse
/// first.
const NO_MODEL: &str = "Tune reached its design without the study's current model.";
/// The defect of a recommendation without an attempt of the structure the engineer fixed.
const NO_ATTEMPT: &str = "Automatic made no attempt of the structure that was asked for.";
/// The defect of a feasible P that the recommendation did not choose: P is the simplest structure (N5.11).
const P_NOT_CHOSEN: &str = "Automatic found P feasible but recommended another structure.";
/// The defect of a constrained comparison without its P, which N5.10 always holds.
const NO_PROPORTIONAL: &str = "The constrained comparison holds no P candidate.";
/// The defect of a manual entry that is neither a `Control.PID` record nor a program's own law, which the entry never is.
const MANUAL_LAW: &str = "A manual entry is a Control.PID record or a program's own law.";

/// Designs the candidate of a request and gives the section to store, unapplied, and the result to answer (B4.1 to B4.7).
/// The checks before the start run first, so a run refuses what the start refuses.
///
/// # Errors
/// A refusal of the checks before the start; a loop with a feature Tune does not design yet; a model no formula reads or
/// whose error is above the limit; requirements that cannot be formed; a method or a structure that does not apply; no
/// structure that meets the requirements; a relay record that does not qualify; settings that the bounds or the pins refuse;
/// or a candidate record whose limits are not in order.
///
/// `held` is the requirements the study holds when the engineer recorded them (B4.3); see `requirements_for`. `running` is the
/// settings of the running controller, when the engine read them (B4.5); see `current_prediction`.
pub(crate) fn run(
    inputs: &TuneInputs<'_>,
    held: Option<&TuningRequirements>,
    running: Option<&BTreeMap<String, f64>>,
) -> Result<TuneOutcome, Failure> {
    // A gain schedule's points are designed together by `gain_schedule::run_schedule` (C10): one point's single-loop design would give
    // that point's PI alone, so the single loop refuses a scheduled loop.
    if gain_schedule::schedule_of(inputs.loop_definition).is_some() {
        return Err(Failure::new(
            gain_schedule::SINGLE_LOOP_REFUSAL,
            Cause::Implementation,
            NextAction::None,
        ));
    }
    // A measured-disturbance feedforward has two inputs, so its model is not a low-order form: it has its own Tune (C8).
    if served_feedforward(inputs.loop_definition).is_some() {
        return feedforward::run(inputs, held, running);
    }
    // A cascade has two controllers and two measurements, so its design is its own (C6): the inner PI, then the outer.
    if served_cascade(inputs.loop_definition).is_some() {
        return cascade::run(inputs, held);
    }
    // A Smith predictor's PI is designed on the delay-free model, with the model's values that the predictor keeps (C9).
    if served_smith(inputs.loop_definition).is_some() {
        return smith::run(inputs, held, running);
    }
    // A switching controller has its own settings (C3): its design is the switching rule of `tune::on_off`, not a PID's.
    if inputs.loop_definition.controller.on_off.is_some() {
        return crate::study::tune::on_off::run(inputs, held, None);
    }
    // A state-feedback law is designed on the model's state-space form, with the study's state-design inputs (C4); a run
    // without them takes the defaults.
    if inputs.request.method == Some(TuneMethod::StateSpace) {
        return state::run(inputs, held, None);
    }
    // A PID group is designed for each member on the study's model, through the program's decoupler when it has one (C7).
    if inputs.loop_definition.controller.group.is_some() {
        return pid_group::run(inputs, held);
    }
    run_single(inputs, held, running)
}

/// The single-loop design of a request (B4.1 to B4.7, C1): the checks before the start, then the design of the candidate and its answer.
/// `run` gives every loop that is not a gain schedule's to this design. `gain_schedule::run_schedule` calls it for each point of a schedule,
/// so that each point's PI is the one its single loop gives.
pub(crate) fn run_single(
    inputs: &TuneInputs<'_>,
    held: Option<&TuningRequirements>,
    running: Option<&BTreeMap<String, f64>>,
) -> Result<TuneOutcome, Failure> {
    check_before_start(inputs)?;
    let Some(current) = inputs.model else {
        return Err(defect(NO_MODEL));
    };
    let loop_definition = inputs.loop_definition;
    if loop_definition.channels.is_some() {
        return Err(Failure::new(
            MULTICHANNEL,
            Cause::Implementation,
            NextAction::None,
        ));
    }
    loop_definition.rust_engine_support()?;
    let request = inputs.request;
    let model = tuning_model(current).map_err(refused(Cause::ModelOrData, NextAction::Identify))?;
    let requirements = run_requirements(inputs, current, &model, held)?;
    // The manual candidate comes first, because its settings shape the prediction; a design's shape is the pinned one.
    let entry = manual::candidate_of(loop_definition, request, inputs.pins)?;
    let mapped = mapped_parameters(loop_definition);
    let shaping = match &entry {
        Some(candidate) => shaping_of_candidate(candidate),
        None => record_shaping(
            loop_definition,
            &pinned_settings(loop_definition, inputs.pins, &mapped)?,
        ),
    };
    let setup = LoopSetup::of(
        loop_definition,
        &current.model,
        model.kind == LowOrderModelKind::Integrating,
    );
    let predictor = setup.predictor(&model, shaping);
    let context = setup.context(&predictor);
    let (designed, candidate, comparison) = match entry {
        Some(candidate) => {
            let designed = manual::design_of(&candidate, declared_response(&requirements))
                .ok_or_else(|| defect(MANUAL_LAW))?;
            (designed, candidate, Vec::new())
        }
        None => designed_candidate(inputs, &model, &requirements, &setup, &predictor, &context)?,
    };
    let stored = CandidateFields::of(candidate.clone());
    let section = TuneSection {
        method: request.method,
        model_revision: current.model.identity().revision.clone(),
        requirements: requirements.clone(),
        candidate: stored.candidate,
        project_candidate: stored.project_candidate,
        state_candidate: stored.state_candidate,
        search_candidate: stored.search_candidate,
        on_off_candidate: stored.on_off_candidate,
        group_candidate: stored.group_candidate,
        reason: designed.reason.clone(),
        closed_loop_time_seconds: designed.closed_loop_time_seconds,
        attempts: designed.attempts.clone(),
        pins: inputs.pins.cloned(),
    };
    // A fit names the recording it was fitted on, so the study holds one; a supplied model names none (B3.7).
    let evidence = stage_evidence(
        loop_definition,
        Some(current),
        Some(&section),
        current.identify.is_some(),
    );
    let current = running
        .and_then(|running| current_prediction(&predictor, loop_definition, &mapped, running));
    let evaluation = Evaluation {
        model: &model,
        requirements: &requirements,
        predictor: &predictor,
        context: &context,
        names: TraceNames::of(loop_definition),
        parameters: &mapped,
        current,
    };
    let result = answer(
        request.method,
        designed,
        candidate,
        comparison,
        evidence,
        &evaluation,
    );
    Ok(TuneOutcome { section, result })
}

/// The design of a request that no manual entry gives (B4.4, B4.2, C0, C1): the relay rule, or a formula or Automatic design,
/// with the pins applied to its gains and its record written by [`candidate_record`]. The comparison is the constrained one of
/// a SIMC request (N5.10).
fn designed_candidate(
    inputs: &TuneInputs<'_>,
    model: &IdentifiedModel,
    requirements: &TuningRequirements,
    setup: &LoopSetup,
    predictor: &SelectedModelPredictor<'_>,
    context: &DesignContext<'_>,
) -> Result<(Design, LawCandidate, Vec<ComparisonEntry>), Failure> {
    let loop_definition = inputs.loop_definition;
    let request = inputs.request;
    let (designed, comparison) = if request.method == Some(TuneMethod::Relay) {
        let designed = relay::design(
            inputs.relay,
            controller_action(loop_definition),
            declared_response(requirements),
        )?;
        (designed, Vec::new())
    } else {
        let formula = request.method.and_then(TuneMethod::formula);
        let plant = checked_plant(loop_definition, model, formula)?;
        let placement = ModelPlacement::for_identified(model, requirements, context);
        let basis = DesignBasis {
            plant: &plant,
            requirements,
            context,
            placement: &placement,
            law: setup.law,
            gain_unit: &setup.gain_unit,
        };
        let designed = design(formula, fixed_structure(request, inputs.pins), &basis)?;
        let comparison = if formula == Some(TuningMethod::Simc) {
            simc_comparison(&plant, requirements, context, predictor, &setup.gain_unit)
        } else {
            Vec::new()
        };
        (designed, comparison)
    };
    let mapped = mapped_parameters(loop_definition);
    let (designed, applied) = pinned_design(loop_definition, inputs.pins, designed, &mapped)?;
    let candidate = match (
        served_trajectory(loop_definition),
        served_seed(loop_definition),
    ) {
        // A trajectory law (C11) takes the PID design as it stands, and its feedforward gains from the model's transfer function.
        (Some(trajectory), _) => {
            let feedforward = trajectory_gains(inputs)?;
            LawCandidate::Project(trajectory_candidate(
                trajectory,
                designed.source,
                designed.gains,
                feedforward,
            ))
        }
        (None, Some(seed)) => {
            LawCandidate::Project(project_candidate(seed, designed.source, designed.gains))
        }
        (None, None) => LawCandidate::Pid(candidate_record(
            designed.gains,
            designed.source,
            &LoopLimits::of(loop_definition),
            record_shaping(loop_definition, &applied),
            applied.get(&GainKey::BackCalculationGain).copied(),
        )?),
    };
    Ok((designed, candidate, comparison))
}

/// The feedforward gains of a trajectory law (C11) from the study's model: `K_v = b/n` and `K_a = a/n` of the model `n/(a·s² + b·s)`. A
/// model of another form has no trajectory feedforward rule, and Tune refuses the candidate with that sentence (Identify).
fn trajectory_gains(inputs: &TuneInputs<'_>) -> Result<(f64, f64), Failure> {
    let Some(current) = inputs.model else {
        return Err(defect(NO_MODEL));
    };
    trajectory_feedforward_of_model(&current.model)
        .map_err(refused(Cause::ModelOrData, NextAction::Identify))
}

/// The plant of the tuning model (N5.2 to N5.7): the form the named formula reads, or for Automatic the form SIMC reads. A
/// named method's model and domain are checked as the checks before the start check them (Choose another method). For
/// Automatic, a model no formula reads, or whose error is above the limit, is refused (Identify); the formula domains are left
/// to each structure's attempt, so that a structure the formula does not take can still be designed another way (B4.4).
fn checked_plant(
    loop_definition: &LoopDefinition,
    model: &IdentifiedModel,
    method: Option<TuningMethod>,
) -> Result<TuningPlant, Failure> {
    let formula = method.unwrap_or(TuningMethod::Simc);
    let next_action = if method.is_some() {
        NextAction::ChooseMethod
    } else {
        NextAction::Identify
    };
    let refusal = refused(Cause::ModelOrData, next_action);
    let plant = from_identified(model, formula).map_err(&refusal)?;
    check_normalized_error(model, formula).map_err(&refusal)?;
    if method.is_some() {
        let unit = gain_unit(loop_definition, plant.is_integrating());
        check_plant(&plant, formula, controller_action(loop_definition), &unit)
            .map_err(&refusal)?;
    }
    Ok(plant)
}

/// The requirements the candidate is designed to (B4.3): the record `requirements_for` fills from the request, the earlier
/// section and the model, checked as the study checks the section that keeps it.
fn run_requirements(
    inputs: &TuneInputs<'_>,
    current: &CurrentModel,
    model: &IdentifiedModel,
    held: Option<&TuningRequirements>,
) -> Result<TuningRequirements, Failure> {
    let requirements_refused = refused(Cause::Settings, NextAction::OpenRequirements);
    let requirements = requirements_for(
        inputs.loop_definition,
        model,
        &current.model,
        inputs.previous,
        held,
        inputs.request,
    )
    .map_err(&requirements_refused)?;
    requirements.validate().map_err(&requirements_refused)?;
    Ok(requirements)
}

/// What a run reads from the loop and from the operating point of the study's model (B4.2, B9.6, C0): the controller action
/// and task period, the operating point, the output and rate limits, the loop's law, the bounds of its gains, and the units
/// as the sentences print them.
struct LoopSetup {
    action: ControllerAction,
    task_period_ms: f64,
    operating_input: f64,
    operating_output: f64,
    output_limits: OutputLimits,
    rate_limits: ParameterInterval,
    law: LawFacts,
    gain_limits: GainLimits,
    /// The plant gain's unit, as the sign sentence prints it (N6.3).
    gain_unit: String,
    /// The process variable's unit, as the shortfall sentences and the method's description print it.
    process_unit: String,
    /// The manipulated variable's unit, as the shortfall sentences and the method's description print it.
    manipulated_unit: String,
}

impl LoopSetup {
    /// The setup of `loop_definition` for a model with the operating point of `model` (zero where it has none, B9.6). The
    /// limits are those of [`LoopLimits::of`].
    fn of(loop_definition: &LoopDefinition, model: &GeneralModel, integrating: bool) -> Self {
        let signals = &loop_definition.signals;
        let limits = LoopLimits::of(loop_definition);
        let operating = model.identity().operating_point.as_ref();
        Self {
            action: controller_action(loop_definition),
            task_period_ms: limits.task_period_ms,
            operating_input: operating
                .and_then(|point| point.inputs.first())
                .copied()
                .unwrap_or(0.0),
            operating_output: operating
                .and_then(|point| point.outputs.first())
                .copied()
                .unwrap_or(0.0),
            output_limits: limits.output,
            rate_limits: limits.rate,
            law: LawFacts::of(loop_definition),
            gain_limits: gain_limits(loop_definition),
            gain_unit: gain_unit(loop_definition, integrating),
            process_unit: display_engineering_unit(&signals.process_variable.unit),
            manipulated_unit: display_engineering_unit(&signals.manipulated_variable.unit),
        }
    }

    /// The predictor of the selected model on this loop (B4.5): its task period, its operating point, its limits and the
    /// setpoint weights and measurement filter of the record it predicts (C0).
    fn predictor<'a>(
        &self,
        model: &'a IdentifiedModel,
        shaping: RecordShaping,
    ) -> SelectedModelPredictor<'a> {
        SelectedModelPredictor {
            model,
            task_period_ms: self.task_period_ms,
            operating_input: self.operating_input,
            operating_output: self.operating_output,
            output_limits: self.output_limits,
            rate_limits: self.rate_limits,
            shaping,
        }
    }

    /// The design context of this loop with `predictor` (B4.4).
    fn context<'a>(&'a self, predictor: &'a dyn ResponsePredictor) -> DesignContext<'a> {
        DesignContext {
            action: self.action,
            task_period_ms: self.task_period_ms,
            operating_input: self.operating_input,
            operating_output: self.operating_output,
            output_limits: self.output_limits,
            gain_limits: self.gain_limits.clone(),
            process_unit: &self.process_unit,
            manipulated_unit: &self.manipulated_unit,
            predictor,
        }
    }
}

/// What the recommendation reads from the loop's law (B4.4, C0): which terms the law has, and so which structures it offers,
/// whether an initialization adapter measures the first output from the operating input, and the bounds of the proportional
/// gain's parameter.
#[derive(Debug, Clone, Copy, PartialEq)]
struct LawFacts {
    /// Whether the law has an integral term, so that PI is offered.
    integral_term: bool,
    /// Whether the law has a derivative term, so that PD is offered.
    derivative_term: bool,
    operating_bias_applies: bool,
    proportional_bounds: Option<GainBounds>,
}

impl LawFacts {
    /// The facts of the law of `loop_definition`: a `Control.PID` has both terms, a program's own law has the terms its seed names
    /// (C2), and a proportional law has neither.
    fn of(loop_definition: &LoopDefinition) -> Self {
        let (integral_term, derivative_term, proportional_symbol) = match (
            &loop_definition.controller.law,
            served_seed(loop_definition),
        ) {
            (Some(ControllerLaw::Pid(pid)), _) => {
                (true, true, Some(pid.proportional_gain_symbol.as_str()))
            }
            (Some(ControllerLaw::Proportional { gain_symbol }), _) => {
                (false, false, Some(gain_symbol.as_str()))
            }
            (None, Some(seed)) => (
                seed.integral_symbol.is_some(),
                seed.derivative_symbol.is_some(),
                Some(seed.proportional_symbol.as_str()),
            ),
            (None, None) => (false, false, None),
        };
        Self {
            integral_term,
            derivative_term,
            operating_bias_applies: operating_bias_applies(loop_definition),
            proportional_bounds: proportional_symbol
                .and_then(|symbol| bound_parameter(loop_definition, symbol))
                .map(bounds_of),
        }
    }

    /// Whether the law offers `structure`, from its terms (C0).
    fn offers(self, structure: Structure) -> bool {
        structure.offered_by_terms(self.integral_term, self.derivative_term)
    }
}

/// Whether the initial output is measured from the operating input (C1, C2): a `Control.PID` with its library initialization, or a
/// program's own law whose declared states start the integral term from the output.
fn operating_bias_applies(loop_definition: &LoopDefinition) -> bool {
    loop_definition.controller.pid_initialization.is_some()
        || loop_definition
            .controller
            .project
            .as_ref()
            .is_some_and(|project| {
                project.initialization.states().iter().any(|state| {
                    matches!(
                        state.initial,
                        InitialValue::From(InitialSource::PidIntegral { .. })
                    )
                })
            })
}

/// The bounds of the gains of a PID law, by magnitude (N6.4), with each parameter's unit as the shortfall sentences print it
/// (`gainRangeShortfall`): the three gains of a `Control.PID`, or the variables of a program's own law that its seed names (C2). A
/// loop whose law is a proportional law, or has no law, declares none.
fn gain_limits(loop_definition: &LoopDefinition) -> GainLimits {
    let (proportional, integral, derivative) = match (
        &loop_definition.controller.law,
        served_seed(loop_definition),
    ) {
        (Some(ControllerLaw::Pid(pid)), _) => (
            Some(pid.proportional_gain_symbol.as_str()),
            Some(pid.integral_gain_symbol.as_str()),
            Some(pid.derivative_gain_symbol.as_str()),
        ),
        (None, Some(seed)) => (
            Some(seed.proportional_symbol.as_str()),
            seed.integral_symbol.as_deref(),
            seed.derivative_symbol.as_deref(),
        ),
        _ => return GainLimits::default(),
    };
    let limit = |symbol: Option<&str>| {
        symbol
            .and_then(|symbol| bound_parameter(loop_definition, symbol))
            .map(|parameter| GainLimit {
                bounds: bounds_of(parameter),
                unit: display_engineering_unit(&parameter.unit),
            })
    };
    GainLimits {
        proportional: limit(proportional),
        integral: limit(integral),
        derivative: limit(derivative),
    }
}

/// The parameter of the loop's parameter binding that has `symbol`.
fn bound_parameter<'a>(loop_definition: &'a LoopDefinition, symbol: &str) -> Option<&'a Parameter> {
    loop_definition
        .parameter_binding
        .as_ref()?
        .parameters
        .iter()
        .find(|parameter| parameter.symbol == symbol)
}

/// The declared bounds of a parameter.
fn bounds_of(parameter: &Parameter) -> GainBounds {
    GainBounds {
        minimum: parameter.minimum,
        maximum: parameter.maximum,
    }
}

/// What a design reads besides the request (B4.2, B4.4): the plant, the requirements (naming the request's method, as
/// `requirements_for` gives them), the shared context, the model's pole placement, the loop's law and the gain's unit.
struct DesignBasis<'a> {
    plant: &'a TuningPlant,
    requirements: &'a TuningRequirements,
    context: &'a DesignContext<'a>,
    placement: &'a dyn PolePlacement,
    law: LawFacts,
    gain_unit: &'a str,
}

/// The design of a request (B4.4), unapplied: where the candidate comes from, its structure and gains, the response and the
/// `λ` it was designed with, the attempts it was chosen from, the reason, and the description of a method that no formula
/// gives (the relay rule and manual settings).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Design {
    /// Where the candidate comes from.
    pub(crate) source: CandidateSource,
    /// The structure of the gains.
    pub(crate) structure: Structure,
    /// The gains, signed.
    pub(crate) gains: ParallelGains,
    /// The response the design used (N5.4).
    pub(crate) objective: TuningObjective,
    /// The closed-loop time constant `λ` of the formula, when the design has one.
    pub(crate) closed_loop_time_seconds: Option<f64>,
    /// Every structure Automatic attempted; empty for a named method.
    pub(crate) attempts: Vec<Attempt>,
    /// Why the candidate was chosen.
    pub(crate) reason: String,
    /// The method in words, for a method that no formula describes.
    pub(crate) description: Option<String>,
}

/// The design of a request (B4.4): a named method's formula for the structure asked, or Automatic's recommendation, which
/// never replaces a structure the engineer fixed. PI, PD and PID need a PID law (C0, "Families and zero terms"): a loop
/// whose law has only a proportional gain offers P, which is then the structure a named method designs.
fn design(
    method: Option<TuningMethod>,
    structure: Option<Structure>,
    basis: &DesignBasis<'_>,
) -> Result<Design, Failure> {
    if let Some(fixed) = structure {
        check_offered(fixed, basis.law)?;
    }
    match method {
        Some(method) => named_design(
            method,
            structure.unwrap_or_else(|| default_structure(basis.law)),
            basis,
        ),
        None => automatic_design(structure, basis),
    }
}

/// The structure a named method designs when none is asked: PI where the law offers it, P otherwise.
fn default_structure(law: LawFacts) -> Structure {
    if law.offers(Structure::PI) {
        Structure::PI
    } else {
        Structure::P
    }
}

/// Whether the loop's law offers a structure (C0): P always, and PI, PD and PID by the terms the law has (`controllerAdapters.ts`).
fn check_offered(structure: Structure, law: LawFacts) -> Result<(), Failure> {
    if law.offers(structure) {
        return Ok(());
    }
    Err(Failure::new(
        &format!(
            "The bound ST adapter cannot realize {}.",
            structure_word(structure)
        ),
        Cause::ControllerCapability,
        NextAction::OpenLoopSetup,
    ))
}

/// The design of a named method (N5.12, last paragraph): its formula PI for the requirements' response, unchanged. A PD or
/// PID adds the constrained derivative to it (N5.10), and a P is that PI without its integral action, the P of the
/// constrained comparison. The candidate keeps the formula's `λ` (N5.4): SIMC has one, AMIGO none.
fn named_design(
    method: TuningMethod,
    structure: Structure,
    basis: &DesignBasis<'_>,
) -> Result<Design, Failure> {
    let not_applicable = refused(Cause::ModelOrData, NextAction::ChooseMethod);
    let objective = basis.requirements.formula_objective();
    let formula = tune_pi(
        basis.plant,
        method,
        objective,
        basis.context.action,
        basis.gain_unit,
    )
    .map_err(&not_applicable)?;
    let gains = match structure {
        Structure::P => formula_proportional(basis.plant, &formula)?,
        Structure::PI | Structure::PD | Structure::PID => {
            design_to_requirements(
                basis.plant,
                structure,
                basis.requirements,
                basis.context,
                basis.placement,
                basis.gain_unit,
            )
            .map_err(&not_applicable)?
            .gains
        }
    };
    Ok(Design {
        source: formula_source(method, structure),
        structure,
        gains,
        objective,
        closed_loop_time_seconds: formula.closed_loop_time_seconds,
        attempts: Vec::new(),
        reason: ready_reason(structure),
        description: None,
    })
}

/// The P of a formula PI (N5.10): the formula PI without its integral action, as the constrained comparison holds it.
fn formula_proportional(plant: &TuningPlant, formula: &PiTuning) -> Result<ParallelGains, Failure> {
    constrained_comparison(plant, formula)
        .into_iter()
        .find(|candidate| candidate.label == ComparisonLabel::P)
        .map(|candidate| candidate.gains)
        .ok_or_else(|| defect(NO_PROPORTIONAL))
}

/// Where a candidate of a formula comes from (N5.10, C1): the formula's own PI keeps the method's name; a P, PD or PID
/// derived from it is labelled Constrained comparison, never SIMC or AMIGO.
fn formula_source(method: TuningMethod, structure: Structure) -> CandidateSource {
    match (structure, method) {
        (Structure::PI, TuningMethod::Simc) => CandidateSource::Simc,
        (Structure::PI, TuningMethod::Amigo) => CandidateSource::Amigo,
        _ => CandidateSource::ConstrainedComparison,
    }
}

/// The reason of a candidate whose structure was asked for (`controllerTuning.ts`): a P needs the actual ST verified; a PI,
/// PD or PID its initialization too.
fn ready_reason(structure: Structure) -> String {
    match structure {
        Structure::P => P_READY.to_string(),
        Structure::PI | Structure::PD | Structure::PID => format!(
            "{} candidate ready. Validate it before applying.",
            structure_word(structure)
        ),
    }
}

/// The design of Automatic (N5.11, N5.12): the recommendation of the simplest structure that meets the requirements, with
/// every attempt kept (Part C, "Candidates and results"). A structure the engineer fixed is decided by its own attempt: it is
/// designed to the requirements when the attempt is feasible, and refused with the attempt's reason when it is not (B4.4).
fn automatic_design(
    structure: Option<Structure>,
    basis: &DesignBasis<'_>,
) -> Result<Design, Failure> {
    let inputs = RecommendationInputs {
        topology: None,
        unsupported_branch: None,
        has_integral_term: basis.law.integral_term,
        has_derivative_term: basis.law.derivative_term,
        operating_bias_applies: basis.law.operating_bias_applies,
        proportional_bounds: basis.law.proportional_bounds,
        initialization: Ok(()),
        action: basis.context.action,
        context: basis.context,
        requirements: basis.requirements,
        placement: basis.placement,
        gain_unit: basis.gain_unit,
    };
    match (recommend(basis.plant, &inputs), structure) {
        (Ok(recommendation), Some(fixed)) if recommendation.design.structure != fixed => {
            fixed_design(fixed, recommendation.attempts, basis)
        }
        (Ok(recommendation), _) => Ok(automatic(
            recommendation.design,
            recommendation.attempts,
            recommendation.reason,
            basis,
        )),
        (Err(RecommendationError::NoCandidate(attempts)), Some(fixed)) => {
            fixed_design(fixed, attempts, basis)
        }
        (Err(RecommendationError::NoCandidate(attempts)), None) => Err(no_candidate(&attempts)),
        (Err(RecommendationError::Refused(refusal)), _) => Err(Failure::new(
            &refusal.0,
            Cause::TaskInfeasible,
            NextAction::OpenRequirements,
        )),
    }
}

/// A structure the engineer fixed under Automatic, when the recommendation did not choose it (B4.4): a feasible PI, PD or PID
/// is designed to the requirements as its attempt was; a rejected structure is refused with its attempt's reason (B4.7).
fn fixed_design(
    fixed: Structure,
    attempts: Vec<Attempt>,
    basis: &DesignBasis<'_>,
) -> Result<Design, Failure> {
    let Some(attempt) = attempts.iter().find(|attempt| attempt.structure == fixed) else {
        return Err(defect(NO_ATTEMPT));
    };
    match (attempt.status, fixed) {
        (AttemptStatus::Rejected, _) => Err(Failure::new(
            &attempt.reason,
            Cause::TaskInfeasible,
            NextAction::OpenRequirements,
        )),
        (AttemptStatus::Feasible, Structure::P) => Err(defect(P_NOT_CHOSEN)),
        (AttemptStatus::Feasible, _) => {
            let designed = design_to_requirements(
                basis.plant,
                fixed,
                basis.requirements,
                basis.context,
                basis.placement,
                basis.gain_unit,
            )
            .map_err(refused(Cause::TaskInfeasible, NextAction::OpenRequirements))?;
            Ok(automatic(designed, attempts, ready_reason(fixed), basis))
        }
    }
}

/// A design of Automatic as the candidate keeps it. The SIMC formula it came from, when one did, gives its source, its
/// response and its `λ`: the formula's own PI is labelled SIMC, and any structure derived from it Constrained comparison.
/// A design from no formula (a pole placement or a proportional design) is labelled Constrained comparison too, keeps the
/// declared response and has no `λ`.
fn automatic(
    design: TypedDesign,
    attempts: Vec<Attempt>,
    reason: String,
    basis: &DesignBasis<'_>,
) -> Design {
    let formula = simc_formula_of(&design, basis);
    let source = if design.structure == Structure::PI && formula.is_some() {
        CandidateSource::Simc
    } else {
        CandidateSource::ConstrainedComparison
    };
    Design {
        source,
        structure: design.structure,
        gains: design.gains,
        objective: formula.map_or_else(
            || declared_response(basis.requirements),
            |formula| formula.objective,
        ),
        closed_loop_time_seconds: formula.and_then(|formula| formula.closed_loop_time_seconds),
        attempts,
        reason,
        description: None,
    }
}

/// The SIMC formula that made a design of Automatic, when one did (N5.12(a), N5.8): the response, nearest the declared one,
/// whose formula gives the design's own gains. A PI, PD or PID of the formula keeps the formula's proportional gain, and its
/// integral gain when it has integral action; the P of a delayed integrator keeps SIMC's proportional gain
/// `s_a/(|K′|·(λ + L))`. The gains are compared exactly: the recommendation computed them with the same formula from the
/// same numbers, so a placement or a proportional design matches no formula.
fn simc_formula_of(design: &TypedDesign, basis: &DesignBasis<'_>) -> Option<PiTuning> {
    let action = basis.context.action;
    nearest_responses(declared_response(basis.requirements))
        .into_iter()
        .find_map(|objective| {
            let formula = tune_pi(
                basis.plant,
                TuningMethod::Simc,
                objective,
                action,
                basis.gain_unit,
            )
            .ok()?;
            let proportional = match (design.structure, basis.plant) {
                (Structure::P, TuningPlant::Integrating(model)) => {
                    action_sign(action)
                        * integrating_proportional_gain(*model, objective.response_choice()).ok()?
                }
                (Structure::P, _) => return None,
                _ => formula.proportional_gain,
            };
            let integral = !design.structure.has_integral()
                || same_number(design.gains.integral_gain, formula.gains().integral_gain);
            (same_number(design.gains.proportional_gain, proportional) && integral)
                .then_some(formula)
        })
}

/// The response the requirements declare (N5.4): More robust for Automatic when they declare none.
fn declared_response(requirements: &TuningRequirements) -> TuningObjective {
    requirements.tuning_objective.unwrap_or(AUTOMATIC_OBJECTIVE)
}

/// Whether two numbers are the same, bit for bit.
fn same_number(left: f64, right: f64) -> bool {
    left.to_bits() == right.to_bits()
}

/// The refusal when no structure is compatible (B4.7: no candidate meets the requirements, Open Requirements…): the PI
/// attempt's reason first, PI being the simplest structure with integral action, then every attempt with its reason, as
/// `automaticControllerRecommendation.ts` listed them.
fn no_candidate(attempts: &[Attempt]) -> Failure {
    let headline = attempts
        .iter()
        .find(|attempt| attempt.structure == Structure::PI)
        .or_else(|| attempts.first())
        .map_or("", |attempt| attempt.reason.as_str());
    let list = attempts
        .iter()
        .map(|attempt| format!("{}: {}", structure_word(attempt.structure), attempt.reason))
        .collect::<Vec<String>>()
        .join("; ");
    Failure::new(
        &format!("{headline} No compatible automatic controller candidate: {list}"),
        Cause::TaskInfeasible,
        NextAction::OpenRequirements,
    )
}

/// The structure that a request fixes (B4.4, B4.2): the structure the engineer chose, or else the family of the pinned
/// settings, which the pins hold for.
fn fixed_structure(
    request: &TuneRequest,
    pins: Option<&ControllerSettingsPins>,
) -> Option<Structure> {
    request.structure.or(pins.map(|pins| pins.family))
}

/// The design with the pins applied (B4.2, C0): each pinned gain replaces the design's value, and the structure follows from
/// the gains that the record writes. Returns the design and the settings that the pins hold for the record.
fn pinned_design(
    loop_definition: &LoopDefinition,
    pins: Option<&ControllerSettingsPins>,
    designed: Design,
    mapped: &[MappedParameter],
) -> Result<(Design, BTreeMap<GainKey, f64>), Failure> {
    let applied = apply_pins(
        &loop_definition.revision,
        designed.structure,
        &gains_map(designed.gains),
        pins,
        mapped,
    )
    .map_err(settings_refused())?;
    let gains = gains_of_map(&applied);
    let structure = Structure::of_gains(gains.integral_gain, gains.derivative_gain);
    Ok((
        Design {
            structure,
            gains,
            ..designed
        },
        applied,
    ))
}

/// What the answer reads besides the design: the tuning model, the requirements, the predictor and the design context, and
/// the names of the loop's traces.
struct Evaluation<'a> {
    model: &'a IdentifiedModel,
    requirements: &'a TuningRequirements,
    predictor: &'a SelectedModelPredictor<'a>,
    context: &'a DesignContext<'a>,
    names: TraceNames<'a>,
    /// The loop's bound parameters, which name the candidate's settings (B4.2).
    parameters: &'a [MappedParameter],
    /// The settings of the running controller as the comparison predicts them (B4.5), when the engine read them.
    current: Option<PredictedGains>,
}

/// The answer of a run (B4.3 to B4.6): the design and its candidate, the candidate's outcome, predicted response and margins
/// on the model, and for a SIMC request the constrained comparison (N5.10). `stage_evidence` is the evidence the stages come from.
fn answer(
    method: Option<TuneMethod>,
    designed: Design,
    candidate: LawCandidate,
    comparison: Vec<ComparisonEntry>,
    stage_evidence: StageEvidence,
    evaluation: &Evaluation<'_>,
) -> TuneResult {
    let requirements = evaluation.requirements;
    let predictor = evaluation.predictor;
    let context = evaluation.context;
    let typed = TypedDesign {
        structure: designed.structure,
        gains: designed.gains,
    };
    let outcome = candidate_outcome(
        predicted_outcome(&typed, requirements, context),
        predictor,
        designed.gains,
        requirements,
    );
    let settings = candidate_settings(&candidate, evaluation.parameters);
    tune_result(ResultInputs {
        model: evaluation.model,
        method,
        action: context.action,
        process_unit: context.process_unit,
        manipulated_unit: context.manipulated_unit,
        candidate,
        settings,
        objective: designed.objective,
        closed_loop_time_seconds: designed.closed_loop_time_seconds,
        attempts: designed.attempts,
        reason: designed.reason,
        description: designed.description,
        requirements: requirements.clone(),
        outcome,
        prediction: candidate_prediction(
            predictor,
            designed.gains,
            requirements,
            evaluation.names,
            evaluation.current,
        ),
        margins: candidate_margins(evaluation.model, &predictor.predicted_gains(designed.gains)),
        comparison,
        stage_evidence,
    })
}

/// A rule's refusal as the failure that ends the run: the rule's sentence, its cause and its one next action.
fn refused(cause: Cause, next_action: NextAction) -> impl Fn(Refusal) -> Failure {
    move |Refusal(sentence)| Failure::new(&sentence, cause, next_action)
}

/// A defect in Control Studio itself (A11.3), reported as such.
fn defect(what: &str) -> Failure {
    Failure::new(
        &format!("{what} This is a defect in Control Studio."),
        Cause::Implementation,
        NextAction::None,
    )
}

/// The Tune of a measured-disturbance feedforward (C8), which reuses this module's answer and evaluation. Its output-path view and
/// single loop are read by Validate (C8), so the module is visible to the crate.
pub(crate) mod cascade;
pub(crate) mod feedforward;
pub(crate) mod on_off;
pub(crate) mod pid_group;

/// The Tune of a gain schedule (C10): each point's single-loop design, and the table of gains that the points share. The studio
/// reaches `run_schedule` through this module, so it is public.
pub mod gain_schedule;

/// The Tune of a Smith predictor (C9), which reuses this module's answer and evaluation on the delay-free model.
pub(crate) mod smith;

#[cfg(test)]
#[path = "design_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "trajectory_tests.rs"]
mod trajectory_tests;
