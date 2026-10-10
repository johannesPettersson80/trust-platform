#![allow(clippy::float_cmp)] // Block inputs, means of whole numbers and exact limits are compared exactly on purpose.

use std::collections::VecDeque;

use super::*;

/// A good sample with no units and no clipped or saturated flag.
pub(crate) fn good_sample(time_seconds: f64, input: f64, output: f64) -> IdentificationSample {
    IdentificationSample {
        time_seconds,
        input,
        output,
        quality: SampleQuality::Good,
        input_unit: None,
        output_unit: None,
        clipped: None,
        saturated: None,
    }
}

/// The first-order plant with a 0.5 s delay of the identification tests (`fopdtSamples`): gain 1.5, time constant 4 s,
/// sampled every 0.1 s for 30 s, with an input step of `amplitude` at 2 s.
pub(crate) fn fopdt_samples(amplitude: f64) -> Vec<IdentificationSample> {
    let dt = 0.1;
    let mut delayed = VecDeque::from([0.0; 5]);
    let mut pv = 0.0;
    let mut samples = Vec::new();
    for index in 0..=300_u32 {
        let input = if index >= 20 { amplitude } else { 0.0 };
        delayed.push_back(input);
        let applied = delayed.pop_front().unwrap_or(0.0);
        pv += dt * (1.5 * applied - pv) / 4.0;
        samples.push(good_sample(f64::from(index) * dt, input, pv));
    }
    samples
}

// control-studio-identify-tune.test.ts: "qualifies data and excludes intervals without changing the raw recording".
#[test]
fn qualifies_data_and_excludes_intervals_without_changing_the_raw_recording() {
    let raw = fopdt_samples(2.0);
    let qualified = qualify_identification_data(
        &raw,
        &[ExcludedInterval {
            start_seconds: 8.0,
            end_seconds: 9.0,
        }],
        false,
    );
    assert_eq!(raw.len(), 301);
    assert_eq!(qualified.raw_samples.len(), 301);
    assert_eq!(
        qualified.raw_samples, raw,
        "the raw recording is kept unchanged"
    );
    assert!(qualified.samples.len() < raw.len());
    assert!(qualified.suitable);
    assert_eq!(qualified.input_unit, "unit");
    assert_eq!(qualified.output_unit, "unit");
    assert!(qualified.sample_time_seconds > 0.0);
    let keys = [
        "rawSampleCount",
        "retainedSampleCount",
        "durationSeconds",
        "medianSampleTimeSeconds",
        "maximumSampleTimeDeviationPercent",
        "gapCount",
        "rejectedQualitySampleCount",
        "clippedSampleCount",
        "saturationSampleCount",
        "baselineDriftPerSecond",
        "inputSpan",
        "outputSpan",
        "signalToNoiseDb",
        "excitation",
        "coherence",
        "independentValidationAvailable",
    ];
    let text = serde_json::to_string(&qualified.metrics).expect("the metrics serialize");
    let positions: Vec<usize> = keys
        .iter()
        .map(|key| {
            text.find(&format!("\"{key}\":"))
                .unwrap_or_else(|| panic!("the metrics name {key}: {text}"))
        })
        .collect();
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "the metrics keep the order of the summary: {text}"
    );
    let value: serde_json::Value = serde_json::from_str(&text).expect("the metrics read back");
    assert_eq!(
        value.as_object().map(serde_json::Map::len),
        Some(keys.len()),
        "the metrics hold exactly the summary's fields: {text}"
    );
    let metrics = &qualified.metrics;
    assert_eq!(metrics.raw_sample_count, 301);
    assert_eq!(metrics.retained_sample_count, qualified.samples.len());
    assert!(metrics.duration_seconds > 20.0);
    assert_eq!(
        metrics.gap_count, 1,
        "excluding a disturbed interval must leave one visible timing gap"
    );
    assert!(metrics.input_span > 0.0);
    assert!(metrics.output_span > 0.0);
    assert!(metrics.signal_to_noise_db.is_finite());
    assert_eq!(metrics.excitation, Excitation::Step);
    assert_eq!(
        metrics.coherence,
        "Not evaluated for time-domain excitation"
    );
    assert!(!metrics.independent_validation_available);
    assert!(!qualified.suitable_for_design);
}

// control-studio-identify-tune.test.ts: "rejects unusable data with a deterministic reason and retains it". Its
// `identifyPlant` assertion belongs to the tests of `plant`.
#[test]
fn rejects_unusable_data_with_a_deterministic_reason_and_retains_it() {
    let samples = [
        good_sample(0.0, 1.0, 1.0),
        IdentificationSample {
            quality: SampleQuality::Bad,
            ..good_sample(1.0, 1.0, f64::NAN)
        },
    ];
    let qualification = qualify_identification_data(&samples, &[], false);
    assert!(!qualification.suitable);
    let reason = qualification.reason.as_deref().unwrap_or_default();
    assert!(
        reason.to_lowercase().contains("finite good samples"),
        "{reason}"
    );
    assert_eq!(qualification.raw_samples.len(), 2);
}

