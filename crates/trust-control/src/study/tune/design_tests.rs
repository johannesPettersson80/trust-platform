//! Tests of the design of a Tune run (specification 35, B4.4 and C1; numerical annex N5.2 to N5.12, N6.4 and N7.7) on
//! hand-built fixtures: a stable first-order model with delay, `1.5·exp(−2·s)/(4·s + 1)`, and an integrating model,
//! `0.05·exp(−s)/s`, each on a 100 ms task from the operating point zero. `run` itself needs a study with an Identify result
//! and the requirement policy of `requirements.rs`; these tests cover the design, the candidate record, the comparison and
//! the answer that `run` composes.

use trust_control_math::identify::LowOrderModelKind;
use trust_control_math::margins::MarginValue;

use super::*;
use crate::identification::identify_result::tests::plant_loop;
use crate::messages::{CandidateOutcome, TuneRole};
use crate::study::tune::record::{candidate_record, LoopLimits, TrackingRule};
use crate::tuning::candidate::DEFAULT_RATE_LIMIT_PER_S;
use crate::tuning::prediction::PredictedGains;
use crate::tuning::recommendation::PI_SELECTED;
use crate::tuning::structure::is_exactly_zero;
use crate::tuning::texts;

/// The first-order model with delay of these tests: `gain·exp(−2·s)/(4·s + 1)`, accepted with a normalized error of 0.05.
fn first_order(gain: f64) -> IdentifiedModel {
    IdentifiedModel {
        kind: LowOrderModelKind::FirstOrderDeadTime,
        gain,
        time_constant_seconds: Some(4.0),
        second_time_constant_seconds: None,
        dead_time_seconds: Some(2.0),
        normalized_error: Some(0.05),
        normalized_estimation_error: None,
        normalized_validation_error: None,
        estimation_residuals: None,
        validation_residuals: None,
        uncertainty: None,
        stable: true,
        on_search_boundary: false,
    }
}

/// The integrating model of these tests: `0.05·exp(−s)/s`.
fn integrating() -> IdentifiedModel {
    IdentifiedModel {
        kind: LowOrderModelKind::Integrating,
        time_constant_seconds: None,
        dead_time_seconds: Some(1.0),
        ..first_order(0.05)
    }
}

/// Requirements that the Balanced SIMC PI of the first-order model meets and its P misses (P keeps a steady error of 0.6):
/// a unit setpoint change, 0.04 of allowed error and a band of 0.02, settled by 40 s of an 80 s test, the Balanced response,
/// and the method of the request.
fn requirements(method: Option<TuningMethod>) -> TuningRequirements {
    TuningRequirements {
        reference_change: 1.0,
        maximum_error: 0.04,
        settling_band: 0.02,
        settling_time_seconds: 40.0,
        duration_seconds: 80.0,
        constant_input_disturbance: None,
        measurement_noise_band: None,
        gain_uncertainty_fraction: None,
        transfer_tolerance: None,
        maximum_overshoot_percent: None,
        tuning_method: method,
        tuning_objective: Some(TuningObjective::Balanced),
        automatic_criteria: None,
    }
}

/// A PID law whose proportional gain is bound between 0 and 100.
fn pid_law() -> LawFacts {
    LawFacts {
        integral_term: true,
        derivative_term: true,
        operating_bias_applies: false,
        proportional_bounds: Some(GainBounds {
            minimum: 0.0,
            maximum: 100.0,
        }),
    }
}

/// A law that has only a proportional gain.
fn proportional_law() -> LawFacts {
    LawFacts {
        integral_term: false,
        derivative_term: false,
        ..pid_law()
    }
}

/// The predictor of `model` on the loop of these tests: a 100 ms task from the operating point zero, outputs between −100
/// and 100, and no declared rate limit.
fn loop_predictor(model: &IdentifiedModel) -> SelectedModelPredictor<'_> {
    SelectedModelPredictor {
        model,
        task_period_ms: 100.0,
        operating_input: 0.0,
        operating_output: 0.0,
        output_limits: OutputLimits {
            minimum: -100.0,
            maximum: 100.0,
        },
        rate_limits: ParameterInterval {
            minimum: -DEFAULT_RATE_LIMIT_PER_S,
            maximum: DEFAULT_RATE_LIMIT_PER_S,
        },
        shaping: RecordShaping::default(),
    }
}

