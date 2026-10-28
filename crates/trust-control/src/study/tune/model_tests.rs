//! Tests of the tuning model (specification 35, B4.2; numerical annex N3.2, N5.2 and N5.7): each low-order form Identify
//! writes is read back from its transfer function, every other transfer function is refused, and the normalized error is
//! the one the study's presentation records for the model. The Identify section built here is shared with the tests of the
//! requirement record.

use trust_control_math::model::{OperatingPoint, StateSpace, TransferFunction};
use trust_control_math::polynomial::convolve;

use super::*;
use crate::experiment::ControlExperimentKind;
use crate::identification::evidence::{
    ModelFitEvidence, ModelFitPoint, OperatingRange as FittedRange,
};
use crate::identification::model_comparison::ComparisonPresentation;
use crate::identification::presentation::identified_model_presentation;
use crate::model::{ModelChannel, ModelIdentity, ModelSource, ModelValidation, OperatingRange};
use crate::recording::EvidenceSource;
use crate::study::current_model::CurrentModelSource;
use crate::study::identify::{IdentifyMethod, IdentifySection, IdentifySignals};
use crate::tuning::objective::TuningMethod;
use crate::tuning::plant::{from_identified, TuningPlant};

/// The estimation recording of the fixture's fit.
const ESTIMATION_RECORDING: &str = "rec-00112233445566778899aabbccddeeff";
/// The validation recording of the fixture's validated fit.
const VALIDATION_RECORDING: &str = "rec-ffeeddccbbaa99887766554433221100";
/// The normalized error of the fixture's fit on its estimation recording.
const ESTIMATION_ERROR: f64 = 0.04;
/// The normalized error the arithmetic passes through.
const ERROR: f64 = 0.07;
/// The refusal of a transfer function that is not a form Identify writes.
const NOT_A_FORM: &str =
    "The model's structure is not a low-order form that the tuning rules take.";

/// A model channel from 0 to 100 in `unit`.
fn channel(id: &str, unit: &str) -> ModelChannel {
    ModelChannel {
        id: id.to_owned(),
        unit: unit.to_owned(),
        operating_range: OperatingRange {
            minimum: 0.0,
            maximum: 100.0,
        },
    }
}

/// The Identify section of the first-order model with delay `1.5·e^(−2s)/(4s + 1)`, as Identify writes it, from
/// `P1.ManipulatedValue` in % to `P1.ProcessValue` in °C (the signals of the commissioning example), each channel from 0
/// to 100, at the operating point 50 % and 50 °C. Without a validation error the fit is provisional, with the estimation
/// error 0.04; with one, it is validated on a second recording with that error.
pub(crate) fn identify_section(validation_error: Option<f64>) -> IdentifySection {
    let validated = validation_error.is_some();
    let mut evidence = vec!["sha256:estimation-samples".to_owned()];
    if validated {
        evidence.push("sha256:validation-samples".to_owned());
    }
    let model = GeneralModel::TransferFunction {
        identity: ModelIdentity {
            revision: "sha256:fopdt-model".to_owned(),
            source: ModelSource::Fitted,
            validation: if validated {
                ModelValidation::Validated
            } else {
                ModelValidation::Unvalidated
            },
            evidence,
            inputs: vec![channel("P1.ManipulatedValue", "%")],
            outputs: vec![channel("P1.ProcessValue", "°C")],
            delay_seconds: 2.0,
            operating_point: Some(OperatingPoint {
                inputs: vec![50.0],
                outputs: vec![50.0],
            }),
        },
        transfer: TransferFunction {
            numerator: vec![1.5],
            denominator: vec![4.0, 1.0],
        },
    };
    let fitted = IdentifiedModel {
        kind: LowOrderModelKind::FirstOrderDeadTime,
        gain: 1.5,
        time_constant_seconds: Some(4.0),
        second_time_constant_seconds: None,
        dead_time_seconds: Some(2.0),
        normalized_error: Some(validation_error.unwrap_or(ESTIMATION_ERROR)),
        normalized_estimation_error: Some(ESTIMATION_ERROR),
        normalized_validation_error: validation_error,
        estimation_residuals: None,
        validation_residuals: None,
        uncertainty: None,
        stable: true,
        on_search_boundary: false,
    };
    // The predicted response is what the presentation shows: two points at least, with increasing times (B3.8, A8.1), as the
    // Identify tests give it.
    let fit = ModelFitEvidence {
        model_structure: None,
        points: vec![
            ModelFitPoint {
                time_seconds: 0.0,
                measured: 50.0,
                predicted: 50.0,
                residual: 0.0,
            },
            ModelFitPoint {
                time_seconds: 4.0,
                measured: 52.0,
                predicted: 51.8,
                residual: 0.2,
            },
        ],
        root_mean_square_error: 0.0,
        output_unit: "°C".to_owned(),
        stable: true,
        final_change_fraction: None,
        delay_seconds: 2.0,
        operating_range: FittedRange {
            input_minimum: 50.0,
            input_maximum: 60.0,
            output_minimum: 50.0,
            output_maximum: 65.0,
        },
        warnings: Vec::new(),
        step_response: None,
        step_response_measurement: None,
    };
    let presentation = ComparisonPresentation {
        presentation: identified_model_presentation(&fitted, &fit, "%", "°C"),
        experiment: None,
        output_id: None,
        input_channels: None,
    };
    IdentifySection::new(
        IdentifyMethod::Live(ControlExperimentKind::MvStep),
        EvidenceSource::SimulatedPlant,
        IdentifySignals {
            inputs: vec!["P1.ManipulatedValue".to_owned()],
            outputs: vec!["P1.ProcessValue".to_owned()],
            setpoint: None,
        },
        ESTIMATION_RECORDING,
        validation_error.map(|_| VALIDATION_RECORDING),
        model,
        presentation,
    )
}

