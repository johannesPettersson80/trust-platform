//! Pole placement of Automatic tuning (specification 35, B4.4 and C1; numerical annex N5.12(c)): the parallel gains
//! `Kp`, `Ki` and `Kd` (N0.5) of a P, PI, PD or PID controller that place a dominant pole pair with damping `ζ` and
//! decay rate `σ` on a delay-free first- or second-order plant without zeros. The matching is that of `poleMatchedPid`
//! in `pidTaskDesign.ts`, except that the decay rate is an input: a first-order proportional pole takes the rate of
//! N5.12(c) for each placement, not the fixed 2 % envelope of that function.
//!
//! The plant is `G(s) = b/D(s)` and the controller is `C(s) = Kp + Ki/s + Kd·s`. The closed-loop characteristic polynomial
//! is `s·D(s) + b·(Kd·s² + Kp·s + Ki)` with an integral term, and `D(s) + b·(Kd·s + Kp)` without one. Each structure
//! matches the coefficients that its gains can set, and the plant fixes the others. The sequence of placements (the
//! envelopes, the 25 % steps and at most eight tries) belongs to the caller, which checks each placement by its prediction.

#![allow(clippy::module_name_repetitions)] // `PlacementPlant`, `PlacementRequest` and `PlacementRefusal` name the module's subject.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::TransferFunction;
use crate::polynomial::{add_aligned, convolve, trim_leading_zeros};

/// The third real pole of PI and PID on a second-order plant sits at `3ω`, which completes the matched polynomial as
/// `(s² + 2ζω·s + ω²)(s + 3ω)`. The factor is a choice of the coefficient matching and is not stated in the specification.
const EXTRA_POLE_FACTOR: f64 = 3.0;

/// The margin of the Routh test on a cubic: `a1·a2 > a0·a3·(1 + 10⁻¹⁰)`, which is the test of `pidTaskDesign.ts`.
const ROUTH_MARGIN: f64 = 1.0 + 1e-10;

/// The controller structure: which parallel terms it keeps (N0.5). A term that the structure does not keep is zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// Proportional only.
    P,
    /// Proportional and integral.
    PI,
    /// Proportional and derivative.
    PD,
    /// Proportional, integral and derivative.
    PID,
}

/// The parallel gains of a controller, `u = Kp·e + Ki·∫e dt + Kd·de/dt` (N0.5). A term that the family does not keep is zero.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ParallelGains {
    /// The proportional gain `Kp`.
    pub proportional_gain: f64,
    /// The integral gain `Ki = Kp / Ti`.
    pub integral_gain: f64,
    /// The derivative gain `Kd`.
    pub derivative_gain: f64,
}

/// What one placement asks for (N5.12(c)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacementRequest {
    /// The decay rate `σ` of the dominant pair, in 1/s. The first placement uses `max(4, ln(|Δr|/b))/Tset`.
    pub decay_rate: f64,
    /// The damping `ζ` of the dominant pair. B4.4 requires 0.8.
    pub damping: f64,
    /// The smallest closed-loop stiffness `a0 + b·Kp` that the caller requires, for P and PD only. The steady error must
    /// stay within its allowance, so the caller passes `N/allowance`, with `N` the numerator of the steady-error bound.
    /// Zero when there is no such requirement. PI and PID keep their matched poles and do not use it.
    pub minimum_stiffness: f64,
    /// The controller's task period `h` in seconds, the actual interval (N0.6).
    pub sample_time_seconds: f64,
    /// The sample-rate limit: `ω·h` must stay below it. N5.12(c) sets it to 0.2.
    pub maximum_omega_sample_product: f64,
}

impl PlacementRequest {
    /// Whether every value is finite, and the decay rate, damping, sample time and limit are positive, and the
    /// minimum stiffness is not negative.
    fn is_valid(&self) -> bool {
        [
            self.decay_rate,
            self.damping,
            self.minimum_stiffness,
            self.sample_time_seconds,
            self.maximum_omega_sample_product,
        ]
        .iter()
        .all(|value| value.is_finite())
            && self.decay_rate > 0.0
            && self.damping > 0.0
            && self.minimum_stiffness >= 0.0
            && self.sample_time_seconds > 0.0
            && self.maximum_omega_sample_product > 0.0
    }
}

