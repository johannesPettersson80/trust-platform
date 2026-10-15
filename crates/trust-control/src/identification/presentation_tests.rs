#![allow(clippy::float_cmp)] // Input changes, dead times and poles are exact, compared exactly on purpose.

use serde_json::json;
use trust_control_math::identify::ParameterInterval;

use super::*;
use crate::experiment::SampleQuality;
use crate::identification::data::tests::fopdt_samples;
use crate::identification::data::{DataSource, IdentificationSample};
use crate::identification::evidence::model_fit_evidence;
use crate::identification::evidence::tests::{fitted_model, open_loop};
use crate::identification::plant::identify_plant;
use crate::identification::step_response::{
    analyze_step_response, measure_step_response, SearchGrid, StepResponseSettings,
};

/// The fit of the persisted model-bound comparison of the engineer-study tests.
fn comparison_fit() -> ModelFitEvidence {
    ModelFitEvidence {
        model_structure: None,
        points: vec![
            ModelFitPoint {
                time_seconds: 0.0,
                measured: 0.0,
                predicted: 0.0,
                residual: 0.0,
            },
            ModelFitPoint {
                time_seconds: 2.0,
                measured: 1.0,
                predicted: 0.9,
                residual: 0.1,
            },
        ],
        root_mean_square_error: 0.1,
        output_unit: "m".to_string(),
        stable: true,
        final_change_fraction: None,
        delay_seconds: 0.0,
        operating_range: OperatingRange {
            input_minimum: 0.0,
            input_maximum: 1.0,
            output_minimum: 0.0,
            output_maximum: 1.0,
        },
        warnings: Vec::new(),
        step_response: None,
        step_response_measurement: None,
    }
}

/// The bounded-step settings of the step-response tests (`settings`).
fn step_settings() -> StepResponseSettings {
    StepResponseSettings {
        baseline_duration_seconds: 5.0,
        final_duration_seconds: 5.0,
        maximum_window_range: 0.1,
        maximum_window_slope_per_second: 0.01,
        measurement_bound: 0.05,
        onset_threshold: 0.15,
        onset_samples: 3,
        delay: SearchGrid {
            minimum: 0.0,
            maximum: 2.0,
            step: 0.1,
        },
        time_constant: SearchGrid {
            minimum: 0.1,
            maximum: 20.0,
            step: 0.1,
        },
        baseline_step: 0.01,
        gain_step: 0.001,
        timestamp_shift_seconds: 0.0,
        sensor_time_constant: None,
        model_adequacy: None,
    }
}

/// The bounded step of the step-response tests (`record`): 452 samples 0.1 s apart from −5 s, the input stepped from
/// 75 % by `change` between 0 and 40 s, and from 35 °C a first-order response of gain 0.2 and time constant 5 s after
/// a delay of 1 s.
fn step_record(change: f64) -> Vec<IdentificationSample> {
    (0..452_u32)
        .map(|index| {
            let time = f64::from(index) / 10.0 - 5.0;
            let input = if (0.0..40.0).contains(&time) {
                75.0 + change
            } else {
                75.0
            };
            let output = if time <= 1.0 {
                35.0
            } else {
                35.0 + 0.2 * change * (1.0 - (-(time - 1.0) / 5.0).exp())
            };
            IdentificationSample {
                time_seconds: time,
                input,
                output,
                quality: SampleQuality::Good,
                input_unit: Some("%".to_string()),
                output_unit: Some("degC".to_string()),
                clipped: None,
                saturated: None,
            }
        })
        .collect()
}

