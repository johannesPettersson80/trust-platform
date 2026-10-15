//! The answer of a Tune run (specification 35, B4.3 to B4.6 and §0.1a; the Tune page specification, "Result and plot"):
//! the unapplied candidate with the `λ` it was designed with and the attempts it was chosen from, the method in words, the
//! reason, the requirements used, the predicted response, the margins with the delay margin, the comparison and the
//! stages. The method's numbers are those of `tuning::method_description`; this module writes them as the engineer reads
//! them, in the shared engineering-number format (B9.5).

use std::collections::BTreeMap;

use trust_control_math::identify::gain::ControllerAction;
use trust_control_math::identify::LowOrderModelKind;

use crate::identification::plant::IdentifiedModel;
use crate::messages::{CandidateOutcome, ComparisonEntry, TuneGainUnits, TuneMethod, TuneResult};
use crate::stages::{stage_availability, StageEvidence};
use crate::study::tune::prediction::CandidateResponse;
use crate::tuning::candidate::LawCandidate;
use crate::tuning::candidate_margins::CandidateMargins;
use crate::tuning::design_to_requirements::structure_word;
use crate::tuning::method_description::{describe_method, MethodDescription};
use crate::tuning::objective::{TuningMethod, TuningObjective};
use crate::tuning::recommendation::Attempt;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::robustness::delay_margin;
use crate::tuning::structure::Structure;
use crate::tuning::texts::{engineering, unit_suffix};

/// The title of the result of Automatic (B4.2).
const AUTOMATIC_TITLE: &str = "Automatic";
/// What AMIGO's result says about the response choice (N5.5: the result must say so).
const AMIGO_RESPONSE: &str =
    "AMIGO has no response-speed parameter, so the response choice does not change these settings.";

/// What the answer of a run is made of (B4.3 to B4.6).
pub(crate) struct ResultInputs<'a> {
    /// The tuning model the candidate was designed on.
    pub(crate) model: &'a IdentifiedModel,
    /// The method the engineer chose, or `None` for Automatic.
    pub(crate) method: Option<TuneMethod>,
    /// The method in words, for a method that no formula describes (the relay rule and manual settings); absent for a
    /// formula method, which is described from its formula.
    pub(crate) description: Option<String>,
    /// The controller action (N6.1).
    pub(crate) action: ControllerAction,
    /// The process variable's unit, as the sentences print it.
    pub(crate) process_unit: &'a str,
    /// The manipulated variable's unit, as the sentences print it.
    pub(crate) manipulated_unit: &'a str,
    /// The candidate, unapplied: a `Control.PID` record, or a program's own law's gains and values.
    pub(crate) candidate: LawCandidate,
    /// The candidate's value of each bound setting, by symbol (B4.2, C0).
    pub(crate) settings: BTreeMap<String, f64>,
    /// The response the design used (N5.4).
    pub(crate) objective: TuningObjective,
    /// The `λ` of the SIMC formula the candidate came from, when it came from one (N5.4).
    pub(crate) closed_loop_time_seconds: Option<f64>,
    /// Every structure Automatic attempted; empty for a named method.
    pub(crate) attempts: Vec<Attempt>,
    /// Why the candidate was chosen.
    pub(crate) reason: String,
    /// The requirements the candidate was designed to.
    pub(crate) requirements: TuningRequirements,
    /// How the candidate stands against the mandatory requirements on its prediction.
    pub(crate) outcome: CandidateOutcome,
    /// The candidate's predicted setpoint response.
    pub(crate) prediction: CandidateResponse,
    /// The candidate's margins, or why there are none.
    pub(crate) margins: (Option<CandidateMargins>, Option<String>),
    /// The constrained comparison of a SIMC request; empty otherwise.
    pub(crate) comparison: Vec<ComparisonEntry>,
    /// The stage evidence after this result, from which the stages are computed.
    pub(crate) stage_evidence: StageEvidence,
}