/// A plant that pole placement accepts: delay-free, first or second order, without zeros, with a positive leading
/// coefficient, `G(s) = b/D(s)` (B4.4, C1). Build it with [`PlacementPlant::new`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlacementPlant {
    /// `G(s) = b/(a·s + a0)`.
    FirstOrder {
        /// The plant gain `b`, nonzero.
        gain: f64,
        /// The leading denominator coefficient `a`, positive.
        leading: f64,
        /// The constant denominator coefficient `a0`.
        constant: f64,
    },
    /// `G(s) = b/(a·s² + a1·s + a0)`.
    SecondOrder {
        /// The plant gain `b`, nonzero.
        gain: f64,
        /// The leading denominator coefficient `a`, positive.
        leading: f64,
        /// The linear denominator coefficient `a1`.
        linear: f64,
        /// The constant denominator coefficient `a0`.
        constant: f64,
    },
}

/// Why a placement gives no gains (B4.4, N5.12(c), C1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PlacementRefusal {
    /// A value is not finite, or a request value is outside its domain.
    #[error("a value is not finite or a request value is outside its domain")]
    InvalidInput,
    /// The plant has a pure delay, and placement needs a delay-free plant.
    #[error("the plant has a pure delay")]
    Delay,
    /// The numerator has a variable part, so the plant has zeros.
    #[error("the plant has zeros")]
    Zeros,
    /// The plant is neither first nor second order.
    #[error("the plant is neither first nor second order")]
    Order,
    /// The plant gain is zero.
    #[error("the plant gain is zero")]
    ZeroGain,
    /// The leading denominator coefficient is not positive.
    #[error("the leading denominator coefficient is not positive")]
    LeadingCoefficient,
    /// The placement's `ω·h` is not below the sample-rate limit (N5.12(c)).
    #[error("the placement is at or above the sample-rate limit")]
    SampleRate,
    /// A derivative term is not justified on a first-order plant (C1).
    #[error("a derivative term is not justified on a first-order plant")]
    DerivativeOnFirstOrder,
    /// Proportional action cannot change the decay of a second-order plant, and the plant's own decay is below the
    /// requested decay rate (C1).
    #[error("the plant's own decay is below the requested decay rate, and proportional action cannot change it")]
    ProportionalDecayTooSlow,
    /// A second-order plant without positive damping gives integral action no damping to use (C1).
    #[error("the second-order plant has no positive damping for the integral action")]
    IntegralWithoutDamping,
    /// The fixed damping of integral action gives a pole pair slower than the one requested (C1).
    #[error("the fixed damping of the integral action gives a pole pair slower than requested")]
    IntegralTooSlow,
    /// A placed gain is not finite.
    #[error("a placed gain is not finite")]
    NonfiniteGain,
    /// The matched characteristic polynomial is not strictly stable.
    #[error("the matched characteristic polynomial is not strictly stable")]
    NotStable,
    /// A placed gain has the sign opposite to the plant gain, outside the magnitude frame (N5.12(c), N6).
    #[error("a placed gain is negative in the magnitude frame")]
    NegativeGain,
}

