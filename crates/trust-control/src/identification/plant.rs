//! Plant identification (specification 35, B3.5, B3.9 and A10.3; numerical annex N2 and N3): the low-order fit of one
//! recording, or of an estimation recording scored on an independent validation recording, the selection of the
//! simplest acceptable model, and the physical refusals of a held step (`identifyPlant` and `selectLowOrderModel` in
//! `identification.ts`). The arithmetic is the library's (`trust_control_math::identify`); this module holds Control
//! Studio's rules, limits and sentences.

#![allow(clippy::module_name_repetitions)] // `identify_plant` keeps the name of the `TypeScript` rule.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::identify::fit::{
    fit_form, identify_low_order_models, prepare_record, FormFit,
};
use trust_control_math::identify::response::step_amplitude;
use trust_control_math::identify::single_run::largest_input_change;
use trust_control_math::identify::{
    LowOrderIdentificationOptions, LowOrderModelFit, LowOrderModelKind, LowOrderRejection,
    ParameterInterval, TimeDomainSample, UncertaintyMethod as FitUncertaintyMethod,
};
use trust_control_math::sampling::statistics::{baseline_noise_rms, mean};
use trust_control_math::sampling::{largest, median_spacing, smallest};

use crate::identification::data::{
    fit_recording, held_step_samples, inputs_of, median_interval, outputs_of,
    qualify_identification_data, DataSource, ExcludedInterval, IdentificationSample,
    QualifiedIdentificationData,
};
use crate::identification::evidence::{dominant_time_constant, held_step_evidence, predicted_span};
use crate::identification::refusal::Refusal;
use crate::identification::selection::{is_stable_form, select, CheckedCandidate};
use crate::identification::texts::{
    minutes_text, noise_refusal, partial_response_refusal, ExcitationWord,
    MINIMUM_REACHED_FRACTION, OUTPUT_TOO_SMALL,
};

/// The normalised error limit of [`identify_plant`] when its caller names none: the default of the identification policy's
/// `maximumNormalizedError`, which N3.2 and N3.5 give as 0.15.
pub const DEFAULT_MAXIMUM_NORMALIZED_ERROR: f64 = 0.15;

/// A single-run step has at least this many samples before it (`stepIndex < 2` is refused).
const MINIMUM_SAMPLES_BEFORE_STEP: usize = 2;

/// A single-run step leaves at least this many samples after its step sample (`stepIndex >= length − 10` is refused).
const MINIMUM_SAMPLES_AFTER_STEP: usize = 10;

/// The final window of a single-run step holds the last tenth of the record, and at least this many samples.
const MINIMUM_FINAL_SAMPLES: usize = 5;

/// The smallest measured gain magnitude a single-run step identifies.
const MINIMUM_GAIN: f64 = 1e-9;

/// The process an identification policy declares to accept an integrating form directly (N2.3a).
const INTEGRATING_PROCESS: &str = "integrating";

/// The refusal when no candidate of a single run is acceptable.
const NO_ACCEPTABLE_MODEL: &str =
    "No acceptable stable low-order model could be identified from the retained data.";

/// How the uncertainty intervals of an identified model were obtained (N2.4). None of them is a statistical
/// confidence interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum UncertaintyMethod {
    /// From the search-grid resolution and the residual on the independent validation record.
    #[serde(rename = "grid-resolution-and-validation-residual")]
    GridResolutionAndValidationResidual,
    /// From the sample resolution of one estimation record: its endpoint gains, and one sample interval on each time.
    #[serde(rename = "sample-resolution-envelope")]
    SampleResolutionEnvelope,
    /// The marginal ranges of the measurement-consistent parameter grid of a bounded step (N2.4).
    #[serde(rename = "measurement-consistent-grid")]
    MeasurementConsistentGrid,
}

