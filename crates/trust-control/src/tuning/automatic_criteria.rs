//! The automatic test criteria (specification 35, B4.3; the Tune page specification, "Control requirements"): the record
//! that marks which test values were generated, and the policy that fills the missing ones from the selected model, its
//! identification recording and the configured limits (`automaticEngineerRequirements.ts`). A value the engineer declared
//! is never replaced; a generated value is marked, frozen before validation, and recalculated only when the model changes.
//!
//! The numbers the policy reads from the model (its timescale, its low-frequency response, its operating point and limits,
//! the baseline noise) are passed in as [`FillEvidence`], computed by the caller from `trust_control_math`. This module
//! decides the values and the sentences. Multichannel criteria are not part of this module (the multichannel stage).

#![allow(clippy::module_name_repetitions)] // `AutomaticCriteria` names the module's subject.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::experiment::number_text::js_to_precision;
use crate::identification::refusal::Refusal;
use crate::tuning::objective::{TuningMethod, TuningObjective, AUTOMATIC_OBJECTIVE};
use crate::tuning::requirements::{maximum_duration_seconds, TuningRequirements};
use crate::tuning::structure::is_exactly_zero;
use crate::tuning::texts;

/// The longest model revision a criteria record may name, and the longest field (`decodeAutomaticTestCriteria`).
const MAXIMUM_TEXT_LENGTH: usize = 256;
/// The most generated fields a criteria record may name (`decodeAutomaticTestCriteria`).
const MAXIMUM_FIELDS: usize = 70;
/// The fraction of the directional range that a test change may use (Tune page, "Control requirements": ten percent).
const TEST_CHANGE_FRACTION: f64 = 0.1;
/// The default settling band as a fraction of the test change (Tune page: two percent).
const BAND_FRACTION: f64 = 0.02;
/// The default error allowance as a fraction of the test change (Tune page: four percent).
const ERROR_FRACTION: f64 = 0.04;
/// The factor of the baseline noise that a test change must exceed (Tune page: "twenty times that estimate").
const NOISE_RATIO: f64 = 20.0;
/// The settling time's multiple of the model timescale (Tune page: "six times that scale").
const SETTLING_TIMESCALE_MULTIPLE: f64 = 6.0;
/// The duration's multiple of the slowest settling time, and of the delay (`roundTime(2·slowest + 2·delay)`).
const DURATION_MULTIPLE: f64 = 2.0;
/// The tolerance of the operating point against its limits (`automaticEngineerRequirements.ts`).
const OPERATING_TOLERANCE: f64 = 1e-7;
/// The tolerance of the test change against the output limits (`automaticEngineerRequirements.ts`).
const CHANGE_TOLERANCE: f64 = 1e-8;

/// The record of the generated test criteria (B4.3): the model revision they were generated from, and the requirement
/// keys that were generated (`referenceChange`, `maximumError`, …). A listed key is marked Automatic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutomaticCriteria {
    /// The revision of the model the values were generated from.
    pub model_revision: String,
    /// The requirement keys that were generated.
    pub fields: Vec<String>,
}

/// The requirement keys that a criteria record may name (`automaticRequirementKeys`).
pub const REQUIREMENT_KEYS: [&str; 6] = [
    "referenceChange",
    "maximumError",
    "settlingBand",
    "settlingTimeSeconds",
    "durationSeconds",
    "tuningObjective",
];