impl PlacementPlant {
    /// Checks a transfer function for pole placement and returns the plant it describes.
    ///
    /// # Errors
    /// Returns [`PlacementRefusal::InvalidInput`] for a nonfinite coefficient or delay,
    /// [`PlacementRefusal::Delay`] for a delay other than zero, [`PlacementRefusal::Zeros`] for a numerator with a
    /// variable part, [`PlacementRefusal::ZeroGain`] for a zero gain, [`PlacementRefusal::LeadingCoefficient`] for a
    /// leading denominator coefficient that is not positive, and [`PlacementRefusal::Order`] for a denominator that is
    /// neither first nor second order.
    pub fn new(transfer: &TransferFunction, delay_seconds: f64) -> Result<Self, PlacementRefusal> {
        let numerator = trim_leading_zeros(&transfer.numerator);
        let denominator = trim_leading_zeros(&transfer.denominator);
        let finite = delay_seconds.is_finite()
            && numerator
                .iter()
                .chain(&denominator)
                .all(|value| value.is_finite());
        if !finite {
            return Err(PlacementRefusal::InvalidInput);
        }
        if !is_exact_zero(delay_seconds) {
            return Err(PlacementRefusal::Delay);
        }
        let &[gain] = numerator.as_slice() else {
            return Err(PlacementRefusal::Zeros);
        };
        if is_exact_zero(gain) {
            return Err(PlacementRefusal::ZeroGain);
        }
        match *denominator.as_slice() {
            [leading, constant] => {
                check_leading(leading)?;
                Ok(Self::FirstOrder {
                    gain,
                    leading,
                    constant,
                })
            }
            [leading, linear, constant] => {
                check_leading(leading)?;
                Ok(Self::SecondOrder {
                    gain,
                    leading,
                    linear,
                    constant,
                })
            }
            _ => Err(PlacementRefusal::Order),
        }
    }

    /// The plant gain `b`.
    fn gain(self) -> f64 {
        match self {
            Self::FirstOrder { gain, .. } | Self::SecondOrder { gain, .. } => gain,
        }
    }

    /// The denominator `D(s)` with its coefficients from the highest power down.
    fn denominator(self) -> Vec<f64> {
        match self {
            Self::FirstOrder {
                leading, constant, ..
            } => vec![leading, constant],
            Self::SecondOrder {
                leading,
                linear,
                constant,
                ..
            } => vec![leading, linear, constant],
        }
    }
}

