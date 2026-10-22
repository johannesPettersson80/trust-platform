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
        channel_requirements: None,
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
    assert_eq!(result.method_title, "Simple internal model control (SIMC)");
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

#[test]
fn automatic_recommends_p_first_when_the_declared_task_needs_no_added_term() {
    // control-studio-engineer-pid: "recommends P first for a typed PID adapter until the declared task proves an added term necessary"
    // (specification 35 B4.4 and C0: Tune recommends the simplest structure that meets every mandatory requirement; numerical annex
    // N5.11). A delay-free first-order plant meets the steady-error allowance with a P inside its gain bounds, so P is chosen on a PID
    // law, and PI, PD and PID are still compared and kept as attempts.
    use crate::tuning::recommendation::P_AFTER_COMPARISON;
    let model = IdentifiedModel {
        dead_time_seconds: Some(0.0),
        ..first_order(1.5)
    };
    let designed = with_basis(
        &model,
        ControllerAction::Direct,
        None,
        pid_law(),
        |basis, _| design(None, None, basis),
    )
    .expect("Automatic recommends a candidate");
    assert_eq!(designed.structure, Structure::P);
    assert_eq!(designed.reason, P_AFTER_COMPARISON);
    let structures: Vec<Structure> = designed
        .attempts
        .iter()
        .map(|attempt| attempt.structure)
        .collect();
    assert_eq!(
        structures,
        [Structure::P, Structure::PI, Structure::PD, Structure::PID],
        "every structure of the law is compared, in order"
    );
    assert_eq!(designed.attempts[0].status, AttemptStatus::Feasible);
}

#[test]
fn an_automatic_p_beyond_the_declared_output_limits_is_refused_with_the_requirements() {
    // control-studio-engineer-pid: "rejects an automatic candidate that exceeds declared actuator output limits before Verify" (B4.4:
    // the first output from zero deviation, bias + Kp·Δr, must lie within the output limits). A plant gain of 0.2 needs Kp = 245 for the
    // allowance, and the loop's outputs end at ±100, so the proportional-only law has no candidate.
    let model = IdentifiedModel {
        dead_time_seconds: Some(0.0),
        ..first_order(0.2)
    };
    let law = LawFacts {
        proportional_bounds: Some(GainBounds {
            minimum: 0.0,
            maximum: 1000.0,
        }),
        ..proportional_law()
    };
    let refused = with_basis(&model, ControllerAction::Direct, None, law, |basis, _| {
        design(None, None, basis)
    })
    .expect_err("the first output is outside the output limits");
    assert!(
        refused.message.contains("output limit"),
        "{}",
        refused.message
    );
    assert_eq!(
        (refused.cause, refused.next_action),
        (Cause::TaskInfeasible, NextAction::OpenRequirements)
    );
}

#[test]
fn the_units_of_the_controllers_gains_come_from_the_loops_signals_in_the_notation_of_the_page() {
    // control-studio-chart: "derives controller-setting units from the bound process and output signals" (B9.5, B9.9). The loop
    // stores `degC`, the page reads `°C`; a gain is the plant input per measured output, an integral gain per second, and a derivative
    // gain times second. A signal without a unit is dimensionless "1", which the loop file gives it.
    use crate::model::decode_general_model;
    use crate::tuning::method_description::describe_method;
    let mut loop_definition = plant_loop();
    loop_definition.signals.process_variable.unit = "degC".to_owned();
    loop_definition.signals.manipulated_variable.unit = "%".to_owned();
    let model = decode_general_model(&serde_json::json!({
        "kind": "transfer-function", "revision": "heater", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0,
        "inputs": [{ "id": "u", "unit": "%", "operatingRange": { "minimum": 0.0, "maximum": 100.0 } }],
        "outputs": [{ "id": "y", "unit": "degC", "operatingRange": { "minimum": 0.0, "maximum": 100.0 } }],
        "numerator": [1.5], "denominator": [4.0, 1.0]
    }))
    .expect("a general model");
    let setup = LoopSetup::of(&loop_definition, &model, false);
    assert_eq!(setup.process_unit, "°C");
    assert_eq!(setup.manipulated_unit, "%");
    let described = describe_method(
        &first_order(1.5),
        TuningMethod::Simc,
        TuningObjective::Balanced,
        ControllerAction::Direct,
        &setup.process_unit,
        &setup.manipulated_unit,
    );
    assert_eq!(described.units.proportional_gain, "%/°C");
    assert_eq!(described.units.integral_gain, "%/(°C·s)");
    assert_eq!(described.units.derivative_gain, "(%·s)/°C");
}

