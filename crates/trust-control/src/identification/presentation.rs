//! The presentation of an identified model (specification 35, B3.8; Identify page specification, "Identified model and
//! plot"): its equation with the identified coefficients, its units, parameters, poles and zeros, the label of its
//! uncertainty, its residual evidence, and the measured response with the prediction (`identifiedModelPresentation` in
//! `identification.ts`). The presentation claims no statistical confidence the fit has not established.
//!
//! The readable parts of a result are read from a stored presentation: the model type in plain words
//! ([`identified_model_type_text`]) and one line of key numbers ([`identified_model_key_numbers`]). The `TypeScript` page
//! composed both from the general model (`threePageModel.ts`); that copy goes when the page shows these.
//!
//! The `TypeScript` presentation also carries a model-structure comparison (parked with the sensor-lag comparison) and
//! the experiment, channel, output-fit and pole-zero fields that the presentation files of the extension add; none of
//! them is decided here.

#![allow(clippy::module_name_repetitions)] // The presentation's records are named as the `TypeScript` names them.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::identify::LowOrderModelKind;
use trust_control_math::sampling::statistics::lag_one_correlation;

use crate::experiment::number_text::{js_number, js_to_precision};
use crate::identification::evidence::{ModelFitEvidence, ModelFitPoint, OperatingRange};
use crate::identification::plant::{IdentifiedModel, IdentifiedUncertainty, UncertaintyMethod};
use crate::identification::step_response::{StepResponseAnalysis, StepResponseMeasurement};

/// The unit of every time parameter.
const SECONDS: &str = "s";

/// The name of the gain parameter.
const GAIN_PARAMETER: &str = "Gain";

/// The name of a time-constant parameter, before its number.
const TIME_CONSTANT_PARAMETER: &str = "Time constant";

/// The uncertainty label of a measurement-consistent parameter grid.
const MEASUREMENT_CONSISTENT_LABEL: &str =
    "Measurement-consistent parameter grid; marginal ranges, not statistical confidence";

/// The uncertainty label of a single-run sample-resolution envelope.
const SAMPLE_RESOLUTION_LABEL: &str =
    "Sample-resolution envelope from the estimation record; not statistical confidence";

/// The uncertainty label of a search-resolution and held-out-residual interval, and of a model without intervals.
const SEARCH_RESOLUTION_LABEL: &str =
    "Conservative search-resolution and held-out-residual interval";

/// The units of a presented model (`units`): the input's, the output's, the gain's `output/input`, and `s` for time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PresentationUnits {
    /// The input's unit.
    pub input: String,
    /// The output's unit.
    pub output: String,
    /// The gain's unit, `output/input`.
    pub gain: String,
    /// The unit of time, `s`.
    pub time: String,
}

/// One identified parameter with its value and unit (`parameters`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PresentationParameter {
    /// The parameter's name: `Gain`, `Time constant 1`, `Time constant 2` or `Dead time`.
    pub name: String,
    /// The identified value.
    pub value: f64,
    /// The value's unit.
    pub unit: String,
}

/// The uncertainty of a presented model (`uncertainty`): the label that says what its intervals are, and the intervals
/// when the fit has them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentationUncertainty {
    /// What the intervals are; never a statistical confidence the fit has not established.
    pub label: String,
    /// The parameter intervals of the fit, when it has them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intervals: Option<IdentifiedUncertainty>,
}

/// The error evidence of one record (`estimation`, `validation`): the normalized error and the lag-one correlation of
/// the residuals, each when the fit has it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PresentationError {
    /// The normalized error on the record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_error: Option<f64>,
    /// The lag-one correlation of the residuals on the record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub residual_lag_one_correlation: Option<f64>,
}

/// The step response a presented model plots (`stepResponse`): the measurement of the plotted record when the fit has
/// one, otherwise the step-response analysis of the estimation record. Both are written as their own record. When one
/// is read, the analysis is tried first: a measurement lacks the analysis's fit, while an analysis holds every field
/// of a measurement and would otherwise be read as one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StepResponseView {
    /// The step-response analysis of the estimation record.
    Analysis(Box<StepResponseAnalysis>),
    /// The measurement of the plotted record, such as an independent validation step.
    Measurement(Box<StepResponseMeasurement>),
}