/// Places the poles of `family` on `plant` (B4.4, N5.12(c), C1) and returns the parallel gains that match the
/// characteristic polynomial for the dominant pair with damping `ζ = request.damping` and decay rate `σ = request.decay_rate`,
/// so that `ω = σ/ζ`.
///
/// Write `a`, `a1` and `a0` for the leading, linear and constant denominator coefficients, `b` for the plant gain, and
/// `S = a0 + b·Kp` for the proportional stiffness. The matched identities are:
///
/// - P, first order: `a·s + S = a·(s + σ)`, so `S = max(a·σ, minimum_stiffness)`, and `Kp = (S − a0)/b`.
/// - PI, first order: `a·s² + S·s + b·Ki = a·(s² + 2ζω·s + ω²)`, so `Kp = (2ζω·a − a0)/b` and `Ki = a·ω²/b`.
/// - P, second order: `S = a·ω²`, so `Kp = (a·ω² − a0)/b`. Proportional action changes no other coefficient, so the
///   pair keeps the plant's own decay `a1/(2a)`, which must be at least `σ`.
/// - PD, second order: `S = max(a·ω², minimum_stiffness)` sets the natural frequency `ω_r = √(S/a)`, and
///   `a·s² + (a1 + b·Kd)·s + S = a·(s² + 2ζω_r·s + ω_r²)`, so `Kp = (S − a0)/b` and `Kd = (2ζω_r·a − a1)/b`.
/// - PI, second order: `a·s³ + a1·s² + S·s + b·Ki = a·(s² + 2ζw·s + w²)(s + 3w)` with `a1 = a·w·(2ζ + 3)`. Integral
///   action can place only this pair, `w = a1/(a·(2ζ + 3))`, and the pair must be at least as fast as `ω`. Then
///   `Kp = (a·w²·(1 + 6ζ) − a0)/b` and `Ki = 3·a·w³/b`.
/// - PID, second order: `a·s³ + (a1 + b·Kd)·s² + S·s + b·Ki = a·(s² + 2ζω·s + ω²)(s + 3ω)`, so
///   `Kd = (a·(2ζω + 3ω) − a1)/b`, `Kp = (a·(ω² + 6ζω²) − a0)/b` and `Ki = 3·a·ω³/b`.
///
/// A placement is refused when `ω·h` is not below the sample-rate limit, when the matched polynomial is not strictly
/// stable, and when a gain has the sign opposite to the plant gain (the magnitude frame of N5.12(c) and N6).
///
/// # Errors
/// Returns [`PlacementRefusal::InvalidInput`] when a request value is nonfinite or outside its domain,
/// [`PlacementRefusal::SampleRate`] when `ω·h` is at or above the limit, and
/// [`PlacementRefusal::DerivativeOnFirstOrder`], [`PlacementRefusal::ProportionalDecayTooSlow`],
/// [`PlacementRefusal::IntegralWithoutDamping`] or [`PlacementRefusal::IntegralTooSlow`] when the structure cannot place
/// the pole pair on this plant. It also returns [`PlacementRefusal::NonfiniteGain`], [`PlacementRefusal::NotStable`] and
/// [`PlacementRefusal::NegativeGain`] for the checks above.
pub fn place_poles(
    plant: PlacementPlant,
    family: Family,
    request: PlacementRequest,
) -> Result<ParallelGains, PlacementRefusal> {
    if !request.is_valid() {
        return Err(PlacementRefusal::InvalidInput);
    }
    let omega = request.decay_rate / request.damping;
    if omega * request.sample_time_seconds >= request.maximum_omega_sample_product {
        return Err(PlacementRefusal::SampleRate);
    }
    let parallel = match plant {
        PlacementPlant::FirstOrder {
            gain,
            leading,
            constant,
        } => first_order_gains(family, gain, leading, constant, request)?,
        PlacementPlant::SecondOrder {
            gain,
            leading,
            linear,
            constant,
        } => second_order_gains(family, gain, leading, linear, constant, request)?,
    };
    let terms = [
        parallel.proportional_gain,
        parallel.integral_gain,
        parallel.derivative_gain,
    ];
    if !terms.iter().all(|value| value.is_finite()) {
        return Err(PlacementRefusal::NonfiniteGain);
    }
    let gain = plant.gain();
    let polynomial = closed_loop_polynomial(
        &plant.denominator(),
        gain,
        parallel,
        matches!(family, Family::PI | Family::PID),
    );
    if !is_strictly_stable(&polynomial) {
        return Err(PlacementRefusal::NotStable);
    }
    let plant_sign = gain.signum();
    if terms.iter().any(|value| value * plant_sign < 0.0) {
        return Err(PlacementRefusal::NegativeGain);
    }
    Ok(parallel)
}

/// The gains of a first-order plant `b/(a·s + a0)` for `family`, from the matched identities of [`place_poles`].
fn first_order_gains(
    family: Family,
    gain: f64,
    leading: f64,
    constant: f64,
    request: PlacementRequest,
) -> Result<ParallelGains, PlacementRefusal> {
    let omega = request.decay_rate / request.damping;
    match family {
        Family::P => {
            let stiffness = (leading * request.decay_rate).max(request.minimum_stiffness);
            Ok(ParallelGains {
                proportional_gain: (stiffness - constant) / gain,
                integral_gain: 0.0,
                derivative_gain: 0.0,
            })
        }
        Family::PI => Ok(ParallelGains {
            proportional_gain: (2.0 * request.damping * omega * leading - constant) / gain,
            integral_gain: leading * omega * omega / gain,
            derivative_gain: 0.0,
        }),
        Family::PD | Family::PID => Err(PlacementRefusal::DerivativeOnFirstOrder),
    }
}