/// The uncertainty intervals of an identified model (N2.4). A parameter the model does not have has no interval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IdentifiedUncertainty {
    /// How the intervals were obtained.
    pub method: UncertaintyMethod,
    /// The interval of the gain.
    pub gain: ParameterInterval,
    /// The interval of the time constant, when the model has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_constant_seconds: Option<ParameterInterval>,
    /// The interval of the second time constant, when the model has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub second_time_constant_seconds: Option<ParameterInterval>,
    /// The interval of the dead time, when the model has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dead_time_seconds: Option<ParameterInterval>,
}

/// A low-order model identified from a recording (B3.5, B3.8; N2.1; `IdentifiedModel`): its form, gain and dynamics,
/// its normalised errors and residuals, and its uncertainty. A parameter the form does not have is absent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IdentifiedModel {
    /// The model form.
    pub kind: LowOrderModelKind,
    /// The static gain `K`, or the rate gain `K′` of an integrating form.
    pub gain: f64,
    /// The time constant `T` in seconds, or the first time constant of a second-order form.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_constant_seconds: Option<f64>,
    /// The second time constant in seconds, for the second-order forms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub second_time_constant_seconds: Option<f64>,
    /// The dead time `L` in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dead_time_seconds: Option<f64>,
    /// The normalised error that decides the model (N3.2): the validation error of an independently validated fit,
    /// the error on the recording itself for a single-run preview. A fit always records it. A supplied model was not fitted, so
    /// it records none, and Tune's error check does not apply to it (B3.7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_error: Option<f64>,
    /// The normalised error on the estimation record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_estimation_error: Option<f64>,
    /// The normalised error on the independent validation record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_validation_error: Option<f64>,
    /// The residuals on the estimation record (N3.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimation_residuals: Option<Vec<f64>>,
    /// The residuals on the validation record (N3.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_residuals: Option<Vec<f64>>,
    /// The uncertainty intervals of the parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncertainty: Option<IdentifiedUncertainty>,
    /// Whether the model may be used as a stable form: an integrating form, or a form whose time constants are
    /// positive.
    pub stable: bool,
    /// Whether the best grid point of the fit lies on the edge of its search grid: a search-boundary finding that N2.4
    /// keeps.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub on_search_boundary: bool,
}

impl IdentifiedModel {
    /// The normalised error that decides a fitted model (N3.2) for the fit's own comparisons. A fit always records it; a model
    /// without one is read as the largest error, so no limit accepts it.
    #[must_use]
    pub fn decisive_error(&self) -> f64 {
        self.normalized_error.unwrap_or(f64::INFINITY)
    }
}

/// The evidence an identification result names: data of a simulated or of a physical target (A2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ResultSource {
    /// Identified from data recorded on a simulated target.
    #[serde(rename = "Identified from simulation data")]
    FromSimulation,
    /// Identified from data recorded on a physical target.
    #[serde(rename = "Identified from physical data")]
    FromPhysical,
}

/// How the data of an identification were used (N1.1, N3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Partition {
    /// One recording: a same-run preview, not qualified for controller design.
    SingleRun,
    /// An estimation recording scored on a distinct validation recording.
    IndependentRuns,
}

/// The result of an identification (B3.5, B3.8; `IdentificationResult`): the recommended model, every fitted model,
/// and the physical refusals a preview reports (B3.9).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IdentificationResult {
    /// Whether the data were simulated or physical.
    pub source: ResultSource,
    /// The recommended model, one of `models`.
    pub recommended: IdentifiedModel,
    /// Every fitted model.
    pub models: Vec<IdentifiedModel>,
    /// How the data were used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<Partition>,
    /// The physical refusal of the recommended form (A10.3, N2.6): a response too small against the noise, or a held
    /// step that has not reached 63 % of its fitted final change. It is reported, never replaced by another model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    /// The integrator rule's refusal (N2.3a): an integrating form the data cannot yet tell apart from a
    /// self-regulating one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrator_refusal: Option<String>,
}