/// The tuning model of `numerator` over `denominator` with the dead time `delay`.
fn read(numerator: &[f64], denominator: &[f64], delay: f64) -> Result<IdentifiedModel, Refusal> {
    low_order_model(numerator, denominator, delay, Some(ERROR))
}

/// The current model of a fit: `section`, as the study keeps it.
fn fitted_current(section: IdentifySection) -> CurrentModel {
    CurrentModel::identified(section)
}

/// The tuning model of a stable form: its gain, time constants and dead time, and no fit evidence beyond its error.
fn expected(
    kind: LowOrderModelKind,
    gain: f64,
    time_constants: [Option<f64>; 2],
    delay: f64,
) -> IdentifiedModel {
    IdentifiedModel {
        kind,
        gain,
        time_constant_seconds: time_constants[0],
        second_time_constant_seconds: time_constants[1],
        dead_time_seconds: Some(delay),
        normalized_error: Some(ERROR),
        normalized_estimation_error: None,
        normalized_validation_error: None,
        estimation_residuals: None,
        validation_residuals: None,
        uncertainty: None,
        stable: true,
        on_search_boundary: false,
    }
}

#[test]
fn a_first_order_form_with_delay_reads_back_its_gain_time_constant_and_dead_time() {
    assert_eq!(
        read(&[1.5], &[4.0, 1.0], 2.0).expect("1.5·e^(−2s)/(4s + 1)"),
        expected(
            LowOrderModelKind::FirstOrderDeadTime,
            1.5,
            [Some(4.0), None],
            2.0
        )
    );
}

#[test]
fn a_first_order_form_without_delay_is_first_order() {
    assert_eq!(
        read(&[1.5], &[4.0, 1.0], 0.0).expect("1.5/(4s + 1)"),
        expected(LowOrderModelKind::FirstOrder, 1.5, [Some(4.0), None], 0.0)
    );
}

#[test]
fn an_integrator_reads_back_its_rate_gain() {
    assert_eq!(
        read(&[0.05], &[1.0, 0.0], 2.0).expect("0.05·e^(−2s)/s"),
        expected(LowOrderModelKind::Integrating, 0.05, [None, None], 2.0)
    );
}

#[test]
fn a_constant_denominator_is_a_static_gain() {
    assert_eq!(
        read(&[2.0], &[1.0], 0.0).expect("2"),
        expected(LowOrderModelKind::StaticGain, 2.0, [None, None], 0.0)
    );
}

#[test]
fn a_second_order_form_with_real_roots_reads_back_both_time_constants_the_larger_first() {
    // (3s + 1)(2s + 1) = 6s² + 5s + 1.
    let model = read(&[1.5], &[6.0, 5.0, 1.0], 0.0).expect("1.5/((3s + 1)(2s + 1))");
    assert_eq!(
        model,
        expected(
            LowOrderModelKind::SecondOrder,
            1.5,
            [Some(3.0), Some(2.0)],
            0.0
        )
    );
    // SIMC reduces a stable second-order model with real poles by its half rule (N5.2).
    assert!(matches!(
        from_identified(&model, TuningMethod::Simc),
        Ok(TuningPlant::SecondOrder(_))
    ));
    assert_eq!(
        read(&[1.5], &[6.0, 5.0, 1.0], 1.0)
            .expect("with a dead time")
            .kind,
        LowOrderModelKind::SecondOrderDeadTime
    );
}

