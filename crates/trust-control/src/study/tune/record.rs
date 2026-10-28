//! The record of a candidate (specification 35, B4.2, C0 and C1; numerical annex N8.9): the task period and the limits that the
//! record takes from the loop, the setpoint weights and the measurement filter that the pins or the entered settings shape,
//! the tracking time of a pinned back-calculation gain, and the candidate that these make. Every design and the manual method
//! write their record here, so the rules are written once. Nothing here reaches a runtime (B4.1).

use std::collections::BTreeMap;

use trust_control_math::identify::ParameterInterval;

use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{ControllerLaw, LoopDefinition};
use crate::study::tune::{refused_as, settings_refused};
use crate::tuning::candidate::{
    candidate_from_control_pid, AntiWindup, CandidateSource, ControlPidCandidate,
    ControlPidParameters, LawCandidate, DEFAULT_RATE_LIMIT_PER_S,
};
use crate::tuning::prediction::RecordShaping;
use crate::tuning::settings_pins::{apply_pins, ControllerSettingsPins, GainKey, MappedParameter};
use crate::tuning::shortfall::OutputLimits;
use crate::tuning::structure::{is_exactly_zero, ParallelGains, Structure};
use crate::tuning::texts;

/// The task period and the limits that a candidate's record and its prediction take from the loop (`pidCandidateFactory`,
/// B9.6): the task period in milliseconds, the output limits the loop declares (or the manipulated variable's range where it
/// declares none), and the output rate limits (`±10⁶` per second where the loop declares none).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LoopLimits {
    /// The task period in milliseconds.
    pub(crate) task_period_ms: f64,
    /// The output limits.
    pub(crate) output: OutputLimits,
    /// The output rate limits per second.
    pub(crate) rate: ParameterInterval,
    /// How the record's tracking time is chosen (C0, C1).
    pub(crate) tracking: TrackingRule,
}

/// How a candidate's tracking time `Tt` is chosen when no back-calculation gain is pinned (C0, C1): the shipped
/// `Control.PID` record's `TrackingTime` takes `max(Ti, Δt)`; a program law that declares back-calculation anti-windup takes
/// `max(Δt, √(Ti·Td))` when the candidate has a derivative term, and `max(Ti, Δt)` otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrackingRule {
    /// The `TrackingTime` field of the parameter record (C1).
    Record,
    /// The tracking gain `1/Tt` that a program law's back-calculation declaration binds (C0).
    BackCalculation,
}

impl TrackingRule {
    /// The rule of `loop_definition`'s controller law.
    #[must_use]
    pub(crate) fn of(loop_definition: &LoopDefinition) -> Self {
        match &loop_definition.controller.law {
            Some(ControllerLaw::Pid(pid)) if pid.anti_windup.is_some() => Self::BackCalculation,
            _ => Self::Record,
        }
    }
}

impl LoopLimits {
    /// The limits of `loop_definition`.
    #[must_use]
    pub(crate) fn of(loop_definition: &LoopDefinition) -> Self {
        let limits = &loop_definition.limits;
        let range = loop_definition.signals.manipulated_variable.range;
        Self {
            task_period_ms: loop_definition.controller.task_period_ms,
            output: OutputLimits {
                minimum: limits.output_minimum.unwrap_or(range.minimum),
                maximum: limits.output_maximum.unwrap_or(range.maximum),
            },
            rate: ParameterInterval {
                minimum: -limits.rate_down_per_s.unwrap_or(DEFAULT_RATE_LIMIT_PER_S),
                maximum: limits.rate_up_per_s.unwrap_or(DEFAULT_RATE_LIMIT_PER_S),
            },
            tracking: TrackingRule::of(loop_definition),
        }
    }
}

