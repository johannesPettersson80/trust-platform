//! The actuator-ramp record of `control-studio-provisional-step.test.ts` (line 5) and its two checks: the ramp stays
//! inside the step, and the recommended model keeps its dead time and gain. Declared by `plant_tests.rs`, which split
//! them off to keep that file under about 1,000 lines.

use super::*;

/// The actuator-ramp record of `control-studio-provisional-step.test.ts` (line 5): 301 samples at `i / 10` s in % and
/// degC. The input is 20, rises by 2.5 per sample from sample 30 to 30 (the rate-limited ramp), holds at 30 and ends at
/// 27.5. The output starts at 50 and follows `y(i+1) = 20 + 1.5*u(max(0, i-5)) + (y(i) - 20 - 1.5*u(max(0, i-5)))*exp(-0.1/4)`.
fn actuator_ramp_record() -> Vec<IdentificationSample> {
    let mut samples = Vec::with_capacity(301);
    let mut inputs: Vec<f64> = Vec::with_capacity(301);
    let mut output = 50.0;
    for index in 0..=300_u32 {
        let input = if index < 30 {
            20.0
        } else if index < 34 {
            20.0 + f64::from(index - 29) * 2.5
        } else if index < 300 {
            30.0
        } else {
            27.5
        };
        inputs.push(input);
        samples.push(with_units(&sample(f64::from(index) / 10.0, input, output)));
        // The plant reads the input of five samples before, or the first one.
        let delayed = inputs[inputs.len().saturating_sub(6)];
        output = 20.0 + 1.5 * delayed + (output - 20.0 - 1.5 * delayed) * (-0.1_f64 / 4.0).exp();
    }
    samples
}

#[test]
fn an_actuator_ramp_stays_in_the_step_instead_of_being_read_as_a_restoration() {
    let samples = actuator_ramp_record();
    let result = identify_plant(
        &samples,
        &[],
        DataSource::Simulation,
        None,
        DEFAULT_MAXIMUM_NORMALIZED_ERROR,
        &IdentifyPlantOptions::default(),
    )
    .expect("a ramp caused by the actuator rate limit must remain in the step record");
    assert_eq!(result.partition, Some(Partition::SingleRun));
    // The delayed candidate is among the fitted models: the ramp stays inside the step, so the delayed first order is
    // fitted on the held step.
    let delayed = result
        .models
        .iter()
        .find(|model| model.kind == LowOrderModelKind::FirstOrderDeadTime)
        .expect("the delayed first order is fitted");
    let dead_time = delayed.dead_time_seconds.expect("a dead time");
    assert!(dead_time > 0.2, "dead time {dead_time}");
    assert!((delayed.gain - 1.5).abs() < 0.05, "gain {}", delayed.gain);
}

/// The recommended model of the actuator-ramp record (`control-studio-provisional-step.test.ts` line 5), with the
/// `TypeScript`'s tolerances: a dead time above 0.2 s and a gain within 0.05 of 1.5. The values come from an
/// independent computation of the grid fit (`fit_form`) on the held record: the first 300 samples, since `held_step_samples` cuts the record at its final drop, with the step at
/// sample 30. First order: gain 1.517187, time constant 4.658611 s, normalized error 0.021149. First order with delay:
/// gain 1.503132, time constant 4.125106 s, delay 0.4 s, normalized error 0.004434. The delay is longer than two sample
/// intervals (0.2 s) and its error is below 98 % of first order's, so N3.3 recommends first order with delay.
#[test]
fn the_recommended_model_of_the_actuator_ramp_keeps_its_dead_time_and_gain() {
    let samples = actuator_ramp_record();
    let result = identify_plant(
        &samples,
        &[],
        DataSource::Simulation,
        None,
        DEFAULT_MAXIMUM_NORMALIZED_ERROR,
        &IdentifyPlantOptions::default(),
    )
    .expect("a ramp caused by the actuator rate limit must remain in the step record");
    assert_eq!(result.partition, Some(Partition::SingleRun));
    assert_eq!(
        result.recommended.kind,
        LowOrderModelKind::FirstOrderDeadTime,
        "N3.3: the resolved delay is selected"
    );
    let dead_time = result
        .recommended
        .dead_time_seconds
        .expect("the recommended model has a dead time");
    assert!(dead_time > 0.2, "dead time {dead_time}");
    assert!(
        (result.recommended.gain - 1.5).abs() < 0.05,
        "gain {}",
        result.recommended.gain
    );
}