/// What the caller states about the process and the excitation (N2.2, N2.3a; `IdentifyPlantOptions`): the process its
/// identification policy declares, whether the fit is closed loop, and whether a single run is a pulse that returns to
/// its baseline. The default states nothing: an open-loop held step.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdentifyPlantOptions {
    /// The process the loop's identification policy declares; `integrating` accepts an integrating form directly.
    pub declared_process: Option<String>,
    /// Whether the fit is closed loop (a setpoint step in Auto), which the integrator rule does not apply to.
    pub closed_loop: bool,
    /// Whether the single run is a pulse that returns to its baseline. The fit then uses the whole record from the steady
    /// baseline, with no fraction, no A10.3 refusal and no integrator rule, since the return is part of the excitation
    /// (N2.2, N2.3, N2.3a). The single-run fit reads it; the validated fit reads the held step.
    pub pulse: bool,
}

/// Identifies a low-order plant model from a recording (B3.5; N2 and N3; `identifyPlant`).
///
/// The samples are qualified first, without the excluded intervals (N1.1). Without validation samples, the fit is a
/// single-run preview of the held step (B3.4 step 5) or of a pulse (`options.pulse`): the six forms of N2.1 are each
/// fitted to the one record by the least-squares grid search of N2.2, and each is scored on that same record (N3.4),
/// with a sample-resolution envelope. With validation samples, every form is fitted on the held step of the estimation
/// record and scored on the held step of the validation record (N3.2, N3.4), with `maximum_normalized_error` as the
/// limit of its validation error. Either way the recommended form is chosen by [`select`] (N3.3), and the fraction it is
/// judged by is read on the estimation record (N2.3). The result carries the held-step refusal of the recommended form
/// (A10.3, N2.6) and the integrator rule's refusal (N2.3a) as text; neither is an error.
///
/// # Errors
/// Returns the [`Refusal`] of a recording that does not qualify (the validation recording's reason after
/// `Independent validation: `), of a validated fit that qualifies no model, of a single run without a bounded step or
/// with a gain that is not identifiable, and of a single run with no acceptable candidate or no time after its step.
pub fn identify_plant(
    samples: &[IdentificationSample],
    excluded: &[ExcludedInterval],
    source: DataSource,
    validation: Option<&[IdentificationSample]>,
    maximum_normalized_error: f64,
    options: &IdentifyPlantOptions,
) -> Result<IdentificationResult, Refusal> {
    let qualified = qualify_identification_data(samples, excluded, false);
    if !qualified.suitable {
        return Err(Refusal::new(qualified.reason.unwrap_or_default()));
    }
    let source = match source {
        DataSource::Physical => ResultSource::FromPhysical,
        DataSource::Simulation => ResultSource::FromSimulation,
    };
    match validation {
        Some(validation) => identify_with_validation(
            &qualified,
            validation,
            source,
            maximum_normalized_error,
            options,
        ),
        None => identify_single_run(&qualified, source, maximum_normalized_error, options),
    }
}

/// The independently validated fit (N3.2, N3.4): every form fitted on the held step of the estimation record and
/// scored on the held step of the validation record, each record as a fit receives it (A4.5). The recommended form is
/// the one choice of [`select`] (N3.3), with the fraction read on the estimation record (N2.3).
fn identify_with_validation(
    qualified: &QualifiedIdentificationData,
    validation_samples: &[IdentificationSample],
    source: ResultSource,
    maximum_normalized_error: f64,
    options: &IdentifyPlantOptions,
) -> Result<IdentificationResult, Refusal> {
    let validation = qualify_identification_data(validation_samples, &[], false);
    if !validation.suitable {
        let reason = validation.reason.unwrap_or_default();
        return Err(Refusal::new(format!("Independent validation: {reason}")));
    }
    let estimation_values = fit_recording(&held_step_samples(&qualified.samples));
    let validation_values = fit_recording(&held_step_samples(&validation.samples));
    let fits = identify_low_order_models(
        &time_domain(&estimation_values),
        &time_domain(&validation_values),
        &low_order_options(),
    )
    .map_err(fit_refusal)?;
    let candidates: Vec<CheckedCandidate> = fits
        .into_iter()
        .map(|fit| validated_candidate(fit, &estimation_values))
        .collect();
    let models: Vec<IdentifiedModel> = candidates
        .iter()
        .map(|candidate| candidate.model.clone())
        .collect();
    let recommended = select(&candidates, maximum_normalized_error)
        .ok_or_else(|| Refusal::new(NO_ACCEPTABLE_MODEL))?;
    let refusal = unreliable_step_refusal(&recommended, &qualified.samples, &estimation_values);
    let integrator_refusal =
        integrator_refusal_for(options, &estimation_values, maximum_normalized_error);
    Ok(IdentificationResult {
        source,
        recommended,
        models,
        partition: Some(Partition::IndependentRuns),
        refusal,
        integrator_refusal,
    })
}

