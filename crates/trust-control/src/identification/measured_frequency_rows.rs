//! The measured tones as rows of the frequency-response table (N2.8; `measuredFrequencyRows.ts`): the tones in ascending
//! frequency, and each phase unwrapped so that it lies within 180° of the previous tone's phase. The first tone keeps its
//! principal phase.

use trust_control_math::frequency_identification::FrequencyResponsePoint;

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

/// The rows of the measured tones: sorted by ascending frequency, with each phase in degrees brought within 180° of the
/// previous row's phase (N2.8: the phase is unwrapped from the lowest tone).
#[must_use]
pub fn measured_frequency_rows(points: &[FrequencyResponsePoint]) -> Vec<MeasuredFrequencyRow> {
    let mut sorted: Vec<&FrequencyResponsePoint> = points.iter().collect();
    sorted.sort_by(|left, right| {
        left.frequency_rad_per_second
            .total_cmp(&right.frequency_rad_per_second)
    });
    let mut last_phase: Option<f64> = None;
    sorted
        .into_iter()
        .map(|point| {
            let mut phase = point.phase_radians * 180.0 / std::f64::consts::PI;
            if let Some(last) = last_phase {
                while phase - last > 180.0 {
                    phase -= 360.0;
                }
                while phase - last < -180.0 {
                    phase += 360.0;
                }
            }
            last_phase = Some(phase);
            MeasuredFrequencyRow {
                frequency: point.frequency_rad_per_second,
                magnitude: point.magnitude,
                phase_degrees: phase,
                coherence: point.coherence,
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "measured_frequency_rows_tests.rs"]
pub(crate) mod tests;
