//! The robustness of a candidate on the selected model (B4.6, N7.7 of the Control Studio numerical annex; the
//! `candidateRobustness` of `candidateRobustness.ts`): how much model error the candidate tolerates. The delay
//! margin is derived here from the phase margin and the gain crossover; the margins themselves are the candidate's.

#![allow(clippy::module_name_repetitions)] // `CandidateRobustness` names the robustness of a candidate; the precedent is `model.rs`.

use schemars::JsonSchema;
use serde::Serialize;
use trust_control_math::margins::MarginValue;

use crate::tuning::candidate_margins::CandidateMargins;

/// How much model error a candidate tolerates on the selected model (N7.7).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CandidateRobustness {
    /// The gain margin in dB (N7.7).
    pub gain_margin_db: MarginValue,
    /// The phase margin in degrees (N7.7).
    pub phase_margin_degrees: MarginValue,
    /// The extra dead time the loop tolerates before it becomes unstable: the phase margin in radians over the
    /// gain-crossover frequency (N7.7).
    pub delay_margin_seconds: MarginValue,
    /// The largest factor by which feedback amplifies a disturbance at any frequency, `Ms = max |1/(1 + L(jω))|`.
    pub peak_sensitivity: f64,
    /// The sentence that says why the crossover margins are not available, when they are not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable: Option<String>,
}

/// The robustness of a candidate, from its margins.
#[must_use]
pub fn candidate_robustness(margins: &CandidateMargins) -> CandidateRobustness {
    CandidateRobustness {
        gain_margin_db: margins.gain_margin_db,
        phase_margin_degrees: margins.phase_margin_degrees,
        delay_margin_seconds: delay_margin(margins),
        peak_sensitivity: margins.peak_sensitivity,
        unavailable: margins.unavailable.clone(),
    }
}

/// The delay margin `PM/ωc` in seconds, with `PM` in radians (N7.7). It is unbounded where the phase margin is
/// unbounded, because the loop has no gain crossover over the whole frequency axis. Where the grid shows no
/// gain crossover, it is not observed in range (N7.8), never infinite.
#[must_use]
pub fn delay_margin(margins: &CandidateMargins) -> MarginValue {
    match (
        margins.phase_margin_degrees,
        margins.gain_crossover_rad_per_second,
    ) {
        (MarginValue::Finite(phase), Some(crossover)) if crossover > 0.0 => {
            MarginValue::Finite(phase.to_radians() / crossover)
        }
        (MarginValue::Unbounded, _) => MarginValue::Unbounded,
        _ => MarginValue::NotObservedInRange,
    }
}

#[cfg(test)]
#[path = "robustness_tests.rs"]
mod tests;