/// An identified model as the engineer reviews it (B3.8; `IdentifiedModelPresentation`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentifiedModelPresentation {
    /// The plant-model equation with the identified coefficients.
    pub equation: String,
    /// The units of the input, the output, the gain and time.
    pub units: PresentationUnits,
    /// The gain, each time constant and the dead time, in that order.
    pub parameters: Vec<PresentationParameter>,
    /// The poles: zero for an integrator, otherwise `−1/τ` for each time constant.
    pub poles: Vec<f64>,
    /// The zeros; the low-order forms have none.
    pub zeros: Vec<f64>,
    /// The dead time, in s; zero without one.
    pub delay_seconds: f64,
    /// The uncertainty label and intervals.
    pub uncertainty: PresentationUncertainty,
    /// The error evidence on the estimation record.
    pub estimation: PresentationError,
    /// The error evidence on the validation record.
    pub validation: PresentationError,
    /// The measured response with the prediction.
    pub predicted_response: Vec<ModelFitPoint>,
    /// The step response to plot, when the fit came from a bounded step review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_response: Option<StepResponseView>,
    /// The range the model is evidence for.
    pub validity_range: OperatingRange,
    /// The warnings of the fit evidence.
    pub warnings: Vec<String>,
}

/// The plant-model equation of a model with its coefficients to six significant digits (B3.8;
/// `identifiedModelEquation`): `G(s) = K e^(-Ls) / s` for an integrator, `G(s) = K e^(-Ls) / (τ₁s + 1)(τ₂s + 1)` with
/// one factor per time constant, and `G(s) = K` without time constants. The delay term appears only for a dead time
/// above zero.
#[must_use]
pub fn identified_model_equation(model: &IdentifiedModel) -> String {
    let gain = engineering_coefficient(model.gain);
    let delay = model.dead_time_seconds.unwrap_or(0.0);
    let delay_term = if delay > 0.0 {
        format!(" e^(-{}s)", engineering_coefficient(delay))
    } else {
        String::new()
    };
    if model.kind == LowOrderModelKind::Integrating {
        return format!("G(s) = {gain}{delay_term} / s");
    }
    let mut denominator = String::new();
    for time_constant in time_constants(model) {
        denominator.push('(');
        denominator.push_str(&engineering_coefficient(time_constant));
        denominator.push_str("s + 1)");
    }
    if denominator.is_empty() {
        format!("G(s) = {gain}")
    } else {
        format!("G(s) = {gain}{delay_term} / {denominator}")
    }
}

