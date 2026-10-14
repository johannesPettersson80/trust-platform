//! Tests of the requirement record Tune designs to (specification 35, B4.3; Tune page specification, "Control
//! requirements"): the test values Automatic fills for the commissioning example's loop and a first-order model with
//! delay, the method and the response of the request, and the values a previous Tune section keeps.

#![allow(clippy::float_cmp)] // Declared values are kept exactly, and the limits are copied exactly; both are compared exactly on purpose.

use trust_control_math::identify::LowOrderModelKind;

use super::*;
use crate::study::tune::model::tests::identify_section;
use crate::study::tune::model::tuning_model;
use crate::tuning::automatic_criteria::{AutomaticCriteria, REQUIREMENT_KEYS};

/// The loop of the commissioning example: a 100 ms task, the measurement `P1.ProcessValue` and its setpoint in °C from 0
/// to 100, and the output `P1.ManipulatedValue` in % from 0 to 100, with output limits 0 to 100 and rate limits of 25 %/s.
fn commissioning_loop() -> LoopDefinition {
    LoopDefinition::decode(include_str!(
        "../../../../../examples/control-pid-commissioning/control-loop.json"
    ))
    .expect("the commissioning example's loop")
}

/// The study's model `1.5·e^(−2s)/(4s + 1)` at the operating point 50 % and 50 °C, its provisional Identify section, and
/// its tuning model.
fn fopdt() -> (GeneralModel, IdentifySection, IdentifiedModel) {
    let identify = identify_section(None);
    let general = identify.selected_model.clone();
    let model = tuning_model(&general, &identify).expect("a first-order model with delay");
    (general, identify, model)
}

/// A request to tune the commissioning loop with the method and the response given.
fn request(method: Option<TuneMethod>, choice: Option<TuneChoice>) -> TuneRequest {
    TuneRequest {
        project_root: "/projects/control-pid-commissioning".to_owned(),
        runtime_id: "simulator".to_owned(),
        loop_id: "control-pid-commissioning".to_owned(),
        storage_folder: "/state/trust".to_owned(),
        method,
        choice,
        structure: None,
        pins: None,
        settings: None,
    }
}

/// The requirements of a first Tune of the commissioning loop, with no previous section.
fn first_requirements(
    loop_definition: &LoopDefinition,
    method: Option<TuneMethod>,
    choice: Option<TuneChoice>,
) -> Result<TuningRequirements, Refusal> {
    let (general, identify, model) = fopdt();
    requirements_for(
        loop_definition,
        &model,
        &general,
        &identify,
        None,
        None,
        &request(method, choice),
    )
}

/// A record the engineer completed for SIMC with the Faster response, a load trial and an overshoot limit, after Automatic
/// had generated its test change and duration from an earlier model.
fn declared_record() -> TuningRequirements {
    TuningRequirements {
        reference_change: 4.0,
        maximum_error: 0.3,
        settling_band: 0.1,
        settling_time_seconds: 30.0,
        duration_seconds: 70.0,
        constant_input_disturbance: Some(-2.0),
        measurement_noise_band: None,
        gain_uncertainty_fraction: None,
        transfer_tolerance: None,
        maximum_overshoot_percent: Some(10.0),
        tuning_method: Some(TuningMethod::Simc),
        tuning_objective: Some(TuningObjective::Fast),
        automatic_criteria: Some(AutomaticCriteria {
            model_revision: "sha256:earlier-model".to_owned(),
            fields: vec!["referenceChange".to_owned(), "durationSeconds".to_owned()],
        }),
    }
}

/// Whether `value` is `expected` within the rounding of the generated values.
fn close(value: f64, expected: f64) -> bool {
    (value - expected).abs() <= 1e-9
}

