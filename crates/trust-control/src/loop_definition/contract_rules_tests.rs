//! The open-time rules of the page that the engine applies to the raw loop file (`contract_rules.rs`).

use serde_json::{json, Value};

use super::check;

/// A scenario with one timed write and one requirement, which the page accepts.
fn scenario(id: &str) -> Value {
    json!({
        "id": id,
        "initialValues": { "P1.Level": 1.0 },
        "events": [{ "atMs": 0, "symbol": "P1.Demand", "value": 2.0 }],
        "requirements": [{ "kind": "settle" }],
    })
}

#[test]
fn a_null_top_level_field_is_refused_by_its_name() {
    let error = check(&json!({ "channels": null })).expect_err("a null channels field");
    assert!(error.contains("channels"), "{error}");
    assert_eq!(check(&json!({ "channels": {} })), Ok(()));
}

#[test]
fn a_null_experiment_topology_or_identification_policy_is_refused_as_the_page_refuses_it() {
    for field in ["experiment", "topology", "identificationPolicy"] {
        let value = json!({ field: null });
        let error = check(&value).expect_err("a null field the page checks");
        assert!(error.contains(field), "{error}");
    }
    assert_eq!(check(&json!({ "topology": "open-loop" })), Ok(()));
}

#[test]
fn a_null_controller_law_is_refused_and_an_absent_one_is_accepted() {
    let error = check(&json!({ "controller": { "law": null } })).expect_err("a null law");
    assert!(error.contains("controller.law"), "{error}");
    assert_eq!(check(&json!({ "controller": {} })), Ok(()));
}

#[test]
fn the_plant_identifier_is_text_that_is_not_blank_when_present() {
    assert!(check(&json!({ "plantId": "  " })).is_err());
    assert!(check(&json!({ "plantId": 7 })).is_err());
    assert_eq!(check(&json!({ "plantId": "tank-1" })), Ok(()));
    assert_eq!(check(&json!({})), Ok(()));
}

#[test]
fn the_analysis_plant_units_are_the_units_of_the_loop_signals() {
    let signals = json!({
        "manipulatedVariable": { "unit": "%" },
        "processVariable": { "unit": "degC" },
    });
    let agreeing = json!({
        "signals": signals.clone(),
        "analysis": { "plant": { "inputUnit": "%", "outputUnit": "degC" } },
    });
    assert_eq!(check(&agreeing), Ok(()));
    let mismatched = json!({
        "signals": signals.clone(),
        "analysis": { "plant": { "inputUnit": "L/min", "outputUnit": "degC" } },
    });
    assert!(check(&mismatched).is_err());
    let no_plant = json!({ "signals": signals, "analysis": {} });
    assert_eq!(
        check(&no_plant),
        Ok(()),
        "an analysis without a plant is left to its own decode"
    );
}

#[test]
fn a_proportional_law_holds_only_its_kind_and_its_gain_symbol() {
    let exact =
        json!({ "controller": { "law": { "kind": "proportional", "gainSymbol": "P1.Kp" } } });
    assert_eq!(check(&exact), Ok(()));
    let extra = json!({ "controller": { "law": { "kind": "proportional", "gainSymbol": "P1.Kp", "offset": 0 } } });
    assert!(check(&extra).is_err());
}

#[test]
fn a_parallel_pid_initialization_holds_exactly_its_six_keys() {
    let state = json!({ "symbol": "P1.Integral" });
    let exact = json!({ "controller": { "pidInitialization": {
        "kind": "parallel-pid-v1",
        "integralState": state.clone(),
        "derivativeState": state.clone(),
        "proportionalSetpointWeight": 1.0,
        "derivativeSetpointWeight": 0.0,
        "maximumSnapshotAgeMs": 500,
    } } });
    assert_eq!(check(&exact), Ok(()));
    let mut extra = exact.clone();
    extra["controller"]["pidInitialization"]["note"] = json!("extra");
    assert!(check(&extra).is_err());
    let mut missing = exact.clone();
    missing["controller"]["pidInitialization"]
        .as_object_mut()
        .expect("an object")
        .remove("maximumSnapshotAgeMs");
    assert!(check(&missing).is_err());
}

