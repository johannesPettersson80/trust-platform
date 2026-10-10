//! The controller structures of a parallel `Control.PID` record (numerical annex N0.5; specification 35, C0): P, PI, PD
//! and PID, and the gains a candidate writes for them. A structure is a fact about its gains, never a name.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Which terms a controller keeps (C0, "Families and zero terms"). A term the family does not use is written as an
/// explicit zero, so the structure follows from the gains that are not zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Structure {
    /// Proportional only.
    P,
    /// Proportional and integral.
    PI,
    /// Proportional and derivative.
    PD,
    /// Proportional, integral and derivative.
    PID,
}

impl Structure {
    /// The structure of a record whose integral and derivative gains are the given values (`candidateFromControlPid`
    /// of the TypeScript): a nonzero integral gain makes an integral term, a nonzero derivative gain a derivative term.
    #[must_use]
    pub fn of_gains(integral_gain: f64, derivative_gain: f64) -> Self {
        match (
            !is_exactly_zero(integral_gain),
            !is_exactly_zero(derivative_gain),
        ) {
            (true, true) => Self::PID,
            (true, false) => Self::PI,
            (false, true) => Self::PD,
            (false, false) => Self::P,
        }
    }

    /// Whether the structure has an integral term.
    #[must_use]
    pub fn has_integral(self) -> bool {
        matches!(self, Self::PI | Self::PID)
    }

    /// Whether the structure has a derivative term.
    #[must_use]
    pub fn has_derivative(self) -> bool {
        matches!(self, Self::PD | Self::PID)
    }
}

/// The parallel gains of a candidate, signed (N6.4): `u = Kp·e + Ki·∫e dt + Kd·de/dt` (N0.5). A term that the structure
/// does not use is zero.
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

impl ParallelGains {
    /// The structure these gains make.
    #[must_use]
    pub fn structure(self) -> Structure {
        Structure::of_gains(self.integral_gain, self.derivative_gain)
    }
}

/// Whether a value is exactly zero. The rules of Tune are about exact zero (a zero gain is refused, N5.7), so the
/// comparison is exact on purpose.
#[allow(clippy::float_cmp)] // Exact zero is the rule's meaning (N5.7, C0), not a tolerance.
#[must_use]
pub fn is_exactly_zero(value: f64) -> bool {
    value == 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_structure_follows_the_nonzero_terms() {
        assert_eq!(Structure::of_gains(0.0, 0.0), Structure::P);
        assert_eq!(Structure::of_gains(0.5, 0.0), Structure::PI);
        assert_eq!(Structure::of_gains(0.0, 0.5), Structure::PD);
        assert_eq!(Structure::of_gains(0.5, 0.5), Structure::PID);
    }

    #[test]
    fn a_structure_names_the_terms_it_keeps() {
        assert!(Structure::PI.has_integral());
        assert!(!Structure::PI.has_derivative());
        assert!(Structure::PD.has_derivative());
        assert!(!Structure::PD.has_integral());
        assert!(!Structure::P.has_integral() && !Structure::P.has_derivative());
    }
}