/// The answer of a run (B4.3 to B4.6): the inputs, with the method's title, its description and the units of the gains
/// from the method's description, and the delay margin from the margins (N7.7). Automatic is described by the SIMC formula
/// it designs from.
pub(crate) fn tune_result(inputs: ResultInputs<'_>) -> TuneResult {
    // The units and the formula's own numbers come from the SIMC formula for the methods that have none (the relay rule and
    // manual settings), which describe their own result.
    let formula = inputs
        .method
        .and_then(TuneMethod::formula)
        .unwrap_or(TuningMethod::Simc);
    let described = describe_method(
        inputs.model,
        formula,
        inputs.objective,
        inputs.action,
        inputs.process_unit,
        inputs.manipulated_unit,
    );
    let automatic = inputs.method.is_none();
    let from_formula = !automatic || inputs.closed_loop_time_seconds.is_some();
    let method_description = match inputs.description {
        Some(text) => text,
        None => method_description(
            &described,
            inputs.model,
            automatic,
            from_formula,
            inputs.candidate.structure(),
        ),
    };
    let (margins, margins_unavailable) = inputs.margins;
    let (candidate, project_candidate) = inputs.candidate.into_parts();
    TuneResult {
        // The feedforward answer (C8) is added by its own Tune run, which holds both paths; other structures have none.
        feedforward: None,
        method_title: inputs
            .method
            .map_or(AUTOMATIC_TITLE, TuneMethod::title)
            .to_string(),
        method_description,
        gain_units: TuneGainUnits {
            proportional: described.units.proportional_gain,
            integral: described.units.integral_gain,
            derivative: described.units.derivative_gain,
        },
        candidate,
        project_candidate,
        settings: inputs.settings,
        method: inputs.method,
        reason: inputs.reason,
        objective: inputs.objective,
        closed_loop_time_seconds: inputs.closed_loop_time_seconds,
        attempts: inputs.attempts,
        requirements: inputs.requirements,
        outcome: inputs.outcome,
        response: inputs.prediction.response,
        prediction_unavailable: inputs.prediction.unavailable,
        settling_time_seconds: inputs.prediction.settling_time_seconds,
        overshoot_percent: inputs.prediction.overshoot_percent,
        delay_margin_seconds: margins.as_ref().map(delay_margin),
        margins,
        margins_unavailable,
        comparison: inputs.comparison,
        stages: stage_availability(&inputs.stage_evidence),
        stage_evidence: inputs.stage_evidence,
    }
}

/// The method in words (B4.4; N5.2 to N5.5, N5.10). A formula is named with the model numbers it reads, its response and
/// `λ` when it has them, and its PI settings, or the reason it does not apply; a candidate that is not the PI itself says how
/// it comes from that PI. Automatic says so first; a candidate of Automatic that comes from no formula (a pole placement or
/// a proportional design) is named with the model it was designed on.
fn method_description(
    described: &MethodDescription,
    model: &IdentifiedModel,
    automatic: bool,
    from_formula: bool,
    structure: Structure,
) -> String {
    if !from_formula {
        return automatic_design(model, &described.units.plant_gain, structure);
    }
    let prefix = if automatic { "Automatic: " } else { "" };
    format!(
        "{prefix}{}{}",
        formula_text(described, model),
        derived_text(structure)
    )
}