/// Runs `check` with the design basis of `model` for the action, the method and the law, and with its predictor: the loop
/// of [`loop_predictor`], no gain bounds in the shortfall, the model's pole placement, and the requirements of
/// [`requirements`].
fn with_basis<T>(
    model: &IdentifiedModel,
    action: ControllerAction,
    method: Option<TuningMethod>,
    law: LawFacts,
    check: impl FnOnce(&DesignBasis<'_>, &SelectedModelPredictor<'_>) -> T,
) -> T {
    let plant = from_identified(model, method.unwrap_or(TuningMethod::Simc))
        .expect("the fixture is a plant the formulas read");
    let requirements = requirements(method);
    let predictor = loop_predictor(model);
    let context = DesignContext {
        action,
        task_period_ms: predictor.task_period_ms,
        operating_input: 0.0,
        operating_output: 0.0,
        output_limits: predictor.output_limits,
        gain_limits: GainLimits::default(),
        process_unit: "°C",
        manipulated_unit: "%",
        predictor: &predictor,
    };
    let placement = ModelPlacement::for_identified(model, &requirements, &context);
    let basis = DesignBasis {
        plant: &plant,
        requirements: &requirements,
        context: &context,
        placement: &placement,
        law,
        gain_unit: "°C/%",
    };
    check(&basis, &predictor)
}

/// The design of a request on the first-order model with a direct action and a PID law.
fn first_order_design(
    method: Option<TuningMethod>,
    structure: Option<Structure>,
) -> Result<Design, Failure> {
    with_basis(
        &first_order(1.5),
        ControllerAction::Direct,
        method,
        pid_law(),
        |basis, _| design(method, structure, basis),
    )
}

#[test]
fn a_simc_pi_is_unapplied_and_its_gains_carry_the_sign_of_the_action() {
    // Balanced SIMC (N5.2, N5.4): λ = 2·max(L, T/3) = 4 s, Kp = T/(|K|·(λ + L)) = 4/9 and Ti = min(T, 4·(λ + L)) = 4 s.
    for (gain, action, sign) in [
        (1.5, ControllerAction::Direct, 1.0),
        (-1.5, ControllerAction::Reverse, -1.0),
    ] {
        let model = first_order(gain);
        let (designed, candidate) = with_basis(
            &model,
            action,
            Some(TuningMethod::Simc),
            pid_law(),
            |basis, predictor| {
                let designed =
                    design(Some(TuningMethod::Simc), None, basis).expect("SIMC designs a PI");
                let limits = LoopLimits {
                    task_period_ms: predictor.task_period_ms,
                    output: predictor.output_limits,
                    rate: predictor.rate_limits,
                    tracking: TrackingRule::Record,
                };
                let candidate = candidate_record(
                    designed.gains,
                    designed.source,
                    &limits,
                    predictor.shaping,
                    None,
                )
                .expect("a valid record");
                (designed, candidate)
            },
        );
        assert_eq!(designed.structure, Structure::PI);
        assert_eq!(designed.source, CandidateSource::Simc);
        assert_eq!(designed.closed_loop_time_seconds, Some(4.0));
        assert_eq!(designed.attempts, [] as [Attempt; 0]);
        assert!(
            (designed.gains.proportional_gain - sign * 4.0 / 9.0).abs() < 1e-7,
            "Kp has the sign of the action (N6.4): {}",
            designed.gains.proportional_gain
        );
        assert!(
            (designed.gains.integral_gain - sign / 9.0).abs() < 1e-7,
            "Ki takes the sign of Kp: {}",
            designed.gains.integral_gain
        );
        assert!(!candidate.applied, "a candidate is never applied by Tune");
        assert_eq!(candidate.structure, Structure::PI);
        assert_eq!(candidate.source, CandidateSource::Simc);
        assert!(
            (candidate.parameters.tracking_time - 4.0).abs() < 1e-9,
            "Tt = max(Ti, task period)"
        );
    }
}

#[test]
fn the_simc_comparison_keeps_the_formula_pi_as_the_candidate_and_shows_what_p_misses() {
    let model = first_order(1.5);
    with_basis(
        &model,
        ControllerAction::Direct,
        Some(TuningMethod::Simc),
        pid_law(),
        |basis, predictor| {
            let designed =
                design(Some(TuningMethod::Simc), None, basis).expect("SIMC designs a PI");
            let entries = simc_comparison(
                basis.plant,
                basis.requirements,
                basis.context,
                predictor,
                basis.gain_unit,
            );
            assert_eq!(
                entries.len(),
                5,
                "the formula PI, the less aggressive PI, P, PD and PID"
            );
            let formula = &entries[0];
            assert_eq!(
                (formula.label.as_str(), formula.structure),
                ("SIMC", Structure::PI)
            );
            assert_eq!(formula.outcome, CandidateOutcome::Meets);
            assert_eq!(
                formula.gains, designed.gains,
                "the formula PI stays the method's candidate"
            );
            assert!(entries[1..]
                .iter()
                .all(|entry| entry.label == "Constrained comparison"));
            let proportional = entries
                .iter()
                .find(|entry| entry.structure == Structure::P)
                .expect("P is compared");
            let CandidateOutcome::Misses { shortfall } = &proportional.outcome else {
                panic!("P keeps a steady error: {:?}", proportional.outcome);
            };
            assert!(shortfall.contains("steady-state error"), "{shortfall}");
            assert_eq!(
                (designed.structure, designed.source),
                (Structure::PI, CandidateSource::Simc),
                "the comparison creates no candidate"
            );
        },
    );
}