/// The presentation of an identified model with its fit evidence (B3.8; `identifiedModelPresentation`). The
/// uncertainty label names what the intervals are: a measurement-consistent grid, a sample-resolution envelope, or
/// otherwise a conservative search-resolution and held-out-residual interval. The estimation error is the model's
/// estimation error, or its normalized error when it has none.
#[must_use]
pub fn identified_model_presentation(
    model: &IdentifiedModel,
    fit: &ModelFitEvidence,
    input_unit: &str,
    output_unit: &str,
) -> IdentifiedModelPresentation {
    let gain_unit = format!("{output_unit}/{input_unit}");
    let time_constants = time_constants(model);
    let poles = if model.kind == LowOrderModelKind::Integrating {
        vec![0.0]
    } else {
        time_constants.iter().map(|value| -1.0 / value).collect()
    };
    let delay = model.dead_time_seconds.unwrap_or(0.0);
    let mut parameters = vec![PresentationParameter {
        name: GAIN_PARAMETER.to_string(),
        value: model.gain,
        unit: gain_unit.clone(),
    }];
    parameters.extend(time_constants.iter().zip(1_u32..).map(|(&value, number)| {
        PresentationParameter {
            name: format!("{TIME_CONSTANT_PARAMETER} {number}"),
            value,
            unit: SECONDS.to_string(),
        }
    }));
    parameters.push(PresentationParameter {
        name: "Dead time".to_string(),
        value: delay,
        unit: SECONDS.to_string(),
    });
    IdentifiedModelPresentation {
        equation: identified_model_equation(model),
        units: PresentationUnits {
            input: input_unit.to_string(),
            output: output_unit.to_string(),
            gain: gain_unit,
            time: SECONDS.to_string(),
        },
        parameters,
        poles,
        zeros: Vec::new(),
        delay_seconds: delay,
        uncertainty: PresentationUncertainty {
            label: uncertainty_label(model).to_string(),
            intervals: model.uncertainty.clone(),
        },
        estimation: PresentationError {
            normalized_error: Some(
                model
                    .normalized_estimation_error
                    .unwrap_or(model.normalized_error),
            ),
            residual_lag_one_correlation: model
                .estimation_residuals
                .as_deref()
                .map(lag_one_correlation),
        },
        validation: PresentationError {
            normalized_error: model.normalized_validation_error,
            residual_lag_one_correlation: model
                .validation_residuals
                .as_deref()
                .map(lag_one_correlation),
        },
        predicted_response: fit.points.clone(),
        step_response: step_response_view(fit),
        validity_range: fit.operating_range.clone(),
        warnings: fit.warnings.clone(),
    }
}

/// The model type of a presented model in plain words (B3.8; Identify page specification, "Identified model and plot"):
/// "Static gain model", "First-order model", "Second-order model" or "Integrating model", each followed by " with delay"
/// when its dead time is above zero, as in the specification's "First-order model with delay". The form is read from the
/// stored presentation: an integrator has its one pole at zero, and otherwise the number of time constants is the order.
#[must_use]
pub fn identified_model_type_text(presentation: &IdentifiedModelPresentation) -> String {
    let form = if is_integrating(presentation) {
        String::from("Integrating")
    } else {
        match presented_time_constants(presentation).len() {
            0 => String::from("Static gain"),
            1 => String::from("First-order"),
            2 => String::from("Second-order"),
            order => format!("{order}-order"),
        }
    };
    let delay = if presentation.delay_seconds > 0.0 {
        " with delay"
    } else {
        ""
    };
    format!("{form} model{delay}")
}

/// The key numbers of a presented model in one plain line (B3.8; Identify page item 6), for example "Gain 0.21 °C per %,
/// time constant 75 s, delay 12 s": the gain in output units per input unit, the rate gain of an integrator per second, each
/// time constant and the dead time above zero. Every number has six significant digits (B9.5), and each duration is in s
/// below two minutes, in min below two hours and in h from two hours. A unit that is empty or `1` reads as `1`, and a gain
/// dimensionless on both sides has no unit. A part the presentation does not hold is left out. The line has no fit error.
#[must_use]
pub fn identified_model_key_numbers(presentation: &IdentifiedModelPresentation) -> String {
    let mut parts = Vec::new();
    let gain = presentation
        .parameters
        .iter()
        .find(|parameter| parameter.name == GAIN_PARAMETER);
    if let Some(gain) = gain {
        let unit = unit_phrase(&presentation.units)
            .map_or_else(String::new, |phrase| format!(" {phrase}"));
        let value = engineering_coefficient(gain.value);
        parts.push(if is_integrating(presentation) {
            format!("Rate gain {value}{unit} per s")
        } else {
            format!("Gain {value}{unit}")
        });
    }
    match presented_time_constants(presentation).as_slice() {
        [] => {}
        [single] => parts.push(format!("time constant {}", duration_text(*single))),
        [leading @ .., last] => {
            let leading: Vec<String> = leading.iter().map(|value| duration_text(*value)).collect();
            parts.push(format!(
                "time constants {} and {}",
                leading.join(", "),
                duration_text(*last)
            ));
        }
    }
    if presentation.delay_seconds > 0.0 {
        parts.push(format!(
            "delay {}",
            duration_text(presentation.delay_seconds)
        ));
    }
    parts.join(", ")
}