/// The candidate's complete record (C0, C1), written by the rules of `tuning::candidate` for every structure: the gains, the
/// setpoint weights and the measurement filter of `shaping`, no bias, the limits of the loop, the tracking time, and
/// back-calculation anti-windup. The tracking time is `1/v` for a pinned back-calculation gain `v` (C0), and otherwise the
/// loop's [`TrackingRule`] with `Ti = |Kp/Ki|` and `Td = |Kd/Kp|`. The candidate is unapplied (N8.9).
///
/// # Errors
/// The refusal of a pinned back-calculation gain that is not finite and positive, and the refusals of the record's limits.
pub(crate) fn candidate_record(
    gains: ParallelGains,
    source: CandidateSource,
    limits: &LoopLimits,
    shaping: RecordShaping,
    tracking_gain: Option<f64>,
) -> Result<ControlPidCandidate, Failure> {
    let period_seconds = limits.task_period_ms / 1000.0;
    let tracking_time = match tracking_gain {
        Some(gain) if gain.is_finite() && gain > 0.0 => gain.recip(),
        Some(_) => {
            return Err(Failure::new(
                texts::TRACKING_GAIN_POSITIVE,
                Cause::Settings,
                NextAction::None,
            ))
        }
        None => default_tracking_time(gains, period_seconds, limits.tracking),
    };
    let parameters = ControlPidParameters {
        proportional_gain: gains.proportional_gain,
        integral_gain: gains.integral_gain,
        derivative_gain: gains.derivative_gain,
        proportional_setpoint_weight: shaping.proportional_setpoint_weight,
        derivative_setpoint_weight: shaping.derivative_setpoint_weight,
        bias: 0.0,
        output_minimum: limits.output.minimum,
        output_maximum: limits.output.maximum,
        rate_minimum: limits.rate.minimum,
        rate_maximum: limits.rate.maximum,
        tracking_time,
        anti_windup: AntiWindup::BackCalculation,
    };
    let mut candidate = candidate_from_control_pid(parameters, source)
        .map_err(refused_as(Cause::Settings, NextAction::OpenLimits))?;
    candidate.measurement_filter_seconds = shaping.measurement_filter_seconds;
    Ok(candidate)
}

/// The tracking time of a record whose tracking gain is not pinned (C0): `max(Ti, task period)` with `Ti = |Kp/Ki|`, and the
/// task period when the record has no integral action.
fn default_tracking_time(gains: ParallelGains, period_seconds: f64, rule: TrackingRule) -> f64 {
    let integral_time = if is_exactly_zero(gains.integral_gain) {
        period_seconds
    } else {
        (gains.proportional_gain / gains.integral_gain).abs()
    };
    let derivative_time = if is_exactly_zero(gains.proportional_gain) {
        0.0
    } else {
        (gains.derivative_gain / gains.proportional_gain).abs()
    };
    // C0: a back-calculation law with derivative action tracks at sqrt(Ti*Td) (Astrom and Hagglund), never faster than a period.
    if rule == TrackingRule::BackCalculation
        && derivative_time > 0.0
        && !is_exactly_zero(gains.integral_gain)
    {
        return (integral_time * derivative_time).sqrt().max(period_seconds);
    }
    integral_time.max(period_seconds)
}

/// The setpoint weights and the measurement filter that a record writes (C0), from the values the pins hold or the entry
/// gives: the weights default to one and zero, and the filter defaults to zero. The filter is present only when the loop's
/// PID law declares a measurement filter.
#[must_use]
pub(crate) fn record_shaping(
    loop_definition: &LoopDefinition,
    applied: &BTreeMap<GainKey, f64>,
) -> RecordShaping {
    let defaults = RecordShaping::default();
    let declares_filter = matches!(
        &loop_definition.controller.law,
        Some(ControllerLaw::Pid(pid)) if pid.measurement_filter_time_symbol.is_some()
    );
    RecordShaping {
        proportional_setpoint_weight: applied
            .get(&GainKey::ProportionalSetpointWeight)
            .copied()
            .unwrap_or(defaults.proportional_setpoint_weight),
        derivative_setpoint_weight: applied
            .get(&GainKey::DerivativeSetpointWeight)
            .copied()
            .unwrap_or(defaults.derivative_setpoint_weight),
        measurement_filter_seconds: declares_filter.then_some(
            applied
                .get(&GainKey::MeasurementFilterSeconds)
                .copied()
                .unwrap_or(0.0),
        ),
    }
}

