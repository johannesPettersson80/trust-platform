//! Tests of the frequency-response estimate (N2.8): a closed-form first-order response recovered from a multisine, the
//! refusals with their limits, and the coherence of a noisy output. The TypeScript cases are `control-studio-numerics.test.ts`
//! ("generates bounded excitation and recovers a frequency response with coherence", and the low-coherence case of "fails
//! closed for noisy, poorly excited ...").

#![allow(clippy::cast_precision_loss)] // Sample indices and counts are far below 2⁵³.

use std::f64::consts::PI;

use super::{estimate_frequency_response, FrequencyRejection};

/// The sample interval of the multisine tests, in seconds.
const SAMPLE_TIME: f64 = 0.05;
/// The record length: four segments of 1,000 samples, each 50 s long, so every tone holds whole periods per segment.
const SAMPLES: usize = 4_000;
const SEGMENTS: usize = 4;
/// The tones, in cycles per 50 s segment, and the phases of their inputs.
const BINS: [f64; 4] = [4.0, 8.0, 16.0, 24.0];
const PHASES: [f64; 4] = [0.1, 0.7, 1.4, 2.2];

/// The angular frequencies of the tones: `ω = 2πk / 50` rad/s.
fn tones() -> Vec<f64> {
    BINS.iter().map(|bin| 2.0 * PI * bin / 50.0).collect()
}

/// A deterministic pseudo-random value in `[−1, 1)`, from the `SplitMix64` finalizer of the index: broadband, so a tone's bin
/// holds its share of the noise power. (A single multiply-add of the index is a structured sequence with almost no power at
/// the test tone.)
fn noise(index: usize) -> f64 {
    let mut mixed = (index as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^= mixed >> 31;
    ((mixed >> 11) as f64) / ((1_u64 << 53) as f64) * 2.0 - 1.0
}

/// The input of the multisine: unit tones with the phases of [`PHASES`].
fn multisine_input() -> Vec<f64> {
    let frequencies = tones();
    (0..SAMPLES)
        .map(|index| {
            let time = index as f64 * SAMPLE_TIME;
            frequencies
                .iter()
                .zip(PHASES)
                .map(|(frequency, phase)| (frequency * time + phase).sin())
                .sum::<f64>()
        })
        .collect()
}

/// The steady output of `G(s) = K / (τs + 1)` to the multisine input: each tone is scaled by `|G|` and shifted by `∠G`.
fn first_order_output(gain: f64, time_constant: f64) -> Vec<f64> {
    let frequencies = tones();
    (0..SAMPLES)
        .map(|index| {
            let time = index as f64 * SAMPLE_TIME;
            frequencies
                .iter()
                .zip(PHASES)
                .map(|(frequency, phase)| {
                    let denominator = 1.0 + (time_constant * frequency).powi(2);
                    let magnitude = gain / denominator.sqrt();
                    let shift = -(time_constant * frequency).atan();
                    magnitude * (frequency * time + phase + shift).sin()
                })
                .sum::<f64>()
        })
        .collect()
}

/// One refused input of the fit: the input, the output, the sample time, the frequencies, the settle samples, the noise and the
/// rejection expected.
type RejectionCase<'a> = (
    &'a [f64],
    &'a [f64],
    f64,
    &'a [f64],
    usize,
    f64,
    FrequencyRejection,
);
#[test]
fn a_first_order_response_is_recovered_from_a_multisine() {
    // G(jω) = K/(1 + jωτ) with K = 2 and τ = 3: the estimate must reproduce G at every tone, with coherence one.
    let (gain, time_constant) = (2.0, 3.0);
    let input = multisine_input();
    let output = first_order_output(gain, time_constant);
    let points =
        estimate_frequency_response(&input, &output, SAMPLE_TIME, &tones(), SEGMENTS, 0.98)
            .expect("the exact multisine response is qualified");
    assert_eq!(points.len(), tones().len());
    for (point, frequency) in points.iter().zip(tones()) {
        let denominator = 1.0 + (time_constant * frequency).powi(2);
        let expected_real = gain / denominator;
        let expected_imaginary = -gain * time_constant * frequency / denominator;
        assert!((point.response.real - expected_real).abs() < 1e-9);
        assert!((point.response.imaginary - expected_imaginary).abs() < 1e-9);
        assert!((point.magnitude - gain / denominator.sqrt()).abs() < 1e-9);
        assert!((point.phase_radians + (time_constant * frequency).atan()).abs() < 1e-9);
        assert!((point.coherence - 1.0).abs() < 1e-9);
        assert!((point.frequency_rad_per_second - frequency).abs() < 1e-15);
    }
}

