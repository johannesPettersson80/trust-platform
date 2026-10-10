//! The evidence of a Feedback relay record (numerical annex N2.9 and N5.R; specification 35, B3.3 and B4.2): the samples
//! the record holds, its complete cycles, and whether those cycles qualify as repeatable evidence. A qualified record gives
//! `Ku` and `Pu`, the numbers of the relay rule; the design gives them the sign of the controller action (N6.4) and the
//! candidate of `study::tune::relay`. The cycle arithmetic is `trust_control_math::identify::relay`; this module reads the
//! record and words the reasons (`relayCycleEvidence.ts`, `relayTuning.ts`). Nothing here is applied (B4.1).

use schemars::JsonSchema;
use serde::Serialize;
use trust_control_math::identify::relay::{
    relay_pi_from_record, RelayPiCalculation, RelayPiFromRecord, RelayQuantity, RelayRecordError,
    RelayRejection, RelaySample,
};

use crate::recording::channels::{binding_value, process_value, MANIPULATED_VARIABLE_BINDING};
use crate::recording::samples::seconds_between;
use crate::recording::RecordedRun;
use crate::tuning::texts;

/// The relay evidence of a Feedback relay record of the study (N2.9, N5.R). The qualified numbers are unsigned: the
/// design gives `Kp` and `Ki` the sign of the controller action (N6.4).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(
    tag = "status",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum RelayCycleEvidence {
    /// The cycles qualify (N2.9): `Ku = 4·d/(π·a)`, `Pu` is their mean period, and the Ziegler–Nichols PI of N5.R.
    Qualified {
        /// The number of complete cycles.
        cycles: usize,
        /// The output amplitude `d`: half the separation of the two relay levels.
        output_amplitude: f64,
        /// The response amplitude `a`: the mean half peak-to-peak range of the measurement over the cycles.
        response_amplitude: f64,
        /// The ultimate period `Pu` in seconds: the mean period of the cycles.
        period_seconds: f64,
        /// The ultimate gain `Ku`.
        ultimate_gain: f64,
        /// `Kp = 0.45·Ku`, unsigned.
        proportional_gain: f64,
        /// `Ti = Pu/1.2` in seconds.
        integral_time_seconds: f64,
        /// `Ki = Kp/Ti`, unsigned.
        integral_gain: f64,
    },
    /// The record does not qualify; the reason names what does not hold (N2.9).
    NotQualified {
        /// The sentence that says why.
        reason: String,
    },
}

/// The samples of a relay record that the rule reads (`relayCycleEvidence.ts`): each sample with its measurement and its
/// manipulated variable, and only the samples where both are numbers. A sample's time is the seconds from the record's
/// first sample (A3.2). A record whose sample has no runtime time gets a time that is not finite, and the cycle rules
/// then refuse it.
#[must_use]
pub fn samples_of(run: &RecordedRun) -> Vec<RelaySample> {
    let origin = run
        .samples
        .first()
        .and_then(|sample| sample.runtime_time_ns);
    run.samples
        .iter()
        .filter_map(|sample| {
            let time_seconds = origin
                .zip(sample.runtime_time_ns)
                .map_or(f64::NAN, |(origin, time)| seconds_between(origin, time));
            Some(RelaySample {
                time_seconds,
                process_value: process_value(run, sample)?,
                output: binding_value(run, sample, MANIPULATED_VARIABLE_BINDING)?,
            })
        })
        .collect()
}

/// The evidence of a relay record between its two output levels (N2.9, N5.R): its complete cycles, and whether they
/// qualify as repeatable evidence. The levels are the relay's declared high and low outputs.
#[must_use]
pub fn evidence(high_output: f64, low_output: f64, samples: &[RelaySample]) -> RelayCycleEvidence {
    match relay_pi_from_record(high_output, low_output, samples) {
        Ok(record) => qualification(&record),
        Err(error) => RelayCycleEvidence::NotQualified {
            reason: extraction_reason(error).to_owned(),
        },
    }
}

/// The evidence of a record whose cycles could be read: the qualified numbers, or the reason the cycles do not qualify.
fn qualification(record: &RelayPiFromRecord) -> RelayCycleEvidence {
    let cycles = record.cycles.len();
    match record.calculation {
        RelayPiCalculation::Qualified(pi) => RelayCycleEvidence::Qualified {
            cycles,
            output_amplitude: pi.output_amplitude,
            response_amplitude: pi.response_amplitude,
            period_seconds: pi.period_seconds,
            ultimate_gain: pi.ultimate_gain,
            proportional_gain: pi.candidate.proportional_gain,
            integral_time_seconds: pi.candidate.integral_time_seconds,
            integral_gain: pi.candidate.integral_gain,
        },
        RelayPiCalculation::NotQualified(rejection) => RelayCycleEvidence::NotQualified {
            reason: rejection_reason(rejection),
        },
    }
}

/// The sentence of a record whose cycles cannot be read (N2.9): its levels, its samples, its transitions or its cycles.
fn extraction_reason(error: RelayRecordError) -> &'static str {
    match error {
        RelayRecordError::Levels => texts::RELAY_LEVELS,
        RelayRecordError::Samples => texts::RELAY_SAMPLES,
        RelayRecordError::TooFewTransitions => texts::RELAY_TRANSITIONS,
        RelayRecordError::IntermediateOutput => texts::RELAY_INTERMEDIATE_OUTPUT,
    }
}

/// The sentence of cycles that do not qualify (N2.9): the number of cycles, the levels, the evidence of each cycle, the
/// identifiability of the means, or the quantity whose spread is above 10 % of its mean.
fn rejection_reason(rejection: RelayRejection) -> String {
    match rejection {
        RelayRejection::TooFewCycles => texts::RELAY_TOO_FEW_CYCLES.to_owned(),
        RelayRejection::OutputSeparation => texts::RELAY_OUTPUT_SEPARATION.to_owned(),
        RelayRejection::CycleEvidence => texts::RELAY_CYCLE_EVIDENCE.to_owned(),
        RelayRejection::NotIdentifiable => texts::RELAY_NOT_IDENTIFIABLE.to_owned(),
        RelayRejection::Spread(RelayQuantity::Amplitude) => texts::relay_spread("amplitude"),
        RelayRejection::Spread(RelayQuantity::Period) => texts::relay_spread("period"),
    }
}

#[cfg(test)]
#[path = "relay_rule_tests.rs"]
mod tests;
