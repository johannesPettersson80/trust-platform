//! Relay feedback identification (N2.9, N5.R): the complete cycles of a measured relay record, their qualification
//! as repeatable evidence, and the Ziegler–Nichols PI candidate they give (`relayTuning.ts`). The candidate is a
//! number for Tune to check; nothing here applies it.

// The relay names carry the module's subject; the precedent is `model.rs`.
#![allow(clippy::module_name_repetitions)]

use std::f64::consts::PI;

use crate::sampling::statistics::mean;
use crate::sampling::{largest, smallest};

/// One measured sample of a relay record: its time, the measured process value and the controller output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RelaySample {
    /// The time of the sample in seconds.
    pub time_seconds: f64,
    /// The measured process value.
    pub process_value: f64,
    /// The controller output at the sample.
    pub output: f64,
}

/// One complete relay cycle: the peak and trough of the measured process value, and the period in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RelayCycle {
    /// The largest process value in the cycle.
    pub peak: f64,
    /// The smallest process value in the cycle.
    pub trough: f64,
    /// The time from the cycle's first low-to-high output transition to its next one, in seconds.
    pub period_seconds: f64,
}

/// Why a relay record cannot be read as cycles (`extractRelayCycles`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RelayRecordError {
    /// The high or low output level is not finite, or the low level is not below the high level.
    #[error("relay output levels are not finite and ordered")]
    Levels,
    /// Fewer than three samples, or a time, process value or output that is not finite, or times that do not
    /// strictly increase.
    #[error("relay samples are not finite or do not have strictly increasing times")]
    Samples,
    /// Fewer than two measured transitions from the low output level to the high one.
    #[error("fewer than two low-to-high output transitions")]
    TooFewTransitions,
    /// A cycle holds an output that is neither level: a clipped or intermediate value.
    #[error("a cycle holds a clipped or intermediate output value")]
    IntermediateOutput,
}

/// Which quantity of the relay cycles varies too much to qualify the identification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayQuantity {
    /// The amplitude of the process value's oscillation, half its peak-to-trough range.
    Amplitude,
    /// The period of the oscillation.
    Period,
}

/// Why measured relay cycles do not qualify (`qualifyRelayCycles`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayRejection {
    /// Fewer than three complete cycles were measured.
    TooFewCycles,
    /// The output levels do not separate by a finite, non-zero amount.
    OutputSeparation,
    /// A cycle lacks a finite peak, trough or positive period, or its peak does not exceed its trough.
    CycleEvidence,
    /// The mean amplitude or period is not finite and positive.
    NotIdentifiable,
    /// The cycles spread by more than 10 % of their mean in the named quantity.
    Spread(RelayQuantity),
}

/// The qualified result of measured relay cycles and the unapplied PI candidate of them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RelayPi {
    /// The ultimate gain `Ku = 4·d/(π·a)`, from the output amplitude `d` and the response amplitude `a`.
    pub ultimate_gain: f64,
    /// The mean period of the cycles, in seconds: the ultimate period `Tu`.
    pub period_seconds: f64,
    /// The output amplitude `d`: half the separation of the relay levels.
    pub output_amplitude: f64,
    /// The response amplitude `a`: the mean half peak-to-trough range of the cycles.
    pub response_amplitude: f64,
    /// The candidate PI gains. Never applied: the candidate is an input to Tune.
    pub candidate: RelayPiCandidate,
}

/// The Ziegler–Nichols PI candidate of a relay: `Kp = 0.45·Ku`, `Ti = Tu/1.2`, and the integral gain `Ki = Kp/Ti`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RelayPiCandidate {
    /// The proportional gain `Kp = 0.45·Ku`.
    pub proportional_gain: f64,
    /// The integral time `Ti = Tu/1.2` in seconds.
    pub integral_time_seconds: f64,
    /// The integral gain `Ki = Kp/Ti`.
    pub integral_gain: f64,
}

/// The result of calculating a relay PI candidate: qualified, with its numbers, or refused with its reason.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RelayPiCalculation {
    /// The cycles qualify; the candidate is computed.
    Qualified(RelayPi),
    /// The cycles do not qualify; no candidate is offered.
    NotQualified(RelayRejection),
}