/// The shaft of lessons 34 to 37, `G(s) = 1/(s² + s)`: an integrator with a lag of one second.
fn shaft() -> IdentifiedModel {
    IdentifiedModel {
        kind: LowOrderModelKind::Integrating,
        time_constant_seconds: Some(1.0),
        dead_time_seconds: Some(0.0),
        ..first_order(1.0)
    }
}

/// A gain's declared bounds: between `minimum` and `maximum`, in `unit`.
fn declared(minimum: f64, maximum: f64, unit: &str) -> GainLimit {
    GainLimit {
        bounds: GainBounds { minimum, maximum },
        unit: unit.to_owned(),
    }
}

/// The design of a request on `model` to the `requirements`, on a 20 ms task with outputs of ±20, with the gain limits that the loop
/// declares and a PID law.
fn design_on(
    model: &IdentifiedModel,
    requirements: &TuningRequirements,
    gain_limits: GainLimits,
    structure: Option<Structure>,
) -> Result<Design, Failure> {
    let plant = from_identified(model, TuningMethod::Simc)
        .expect("the fixture is a plant the formulas read");
    let predictor = SelectedModelPredictor {
        task_period_ms: 20.0,
        output_limits: OutputLimits {
            minimum: -20.0,
            maximum: 20.0,
        },
        ..loop_predictor(model)
    };
    let context = DesignContext {
        action: ControllerAction::Direct,
        task_period_ms: predictor.task_period_ms,
        operating_input: 0.0,
        operating_output: 0.0,
        output_limits: predictor.output_limits,
        gain_limits,
        process_unit: "m",
        manipulated_unit: "V",
        predictor: &predictor,
    };
    let placement = ModelPlacement::for_identified(model, requirements, &context);
    let basis = DesignBasis {
        plant: &plant,
        requirements,
        context: &context,
        placement: &placement,
        law: pid_law(),
        gain_unit: "m/V",
    };
    design(None, structure, &basis)
}

/// The design of a request on the shaft lifting a weight that pulls with 1.962 on the drive: a unit setpoint change, 0.02 of
/// allowed error and a band of 0.01, settled by 4 s of an 8 s test, and each gain declared between 0 and 100.
fn loaded_shaft_design(structure: Option<Structure>) -> Result<Design, Failure> {
    let requirements = TuningRequirements {
        maximum_error: 0.02,
        settling_band: 0.01,
        settling_time_seconds: 4.0,
        duration_seconds: 8.0,
        constant_input_disturbance: Some(-1.962),
        ..requirements(None)
    };
    let gain_limits = GainLimits {
        proportional: Some(declared(0.0, 100.0, "V/m")),
        integral: Some(declared(0.0, 100.0, "V/(m·s)")),
        derivative: Some(declared(0.0, 100.0, "V·s/m")),
    };
    design_on(&shaft(), &requirements, gain_limits, structure)
}