#[test]
fn the_verification_scenarios_have_the_structure_the_verify_run_needs() {
    assert_eq!(
        check(&json!({ "verificationScenarios": [scenario("step")] })),
        Ok(())
    );
    assert!(check(&json!({ "verificationScenarios": [scenario("target-response")] })).is_err());
    assert!(
        check(&json!({ "verificationScenarios": [scenario("step"), scenario("step")] })).is_err()
    );
    assert!(check(&json!({ "verificationScenarios": {} })).is_err());
    let mut negative = scenario("late");
    negative["events"][0]["atMs"] = json!(-1);
    assert!(check(&json!({ "verificationScenarios": [negative] })).is_err());
    let mut none = scenario("bare");
    none["requirements"] = json!([]);
    assert!(check(&json!({ "verificationScenarios": [none] })).is_err());
    let mut text_value = scenario("text");
    text_value["initialValues"]["P1.Level"] = json!("1.0");
    assert!(check(&json!({ "verificationScenarios": [text_value] })).is_err());
    let too_many: Vec<Value> = (0..25)
        .map(|index| scenario(&format!("s{index}")))
        .collect();
    assert!(check(&json!({ "verificationScenarios": too_many })).is_err());
}

#[test]
fn a_null_at_any_depth_is_refused_except_in_the_fields_the_page_does_not_read() {
    let array_item =
        check(&json!({ "signals": { "additional": [null] } })).expect_err("a null item");
    assert!(array_item.contains("signals.additional[0]"), "{array_item}");
    let nested = check(&json!({ "controller": { "pidInitialization": {
        "kind": "control-pid-v1", "applicationStates": null } } }))
    .expect_err("a null list");
    assert!(
        nested.contains("controller.pidInitialization.applicationStates"),
        "{nested}"
    );
    let optional = check(&json!({ "limits": { "rateUpPerS": null } })).expect_err("a null rate");
    assert!(optional.contains("limits.rateUpPerS"), "{optional}");
    assert_eq!(
        check(&json!({ "cascade": null, "operatingPoint": null })),
        Ok(())
    );
    assert_eq!(
        check(&json!({ "experiment": { "relayDirection": null } })),
        Ok(())
    );
    assert_eq!(
        check(&json!({ "limits": { "expectedSettlingSeconds": null } })),
        Ok(())
    );
}

#[test]
fn a_signal_with_a_blank_unit_is_dimensionless_when_the_analysis_units_are_compared() {
    let blank = json!({
        "signals": { "manipulatedVariable": { "unit": "  " }, "processVariable": { "unit": "1" } },
        "analysis": { "plant": { "inputUnit": "1", "outputUnit": "1" } },
    });
    assert_eq!(
        check(&blank),
        Ok(()),
        "a blank unit reads as \"1\", as the page states it"
    );
    let empty_against_empty = json!({
        "signals": { "manipulatedVariable": { "unit": "%" }, "processVariable": { "unit": "" } },
        "analysis": { "plant": { "inputUnit": "%", "outputUnit": "" } },
    });
    assert!(
        check(&empty_against_empty).is_err(),
        "the page compares \"1\" with \"\""
    );
}

#[test]
fn an_initial_index_is_a_safe_integer_as_the_page_reads_it() {
    let with_index = |index: Value| {
        json!({ "controller": { "project": { "initialization": { "kind": "declared-state-v1", "states": [
            { "symbol": "P1.X", "initial": { "source": "model-input", "index": index } }] } } } })
    };
    assert_eq!(check(&with_index(json!(3))), Ok(()));
    assert_eq!(check(&with_index(json!(9_007_199_254_740_991_u64))), Ok(()));
    assert!(check(&with_index(json!(9_007_199_254_740_992_u64))).is_err());
}

#[test]
fn a_control_pid_initialization_holds_only_its_declared_keys() {
    let exact = json!({ "controller": { "pidInitialization": {
        "kind": "control-pid-v1", "maximumSnapshotAgeMs": 500, "states": [] } } });
    assert_eq!(check(&exact), Ok(()));
    let mut extra = exact.clone();
    extra["controller"]["pidInitialization"]["note"] = json!("extra");
    assert!(check(&extra).is_err());
}