#[test]
fn a_second_order_form_with_complex_roots_has_no_time_constants_and_amigo_refuses_it() {
    // s² + 0.2s + 1 has the roots −0.1 ± 0.995j: no real time constants.
    let model = read(&[1.0], &[1.0, 0.2, 1.0], 0.0).expect("still a second-order form");
    assert_eq!(
        model,
        expected(LowOrderModelKind::SecondOrder, 1.0, [None, None], 0.0)
    );
    assert_eq!(
        from_identified(&model, TuningMethod::Amigo)
            .expect_err("AMIGO takes first-order models only")
            .0,
        "AMIGO PI tuning requires a stable first-order model."
    );
}

#[test]
fn a_cubic_is_not_a_form_identify_writes() {
    assert_eq!(
        read(&[1.0], &[1.0, 2.0, 3.0, 4.0], 0.0)
            .expect_err("a cubic")
            .0,
        NOT_A_FORM
    );
}

#[test]
fn the_same_dynamics_in_another_scale_or_with_a_zero_are_not_forms_identify_writes() {
    // 3/(8s + 2) is 1.5/(4s + 1), but Identify writes the constant term one.
    assert_eq!(
        read(&[3.0], &[8.0, 2.0], 0.0).expect_err("scaled").0,
        NOT_A_FORM
    );
    assert_eq!(
        read(&[1.0], &[2.0, 0.0], 0.0)
            .expect_err("a scaled integrator")
            .0,
        NOT_A_FORM
    );
    assert_eq!(
        read(&[1.0, 0.5], &[4.0, 1.0], 0.0)
            .expect_err("a numerator with a zero")
            .0,
        NOT_A_FORM
    );
    // An integrator with a lag is read only with its `s` coefficient one (`1/(3s² + 2s)` is not `1/(3s² + s)` in that scale).
    assert_eq!(
        read(&[1.0], &[3.0, 2.0, 0.0], 0.0)
            .expect_err("an integrator with a lag in another scale")
            .0,
        NOT_A_FORM
    );
}

#[test]
fn an_integrator_with_a_lag_reads_back_its_rate_gain_and_lag() {
    // Lesson 90's axis `1/(s² + s)`: the rate gain 1 and the lag 1 (N5.12(c)). With a dead time the lag keeps its own delay.
    assert_eq!(
        read(&[1.0], &[1.0, 1.0, 0.0], 0.0).expect("1/(s² + s)"),
        expected(LowOrderModelKind::Integrating, 1.0, [Some(1.0), None], 0.0)
    );
    assert_eq!(
        read(&[2.0], &[0.5, 1.0, 0.0], 3.0).expect("2·e^(−3s)/(0.5s² + s)"),
        expected(LowOrderModelKind::Integrating, 2.0, [Some(0.5), None], 3.0)
    );
}

#[test]
fn a_repeated_time_constant_stays_real_when_its_discriminant_rounds_below_zero() {
    // The square of 0.1 rounds up, so the discriminant of (0.1s + 1)² rounds below zero; the others are checked as well.
    for tau in [0.1, 0.3, 3.0, 7.7] {
        let denominator = convolve(&[tau, 1.0], &[tau, 1.0]);
        let model = read(&[1.0], &denominator, 0.0).expect("a repeated time constant");
        for time_constant in [
            model.time_constant_seconds,
            model.second_time_constant_seconds,
        ] {
            let Some(time_constant) = time_constant else {
                panic!("the time constants of (τs + 1)² are real, for τ = {tau}");
            };
            assert!(
                (time_constant - tau).abs() <= 1e-7 * tau,
                "{time_constant} for τ = {tau}"
            );
        }
        assert!(model.stable);
    }
}

#[test]
fn a_provisional_model_is_tuned_on_its_estimation_error() {
    let section = identify_section(None);
    assert_eq!(
        tuning_model(&fitted_current(section)).expect("the study's model"),
        IdentifiedModel {
            normalized_error: Some(ESTIMATION_ERROR),
            ..expected(
                LowOrderModelKind::FirstOrderDeadTime,
                1.5,
                [Some(4.0), None],
                2.0
            )
        }
    );
}