#[test]
fn the_simc_pi_of_the_stable_first_order_model_has_finite_positive_margins() {
    let model = first_order(1.5);
    let designed = first_order_design(Some(TuningMethod::Simc), None).expect("SIMC designs a PI");
    let (margins, unavailable) =
        candidate_margins(&model, &PredictedGains::from_parallel(designed.gains));
    assert_eq!(unavailable, None);
    let margins = margins.expect("a linear candidate on a stable model has margins");
    let (MarginValue::Finite(gain_margin), MarginValue::Finite(phase_margin)) =
        (margins.gain_margin_db, margins.phase_margin_degrees)
    else {
        panic!("finite margins: {margins:?}");
    };
    // The PI cancels the lag, so L(s) = exp(−2·s)/(6·s): GM = 20·log10(6·π/4) ≈ 13.46 dB at π/4 rad/s, and
    // PM = 90° − (2/6)·180°/π ≈ 70.9° at 1/6 rad/s.
    assert!(
        gain_margin.is_finite() && gain_margin > 0.0,
        "{gain_margin}"
    );
    assert!(
        phase_margin.is_finite() && phase_margin > 0.0,
        "{phase_margin}"
    );
    assert!((gain_margin - 13.46).abs() < 1.0, "{gain_margin}");
    assert!((phase_margin - 70.9).abs() < 2.0, "{phase_margin}");
}

#[test]
fn a_named_p_is_the_formula_pi_without_its_integral_action() {
    let designed =
        first_order_design(Some(TuningMethod::Simc), Some(Structure::P)).expect("SIMC gives a P");
    assert_eq!(designed.structure, Structure::P);
    assert!(is_exactly_zero(designed.gains.integral_gain));
    assert!(is_exactly_zero(designed.gains.derivative_gain));
    assert!((designed.gains.proportional_gain - 4.0 / 9.0).abs() < 1e-7);
    assert_eq!(
        designed.source,
        CandidateSource::ConstrainedComparison,
        "a structure derived from the formula PI is not labelled SIMC (N5.10)"
    );
    assert_eq!(designed.closed_loop_time_seconds, Some(4.0));
}

#[test]
fn an_integrating_model_refuses_amigo_and_keeps_p_eligible_for_simc() {
    let model = integrating();
    // AMIGO has no integrating form (N5.5): its checks refuse it, and so does its formula.
    let checked = checked_plant(&plant_loop(), &model, Some(TuningMethod::Amigo))
        .expect_err("AMIGO refuses an integrating model");
    assert_eq!(
        (checked.message.as_str(), checked.next_action),
        (texts::AMIGO_INTEGRATING, NextAction::ChooseMethod)
    );
    let formula = with_basis(
        &model,
        ControllerAction::Direct,
        Some(TuningMethod::Amigo),
        pid_law(),
        |basis, _| design(Some(TuningMethod::Amigo), None, basis),
    )
    .expect_err("AMIGO has no PI for an integrating model");
    assert_eq!(formula.message, texts::AMIGO_INTEGRATING);
    // SIMC keeps P eligible (N5.8): Kp = 1/(|K′|·(λ + L)) with the Balanced λ = 2·L = 2 s, so 1/(0.05·3).
    with_basis(
        &model,
        ControllerAction::Direct,
        Some(TuningMethod::Simc),
        pid_law(),
        |basis, predictor| {
            let designed = design(Some(TuningMethod::Simc), Some(Structure::P), basis)
                .expect("SIMC gives a P");
            assert_eq!(designed.structure, Structure::P);
            assert!((designed.gains.proportional_gain - 1.0 / 0.15).abs() < 1e-6);
            let entries = simc_comparison(
                basis.plant,
                basis.requirements,
                basis.context,
                predictor,
                basis.gain_unit,
            );
            let structures: Vec<Structure> = entries.iter().map(|entry| entry.structure).collect();
            assert_eq!(structures, [Structure::PI, Structure::PI, Structure::P]);
            assert_eq!(entries[2].outcome, CandidateOutcome::Meets);
        },
    );
}

