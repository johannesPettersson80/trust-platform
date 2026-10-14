// The probe tests compare exact step counts and the half range of exact samples.
#![allow(clippy::float_cmp)]

use super::*;
use crate::commissioning::fixture_loops::first_order_model;
use crate::model::general::decode_general_model;
use serde_json::json;

fn good(value: f64) -> RecordedValue {
    RecordedValue { good: true, value }
}

#[test]
fn the_resting_probe_holds_one_second_at_rest_in_steps_of_the_trial() {
    // Transfer-noise test of the `TypeScript`: a 20 ms trial rests for one second, and at most 51 cycles run.
    let probe = resting_probe(Some(20.0));
    assert_eq!(probe.duration_ms, 1000.0, "one second at rest");
    assert_eq!(probe.max_total_cycles, 51);
    assert_eq!(probe.step_ms, 20.0);
    assert_eq!(
        resting_probe(None),
        probe,
        "a trial without a step probes with 20 ms"
    );
    assert_eq!(
        resting_probe(Some(0.0)),
        probe,
        "a step that is not positive is 20 ms too"
    );
}

#[test]
fn the_resting_probe_takes_at_least_ten_steps_however_slow_the_trial() {
    let slow = resting_probe(Some(1000.0));
    assert_eq!((slow.duration_ms, slow.max_total_cycles), (10_000.0, 11));
    let steps = resting_probe(Some(100.0));
    assert_eq!((steps.duration_ms, steps.max_total_cycles), (1000.0, 11));
}

#[test]
fn the_half_range_of_the_good_samples_is_the_noise_amplitude() {
    // Transfer-noise test of the `TypeScript`: half of the range 0.0007 − (−0.0009) is 0.0008.
    let samples = [0.0004, -0.0009, 0.0007, -0.0002].map(good);
    assert!((recorded_half_range(&samples).expect("four good samples") - 0.0008).abs() < 1e-12);
}

#[test]
fn a_half_range_needs_four_good_finite_samples() {
    let three = [good(1.0), good(2.0), good(3.0)];
    assert_eq!(recorded_half_range(&three), None);

    let bad = RecordedValue {
        good: false,
        value: 1000.0,
    };
    let with_bad = [good(0.0), good(1.0), good(0.5), good(0.25), bad];
    assert_eq!(
        recorded_half_range(&with_bad),
        Some(0.5),
        "the bad sample is not counted"
    );

    let not_finite = [good(0.0), good(1.0), good(0.5), good(f64::NAN)];
    assert_eq!(
        recorded_half_range(&not_finite),
        None,
        "three finite samples are not four"
    );
}

#[test]
fn the_mean_of_the_good_samples_is_their_average() {
    let samples = [
        good(1.0),
        good(2.0),
        good(3.0),
        RecordedValue {
            good: false,
            value: 100.0,
        },
    ];
    assert_eq!(recorded_mean(&samples), Some(2.0));
    assert_eq!(recorded_mean(&[]), None);
}

#[test]
fn the_transfer_check_starts_where_the_plant_rests_and_keeps_the_fitted_dynamics() {
    // Transfer-noise test of the `TypeScript`: a fitted operating point of 20.128 becomes the resting 20, and the dynamics stay.
    let fitted = decode_general_model(&json!({ "kind": "transfer-function", "revision": "fitted", "source": "fitted",
        "validation": "validated", "evidence": ["a", "b"], "delaySeconds": 0.7, "numerator": [1.44], "denominator": [3.7, 1],
        "inputs": [{ "id": "u", "unit": "%", "operatingRange": { "minimum": 0, "maximum": 100 } }],
        "outputs": [{ "id": "y", "unit": "°C", "operatingRange": { "minimum": 0, "maximum": 100 } }],
        "operatingPoint": { "inputs": [0], "outputs": [20.128] } }))
    .expect("the fitted model decodes");
    let plant = resting_operating_model(&fitted, &[Some(20.0)]);
    let point = plant
        .identity()
        .operating_point
        .clone()
        .expect("an operating point");
    assert_eq!((point.inputs, point.outputs), (vec![0.0], vec![20.0]));
    let GeneralModel::TransferFunction { transfer, .. } = &plant else {
        panic!("the model keeps its kind");
    };
    assert_eq!(
        (transfer.numerator.clone(), transfer.denominator.clone()),
        (vec![1.44], vec![3.7, 1.0])
    );
    assert_eq!(
        fitted
            .identity()
            .operating_point
            .as_ref()
            .map(|point| point.outputs.clone()),
        Some(vec![20.128]),
        "the selected model itself is unchanged"
    );
    assert_eq!(
        resting_operating_model(&fitted, &[None]),
        fitted,
        "a missing resting mean keeps the model"
    );
    assert_eq!(
        resting_operating_model(&first_order_model(), &[Some(2.5)])
            .identity()
            .operating_point
            .as_ref()
            .map(|point| point.outputs.clone()),
        Some(vec![2.5])
    );
}