#[test]
fn without_a_previous_section_automatic_generates_every_test_value_inside_the_output_range() {
    let requirements =
        first_requirements(&commissioning_loop(), None, None).expect("Automatic fills the test");
    let criteria = requirements
        .automatic_criteria
        .as_ref()
        .expect("the generated values are marked");
    assert_eq!(criteria.model_revision, "sha256:fopdt-model");
    for key in REQUIREMENT_KEYS {
        assert!(
            criteria.fields.iter().any(|field| field == key),
            "{key} is marked Automatic"
        );
    }
    let target = 50.0 + requirements.reference_change;
    assert!(
        requirements.reference_change != 0.0 && (0.0..=100.0).contains(&target),
        "the test moves from 50 °C to {target} °C"
    );
    // A tenth of the 50 °C up to the range's top; a band of 2 % and an error of 4 % of the change; six timescales of
    // 4 s, the 2 s dead time and the 0.2 s the output takes to slew its 5 % headroom at 25 %/s; twice that and the
    // dead time again for the test (Tune page, "Control requirements").
    for (value, expected) in [
        (requirements.reference_change, 5.0),
        (requirements.settling_band, 0.1),
        (requirements.maximum_error, 0.2),
        (requirements.settling_time_seconds, 26.2),
        (requirements.duration_seconds, 56.4),
    ] {
        assert!(close(value, expected), "{value} is not {expected}");
    }
    assert_eq!(requirements.tuning_method, None);
    assert_eq!(requirements.tuning_objective, Some(TuningObjective::Robust));
}

#[test]
fn the_requests_method_is_the_method_of_the_record() {
    let requirements = first_requirements(&commissioning_loop(), Some(TuneMethod::Simc), None)
        .expect("SIMC's test");
    assert_eq!(requirements.tuning_method, Some(TuningMethod::Simc));
    let supplied = supplied_requirements(Some(&declared_record()), Some(TuningMethod::Amigo), None);
    assert_eq!(
        supplied.tuning_method,
        Some(TuningMethod::Amigo),
        "the request's method replaces the record's"
    );
}

/// A declared response replaces the one an earlier Tune generated (B4.3): the earlier record marks its More robust
/// response Automatic, and a request for Faster is the engineer's value, not a generated one to fill again.
#[test]
fn a_declared_response_replaces_the_one_an_earlier_tune_generated() {
    let loop_definition = commissioning_loop();
    let automatic = first_requirements(&loop_definition, None, None).expect("Automatic's test");
    assert_eq!(automatic.tuning_objective, Some(TuningObjective::Robust));
    let (general, identify, model) = fopdt();
    let mut previous = crate::study::tune::tests::valid_section(&general.identity().revision);
    previous.requirements = automatic;
    let faster = requirements_for(
        &loop_definition,
        &model,
        &general,
        &identify,
        Some(&previous),
        None,
        &request(Some(TuneMethod::Simc), Some(TuneChoice::Faster)),
    )
    .expect("SIMC's test");
    assert_eq!(faster.tuning_objective, Some(TuningObjective::Fast));
}

#[test]
fn the_requests_choice_replaces_a_declared_response() {
    let requirements = first_requirements(
        &commissioning_loop(),
        Some(TuneMethod::Simc),
        Some(TuneChoice::Faster),
    )
    .expect("SIMC's test");
    assert_eq!(requirements.tuning_objective, Some(TuningObjective::Fast));
    let fields = requirements
        .automatic_criteria
        .expect("the test values are generated")
        .fields;
    assert!(
        !fields.iter().any(|field| field == "tuningObjective"),
        "a declared response is not marked Automatic"
    );

    let declared = declared_record();
    let supplied = supplied_requirements(
        Some(&declared),
        Some(TuningMethod::Simc),
        Some(TuningObjective::Balanced),
    );
    assert_eq!(supplied.tuning_objective, Some(TuningObjective::Balanced));
    let (general, _, model) = fopdt();
    let evidence = fill_evidence(&commissioning_loop(), &model, &general).expect("the evidence");
    let filled = fill_requirements(&evidence, supplied, declared.automatic_criteria.as_ref())
        .expect("the record");
    assert_eq!(filled.tuning_objective, Some(TuningObjective::Balanced));
    // The declared values stay as they are; the generated ones follow the current model.
    assert_eq!(filled.maximum_error, 0.3);
    assert_eq!(filled.settling_band, 0.1);
    assert_eq!(filled.settling_time_seconds, 30.0);
    assert_eq!(filled.constant_input_disturbance, Some(-2.0));
    assert_eq!(filled.maximum_overshoot_percent, Some(10.0));
    assert!(close(filled.reference_change, 5.0));
    assert!(close(filled.duration_seconds, 64.0));
    assert_eq!(
        filled.automatic_criteria,
        Some(AutomaticCriteria {
            model_revision: "sha256:fopdt-model".to_owned(),
            fields: vec!["referenceChange".to_owned(), "durationSeconds".to_owned()],
        })
    );
}

