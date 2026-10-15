//! The `Control.PID` candidate record (specification 35, B4.2 and C1; numerical annex N0.5): the complete parameter record a
//! candidate writes, its source and its structure, and the rules that check a record before it is stored or sent. A
//! candidate is always unapplied: nothing here writes to a runtime (B6.1 does that, behind its own transaction).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::identification::refusal::Refusal;
use crate::tuning::formula::PiTuning;
use crate::tuning::project_candidate::ProjectCandidate;
use crate::tuning::structure::{ParallelGains, Structure};
use crate::tuning::texts;

/// The default rate limit of a candidate when the loop declares none (`pidCandidateFactory.ts`).
pub const DEFAULT_RATE_LIMIT_PER_S: f64 = 1_000_000.0;
/// The default output limits of a candidate when the loop declares none (`pidCandidateFactory.ts`).
pub const DEFAULT_OUTPUT_MINIMUM: f64 = 0.0;
/// See [`DEFAULT_OUTPUT_MINIMUM`].
pub const DEFAULT_OUTPUT_MAXIMUM: f64 = 100.0;
/// The default task period of a candidate's tracking time when the loop declares none, in milliseconds.
pub const DEFAULT_TASK_PERIOD_MS: f64 = 10.0;

/// The anti-windup method of a `Control.PID` record (C0 and C1): back-calculation or the combined method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum AntiWindup {
    /// Back-calculation, with the tracking time `Tt`.
    BackCalculation,
    /// The combined method of the library block.
    Combined,
}

/// The complete parameter record of a `Control.PID` candidate (C1, `ControlPidParameters`): every field the record writes, in
/// the units of the library block.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlPidParameters {
    /// The proportional gain `Kp`, signed.
    pub proportional_gain: f64,
    /// The integral gain `Ki`, signed.
    pub integral_gain: f64,
    /// The derivative gain `Kd`, signed.
    pub derivative_gain: f64,
    /// The proportional setpoint weight `β` (C0): one for a calculated candidate unless pinned.
    pub proportional_setpoint_weight: f64,
    /// The derivative setpoint weight `γ`.
    pub derivative_setpoint_weight: f64,
    /// The bias of the output.
    pub bias: f64,
    /// The smallest output.
    pub output_minimum: f64,
    /// The largest output.
    pub output_maximum: f64,
    /// The smallest rate of change of the output per second.
    pub rate_minimum: f64,
    /// The largest rate of change of the output per second.
    pub rate_maximum: f64,
    /// The tracking time `Tt` in seconds, positive.
    pub tracking_time: f64,
    /// The anti-windup method.
    pub anti_windup: AntiWindup,
}

/// Where a candidate came from (B4.2, C1): the engineer's entry, an import from the runtime, or a formula result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum CandidateSource {
    /// Entered by the engineer.
    Manual,
    /// Read from the runtime's parameter record.
    Imported,
    /// The SIMC formula result (N5.2, N5.3).
    #[serde(rename = "SIMC")]
    Simc,
    /// The AMIGO formula result (N5.5).
    #[serde(rename = "AMIGO")]
    Amigo,
    /// A result of the constrained comparison (N5.10).
    #[serde(rename = "Constrained comparison")]
    ConstrainedComparison,
    /// The relay rule of a qualified Feedback relay record (N5.R): `Kp = 0.45·Ku` and `Ti = Pu/1.2`.
    #[serde(rename = "Relay rule")]
    RelayRule,
}

/// A candidate of a `Control.PID` record, unapplied (B4.2): its source, its structure and the complete record it writes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlPidCandidate {
    /// Always false: a candidate is never applied by Tune (N8.9).
    pub applied: bool,
    /// Where the candidate came from.
    pub source: CandidateSource,
    /// The structure, from the gains that are not zero.
    pub structure: Structure,
    /// The complete record the candidate writes.
    pub parameters: ControlPidParameters,
    /// The measurement filter time `Tf` in seconds (C0): absent when the loop's PID law declares no filter. Otherwise it is
    /// zero for a calculated candidate unless `Tf` is pinned, and the entered value for manual settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measurement_filter_seconds: Option<f64>,
}