// control-studio-jittered-sampling.test.ts: "block-averages a long recording without mixing the two sides of a step".
// The `TypeScript` test calls `blockAveraged` with the step's time as the cut; `fit_recording` takes that cut from the
// largest input change, the step at 30 s.
#[test]
fn block_averages_a_long_recording_without_mixing_the_two_sides_of_a_step() {
    let samples: Vec<IdentificationSample> = (0..12_000_u32)
        .map(|index| {
            let input = if index < 3_000 { 0.0 } else { 1.0 };
            good_sample(f64::from(index) / 100.0, input, f64::from(index) / 100.0)
        })
        .collect();
    let averaged = fit_recording(&samples);
    assert!(
        averaged.len() <= MAXIMUM_FIT_SAMPLES,
        "{} blocks",
        averaged.len()
    );
    assert!(
        averaged
            .iter()
            .all(|sample| sample.input == 0.0 || sample.input == 1.0),
        "no block mixes the two sides of the step"
    );
    assert!(
        averaged
            .windows(2)
            .all(|pair| pair[1].time_seconds > pair[0].time_seconds),
        "block times stay ordered"
    );
}

#[test]
fn a_fit_block_keeps_the_units_of_its_first_member_and_combines_its_flags() {
    let mut samples: Vec<IdentificationSample> = (0..5_001_u32)
        .map(|index| good_sample(f64::from(index), 1.0, f64::from(index)))
        .collect();
    samples[0].input_unit = Some("%".to_string());
    samples[1].input_unit = Some("kW".to_string());
    samples[1].clipped = Some(true);
    let fitted = fit_recording(&samples);
    assert_eq!(
        fitted.len(),
        2_501,
        "blocks of two, and the last sample alone"
    );
    assert_eq!(fitted[0].time_seconds, 0.5);
    assert_eq!(fitted[0].input_unit.as_deref(), Some("%"));
    assert_eq!(fitted[0].clipped, Some(true));
    assert_eq!(fitted[0].quality, SampleQuality::Bad);
    assert_eq!(fitted[1].clipped, Some(false));
    assert_eq!(fitted[1].saturated, Some(false));
    assert_eq!(fitted[1].quality, SampleQuality::Good);
    assert_eq!(fitted[2_500].time_seconds, 5_000.0);
    let short = &samples[..3];
    assert_eq!(
        fit_recording(short),
        short.to_vec(),
        "a recording within the limit is fitted unchanged"
    );
}

#[test]
fn the_held_step_ends_at_the_restoration_of_its_input() {
    let samples: Vec<IdentificationSample> = [0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0]
        .iter()
        .zip(0_u32..)
        .map(|(&input, index)| good_sample(f64::from(index), input, 0.0))
        .collect();
    assert_eq!(
        held_step_samples(&samples).len(),
        5,
        "the restoration and the samples after it are not held"
    );
    assert_eq!(
        held_step_samples(&samples[..5]).len(),
        5,
        "a step that is never restored is held whole"
    );
    let constant = [good_sample(0.0, 1.0, 0.0), good_sample(1.0, 1.0, 0.0)];
    assert_eq!(held_step_samples(&constant).len(), 2);
}

#[test]
fn the_operating_point_is_the_mean_before_the_largest_input_change() {
    let samples = [
        good_sample(0.0, 0.0, 1.0),
        good_sample(1.0, 0.0, 3.0),
        good_sample(2.0, 2.0, 5.0),
        good_sample(3.0, 2.0, 7.0),
    ];
    assert_eq!(
        identification_operating_point(&samples),
        Ok(OperatingPoint {
            step_index: 2,
            input0: 0.0,
            output0: 2.0,
        })
    );
    let finite = Refusal::new("Model evidence requires finite measured samples.");
    assert_eq!(
        identification_operating_point(&samples[..1]),
        Err(finite.clone())
    );
    let mut missing = samples;
    missing[3].output = f64::NAN;
    assert_eq!(identification_operating_point(&missing), Err(finite));
    let constant = [good_sample(0.0, 1.0, 1.0), good_sample(1.0, 1.0, 2.0)];
    assert_eq!(
        identification_operating_point(&constant),
        Err(Refusal::new(
            "Model evidence requires a measured input change."
        ))
    );
}

#[test]
fn the_median_interval_and_the_same_recording_follow_the_recorded_samples() {
    let samples = [0.0, 1.0, 1.0, 3.0, 4.0].map(|time| good_sample(time, 0.0, 0.0));
    assert_eq!(
        median_interval(&samples),
        1.0,
        "repeated times are no interval"
    );
    assert_eq!(median_interval(&samples[..1]), 0.0);
    let recording = fopdt_samples(2.0);
    assert!(same_recording(&recording, &fopdt_samples(2.0)));
    assert!(!same_recording(&recording, &fopdt_samples(1.25)));
    assert!(!same_recording(&recording, &recording[1..]));
    let mut relabelled = recording.clone();
    relabelled[0].quality = SampleQuality::Uncertain;
    assert!(!same_recording(&recording, &relabelled));
}