/// The refusal of a validated fit that the library rejects. A flat output is the sentence of a flat record; the other
/// rejections name their reason.
fn fit_refusal(reason: LowOrderRejection) -> Refusal {
    match reason {
        LowOrderRejection::FlatOutput => Refusal::new(OUTPUT_TOO_SMALL),
        other => Refusal::new(format!(
            "No qualified low-order model: {}.",
            rejection_text(other)
        )),
    }
}

/// A validated fit as a candidate of the choice: its validation errors and sample interval, and the fraction of its
/// fitted final change that the held step of the estimation record reached (N2.3).
fn validated_candidate(
    fit: LowOrderModelFit,
    estimation: &[IdentificationSample],
) -> CheckedCandidate {
    let final_normalized_error = fit.final_normalized_validation_error;
    let sample_interval_seconds = fit.sample_interval_seconds;
    let model = low_order_model(fit);
    let reached_fraction =
        held_step_evidence(&model, estimation).and_then(|evidence| evidence.fraction);
    CheckedCandidate {
        model,
        final_normalized_error,
        reached_fraction,
        sample_interval_seconds,
    }
}

/// The single-run preview of a held step or a pulse (B3.5; N2.2): the six forms fitted to the record by the grid fit,
/// their errors on the same record (N3.4), their sample-resolution envelopes, and the selection. A held step is cut at
/// its restoration and measured as a bounded step; a pulse is fitted on its whole record from the steady baseline, with
/// no
/// fraction and no integrator rule (N2.2, N2.3, N2.3a).
fn identify_single_run(
    qualified: &QualifiedIdentificationData,
    source: ResultSource,
    maximum_normalized_error: f64,
    options: &IdentifyPlantOptions,
) -> Result<IdentificationResult, Refusal> {
    let held = single_run_samples(&qualified.samples, options.pulse);
    let values = fit_recording(&held);
    let step = if options.pulse {
        MeasuredStep {
            index: step_index(&values)?,
            gain_range: None,
        }
    } else {
        measure_step(&values)?
    };
    let times = times_of(&values);
    let inputs = inputs_of(&values);
    let outputs = outputs_of(&values);
    // Block averaging, when it applies, widens the sample spacing the envelope is measured on.
    let sample_time = if values.len() == held.len() {
        qualified.sample_time_seconds
    } else {
        median_spacing(&times)
    };
    let horizon = times[times.len() - 1] - times[step.index];
    // The record runs from the start of the steady baseline to the end of the hold, or to the last sample of a pulse,
    // and its span normalises every error (N1.1, N2.2). A flat response has no span and is refused by the preparation.
    let record = prepare_record(&times, &inputs, &outputs, step.index).map_err(fit_refusal)?;
    let mut checked = Vec::new();
    for kind in LOW_ORDER_FORMS {
        let Some(form) = fit_form(&record, kind) else {
            continue;
        };
        let model = single_run_model(&form, &step, sample_time);
        // The fraction the held step reached is the one the refusal reports (N2.3, A10.3). A pulse has no final change,
        // so its fraction is not defined, and its acceptance rests on the error limit alone (N2.3).
        let reached_fraction = if options.pulse {
            None
        } else {
            held_step_evidence(&model, &values).and_then(|evidence| evidence.fraction)
        };
        checked.push(CheckedCandidate {
            model,
            final_normalized_error: form.prediction.final_normalized_error,
            reached_fraction,
            sample_interval_seconds: sample_time,
        });
    }
    let all_models: Vec<IdentifiedModel> = checked
        .iter()
        .map(|candidate| candidate.model.clone())
        .collect();
    let recommended = select(&checked, maximum_normalized_error)
        .ok_or_else(|| Refusal::new(NO_ACCEPTABLE_MODEL))?;
    if horizon <= sample_time {
        return Err(Refusal::new(NO_ACCEPTABLE_MODEL));
    }
    let refusal = if options.pulse {
        pulse_refusal(&recommended, &held, &values)
    } else {
        unreliable_step_refusal(&recommended, &held, &values)
    };
    let integrator_refusal = integrator_refusal_for(options, &values, maximum_normalized_error);
    Ok(IdentificationResult {
        source,
        recommended,
        models: all_models,
        partition: Some(Partition::SingleRun),
        refusal,
        integrator_refusal,
    })
}