// control-studio-identify-tune.test.ts: "presents an identified model with engineering and residual evidence".
#[test]
fn presents_an_identified_model_with_engineering_and_residual_evidence() {
    let validation = fopdt_samples(1.25);
    let result = identify_plant(
        &fopdt_samples(2.0),
        &[],
        DataSource::Simulation,
        Some(validation.as_slice()),
        0.02,
        &open_loop(),
    )
    .expect("the independently validated step identifies a model");
    let model = result.recommended;
    let fit = model_fit_evidence(&model, &fopdt_samples(1.25))
        .expect("the validation record has a measured step");
    let evidence = identified_model_presentation(&model, &fit, "kW", "°C");
    assert_eq!(
        evidence
            .equation
            .strip_prefix("G(s) = ")
            .map(|rest| rest.contains("/ (")),
        Some(true),
        "{}",
        evidence.equation
    );
    assert_eq!(
        evidence.units,
        PresentationUnits {
            input: "kW".to_string(),
            output: "°C".to_string(),
            gain: "°C/kW".to_string(),
            time: "s".to_string(),
        }
    );
    assert_eq!(evidence.poles.len(), 1);
    assert_eq!(evidence.zeros, [] as [f64; 0]);
    // Specification N3.3: the resolved delay of this record is the recommended form's dead time (about 0.3 s), so the
    // presentation reports a delay where round 1 expected none.
    assert!(evidence.delay_seconds > 0.0, "{}", evidence.delay_seconds);
    assert!(
        evidence
            .uncertainty
            .label
            .to_lowercase()
            .contains("search-resolution and held-out-residual"),
        "{}",
        evidence.uncertainty.label
    );
    assert!(evidence
        .estimation
        .residual_lag_one_correlation
        .is_some_and(f64::is_finite));
    assert!(evidence
        .validation
        .residual_lag_one_correlation
        .is_some_and(f64::is_finite));
    assert_eq!(evidence.predicted_response.len(), fit.points.len());
    assert_eq!(evidence.validity_range, fit.operating_range);
    let compact = identified_model_presentation(
        &IdentifiedModel {
            gain: 1.0 / 3.0,
            time_constant_seconds: Some(2.0 / 3.0),
            dead_time_seconds: Some(1.0 / 7.0),
            ..model
        },
        &fit,
        "kW",
        "°C",
    );
    for repeated in ["333333333", "666666666", "142857142"] {
        assert!(
            !compact.equation.contains(repeated),
            "model equations must use readable engineering precision: {}",
            compact.equation
        );
    }
}

// control-studio-engineer-study-persistence.test.ts: "retains the model-bound measured comparison through study
// restore". The study decoder is not part of this module: the presentation the study keeps is checked here field for
// field, and it reads back unchanged.
#[test]
fn the_presentation_of_a_model_bound_comparison_keeps_its_fields_through_json() {
    let model = IdentifiedModel {
        time_constant_seconds: Some(1.0),
        normalized_error: Some(0.1),
        ..fitted_model(LowOrderModelKind::FirstOrder, 1.0)
    };
    let presentation = identified_model_presentation(&model, &comparison_fit(), "V", "m");
    let value = serde_json::to_value(&presentation).expect("the presentation serializes");
    assert_eq!(
        value,
        json!({
            "equation": "G(s) = 1 / (1s + 1)",
            "units": { "input": "V", "output": "m", "gain": "m/V", "time": "s" },
            "parameters": [
                { "name": "Gain", "value": 1.0, "unit": "m/V" },
                { "name": "Time constant 1", "value": 1.0, "unit": "s" },
                { "name": "Dead time", "value": 0.0, "unit": "s" }
            ],
            "poles": [-1.0],
            "zeros": [],
            "delaySeconds": 0.0,
            "uncertainty": { "label": "Conservative search-resolution and held-out-residual interval" },
            "estimation": { "normalizedError": 0.1 },
            "validation": {},
            "predictedResponse": [
                { "timeSeconds": 0.0, "measured": 0.0, "predicted": 0.0, "residual": 0.0 },
                { "timeSeconds": 2.0, "measured": 1.0, "predicted": 0.9, "residual": 0.1 }
            ],
            "validityRange": {
                "inputMinimum": 0.0,
                "inputMaximum": 1.0,
                "outputMinimum": 0.0,
                "outputMaximum": 1.0
            },
            "warnings": []
        })
    );
    let restored: IdentifiedModelPresentation =
        serde_json::from_value(value).expect("the presentation reads back");
    assert_eq!(restored, presentation);
}

