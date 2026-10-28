//! The excitation fit (`excitationIdentification.ts`): the simple forms of the Find model flow, static gain, integrating,
//! first order, first order with delay and the two second-order forms, fitted to any recorded excitation such as a step,
//! relay, PRBS or multisine test. Every form is fitted by the library's least-squares grid fit on the recorded input and
//! the recorded sample times (N2.2, N3.1), and its gain is the least-squares value for those dynamics, from the steady
//! baseline before the hold. The recommended form is the one choice of N3.3, shared with the plant fits. Its predicted
//! response must be larger than the measured noise (N2.6, B3.9), and a single held step is judged by its fraction (N2.3)
//! and by the integrator rule (N2.3a).

#![allow(clippy::module_name_repetitions)] // `ExcitationFit` is the name the `TypeScript` record has.

use std::cmp::Ordering;

use trust_control_math::identify::fit::{fit_form, prepare_record, FormFit, PreparedRecord};
use trust_control_math::identify::LowOrderModelKind;
use trust_control_math::sampling::statistics::{baseline_noise_rms, mean};
use trust_control_math::sampling::{block_averaged, first_gap, median_spacing, EvenSample};

use crate::experiment::number_text::{js_number, js_to_fixed};
use crate::experiment::SampleQuality;
use crate::identification::data::{held_step_samples, IdentificationSample, MAXIMUM_FIT_SAMPLES};
use crate::identification::evidence::{dominant_time_constant, held_step_evidence, ModelFitPoint};
use crate::identification::plant::{
    integrator_refusal_for, js_max, js_min, IdentifiedModel, IdentifyPlantOptions, LOW_ORDER_FORMS,
};
use crate::identification::refusal::Refusal;
use crate::identification::selection::{is_stable_form, select, CheckedCandidate};
use crate::identification::texts::{
    gap_phrase, noise_refusal, partial_response_refusal, ExcitationWord, MINIMUM_REACHED_FRACTION,
    MISSING_NONZERO_STEP, OUTPUT_TOO_SMALL,
};

/// The fewest samples an excitation fit accepts (`samples.length < 20`).
const MINIMUM_SAMPLES: usize = 20;

/// The fewest samples of steady baseline before the first input change that a fit accepts (`first < 3`).
const MINIMUM_BASELINE_SAMPLES: usize = 3;

/// The refusal for a recording with too few samples, or with a sample that is not good, finite and in time order.
const UNUSABLE_SAMPLES: &str =
    "Identification requires at least 20 good, finite, time-ordered samples.";

/// The refusal for a steady baseline of fewer than three samples before the first input change.
const SHORT_BASELINE: &str =
    "A steady baseline of at least three samples before the first input change is required.";

/// The result of an excitation fit (`ExcitationFit`): the recommended form, every form that was fitted, the operating
/// point the fit is relative to, the measured output with the recommended form's prediction at each sample, and the
/// physical refusals of the recommended form.
#[derive(Debug, Clone, PartialEq)]
pub struct ExcitationFit {
    /// The recommended form: the one choice of N3.3 among the fitted forms.
    pub recommended: IdentifiedModel,
    /// Every form that could be fitted, adequate or not: the six forms of N2.1 items 1 to 6.
    pub models: Vec<IdentifiedModel>,
    /// The mean plant input over the steady baseline before the first input change, `u0`.
    pub input0: f64,
    /// The mean measured output over the same baseline, `y0`.
    pub output0: f64,
    /// The measured output and the recommended form's prediction at each sample the fit used.
    pub points: Vec<ModelFitPoint>,
    /// The refusal of the recommended form: a held step that reached less than 63 % of its fitted final change (A10.3),
    /// or a predicted response too small against the noise (N2.6). It is reported, never replaced by another form.
    pub refusal: Option<String>,
    /// The integrator rule's refusal (N2.3a) of a single held step: it applies when the recommended form is integrating.
    pub integrator_refusal: Option<String>,
}