/// The candidate of a loop's law, unapplied (B4.1, C0): a `Control.PID` record, or the gains and seed variables of a program's own
/// law (C2). A design, a manual entry and a stored section each hold one of the two; the structure and the gains are the same
/// for either.
#[derive(Debug, Clone, PartialEq)]
pub enum LawCandidate {
    /// The candidate of a `Control.PID` (C1): the complete record it writes.
    Pid(ControlPidCandidate),
    /// The candidate of a program's own law (C2): its gains, and the values of the seed variables that Apply writes.
    Project(ProjectCandidate),
}

impl LawCandidate {
    /// The structure of the candidate.
    #[must_use]
    pub fn structure(&self) -> Structure {
        match self {
            Self::Pid(candidate) => candidate.structure,
            Self::Project(candidate) => candidate.structure,
        }
    }

    /// The gains of the candidate, signed.
    #[must_use]
    pub fn gains(&self) -> ParallelGains {
        match self {
            Self::Pid(candidate) => gains_of(candidate),
            Self::Project(candidate) => candidate.gains,
        }
    }

    /// The candidate in the two fields that a stored section and an answer keep: the `Control.PID` record, or the program's law.
    #[must_use]
    pub fn into_parts(self) -> (Option<ControlPidCandidate>, Option<ProjectCandidate>) {
        match self {
            Self::Pid(candidate) => (Some(candidate), None),
            Self::Project(candidate) => (None, Some(candidate)),
        }
    }
}

/// The loop limits that a generated candidate takes its defaults from (`pidCandidateFactory.ts`): each is absent when the
/// loop declares none.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CandidateLimits {
    /// The loop's output minimum.
    pub output_minimum: Option<f64>,
    /// The loop's output maximum.
    pub output_maximum: Option<f64>,
    /// The loop's rate limit up, per second.
    pub rate_up_per_s: Option<f64>,
    /// The loop's rate limit down, per second.
    pub rate_down_per_s: Option<f64>,
    /// The loop's task period in milliseconds.
    pub task_period_ms: Option<f64>,
}

/// Checks a record's limits and time (`validateParameters` of `controllerCandidate.ts`): finite, the output minimum below the
/// maximum, the rate minimum not above the maximum, and a positive tracking time.
///
/// # Errors
/// Returns a [`Refusal`] with the sentence of the first check that fails.
pub fn validate_parameters(parameters: &ControlPidParameters) -> Result<(), Refusal> {
    let finite = [
        parameters.proportional_gain,
        parameters.integral_gain,
        parameters.derivative_gain,
        parameters.proportional_setpoint_weight,
        parameters.derivative_setpoint_weight,
        parameters.bias,
        parameters.output_minimum,
        parameters.output_maximum,
        parameters.rate_minimum,
        parameters.rate_maximum,
        parameters.tracking_time,
    ]
    .iter()
    .all(|value| value.is_finite());
    if !finite {
        return Err(Refusal::new(texts::CANDIDATE_NOT_FINITE));
    }
    if parameters.output_minimum >= parameters.output_maximum {
        return Err(Refusal::new(texts::CANDIDATE_OUTPUT_ORDER));
    }
    if parameters.rate_minimum > parameters.rate_maximum {
        return Err(Refusal::new(texts::CANDIDATE_RATE_ORDER));
    }
    if parameters.tracking_time <= 0.0 {
        return Err(Refusal::new(texts::SETTINGS_TRACKING_TIME));
    }
    Ok(())
}

/// A candidate from a complete record (`candidateFromControlPid`): the structure follows from the nonzero integral and
/// derivative gains.
///
/// # Errors
/// Returns the refusal of [`validate_parameters`].
pub fn candidate_from_control_pid(
    parameters: ControlPidParameters,
    source: CandidateSource,
) -> Result<ControlPidCandidate, Refusal> {
    validate_parameters(&parameters)?;
    Ok(ControlPidCandidate {
        applied: false,
        source,
        structure: Structure::of_gains(parameters.integral_gain, parameters.derivative_gain),
        parameters,
        measurement_filter_seconds: None,
    })
}