/// The shaping of a candidate that the manual method has already written (C0).
#[must_use]
pub(crate) fn shaping_of_candidate(candidate: &LawCandidate) -> RecordShaping {
    match candidate {
        LawCandidate::Pid(candidate) => RecordShaping {
            proportional_setpoint_weight: candidate.parameters.proportional_setpoint_weight,
            derivative_setpoint_weight: candidate.parameters.derivative_setpoint_weight,
            measurement_filter_seconds: candidate.measurement_filter_seconds,
        },
        // A program's own law has no setpoint weight and no filter of its own (C2), so its shaping is the default; so has a
        // state, search or switching law, which has none either (C3, C4, C5).
        LawCandidate::Project(_)
        | LawCandidate::State(_)
        | LawCandidate::Search(_)
        | LawCandidate::OnOff(_)
        | LawCandidate::Group(_) => RecordShaping::default(),
    }
}

/// The settings that the pins hold before a design (B4.2, C0): the pinned values, each checked against its bound parameter.
/// The design's gains follow when its pinned gains are applied.
///
/// # Errors
/// The settings refusals of [`apply_pins`], as the settings cause with no next action.
pub(crate) fn pinned_settings(
    loop_definition: &LoopDefinition,
    pins: Option<&ControllerSettingsPins>,
    mapped: &[MappedParameter],
) -> Result<BTreeMap<GainKey, f64>, Failure> {
    let family = pins.map_or(Structure::PI, |pins| pins.family);
    apply_pins(
        &loop_definition.revision,
        family,
        &BTreeMap::new(),
        pins,
        mapped,
    )
    .map_err(settings_refused())
}

/// The three gains of a design, by the record field each one holds (B4.2).
pub(crate) fn gains_map(gains: ParallelGains) -> BTreeMap<GainKey, f64> {
    BTreeMap::from([
        (GainKey::ProportionalGain, gains.proportional_gain),
        (GainKey::IntegralGain, gains.integral_gain),
        (GainKey::DerivativeGain, gains.derivative_gain),
    ])
}

/// The gains of a record's settings: each gain the settings hold, zero where they hold none.
pub(crate) fn gains_of_map(values: &BTreeMap<GainKey, f64>) -> ParallelGains {
    let value = |key: GainKey| values.get(&key).copied().unwrap_or(0.0);
    ParallelGains {
        proportional_gain: value(GainKey::ProportionalGain),
        integral_gain: value(GainKey::IntegralGain),
        derivative_gain: value(GainKey::DerivativeGain),
    }
}

/// The value the candidate's record gives each bound parameter, under the parameter's symbol (B4.2, C0): the gains, the setpoint
/// weights, the measurement filter time (absent when the record holds none) and the back-calculation gain `1/Tt`.
#[must_use]
pub(crate) fn record_settings(
    candidate: &ControlPidCandidate,
    parameters: &[MappedParameter],
) -> BTreeMap<String, f64> {
    parameters
        .iter()
        .filter_map(|parameter| {
            let value = match parameter.key {
                GainKey::ProportionalGain => candidate.parameters.proportional_gain,
                GainKey::IntegralGain => candidate.parameters.integral_gain,
                GainKey::DerivativeGain => candidate.parameters.derivative_gain,
                GainKey::BackCalculationGain => candidate.parameters.tracking_time.recip(),
                GainKey::ProportionalSetpointWeight => {
                    candidate.parameters.proportional_setpoint_weight
                }
                GainKey::DerivativeSetpointWeight => {
                    candidate.parameters.derivative_setpoint_weight
                }
                GainKey::MeasurementFilterSeconds => candidate.measurement_filter_seconds?,
            };
            Some((parameter.symbol.clone(), value))
        })
        .collect()
}