#[test]
fn automatic_fills_more_robust_and_a_formula_method_keeps_the_declared_response() {
    let automatic = supplied_requirements(Some(&declared_record()), None, None);
    assert_eq!(automatic.tuning_method, None);
    assert_eq!(automatic.tuning_objective, None, "left to the policy");
    let formula = supplied_requirements(Some(&declared_record()), Some(TuningMethod::Simc), None);
    assert_eq!(formula.tuning_objective, Some(TuningObjective::Fast));

    let declared = declared_record();
    let (general, _, model) = fopdt();
    let evidence = fill_evidence(&commissioning_loop(), &model, &general).expect("the evidence");
    let filled = fill_requirements(&evidence, automatic, declared.automatic_criteria.as_ref())
        .expect("the record");
    assert_eq!(filled.tuning_objective, Some(TuningObjective::Robust));
    assert!(filled
        .automatic_criteria
        .expect("generated values are marked")
        .fields
        .iter()
        .any(|field| field == "tuningObjective"));
}

#[test]
fn the_evidence_comes_from_the_model_its_operating_point_and_the_loop() {
    let (general, _, model) = fopdt();
    let evidence = fill_evidence(&commissioning_loop(), &model, &general).expect("the evidence");
    assert_eq!(evidence.model_revision, "sha256:fopdt-model");
    assert_eq!(evidence.output_symbol, "P1.ProcessValue");
    assert_eq!(evidence.output_unit, "°C");
    assert_eq!(evidence.input_symbol, "P1.ManipulatedValue");
    assert!(close(evidence.period_seconds, 0.1));
    assert_eq!(evidence.delay_seconds, 2.0);
    // The 4 s time constant: as long as twice the 2 s dead time, and longer than ten task periods (1 s).
    assert_eq!(evidence.timescale_seconds, 4.0);
    assert_eq!(
        (evidence.operating_input, evidence.operating_output),
        (50.0, 50.0)
    );
    assert_eq!(
        (evidence.output_minimum, evidence.output_maximum),
        (0.0, 100.0)
    );
    assert_eq!(
        (evidence.input_minimum, evidence.input_maximum),
        (0.0, 100.0)
    );
    assert_eq!(
        (evidence.input_rate_up_per_s, evidence.input_rate_down_per_s),
        (25.0, 25.0)
    );
    // At 0.001/4 rad/s the response is the static gain 1.5, with a phase of a fraction of a degree.
    assert!((evidence.low_frequency_real - 1.5).abs() <= 1e-5);
    assert!((evidence.low_frequency_magnitude - 1.5).abs() <= 1e-5);
    assert_eq!(evidence.baseline_noise, 0.0);
}

#[test]
fn the_test_ranges_are_the_signal_ranges_within_the_model_and_the_declared_limits() {
    let mut loop_definition = commissioning_loop();
    loop_definition.limits.output_maximum = Some(80.0);
    let Some(setpoint) = loop_definition.signals.setpoint.as_mut() else {
        panic!("the commissioning example declares a setpoint");
    };
    setpoint.range.maximum = 60.0;
    let (general, _, model) = fopdt();
    let bounded = fill_evidence(&loop_definition, &model, &general).expect("the evidence");
    assert_eq!((bounded.input_minimum, bounded.input_maximum), (0.0, 80.0));
    assert_eq!(
        (bounded.output_minimum, bounded.output_maximum),
        (0.0, 60.0)
    );
    loop_definition.limits.output_maximum = None;
    let open = fill_evidence(&loop_definition, &model, &general).expect("the evidence");
    assert_eq!(
        open.input_maximum, 100.0,
        "an output limit the loop does not declare bounds nothing"
    );
}

#[test]
fn an_undeclared_rate_limit_keeps_the_headroom_and_adds_no_slew_time() {
    let mut loop_definition = commissioning_loop();
    loop_definition.limits.rate_up_per_s = None;
    loop_definition.limits.rate_down_per_s = None;
    let requirements =
        first_requirements(&loop_definition, None, None).expect("a test without rate limits");
    assert!(close(requirements.reference_change, 5.0));
    // Six timescales of 4 s and the 2 s dead time, with no slew allowance.
    assert!(close(requirements.settling_time_seconds, 26.0));
}

