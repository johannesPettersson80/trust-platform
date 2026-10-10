//! The notch filter of a project tuning component (`componentChoices` in `projectTuningComponents.ts`; C5 of
//! specification 35). The notch is a discrete second-order section whose zeros lie on the unit circle at the centre
//! frequency. Its gain there is zero, its gain at zero frequency and at the Nyquist frequency is one, and its quality
//! factor sets its width. The rounding to the parameters' types, their bounds and their pins belong to the caller.

#![allow(clippy::module_name_repetitions)] // `NotchCoefficients` and `NotchRefusal` name the module's subject.

use std::f64::consts::PI;

/// The margin with which the section's denominator must be strictly stable (`componentChoices`: `1e-12`).
const STABILITY_MARGIN: f64 = 1e-12;

/// The coefficients of a notch in the direct form with minus feedback:
/// `y[n] = b0·u[n] + b1·u[n−1] + b2·u[n−2] − a1·y[n−1] − a2·y[n−2]`, so that
/// `H(z) = (b0 + b1·z⁻¹ + b2·z⁻²)/(1 + a1·z⁻¹ + a2·z⁻²)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NotchCoefficients {
    /// The feed-forward coefficients `[b0, b1, b2]`.
    pub feed_forward: [f64; 3],
    /// The feedback coefficients `[a1, a2]`, in the minus form above.
    pub feedback: [f64; 2],
}

/// Why a notch cannot be designed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NotchRefusal {
    /// A value is not finite, or the quality factor or the sample time is not positive.
    #[error("a value is not finite, or the quality factor or sample time is not positive")]
    InvalidInput,
    /// The centre frequency is not positive, or it is not below the sampled Nyquist frequency `1/(2h)`.
    #[error("the centre frequency is not positive or not below the sampled Nyquist frequency")]
    FrequencyOutOfRange,
}

impl NotchCoefficients {
    /// Whether the section is strictly stable, by the Jury conditions of its denominator `1 + a1·z⁻¹ + a2·z⁻²`:
    /// `1 + a1 + a2 > 0`, `1 − a1 + a2 > 0` and `1 − a2 > 0`, each with the margin of `componentChoices`.
    #[must_use]
    pub fn is_stable(&self) -> bool {
        let [first, second] = self.feedback;
        1.0 + first + second > STABILITY_MARGIN
            && 1.0 - first + second > STABILITY_MARGIN
            && 1.0 - second > STABILITY_MARGIN
    }
}