// control-studio-step-response.test.ts: "validation chart references describe the plotted validation record while
// preserving the estimation grid". The `TypeScript` test reaches the review through the workflow; here the review's fit
// is built from the same estimation analysis and validation measurement the bounded step review puts in it.
#[test]
fn validation_chart_references_describe_the_validation_record_and_keep_the_estimation_grid() {
    let settings = step_settings();
    let analysis = analyze_step_response(&step_record(10.0), &settings, 0.15)
        .expect("the estimation step is analysed");
    let measurement = measure_step_response(&step_record(12.0), &settings)
        .expect("the validation step is measured");
    let model = IdentifiedModel {
        time_constant_seconds: Some(analysis.best.time_constant_seconds),
        dead_time_seconds: Some(analysis.best.delay_seconds),
        ..fitted_model(LowOrderModelKind::FirstOrderDeadTime, analysis.best.gain)
    };
    let fit = ModelFitEvidence {
        step_response: Some(analysis.clone()),
        step_response_measurement: Some(measurement),
        ..comparison_fit()
    };
    let presentation = identified_model_presentation(&model, &fit, "%", "degC");
    let Some(StepResponseView::Measurement(plotted)) = &presentation.step_response else {
        panic!(
            "the validation measurement is plotted: {:?}",
            presentation.step_response
        );
    };
    assert_eq!(
        plotted.input_change, 12.0,
        "the plotted validation trace must use its own measurement reference levels"
    );
    assert_eq!(
        fit.step_response
            .as_ref()
            .map(|estimated| estimated.input_change),
        Some(10.0),
        "the frozen estimation grid keeps its original source"
    );
    // The rule's other two cases: without a measurement the analysis is plotted, and without an analysis nothing is.
    let estimation_only = ModelFitEvidence {
        step_response_measurement: None,
        ..fit.clone()
    };
    let estimated = identified_model_presentation(&model, &estimation_only, "%", "degC");
    assert_eq!(
        estimated.step_response,
        Some(StepResponseView::Analysis(Box::new(analysis)))
    );
    let measurement_only = ModelFitEvidence {
        step_response: None,
        ..fit
    };
    assert_eq!(
        identified_model_presentation(&model, &measurement_only, "%", "degC").step_response,
        None
    );
    // Each plotted step response reads back as what it is.
    for view in [&presentation.step_response, &estimated.step_response] {
        let value = serde_json::to_value(view).expect("the step response serializes");
        let restored: Option<StepResponseView> =
            serde_json::from_value(value).expect("the step response reads back");
        assert_eq!(&restored, view);
    }
}

#[test]
fn the_equation_poles_and_parameters_follow_the_model_form() {
    let integrating = IdentifiedModel {
        dead_time_seconds: Some(2.0),
        ..fitted_model(LowOrderModelKind::Integrating, 0.0004)
    };
    assert_eq!(
        identified_model_equation(&integrating),
        "G(s) = 0.0004 e^(-2s) / s"
    );
    assert_eq!(
        identified_model_equation(&fitted_model(LowOrderModelKind::Integrating, 0.0004)),
        "G(s) = 0.0004 / s"
    );
    assert_eq!(
        identified_model_equation(&fitted_model(LowOrderModelKind::StaticGain, 2.0)),
        "G(s) = 2"
    );
    let compact = IdentifiedModel {
        time_constant_seconds: Some(2.0 / 3.0),
        dead_time_seconds: Some(1.0 / 7.0),
        ..fitted_model(LowOrderModelKind::FirstOrderDeadTime, 1.0 / 3.0)
    };
    assert_eq!(
        identified_model_equation(&compact),
        "G(s) = 0.333333 e^(-0.142857s) / (0.666667s + 1)"
    );
    let second_order = IdentifiedModel {
        time_constant_seconds: Some(4.0),
        second_time_constant_seconds: Some(0.5),
        dead_time_seconds: Some(0.3),
        ..fitted_model(LowOrderModelKind::SecondOrderDeadTime, 1.5)
    };
    assert_eq!(
        identified_model_equation(&second_order),
        "G(s) = 1.5 e^(-0.3s) / (4s + 1)(0.5s + 1)"
    );
    let presentation = identified_model_presentation(&second_order, &comparison_fit(), "kW", "°C");
    assert_eq!(presentation.poles, [-0.25, -2.0]);
    let names: Vec<&str> = presentation
        .parameters
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["Gain", "Time constant 1", "Time constant 2", "Dead time"]
    );
    let units: Vec<&str> = presentation
        .parameters
        .iter()
        .map(|parameter| parameter.unit.as_str())
        .collect();
    assert_eq!(units, ["°C/kW", "s", "s", "s"]);
    assert_eq!(presentation.delay_seconds, 0.3);
    assert_eq!(
        identified_model_presentation(&integrating, &comparison_fit(), "%", "m").poles,
        [0.0]
    );
}