#[test]
fn a_validated_model_is_tuned_on_its_validation_error() {
    let section = identify_section(Some(0.12));
    assert_eq!(
        tuning_model(&fitted_current(section)).expect("the study's model"),
        IdentifiedModel {
            normalized_error: Some(0.12),
            ..expected(
                LowOrderModelKind::FirstOrderDeadTime,
                1.5,
                [Some(4.0), None],
                2.0
            )
        }
    );
}

#[test]
fn a_model_without_its_deciding_error_is_refused() {
    let mut section = identify_section(Some(0.12));
    section
        .presentation
        .as_mut()
        .expect("a fit keeps its presentation")
        .presentation
        .validation
        .normalized_error = None;
    assert_eq!(
        tuning_model(&fitted_current(section))
            .expect_err("no validation error")
            .0,
        "The model's normalized error is not recorded, so the model cannot be tuned."
    );
}

/// The study's current model of a supplied continuous state-space model with no dead time (B3.7): the states and the matrices
/// as given, with no fit error, because a supplied model was not fitted.
fn supplied_state_space(states: Vec<ModelChannel>, system: StateSpace) -> CurrentModel {
    let section = identify_section(None);
    let identity = ModelIdentity {
        delay_seconds: 0.0,
        ..section.selected_model.identity().clone()
    };
    CurrentModel {
        source: CurrentModelSource::Supplied,
        model: GeneralModel::ContinuousStateSpace {
            identity,
            states,
            system,
        },
        identify: None,
    }
}

#[test]
fn a_supplied_state_space_model_is_read_as_its_transfer_function_in_the_form_identify_writes() {
    // `0.375/(s + 0.25)` is `1.5/(4s + 1)`: the monic conversion, scaled by its constant term (N4.7(c), N5.13).
    let current = supplied_state_space(
        vec![channel("x", "°C")],
        StateSpace {
            a: vec![vec![-0.25]],
            b: vec![vec![1.0]],
            c: vec![vec![0.375]],
            d: vec![vec![0.0]],
        },
    );
    assert_eq!(
        tuning_model(&current).expect("a first-order state-space model"),
        IdentifiedModel {
            normalized_error: None,
            ..expected(LowOrderModelKind::FirstOrder, 1.5, [Some(4.0), None], 0.0)
        }
    );
}

#[test]
fn lesson_90s_state_space_axis_is_the_integrator_with_a_lag() {
    // The shipped axis has J·ω′ = u − B·ω and θ′ = ω with J = 1 and B = 1, so θ/u = 1/(s² + s) (N4.7(c)): its tuning model is
    // the integrator with a lag of one second and rate gain one.
    let current = supplied_state_space(
        vec![channel("angle", "rad"), channel("velocity", "rad/s")],
        StateSpace {
            a: vec![vec![0.0, 1.0], vec![0.0, -1.0]],
            b: vec![vec![0.0], vec![1.0]],
            c: vec![vec![1.0, 0.0]],
            d: vec![vec![0.0]],
        },
    );
    assert_eq!(
        tuning_model(&current).expect("lesson 90's axis"),
        IdentifiedModel {
            normalized_error: None,
            ..expected(LowOrderModelKind::Integrating, 1.0, [Some(1.0), None], 0.0)
        }
    );
}

/// The study's current model of a supplied transfer function with no dead time (B3.7): the numerator and the denominator as given,
/// with no fit error, because a supplied model was not fitted.
fn supplied_transfer(numerator: Vec<f64>, denominator: Vec<f64>) -> CurrentModel {
    let section = identify_section(None);
    let identity = ModelIdentity {
        delay_seconds: 0.0,
        ..section.selected_model.identity().clone()
    };
    CurrentModel {
        source: CurrentModelSource::Supplied,
        model: GeneralModel::TransferFunction {
            identity,
            transfer: TransferFunction {
                numerator,
                denominator,
            },
        },
        identify: None,
    }
}