/// The cycles of a relay record with the PI calculation they give (`relayPiFromMeasuredRecord`).
#[derive(Debug, Clone, PartialEq)]
pub struct RelayPiFromRecord {
    /// The complete cycles, in time order.
    pub cycles: Vec<RelayCycle>,
    /// The calculation of the candidate from those cycles.
    pub calculation: RelayPiCalculation,
}

/// Which of the two output levels a sample's output is at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    /// The high output level.
    High,
    /// The low output level.
    Low,
}

/// The cycles of a relay record (`extractRelayCycles`). A complete cycle starts at an observed low-to-high output
/// transition and ends at the next one, so a record that starts midway through a leg, and any leg before the first
/// transition, contribute nothing. Each cycle's peak and trough are the largest and smallest process value within
/// it, and its period is the time between its two transitions.
///
/// The output levels are matched within `min((high − low)/4, 10⁻⁶·max(1, |high|, |low|))`.
///
/// # Errors
/// Returns the [`RelayRecordError`] for malformed levels or samples, fewer than two measured transitions, or a
/// cycle that holds an output which is neither level.
pub fn extract_relay_cycles(
    high_output: f64,
    low_output: f64,
    samples: &[RelaySample],
) -> Result<Vec<RelayCycle>, RelayRecordError> {
    if !high_output.is_finite() || !low_output.is_finite() || low_output >= high_output {
        return Err(RelayRecordError::Levels);
    }
    let well_formed = samples.iter().enumerate().all(|(index, sample)| {
        sample.time_seconds.is_finite()
            && sample.process_value.is_finite()
            && sample.output.is_finite()
            && (index == 0 || sample.time_seconds > samples[index - 1].time_seconds)
    });
    if samples.len() < 3 || !well_formed {
        return Err(RelayRecordError::Samples);
    }
    let tolerance = ((high_output - low_output) / 4.0)
        .min(1e-6 * 1.0_f64.max(high_output.abs()).max(low_output.abs()));
    let level = |output: f64| level_of(output, high_output, low_output, tolerance);

    let mut high_starts = Vec::new();
    let mut previous: Option<Level> = None;
    for (index, sample) in samples.iter().enumerate() {
        let current = level(sample.output);
        if current == Some(Level::High) && previous == Some(Level::Low) {
            high_starts.push(index);
        }
        if current.is_some() {
            previous = current;
        }
    }
    if high_starts.len() < 2 {
        return Err(RelayRecordError::TooFewTransitions);
    }

    let mut cycles = Vec::with_capacity(high_starts.len() - 1);
    for pair in high_starts.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        let segment = &samples[start..=end];
        if segment.iter().any(|sample| level(sample.output).is_none()) {
            return Err(RelayRecordError::IntermediateOutput);
        }
        let values: Vec<f64> = segment.iter().map(|sample| sample.process_value).collect();
        cycles.push(RelayCycle {
            peak: largest(&values, f64::NEG_INFINITY),
            trough: smallest(&values),
            period_seconds: samples[end].time_seconds - samples[start].time_seconds,
        });
    }
    Ok(cycles)
}

/// The level an output is at, when it lies within the tolerance of one of the two levels.
fn level_of(output: f64, high: f64, low: f64, tolerance: f64) -> Option<Level> {
    if (output - high).abs() <= tolerance {
        Some(Level::High)
    } else if (output - low).abs() <= tolerance {
        Some(Level::Low)
    } else {
        None
    }
}

/// The amplitude and period that the cycles give once they qualify.
struct Qualification {
    /// The output amplitude `d`.
    output_amplitude: f64,
    /// The mean response amplitude `a`.
    response_amplitude: f64,
    /// The mean period `Tu`.
    period_seconds: f64,
}