/// Checks a stored criteria record (`decodeAutomaticTestCriteria`): a non-empty revision of at most 256 characters, at
/// most 70 fields, each a requirement key (optionally with a channel after a colon) of at most 256 characters, and no
/// field named twice.
///
/// # Errors
/// Returns a [`Refusal`] with [`texts::CRITERIA_INVALID`] when the record breaks one of these rules.
pub fn validate_criteria(criteria: &AutomaticCriteria) -> Result<(), Refusal> {
    let revision_ok =
        !criteria.model_revision.is_empty() && criteria.model_revision.len() <= MAXIMUM_TEXT_LENGTH;
    let count_ok = criteria.fields.len() <= MAXIMUM_FIELDS;
    let names_ok = criteria.fields.iter().all(|field| {
        field.len() <= MAXIMUM_TEXT_LENGTH
            && REQUIREMENT_KEYS
                .iter()
                .any(|key| field.as_str() == *key || field.starts_with(&format!("{key}:")))
    });
    let distinct = criteria
        .fields
        .iter()
        .enumerate()
        .all(|(index, field)| !criteria.fields[..index].contains(field));
    if revision_ok && count_ok && names_ok && distinct {
        Ok(())
    } else {
        Err(Refusal::new(texts::CRITERIA_INVALID))
    }
}

/// The value that a requirement key names in a record: the shared value of `referenceChange`, or a channel's value of
/// `referenceChange:<channel>` (B4.3). `None` when the record leaves the key out, or the key names no value of the record (such as
/// `tuningObjective`).
#[must_use]
fn requirement_value(requirements: &TuningRequirements, key: &str) -> Option<f64> {
    let (name, channel) = match key.split_once(':') {
        Some((name, channel)) => (name, Some(channel)),
        None => (key, None),
    };
    match channel {
        None => match name {
            "referenceChange" => Some(requirements.reference_change),
            "maximumError" => Some(requirements.maximum_error),
            "settlingBand" => Some(requirements.settling_band),
            "settlingTimeSeconds" => Some(requirements.settling_time_seconds),
            "durationSeconds" => Some(requirements.duration_seconds),
            _ => None,
        },
        Some(channel) => {
            let output = requirements.channel_requirements.as_ref()?.get(channel)?;
            match name {
                "referenceChange" => Some(output.reference_change),
                "maximumError" => Some(output.maximum_error),
                "settlingBand" => Some(output.settling_band),
                "settlingTimeSeconds" => Some(output.settling_time_seconds),
                _ => None,
            }
        }
    }
}

/// Keeps Automatic on each generated requirement that a change leaves as it was (B4.3): a requirement whose value the change gives a new
/// value is no longer generated, and one that the new requirements leave out, or give the value they had, stays generated. The page sends
/// the record as the study holds it, and the engine decides which values stay generated. A change with no previous record is left as it is.
#[allow(clippy::float_cmp)] // A value is the same as before only when the engineer did not change it: the values are compared exactly (B4.3).
pub fn keep_generated_fields(previous: &TuningRequirements, next: &mut TuningRequirements) {
    let Some(before) = previous.automatic_criteria.as_ref() else {
        return;
    };
    let current = &*next;
    let fields: Vec<String> = before
        .fields
        .iter()
        .filter(|key| {
            requirement_value(current, key.as_str())
                .is_none_or(|value| requirement_value(previous, key.as_str()) == Some(value))
        })
        .cloned()
        .collect();
    if let Some(after) = next.automatic_criteria.as_mut() {
        after.fields = fields;
    }
}

/// The values the engineer supplied (or that a stored record holds) for the test. A value that is absent is the policy's
/// to fill; a value that is present is kept exactly (B4.3: "explicit values are never rounded or relaxed").
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SuppliedRequirements {
    /// The setpoint change.
    pub reference_change: Option<f64>,
    /// The error limit.
    pub maximum_error: Option<f64>,
    /// The settling band.
    pub settling_band: Option<f64>,
    /// The settling-time deadline.
    pub settling_time_seconds: Option<f64>,
    /// The validation duration.
    pub duration_seconds: Option<f64>,
    /// The constant input disturbance.
    pub constant_input_disturbance: Option<f64>,
    /// The measurement noise band.
    pub measurement_noise_band: Option<f64>,
    /// The gain uncertainty fraction.
    pub gain_uncertainty_fraction: Option<f64>,
    /// The Auto-entry tolerance.
    pub transfer_tolerance: Option<f64>,
    /// The overshoot limit, in percent.
    pub maximum_overshoot_percent: Option<f64>,
    /// The formula method, when the engineer chose one.
    pub tuning_method: Option<TuningMethod>,
    /// The declared response, when the engineer declared one.
    pub tuning_objective: Option<TuningObjective>,
}

