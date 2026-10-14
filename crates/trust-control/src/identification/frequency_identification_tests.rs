//! Tests of the periodic frequency estimate: the startup samples are dropped, and the records must agree in length and leave
//! samples after the startup. The TypeScript wrapper is `estimatePeriodicFrequency`, whose startup rule is the one tested.

use std::f64::consts::PI;

use trust_control_math::frequency_identification::FrequencyRejection;

use super::estimate_periodic_frequency;

/// A sine of 64 samples at 0.1 s, one cycle per 6.4 s, after `prefix` junk samples in both records.
fn records(prefix: usize) -> (Vec<f64>, Vec<f64>) {
    let frequency = 2.0 * PI / 6.4;
    let mut input = vec![1000.0; prefix];
    let mut output = vec![3000.0; prefix];
    for index in 0..64 {
        let value = (frequency * f64::from(index) * 0.1).sin();
        input.push(value);
        output.push(2.0 * value);
    }
    (input, output)
}

#[test]
fn the_startup_samples_are_dropped_before_the_estimate() {
    // The junk prefix would break the static gain of 2 if it were used; dropped, the estimate is exactly 2 at the tone.
    let (input, output) = records(40);
    let frequency = [2.0 * PI / 6.4];
    let points = estimate_periodic_frequency(&input, &output, 0.1, &frequency, 2, 0.8, 40)
        .expect("the tail holds two segments of 32 samples");
    assert_eq!(points.len(), 1);
    assert!((points[0].magnitude - 2.0).abs() < 1e-9, "{:?}", points[0]);
    assert!(points[0].phase_radians.abs() < 1e-9);
    assert!((points[0].coherence - 1.0).abs() < 1e-9);
    // The same tail estimated without the prefix gives the same point.
    let (tail_input, tail_output) = records(0);
    let tail = estimate_periodic_frequency(&tail_input, &tail_output, 0.1, &frequency, 2, 0.8, 0)
        .expect("the same tail");
    assert_eq!(points, tail);
}

#[test]
fn records_of_unequal_length_or_without_samples_after_startup_are_insufficient() {
    let (input, output) = records(0);
    let frequency = [1.0];
    assert_eq!(
        estimate_periodic_frequency(&input, &output[..10], 0.1, &frequency, 2, 0.8, 0),
        Err(FrequencyRejection::InsufficientData)
    );
    assert_eq!(
        estimate_periodic_frequency(&input, &output, 0.1, &frequency, 2, 0.8, input.len()),
        Err(FrequencyRejection::InsufficientData)
    );
}
