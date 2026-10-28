//! The measured tones as rows of the frequency-response table (N2.8; `measuredFrequencyRows.ts`): the tones in ascending
//! frequency, with the phase unwrapped from the lowest tone by the plotted-phase rule of N4.3. A measured response has no
//! known delay, so the rule applies to its whole phase: the first tone keeps its principal phase, and each further tone's
//! step from the previous tone lies in (−180°, 180°].

use trust_control_math::frequency::plotted_phase_degrees;
use trust_control_math::frequency_identification::FrequencyResponsePoint;
use trust_control_math::linalg::ComplexValue;

/// One row of the measured frequency response (`MeasuredFrequencyRow`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasuredFrequencyRow {
    /// The tone's angular frequency in radians per second.
    pub frequency: f64,
    /// The magnitude `|H(jω)|`.
    pub magnitude: f64,
    /// The unwrapped phase in degrees.
    pub phase_degrees: f64,
    /// The coherence `γ²(ω)`.
    pub coherence: f64,
}

/// The rows of the measured tones: sorted by ascending frequency, with the phase in degrees unwrapped from the lowest tone
/// (N2.8, N4.3).
#[must_use]
pub fn measured_frequency_rows(points: &[FrequencyResponsePoint]) -> Vec<MeasuredFrequencyRow> {
    let mut sorted: Vec<&FrequencyResponsePoint> = points.iter().collect();
    sorted.sort_by(|left, right| {
        left.frequency_rad_per_second
            .total_cmp(&right.frequency_rad_per_second)
    });
    let responses: Vec<ComplexValue> = sorted.iter().map(|point| point.response).collect();
    let frequencies: Vec<f64> = sorted
        .iter()
        .map(|point| point.frequency_rad_per_second)
        .collect();
    let phases = plotted_phase_degrees(&responses, &frequencies, 0.0);
    sorted
        .into_iter()
        .zip(phases)
        .map(|(point, phase_degrees)| MeasuredFrequencyRow {
            frequency: point.frequency_rad_per_second,
            magnitude: point.magnitude,
            phase_degrees,
            coherence: point.coherence,
        })
        .collect()
}

#[cfg(test)]
#[path = "measured_frequency_rows_tests.rs"]
pub(crate) mod tests;