impl SuppliedRequirements {
    /// The record with every requirement present, when the five required values all are (no generation is needed). The
    /// response is the declared one, or More robust for Automatic (B4.3).
    fn complete(&self) -> Option<TuningRequirements> {
        Some(TuningRequirements {
            channel_requirements: None,
            reference_change: self.reference_change?,
            maximum_error: self.maximum_error?,
            settling_band: self.settling_band?,
            settling_time_seconds: self.settling_time_seconds?,
            duration_seconds: self.duration_seconds?,
            constant_input_disturbance: self.constant_input_disturbance,
            measurement_noise_band: self.measurement_noise_band,
            gain_uncertainty_fraction: self.gain_uncertainty_fraction,
            transfer_tolerance: self.transfer_tolerance,
            maximum_overshoot_percent: self.maximum_overshoot_percent,
            tuning_method: self.tuning_method,
            tuning_objective: Some(self.tuning_objective.unwrap_or(AUTOMATIC_OBJECTIVE)),
            automatic_criteria: None,
        })
    }

    /// The values without those that a stored record generated: they are filled again, so that the generated ones follow
    /// the model while the engineer's values stay (B4.3, "retuning recalculates only generated values").
    fn without_generated(mut self, generated: &[String]) -> Self {
        for key in generated {
            match key.as_str() {
                "referenceChange" => self.reference_change = None,
                "maximumError" => self.maximum_error = None,
                "settlingBand" => self.settling_band = None,
                "settlingTimeSeconds" => self.settling_time_seconds = None,
                "durationSeconds" => self.duration_seconds = None,
                "tuningObjective" => self.tuning_objective = None,
                _ => {}
            }
        }
        self
    }
}

/// The numbers that Automatic reads from the model, its recording and its limits (B4.3). The caller computes them from the
/// selected model with `trust_control_math`; this module decides the test from them. The output and input limits are the
/// tightest of the signal range, the operating range and the recorded values.
#[derive(Debug, Clone, PartialEq)]
pub struct FillEvidence {
    /// The revision of the model the values are generated from.
    pub model_revision: String,
    /// The symbol and unit of the controlled output, as the sentences name them.
    pub output_symbol: String,
    /// The unit of the controlled output.
    pub output_unit: String,
    /// The symbol of the manipulated input, as the sentences name it.
    pub input_symbol: String,
    /// The task period in seconds (`taskPeriodMs / 1000`).
    pub period_seconds: f64,
    /// The model's resolved dead time in seconds.
    pub delay_seconds: f64,
    /// The conservative timescale of the model in seconds (Tune page: the modal analysis, the delay and the period).
    pub timescale_seconds: f64,
    /// The operating point's output.
    pub operating_output: f64,
    /// The operating point's input.
    pub operating_input: f64,
    /// The smallest output the test may reach.
    pub output_minimum: f64,
    /// The largest output the test may reach.
    pub output_maximum: f64,
    /// The smallest input the actuator and the model admit.
    pub input_minimum: f64,
    /// The largest input the actuator and the model admit.
    pub input_maximum: f64,
    /// The largest input rise per second: infinite when the actuator declares no rise limit, zero when it declares that
    /// the input cannot rise.
    pub input_rate_up_per_s: f64,
    /// The largest input fall per second: infinite when the actuator declares no fall limit, zero when it declares that
    /// the input cannot fall.
    pub input_rate_down_per_s: f64,
    /// The real part of the model's response at the test's low frequency: its direction.
    pub low_frequency_real: f64,
    /// The magnitude of the model's response at the test's low frequency: its size.
    pub low_frequency_magnitude: f64,
    /// The three-standard-deviation baseline noise of the recording; zero when none is known.
    pub baseline_noise: f64,
}