#[test]
fn a_tone_at_the_nyquist_frequency_is_refused() {
    let input = multisine_input();
    let refusal = estimate_frequency_response(
        &input,
        &input,
        SAMPLE_TIME,
        &[PI / SAMPLE_TIME],
        SEGMENTS,
        0.8,
    );
    assert_eq!(refusal, Err(FrequencyRejection::AliasRisk));
}

#[test]
fn an_output_dominated_by_noise_is_refused_below_the_minimum_coherence() {
    // The input is one tone; the output is a 1 % response buried in unit noise, so the coherence is far below 0.8.
    let frequency = tones()[0];
    let input: Vec<f64> = (0..SAMPLES)
        .map(|index| (frequency * index as f64 * SAMPLE_TIME).sin())
        .collect();
    let output: Vec<f64> = input
        .iter()
        .enumerate()
        .map(|(index, value)| 0.01 * value + noise(index))
        .collect();
    let refusal =
        estimate_frequency_response(&input, &output, SAMPLE_TIME, &[frequency], SEGMENTS, 0.8);
    assert_eq!(refusal, Err(FrequencyRejection::LowCoherence));
}

#[test]
fn every_refusal_names_the_rule_it_breaks() {
    let input = multisine_input();
    let frequency = tones();
    let short = vec![0.0; 2 * 16 - 1];
    let zeros = vec![0.0; SAMPLES];
    let cases: [RejectionCase<'_>; 9] = [
        (
            &input[..SAMPLES - 1],
            &input,
            SAMPLE_TIME,
            &frequency,
            SEGMENTS,
            0.8,
            FrequencyRejection::InconsistentDimensions,
        ),
        (
            &input,
            &input,
            0.0,
            &frequency,
            SEGMENTS,
            0.8,
            FrequencyRejection::NonfiniteInput,
        ),
        (
            &[f64::NAN; 64],
            &[0.0; 64],
            SAMPLE_TIME,
            &frequency,
            2,
            0.8,
            FrequencyRejection::NonfiniteInput,
        ),
        (
            &short,
            &short,
            SAMPLE_TIME,
            &frequency,
            2,
            0.8,
            FrequencyRejection::InsufficientData,
        ),
        (
            &input,
            &input,
            SAMPLE_TIME,
            &frequency,
            1,
            0.8,
            FrequencyRejection::InsufficientData,
        ),
        (
            &input,
            &input,
            SAMPLE_TIME,
            &frequency,
            SEGMENTS,
            1.5,
            FrequencyRejection::InsufficientData,
        ),
        (
            &input,
            &input,
            SAMPLE_TIME,
            &[],
            SEGMENTS,
            0.8,
            FrequencyRejection::InsufficientData,
        ),
        (
            &zeros,
            &input,
            SAMPLE_TIME,
            &frequency,
            SEGMENTS,
            0.8,
            FrequencyRejection::PoorExcitation,
        ),
        (
            &input,
            &input,
            SAMPLE_TIME,
            &[0.0],
            SEGMENTS,
            0.8,
            FrequencyRejection::AliasRisk,
        ),
    ];
    for (case, (input, output, sample_time, tones, segments, minimum, expected)) in
        cases.into_iter().enumerate()
    {
        let refusal =
            estimate_frequency_response(input, output, sample_time, tones, segments, minimum);
        assert_eq!(refusal, Err(expected), "case {case}");
    }
}
