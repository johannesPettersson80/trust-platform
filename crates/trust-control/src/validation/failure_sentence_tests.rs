//! The sentence of a failed requirement (specification 35, B5.5): it names the measured value and the limit in the units of the
//! loop's signals, with the words the extension used for the recorded target error and the response that did not settle.

use super::failed_requirement_sentence;
use crate::identification::independent_validation::RequirementStatus;
use crate::messages::{LimitSide, RequirementChannel, RequirementOutcome, ValueRange};

/// A failed requirement of the measurement `Main.PV`, whose result holds `value` against `limit` in `unit`.
fn failed(
    id: &str,
    label: &str,
    unit: &str,
    value: Option<f64>,
    limit: Option<f64>,
    side: LimitSide,
) -> RequirementOutcome {
    RequirementOutcome {
        id: id.to_owned(),
        label: label.to_owned(),
        channel: RequirementChannel {
            role: "Measurement".to_owned(),
            unit: unit.to_owned(),
            symbol: "Main.PV".to_owned(),
        },
        limit_side: side,
        status: RequirementStatus::Fail,
        value,
        limit,
        unit: unit.to_owned(),
        source: "model model-4".to_owned(),
        reason: "Observed measurement exceeds the declared limit.".to_owned(),
        automatic: false,
        guard: false,
        measured_range: None,
        limit_range: None,
    }
}

#[test]
fn the_recorded_target_error_names_its_value_and_its_limit_in_the_loops_unit() {
    let error = failed(
        "target-error",
        "Maximum steady-state error",
        "degC",
        Some(0.8935),
        Some(0.1),
        LimitSide::AtMost,
    );
    assert_eq!(
        failed_requirement_sentence(&error).as_deref(),
        Some("Recorded target error 0.8935 °C exceeds 0.1 °C.")
    );
    let mut second_output = error;
    second_output.id = "P1.VesselC:target-error".to_owned();
    assert_eq!(
        failed_requirement_sentence(&second_output).as_deref(),
        Some("Recorded target error 0.8935 °C exceeds 0.1 °C."),
        "the error of an output of a loop with several outputs reads the same"
    );
}

#[test]
fn a_response_that_did_not_settle_names_its_limit_and_when_it_settled() {
    let late = failed(
        "response-deadline",
        "Maximum settling time",
        "s",
        Some(20.0),
        Some(15.0),
        LimitSide::AtMost,
    );
    assert_eq!(
        failed_requirement_sentence(&late).as_deref(),
        Some("The recorded response did not settle within 15 s. It settled after 20 s.")
    );
    let never = failed(
        "response-deadline",
        "Maximum settling time",
        "s",
        None,
        Some(15.0),
        LimitSide::AtMost,
    );
    assert_eq!(
        failed_requirement_sentence(&never).as_deref(),
        Some("The recorded response did not settle within 15 s."),
        "a response that never arrived in the band has no time to name"
    );
}

#[test]
fn any_other_requirement_is_named_by_its_label_with_the_side_of_its_limit() {
    let overshoot = failed(
        "overshoot",
        "Maximum overshoot",
        "%",
        Some(12.5),
        Some(10.0),
        LimitSide::AtMost,
    );
    assert_eq!(
        failed_requirement_sentence(&overshoot).as_deref(),
        Some("Maximum overshoot: 12.5 % exceeds the limit of 10 %.")
    );
    let dwell = failed(
        "minimum-on-time",
        "Minimum ON time",
        "s",
        Some(3.0),
        Some(5.0),
        LimitSide::AtLeast,
    );
    assert_eq!(
        failed_requirement_sentence(&dwell).as_deref(),
        Some("Minimum ON time: 3 s is below the limit of 5 s.")
    );
}

#[test]
fn a_range_requirement_names_the_range_it_took_and_the_bounds_it_broke() {
    let mut range = failed(
        "process-bounds",
        "Operating range",
        "%",
        Some(4.0),
        Some(0.0),
        LimitSide::AtMost,
    );
    range.measured_range = Some(ValueRange {
        minimum: 12.0,
        maximum: 104.0,
    });
    range.limit_range = Some(ValueRange {
        minimum: 0.0,
        maximum: 100.0,
    });
    assert_eq!(
        failed_requirement_sentence(&range).as_deref(),
        Some("Operating range: 12–104 % is outside the declared 0–100 %.")
    );
    let mut fixed = failed(
        "fixed-parameter:Main.Kp",
        "Parameter unchanged",
        "",
        Some(2.5),
        Some(0.0),
        LimitSide::AtMost,
    );
    fixed.measured_range = Some(ValueRange {
        minimum: 3.5,
        maximum: 3.5,
    });
    fixed.limit_range = Some(ValueRange {
        minimum: 1.0,
        maximum: 1.0,
    });
    assert_eq!(
        failed_requirement_sentence(&fixed).as_deref(),
        Some("Parameter unchanged: 3.5 differs from the declared 1."),
        "bounds of one value are a fixed value"
    );
    // Without a good value in its window the check took no range, and the size of its violation is no value against a limit: the failure
    // keeps the requirement's own reason.
    let mut unmeasured = failed(
        "process-bounds",
        "Operating range",
        "%",
        Some(4.0),
        Some(0.0),
        LimitSide::AtMost,
    );
    unmeasured.limit_range = Some(ValueRange {
        minimum: 0.0,
        maximum: 100.0,
    });
    assert_eq!(
        failed_requirement_sentence(&unmeasured),
        None,
        "a range that was not measured is not named"
    );
}

#[test]
fn a_requirement_that_did_not_fail_or_holds_no_number_has_no_sentence() {
    let mut passing = failed(
        "target-error",
        "Maximum steady-state error",
        "degC",
        Some(0.05),
        Some(0.1),
        LimitSide::AtMost,
    );
    passing.status = RequirementStatus::Pass;
    assert_eq!(failed_requirement_sentence(&passing), None);
    passing.status = RequirementStatus::Inconclusive;
    assert_eq!(failed_requirement_sentence(&passing), None);
    for (value, limit) in [
        (None, Some(0.1)),
        (Some(f64::NAN), Some(0.1)),
        (Some(0.5), None),
        (Some(0.5), Some(f64::INFINITY)),
    ] {
        let unnamed = failed(
            "target-error",
            "Maximum steady-state error",
            "degC",
            value,
            limit,
            LimitSide::AtMost,
        );
        assert_eq!(
            failed_requirement_sentence(&unnamed),
            None,
            "{value:?} against {limit:?} names no value and limit"
        );
    }
}
