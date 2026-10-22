//! The identification policy check of a loop file (`identification_policy.rs`): the limits and values the page's validator refuses.

use serde_json::{json, Value};

use super::check;

/// A policy that names every field, with values inside their rules.
fn complete() -> Value {
    json!({
        "maximumNormalizedError": 0.2,
        "maximumOrder": 4,
        "openLoopPermitted": true,
        "sustainedHoldPermitted": false,
        "information": "frequency",
        "process": "integrating",
    })
}

/// The complete policy with one field changed.
fn with(field: &str, value: Value) -> Value {
    let mut policy = complete();
    policy[field] = value;
    policy
}

#[test]
fn a_policy_with_its_required_fields_alone_is_accepted() {
    let minimal = json!({
        "openLoopPermitted": false,
        "sustainedHoldPermitted": true,
        "information": "basic",
    });
    assert_eq!(check(&minimal), Ok(()));
    assert_eq!(check(&complete()), Ok(()));
}

#[test]
fn the_normalised_error_is_above_zero_and_below_one() {
    for refused in [json!(0.0), json!(1.0), json!(-0.1), json!(null)] {
        assert!(
            check(&with("maximumNormalizedError", refused.clone())).is_err(),
            "{refused}"
        );
    }
    assert_eq!(check(&with("maximumNormalizedError", json!(0.999))), Ok(()));
}

#[test]
fn the_maximum_order_is_a_whole_number_from_one_to_eight() {
    for refused in [json!(0), json!(9), json!(2.5), json!(-1), json!(null)] {
        assert!(
            check(&with("maximumOrder", refused.clone())).is_err(),
            "{refused}"
        );
    }
    assert_eq!(check(&with("maximumOrder", json!(8))), Ok(()));
}

#[test]
fn the_permitted_tests_and_the_information_are_the_values_the_page_offers() {
    assert!(check(&with("openLoopPermitted", json!("yes"))).is_err());
    assert!(check(&with("sustainedHoldPermitted", json!(null))).is_err());
    assert!(check(&with("information", json!("all"))).is_err());
    assert!(check(&with("information", json!("multiple-modes"))).is_ok());
}

#[test]
fn the_process_is_absent_or_integrating_and_a_null_process_is_refused() {
    assert!(check(&with("process", json!("static"))).is_err());
    assert!(check(&with("process", json!(null))).is_err());
}

#[test]
fn a_policy_that_is_not_an_object_is_refused() {
    assert!(check(&json!([1, 2])).is_err());
    assert!(check(&json!("integrating")).is_err());
}
