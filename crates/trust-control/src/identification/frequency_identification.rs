//! The frequency-response estimate of a periodic recording (N2.8, N4.3; `estimatePeriodicFrequency` in
//! `threePageAnalysisOperations.ts`): the startup samples are dropped from both records, and the estimate of
//! `trust_control_math::frequency_identification` runs on what remains. Its refusals are the typed reasons of the math, and
//! `frequency_texts` says them in words.

use trust_control_math::frequency_identification::{
    estimate_frequency_response, FrequencyRejection, FrequencyResponsePoint,
};

/// Estimates the frequency response of a periodic recording after its startup samples (`estimatePeriodicFrequency`).
///
/// The first `startup_samples` samples of both records are dropped. The estimate uses whole segments, so the samples left
/// over at the end are not used.
///
/// # Errors
/// Returns [`FrequencyRejection::InsufficientData`] when the two records differ in length or when `startup_samples` is not
/// less than their length, and otherwise the refusal of the estimate itself (N2.8).
pub fn estimate_periodic_frequency(
    input: &[f64],
    output: &[f64],
    sample_time_seconds: f64,
    frequencies_rad_per_second: &[f64],
    segment_count: usize,
    minimum_coherence: f64,
    startup_samples: usize,
) -> Result<Vec<FrequencyResponsePoint>, FrequencyRejection> {
    if input.len() != output.len() || startup_samples >= input.len() {
        return Err(FrequencyRejection::InsufficientData);
    }
    estimate_frequency_response(
        &input[startup_samples..],
        &output[startup_samples..],
        sample_time_seconds,
        frequencies_rad_per_second,
        segment_count,
        minimum_coherence,
    )
}

#[cfg(test)]
#[path = "frequency_identification_tests.rs"]
pub(crate) mod tests;