#[test]
fn an_integrators_timescale_has_no_time_of_its_pole() {
    let integrating = IdentifiedModel {
        kind: LowOrderModelKind::Integrating,
        gain: 0.05,
        time_constant_seconds: None,
        second_time_constant_seconds: None,
        dead_time_seconds: Some(2.0),
        normalized_error: 0.04,
        normalized_estimation_error: None,
        normalized_validation_error: None,
        estimation_residuals: None,
        validation_residuals: None,
        uncertainty: None,
        stable: true,
        on_search_boundary: false,
    };
    let (general, _, _) = fopdt();
    let evidence =
        fill_evidence(&commissioning_loop(), &integrating, &general).expect("the evidence");
    // Twice the dead time is longer than ten task periods; the integrator's pole at zero gives no time.
    assert_eq!(evidence.timescale_seconds, 4.0);
    // |K′/(jω)| at ω = 0.001/4 rad/s is 0.05/0.00025 = 200.
    assert!((evidence.low_frequency_magnitude - 200.0).abs() <= 1e-9 * 200.0);
}

#[test]
fn a_model_without_a_finite_operating_point_is_refused_with_the_policys_sentence() {
    let (mut general, identify, model) = fopdt();
    let GeneralModel::TransferFunction { identity, .. } = &mut general else {
        panic!("the study's model is a transfer function");
    };
    identity.operating_point = None;
    let refuse = |general: &GeneralModel| {
        requirements_for(
            &commissioning_loop(),
            &model,
            general,
            &identify,
            None,
            None,
            &request(None, None),
        )
        .expect_err("no operating point")
        .0
    };
    let sentence = "The model needs its operating point before automatic tuning setup.";
    assert_eq!(refuse(&general), sentence);
    let GeneralModel::TransferFunction { identity, .. } = &mut general else {
        panic!("the study's model is a transfer function");
    };
    identity.operating_point = Some(OperatingPoint {
        inputs: vec![f64::NAN],
        outputs: vec![50.0],
    });
    assert_eq!(refuse(&general), sentence);
}

#[test]
fn a_model_with_two_inputs_needs_a_channel_mapping() {
    let (mut general, identify, model) = fopdt();
    let GeneralModel::TransferFunction { identity, .. } = &mut general else {
        panic!("the study's model is a transfer function");
    };
    let mut second = identity.inputs[0].clone();
    second.id = "P1.FeedForward".to_owned();
    identity.inputs.push(second);
    assert_eq!(
        requirements_for(
            &commissioning_loop(),
            &model,
            &general,
            &identify,
            None,
            None,
            &request(None, None),
        )
        .expect_err("two inputs")
        .0,
        "Map every model input and output in loop channel bindings."
    );
}

#[test]
fn the_requirements_the_study_holds_are_taken_as_given_over_an_earlier_generated_value() {
    let (general, identify, model) = fopdt();
    let held = TuningRequirements {
        reference_change: 2.0,
        maximum_error: 0.4,
        settling_band: 0.1,
        settling_time_seconds: 45.0,
        duration_seconds: 90.0,
        constant_input_disturbance: None,
        measurement_noise_band: None,
        gain_uncertainty_fraction: None,
        transfer_tolerance: None,
        maximum_overshoot_percent: None,
        tuning_method: None,
        tuning_objective: None,
        automatic_criteria: None,
    };
    // An earlier Tune generated the test change, so Tune would fill it again; the value the engineer holds is kept instead (B4.3).
    let mut previous = crate::study::tune::tests::valid_section(&general.identity().revision);
    previous.requirements.automatic_criteria = Some(AutomaticCriteria {
        model_revision: general.identity().revision.clone(),
        fields: vec!["referenceChange".to_owned()],
    });
    let given = requirements_for(
        &commissioning_loop(),
        &model,
        &general,
        &identify,
        Some(&previous),
        Some(&held),
        &request(Some(TuneMethod::Simc), None),
    )
    .expect("the study's requirements");
    assert_eq!(
        given.reference_change, 2.0,
        "the held value is kept as given"
    );
    assert_eq!(given.duration_seconds, 90.0);
    assert_eq!(
        given.automatic_criteria, None,
        "no value of the record is generated"
    );
}