#[test]
fn the_uncertainty_label_names_what_the_intervals_are() {
    let intervals = |method: UncertaintyMethod| IdentifiedUncertainty {
        method,
        gain: ParameterInterval {
            minimum: 0.9,
            maximum: 1.1,
        },
        time_constant_seconds: None,
        second_time_constant_seconds: None,
        dead_time_seconds: None,
    };
    let presented = |uncertainty: Option<IdentifiedUncertainty>| {
        let model = IdentifiedModel {
            uncertainty,
            ..fitted_model(LowOrderModelKind::StaticGain, 1.0)
        };
        identified_model_presentation(&model, &comparison_fit(), "%", "m").uncertainty
    };
    let grid = presented(Some(intervals(
        UncertaintyMethod::MeasurementConsistentGrid,
    )));
    assert_eq!(
        grid.label,
        "Measurement-consistent parameter grid; marginal ranges, not statistical confidence"
    );
    assert_eq!(
        grid.intervals,
        Some(intervals(UncertaintyMethod::MeasurementConsistentGrid))
    );
    assert_eq!(
        presented(Some(intervals(UncertaintyMethod::SampleResolutionEnvelope))).label,
        "Sample-resolution envelope from the estimation record; not statistical confidence"
    );
    assert_eq!(
        presented(Some(intervals(
            UncertaintyMethod::GridResolutionAndValidationResidual
        )))
        .label,
        "Conservative search-resolution and held-out-residual interval"
    );
    let without = presented(None);
    assert_eq!(
        without.label,
        "Conservative search-resolution and held-out-residual interval"
    );
    assert_eq!(without.intervals, None);
}

/// The presentation of `model` with the comparison fit, from an input in `input_unit` to an output in `output_unit`.
fn presented(
    model: &IdentifiedModel,
    input_unit: &str,
    output_unit: &str,
) -> IdentifiedModelPresentation {
    identified_model_presentation(model, &comparison_fit(), input_unit, output_unit)
}

/// A model of `kind` with `gain`, its time constants, its dead time and a normalized error of 0.021.
fn model_of(
    kind: LowOrderModelKind,
    gain: f64,
    time_constants: [Option<f64>; 2],
    dead_time: Option<f64>,
) -> IdentifiedModel {
    IdentifiedModel {
        time_constant_seconds: time_constants[0],
        second_time_constant_seconds: time_constants[1],
        dead_time_seconds: dead_time,
        normalized_error: Some(0.021),
        ..fitted_model(kind, gain)
    }
}

// Specification 35, B3.8; Identify page specification, "Identified model and plot": the model type in plain words, read
// from the stored presentation, with " with delay" for a dead time above zero.
#[test]
fn the_model_type_names_each_form_and_whether_it_has_a_delay() {
    let cases = [
        (
            LowOrderModelKind::StaticGain,
            [None, None],
            None,
            "Static gain model",
        ),
        (
            LowOrderModelKind::FirstOrder,
            [Some(5.0), None],
            Some(0.0),
            "First-order model",
        ),
        (
            LowOrderModelKind::FirstOrderDeadTime,
            [Some(5.0), None],
            Some(1.0),
            "First-order model with delay",
        ),
        (
            LowOrderModelKind::SecondOrder,
            [Some(4.0), Some(0.5)],
            Some(0.0),
            "Second-order model",
        ),
        (
            LowOrderModelKind::SecondOrderDeadTime,
            [Some(4.0), Some(0.5)],
            Some(0.3),
            "Second-order model with delay",
        ),
        (
            LowOrderModelKind::Integrating,
            [None, None],
            None,
            "Integrating model",
        ),
        (
            LowOrderModelKind::Integrating,
            [None, None],
            Some(2.0),
            "Integrating model with delay",
        ),
    ];
    for (kind, time_constants, dead_time, expected) in cases {
        let presentation = presented(&model_of(kind, 0.5, time_constants, dead_time), "%", "°C");
        assert_eq!(
            identified_model_type_text(&presentation),
            expected,
            "{kind:?}"
        );
    }
}