/// The test criteria of a model (`automaticEngineerRequirements`): the supplied values kept, every missing value filled from
/// the evidence, and the fields that were filled listed in the record. A complete declared record is kept as it is, so the
/// same requirements give the same design whether or not criteria were generated first.
///
/// # Errors
/// Returns a [`Refusal`] when the stored record does not decode, a supplied value is not a finite number, the operating
/// point is outside its limits, no test change is possible, the test change leaves the identified range, or the validation
/// window is longer than the runtime supports.
pub fn fill_requirements(
    evidence: &FillEvidence,
    supplied: SuppliedRequirements,
    previous: Option<&AutomaticCriteria>,
) -> Result<TuningRequirements, Refusal> {
    let generated: Vec<String> = match previous {
        Some(criteria) => {
            validate_criteria(criteria)?;
            criteria.fields.clone()
        }
        None => Vec::new(),
    };
    let supplied = supplied.without_generated(&generated);
    if generated.is_empty()
        && let Some(complete) = supplied
            .complete()
            .filter(|record| record.validate().is_ok())
    {
        return Ok(complete);
    }
    let excursions = test_excursions(evidence)?;
    let mut fields: Vec<String> = Vec::new();
    let change = reference_change(evidence, &supplied, &excursions, &mut fields)?;
    let (band, error) = tolerances(evidence, &supplied, change, &mut fields)?;
    let (settling, duration) = timing(evidence, &supplied, excursions.slew_time, &mut fields)?;
    let objective = tuning_objective(&supplied, &mut fields);
    Ok(TuningRequirements {
        channel_requirements: None,
        reference_change: change,
        maximum_error: error,
        settling_band: band,
        settling_time_seconds: settling,
        duration_seconds: duration,
        constant_input_disturbance: supplied.constant_input_disturbance,
        measurement_noise_band: supplied.measurement_noise_band,
        gain_uncertainty_fraction: supplied.gain_uncertainty_fraction,
        transfer_tolerance: supplied.transfer_tolerance,
        maximum_overshoot_percent: supplied.maximum_overshoot_percent,
        tuning_method: supplied.tuning_method,
        tuning_objective: Some(objective),
        automatic_criteria: (!fields.is_empty()).then(|| AutomaticCriteria {
            model_revision: evidence.model_revision.clone(),
            fields,
        }),
    })
}

/// The test change of the operating point: the sign and the size of the larger excursion, and the time the input takes to
/// slew through its headroom.
struct Excursions {
    /// The time the input takes to slew through its headroom (`slewTime`).
    slew_time: f64,
    /// `1.0` when the upward excursion is at least the downward one, otherwise `-1.0`.
    direction: f64,
    /// The larger of the two excursions.
    size: f64,
}

/// The excursions each way from the operating point and the input limits: the headroom of the input in each direction caps
/// the excursion of the model in that direction (`excursionFor` of the TypeScript). The operating point must lie within the
/// input limits.
fn test_excursions(evidence: &FillEvidence) -> Result<Excursions, Refusal> {
    let operating_input = evidence.operating_input;
    if operating_input < evidence.input_minimum - OPERATING_TOLERANCE
        || operating_input > evidence.input_maximum + OPERATING_TOLERANCE
    {
        return Err(Refusal::new(texts::review_operating_limits(
            &evidence.input_symbol,
        )));
    }
    let up = headroom(
        evidence.input_maximum - operating_input,
        evidence.input_rate_up_per_s,
    );
    let down = headroom(
        operating_input - evidence.input_minimum,
        evidence.input_rate_down_per_s,
    );
    let upward = excursion(evidence, 1.0, up, down);
    let downward = excursion(evidence, -1.0, up, down);
    Ok(Excursions {
        slew_time: slew_allowance(evidence, up, down),
        direction: if upward >= downward { 1.0 } else { -1.0 },
        size: upward.max(downward),
    })
}