/// Fits the simple forms to a recorded excitation (`identifyPlantFromExcitation`).
///
/// The recording needs at least 20 samples, each good and finite, in strictly increasing time. An interval longer than one
/// and a half median spacings is a gap, refused with its time (A4.3). The operating point is the mean over the steady
/// baseline before the first input change, or, when `baseline_end_seconds` names the end of the baseline (B3.4), the mean
/// over the samples up to that time. A loop in Auto moves its plant input with the measured noise before a setpoint step,
/// so the first change of the input is not where its experiment begins (N1.3, N2.2). The record runs from the start of the
/// recording, which begins at the steady baseline window, to the last sample; each form is fitted to it by the library's
/// least-squares grid fit, driven by the recorded input on the recorded sample times (N2.2, N3.1). A recording need not hold
/// a single dominant step: a PRBS, a multisine or a setpoint step in Auto is fitted the same way (N2.6).
///
/// The recommended form is the one choice of N3.3 ([`select`]): within the limit on the record and on its final set
/// (N3.2), and, for a single held step with a finite final change, 63 % reached (N2.3, A10.3). Its refusals are returned
/// in the fit, not as an error: a held step that reached less than 63 % (A10.3), a predicted response not larger than 20
/// times the baseline noise (N2.6, B3.9), and, for a single held step whose recommended form is integrating, the
/// integrator rule (N2.3a), which a declared integrating process or a closed-loop fit skips (`options`).
///
/// # Errors
/// Returns a [`Refusal`] when the recording is unusable, when it has a gap (named with its time), when its plant input never
/// changes (the missing nonzero step, N2.2 and N2.6), when its steady baseline has fewer than three samples, when its output
/// does not change, and when no form is within the limit (naming the best form).
pub fn identify_plant_from_excitation(
    samples: &[IdentificationSample],
    maximum_normalized_error: f64,
    baseline_end_seconds: Option<f64>,
    options: &IdentifyPlantOptions,
) -> Result<ExcitationFit, Refusal> {
    let prepared = prepare(samples, baseline_end_seconds)?;
    let forms = fit_forms(&prepared.record);
    let checked: Vec<CheckedCandidate> = forms
        .iter()
        .map(|form| checked_candidate(form, samples, prepared.record.sample_interval_seconds))
        .collect();
    let selected =
        select(&checked, maximum_normalized_error).ok_or_else(|| no_simple_model(&forms))?;
    let chosen = forms
        .iter()
        .find(|form| form.kind == selected.kind)
        .ok_or_else(|| no_simple_model(&forms))?;
    let predicted: Vec<f64> = chosen
        .prediction
        .unit_response
        .iter()
        .map(|&unit| prepared.output0 + chosen.gain * unit)
        .collect();
    let range = js_max_of(&predicted) - js_min_of(&predicted);
    let refusal = partial_refusal(&selected, samples).or_else(|| {
        noise_refusal(
            range,
            prepared.noise,
            &prepared.unit,
            ExcitationWord::Excitation,
        )
    });
    let integrator_refusal = if selected.kind == LowOrderModelKind::Integrating
        && held_step_evidence(&selected, samples).is_some()
    {
        integrator_refusal_for(
            options,
            &held_step_samples(samples),
            maximum_normalized_error,
        )
    } else {
        None
    };
    let points = prepared
        .record
        .times
        .iter()
        .zip(&prepared.record.outputs)
        .zip(&predicted)
        .map(|((&time_seconds, &measured), &prediction)| ModelFitPoint {
            time_seconds,
            measured,
            predicted: prediction,
            residual: measured - prediction,
        })
        .collect();
    Ok(ExcitationFit {
        recommended: selected,
        models: forms.iter().map(identified_model).collect(),
        input0: prepared.input0,
        output0: prepared.output0,
        points,
        refusal,
        integrator_refusal,
    })
}

/// The recording prepared for the fits (the first part of `identifyPlantFromExcitation`): the operating point, the noise
/// of the steady baseline, the output's unit, and the record the forms are fitted to.
struct Prepared {
    /// The mean plant input over the steady baseline, `u0`.
    input0: f64,
    /// The mean measured output over the steady baseline, `y0`.
    output0: f64,
    /// The RMS noise of the steady baseline (N1.3); zero when the baseline is too short to tell.
    noise: f64,
    /// The unit of the output as the first sample states it; empty when none does.
    unit: String,
    /// The block-averaged record the forms are fitted to, prepared by the library (N2.2).
    record: PreparedRecord,
}