/// The settings of a candidate of either law, by the symbol of each bound setting (B4.2, C0): a `Control.PID` record's settings, or
/// the values that a program's own law writes to its seed variables (C2).
#[must_use]
pub(crate) fn candidate_settings(
    candidate: &LawCandidate,
    parameters: &[MappedParameter],
) -> BTreeMap<String, f64> {
    match candidate {
        LawCandidate::Pid(candidate) => record_settings(candidate, parameters),
        LawCandidate::Project(candidate) => candidate.values.clone(),
        LawCandidate::State(design) => design.candidate.parameters.clone(),
        LawCandidate::Search(search) => search.parameters.clone(),
        LawCandidate::OnOff(settings) => settings.values.clone(),
        LawCandidate::Group(group) => group.values.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_tracking_time_is_the_larger_of_ti_and_the_task_period() {
        let gains = ParallelGains {
            proportional_gain: 2.0,
            integral_gain: 0.5,
            derivative_gain: 0.0,
        };
        assert!((default_tracking_time(gains, 0.1, TrackingRule::Record) - 4.0).abs() <= 1e-12);
        let no_integral = ParallelGains {
            integral_gain: 0.0,
            ..gains
        };
        assert!(
            (default_tracking_time(no_integral, 0.1, TrackingRule::Record) - 0.1).abs() <= 1e-12
        );
    }

    #[test]
    fn a_back_calculation_law_with_derivative_action_tracks_at_the_root_of_ti_times_td() {
        // C0, controllerAdapters.ts: Ti = 2/0.5 = 4 s and Td = 1/2 = 0.5 s give Tt = sqrt(2) s; the record keeps max(Ti, dt).
        let gains = ParallelGains {
            proportional_gain: 2.0,
            integral_gain: 0.5,
            derivative_gain: 1.0,
        };
        let tracked = default_tracking_time(gains, 0.1, TrackingRule::BackCalculation);
        assert!((tracked - 2.0_f64.sqrt()).abs() <= 1e-12, "{tracked}");
        assert!((default_tracking_time(gains, 0.1, TrackingRule::Record) - 4.0).abs() <= 1e-12);
        assert!(
            (default_tracking_time(gains, 2.0, TrackingRule::BackCalculation) - 2.0).abs() <= 1e-12,
            "never faster than a task period"
        );
        let pi = ParallelGains {
            derivative_gain: 0.0,
            ..gains
        };
        assert!(
            (default_tracking_time(pi, 0.1, TrackingRule::BackCalculation) - 4.0).abs() <= 1e-12
        );
    }

    #[test]
    fn a_pinned_back_calculation_gain_sets_the_tracking_time_to_its_inverse() {
        let limits = LoopLimits {
            task_period_ms: 100.0,
            output: OutputLimits {
                minimum: 0.0,
                maximum: 100.0,
            },
            rate: ParameterInterval {
                minimum: -25.0,
                maximum: 25.0,
            },
            tracking: TrackingRule::Record,
        };
        let gains = ParallelGains {
            proportional_gain: 2.0,
            integral_gain: 0.5,
            derivative_gain: 0.0,
        };
        let pinned = candidate_record(
            gains,
            CandidateSource::Manual,
            &limits,
            RecordShaping::default(),
            Some(0.5),
        )
        .expect("a positive tracking gain is a record");
        assert!((pinned.parameters.tracking_time - 2.0).abs() <= 1e-12);
        let refused = candidate_record(
            gains,
            CandidateSource::Manual,
            &limits,
            RecordShaping::default(),
            Some(0.0),
        )
        .expect_err("a tracking gain of zero is refused");
        assert_eq!(refused.message, texts::TRACKING_GAIN_POSITIVE);
        assert_eq!(refused.cause, Cause::Settings);
        assert_eq!(refused.next_action, NextAction::None);
    }
}