/// Whether a presented model integrates: its one pole is at zero.
#[allow(clippy::float_cmp)] // The pole of an integrator is written as exactly zero.
fn is_integrating(presentation: &IdentifiedModelPresentation) -> bool {
    matches!(presentation.poles.as_slice(), [pole] if *pole == 0.0)
}

/// The time constants of a presented model, in the order of its parameters `Time constant 1`, `Time constant 2`.
fn presented_time_constants(presentation: &IdentifiedModelPresentation) -> Vec<f64> {
    presentation
        .parameters
        .iter()
        .filter(|parameter| is_time_constant(&parameter.name))
        .map(|parameter| parameter.value)
        .collect()
}

/// Whether a parameter name is that of a time constant: `Time constant` and its number.
fn is_time_constant(name: &str) -> bool {
    name.strip_prefix(TIME_CONSTANT_PARAMETER)
        .and_then(|rest| rest.strip_prefix(' '))
        .is_some_and(|number| {
            !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
        })
}

/// The unit of a gain in words, `output per input` (for example "°C per %"), or `None` when both sides are dimensionless.
fn unit_phrase(units: &PresentationUnits) -> Option<String> {
    let output = dimensioned(&units.output);
    let input = dimensioned(&units.input);
    if output.is_none() && input.is_none() {
        return None;
    }
    Some(format!(
        "{} per {}",
        output.unwrap_or("1"),
        input.unwrap_or("1")
    ))
}

/// A unit that has a dimension: not empty and not `1`.
fn dimensioned(unit: &str) -> Option<&str> {
    let unit = unit.trim();
    (!unit.is_empty() && unit != "1").then_some(unit)
}

/// A duration of the key numbers (Identify page item 6): in s below two minutes, in min below two hours and in h from two
/// hours, each with the engineering coefficient format (B9.5).
fn duration_text(seconds: f64) -> String {
    if seconds < 120.0 {
        format!("{} {SECONDS}", engineering_coefficient(seconds))
    } else if seconds < 7_200.0 {
        format!("{} min", engineering_coefficient(seconds / 60.0))
    } else {
        format!("{} h", engineering_coefficient(seconds / 3_600.0))
    }
}

/// The time constants a model has, the first before the second.
fn time_constants(model: &IdentifiedModel) -> Vec<f64> {
    [
        model.time_constant_seconds,
        model.second_time_constant_seconds,
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// A coefficient of the equation to six significant digits, as the `TypeScript` prints it (`engineeringCoefficient`:
/// `Number(value.toPrecision(6)).toString()`).
fn engineering_coefficient(value: f64) -> String {
    js_number(js_to_precision(value, 6))
}

/// The label that says what a model's uncertainty intervals are.
fn uncertainty_label(model: &IdentifiedModel) -> &'static str {
    match model
        .uncertainty
        .as_ref()
        .map(|uncertainty| &uncertainty.method)
    {
        Some(UncertaintyMethod::MeasurementConsistentGrid) => MEASUREMENT_CONSISTENT_LABEL,
        Some(UncertaintyMethod::SampleResolutionEnvelope) => SAMPLE_RESOLUTION_LABEL,
        Some(UncertaintyMethod::GridResolutionAndValidationResidual) | None => {
            SEARCH_RESOLUTION_LABEL
        }
    }
}

/// The step response to plot: none without a step-response analysis; with one, the measurement of the plotted record
/// when the fit has it, and otherwise the analysis itself.
fn step_response_view(fit: &ModelFitEvidence) -> Option<StepResponseView> {
    let analysis = fit.step_response.as_ref()?;
    Some(match &fit.step_response_measurement {
        Some(measurement) => StepResponseView::Measurement(Box::new(measurement.clone())),
        None => StepResponseView::Analysis(Box::new(analysis.clone())),
    })
}

#[cfg(test)]
#[path = "presentation_tests.rs"]
mod tests;