/// A generated PI candidate of a formula result (`generatedPiCandidate`): `Kp`, `Ki = Kp / Ti`, `Kd = 0`, the setpoint
/// weight one, the derivative weight zero, a zero bias, the loop's limits (or their defaults), the rate limits, the
/// tracking time `max(Ti, task period)`, and back-calculation anti-windup.
///
/// The tracking time is the TypeScript's choice: the specification asks only for a finite positive `Tt` (C0).
///
/// # Errors
/// Returns the refusal of [`candidate_from_control_pid`] when the limits are not in order.
pub fn generated_candidate(
    tuning: &PiTuning,
    limits: CandidateLimits,
) -> Result<ControlPidCandidate, Refusal> {
    let gains = tuning.gains();
    let rate_up = limits.rate_up_per_s.unwrap_or(DEFAULT_RATE_LIMIT_PER_S);
    let rate_down = limits.rate_down_per_s.unwrap_or(DEFAULT_RATE_LIMIT_PER_S);
    let period_seconds = limits.task_period_ms.unwrap_or(DEFAULT_TASK_PERIOD_MS) / 1000.0;
    let parameters = ControlPidParameters {
        proportional_gain: gains.proportional_gain,
        integral_gain: gains.integral_gain,
        derivative_gain: 0.0,
        proportional_setpoint_weight: 1.0,
        derivative_setpoint_weight: 0.0,
        bias: 0.0,
        output_minimum: limits.output_minimum.unwrap_or(DEFAULT_OUTPUT_MINIMUM),
        output_maximum: limits.output_maximum.unwrap_or(DEFAULT_OUTPUT_MAXIMUM),
        rate_minimum: -rate_down,
        rate_maximum: rate_up,
        tracking_time: tuning.integral_time_seconds.max(period_seconds),
        anti_windup: AntiWindup::BackCalculation,
    };
    let source = match tuning.method {
        crate::tuning::objective::TuningMethod::Simc => CandidateSource::Simc,
        crate::tuning::objective::TuningMethod::Amigo => CandidateSource::Amigo,
    };
    candidate_from_control_pid(parameters, source)
}

/// The gains of a candidate's record, for the gains a caller compares.
#[must_use]
pub fn gains_of(candidate: &ControlPidCandidate) -> ParallelGains {
    ParallelGains {
        proportional_gain: candidate.parameters.proportional_gain,
        integral_gain: candidate.parameters.integral_gain,
        derivative_gain: candidate.parameters.derivative_gain,
    }
}

/// The record a runtime reports for its parameters (`candidateFromRuntimeParameters`): every numeric field must be present
/// and finite, the anti-windup is `Combined` when the runtime says so and back-calculation otherwise. The candidate is
/// imported and unapplied.
///
/// # Errors
/// Returns a [`Refusal`] naming the first field the runtime did not report, or the refusal of [`validate_parameters`].
pub fn candidate_from_runtime(value: &Value) -> Result<ControlPidCandidate, Refusal> {
    let number = |name: &str| -> Result<f64, Refusal> {
        value
            .get(name)
            .and_then(Value::as_f64)
            .filter(|parsed| parsed.is_finite())
            .ok_or_else(|| Refusal::new(texts::runtime_parameter_missing(name)))
    };
    let parameters = ControlPidParameters {
        proportional_gain: number("ProportionalGain")?,
        integral_gain: number("IntegralGain")?,
        derivative_gain: number("DerivativeGain")?,
        proportional_setpoint_weight: number("ProportionalSetpointWeight")?,
        derivative_setpoint_weight: number("DerivativeSetpointWeight")?,
        bias: number("Bias")?,
        output_minimum: number("OutputMinimum")?,
        output_maximum: number("OutputMaximum")?,
        rate_minimum: number("RateMinimum")?,
        rate_maximum: number("RateMaximum")?,
        tracking_time: number("TrackingTime")?,
        anti_windup: if value.get("AntiWindup").and_then(Value::as_str) == Some("Combined") {
            AntiWindup::Combined
        } else {
            AntiWindup::BackCalculation
        },
    };
    candidate_from_control_pid(parameters, CandidateSource::Imported)
}

