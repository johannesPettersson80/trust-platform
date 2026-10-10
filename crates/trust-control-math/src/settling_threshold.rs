//! The settling threshold of a proportional loop on a first-order plant: the settling term of the least-gain search of
//! `designSimpleProportionalController` (`simpleControllerDesign.ts`; B4.4 and C1 of specification 35). It is the smallest
//! loop gain `q = K·Kp` that brings the response into the settling band by the settling time. The caller takes the
//! larger of this threshold, its steady-error threshold and zero.

use crate::proportional::ProportionalRefusal;

/// The smallest loop gain `q = K·Kp` at which a first-order loop reaches the settling band `b` by the settling time
/// `Tset`, for the first-order plant `K/(τs + 1)`:
///
/// `q = τ·ln(M/b)/Tset − 1`, with `M = max(|Δr|, |K·u0|)`.
///
/// Derivation. With `u = Kp·(r − y)` and `q = K·Kp`, the closed-loop pole is `−(1 + q)/τ`, and the reference transient is
/// bounded by `M·exp(−(1 + q)·t/τ)`. That bound is at most `b` at `t = Tset` when `M·exp(−(1 + q)·Tset/τ) ≤ b`, which is
/// `(1 + q)·Tset/τ ≥ ln(M/b)`. The least `q` with this property is the formula. A negative value means that the bound is
/// inside the band even at `q = 0`, so the caller's zero applies.
///
/// # Errors
/// Returns [`ProportionalRefusal::InvalidInput`] unless every value is finite, `τ`, `b` and `Tset` are positive, and `M` is
/// positive. Returns [`ProportionalRefusal::NumericalUncertainty`] when the threshold is not finite.
pub fn minimum_loop_gain(
    time_constant_seconds: f64,
    reference_change: f64,
    operating_effect: f64,
    settling_band: f64,
    settling_time_seconds: f64,
) -> Result<f64, ProportionalRefusal> {
    let finite = [
        time_constant_seconds,
        reference_change,
        operating_effect,
        settling_band,
        settling_time_seconds,
    ]
    .iter()
    .all(|value| value.is_finite());
    let magnitude = reference_change.abs().max(operating_effect.abs());
    if !finite
        || time_constant_seconds <= 0.0
        || settling_band <= 0.0
        || settling_time_seconds <= 0.0
        || magnitude <= 0.0
    {
        return Err(ProportionalRefusal::InvalidInput);
    }
    let threshold =
        time_constant_seconds * (magnitude / settling_band).ln() / settling_time_seconds - 1.0;
    if threshold.is_finite() {
        Ok(threshold)
    } else {
        Err(ProportionalRefusal::NumericalUncertainty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bound `M·exp(−(1 + q)·Tset/τ)` of the derivation, at the loop gain `q`.
    fn bound_at(gain: f64, time_constant: f64, magnitude: f64, settling_time: f64) -> f64 {
        magnitude * (-(1.0 + gain) * settling_time / time_constant).exp()
    }

    #[test]
    fn the_threshold_for_a_reference_change_is_the_closed_form() {
        // τ = 2, Δr = 10, K·u0 = 0, b = 0.5 and Tset = 4: M = 10 and q = 0.5·ln(20) − 1 = 0.497866...
        let threshold = minimum_loop_gain(2.0, 10.0, 0.0, 0.5, 4.0).expect("a threshold");
        assert!((threshold - (0.5 * 20.0_f64.ln() - 1.0)).abs() <= 1e-12);
        assert!((threshold - 0.497_866_136_776_995_4).abs() <= 1e-12);
    }

    #[test]
    fn at_the_threshold_the_bound_reaches_the_band_exactly_at_the_settling_time() {
        let threshold = minimum_loop_gain(2.0, 10.0, 0.0, 0.5, 4.0).expect("a threshold");
        // The derivation: M·exp(−(1 + q)·Tset/τ) = b at the threshold.
        assert!((bound_at(threshold, 2.0, 10.0, 4.0) - 0.5).abs() <= 1e-12);
        // Any smaller loop gain leaves the bound above the band at Tset.
        assert!(bound_at(threshold - 0.01, 2.0, 10.0, 4.0) > 0.5);
    }

    #[test]
    fn the_operating_effect_sets_the_magnitude_when_it_is_larger() {
        // τ = 1.5, Δr = 1, K·u0 = −8, b = 0.5 and Tset = 3: M = 8 and q = 1.5·ln(16)/3 − 1 = 0.386294...
        let threshold = minimum_loop_gain(1.5, 1.0, -8.0, 0.5, 3.0).expect("a threshold");
        assert!((threshold - 0.386_294_361_119_890_6).abs() <= 1e-12);
        assert!((bound_at(threshold, 1.5, 8.0, 3.0) - 0.5).abs() <= 1e-12);
    }

    #[test]
    fn a_response_already_inside_the_band_has_a_negative_threshold() {
        // τ = 1, Δr = 0.1, b = 1 and Tset = 1: q = ln(0.1) − 1 = −3.302585..., so the caller's zero applies.
        let threshold = minimum_loop_gain(1.0, 0.1, 0.0, 1.0, 1.0).expect("a threshold");
        assert!((threshold - (0.1_f64.ln() - 1.0)).abs() <= 1e-12);
        assert!(threshold < 0.0);
    }

    #[test]
    fn invalid_inputs_are_refused() {
        // A zero band, a zero settling time, a zero time constant, a zero magnitude and a nonfinite input.
        assert_eq!(
            minimum_loop_gain(2.0, 10.0, 0.0, 0.0, 4.0),
            Err(ProportionalRefusal::InvalidInput)
        );
        assert_eq!(
            minimum_loop_gain(2.0, 10.0, 0.0, 0.5, 0.0),
            Err(ProportionalRefusal::InvalidInput)
        );
        assert_eq!(
            minimum_loop_gain(0.0, 10.0, 0.0, 0.5, 4.0),
            Err(ProportionalRefusal::InvalidInput)
        );
        assert_eq!(
            minimum_loop_gain(2.0, 0.0, 0.0, 0.5, 4.0),
            Err(ProportionalRefusal::InvalidInput)
        );
        assert_eq!(
            minimum_loop_gain(f64::NAN, 10.0, 0.0, 0.5, 4.0),
            Err(ProportionalRefusal::InvalidInput)
        );
    }
}