#[test]
fn the_key_numbers_give_the_gain_per_unit_and_each_duration_in_its_unit() {
    let first_order = model_of(
        LowOrderModelKind::FirstOrderDeadTime,
        0.21,
        [Some(75.0), None],
        Some(12.0),
    );
    assert_eq!(
        identified_model_key_numbers(&presented(&first_order, "%", "°C")),
        "Gain 0.21 °C per %, time constant 75 s, delay 12 s"
    );
    // Six significant digits; a duration of two minutes or more is in min (4500 s is 75 min); the two time constants are in
    // order; no delay part without a dead time.
    let second_order = model_of(
        LowOrderModelKind::SecondOrder,
        1.23456,
        [Some(4500.0), Some(0.123_456)],
        Some(0.0),
    );
    assert_eq!(
        identified_model_key_numbers(&presented(&second_order, "kW", "°C")),
        "Gain 1.23456 °C per kW, time constants 75 min and 0.123456 s"
    );
    let static_gain = model_of(LowOrderModelKind::StaticGain, -2.5, [None, None], None);
    assert_eq!(
        identified_model_key_numbers(&presented(&static_gain, "V", "m")),
        "Gain -2.5 m per V"
    );
    // The line has no fit error, with or without an estimation error (Identify page item 6).
    let mut without_error = presented(&first_order, "%", "°C");
    without_error.estimation.normalized_error = None;
    assert_eq!(
        identified_model_key_numbers(&without_error),
        "Gain 0.21 °C per %, time constant 75 s, delay 12 s"
    );
}

#[test]
fn an_integrator_gives_its_rate_gain_per_second() {
    let integrating = model_of(
        LowOrderModelKind::Integrating,
        0.0004,
        [None, None],
        Some(2.0),
    );
    assert_eq!(
        identified_model_key_numbers(&presented(&integrating, "%", "m")),
        "Rate gain 0.0004 m per % per s, delay 2 s"
    );
    assert_eq!(
        identified_model_key_numbers(&presented(&integrating, "", "")),
        "Rate gain 0.0004 per s, delay 2 s",
        "a gain dimensionless on both sides has no unit"
    );
}

#[test]
fn an_empty_or_unit_one_side_of_the_gain_reads_as_one() {
    let model = model_of(
        LowOrderModelKind::FirstOrder,
        0.21,
        [Some(75.0), None],
        Some(0.0),
    );
    let gain_of = |input_unit: &str, output_unit: &str| {
        let line = identified_model_key_numbers(&presented(&model, input_unit, output_unit));
        line.split(", ").next().unwrap_or_default().to_owned()
    };
    assert_eq!(gain_of("", "°C"), "Gain 0.21 °C per 1");
    assert_eq!(gain_of("%", " "), "Gain 0.21 1 per %");
    assert_eq!(gain_of("1", "°C"), "Gain 0.21 °C per 1");
    assert_eq!(gain_of("", "1"), "Gain 0.21", "dimensionless on both sides");
}

#[test]
fn a_presentation_with_more_time_constants_names_its_order_and_lists_them() {
    let mut presentation = presented(
        &model_of(
            LowOrderModelKind::SecondOrderDeadTime,
            1.5,
            [Some(4.0), Some(0.5)],
            Some(0.3),
        ),
        "kW",
        "°C",
    );
    presentation.parameters.insert(
        3,
        PresentationParameter {
            name: "Time constant 3".to_string(),
            value: 0.25,
            unit: "s".to_string(),
        },
    );
    assert_eq!(
        identified_model_type_text(&presentation),
        "3-order model with delay"
    );
    assert_eq!(
        identified_model_key_numbers(&presentation),
        "Gain 1.5 °C per kW, time constants 4 s, 0.5 s and 0.25 s, delay 0.3 s"
    );
}
