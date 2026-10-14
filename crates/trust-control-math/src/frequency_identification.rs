//! The frequency-response estimate of N2.8 (`estimateFrequencyResponse` in `numerics/frequencyIdentification.ts`).
//!
//! The record is split into equal segments. In each segment the input and the output are fitted, at each requested
//! tone, by least squares with a constant and a cosine and sine of that tone. The fitted coefficients are the tone's
//! discrete spectrum on that segment, and the H1 estimate and the magnitude-squared coherence are formed from the sums
//! over the segments:
//!
//! ```text
//! H(jω) = Σ Syu(jω) / Σ Suu(jω)        γ²(ω) = |Σ Syu(jω)|² / ( Σ Suu(jω) · Σ Syy(jω) )
//! ```
//!
//! Each tone is evaluated directly, so no transform of the whole record is needed (the FFT of the checklist's D6 is
//! not used). The fitted coefficient of a segment is exact for a sinusoid over whole periods and stays exact about any
//! operating point when the segment holds a part period.

use thiserror::Error;

use crate::linalg::ComplexValue;

/// Why a frequency-response estimate is refused. Each refusal names the rule it breaks (N2.8, N4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum FrequencyRejection {
    /// The input and output records have different lengths.
    #[error("the input and output records differ in length")]
    InconsistentDimensions,
    /// The sample time is not a positive finite number, or a sample, a tone or the coherence limit is not finite.
    #[error("a value is not finite, or the sample time is not positive")]
    NonfiniteInput,
    /// The record is shorter than sixteen samples per segment, the coherence limit lies outside `[0, 1]`, or no tone
    /// is requested.
    #[error(
        "the record is too short for the segments, or the coherence limit or the tones are invalid"
    )]
    InsufficientData,
    /// A tone is not positive or is at or above the Nyquist frequency `π / Ts` (N4.3).
    #[error("a tone is not positive or is at or above the Nyquist frequency")]
    AliasRisk,
    /// The input has no energy at a tone, so the response there is undefined.
    #[error("the input has no energy at a tone")]
    PoorExcitation,
    /// A tone's coherence is below the minimum coherence (N2.8).
    #[error("a tone's coherence is below the minimum")]
    LowCoherence,
}

/// The estimated response at one tone (`FrequencyResponsePoint`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrequencyResponsePoint {
    /// The tone's angular frequency in radians per second.
    pub frequency_rad_per_second: f64,
    /// The complex response `H(jω)`.
    pub response: ComplexValue,
    /// The magnitude `|H(jω)|`.
    pub magnitude: f64,
    /// The phase `atan2(Im H, Re H)` in radians, in `(−π, π]`.
    pub phase_radians: f64,
    /// The magnitude-squared coherence `γ²(ω)`, at most one.
    pub coherence: f64,
}

/// Estimates the H1 frequency response at each requested tone (N2.8).
///
/// The record is cut into `segment_count` segments of `⌊N / segment_count⌋` samples each, and the samples left over
/// at the end are not used. The points are returned in the order of `frequencies_rad_per_second`.
///
/// # Errors
/// Returns [`FrequencyRejection::InconsistentDimensions`] for records of different lengths,
/// [`FrequencyRejection::NonfiniteInput`] for a non-finite value or a sample time that is not positive,
/// [`FrequencyRejection::InsufficientData`] when there are fewer than sixteen samples per segment, the coherence limit
/// is outside `[0, 1]` or no tone is given, [`FrequencyRejection::AliasRisk`] for a tone outside `(0, π / Ts)`,
/// [`FrequencyRejection::PoorExcitation`] when the input has no energy at a tone, and
/// [`FrequencyRejection::LowCoherence`] when a tone's coherence is below `minimum_coherence`.
pub fn estimate_frequency_response(
    input: &[f64],
    output: &[f64],
    sample_time_seconds: f64,
    frequencies_rad_per_second: &[f64],
    segment_count: usize,
    minimum_coherence: f64,
) -> Result<Vec<FrequencyResponsePoint>, FrequencyRejection> {
    if input.len() != output.len() {
        return Err(FrequencyRejection::InconsistentDimensions);
    }
    let values_finite = input
        .iter()
        .chain(output)
        .chain(frequencies_rad_per_second)
        .all(|value| value.is_finite())
        && minimum_coherence.is_finite();
    if !(sample_time_seconds.is_finite() && sample_time_seconds > 0.0 && values_finite) {
        return Err(FrequencyRejection::NonfiniteInput);
    }
    if segment_count < 2
        || !(0.0..=1.0).contains(&minimum_coherence)
        || frequencies_rad_per_second.is_empty()
        || input.len() < segment_count.saturating_mul(16)
    {
        return Err(FrequencyRejection::InsufficientData);
    }
    let nyquist = std::f64::consts::PI / sample_time_seconds;
    if frequencies_rad_per_second
        .iter()
        .any(|&frequency| frequency <= 0.0 || frequency >= nyquist)
    {
        return Err(FrequencyRejection::AliasRisk);
    }
    let segment_length = input.len() / segment_count;
    let mut points = Vec::with_capacity(frequencies_rad_per_second.len());
    for &frequency in frequencies_rad_per_second {
        let mut input_power = 0.0;
        let mut output_power = 0.0;
        let mut cross = ComplexValue {
            real: 0.0,
            imaginary: 0.0,
        };
        for segment in 0..segment_count {
            let start = segment * segment_length;
            let input_coefficient =
                tone_coefficient(input, start, segment_length, sample_time_seconds, frequency);
            let output_coefficient = tone_coefficient(
                output,
                start,
                segment_length,
                sample_time_seconds,
                frequency,
            );
            input_power += magnitude_squared(input_coefficient);
            output_power += magnitude_squared(output_coefficient);
            cross = add(
                cross,
                multiply(output_coefficient, conjugate(input_coefficient)),
            );
        }
        if input_power <= f64::EPSILON {
            return Err(FrequencyRejection::PoorExcitation);
        }
        let response = ComplexValue {
            real: cross.real / input_power,
            imaginary: cross.imaginary / input_power,
        };
        let coherence =
            (magnitude_squared(cross) / (input_power * output_power).max(f64::EPSILON)).min(1.0);
        points.push(FrequencyResponsePoint {
            frequency_rad_per_second: frequency,
            response,
            magnitude: response.real.hypot(response.imaginary),
            phase_radians: response.imaginary.atan2(response.real),
            coherence,
        });
    }
    if points
        .iter()
        .any(|point| point.coherence < minimum_coherence)
    {
        return Err(FrequencyRejection::LowCoherence);
    }
    Ok(points)
}