/// The notch at `centre_frequency_hz` with quality factor `quality`, for a task of period `sample_time_seconds`.
///
/// With `ω = 2π·f·h` and `α = sin(ω)/(2Q)`, the coefficients are
///
/// `b0 = b2 = 1/(1 + α)`, `b1 = a1 = −2·cos(ω)/(1 + α)` and `a2 = (1 − α)/(1 + α)`.
///
/// The zeros are `exp(±jω)`, so the gain at the centre frequency is zero. The gain at zero frequency and at the Nyquist
/// frequency is one, and `Q` sets the width of the notch.
///
/// # Errors
/// Returns [`NotchRefusal::InvalidInput`] when a value is not finite, or when `Q` or `h` is not positive, and
/// [`NotchRefusal::FrequencyOutOfRange`] when `ω` is not in `(0, π)`, that is when the centre frequency is not positive
/// and below `1/(2h)`.
pub fn design(
    centre_frequency_hz: f64,
    quality: f64,
    sample_time_seconds: f64,
) -> Result<NotchCoefficients, NotchRefusal> {
    let finite =
        centre_frequency_hz.is_finite() && quality.is_finite() && sample_time_seconds.is_finite();
    if !finite || quality <= 0.0 || sample_time_seconds <= 0.0 {
        return Err(NotchRefusal::InvalidInput);
    }
    let omega = 2.0 * PI * centre_frequency_hz * sample_time_seconds;
    if omega <= 0.0 || omega >= PI {
        return Err(NotchRefusal::FrequencyOutOfRange);
    }
    let alpha = omega.sin() / (2.0 * quality);
    let denominator = 1.0 + alpha;
    let cosine = -2.0 * omega.cos() / denominator;
    Ok(NotchCoefficients {
        feed_forward: [1.0 / denominator, cosine, 1.0 / denominator],
        feedback: [cosine, (1.0 - alpha) / denominator],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    /// The magnitude `|H(e^{jω})|` of a section from its coefficients: the numerator `b0 + b1·e^{−jω} + b2·e^{−2jω}`
    /// over the denominator `1 + a1·e^{−jω} + a2·e^{−2jω}`.
    fn magnitude_at(section: NotchCoefficients, omega: f64) -> f64 {
        let [b0, b1, b2] = section.feed_forward;
        let [a1, a2] = section.feedback;
        let (cos_once, sin_once) = (omega.cos(), omega.sin());
        let (cos_twice, sin_twice) = ((2.0 * omega).cos(), (2.0 * omega).sin());
        let numerator = (b0 + b1 * cos_once + b2 * cos_twice).hypot(b1 * sin_once + b2 * sin_twice);
        let denominator =
            (1.0 + a1 * cos_once + a2 * cos_twice).hypot(a1 * sin_once + a2 * sin_twice);
        numerator / denominator
    }

    #[test]
    fn a_quarter_sample_notch_at_quality_one_has_the_closed_form_coefficients() {
        // f·h = 1/4 gives ω = π/2, so cos ω = 0 and sin ω = 1. With Q = 1, α = 1/2 and 1 + α = 3/2:
        // b0 = b2 = 2/3, b1 = a1 = 0 and a2 = (1 − α)/(1 + α) = 1/3.
        let section = design(0.25, 1.0, 1.0).expect("a notch");
        assert_close(section.feed_forward[0], 2.0 / 3.0, 1e-12);
        assert_close(section.feed_forward[1], 0.0, 1e-12);
        assert_close(section.feed_forward[2], 2.0 / 3.0, 1e-12);
        assert_close(section.feedback[0], 0.0, 1e-12);
        assert_close(section.feedback[1], 1.0 / 3.0, 1e-12);
    }

    #[test]
    fn the_gain_at_the_centre_frequency_is_the_depth_and_far_from_it_the_gain_is_one() {
        // The depth of this notch is zero, because its zeros lie on the unit circle at the centre frequency.
        // Its gain at zero frequency and at the Nyquist frequency is one, for every centre and width.
        for (frequency_hz, quality, sample_time) in [(1.0 / 6.0, 0.5, 1.0), (50.0, 10.0, 0.001)] {
            let section = design(frequency_hz, quality, sample_time).expect("a notch");
            let centre = 2.0 * PI * frequency_hz * sample_time;
            assert_close(magnitude_at(section, centre), 0.0, 1e-12);
            assert_close(magnitude_at(section, 0.0), 1.0, 1e-12);
            assert_close(magnitude_at(section, PI), 1.0, 1e-12);
        }
    }

    #[test]
    fn the_sixty_degree_notch_has_the_reference_coefficients() {
        // f·h = 1/6 gives ω = π/3, and Q = 0.5 gives α = sin(π/3) = √3/2.
        let section = design(1.0 / 6.0, 0.5, 1.0).expect("a notch");
        assert_close(section.feed_forward[0], 0.535_898_384_862_245_4, 1e-12);
        assert_close(section.feed_forward[1], -0.535_898_384_862_245_5, 1e-12);
        assert_close(section.feed_forward[2], 0.535_898_384_862_245_4, 1e-12);
        assert_close(section.feedback[0], -0.535_898_384_862_245_5, 1e-12);
        assert_close(section.feedback[1], 0.071_796_769_724_490_85, 1e-12);
    }

    #[test]
    fn a_fifty_hertz_mains_notch_at_one_kilohertz_has_the_reference_coefficients() {
        // f = 50 Hz at h = 1 ms gives ω = π/10, and Q = 10 gives a narrow mains notch.
        let section = design(50.0, 10.0, 0.001).expect("a notch");
        assert_close(section.feed_forward[0], 0.984_784_246_600_387_5, 1e-12);
        assert_close(section.feed_forward[1], -1.873_170_949_748_224, 1e-12);
        assert_close(section.feed_forward[2], 0.984_784_246_600_387_5, 1e-12);
        assert_close(section.feedback[0], -1.873_170_949_748_224, 1e-12);
        assert_close(section.feedback[1], 0.969_568_493_200_775_2, 1e-12);
    }

    #[test]
    fn a_centre_frequency_at_or_above_nyquist_or_not_positive_is_refused() {
        // At f = 1/(2h) the frequency is the Nyquist frequency itself, where ω = π.
        assert_eq!(
            design(0.5, 1.0, 1.0),
            Err(NotchRefusal::FrequencyOutOfRange)
        );
        assert_eq!(
            design(0.6, 1.0, 1.0),
            Err(NotchRefusal::FrequencyOutOfRange)
        );
        assert_eq!(
            design(0.0, 1.0, 1.0),
            Err(NotchRefusal::FrequencyOutOfRange)
        );
    }

    #[test]
    fn a_nonpositive_quality_or_sample_time_or_a_nonfinite_value_is_refused() {
        assert_eq!(design(0.25, 0.0, 1.0), Err(NotchRefusal::InvalidInput));
        assert_eq!(design(0.25, 1.0, 0.0), Err(NotchRefusal::InvalidInput));
        assert_eq!(design(f64::NAN, 1.0, 1.0), Err(NotchRefusal::InvalidInput));
    }

    #[test]
    fn a_notch_is_stable_and_an_unstable_section_is_reported() {
        let section = design(50.0, 10.0, 0.001).expect("a notch");
        assert!(section.is_stable());
        // 1 − a2 = −0.5 is not positive, so the section is unstable.
        let unstable = NotchCoefficients {
            feed_forward: [1.0, 0.0, 0.0],
            feedback: [0.0, 1.5],
        };
        assert!(!unstable.is_stable());
    }
}
