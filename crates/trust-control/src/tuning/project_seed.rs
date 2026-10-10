//! The program's own laws (specification 35, C2 and §17.16; numerical annex N6.7): a `parallel-pid` seed names the variables
//! that hold a program's gains, so that the law can be designed on the model as a `Control.PID` is, and the trajectory
//! feedforward of a trajectory-following law. The designed gains are written to the variables the seed names; nothing here
//! replaces the program's equations.
//!
//! The other seed kinds (decentralized, cascade, feedforward, Smith predictor and decoupled groups) are not part of this
//! module; the decoder refuses them.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::identification::refusal::Refusal;
use crate::tuning::structure::{is_exactly_zero, ParallelGains, Structure};
use crate::tuning::texts;

/// A program law whose parallel gains are named by variables (C2): the family, the proportional variable, and the integral
/// and derivative variables when the law has them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParallelPidSeed {
    /// The family the law has: P, PI, PD or PID.
    pub family: Structure,
    /// The variable that holds the proportional gain.
    pub proportional_symbol: String,
    /// The variable that holds the integral gain, when the law has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integral_symbol: Option<String>,
    /// The variable that holds the derivative gain, when the law has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derivative_symbol: Option<String>,
}

/// A program's controller seed that the scalar rules design (C2, §17.16).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ProjectSeed {
    /// A parallel law with named gain variables.
    ParallelPid(ParallelPidSeed),
}

/// The gains that a seed's variables hold, from their values (`parallelSeedGains`): the proportional variable must be
/// finite, and an absent integral or derivative variable is zero.
#[must_use]
pub fn parallel_seed_gains(
    seed: &ProjectSeed,
    parameters: &BTreeMap<String, f64>,
) -> Option<ParallelGains> {
    let ProjectSeed::ParallelPid(seed) = seed;
    let proportional = parameters
        .get(&seed.proportional_symbol)
        .copied()
        .filter(|value| value.is_finite())?;
    Some(ParallelGains {
        proportional_gain: proportional,
        integral_gain: seed
            .integral_symbol
            .as_ref()
            .and_then(|symbol| parameters.get(symbol).copied())
            .unwrap_or(0.0),
        derivative_gain: seed
            .derivative_symbol
            .as_ref()
            .and_then(|symbol| parameters.get(symbol).copied())
            .unwrap_or(0.0),
    })
}

/// Checks that a seed's law has the terms a family needs (`familyGains` of the TypeScript): an integral variable for PI and
/// PID, a derivative variable for PD and PID.
///
/// # Errors
/// Returns a [`Refusal`] naming the family and the missing term.
pub fn check_family_terms(seed: &ParallelPidSeed, family: Structure) -> Result<(), Refusal> {
    let word = crate::tuning::design_to_requirements::structure_word(family);
    if family.has_integral() && seed.integral_symbol.is_none() {
        return Err(Refusal::new(texts::seed_needs_integral(word)));
    }
    if family.has_derivative() && seed.derivative_symbol.is_none() {
        return Err(Refusal::new(texts::seed_needs_derivative(word)));
    }
    Ok(())
}

/// The values of the variables a seed names for the designed gains (the return of `familyGains`): the proportional variable
/// always, and the integral and derivative variables only where the seed names them.
#[must_use]
pub fn seed_values(seed: &ParallelPidSeed, gains: ParallelGains) -> BTreeMap<String, f64> {
    let mut values = BTreeMap::from([(seed.proportional_symbol.clone(), gains.proportional_gain)]);
    if let Some(symbol) = &seed.integral_symbol {
        values.insert(symbol.clone(), gains.integral_gain);
    }
    if let Some(symbol) = &seed.derivative_symbol {
        values.insert(symbol.clone(), gains.derivative_gain);
    }
    values
}

/// The rate and acceleration feedforward of `n/(a·s² + b·s)` (`trajectoryFeedforward`): `K_v = b/n` and `K_a = a/n`, the
/// output with which the model follows the trajectory exactly (C11). A zero numerator, a zero `b`, or a value that is not
/// finite has no such feedforward.
///
/// # Errors
/// Returns a [`Refusal`] with the trajectory sentence for a model without that form.
pub fn trajectory_feedforward(numerator: f64, a: f64, b: f64) -> Result<(f64, f64), Refusal> {
    let usable = [numerator, a, b].iter().all(|value| value.is_finite())
        && !is_exactly_zero(numerator)
        && !is_exactly_zero(b);
    if usable {
        Ok((b / numerator, a / numerator))
    } else {
        Err(Refusal::new(texts::TRAJECTORY_REFUSED))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)] // The feedforward gains are exact quotients of exact inputs.

    use super::*;

    fn seed() -> ParallelPidSeed {
        ParallelPidSeed {
            family: Structure::PI,
            proportional_symbol: "Main.Kp".to_string(),
            integral_symbol: Some("Main.Ki".to_string()),
            derivative_symbol: None,
        }
    }

    #[test]
    fn the_gains_are_read_from_the_named_variables() {
        let parameters =
            BTreeMap::from([("Main.Kp".to_string(), 2.0), ("Main.Ki".to_string(), 0.5)]);
        let gains =
            parallel_seed_gains(&ProjectSeed::ParallelPid(seed()), &parameters).expect("bound");
        assert_eq!(gains.proportional_gain, 2.0);
        assert_eq!(gains.integral_gain, 0.5);
        assert_eq!(gains.derivative_gain, 0.0);
    }

    #[test]
    fn a_pi_law_without_an_integral_variable_is_refused() {
        let mut without = seed();
        without.integral_symbol = None;
        assert_eq!(
            check_family_terms(&without, Structure::PI)
                .expect_err("no integral")
                .0,
            "A PI design needs an integral gain; the program's law has no integral gain."
        );
    }

    #[test]
    fn the_designed_gains_are_written_to_the_named_variables_only() {
        let gains = ParallelGains {
            proportional_gain: 1.5,
            integral_gain: 0.3,
            derivative_gain: 0.0,
        };
        let values = seed_values(&seed(), gains);
        assert_eq!(values.len(), 2);
        assert_eq!(values["Main.Ki"], 0.3);
    }

    #[test]
    fn the_trajectory_feedforward_is_b_over_n_and_a_over_n() {
        // n/(a s² + b s) with n = 2, a = 0.5, b = 4: K_v = 4/2 = 2, K_a = 0.5/2 = 0.25.
        assert_eq!(
            trajectory_feedforward(2.0, 0.5, 4.0).expect("integrator"),
            (2.0, 0.25)
        );
        assert!(trajectory_feedforward(0.0, 0.5, 4.0).is_err());
        assert!(trajectory_feedforward(2.0, 0.5, 0.0).is_err());
    }
}