#[test]
fn automatic_recommends_the_simc_pi_and_keeps_every_attempt() {
    // P has no design for a delayed self-regulating model and no placement form; PD keeps a steady error; PI and PID meet.
    let designed = first_order_design(None, None).expect("Automatic recommends a candidate");
    assert_eq!(designed.structure, Structure::PI);
    assert_eq!(designed.source, CandidateSource::Simc);
    assert_eq!(designed.reason, PI_SELECTED);
    assert_eq!(designed.objective, TuningObjective::Balanced);
    assert_eq!(designed.closed_loop_time_seconds, Some(4.0));
    let attempts: Vec<(Structure, AttemptStatus)> = designed
        .attempts
        .iter()
        .map(|attempt| (attempt.structure, attempt.status))
        .collect();
    assert_eq!(
        attempts,
        [
            (Structure::P, AttemptStatus::Rejected),
            (Structure::PI, AttemptStatus::Feasible),
            (Structure::PD, AttemptStatus::Rejected),
            (Structure::PID, AttemptStatus::Feasible),
        ]
    );
}

#[test]
fn a_structure_fixed_under_automatic_is_designed_or_refused_with_its_own_attempt() {
    let pid = first_order_design(None, Some(Structure::PID)).expect("PID meets the requirements");
    assert_eq!(pid.structure, Structure::PID);
    assert_eq!(pid.source, CandidateSource::ConstrainedComparison);
    assert_eq!(pid.closed_loop_time_seconds, Some(4.0));
    assert_eq!(
        pid.reason,
        "PID candidate ready. Validate it before applying."
    );
    assert_eq!(pid.attempts.len(), 4);
    let pd = first_order_design(None, Some(Structure::PD)).expect_err("PD keeps a steady error");
    assert_eq!(
        (pd.cause, pd.next_action),
        (Cause::TaskInfeasible, NextAction::OpenRequirements)
    );
    assert!(
        pd.message
            .starts_with("PD cannot meet the requirements on the model"),
        "{}",
        pd.message
    );
}

#[test]
fn a_proportional_law_offers_p_only() {
    let refused = check_offered(Structure::PI, proportional_law()).expect_err("no integral gain");
    assert_eq!(
        (refused.message.as_str(), refused.cause),
        (
            "The bound ST adapter cannot realize PI.",
            Cause::ControllerCapability
        )
    );
    assert!(check_offered(Structure::P, proportional_law()).is_ok());
    let designed = with_basis(
        &first_order(1.5),
        ControllerAction::Direct,
        Some(TuningMethod::Simc),
        proportional_law(),
        |basis, _| design(Some(TuningMethod::Simc), None, basis),
    )
    .expect("a named method designs P on a proportional law");
    assert_eq!(designed.structure, Structure::P);
}

#[test]
fn the_answer_of_a_simc_pi_carries_its_lambda_its_margins_and_its_response() {
    let model = first_order(1.5);
    let loop_definition = plant_loop();
    let result = with_basis(
        &model,
        ControllerAction::Direct,
        Some(TuningMethod::Simc),
        pid_law(),
        |basis, predictor| {
            let designed =
                design(Some(TuningMethod::Simc), None, basis).expect("SIMC designs a PI");
            let limits = LoopLimits {
                task_period_ms: predictor.task_period_ms,
                output: predictor.output_limits,
                rate: predictor.rate_limits,
                tracking: TrackingRule::Record,
            };
            let candidate = candidate_record(
                designed.gains,
                designed.source,
                &limits,
                predictor.shaping,
                None,
            )
            .expect("a valid record");
            let mapped = mapped_parameters(&loop_definition);
            let evaluation = Evaluation {
                model: &model,
                requirements: basis.requirements,
                predictor,
                context: basis.context,
                names: TraceNames::of(&loop_definition),
                parameters: &mapped,
                current: None,
            };
            let comparison = simc_comparison(
                basis.plant,
                basis.requirements,
                basis.context,
                predictor,
                basis.gain_unit,
            );
            answer(
                Some(TuneMethod::Simc),
                designed,
                crate::tuning::candidate::LawCandidate::Pid(candidate),
                comparison,
                StageEvidence::default(),
                &evaluation,
            )
        },
    );
    assert_eq!(result.method_title, "SIMC PI (Skogestad 2003)");
    assert!(
        result.method_description.contains("λ = 4 s"),
        "{}",
        result.method_description
    );
    assert_eq!(result.closed_loop_time_seconds, Some(4.0));
    assert_eq!(result.attempts, [] as [Attempt; 0]);
    assert_eq!(result.outcome, CandidateOutcome::Meets);
    assert_eq!(result.comparison.len(), 5);
    assert_eq!(result.gain_units.proportional, "%/°C");
    assert!(
        matches!(result.delay_margin_seconds, Some(MarginValue::Finite(seconds)) if seconds > 0.0),
        "{:?}",
        result.delay_margin_seconds
    );
    let response = result.response.expect("a predicted response");
    let roles: Vec<TuneRole> = response.traces.iter().map(|trace| trace.role).collect();
    assert_eq!(
        roles,
        [TuneRole::Setpoint, TuneRole::Measurement, TuneRole::Command]
    );
    assert_eq!(result.prediction_unavailable, None);
    assert!(result.settling_time_seconds.is_some());
}
