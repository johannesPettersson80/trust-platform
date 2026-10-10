//! The design of the relay rule (numerical annex N5.R and N6.4; specification 35, B4.2): the PI of a qualified Feedback relay
//! record of the study, `Kp = s_a·0.45·Ku`, `Ti = Pu/1.2` and `Ki = Kp/Ti`, unapplied. The record is read by
//! `study::tune::relay_evidence_for` and judged by `tuning::relay_rule`; this module only gives the qualified numbers the sign
//! of the controller action and states the candidate.

use trust_control_math::identify::gain::ControllerAction;

use crate::failure::{Cause, Failure, NextAction};
use crate::study::tune::design::Design;
use crate::tuning::candidate::CandidateSource;
use crate::tuning::formula::action_sign;
use crate::tuning::objective::TuningObjective;
use crate::tuning::relay_rule::RelayCycleEvidence;
use crate::tuning::structure::{ParallelGains, Structure};
use crate::tuning::texts;

/// The proportional factor of the relay rule, `Kp = 0.45·Ku` (N5.R).
const PROPORTIONAL_FACTOR: f64 = 0.45;
/// The divisor of the relay rule's integral time, `Ti = Pu/1.2` (N5.R).
const INTEGRAL_DIVISOR: f64 = 1.2;

/// The refusal of a relay rule that has no qualified record to read (N5.R, N2.9; B4.7: choose another method).
#[must_use]
pub(crate) fn refusal(reason: &str) -> Failure {
    Failure::new(reason, Cause::ModelOrData, NextAction::ChooseMethod)
}

/// The refusal of a relay rule whose record is missing or does not qualify; `None` when the record qualifies.
#[must_use]
pub(crate) fn refusal_of(evidence: Option<&RelayCycleEvidence>) -> Option<Failure> {
    match evidence {
        Some(RelayCycleEvidence::Qualified { .. }) => None,
        Some(RelayCycleEvidence::NotQualified { reason }) => Some(refusal(reason)),
        None => Some(refusal(texts::RELAY_NOT_IDENTIFIED)),
    }
}

/// The design of the relay rule (N5.R, N6.4): the PI of the qualified record with the sign of the controller action, and
/// unapplied. The pins apply to it as to every candidate.
///
/// # Errors
/// The refusal of [`refusal_of`] when the record is missing or does not qualify.
pub(crate) fn design(
    evidence: Option<&RelayCycleEvidence>,
    action: ControllerAction,
    objective: TuningObjective,
) -> Result<Design, Failure> {
    let (cycles, ultimate_gain, period_seconds) = match evidence {
        Some(RelayCycleEvidence::Qualified {
            cycles,
            ultimate_gain,
            period_seconds,
            ..
        }) => (*cycles, *ultimate_gain, *period_seconds),
        Some(RelayCycleEvidence::NotQualified { reason }) => return Err(refusal(reason)),
        None => return Err(refusal(texts::RELAY_NOT_IDENTIFIED)),
    };
    // Ku and Pu are magnitudes; the gains take the sign of the action (N6.4), and Ki takes the sign of Kp because Ti > 0.
    let proportional_gain = action_sign(action) * PROPORTIONAL_FACTOR * ultimate_gain;
    let integral_gain = proportional_gain / (period_seconds / INTEGRAL_DIVISOR);
    Ok(Design {
        source: CandidateSource::RelayRule,
        structure: Structure::PI,
        gains: ParallelGains {
            proportional_gain,
            integral_gain,
            derivative_gain: 0.0,
        },
        objective,
        closed_loop_time_seconds: None,
        attempts: Vec::new(),
        reason: texts::relay_rule_reason(ultimate_gain, period_seconds),
        description: Some(texts::relay_rule_description(
            ultimate_gain,
            period_seconds,
            cycles,
        )),
    })
}

#[cfg(test)]
#[path = "relay_tests.rs"]
mod tests;