/// Checks a complete record that the engineer entered or the extension posts (`validateControlPidSettings`): exactly the
/// required fields, each numeric field finite, the anti-windup one of its two methods, the output and rate limits in order,
/// and a positive tracking time.
///
/// # Errors
/// Returns a [`Refusal`] with the sentence of the first check that fails.
pub fn validate_control_pid_settings(value: &Value) -> Result<ControlPidParameters, Refusal> {
    const NUMERIC: [&str; 11] = [
        "proportionalGain",
        "integralGain",
        "derivativeGain",
        "proportionalSetpointWeight",
        "derivativeSetpointWeight",
        "bias",
        "outputMinimum",
        "outputMaximum",
        "rateMinimum",
        "rateMaximum",
        "trackingTime",
    ];
    let Some(record) = value.as_object() else {
        return Err(Refusal::new(texts::SETTINGS_NOT_OBJECT));
    };
    if record.len() != NUMERIC.len() + 1 || !record.contains_key("antiWindup") {
        return Err(Refusal::new(texts::SETTINGS_FIELD_SET));
    }
    for key in NUMERIC {
        if !record
            .get(key)
            .and_then(Value::as_f64)
            .is_some_and(f64::is_finite)
        {
            return Err(Refusal::new(format!(
                "Control.PID {key} must be a finite number."
            )));
        }
    }
    let anti_windup = match record.get("antiWindup").and_then(Value::as_str) {
        Some("BackCalculation") => AntiWindup::BackCalculation,
        Some("Combined") => AntiWindup::Combined,
        _ => return Err(Refusal::new(texts::SETTINGS_ANTI_WINDUP)),
    };
    let field = |key: &str| record.get(key).and_then(Value::as_f64).unwrap_or(f64::NAN);
    if field("outputMinimum") > field("outputMaximum") {
        return Err(Refusal::new(texts::SETTINGS_OUTPUT_ORDER));
    }
    if field("rateMinimum") > field("rateMaximum") {
        return Err(Refusal::new(texts::SETTINGS_RATE_ORDER));
    }
    if field("trackingTime") <= 0.0 {
        return Err(Refusal::new(texts::SETTINGS_TRACKING_TIME));
    }
    Ok(ControlPidParameters {
        proportional_gain: field("proportionalGain"),
        integral_gain: field("integralGain"),
        derivative_gain: field("derivativeGain"),
        proportional_setpoint_weight: field("proportionalSetpointWeight"),
        derivative_setpoint_weight: field("derivativeSetpointWeight"),
        bias: field("bias"),
        output_minimum: field("outputMinimum"),
        output_maximum: field("outputMaximum"),
        rate_minimum: field("rateMinimum"),
        rate_maximum: field("rateMaximum"),
        tracking_time: field("trackingTime"),
        anti_windup,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)] // The generated gains are exact quotients of exact inputs.

    use super::*;
    use crate::tuning::objective::{TuningMethod, TuningObjective};
    use serde_json::json;

    fn parameters() -> ControlPidParameters {
        ControlPidParameters {
            proportional_gain: 0.8,
            integral_gain: 0.0,
            derivative_gain: 0.0,
            proportional_setpoint_weight: 1.0,
            derivative_setpoint_weight: 0.0,
            bias: 0.0,
            output_minimum: -10.0,
            output_maximum: 10.0,
            rate_minimum: -100.0,
            rate_maximum: 100.0,
            tracking_time: 1.0,
            anti_windup: AntiWindup::BackCalculation,
        }
    }

    #[test]
    fn a_record_without_integral_or_derivative_terms_is_a_proportional_candidate() {
        let candidate =
            candidate_from_control_pid(parameters(), CandidateSource::Manual).expect("valid");
        assert_eq!(candidate.structure, Structure::P);
        assert!(!candidate.applied);
    }

    #[test]
    fn the_output_minimum_must_be_below_the_maximum_for_a_candidate() {
        let mut bad = parameters();
        bad.output_minimum = 10.0;
        assert_eq!(
            candidate_from_control_pid(bad, CandidateSource::Manual)
                .expect_err("reversed")
                .0,
            "Control.PID output minimum must be below output maximum."
        );
    }

    #[test]
    fn a_generated_pi_takes_ki_from_ti_and_the_defaults_of_the_loop() {
        let tuning = PiTuning {
            method: TuningMethod::Simc,
            objective: TuningObjective::Balanced,
            proportional_gain: 2.0,
            integral_time_seconds: 4.0,
            closed_loop_time_seconds: Some(2.0),
        };
        let candidate = generated_candidate(&tuning, CandidateLimits::default()).expect("valid");
        assert_eq!(candidate.structure, Structure::PI);
        assert_eq!(candidate.source, CandidateSource::Simc);
        assert_eq!(candidate.parameters.integral_gain, 0.5);
        assert_eq!(candidate.parameters.output_maximum, 100.0);
        assert_eq!(candidate.parameters.rate_maximum, DEFAULT_RATE_LIMIT_PER_S);
        assert_eq!(candidate.parameters.tracking_time, 4.0);
        assert_eq!(
            candidate.parameters.anti_windup,
            AntiWindup::BackCalculation
        );
    }

    #[test]
    fn the_runtime_record_is_imported_and_its_missing_field_is_named() {
        let record = json!({
            "ProportionalGain": 1.0, "IntegralGain": 0.2, "DerivativeGain": 0.0,
            "ProportionalSetpointWeight": 1.0, "DerivativeSetpointWeight": 0.0, "Bias": 0.0,
            "OutputMinimum": 0.0, "OutputMaximum": 100.0, "RateMinimum": -5.0, "RateMaximum": 5.0,
            "TrackingTime": 2.0, "AntiWindup": "Combined"
        });
        let imported = candidate_from_runtime(&record).expect("complete record");
        assert_eq!(imported.source, CandidateSource::Imported);
        assert_eq!(imported.parameters.anti_windup, AntiWindup::Combined);
        let mut missing = record.clone();
        missing.as_object_mut().expect("object").remove("Bias");
        assert_eq!(
            candidate_from_runtime(&missing)
                .expect_err("bias missing")
                .0,
            "Runtime controller parameter Bias is unavailable."
        );
    }

    #[test]
    fn a_settings_record_must_hold_exactly_the_required_fields() {
        let mut record = json!({
            "proportionalGain": 1.0, "integralGain": 0.0, "derivativeGain": 0.0,
            "proportionalSetpointWeight": 1.0, "derivativeSetpointWeight": 0.0, "bias": 0.0,
            "outputMinimum": 0.0, "outputMaximum": 100.0, "rateMinimum": -5.0, "rateMaximum": 5.0,
            "trackingTime": 2.0, "antiWindup": "BackCalculation"
        });
        assert!(validate_control_pid_settings(&record).is_ok());
        record
            .as_object_mut()
            .expect("object")
            .insert("extra".to_string(), json!(1));
        assert_eq!(
            validate_control_pid_settings(&record)
                .expect_err("extra field")
                .0,
            "Control.PID settings must contain every required field exactly once."
        );
        assert_eq!(
            validate_control_pid_settings(&json!(3))
                .expect_err("not an object")
                .0,
            "Control.PID settings must be a JSON object."
        );
    }

    #[test]
    fn a_settings_record_with_reversed_rate_limits_is_refused() {
        let record = json!({
            "proportionalGain": 1.0, "integralGain": 0.0, "derivativeGain": 0.0,
            "proportionalSetpointWeight": 1.0, "derivativeSetpointWeight": 0.0, "bias": 0.0,
            "outputMinimum": 0.0, "outputMaximum": 100.0, "rateMinimum": 5.0, "rateMaximum": -5.0,
            "trackingTime": 2.0, "antiWindup": "Combined"
        });
        assert_eq!(
            validate_control_pid_settings(&record)
                .expect_err("rate order")
                .0,
            "Control.PID rate minimum must not exceed rate maximum."
        );
    }
}

#[cfg(test)]
#[path = "candidate_ported_tests.rs"]
mod ported_tests;

#[cfg(test)]
#[path = "candidate_identify_tune_tests.rs"]
mod identify_tune_tests;