/// The test change: the one the engineer supplied, or the larger of the two excursions with its direction. A change that
/// cannot be seen above the noise, or that leaves the output range, is refused (`referenceChange`).
fn reference_change(
    evidence: &FillEvidence,
    supplied: &SuppliedRequirements,
    excursions: &Excursions,
    fields: &mut Vec<String>,
) -> Result<f64, Refusal> {
    let size = excursions.size;
    if supplied.reference_change.is_none()
        && (size <= 1e-9 || size.is_nan() || size < NOISE_RATIO * evidence.baseline_noise)
    {
        return Err(Refusal::new(texts::no_automatic_change(
            &evidence.output_symbol,
        )));
    }
    let change = pick(
        fields,
        supplied.reference_change,
        excursions.direction * size,
        "referenceChange",
    )?;
    let minimum = evidence.output_minimum;
    let maximum = evidence.output_maximum;
    let operating = evidence.operating_output;
    if is_exactly_zero(change)
        || change.is_nan()
        || operating + change < minimum - CHANGE_TOLERANCE
        || operating + change > maximum + CHANGE_TOLERANCE
    {
        let room = (maximum - operating).max(operating - minimum);
        return Err(Refusal::new(texts::change_leaves_range(
            change,
            &evidence.output_symbol,
            &evidence.output_unit,
            minimum,
            maximum,
            room,
        )));
    }
    Ok(change)
}

/// The settling band and the error limit, each filled from the test change and the noise unless the engineer supplied it
/// (`settlingBand` and `maximumError`).
fn tolerances(
    evidence: &FillEvidence,
    supplied: &SuppliedRequirements,
    change: f64,
    fields: &mut Vec<String>,
) -> Result<(f64, f64), Refusal> {
    let noise = evidence.baseline_noise;
    let band_fallback = noise.max(
        (change.abs() * BAND_FRACTION).min(
            supplied
                .maximum_error
                .map_or(f64::INFINITY, |error| error / 2.0),
        ),
    );
    let band = pick(
        fields,
        supplied.settling_band,
        band_fallback,
        "settlingBand",
    )?;
    let error_fallback = (2.0 * band).max(change.abs() * ERROR_FRACTION);
    let error = pick(
        fields,
        supplied.maximum_error,
        error_fallback,
        "maximumError",
    )?;
    Ok((band, error))
}

/// The settling-time deadline and the validation duration, each filled from the model timescale, its delay and the slew
/// allowance, or from the settling time, unless the engineer supplied it (`settlingTimeSeconds` and `durationSeconds`).
fn timing(
    evidence: &FillEvidence,
    supplied: &SuppliedRequirements,
    slew_time: f64,
    fields: &mut Vec<String>,
) -> Result<(f64, f64), Refusal> {
    let period = evidence.period_seconds;
    let settling_fallback = round_time(
        SETTLING_TIMESCALE_MULTIPLE * evidence.timescale_seconds
            + evidence.delay_seconds
            + slew_time,
        period,
    );
    let settling = pick(
        fields,
        supplied.settling_time_seconds,
        settling_fallback,
        "settlingTimeSeconds",
    )?;
    let duration_fallback = round_time(
        DURATION_MULTIPLE * settling + DURATION_MULTIPLE * evidence.delay_seconds,
        period,
    );
    let duration = pick(
        fields,
        supplied.duration_seconds,
        duration_fallback,
        "durationSeconds",
    )?;
    if duration > maximum_duration_seconds() {
        return Err(Refusal::new(texts::DURATION_TOO_LONG));
    }
    Ok((settling, duration))
}

