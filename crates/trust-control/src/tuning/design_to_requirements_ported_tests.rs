//! Ported assertions of `control-studio-explicit-tuning-method.test.ts`: a declared formula is refused and is not replaced by
//! pole matching. A declared SIMC or AMIGO calculates its formula unchanged (`control-studio-numerics.md`, N5.12, last
//! paragraph); only Automatic without a declared method places poles.

use super::*;
use crate::identification::plant::IdentifiedModel;
use crate::tuning::placement::ModelPlacement;
use crate::tuning::shortfall::{GainLimits, OutputLimits, ResponsePredictor, TimePoint, Trial};
use trust_control_math::identify::LowOrderModelKind;

/// A predictor that predicts nothing: the explicit-method test reads the formula and the placement only.
struct NoPrediction;

impl ResponsePredictor for NoPrediction {
    fn predict(
        &self,
        _design: &TypedDesign,
        _trial: Trial,
        _requirements: &TuningRequirements,
    ) -> Option<Vec<TimePoint>> {
        None
    }
}

/// The integrator `G(s) = 1/s` of the test, as the formula methods read a plant: a rate gain of one and no dead time.
fn integrator() -> TuningPlant {
    TuningPlant::Integrating(IntegratingModel {
        rate_gain: 1.0,
        delay_seconds: 0.0,
    })
}

/// The identified form of the same integrator: an integrating model with gain one and no lag or dead time.
fn integrator_model() -> IdentifiedModel {
    IdentifiedModel {
        kind: LowOrderModelKind::Integrating,
        gain: 1.0,
        time_constant_seconds: None,
        second_time_constant_seconds: None,
        dead_time_seconds: None,
        normalized_error: Some(0.0),
        normalized_estimation_error: None,
        normalized_validation_error: None,
        estimation_residuals: None,
        validation_residuals: None,
        uncertainty: None,
        stable: true,
        on_search_boundary: false,
    }
}

/// The requirements of the test: a unit change, an error limit of 0.1, a settling band of 0.02 and a deadline of 2 s, with
/// the declared method, if any.
fn task(tuning_method: Option<TuningMethod>) -> TuningRequirements {
    TuningRequirements {
        channel_requirements: None,
        reference_change: 1.0,
        maximum_error: 0.1,
        settling_band: 0.02,
        settling_time_seconds: 2.0,
        duration_seconds: 4.0,
        constant_input_disturbance: None,
        measurement_noise_band: None,
        gain_uncertainty_fraction: None,
        transfer_tolerance: None,
        maximum_overshoot_percent: None,
        tuning_method,
        tuning_objective: None,
        automatic_criteria: None,
    }
}

/// The loop of the test: a direct action, a 20 ms task period and an operating point at zero, in metres and volts.
fn direct_context(predictor: &dyn ResponsePredictor) -> DesignContext<'_> {
    DesignContext {
        action: ControllerAction::Direct,
        task_period_ms: 20.0,
        operating_input: 0.0,
        operating_output: 0.0,
        output_limits: OutputLimits {
            minimum: -20.0,
            maximum: 20.0,
        },
        gain_limits: GainLimits::default(),
        process_unit: "m",
        manipulated_unit: "V",
        predictor,
    }
}

/// The PI design of the integrator through `design_for_task`, with the objective that the requirements imply and the unit
/// `m/V/s` of its gain.
fn pi_for_task(
    placement: &ModelPlacement,
    context: &DesignContext,
    requirements: &TuningRequirements,
) -> Result<TypedDesign, Refusal> {
    design_for_task(
        &integrator(),
        Structure::PI,
        requirements,
        requirements.formula_objective(),
        context,
        placement,
        "m/V/s",
    )
}

// control-studio-explicit-tuning-method: "does not replace an explicit unsupported rule with pole matching"
#[test]
fn does_not_replace_an_explicit_unsupported_rule_with_pole_matching() {
    let predictor = NoPrediction;
    let context = direct_context(&predictor);
    let placement = ModelPlacement::for_identified(&integrator_model(), &task(None), &context);

    // No declared method: SIMC refuses the delay-free integrator, and the design is the coefficient-matched PI of the
    // placement. Its gains are `poleMatchedPid` at the 2 % envelope: ω = 4 / (0.8 · 2 s) = 2.5 rad/s, Kp = 2ζω = 4 and
    // Ki = ω² = 6.25. The TypeScript `scope` sentence ("coefficient-matched") has no Rust field, so these gains carry it.
    let design = pi_for_task(&placement, &context, &task(None))
        .expect("the coefficient-matched PI designs a delay-free integrator");
    assert_eq!(design.structure, Structure::PI);
    assert!(
        (design.gains.proportional_gain - 4.0).abs() <= 1e-9,
        "{}",
        design.gains.proportional_gain
    );
    assert!(
        (design.gains.integral_gain - 6.25).abs() <= 1e-9,
        "{}",
        design.gains.integral_gain
    );
    assert!(design.gains.derivative_gain.abs() <= 1e-9);

    // A declared SIMC is refused with its sentence, although the placement could design this plant.
    let simc_refusal = pi_for_task(&placement, &context, &task(Some(TuningMethod::Simc)))
        .expect_err("a declared SIMC needs a positive effective delay");
    assert!(
        simc_refusal.0.contains("positive effective delay"),
        "{}",
        simc_refusal.0
    );

    // A declared AMIGO has no integrating form, and the placement does not replace it either.
    let amigo_refusal = pi_for_task(&placement, &context, &task(Some(TuningMethod::Amigo)))
        .expect_err("a declared AMIGO has no integrating form");
    assert!(amigo_refusal.0.contains("AMIGO"), "{}", amigo_refusal.0);
}