#[test]
fn a_pd_that_needs_a_gain_beyond_its_range_to_hold_a_load_is_refused_and_pid_is_recommended() {
    // Ported from control-studio-automatic-response.test.ts, "refuses PD when holding a declared load within the allowed error needs a
    // gain beyond its range, and recommends PID" (specification 35, B4.4; numerical annex N5.9, N5.11, N5.12). On the shaft PD holds the
    // load of 1.962 with the error d/Kp, which is within the 0.02 − 0.01 = 0.01 allowed only for Kp ≥ 1.962/0.01 = 196.2, and the loop
    // declares at most 100. PI has no placement on the shaft, and P cannot change its decay. PID has integral action and meets both
    // trials at its second placement, the lessons' Kp 18.77, Ki 17.46 and Kd 7.27.
    use crate::tuning::recommendation::PID_SELECTED;
    let refused =
        loaded_shaft_design(Some(Structure::PD)).expect_err("PD needs a gain beyond its range");
    // The first sentence is the failure's message and the whole text its detail (`Failure::new`).
    assert_eq!(
        refused.message,
        "PD cannot meet the requirements on the model: even its fastest response needs Kp of magnitude 196.2 V/m, above its declared maximum of 100 V/m, to keep its steady-state error within 0.01 m."
    );
    assert!(
        refused.detail.as_deref().is_some_and(|detail| detail
            .ends_with("Choose a structure with integral action or allow a larger error.")),
        "{:?}",
        refused.detail
    );
    assert_eq!(
        (refused.cause, refused.next_action),
        (Cause::TaskInfeasible, NextAction::OpenRequirements)
    );
    let recommended = loaded_shaft_design(None)
        .expect("Automatic recommends a structure that meets the requirements");
    assert_eq!(recommended.structure, Structure::PID);
    assert_eq!(recommended.reason, PID_SELECTED);
    let attempts: Vec<(Structure, AttemptStatus)> = recommended
        .attempts
        .iter()
        .map(|attempt| (attempt.structure, attempt.status))
        .collect();
    assert_eq!(
        attempts,
        [
            (Structure::P, AttemptStatus::Rejected),
            (Structure::PI, AttemptStatus::Rejected),
            (Structure::PD, AttemptStatus::Rejected),
            (Structure::PID, AttemptStatus::Feasible),
        ]
    );
    assert!(recommended.gains.integral_gain > 0.0);
    assert!(recommended.gains.proportional_gain <= 100.0);
    assert!((recommended.gains.proportional_gain - 18.768_926).abs() < 1e-5);
    assert!((recommended.gains.integral_gain - 17.463_786).abs() < 1e-5);
    assert!((recommended.gains.derivative_gain - 7.274_915).abs() < 1e-5);
    let fixed = loaded_shaft_design(Some(Structure::PID)).expect("PID meets the same requirements");
    assert_eq!(
        (fixed.structure, fixed.gains),
        (Structure::PID, recommended.gains)
    );
}

#[test]
fn a_pd_is_the_alternative_when_the_integral_gain_cannot_be_realized() {
    // Ported from control-studio-controller-law.test.ts, "considers a compatible PD alternative instead of claiming all finite error
    // tasks require integral action" (specification 35, B4.4 and B4.7; numerical annex N5.10 to N5.12). The loop declares its integral
    // gain between 0 and 0, so PI and PID cannot be realized; P has no design for a model with a dead time; and PD, the constrained
    // alternative of the formula PI, is recommended while its steady error 1/(1 + K·Kp) = 0.43 is within the 0.5 − 0.02 allowed. With an
    // error limit of 0.1 no structure meets the task, and the refusal keeps PD's steady-state error among the attempts.
    use crate::tuning::recommendation::PD_SELECTED;
    let model = IdentifiedModel {
        gain: 1.0,
        time_constant_seconds: Some(1.0),
        dead_time_seconds: Some(0.1),
        ..first_order(1.0)
    };
    let gain_limits = || GainLimits {
        proportional: Some(declared(0.0, 100.0, "V/m")),
        integral: Some(declared(0.0, 0.0, "V/(m·s)")),
        derivative: Some(declared(0.0, 100.0, "V·s/m")),
    };
    let task = |maximum_error: f64| TuningRequirements {
        maximum_error,
        settling_band: 0.02,
        settling_time_seconds: 2.0,
        duration_seconds: 4.0,
        ..requirements(None)
    };
    let recommended = design_on(&model, &task(0.5), gain_limits(), None)
        .expect("a PD alternative meets the task without integral action");
    assert_eq!(recommended.structure, Structure::PD);
    assert!(is_exactly_zero(recommended.gains.integral_gain));
    assert_eq!(recommended.reason, PD_SELECTED);
    let attempts: Vec<(Structure, AttemptStatus)> = recommended
        .attempts
        .iter()
        .map(|attempt| (attempt.structure, attempt.status))
        .collect();
    assert_eq!(
        attempts,
        [
            (Structure::P, AttemptStatus::Rejected),
            (Structure::PI, AttemptStatus::Rejected),
            (Structure::PD, AttemptStatus::Feasible),
            (Structure::PID, AttemptStatus::Rejected),
        ]
    );
    // With a tighter error limit no structure meets the task: the engine names the fastest miss (N5.12) and opens Requirements.
    let refused = design_on(&model, &task(0.1), gain_limits(), None)
        .expect_err("no structure meets the tighter task");
    assert!(
        refused.message.contains("cannot meet the requirements"),
        "{}",
        refused.message
    );
    assert_eq!(
        (refused.cause, refused.next_action),
        (Cause::TaskInfeasible, NextAction::OpenRequirements)
    );
}