/// The samples a single run fits: the whole record of a pulse, which returns to its baseline (N2.2), and the held part
/// of a step, cut at its restoration (B3.4 step 5).
pub(crate) fn single_run_samples(
    samples: &[IdentificationSample],
    pulse: bool,
) -> Vec<IdentificationSample> {
    if pulse {
        samples.to_vec()
    } else {
        held_step_samples(samples)
    }
}

/// The bounded step of a single-run record, measured on its fitted samples: the step's index and, for a held step, the
/// gain range of its endpoint resolution (N2.4). A pulse has no endpoint gains, so its gain range is absent.
struct MeasuredStep {
    /// The index of the step: the first sample at the new input level.
    index: usize,
    /// The smallest and the largest of the two endpoint gains and the measured gain of a held step; `None` for a pulse.
    gain_range: Option<(f64, f64)>,
}

/// Measures the bounded step of a fitted record (`identifyPlant`): the largest input change, with at least two
/// samples before it and ten after it; the means before the step and over the final window, the last tenth of the
/// record (at least five samples, all after the step); and the endpoint gains of the extreme outputs of the two
/// windows. One estimation run cannot establish statistical confidence, so the gain envelope keeps only that endpoint
/// resolution.
fn measure_step(values: &[IdentificationSample]) -> Result<MeasuredStep, Refusal> {
    let index = step_index(values)?;
    let final_samples = (values.len() / 10).max(MINIMUM_FINAL_SAMPLES);
    let before = &values[..index];
    let after = &values[(index + 1).max(values.len().saturating_sub(final_samples))..];
    let before_outputs = outputs_of(before);
    let after_outputs = outputs_of(after);
    let input_before = mean(&inputs_of(before));
    let output_before = mean(&before_outputs);
    let input_change = step_amplitude(input_before, &inputs_of(&values[index..]));
    let response = mean(&after_outputs) - output_before;
    let gain = response / input_change;
    if !gain.is_finite() || gain.abs() < MINIMUM_GAIN {
        return Err(Refusal::new(
            "The measured input/output gain is not identifiable.",
        ));
    }
    let low =
        (smallest(&after_outputs) - largest(&before_outputs, f64::NEG_INFINITY)) / input_change;
    let high =
        (largest(&after_outputs, f64::NEG_INFINITY) - smallest(&before_outputs)) / input_change;
    Ok(MeasuredStep {
        index,
        gain_range: Some((
            js_min(js_min(low, high), gain),
            js_max(js_max(low, high), gain),
        )),
    })
}

/// The index of the bounded step of a single run: its largest input change, with at least two samples before it and ten
/// after it (B3.4 step 5). For a pulse it is the pulse's first input change, the first of two equal changes (N2.2).
///
/// # Errors
/// Returns the refusal of a recording without such a change, which asks for pre- and post-step data.
fn step_index(values: &[IdentificationSample]) -> Result<usize, Refusal> {
    largest_input_change(&inputs_of(values))
        .filter(|&index| {
            index >= MINIMUM_SAMPLES_BEFORE_STEP
                && index + MINIMUM_SAMPLES_AFTER_STEP < values.len()
        })
        .ok_or_else(|| {
            Refusal::new("A bounded input step with pre- and post-step data is required.")
        })
}