/// The complex coefficient of one tone over one segment, as a discrete Fourier bin `a − jb`: the least-squares fit of
/// `k + a·cos θ + b·sin θ` to the segment, where `θ = ω·t_n` and `t_n = n·Ts`. A segment whose fit is singular has
/// no coefficient, which is zero.
#[allow(clippy::cast_precision_loss)] // A sample index is far below 2⁵³, so its value as a float is exact.
fn tone_coefficient(
    values: &[f64],
    start: usize,
    length: usize,
    sample_time_seconds: f64,
    frequency: f64,
) -> ComplexValue {
    let mut count = 0.0;
    let mut sum_cos = 0.0;
    let mut sum_sin = 0.0;
    let mut sum_cos2 = 0.0;
    let mut sum_sin2 = 0.0;
    let mut sum_cos_sin = 0.0;
    let mut sum_value = 0.0;
    let mut sum_value_cos = 0.0;
    let mut sum_value_sin = 0.0;
    for (index, value) in values.iter().copied().enumerate().skip(start).take(length) {
        let angle = frequency * (index as f64) * sample_time_seconds;
        let (sin, cos) = angle.sin_cos();
        count += 1.0;
        sum_cos += cos;
        sum_sin += sin;
        sum_cos2 += cos * cos;
        sum_sin2 += sin * sin;
        sum_cos_sin += cos * sin;
        sum_value += value;
        sum_value_cos += value * cos;
        sum_value_sin += value * sin;
    }
    let matrix = [
        [count, sum_cos, sum_sin],
        [sum_cos, sum_cos2, sum_cos_sin],
        [sum_sin, sum_cos_sin, sum_sin2],
    ];
    let determinant = determinant3(&matrix);
    if !determinant.is_finite() || determinant.abs() <= 1e-12 * count * count * count {
        return ComplexValue {
            real: 0.0,
            imaginary: 0.0,
        };
    }
    let right = [sum_value, sum_value_cos, sum_value_sin];
    let a = determinant3(&replace_column(&matrix, 1, &right)) / determinant;
    let b = determinant3(&replace_column(&matrix, 2, &right)) / determinant;
    ComplexValue {
        real: a,
        imaginary: -b,
    }
}

/// The determinant of a 3 × 3 matrix by cofactor expansion along its first row.
fn determinant3(matrix: &[[f64; 3]; 3]) -> f64 {
    matrix[0][0] * (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1])
        - matrix[0][1] * (matrix[1][0] * matrix[2][2] - matrix[1][2] * matrix[2][0])
        + matrix[0][2] * (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0])
}

/// The matrix with one column replaced by a vector (Cramer's rule).
fn replace_column(matrix: &[[f64; 3]; 3], column: usize, vector: &[f64; 3]) -> [[f64; 3]; 3] {
    let mut replaced = *matrix;
    for (row, value) in vector.iter().enumerate() {
        replaced[row][column] = *value;
    }
    replaced
}

fn magnitude_squared(value: ComplexValue) -> f64 {
    value.real * value.real + value.imaginary * value.imaginary
}

fn conjugate(value: ComplexValue) -> ComplexValue {
    ComplexValue {
        real: value.real,
        imaginary: -value.imaginary,
    }
}

fn add(left: ComplexValue, right: ComplexValue) -> ComplexValue {
    ComplexValue {
        real: left.real + right.real,
        imaginary: left.imaginary + right.imaginary,
    }
}

fn multiply(left: ComplexValue, right: ComplexValue) -> ComplexValue {
    ComplexValue {
        real: left.real * right.real - left.imaginary * right.imaginary,
        imaginary: left.real * right.imaginary + left.imaginary * right.real,
    }
}

#[cfg(test)]
#[path = "frequency_identification_tests.rs"]
mod tests;