/// The gains of a second-order plant `b/(a·s² + a1·s + a0)` for `family`, from the matched identities of [`place_poles`].
fn second_order_gains(
    family: Family,
    gain: f64,
    leading: f64,
    linear: f64,
    constant: f64,
    request: PlacementRequest,
) -> Result<ParallelGains, PlacementRefusal> {
    let damping = request.damping;
    let omega = request.decay_rate / damping;
    match family {
        Family::P => {
            if linear / (2.0 * leading) < request.decay_rate {
                return Err(PlacementRefusal::ProportionalDecayTooSlow);
            }
            let stiffness = (leading * omega * omega).max(request.minimum_stiffness);
            Ok(ParallelGains {
                proportional_gain: (stiffness - constant) / gain,
                integral_gain: 0.0,
                derivative_gain: 0.0,
            })
        }
        Family::PI => {
            if linear <= 0.0 {
                return Err(PlacementRefusal::IntegralWithoutDamping);
            }
            let achieved = linear / (leading * (2.0 * damping + EXTRA_POLE_FACTOR));
            if achieved < omega {
                return Err(PlacementRefusal::IntegralTooSlow);
            }
            let cube = achieved * achieved * achieved;
            Ok(ParallelGains {
                proportional_gain: (leading
                    * achieved
                    * achieved
                    * (1.0 + 2.0 * damping * EXTRA_POLE_FACTOR)
                    - constant)
                    / gain,
                integral_gain: leading * EXTRA_POLE_FACTOR * cube / gain,
                derivative_gain: 0.0,
            })
        }
        Family::PD => {
            let stiffness = (leading * omega * omega).max(request.minimum_stiffness);
            let realized = (stiffness / leading).sqrt();
            Ok(ParallelGains {
                proportional_gain: (stiffness - constant) / gain,
                integral_gain: 0.0,
                derivative_gain: (2.0 * damping * realized * leading - linear) / gain,
            })
        }
        Family::PID => {
            let extra = EXTRA_POLE_FACTOR * omega;
            Ok(ParallelGains {
                proportional_gain: (leading * (omega * omega + 2.0 * damping * omega * extra)
                    - constant)
                    / gain,
                integral_gain: leading * omega * omega * extra / gain,
                derivative_gain: (leading * (2.0 * damping * omega + extra) - linear) / gain,
            })
        }
    }
}

/// The characteristic polynomial of the loop `1 + C(s)·b/D(s) = 0` with the parallel controller `C(s) = Kp + Ki/s + Kd·s`
/// (N0.5), with descending coefficients: `s·D(s) + b·(Kd·s² + Kp·s + Ki)` with an integral term, and
/// `D(s) + b·(Kd·s + Kp)` without one.
fn closed_loop_polynomial(
    denominator: &[f64],
    gain: f64,
    parallel: ParallelGains,
    integral: bool,
) -> Vec<f64> {
    if integral {
        let controller = [
            gain * parallel.derivative_gain,
            gain * parallel.proportional_gain,
            gain * parallel.integral_gain,
        ];
        add_aligned(&convolve(&[1.0, 0.0], denominator), &controller)
    } else {
        let controller = [
            gain * parallel.derivative_gain,
            gain * parallel.proportional_gain,
        ];
        add_aligned(denominator, &controller)
    }
}

/// Whether a characteristic polynomial of degree at most three, with descending coefficients, is strictly stable: every
/// coefficient is positive, and a cubic also passes the Routh product `a1·a2 > a0·a3` with its margin.
fn is_strictly_stable(polynomial: &[f64]) -> bool {
    polynomial.iter().all(|coefficient| *coefficient > 0.0)
        && (polynomial.len() != 4
            || polynomial[1] * polynomial[2] > polynomial[0] * polynomial[3] * ROUTH_MARGIN)
}

/// A leading denominator coefficient must be positive.
fn check_leading(leading: f64) -> Result<(), PlacementRefusal> {
    if leading > 0.0 {
        Ok(())
    } else {
        Err(PlacementRefusal::LeadingCoefficient)
    }
}

/// Whether a value is exactly zero. A zero gain and a zero delay are refused on exact zero (N5.7, C1), not on a tolerance.
#[allow(clippy::float_cmp)] // Exact zero is the refusal's meaning, not a tolerance.
fn is_exact_zero(value: f64) -> bool {
    value == 0.0
}

#[cfg(test)]
mod tests;