/// The six forms of N2.1 in the order the list gives them, which a single run and an excitation fit both fit.
pub(crate) const LOW_ORDER_FORMS: [LowOrderModelKind; 6] = [
    LowOrderModelKind::StaticGain,
    LowOrderModelKind::FirstOrder,
    LowOrderModelKind::FirstOrderDeadTime,
    LowOrderModelKind::Integrating,
    LowOrderModelKind::SecondOrder,
    LowOrderModelKind::SecondOrderDeadTime,
];

/// The single-run model of a fitted form (B3.5, N2.2): its gain and dynamics from the grid fit, its normalised error on the
/// recorded step (N3.2), its stability (N2.1), and the sample-resolution envelope of the run.
fn single_run_model(form: &FormFit, step: &MeasuredStep, sample_time: f64) -> IdentifiedModel {
    let shape = form.shape;
    let mut model = IdentifiedModel {
        kind: form.kind,
        gain: form.gain,
        time_constant_seconds: shape.time_constant_seconds,
        second_time_constant_seconds: shape.second_time_constant_seconds,
        dead_time_seconds: shape.dead_time_seconds,
        normalized_error: Some(form.prediction.normalized_error),
        normalized_estimation_error: None,
        normalized_validation_error: None,
        estimation_residuals: None,
        validation_residuals: None,
        uncertainty: None,
        stable: is_stable_form(
            form.kind,
            shape.time_constant_seconds,
            shape.second_time_constant_seconds,
        ),
        on_search_boundary: form.on_search_boundary,
    };
    model.uncertainty = Some(sample_resolution_envelope(&model, step, sample_time));
    model
}

/// The sample-resolution envelope of a single-run candidate: one estimation run cannot establish statistical
/// confidence, so the intervals keep only the resolution of the record. The gain interval spans the endpoint gains, the
/// measured gain and the candidate's gain, or for a pulse, which has no endpoint gains, the candidate's gain alone
/// (N2.4). A time constant lies within one sample interval of its value (and not below one interval); the dead time
/// lies from its value (not below zero) to one sample interval more.
fn sample_resolution_envelope(
    model: &IdentifiedModel,
    step: &MeasuredStep,
    sample_time: f64,
) -> IdentifiedUncertainty {
    let lag_interval = |lag: f64| ParameterInterval {
        minimum: js_max(sample_time, lag - sample_time),
        maximum: lag + sample_time,
    };
    let gain = match step.gain_range {
        Some((low, high)) => ParameterInterval {
            minimum: js_min(low, model.gain),
            maximum: js_max(high, model.gain),
        },
        None => ParameterInterval {
            minimum: model.gain,
            maximum: model.gain,
        },
    };
    IdentifiedUncertainty {
        method: UncertaintyMethod::SampleResolutionEnvelope,
        gain,
        time_constant_seconds: model.time_constant_seconds.map(lag_interval),
        second_time_constant_seconds: model.second_time_constant_seconds.map(lag_interval),
        dead_time_seconds: model.dead_time_seconds.map(|delay| ParameterInterval {
            minimum: js_max(0.0, delay),
            maximum: delay + sample_time,
        }),
    }
}

/// The two refusals of a held-step fit (A10.3, N2.3, N2.6; `unreliableStepRefusal`): a response buried in the
/// baseline noise, and a held response that has not reached 63 % of its fitted final change. `raw` keeps the
/// baseline before the step; `values` is the fitted record. `None` when the fit is acceptable, or when the record
/// holds more than one excitation, where neither refusal is defined.
fn unreliable_step_refusal(
    model: &IdentifiedModel,
    raw: &[IdentificationSample],
    values: &[IdentificationSample],
) -> Option<String> {
    let evidence = held_step_evidence(model, values)?;
    let (noise_rms, unit) = baseline_noise_and_unit(raw);
    noise_refusal(
        evidence.response_size,
        noise_rms,
        unit,
        ExcitationWord::Step,
    )
    .or_else(|| {
        evidence
            .fraction
            .filter(|fraction| *fraction < MINIMUM_REACHED_FRACTION)
            .map(|fraction| {
                partial_response_refusal(
                    fraction,
                    evidence.hold_seconds,
                    model.dead_time_seconds.unwrap_or(0.0),
                    dominant_time_constant(model),
                )
            })
    })
}