/// The response that the engineer declared, or the automatic one, which is then listed as generated (`tuningObjective`).
fn tuning_objective(supplied: &SuppliedRequirements, fields: &mut Vec<String>) -> TuningObjective {
    if let Some(declared) = supplied.tuning_objective {
        declared
    } else {
        fields.push("tuningObjective".to_string());
        AUTOMATIC_OBJECTIVE
    }
}

/// A requirement's value: the supplied one when it is a finite number, otherwise the fallback, rounded as `fill_value` gives
/// it, with the key recorded as generated. A supplied value that is not finite is refused.
fn pick(
    fields: &mut Vec<String>,
    value: Option<f64>,
    fallback: f64,
    key: &str,
) -> Result<f64, Refusal> {
    match value {
        Some(number) if number.is_finite() => Ok(number),
        Some(_) => Err(Refusal::new(texts::review_entered_value(key))),
        None => {
            fields.push(key.to_string());
            Ok(fill_value(key, fallback))
        }
    }
}

/// The value a generated field takes from its fallback, at the precision the Tune page shows (`pick` of the TypeScript):
/// the test change rounded down to three digits, the band and the error rounded up to two. Other fields keep the fallback.
fn fill_value(key: &str, fallback: f64) -> f64 {
    match key {
        "referenceChange" => engineering_value(fallback, 3, false),
        "maximumError" | "settlingBand" => engineering_value(fallback, 2, true),
        _ => fallback,
    }
}

/// The headroom of an input in one direction (`headroom` of the TypeScript): ten percent of the distance to the limit, and
/// none when the input cannot move in that direction (a declared rate of zero).
fn headroom(distance: f64, rate: f64) -> f64 {
    if is_exactly_zero(rate) {
        0.0
    } else {
        distance.max(0.0) * TEST_CHANGE_FRACTION
    }
}

/// The time the input takes to slew through its headroom, the longest of the two directions (`slewTime`).
fn slew_allowance(evidence: &FillEvidence, up: f64, down: f64) -> f64 {
    let up_time = if is_exactly_zero(evidence.input_rate_up_per_s) {
        0.0
    } else {
        up / evidence.input_rate_up_per_s
    };
    let down_time = if is_exactly_zero(evidence.input_rate_down_per_s) {
        0.0
    } else {
        down / evidence.input_rate_down_per_s
    };
    up_time.max(down_time).max(0.0)
}

/// The largest test change in one direction (`excursionFor` of the TypeScript): ten percent of the directional range,
/// capped by the input headroom that the model's response can use in that direction. The response's sign decides which
/// headroom a positive input uses; a response of exactly zero matches neither direction.
fn excursion(evidence: &FillEvidence, direction: f64, up: f64, down: f64) -> f64 {
    let range = if direction > 0.0 {
        evidence.output_maximum - evidence.operating_output
    } else {
        evidence.operating_output - evidence.output_minimum
    };
    let real = evidence.low_frequency_real;
    let sign_matches = if real > 0.0 {
        direction > 0.0
    } else if real < 0.0 {
        direction < 0.0
    } else {
        false
    };
    let capacity = evidence.low_frequency_magnitude * if sign_matches { up } else { down };
    (range.max(0.0) * TEST_CHANGE_FRACTION).min(capacity)
}

/// A duration or a settling time rounded up to a whole task period (`roundTime`).
fn round_time(seconds: f64, period: f64) -> f64 {
    let rounded = (engineering_value(seconds, 3, true) / period).ceil() * period;
    js_to_precision(rounded, 12)
}

