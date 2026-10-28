//! The candidate of a program's own law (specification 35, C2, C11 and B4.1): the gains a design chose, the structure they give, and the
//! value of each seed variable that Apply writes. It is unapplied, as a `Control.PID` candidate is (N8.9). The program's law has no
//! record of its own, so the candidate holds the seed's variables and nothing else (`seed_values`). A trajectory-following law's
//! candidate also holds its rate and acceleration feedforward gains, by their variables, beside the PID gains (C11).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::loop_definition::LoopDefinition;
use crate::tuning::candidate::CandidateSource;
use crate::tuning::project_seed::{
    FeedforwardPidSeed, FeedforwardTerms, ParallelPidSeed, SmithPidSeed, SmithTerms,
    TrajectoryFeedforwardSeed,
};
use crate::tuning::structure::{ParallelGains, Structure};

/// A candidate of a program's own law (C2, C11), unapplied: where it came from, its structure and gains, and the value of each seed
/// variable that Apply writes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectCandidate {
    /// Always false: a candidate is never applied by Tune (N8.9).
    pub applied: bool,
    /// Where the candidate came from.
    pub source: CandidateSource,
    /// The structure, from the gains that are not zero.
    pub structure: Structure,
    /// The gains, signed. A term that the structure does not use is zero.
    pub gains: ParallelGains,
    /// The value of each variable of the structure, by symbol: the proportional variable always, and the integral and derivative
    /// variables where the structure has those terms and the seed names them. A trajectory-following law also holds its rate and
    /// acceleration feedforward gains by their variables (C11). Apply writes an explicit zero to every other variable that the seed
    /// names (C0, C2), so this map does not hold those zeros.
    pub values: BTreeMap<String, f64>,
}

impl ProjectCandidate {
    /// Whether every gain and every value is finite, as a stored candidate must be (B10.4).
    #[must_use]
    pub fn is_finite(&self) -> bool {
        let gains = self.gains;
        [
            gains.proportional_gain,
            gains.integral_gain,
            gains.derivative_gain,
        ]
        .iter()
        .chain(self.values.values())
        .all(|value| value.is_finite())
    }
}

/// The PID part of the `parallel-pid` or trajectory seed of a loop's program law, when the loop declares one that the engine serves (C2,
/// C11). A loop with a controller law has none, and so does a block whose seed is parked or missing: the parked-feature gate refuses
/// those by name.
#[must_use]
pub fn served_seed(loop_definition: &LoopDefinition) -> Option<&ParallelPidSeed> {
    if loop_definition.controller.law.is_some() {
        return None;
    }
    loop_definition
        .controller
        .project
        .as_ref()?
        .seed
        .as_ref()?
        .pid_seed()
}

/// The trajectory feedforward seed of a loop's program law (C11), when the loop declares one. A loop with a controller law has none.
#[must_use]
pub fn served_trajectory(loop_definition: &LoopDefinition) -> Option<&TrajectoryFeedforwardSeed> {
    if loop_definition.controller.law.is_some() {
        return None;
    }
    loop_definition.controller.project.as_ref()?.trajectory()
}

/// The feedforward seed of a loop's program law (C8), when the loop declares one. A loop with a controller law has none.
#[must_use]
pub fn served_feedforward(loop_definition: &LoopDefinition) -> Option<&FeedforwardPidSeed> {
    if loop_definition.controller.law.is_some() {
        return None;
    }
    loop_definition.controller.project.as_ref()?.feedforward()
}

/// The Smith predictor's seed of a loop's program law (C9), when the loop declares one. A loop with a controller law has none.
#[must_use]
pub fn served_smith(loop_definition: &LoopDefinition) -> Option<&SmithPidSeed> {
    if loop_definition.controller.law.is_some() {
        return None;
    }
    loop_definition.controller.project.as_ref()?.smith()
}