/// The RMS noise of the outputs before the first input change of a raw recording (N1.3, N2.6), and the output unit its
/// first sample states (empty when none does).
fn baseline_noise_and_unit(raw: &[IdentificationSample]) -> (f64, &str) {
    let noise_rms = largest_input_change(&inputs_of(raw))
        .map_or(0.0, |step| baseline_noise_rms(&outputs_of(&raw[..step])));
    let unit = raw
        .iter()
        .filter_map(|sample| sample.output_unit.as_deref())
        .find(|unit| !unit.is_empty())
        .unwrap_or("");
    (noise_rms, unit)
}

/// The refusal of a pulse whose predicted response is buried in the baseline noise (N2.6). The response is the span of
/// the output the model predicts over the record, read as an excitation fit reads it. A pulse has no final change, so
/// no fraction and no A10.3 refusal apply (N2.3).
fn pulse_refusal(
    model: &IdentifiedModel,
    raw: &[IdentificationSample],
    values: &[IdentificationSample],
) -> Option<String> {
    let response_size = predicted_span(model, values)?;
    let (noise_rms, unit) = baseline_noise_and_unit(raw);
    noise_refusal(response_size, noise_rms, unit, ExcitationWord::Excitation)
}

/// The integrator rule of a held step in open loop (N2.3a; `integratorRefusalFor`). An integrating form is accepted
/// only when the data tell it apart from a self-regulating form: when the best first-order-with-delay fit meets the
/// error limit with a time constant at least as long as the hold, the response has not yet shown that it levels off,
/// and the rule refuses, naming the hold and the hold that fit suggests, `L + 3·T` (N2.3). A declared integrating
/// process, a closed-loop fit and a pulse skip the rule; so does a record without a step, a sample interval, a hold or
/// a response. `None` when the rule does not refuse.
pub(crate) fn integrator_refusal_for(
    options: &IdentifyPlantOptions,
    values: &[IdentificationSample],
    maximum_normalized_error: f64,
) -> Option<String> {
    if options.closed_loop
        || options.pulse
        || options.declared_process.as_deref() == Some(INTEGRATING_PROCESS)
    {
        return None;
    }
    let sample_time = median_interval(values);
    let step_index = largest_input_change(&inputs_of(values)).filter(|_| sample_time > 0.0)?;
    let times = times_of(values);
    let inputs = inputs_of(values);
    let outputs = outputs_of(values);
    let measured = &outputs[step_index..];
    let hold_seconds = times[times.len() - 1] - times[step_index];
    let span = largest(measured, f64::NEG_INFINITY) - smallest(measured);
    let measurable = hold_seconds > 0.0 && span > 0.0;
    if !measurable {
        return None;
    }
    // The first-order-with-delay form is fitted to the same data by the same grid fit (N2.3a, N2.2).
    let record = prepare_record(&times, &inputs, &outputs, step_index).ok()?;
    let form = fit_form(&record, LowOrderModelKind::FirstOrderDeadTime)?;
    let within_limit = form.prediction.normalized_error <= maximum_normalized_error
        && form.prediction.final_normalized_error <= maximum_normalized_error;
    let time_constant = form.shape.time_constant_seconds?;
    if !within_limit || time_constant < hold_seconds {
        return None;
    }
    let minutes = minutes_text(hold_seconds);
    // The hold the first-order-with-delay fit suggests (N2.3a, N2.3): `L + 3·T`, at which it reaches 95 %.
    let suggested = minutes_text(form.shape.dead_time_seconds.unwrap_or(0.0) + 3.0 * time_constant);
    Some(format!(
        "The response is still rising like a ramp after {minutes} min; it cannot yet be told whether it levels off. Hold the step for about {suggested} min (Limits… → maximum duration), or identify with a setpoint step in Auto."
    ))
}