/// A value rounded to `digits` significant digits, upward or downward in magnitude (`engineeringValue`).
fn engineering_value(value: f64, digits: i32, upward: bool) -> f64 {
    if is_exactly_zero(value) || !value.is_finite() {
        return value;
    }
    let unit = 10f64.powf(value.abs().log10().floor() - f64::from(digits) + 1.0);
    let scaled = value.abs() / unit;
    let whole = if upward {
        (scaled - 1e-9).ceil()
    } else {
        (scaled + 1e-9).floor()
    };
    let precision = usize::try_from(digits).unwrap_or(1);
    js_to_precision(value.signum() * whole * unit, precision)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)] // The rounded values and the declared values are compared exactly, on purpose.

    use super::*;

    /// A first-order plant of gain 1.5 with a timescale of `time_constant` seconds and a 0.5 s delay, an output range of
    /// ±0.5 around zero and an input range of ±10 around zero, with rate limits on the input.
    fn evidence(time_constant: f64) -> FillEvidence {
        FillEvidence {
            model_revision: format!("model-{time_constant}"),
            output_symbol: "Main.PV".to_string(),
            output_unit: "m".to_string(),
            input_symbol: "Main.MV".to_string(),
            period_seconds: 0.1,
            delay_seconds: 0.5,
            timescale_seconds: time_constant,
            operating_output: 0.0,
            operating_input: 0.0,
            output_minimum: -0.5,
            output_maximum: 0.5,
            input_minimum: -10.0,
            input_maximum: 10.0,
            input_rate_up_per_s: 5.0,
            input_rate_down_per_s: 5.0,
            low_frequency_real: 1.5,
            low_frequency_magnitude: 1.5,
            baseline_noise: 0.0,
        }
    }

    fn wide(time_constant: f64) -> FillEvidence {
        FillEvidence {
            output_minimum: -5.0,
            output_maximum: 5.0,
            ..evidence(time_constant)
        }
    }

    #[test]
    fn a_slower_model_gets_a_later_settling_deadline_and_a_longer_test() {
        let fast = fill_requirements(&evidence(2.0), SuppliedRequirements::default(), None)
            .expect("fills");
        let slow = fill_requirements(&evidence(8.0), SuppliedRequirements::default(), None)
            .expect("fills");
        assert!(slow.settling_time_seconds > fast.settling_time_seconds);
        assert!(slow.duration_seconds > slow.settling_time_seconds);
        assert!(fast.maximum_error > fast.settling_band);
        let criteria = fast
            .automatic_criteria
            .expect("generated values are marked");
        assert_eq!(criteria.model_revision, "model-2");
        assert!(criteria.fields.iter().any(|field| field == "maximumError"));
    }

    #[test]
    fn the_test_change_uses_at_most_a_tenth_of_the_directional_range() {
        // Range 0.5 at a tenth is 0.05; the headroom and the response allow more.
        let filled = fill_requirements(&evidence(4.0), SuppliedRequirements::default(), None)
            .expect("fills");
        assert!(
            filled.reference_change > 0.0 && filled.reference_change <= 0.05 + 1e-9,
            "{}",
            filled.reference_change
        );
    }

    #[test]
    fn automatic_uses_the_downward_movement_when_the_upper_headroom_is_gone() {
        let mut at_top = evidence(4.0);
        at_top.operating_input = 10.0;
        let filled = fill_requirements(&at_top, SuppliedRequirements::default(), None)
            .expect("fills downward");
        assert!(
            filled.reference_change < 0.0,
            "no upward actuation is available"
        );
    }

    #[test]
    fn a_complete_declared_record_is_kept_and_gets_the_more_robust_response() {
        let supplied = SuppliedRequirements {
            reference_change: Some(2.0),
            maximum_error: Some(0.4),
            settling_band: Some(0.1),
            settling_time_seconds: Some(45.0),
            duration_seconds: Some(90.0),
            ..SuppliedRequirements::default()
        };
        let complete = fill_requirements(&wide(4.0), supplied, None).expect("complete");
        assert_eq!(complete.maximum_error, 0.4);
        assert_eq!(complete.settling_band, 0.1);
        assert_eq!(complete.reference_change, 2.0);
        assert_eq!(
            complete.tuning_objective,
            Some(TuningObjective::Robust),
            "Automatic fills More robust"
        );
        assert!(
            complete.automatic_criteria.is_none(),
            "a complete declared record generates nothing"
        );
    }

    #[test]
    fn a_partly_supplied_record_fills_only_the_missing_settings() {
        let supplied = SuppliedRequirements {
            maximum_error: Some(0.4),
            settling_band: Some(0.1),
            settling_time_seconds: Some(45.0),
            reference_change: Some(2.0),
            ..SuppliedRequirements::default()
        };
        let filled = fill_requirements(&wide(4.0), supplied, None).expect("fills the rest");
        assert_eq!(filled.maximum_error, 0.4);
        assert_eq!(filled.settling_time_seconds, 45.0);
        assert!(filled.duration_seconds > 45.0);
        let fields = filled
            .automatic_criteria
            .expect("the duration and objective are generated")
            .fields;
        assert!(!fields.iter().any(|field| field == "maximumError"));
        assert!(fields.iter().any(|field| field == "durationSeconds"));
        assert!(fields.iter().any(|field| field == "tuningObjective"));
    }

    #[test]
    fn a_change_beyond_the_identified_range_says_how_to_fix_it() {
        let supplied = SuppliedRequirements {
            reference_change: Some(30.0),
            ..SuppliedRequirements::default()
        };
        let refusal = fill_requirements(&evidence(4.0), supplied, None).expect_err("out of range");
        assert!(
            refusal
                .0
                .contains("leaves the range the model was identified in"),
            "{}",
            refusal.0
        );
        assert!(
            refusal
                .0
                .contains("Identify with a larger step, or choose a change of at most"),
            "{}",
            refusal.0
        );
    }

    #[test]
    fn a_recalculated_record_keeps_the_engineers_values_and_follows_the_model() {
        let first = fill_requirements(
            &evidence(2.0),
            SuppliedRequirements {
                maximum_error: Some(0.4),
                ..SuppliedRequirements::default()
            },
            None,
        )
        .expect("first");
        let stored = first.automatic_criteria.clone().expect("generated");
        let recalculated = fill_requirements(
            &evidence(8.0),
            SuppliedRequirements {
                maximum_error: first.maximum_error.into(),
                ..SuppliedRequirements::default()
            },
            Some(&stored),
        )
        .expect("recalculated");
        assert_eq!(recalculated.maximum_error, 0.4);
        assert!(recalculated.settling_time_seconds > first.settling_time_seconds);
        assert_eq!(
            recalculated
                .automatic_criteria
                .expect("still generated")
                .model_revision,
            "model-8"
        );
    }

    #[test]
    fn stored_criteria_are_checked_before_they_are_reused() {
        let unknown = AutomaticCriteria {
            model_revision: "m".into(),
            fields: vec!["unknownKey".into()],
        };
        assert_eq!(
            validate_criteria(&unknown).expect_err("unknown key").0,
            "Invalid automatic test criteria."
        );
        let duplicate = AutomaticCriteria {
            model_revision: "m".into(),
            fields: vec!["maximumError".into(), "maximumError".into()],
        };
        assert!(validate_criteria(&duplicate).is_err());
        let channel = AutomaticCriteria {
            model_revision: "m".into(),
            fields: vec!["maximumError:y2".into()],
        };
        assert!(validate_criteria(&channel).is_ok());
    }

    #[test]
    fn generated_values_round_as_the_tune_page_shows_them() {
        assert_eq!(engineering_value(0.0137, 2, true), 0.014);
        assert_eq!(engineering_value(0.01379, 2, false), 0.013);
        assert_eq!(engineering_value(123.4, 3, true), 124.0);
    }
}

#[cfg(test)]
#[path = "automatic_criteria_ported_tests.rs"]
mod ported_tests;

#[cfg(test)]
#[path = "automatic_criteria_provenance_tests.rs"]
mod provenance_tests;