/// The candidate of a program's own law for the gains a design chose (C2): the structure the gains give, and the variables of that
/// structure with their values (`values`).
#[must_use]
pub fn project_candidate(
    seed: &ParallelPidSeed,
    source: CandidateSource,
    gains: ParallelGains,
) -> ProjectCandidate {
    let structure = Structure::of_gains(gains.integral_gain, gains.derivative_gain);
    let mut values = BTreeMap::from([(seed.proportional_symbol.clone(), gains.proportional_gain)]);
    let integral = seed
        .integral_symbol
        .as_ref()
        .filter(|_| structure.has_integral());
    if let Some(symbol) = integral {
        values.insert(symbol.clone(), gains.integral_gain);
    }
    let derivative = seed
        .derivative_symbol
        .as_ref()
        .filter(|_| structure.has_derivative());
    if let Some(symbol) = derivative {
        values.insert(symbol.clone(), gains.derivative_gain);
    }
    ProjectCandidate {
        applied: false,
        source,
        structure,
        gains,
        values,
    }
}

/// The candidate of a trajectory-following law (C11): the PID candidate for the gains a design chose, with the rate and acceleration
/// feedforward gains in `values` by their variables, beside the PID's. `feedforward` is `(K_v, K_a)`.
#[must_use]
pub fn trajectory_candidate(
    seed: &TrajectoryFeedforwardSeed,
    source: CandidateSource,
    gains: ParallelGains,
    feedforward: (f64, f64),
) -> ProjectCandidate {
    let mut candidate = project_candidate(&seed.pid, source, gains);
    candidate
        .values
        .insert(seed.rate_symbol.clone(), feedforward.0);
    candidate
        .values
        .insert(seed.acceleration_symbol.clone(), feedforward.1);
    candidate
}

/// The candidate of a measured-disturbance feedforward (C8): the PI candidate for the gains a design chose, with the feedforward gain
/// `K_ff` in `values` by its variable, and the lead and lag times beside it when the program has them. Apply writes the derivative
/// variable as zero (C0), as it does for any PI.
#[must_use]
pub fn feedforward_candidate(
    seed: &FeedforwardPidSeed,
    source: CandidateSource,
    gains: ParallelGains,
    terms: FeedforwardTerms,
) -> ProjectCandidate {
    let mut candidate = project_candidate(&seed.pid, source, gains);
    candidate
        .values
        .insert(seed.gain_symbol.clone(), terms.gain);
    if let (Some(lead_lag), Some(lead), Some(lag)) =
        (&seed.lead_lag, terms.lead_seconds, terms.lag_seconds)
    {
        candidate.values.insert(lead_lag.lead_symbol.clone(), lead);
        candidate.values.insert(lead_lag.lag_symbol.clone(), lag);
    }
    candidate
}

/// The candidate of a Smith predictor (C9): the PI candidate for the gains a design chose, with the model's gain, time constant and
/// delay in `values` by their variables, and the block's operating point beside them when the program uses the library block (D-18).
/// Apply writes the derivative variable as zero (C0), as it does for any PI.
#[must_use]
pub fn smith_candidate(
    seed: &SmithPidSeed,
    source: CandidateSource,
    gains: ParallelGains,
    terms: SmithTerms,
) -> ProjectCandidate {
    let mut candidate = project_candidate(&seed.pid, source, gains);
    candidate
        .values
        .insert(seed.gain_symbol.clone(), terms.gain);
    candidate.values.insert(
        seed.time_constant_symbol.clone(),
        terms.time_constant_seconds,
    );
    candidate
        .values
        .insert(seed.delay_symbol.clone(), terms.delay_seconds);
    if let Some(block) = &seed.library_block {
        candidate
            .values
            .insert(block.operating_input_symbol.clone(), terms.operating.0);
        candidate
            .values
            .insert(block.operating_output_symbol.clone(), terms.operating.1);
    }
    candidate
}

#[cfg(test)]
#[path = "project_candidate_tests.rs"]
mod tests;