#[test]
fn equivalent_transfer_and_state_space_forms_give_one_tuning_model_and_so_one_pi_and_one_p_design()
{
    // control-studio-controller-law: "makes the same automatic choice for equivalent transfer and state-space models" and "shares
    // state-space dynamics across PI tuning, manual P and Routh analysis"; control-studio-simple-controller-design: "uses equivalent
    // continuous state-space dynamics without changing selected model identity" (N4.7(c), N5.13, specification 35 B4.2). The model
    // x′ = −x + 2u, y = 0.5x is 1/(s + 1), the transfer function itself, so everything that reads the tuning model reads one plant.
    use crate::tuning::formula::tune_identified;
    use crate::tuning::objective::TuningObjective;
    use crate::tuning::proportional::{first_order_design, GainBounds, ProportionalRequirements};
    use trust_control_math::identify::gain::ControllerAction;

    let transfer = supplied_transfer(vec![1.0], vec![1.0, 1.0]);
    let state_space = supplied_state_space(
        vec![channel("x", "m")],
        StateSpace {
            a: vec![vec![-1.0]],
            b: vec![vec![2.0]],
            c: vec![vec![0.5]],
            d: vec![vec![0.0]],
        },
    );
    let revision = state_space.model.identity().revision.clone();
    let requirements = ProportionalRequirements {
        reference_change: 1.0,
        constant_input_disturbance: 0.0,
        maximum_steady_error: 0.1,
        settling_band: 0.02,
        settling_time_seconds: 2.0,
        gain_bounds: GainBounds {
            minimum: -20.0,
            maximum: 20.0,
        },
    };
    for current in [&transfer, &state_space] {
        let model = tuning_model(current).expect("a first-order form in the shape Identify writes");
        assert_eq!(model.kind, LowOrderModelKind::FirstOrder);
        assert!((model.gain - 1.0).abs() <= 1e-12, "{}", model.gain);
        assert!(matches!(
            model.time_constant_seconds,
            Some(time) if (time - 1.0).abs() <= 1e-12
        ));
        // SIMC, balanced: λ = 2·max(L, T/3) = 2/3, so Kp = T/(K·(λ + L)) = 1.5.
        let pi = tune_identified(
            &model,
            TuningMethod::Simc,
            TuningObjective::Balanced,
            ControllerAction::Direct,
            "m/V",
        )
        .expect("SIMC tunes the first-order form");
        assert!(
            (pi.proportional_gain - 1.5).abs() <= 1e-6,
            "{}",
            pi.proportional_gain
        );
        // The simple P design: q = K·Kp = 1/0.1 − 1 = 9.
        let TuningPlant::FirstOrder(plant) =
            from_identified(&model, TuningMethod::Simc).expect("a plant the formulas read")
        else {
            panic!("a first-order plant");
        };
        let proportional = first_order_design(plant, 0.0, &requirements)
            .expect("a gain within the bounds meets the requirements");
        assert!(
            (proportional.proportional_gain - 9.0).abs() <= 1e-9,
            "{}",
            proportional.proportional_gain
        );
    }
    assert!(
        matches!(state_space.model, GeneralModel::ContinuousStateSpace { .. }),
        "reading the model as a transfer function leaves the selected model a state-space model"
    );
    assert_eq!(
        state_space.model.identity().revision,
        revision,
        "and keeps its identity"
    );
}

#[test]
fn a_manual_p_on_a_state_space_integrator_closes_on_the_pole_minus_rate_times_gain() {
    // control-studio-controller-law, "shares state-space dynamics across PI tuning, manual P and Routh analysis" (N5.8, specification 35
    // B4.2): the model x′ = 2u, y = 0.5x is the integrator 1/s with the rate gain 0.5·2 = 1, so an entered Kp of 2 closes it on the
    // pole −2, and the pole is the plant's own, whatever form the model was supplied in.
    use crate::tuning::proportional::{integrating_manual, GainBounds, ProportionalRequirements};

    let current = supplied_state_space(
        vec![channel("x", "m")],
        StateSpace {
            a: vec![vec![0.0]],
            b: vec![vec![2.0]],
            c: vec![vec![0.5]],
            d: vec![vec![0.0]],
        },
    );
    let model = tuning_model(&current).expect("an integrating form in the shape Identify writes");
    assert_eq!(model.kind, LowOrderModelKind::Integrating);
    let TuningPlant::Integrating(plant) =
        from_identified(&model, TuningMethod::Simc).expect("a plant the formulas read")
    else {
        panic!("a plain integrator");
    };
    let requirements = ProportionalRequirements {
        reference_change: 1.0,
        constant_input_disturbance: 0.0,
        maximum_steady_error: 0.1,
        settling_band: 0.02,
        settling_time_seconds: 2.0,
        gain_bounds: GainBounds {
            minimum: -10.0,
            maximum: 10.0,
        },
    };
    let manual = integrating_manual(plant, 2.0, 0.0, &requirements)
        .expect("an entered gain within the bounds is kept");
    assert!((manual.proportional_gain - 2.0).abs() <= 1e-12);
    assert!((manual.prediction.closed_loop_pole + 2.0).abs() <= 1e-12);
}
