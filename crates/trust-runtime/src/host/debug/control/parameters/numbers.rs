//! A parameter's number in its IEC type (protocol annex E7.1, E7.3 step 10): a set's numbers within their bounds
//! and representable, a REAL's rounded to single precision and a TIME's the seconds of the whole number of
//! nanoseconds TIME holds, which a write takes and a read gives back.

use std::collections::BTreeMap;

use super::Binding;

/// The numbers a set writes are its binding's exact members, within their bounds and representable in their IEC
/// types (E7.3 step 10).
pub(super) fn validate_candidate(
    binding: &Binding,
    candidate: &BTreeMap<String, f64>,
) -> Result<(), String> {
    if candidate.len() != binding.parameters.len() {
        return Err("exact parameter set required".into());
    }
    for parameter in &binding.parameters {
        let value = *candidate
            .get(&parameter.symbol)
            .ok_or("exact parameter set required")?;
        if !value.is_finite()
            || value < parameter.minimum
            || value > parameter.maximum
            || (parameter.iec_type == "REAL" && !(value as f32).is_finite())
        {
            return Err(format!(
                "{} outside declared bounds or IEC range: {value} is not within [{}, {}]",
                parameter.symbol, parameter.minimum, parameter.maximum
            ));
        }
        if parameter.iec_type == "REAL" && f64::from(value as f32) != value {
            return Err(
                "REAL values must be explicitly rounded to their runtime representation".into(),
            );
        }
        if parameter.iec_type == "TIME" && time_nanos(value).is_none() {
            return Err(format!(
                "{}: {TIME_NOT_WHOLE_NANOSECONDS}",
                parameter.symbol
            ));
        }
    }
    Ok(())
}

/// Refusal of a TIME value that is not the seconds of a whole number of nanoseconds (E7.3 step 10).
pub(super) const TIME_NOT_WHOLE_NANOSECONDS: &str =
    "TIME values must be explicitly rounded to whole nanoseconds, their runtime representation";

/// The nanoseconds of a TIME parameter's value `seconds` (E7.1, E7.3 step 10): those of the shortest decimal that reads
/// as `seconds`, the number as the client's JSON wrote it, when that decimal has at most nine digits after the point and
/// its nanoseconds `n` read back as `seconds` (`n / 10^9`); `None` for any other value and outside TIME's range. The
/// product `seconds x 10^9` in binary misses a whole nanosecond by half of one or more from 2^22 s (about 48.5 days)
/// on, for example 4194792.263 s, whose product is 4194792263000000.5.
pub(super) fn time_nanos(seconds: f64) -> Option<i64> {
    if !seconds.is_finite() {
        return None;
    }
    // `Display` writes the shortest decimal that reads back as the number, never with an exponent.
    let decimal = seconds.abs().to_string();
    let (whole, fraction) = decimal.split_once('.').unwrap_or((decimal.as_str(), ""));
    if fraction.len() > 9 {
        return None;
    }
    let nanos = whole
        .parse::<i64>()
        .ok()?
        .checked_mul(1_000_000_000)?
        .checked_add(format!("{fraction:0<9}").parse::<i64>().ok()?)?;
    let nanos = if seconds < 0.0 { -nanos } else { nanos };
    (time_seconds(nanos) == seconds).then_some(nanos)
}

/// The seconds of a TIME value of `nanos` nanoseconds, as a parameter's number and a transfer's TIME guard read it (E7.2,
/// E6.3).
pub(in super::super) fn time_seconds(nanos: i64) -> f64 {
    nanos as f64 / 1e9
}
