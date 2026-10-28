use super::*;
use crate::value::{StructValue, Value};
use indexmap::IndexMap;
use smol_str::SmolStr;
use std::sync::Arc;

/// A `Control.PID` record whose fields are zero unless `set` names them (specification 45 §2.4).
fn record(set: &[(&str, f64)]) -> Value {
    let fields = [
        "ProportionalGain",
        "IntegralGain",
        "DerivativeGain",
        "ProportionalSetpointWeight",
        "DerivativeSetpointWeight",
        "Bias",
        "OutputMinimum",
        "OutputMaximum",
        "RateMinimum",
        "RateMaximum",
        "TrackingTime",
    ]
    .into_iter()
    .map(|name| {
        let value = set
            .iter()
            .find(|(field, _)| *field == name)
            .map_or(0.0, |(_, value)| *value);
        (SmolStr::new(name), Value::LReal(value))
    })
    .collect::<IndexMap<_, _>>();
    Value::Struct(Arc::new(StructValue::from_canonical_parts(
        SmolStr::new("PIDParameters"),
        fields,
    )))
}

/// Specification 45 §2.4 and protocol annex E7.7 ("a bounds check of each value"): a record is in
/// the controller's domain on each boundary of an inequality, equal output limits, zero rate limits
/// and no tracking time without integral action, and outside it on the far side of each one, or
/// with a number that is not finite. Catches a strict comparison where the domain allows equality,
/// and a check that forgets one of the three rules.
#[test]
fn a_controller_record_is_checked_against_the_operating_domain_of_specification_45() {
    assert_eq!(validate_record(&record(&[])), Ok(()));
    assert_eq!(
        validate_record(&record(&[("OutputMinimum", 5.0), ("OutputMaximum", 5.0)])),
        Ok(())
    );
    assert_eq!(
        validate_record(&record(&[("IntegralGain", 0.5), ("TrackingTime", 0.1)])),
        Ok(())
    );

    let refusal = validate_record(&record(&[("OutputMinimum", 5.0), ("OutputMaximum", 4.0)]))
        .expect_err("output limits in the wrong order");
    assert!(
        refusal.contains("OutputMinimum 5 exceeds OutputMaximum 4"),
        "{refusal}"
    );
    let refusal =
        validate_record(&record(&[("RateMinimum", 0.5)])).expect_err("a positive falling rate");
    assert!(
        refusal.contains("RateMinimum 0.5 and RateMaximum 0"),
        "{refusal}"
    );
    let refusal =
        validate_record(&record(&[("RateMaximum", -0.5)])).expect_err("a negative rising rate");
    assert!(
        refusal.contains("RateMinimum 0 and RateMaximum -0.5"),
        "{refusal}"
    );
    let refusal = validate_record(&record(&[("IntegralGain", 0.5)]))
        .expect_err("integral action without a tracking time");
    assert!(
        refusal.contains("TrackingTime 0 with IntegralGain 0.5"),
        "{refusal}"
    );
    assert!(
        validate_record(&record(&[("IntegralGain", -0.5), ("TrackingTime", -1.0)])).is_err(),
        "a negative tracking time with negative integral action"
    );
    let refusal = validate_record(&record(&[("Bias", f64::INFINITY)]))
        .expect_err("a number that is not finite");
    assert!(refusal.contains("'Bias' must be finite"), "{refusal}");
}