/// Checks the recording and prepares the fit: its gaps, its baseline noise, its operating point and its record.
fn prepare(
    samples: &[IdentificationSample],
    baseline_end_seconds: Option<f64>,
) -> Result<Prepared, Refusal> {
    if !usable(samples) {
        return Err(Refusal::new(UNUSABLE_SAMPLES));
    }
    // Task jitter is normal; an interval longer than one and a half median spacings is a gap, refused with its time.
    let times: Vec<f64> = samples.iter().map(|sample| sample.time_seconds).collect();
    let step = median_spacing(&times);
    if let Some(gap) = first_gap(&times, step) {
        return Err(Refusal::new(format!(
            "The recording has {}.",
            gap_phrase(gap)
        )));
    }
    let unit = samples
        .first()
        .and_then(|sample| sample.output_unit.clone())
        .unwrap_or_default();
    let recorded: Vec<EvenSample> = samples.iter().map(even_sample).collect();
    // A record whose plant input never changes has no nonzero step to fit (N2.2, N2.6). It is checked after the sample and gap
    // checks, where the TypeScript's baseline check sits, so an unusable or gapped record keeps its own sentence.
    if first_input_change(&recorded).is_none() {
        return Err(Refusal::new(MISSING_NONZERO_STEP));
    }
    // The noise is that of the baseline before the first change, or of the whole record when there is no change to find
    // (`rawChange > 0`).
    let change = after_baseline(&recorded, baseline_end_seconds).filter(|&index| index > 0);
    let baseline_len = change.unwrap_or(recorded.len());
    let noise = baseline_noise_rms(&output_values(&recorded[..baseline_len]));
    // The fit keeps the recorded instants. No block straddles the first input change, so the steady baseline stays exact.
    let cuts: Vec<f64> = change.map_or_else(Vec::new, |index| vec![recorded[index].time_seconds]);
    // The limit is a nonzero constant, which the averaging always accepts.
    let fitted = block_averaged(&recorded, MAXIMUM_FIT_SAMPLES, &cuts).unwrap_or(recorded);
    let first = after_baseline(&fitted, baseline_end_seconds)
        .filter(|&index| index >= MINIMUM_BASELINE_SAMPLES)
        .ok_or_else(|| Refusal::new(SHORT_BASELINE))?;
    let fitted_times: Vec<f64> = fitted.iter().map(|sample| sample.time_seconds).collect();
    let inputs = input_values(&fitted);
    let outputs = output_values(&fitted);
    let record = prepare_record(&fitted_times, &inputs, &outputs, first).map_err(|reason| {
        match reason {
            // A flat output has no span to normalise the errors by (N3.2): it is refused before any division.
            trust_control_math::identify::LowOrderRejection::FlatOutput => {
                Refusal::new(OUTPUT_TOO_SMALL)
            }
            _ => Refusal::new(UNUSABLE_SAMPLES),
        }
    })?;
    Ok(Prepared {
        input0: mean(&inputs[..first]),
        output0: record.output0,
        noise,
        unit,
        record,
    })
}

/// Whether the recording has enough samples, and each is good and finite, with every time after the one before it.
fn usable(samples: &[IdentificationSample]) -> bool {
    samples.len() >= MINIMUM_SAMPLES
        && samples.iter().all(|sample| {
            sample.quality == SampleQuality::Good
                && sample.time_seconds.is_finite()
                && sample.input.is_finite()
                && sample.output.is_finite()
        })
        && samples
            .windows(2)
            .all(|pair| pair[1].time_seconds > pair[0].time_seconds)
}

/// A sample as the block averaging and the fit read it. An unset clipped or saturated flag reads as not set, as the
/// `TypeScript` `combinedFlags` reads it.
fn even_sample(sample: &IdentificationSample) -> EvenSample {
    EvenSample {
        time_seconds: sample.time_seconds,
        input: sample.input,
        output: sample.output,
        quality: sample.quality,
        clipped: sample.clipped.unwrap_or(false),
        saturated: sample.saturated.unwrap_or(false),
    }
}

/// The index of the first sample past the operating point (`afterBaseline`): the first sample after the named end of the
/// baseline (with the `TypeScript`'s allowance of `10⁻⁹` s), or else the first input change. `None` when there is none.
fn after_baseline(series: &[EvenSample], baseline_end_seconds: Option<f64>) -> Option<usize> {
    match baseline_end_seconds {
        Some(end) => series
            .iter()
            .position(|sample| sample.time_seconds > end + 1e-9),
        None => first_input_change(series),
    }
}

/// The index of the first sample whose input differs from the one before it by more than `10⁻⁹` times the largest input
/// magnitude (at least one), as `firstInputChange` finds it; `None` when the input never changes.
fn first_input_change(series: &[EvenSample]) -> Option<usize> {
    let largest_magnitude = series
        .iter()
        .map(|sample| sample.input.abs())
        .fold(1.0, js_max);
    let tolerance = 1e-9 * largest_magnitude;
    series
        .windows(2)
        .position(|pair| (pair[1].input - pair[0].input).abs() > tolerance)
        .map(|index| index + 1)
}

/// The inputs of a series, in order.
fn input_values(series: &[EvenSample]) -> Vec<f64> {
    series.iter().map(|sample| sample.input).collect()
}

/// The outputs of a series, in order.
fn output_values(series: &[EvenSample]) -> Vec<f64> {
    series.iter().map(|sample| sample.output).collect()
}

/// `Math.max(...values)`: the largest value, `NaN` when any value is `NaN`, and `-∞` for no values.
fn js_max_of(values: &[f64]) -> f64 {
    values.iter().copied().fold(f64::NEG_INFINITY, js_max)
}

/// `Math.min(...values)`: the smallest value, `NaN` when any value is `NaN`, and `+∞` for no values.
fn js_min_of(values: &[f64]) -> f64 {
    values.iter().copied().fold(f64::INFINITY, js_min)
}