/// Qualifies the measured cycles as repeatable evidence (`qualifyRelayCycles`): at least three cycles, a finite
/// non-zero output separation, every cycle with finite positive evidence, a finite positive mean amplitude and
/// period, and a spread of no more than 10 % of the mean (plus `32·ε`) in both the amplitude and the period.
fn qualify_relay_cycles(
    high_output: f64,
    low_output: f64,
    cycles: &[RelayCycle],
) -> Result<Qualification, RelayRejection> {
    if cycles.len() < 3 {
        return Err(RelayRejection::TooFewCycles);
    }
    let output_amplitude = (high_output - low_output).abs() / 2.0;
    if !output_amplitude.is_finite() || output_amplitude <= 0.0 {
        return Err(RelayRejection::OutputSeparation);
    }
    let evidence = cycles.iter().all(|cycle| {
        cycle.peak.is_finite()
            && cycle.trough.is_finite()
            && cycle.period_seconds.is_finite()
            && cycle.peak > cycle.trough
            && cycle.period_seconds > 0.0
    });
    if !evidence {
        return Err(RelayRejection::CycleEvidence);
    }
    let amplitudes: Vec<f64> = cycles
        .iter()
        .map(|cycle| (cycle.peak - cycle.trough) / 2.0)
        .collect();
    let periods: Vec<f64> = cycles.iter().map(|cycle| cycle.period_seconds).collect();
    let response_amplitude = mean(&amplitudes);
    let period_seconds = mean(&periods);
    if !response_amplitude.is_finite()
        || response_amplitude <= 0.0
        || !period_seconds.is_finite()
        || period_seconds <= 0.0
    {
        return Err(RelayRejection::NotIdentifiable);
    }
    let repeatable = [
        (RelayQuantity::Amplitude, &amplitudes, response_amplitude),
        (RelayQuantity::Period, &periods, period_seconds),
    ];
    for (quantity, values, average) in repeatable {
        let spread = largest(values, f64::NEG_INFINITY) - smallest(values);
        if spread / average > 0.1 + 32.0 * f64::EPSILON {
            return Err(RelayRejection::Spread(quantity));
        }
    }
    Ok(Qualification {
        output_amplitude,
        response_amplitude,
        period_seconds,
    })
}

/// The unapplied Ziegler–Nichols PI candidate of qualified relay cycles (`calculateRelayPiCandidate`):
/// `Ku = 4·d/(π·a)`, `Kp = 0.45·Ku`, `Ti = Tu/1.2`, `Ki = Kp/Ti`, where `d` is half the level separation, `a` the mean
/// half peak-to-trough range and `Tu` the mean period of the cycles.
#[must_use]
pub fn calculate_relay_pi_candidate(
    high_output: f64,
    low_output: f64,
    cycles: &[RelayCycle],
) -> RelayPiCalculation {
    match qualify_relay_cycles(high_output, low_output, cycles) {
        Err(reason) => RelayPiCalculation::NotQualified(reason),
        Ok(qualification) => {
            let ultimate_gain =
                4.0 * qualification.output_amplitude / (PI * qualification.response_amplitude);
            let proportional_gain = 0.45 * ultimate_gain;
            let integral_time_seconds = qualification.period_seconds / 1.2;
            RelayPiCalculation::Qualified(RelayPi {
                ultimate_gain,
                period_seconds: qualification.period_seconds,
                output_amplitude: qualification.output_amplitude,
                response_amplitude: qualification.response_amplitude,
                candidate: RelayPiCandidate {
                    proportional_gain,
                    integral_time_seconds,
                    integral_gain: proportional_gain / integral_time_seconds,
                },
            })
        }
    }
}

/// The cycles of a relay record and the PI calculation they give (`relayPiFromMeasuredRecord`).
///
/// # Errors
/// Returns the [`RelayRecordError`] of [`extract_relay_cycles`]; a record that does not qualify is an `Ok` result
/// whose calculation is [`RelayPiCalculation::NotQualified`].
pub fn relay_pi_from_record(
    high_output: f64,
    low_output: f64,
    samples: &[RelaySample],
) -> Result<RelayPiFromRecord, RelayRecordError> {
    let cycles = extract_relay_cycles(high_output, low_output, samples)?;
    let calculation = calculate_relay_pi_candidate(high_output, low_output, &cycles);
    Ok(RelayPiFromRecord {
        cycles,
        calculation,
    })
}

#[cfg(test)]
mod tests;
