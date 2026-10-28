//! The test signals of N2.8 (`generatePrbs` and `generateMultisine` in `numerics/frequencyIdentification.ts`).
//!
//! A pseudo-random binary sequence is the output of a maximal-length linear feedback shift register of a declared order,
//! read at its least significant bit, so it repeats every `2^order − 1` values and has a two-valued amplitude. A multisine
//! is the sum of declared sinusoids, scaled by one common factor when its peak would exceed the declared bound. Both are
//! pure functions of their declared inputs: the same declaration always gives the same sequence.

use std::f64::consts::PI;

use thiserror::Error;

/// Why a test signal cannot be generated (N2.8, N4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ExcitationRejection {
    /// A declared value is outside its range: an order outside 2 to 16, fewer than two values, a non-positive or non-finite
    /// amplitude, a seed whose low bits are zero, fewer than eight samples, or a non-finite or non-positive tone or bound.
    #[error("the excitation configuration is invalid")]
    InvalidConfiguration,
    /// A multisine tone is at or above the Nyquist frequency `π / Ts` (N2.8).
    #[error("a tone is at or above the Nyquist frequency")]
    AliasRisk,
}

/// A generated test signal: its values and the common scale applied to them (`scale` is one for a PRBS).
#[derive(Debug, Clone, PartialEq)]
pub struct Excitation {
    /// The values of the signal, one per sample.
    pub values: Vec<f64>,
    /// The common factor applied to the raw signal; one unless the peak was bounded.
    pub scale: f64,
}

/// One sinusoid of a multisine: `amplitude · sin(ω·t + φ)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tone {
    /// The angular frequency `ω` in radians per second; it must be positive and below the Nyquist frequency.
    pub frequency_rad_per_second: f64,
    /// The amplitude, which must be positive.
    pub amplitude: f64,
    /// The phase `φ` in radians.
    pub phase_radians: f64,
}

/// Generates a pseudo-random binary sequence (N2.8, `generatePrbs`).
///
/// The register holds `order` bits and starts at `seed` masked to those bits; each step outputs `+amplitude` when its least
/// significant bit is one and `−amplitude` otherwise, then shifts the register with the feedback taken from the primitive
/// polynomial of the order. The taps are those of a maximal-length register, so the sequence has period `2^order − 1`.
///
/// # Errors
/// Returns [`ExcitationRejection::InvalidConfiguration`] when `order` is outside `2..=16`, when `length` is below two, when
/// `amplitude` is not a positive finite number, or when the masked seed is zero.
pub fn generate_prbs(
    order: u32,
    length: usize,
    amplitude: f64,
    seed: i64,
) -> Result<Excitation, ExcitationRejection> {
    if !(2..=16).contains(&order) || length < 2 || !(amplitude.is_finite() && amplitude > 0.0) {
        return Err(ExcitationRejection::InvalidConfiguration);
    }
    let mask = (1_i64 << order) - 1;
    let mut state = seed & mask;
    if state == 0 {
        return Err(ExcitationRejection::InvalidConfiguration);
    }
    let taps = primitive_taps(order);
    let mut values = Vec::with_capacity(length);
    for _ in 0..length {
        values.push(if state & 1 == 1 {
            amplitude
        } else {
            -amplitude
        });
        let feedback = taps
            .iter()
            .fold(0_i64, |bit, &tap| bit ^ ((state >> tap) & 1));
        state = ((state >> 1) | (feedback << (order - 1))) & mask;
    }
    Ok(Excitation { values, scale: 1.0 })
}

/// The feedback taps of a maximal-length register of each order (bit positions counted from zero).
fn primitive_taps(order: u32) -> &'static [u32] {
    match order {
        2 => &[0, 1],
        3 => &[0, 2],
        4 => &[0, 3],
        5 => &[1, 4],
        6 => &[0, 5],
        7 => &[0, 6],
        8 => &[1, 2, 3, 7],
        9 => &[3, 8],
        10 => &[2, 9],
        11 => &[1, 10],
        12 => &[0, 3, 5, 11],
        13 => &[0, 2, 3, 12],
        14 => &[0, 2, 4, 13],
        15 => &[0, 14],
        _ => &[1, 2, 4, 15],
    }
}

/// Generates a multisine (N2.8, `generateMultisine`): `values[n] = Σₖ Aₖ·sin(ωₖ·n·Ts + φₖ)` for `n = 0, …, N − 1`.
///
/// When the largest absolute value exceeds `maximum_absolute_value`, every value is multiplied by the one factor
/// `maximum_absolute_value / peak`, which is the `scale` returned.
///
/// # Errors
/// Returns [`ExcitationRejection::InvalidConfiguration`] when there is no tone, when a tone has a non-finite value or a
/// non-positive frequency or amplitude, when the sample time is not a positive finite number, when `sample_count` is below
/// eight, or when `maximum_absolute_value` is not a positive finite number. Returns [`ExcitationRejection::AliasRisk`] when
/// a tone is at or above `π / Ts`.
pub fn generate_multisine(
    tones: &[Tone],
    sample_time_seconds: f64,
    sample_count: usize,
    maximum_absolute_value: f64,
) -> Result<Excitation, ExcitationRejection> {
    let valid = !tones.is_empty()
        && sample_time_seconds.is_finite()
        && sample_time_seconds > 0.0
        && sample_count >= 8
        && maximum_absolute_value.is_finite()
        && maximum_absolute_value > 0.0
        && tones.iter().all(|tone| {
            tone.frequency_rad_per_second.is_finite()
                && tone.amplitude.is_finite()
                && tone.phase_radians.is_finite()
                && tone.frequency_rad_per_second > 0.0
                && tone.amplitude > 0.0
        });
    if !valid {
        return Err(ExcitationRejection::InvalidConfiguration);
    }
    let nyquist = PI / sample_time_seconds;
    if tones
        .iter()
        .any(|tone| tone.frequency_rad_per_second >= nyquist)
    {
        return Err(ExcitationRejection::AliasRisk);
    }
    let raw: Vec<f64> = (0..sample_count)
        .map(|index| {
            #[allow(clippy::cast_precision_loss)] // A sample index is far below 2⁵³.
            let time = index as f64 * sample_time_seconds;
            tones.iter().fold(0.0, |sum, tone| {
                sum + tone.amplitude
                    * (tone.frequency_rad_per_second * time + tone.phase_radians).sin()
            })
        })
        .collect();
    let peak = raw
        .iter()
        .fold(f64::NEG_INFINITY, |largest, value| largest.max(value.abs()));
    let scale = if peak > maximum_absolute_value {
        maximum_absolute_value / peak
    } else {
        1.0
    };
    Ok(Excitation {
        values: raw.iter().map(|value| value * scale).collect(),
        scale,
    })
}

#[cfg(test)]
#[path = "excitation_tests.rs"]
mod tests;