/// The options of the validated fit (`identifyPlant`): every low-order form, in the order the `TypeScript` lists them.
fn low_order_options() -> LowOrderIdentificationOptions {
    LowOrderIdentificationOptions {
        allowed_kinds: vec![
            LowOrderModelKind::StaticGain,
            LowOrderModelKind::Integrating,
            LowOrderModelKind::FirstOrder,
            LowOrderModelKind::FirstOrderDeadTime,
            LowOrderModelKind::SecondOrder,
            LowOrderModelKind::SecondOrderDeadTime,
        ],
        minimum_samples: 20,
        minimum_input_span: 1e-9,
    }
}

/// An independently validated fit as an identified model (`lowOrderModel`): its deciding error is its validation
/// error, and its stability is the form's (N2.1).
fn low_order_model(fit: LowOrderModelFit) -> IdentifiedModel {
    let stable = is_stable_form(
        fit.kind,
        fit.time_constant_seconds,
        fit.second_time_constant_seconds,
    );
    let method = match fit.uncertainty.method {
        FitUncertaintyMethod::GridResolutionAndValidationResidual => {
            UncertaintyMethod::GridResolutionAndValidationResidual
        }
    };
    IdentifiedModel {
        kind: fit.kind,
        gain: fit.gain,
        time_constant_seconds: fit.time_constant_seconds,
        second_time_constant_seconds: fit.second_time_constant_seconds,
        dead_time_seconds: fit.dead_time_seconds,
        normalized_error: Some(fit.normalized_validation_error),
        normalized_estimation_error: Some(fit.normalized_estimation_error),
        normalized_validation_error: Some(fit.normalized_validation_error),
        estimation_residuals: Some(fit.estimation_residuals),
        validation_residuals: Some(fit.validation_residuals),
        uncertainty: Some(IdentifiedUncertainty {
            method,
            gain: fit.uncertainty.gain,
            time_constant_seconds: fit.uncertainty.time_constant_seconds,
            second_time_constant_seconds: fit.uncertainty.second_time_constant_seconds,
            dead_time_seconds: fit.uncertainty.dead_time_seconds,
        }),
        stable,
        on_search_boundary: fit.on_search_boundary,
    }
}

/// The word of a rejected validated fit in its refusal: the JSON word of the rejection.
fn rejection_text(reason: LowOrderRejection) -> &'static str {
    match reason {
        LowOrderRejection::NonfiniteInput => "nonfinite-input",
        LowOrderRejection::InsufficientData => "insufficient-data",
        LowOrderRejection::PoorExcitation => "poor-excitation",
        LowOrderRejection::MissingStep => "missing-step",
        LowOrderRejection::FlatOutput => "flat-output",
    }
}

/// The samples as the library's fit reads them: time, input, output and quality.
fn time_domain(samples: &[IdentificationSample]) -> Vec<TimeDomainSample> {
    samples
        .iter()
        .map(|sample| TimeDomainSample {
            time_seconds: sample.time_seconds,
            input: sample.input,
            output: sample.output,
            quality: sample.quality,
        })
        .collect()
}

/// The times of the samples, in order.
fn times_of(samples: &[IdentificationSample]) -> Vec<f64> {
    samples.iter().map(|sample| sample.time_seconds).collect()
}

/// `Math.min(left, right)` as JavaScript computes it: `NaN` when either value is `NaN`, where `f64::min` returns the
/// other value. The envelope and limit arithmetic of the `TypeScript` is kept exactly.
pub(super) fn js_min(left: f64, right: f64) -> f64 {
    if left.is_nan() || right.is_nan() {
        f64::NAN
    } else {
        left.min(right)
    }
}

/// `Math.max(left, right)` as JavaScript computes it: `NaN` when either value is `NaN`.
pub(super) fn js_max(left: f64, right: f64) -> f64 {
    if left.is_nan() || right.is_nan() {
        f64::NAN
    } else {
        left.max(right)
    }
}

#[cfg(test)]
#[path = "plant_tests.rs"]
pub(crate) mod tests;