/// The grid fit of every form of N2.1 on the record, in the order the list gives them (`fitForms`). A form whose search
/// finds no finite gain is absent.
fn fit_forms(record: &PreparedRecord) -> Vec<FormFit> {
    LOW_ORDER_FORMS
        .into_iter()
        .filter_map(|kind| fit_form(record, kind))
        .collect()
}

/// The checked candidate of a fitted form: its model, its final-set error (N3.2), its fraction of the final change on a
/// single held step (N2.3), and the sample interval of the fitted record (N3.3).
fn checked_candidate(
    form: &FormFit,
    samples: &[IdentificationSample],
    sample_interval_seconds: f64,
) -> CheckedCandidate {
    let model = identified_model(form);
    let reached_fraction =
        held_step_evidence(&model, samples).and_then(|evidence| evidence.fraction);
    CheckedCandidate {
        model,
        final_normalized_error: form.prediction.final_normalized_error,
        reached_fraction,
        sample_interval_seconds,
    }
}

/// The A10.3 refusal of the recommended form when its held step reached less than 63 % of its fitted final change (N2.3):
/// the fraction in percent, the time held so far, and the hold the dominant time constant suggests. `None` for a record
/// that is not a single held step, and for a form whose fraction is not defined or is acceptable.
fn partial_refusal(selected: &IdentifiedModel, samples: &[IdentificationSample]) -> Option<String> {
    let evidence = held_step_evidence(selected, samples)?;
    evidence
        .fraction
        .filter(|fraction| *fraction < MINIMUM_REACHED_FRACTION)
        .map(|fraction| {
            partial_response_refusal(
                fraction,
                evidence.hold_seconds,
                selected.dead_time_seconds.unwrap_or(0.0),
                dominant_time_constant(selected),
            )
        })
}

/// The refusal when no form is within the limit: it names the form with the lowest normalised error
/// (`identifyPlantFromExcitation`).
fn no_simple_model(forms: &[FormFit]) -> Refusal {
    let mut ranked: Vec<&FormFit> = forms.iter().collect();
    ranked.sort_by(|left, right| {
        compare_errors(
            left.prediction.normalized_error,
            right.prediction.normalized_error,
        )
    });
    let detail = ranked.first().map_or_else(String::new, |best| {
        format!(
            " (best {}: normalized error {})",
            kind_words(best.kind),
            fixed_three(best.prediction.normalized_error)
        )
    });
    Refusal::new(format!(
        "No simple model explains the recorded excitation{detail}."
    ))
}

/// The order of two normalised errors, with a `NaN` equal to any value, as the `TypeScript` comparator reads it.
fn compare_errors(left: f64, right: f64) -> Ordering {
    left.partial_cmp(&right).unwrap_or(Ordering::Equal)
}

/// The form's name in words, as the refusal prints it (`kind.replace(/_/g, " ")`).
fn kind_words(kind: LowOrderModelKind) -> &'static str {
    match kind {
        LowOrderModelKind::StaticGain => "static gain",
        LowOrderModelKind::Integrating => "integrating",
        LowOrderModelKind::FirstOrder => "first order",
        LowOrderModelKind::FirstOrderDeadTime => "first order dead time",
        LowOrderModelKind::SecondOrder => "second order",
        LowOrderModelKind::SecondOrderDeadTime => "second order dead time",
    }
}

/// The recorded model of a fitted form (`identifiedModel`): its form, gain and dynamics, its normalised error as both the
/// decisive and the estimation error, and no validation data.
fn identified_model(form: &FormFit) -> IdentifiedModel {
    let time_constant_seconds = form.shape.time_constant_seconds;
    let second_time_constant_seconds = form.shape.second_time_constant_seconds;
    IdentifiedModel {
        kind: form.kind,
        gain: form.gain,
        time_constant_seconds,
        second_time_constant_seconds,
        // A dead time is reported where the form has one and it is not zero: a delay-free integrator has none.
        dead_time_seconds: form.shape.dead_time_seconds.filter(|delay| *delay > 0.0),
        normalized_error: Some(form.prediction.normalized_error),
        normalized_estimation_error: Some(form.prediction.normalized_error),
        normalized_validation_error: None,
        estimation_residuals: None,
        validation_residuals: None,
        uncertainty: None,
        stable: is_stable_form(
            form.kind,
            time_constant_seconds,
            second_time_constant_seconds,
        ),
        on_search_boundary: form.on_search_boundary,
    }
}

/// `value.toFixed(3)`: three decimals, rounded as JavaScript rounds them. A value that is not finite, or is 10²¹ or more,
/// prints as JavaScript's `ToString` gives it, as `toFixed` does.
fn fixed_three(value: f64) -> String {
    if value.is_finite() && value.abs() < 1e21 {
        format!("{:.3}", js_to_fixed(value, 3))
    } else {
        js_number(value)
    }
}

#[cfg(test)]
#[path = "excitation_tests.rs"]
pub(crate) mod tests;