/// A formula on the model (N5.2 to N5.6): its title, the model numbers, the response and `λ` of SIMC, and the PI settings
/// `Kp`, `Ti` and `Ki` with their units, or why the formula does not apply. AMIGO's text says that the response choice does
/// not change its settings (N5.5).
fn formula_text(described: &MethodDescription, model: &IdentifiedModel) -> String {
    let reduced = if described.method == TuningMethod::Simc && is_second_order(model) {
        ", reduced by the half rule"
    } else {
        ""
    };
    let basis = format!(
        "{} on {}{reduced}",
        described.title,
        model_numbers(model, &described.units.plant_gain)
    );
    let Some(result) = described.result else {
        return format!(
            "{basis} is unavailable: {}",
            described.reason.as_deref().unwrap_or_default()
        );
    };
    let units = &described.units;
    let response = described
        .closed_loop_time_seconds
        .map_or_else(String::new, |lambda| {
            format!(
                ", with the {} response, λ = {} s",
                response_word(described.objective),
                engineering(lambda, None)
            )
        });
    let note = if described.method == TuningMethod::Amigo {
        format!(" {AMIGO_RESPONSE}")
    } else {
        String::new()
    };
    format!(
        "{basis}{response}: Kp = {}{}, Ti = {} s, Ki = {}{}.{note}",
        engineering(result.proportional_gain, None),
        unit_suffix(&units.proportional_gain),
        engineering(result.integral_time_seconds, None),
        engineering(result.integral_gain, None),
        unit_suffix(&units.integral_gain)
    )
}

/// How a candidate comes from its formula's PI (N5.10): nothing for the PI itself, the integral action removed for P, and the
/// constrained derivative gain for PD and PID. These candidates are labelled Constrained comparison.
fn derived_text(structure: Structure) -> &'static str {
    match structure {
        Structure::PI => "",
        Structure::P => " The P candidate is this PI without its integral action (constrained comparison).",
        Structure::PD => " The PD candidate is this PI without its integral action and with the constrained derivative gain (constrained comparison).",
        Structure::PID => " The PID candidate is this PI with the constrained derivative gain (constrained comparison).",
    }
}

/// A candidate of Automatic that comes from no formula (N5.12(c), N5.8, N5.9a): designed to the requirements on the model,
/// by the pole placement or, for P, by the proportional design of the model.
fn automatic_design(model: &IdentifiedModel, plant_unit: &str, structure: Structure) -> String {
    let source = if structure == Structure::P {
        "the proportional design of the model or its pole placement"
    } else {
        "the pole placement of the model"
    };
    format!(
        "Automatic: {} designed to the requirements on {}, from {source}.",
        structure_word(structure),
        model_numbers(model, plant_unit)
    )
}

/// The numbers of the model that a formula reads (N0.4, N5.2, N5.3): the gain `K`, or the rate gain `K′` of an integrating
/// model, with its unit, then the time constant `T` and the delay `L`, or for a second-order model `τ1`, `τ2` and the delay
/// `θ`.
fn model_numbers(model: &IdentifiedModel, plant_unit: &str) -> String {
    let gain = |name: &str| {
        format!(
            "{name} = {}{}",
            engineering(model.gain, None),
            unit_suffix(plant_unit)
        )
    };
    let seconds = |name: &str, value: f64| format!("{name} = {} s", engineering(value, None));
    let delay = model.dead_time_seconds.unwrap_or(0.0);
    match (
        model.kind,
        model.time_constant_seconds,
        model.second_time_constant_seconds,
    ) {
        (LowOrderModelKind::Integrating, _, _) => {
            format!("{}, {}", gain("K′"), seconds("L", delay))
        }
        (_, Some(first), Some(second)) => format!(
            "{}, {}, {}, {}",
            gain("K"),
            seconds("τ1", first),
            seconds("τ2", second),
            seconds("θ", delay)
        ),
        (_, Some(time), None) => format!(
            "{}, {}, {}",
            gain("K"),
            seconds("T", time),
            seconds("L", delay)
        ),
        (_, None, _) => format!("{}, {}", gain("K"), seconds("L", delay)),
    }
}

/// Whether a model has a second-order form, which SIMC reduces by its half rule (N5.2).
fn is_second_order(model: &IdentifiedModel) -> bool {
    matches!(
        model.kind,
        LowOrderModelKind::SecondOrder | LowOrderModelKind::SecondOrderDeadTime
    )
}

/// The words of a response (N5.4), as the Tune page names them.
fn response_word(objective: TuningObjective) -> &'static str {
    match objective {
        TuningObjective::Fast => "Faster",
        TuningObjective::Balanced => "Balanced",
        TuningObjective::Robust => "More robust",
    }
}
