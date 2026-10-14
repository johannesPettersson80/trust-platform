// Ported from `control-studio-identify-tune.test.ts`, `control-studio-provisional-step.test.ts` and
// `control-studio-long-process-fit.test.ts`: the `identifyPlant` assertions, numbers kept. The choice of the recommended form
// follows the one rule of N3.3 (`selection.rs`). Where the TypeScript expected the first
// order, the expectation follows the specification, with the reason in each comment.

use std::collections::VecDeque;

use trust_control_math::identify::SampleQuality;

use super::*;

/// A good sample without units.
fn sample(time_seconds: f64, input: f64, output: f64) -> IdentificationSample {
    IdentificationSample {
        time_seconds,
        input,
        output,
        quality: SampleQuality::Good,
        input_unit: None,
        output_unit: None,
        clipped: None,
        saturated: None,
    }
}

/// A first-order plant with gain 1.5, a 4 s time constant and a five-sample (0.5 s) delay, stepped by `amplitude` at
/// 2 s and sampled every 0.1 s for 30 s (`fopdtSamples`).
pub(crate) fn fopdt_samples(amplitude: f64) -> Vec<IdentificationSample> {
    first_order_with_delay(1.5, 4.0, 5, amplitude)
}

/// A first-order plant with gain `gain`, time constant `tau_seconds` and a delay of `delay_samples` samples, stepped by
/// `amplitude` at 2 s and sampled every 0.1 s for 30 s: the same Euler recursion as `fopdtSamples`.
fn first_order_with_delay(
    gain: f64,
    tau_seconds: f64,
    delay_samples: usize,
    amplitude: f64,
) -> Vec<IdentificationSample> {
    let dt = 0.1;
    let mut delayed = VecDeque::from(vec![0.0; delay_samples]);
    let mut pv = 0.0;
    let mut samples = Vec::with_capacity(301);
    for index in 0..=300 {
        let input = if index >= 20 { amplitude } else { 0.0 };
        delayed.push_back(input);
        let applied = delayed.pop_front().unwrap_or(0.0);
        pv += dt * (gain * applied - pv) / tau_seconds;
        samples.push(sample(f64::from(index) * dt, input, pv));
    }
    samples
}

/// A pumped tank (`delayedIntegratorSamples`): the level ramps at 0.0004 m/s per % of pump command, 2 s after a 10 %
/// step at 2 s.
fn delayed_integrator_samples() -> Vec<IdentificationSample> {
    let dt = 0.1;
    let mut delayed = VecDeque::from([0.0; 20]);
    let mut level = 1.0;
    let mut samples = Vec::with_capacity(601);
    for index in 0..=600 {
        let input = if index >= 20 { 10.0 } else { 0.0 };
        delayed.push_back(input);
        level += dt * 0.0004 * delayed.pop_front().unwrap_or(0.0);
        samples.push(sample(f64::from(index) * dt, input, level));
    }
    samples
}

/// The sample with the units of the long-process records: a plant input in % and an output in °C.
fn with_units(sample: &IdentificationSample) -> IdentificationSample {
    IdentificationSample {
        input_unit: Some("%".to_string()),
        output_unit: Some("°C".to_string()),
        ..sample.clone()
    }
}

/// A held step from 75 to 85 input units at `step_seconds`, sampled every 10 s from 0 to `end_seconds`
/// (`stepRecordAt`); the output rises from 35 by `response(elapsed)`.
fn step_record_at(
    end_seconds: f64,
    step_seconds: f64,
    response: impl Fn(f64) -> f64,
) -> Vec<IdentificationSample> {
    let mut samples = Vec::new();
    let mut time_seconds = 0.0;
    while time_seconds <= end_seconds {
        let input = if time_seconds >= step_seconds {
            85.0
        } else {
            75.0
        };
        let output = 35.0 + response((time_seconds - step_seconds).max(0.0));
        samples.push(with_units(&sample(time_seconds, input, output)));
        time_seconds += 10.0;
    }
    samples
}

/// A slow first-order response: 20 units with a 4500 s time constant.
fn slow_first_order(elapsed: f64) -> f64 {
    20.0 * (1.0 - (-elapsed / 4500.0).exp())
}

/// A true integrator: 20 units over 2300 s, without curvature.
fn ramp(elapsed: f64) -> f64 {
    (20.0 / 2300.0) * elapsed
}

/// A slow first-order plant (gain 2 per input unit, time constant 4500 s) held after a step of `amplitude` at 100 s and
/// sampled every 10 s to `end_seconds`: the record of the validated integrator rule.
fn slow_recording(end_seconds: f64, amplitude: f64) -> Vec<IdentificationSample> {
    let mut samples = Vec::new();
    let mut time_seconds = 0.0;
    while time_seconds <= end_seconds {
        let input = if time_seconds >= 100.0 {
            75.0 + amplitude
        } else {
            75.0
        };
        let elapsed = (time_seconds - 100.0).max(0.0);
        let output = 35.0 + 2.0 * amplitude * (1.0 - (-elapsed / 4500.0).exp());
        samples.push(sample(time_seconds, input, output));
        time_seconds += 10.0;
    }
    samples
}

/// The options of a loop whose identification policy declares an integrating process.
fn declared_integrating() -> IdentifyPlantOptions {
    IdentifyPlantOptions {
        declared_process: Some("integrating".to_string()),
        closed_loop: false,
        pulse: false,
    }
}

/// `identifyPlant(samples)`: a simulated recording, no exclusions, no validation, the default limit and no options.
fn identify(samples: &[IdentificationSample]) -> IdentificationResult {
    identify_with(
        samples,
        DEFAULT_MAXIMUM_NORMALIZED_ERROR,
        &IdentifyPlantOptions::default(),
    )
}

/// A single-run identification of a simulated recording with the given limit and options.
fn identify_with(
    samples: &[IdentificationSample],
    maximum_normalized_error: f64,
    options: &IdentifyPlantOptions,
) -> IdentificationResult {
    identify_plant(
        samples,
        &[],
        DataSource::Simulation,
        None,
        maximum_normalized_error,
        options,
    )
    .expect("the recording is identified")
}

/// A first-order model with the given gain and time constant and no delay, for the fraction of its held step.
fn fitted_first_order(gain: f64, time_constant_seconds: f64) -> IdentifiedModel {
    IdentifiedModel {
        kind: LowOrderModelKind::FirstOrder,
        gain,
        time_constant_seconds: Some(time_constant_seconds),
        second_time_constant_seconds: None,
        dead_time_seconds: None,
        normalized_error: Some(0.0),
        normalized_estimation_error: None,
        normalized_validation_error: None,
        estimation_residuals: None,
        validation_residuals: None,
        uncertainty: None,
        stable: true,
        on_search_boundary: false,
    }
}

#[test]
fn a_delayed_integrator_is_fitted_with_its_rate_gain_and_delay() {
    // Specification N2.1 item 4 and N2.2: the integrating form is fitted with the rate gain of the ramp and the delay of its
    // start, 2 s after the step, and it is exact on this record. The first order with delay also fits within the limit, but
    // its time constant grows to the largest grid value, 10·H = 580 s (a search-boundary finding of N2.4), and it reaches
    // only 9 % of its fitted final change over the hold, so it is not acceptable (N2.3). The integrating form is therefore the
    // simplest acceptable one and is recommended. Its first-order-with-delay fit is within the limit with a time constant
    // longer than the 58 s hold, so the integrator rule refuses it (N2.3a): the data cannot yet tell a ramp from a slow lag.
    let result = identify(&delayed_integrator_samples());
    let integrating = result
        .models
        .iter()
        .find(|model| model.kind == LowOrderModelKind::Integrating)
        .expect("the integrating form is fitted");
    assert!(
        (integrating.gain - 0.0004).abs() < 2e-5,
        "rate gain {}",
        integrating.gain
    );
    let delay = integrating.dead_time_seconds.unwrap_or(0.0);
    assert!(
        (delay - 2.0).abs() < 0.2,
        "the ramp starts 2 s after the step, so the grid has the delay it needs: {delay}"
    );
    assert_eq!(result.recommended.kind, LowOrderModelKind::Integrating);
    let refusal = result.integrator_refusal.unwrap_or_default();
    assert!(
        refusal.contains("still rising like a ramp after 0.97 min"),
        "{refusal}"
    );
}

#[test]
fn exclusions_leave_the_samples_unchanged_and_physical_evidence_is_named() {
    let samples = fopdt_samples(2.0);
    let original = samples.clone();
    let excluded = [ExcludedInterval {
        start_seconds: 0.0,
        end_seconds: 0.1,
    }];
    let result = identify_plant(
        &samples,
        &excluded,
        DataSource::Physical,
        None,
        DEFAULT_MAXIMUM_NORMALIZED_ERROR,
        &IdentifyPlantOptions::default(),
    )
    .expect("the recording is identified");
    assert_eq!(result.source, ResultSource::FromPhysical);
    assert_eq!(samples, original, "the raw samples are not rewritten");
}

#[test]
fn the_simplest_acceptable_low_order_model_is_selected() {
    // Specification N3.3: the first order and the first order with delay are both acceptable on this record. The delay of
    // 0.4 s is longer than two sample intervals (0.2 s), and the delayed form's whole-response error (0.0012) is far below
    // 98 % of the first order's (0.0173). The resolved delay is therefore selected, and the delay is read as about 0.4 s.
    // The specification (N3.3) does not select the first order here, as the TypeScript expected.
    let result = identify(&fopdt_samples(2.0));
    assert_eq!(result.source, ResultSource::FromSimulation);
    assert_eq!(
        result.recommended.kind,
        LowOrderModelKind::FirstOrderDeadTime
    );
    assert!(
        (result.recommended.gain - 1.5).abs() < 0.15,
        "gain {}",
        result.recommended.gain
    );
    let time_constant = result
        .recommended
        .time_constant_seconds
        .expect("a time constant");
    assert!(
        (time_constant - 4.0).abs() < 0.8,
        "time constant {time_constant}"
    );
    let delay = result
        .recommended
        .dead_time_seconds
        .expect("the resolved delay is reported");
    assert!((delay - 0.5).abs() < 0.2, "dead time {delay}");
    let first_order = result
        .models
        .iter()
        .find(|model| model.kind == LowOrderModelKind::FirstOrder)
        .expect("the first order is fitted");
    // The single-run candidate carries a zero delay (the TypeScript's `deadTimeSeconds: 0`), which presents as none.
    assert!(
        first_order.dead_time_seconds.unwrap_or(0.0) <= 0.0,
        "the first order has no delay"
    );
    for kind in [
        LowOrderModelKind::StaticGain,
        LowOrderModelKind::Integrating,
        LowOrderModelKind::FirstOrder,
        LowOrderModelKind::SecondOrder,
        LowOrderModelKind::SecondOrderDeadTime,
    ] {
        assert!(
            result.models.iter().any(|model| model.kind == kind),
            "{kind:?} is fitted"
        );
    }
    assert!(result
        .models
        .iter()
        .all(|model| model.normalized_error.is_some_and(f64::is_finite)));
}

#[test]
fn a_first_order_plant_with_a_delay_is_identified_as_first_order_with_delay_on_the_single_run_path()
{
    // The example of the numerical annex: K = 1.5, T = 4 s, L = 1 s, sampled every 0.1 s, stepped at 2 s. The first order with
    // delay is recommended (N3.3) with K 1.50, T 4.0 s and the delay read to 0.9 s on the sample grid.
    let result = identify(&first_order_with_delay(1.5, 4.0, 10, 2.0));
    assert_eq!(
        result.recommended.kind,
        LowOrderModelKind::FirstOrderDeadTime
    );
    assert!(
        (result.recommended.gain - 1.5).abs() < 0.05,
        "gain {}",
        result.recommended.gain
    );
    let time_constant = result
        .recommended
        .time_constant_seconds
        .expect("a time constant");
    assert!(
        (time_constant - 4.0).abs() < 0.5,
        "time constant {time_constant}"
    );
    let delay = result.recommended.dead_time_seconds.expect("a dead time");
    assert!((delay - 1.0).abs() < 0.2, "dead time {delay}");
}

#[test]
fn a_first_order_plant_with_a_delay_is_identified_as_first_order_with_delay_on_the_validated_path()
{
    // The same plant as the single-run test, fitted on a step of 2 and scored on a step of 1.25 (N3.4). The validated fit's
    // delay is 0.8 s, longer than two sample intervals, and its error 0.0045 is below 98 % of the first order's 0.0179.
    let estimation = first_order_with_delay(1.5, 4.0, 10, 2.0);
    let validation = first_order_with_delay(1.5, 4.0, 10, 1.25);
    let result = identify_plant(
        &estimation,
        &[],
        DataSource::Simulation,
        Some(validation.as_slice()),
        DEFAULT_MAXIMUM_NORMALIZED_ERROR,
        &IdentifyPlantOptions::default(),
    )
    .expect("the validated fit qualifies");
    assert_eq!(result.partition, Some(Partition::IndependentRuns));
    assert_eq!(
        result.recommended.kind,
        LowOrderModelKind::FirstOrderDeadTime
    );
    assert!(
        (result.recommended.gain - 1.5).abs() < 0.1,
        "gain {}",
        result.recommended.gain
    );
    let delay = result.recommended.dead_time_seconds.expect("a dead time");
    assert!((delay - 1.0).abs() < 0.25, "dead time {delay}");
}

#[test]
fn a_plant_without_dynamics_is_identified_as_static_gain() {
    // Specification N2.1 item 1: y − y0 = K·(u − u0). The output follows the input at once, so the static gain is the
    // simplest acceptable form, and its least-squares gain is the plant's 1.5 exactly (N2.2 with s_k = d_k).
    let samples: Vec<IdentificationSample> = (0..301_u32)
        .map(|index| {
            let input = if index >= 20 { 2.0 } else { 0.0 };
            sample(f64::from(index) * 0.1, input, 1.5 * input)
        })
        .collect();
    let result = identify(&samples);
    assert_eq!(result.recommended.kind, LowOrderModelKind::StaticGain);
    assert!(
        (result.recommended.gain - 1.5).abs() < 1e-9,
        "gain {}",
        result.recommended.gain
    );
    assert!(result.recommended.dead_time_seconds.is_none());
}

#[test]
fn a_first_order_plant_with_a_time_constant_longer_than_the_sample_interval_is_not_a_static_gain() {
    // Specification N2.1 item 1: a static gain follows the input at once, so it cannot fit a response that takes several
    // sample intervals to rise (T = 4 s against 0.1 s samples). Its whole-response error is above the limit.
    let result = identify(&fopdt_samples(2.0));
    assert_ne!(result.recommended.kind, LowOrderModelKind::StaticGain);
    let static_gain = result
        .models
        .iter()
        .find(|model| model.kind == LowOrderModelKind::StaticGain)
        .expect("the static gain is fitted");
    assert!(
        static_gain.decisive_error() > DEFAULT_MAXIMUM_NORMALIZED_ERROR,
        "static gain error {}",
        static_gain.decisive_error()
    );
}

#[test]
fn an_acceptable_model_is_kept_when_a_bounded_experiment_restores_its_input() {
    let samples: Vec<IdentificationSample> = (0..=172)
        .map(|index| {
            let time_seconds = f64::from(index) * 0.3;
            let input = if (11.4..50.4).contains(&time_seconds) {
                30.0
            } else {
                20.0
            };
            let response_time = (time_seconds - 11.4 - 0.9).max(0.0);
            let output = 30.0 + 2.0 * (1.0 - (-response_time / 5.0).exp());
            sample(time_seconds, input, output)
        })
        .collect();
    let result = identify(&samples);
    // Specification N3.3 and N3.2: the first order and the first order with delay are both acceptable on this held step,
    // and the delay of 0.9 s is longer than two sample intervals (0.6 s) and improves the error (0.0028 against 0.0285). The
    // delayed form is the simplest acceptable form in the sense of N3.3, so it is recommended, where the TypeScript expected the first
    // order.
    assert_eq!(
        result.recommended.kind,
        LowOrderModelKind::FirstOrderDeadTime
    );
    assert!(
        result.recommended.decisive_error() <= 0.15,
        "normalized error {}",
        result.recommended.decisive_error()
    );
}

#[test]
fn the_held_part_of_a_bounded_step_is_fitted_before_its_final_restoration() {
    let mut samples = fopdt_samples(2.0);
    let last_output = samples.last().expect("a last sample").output;
    samples.push(sample(30.1, 0.0, last_output));
    let result = identify(&samples);
    // The held-step gain of this simulated record is 1.5. Its least-squares gain (N2.2) is close to that value but
    // not exactly it, so the check uses the tolerance of the simplest-model test. The restoration sample lies outside
    // the hold (B3.4 step 5).
    assert!(
        (result.recommended.gain - 1.5).abs() < 0.15,
        "the final restoration must not replace the held-step response used for gain estimation: {}",
        result.recommended.gain
    );
}

#[test]
fn a_model_is_qualified_on_a_separate_immutable_validation_run() {
    let validation = fopdt_samples(1.25);
    let result = identify_plant(
        &fopdt_samples(2.0),
        &[],
        DataSource::Simulation,
        Some(validation.as_slice()),
        0.02,
        &IdentifyPlantOptions::default(),
    )
    .expect("the validated fit qualifies");
    assert_eq!(result.partition, Some(Partition::IndependentRuns));
    // Specification N3.3: on this record the first order (validation error 0.0179) and the first order with delay (0.0045,
    // delay 0.3 s) both meet the limit 0.02 in both checks, and the resolved delay is selected, where the TypeScript expected the first
    // order.
    assert_eq!(
        result.recommended.kind,
        LowOrderModelKind::FirstOrderDeadTime
    );
    let error = result
        .recommended
        .normalized_validation_error
        .expect("a validation error");
    assert!(error <= 0.03, "validation error {error}");
    let residuals = result
        .recommended
        .validation_residuals
        .as_ref()
        .expect("validation residuals");
    assert_eq!(residuals.len(), 301);
    let uncertainty = result
        .recommended
        .uncertainty
        .as_ref()
        .expect("an uncertainty");
    assert_eq!(
        uncertainty.method,
        UncertaintyMethod::GridResolutionAndValidationResidual
    );
}

#[test]
fn unusable_data_is_refused_with_its_deterministic_reason() {
    let samples = [
        sample(0.0, 1.0, 1.0),
        IdentificationSample {
            quality: SampleQuality::Bad,
            ..sample(1.0, 1.0, f64::NAN)
        },
    ];
    let refusal = identify_plant(
        &samples,
        &[],
        DataSource::Simulation,
        None,
        DEFAULT_MAXIMUM_NORMALIZED_ERROR,
        &IdentifyPlantOptions::default(),
    )
    .expect_err("unusable data is refused");
    assert!(
        refusal.0.to_lowercase().contains("finite good samples"),
        "{refusal}"
    );
}

#[test]
fn a_flat_output_is_refused_with_the_output_response_sentence() {
    // N3.2: a record whose output does not change has no span to normalise its errors by.
    let samples: Vec<IdentificationSample> = (0..301_u32)
        .map(|index| {
            let input = if index >= 20 { 2.0 } else { 0.0 };
            sample(f64::from(index) * 0.1, input, 0.0)
        })
        .collect();
    let refusal = identify_plant(
        &samples,
        &[],
        DataSource::Simulation,
        None,
        DEFAULT_MAXIMUM_NORMALIZED_ERROR,
        &IdentifyPlantOptions::default(),
    )
    .expect_err("a flat output is refused");
    assert_eq!(refusal.0, OUTPUT_TOO_SMALL);
}

#[cfg(test)]
#[path = "plant_ramp_tests.rs"]
mod ramp_tests;

#[test]
fn a_slow_first_order_response_is_refused_as_an_integrator_the_data_cannot_yet_tell_apart() {
    // N2.3a: a 4500 s time constant stepped for 2300 s looks like a ramp, and the integrator rule refuses an integrating form
    // on it, naming the hold. The first order reaches only 40 % of its fitted final change over the hold, and the first order
    // with delay the same, so neither is acceptable (N2.3); the integrating form has no fraction, so the error limit alone
    // accepts it (N2.3), and it is recommended. The rule then refuses it, because the first-order-with-delay fit is within the
    // limit with a time constant of 4453 s, longer than the hold.
    let samples = step_record_at(2400.0, 100.0, slow_first_order);
    let refusal = integrator_refusal_for(
        &IdentifyPlantOptions::default(),
        &samples,
        DEFAULT_MAXIMUM_NORMALIZED_ERROR,
    )
    .unwrap_or_default();
    assert!(
        refusal.contains(
            "still rising like a ramp after 38 min; it cannot yet be told whether it levels off. Hold the step for about "
        ),
        "{refusal}"
    );
    let result = identify(&samples);
    assert_eq!(result.recommended.kind, LowOrderModelKind::Integrating);
    assert_eq!(result.integrator_refusal.as_deref(), Some(refusal.as_str()));
}

#[test]
fn a_declared_integrating_process_skips_the_integrator_rule() {
    // N2.3a: a policy that declares an integrating process skips the integrator rule; the same slow record is refused by the
    // rule without the declaration and not with it.
    let samples = step_record_at(2400.0, 100.0, slow_first_order);
    assert!(integrator_refusal_for(
        &IdentifyPlantOptions::default(),
        &samples,
        DEFAULT_MAXIMUM_NORMALIZED_ERROR,
    )
    .is_some());
    assert_eq!(
        integrator_refusal_for(
            &declared_integrating(),
            &samples,
            DEFAULT_MAXIMUM_NORMALIZED_ERROR
        ),
        None
    );
}

#[test]
fn a_true_integrator_without_curvature_is_refused_unless_declared_or_closed_loop() {
    // A ramp has no curvature: a first-order fit with a time constant far beyond the hold fits it as well, so the integrator
    // rule cannot tell the two apart (N2.3a). A declared integrating process and a closed-loop fit skip the rule.
    let samples = step_record_at(2400.0, 100.0, ramp);
    let refusal = integrator_refusal_for(
        &IdentifyPlantOptions::default(),
        &samples,
        DEFAULT_MAXIMUM_NORMALIZED_ERROR,
    )
    .unwrap_or_default();
    assert!(refusal.contains("still rising like a ramp"), "{refusal}");
    assert_eq!(
        integrator_refusal_for(
            &declared_integrating(),
            &samples,
            DEFAULT_MAXIMUM_NORMALIZED_ERROR
        ),
        None
    );
    let closed_loop = IdentifyPlantOptions {
        declared_process: None,
        closed_loop: true,
        pulse: false,
    };
    assert_eq!(
        integrator_refusal_for(&closed_loop, &samples, DEFAULT_MAXIMUM_NORMALIZED_ERROR),
        None
    );
}

#[test]
fn a_validated_integrating_choice_on_a_short_estimation_hold_is_refused_by_the_integrator_rule() {
    // N2.3a on the validated path: a slow first order (4500 s) held for 200 s after its step. At the limit 0.03 its first and
    // second order forms miss the limit or reach less than 63 % of their final change (the second order reaches 30 %, and the
    // first order's time constant lies on the grid's upper edge, 10·H = 2000 s), so the integrating form is the simplest
    // acceptable one. The rule then refuses it, naming the hold, until the policy declares an integrating process.
    let estimation = slow_recording(300.0, 10.0);
    let validation = slow_recording(300.0, 5.0);
    let result = identify_plant(
        &estimation,
        &[],
        DataSource::Simulation,
        Some(validation.as_slice()),
        0.03,
        &IdentifyPlantOptions::default(),
    )
    .expect("the validated fit is identified");
    assert_eq!(result.recommended.kind, LowOrderModelKind::Integrating);
    let refusal = result.integrator_refusal.unwrap_or_default();
    assert!(
        refusal.contains("still rising like a ramp after 3.3 min"),
        "{refusal}"
    );
    let declared = identify_plant(
        &estimation,
        &[],
        DataSource::Simulation,
        Some(validation.as_slice()),
        0.03,
        &declared_integrating(),
    )
    .expect("the validated fit is identified");
    assert_eq!(declared.integrator_refusal, None);
}

#[test]
fn the_step_amplitude_is_the_mean_hold_input_less_the_baseline_mean_when_the_input_ramps_to_its_final_value(
) {
    // N2.2: Δu = ū_hold − u₀. The input is 0 before the step at sample 10, which takes 1.0; it then ramps 1.25, 1.5, 1.75 and
    // holds 2.0 to the end of the 301 samples. The hold has 291 samples, with 1.0, 1.25, 1.5 and 1.75 once and 2.0 for the other
    // 287. Its mean is (5.5 + 2.0 × 287) / 291 = 2 − 2.5 / 291 ≈ 1.991409, so Δu = 1.991409 − 0 (the final tenth gave 2.0).
    let samples: Vec<IdentificationSample> = (0..301_u32)
        .map(|index| {
            let input = match index {
                0..=9 => 0.0,
                10 => 1.0,
                11 => 1.25,
                12 => 1.5,
                13 => 1.75,
                _ => 2.0,
            };
            let output = if index < 10 { 0.0 } else { 2.0 };
            sample(f64::from(index) * 0.1, input, output)
        })
        .collect();
    let step = measure_step(&samples).expect("the ramped step is measured");
    assert_eq!(step.index, 10);
    let expected = 2.0 - 2.5 / 291.0;
    // The output changes by 2.0 at once, so both measured gain bounds are 2.0 / Δu.
    let (gain_minimum, gain_maximum) = step.gain_range.expect("a held step has endpoint gains");
    for bound in [gain_minimum, gain_maximum] {
        assert!(
            (bound - 2.0 / expected).abs() <= 1e-12,
            "gain bound {bound} against {}",
            2.0 / expected
        );
    }
}

#[test]
fn a_held_step_that_reaches_half_of_its_fitted_final_change_is_refused_with_its_fraction_and_hold()
{
    // A10.3 and N2.3: the fitted final change is K·Δu = 2.0 × 1.0 = 2.0. The held output settles at 1.0, which is 50 % of it,
    // below 63 %. The refusal names the fraction and the hold L + 3·Tdom = 0 + 3 × 10 s = 30 s, which is 0.5 min.
    let model = fitted_first_order(2.0, 10.0);
    let samples: Vec<IdentificationSample> = (0..=30_u32)
        .map(|index| {
            let (input, output) = if index < 20 { (0.0, 0.0) } else { (1.0, 1.0) };
            sample(f64::from(index) * 0.1, input, output)
        })
        .collect();
    let refusal =
        unreliable_step_refusal(&model, &samples, &samples).expect("the held step is refused");
    assert!(
        refusal.contains("reached 50 % of its fitted final change"),
        "{refusal}"
    );
    assert!(
        refusal.contains("hold the step for about 0.5 min"),
        "{refusal}"
    );
}

#[test]
fn a_pure_first_order_sampled_every_10_s_is_first_order_and_its_delay_is_not_resolved() {
    // Specification N2.2 and N3.3: a first-order lag T = 1,500 s, sampled every 10 s, stepped from 0 to 1 at 100 s and held to
    // 6,000 s. The first order fits with gain 0.997 and T = 1,483 s (1.1 % short of 1,500 s). The first order with delay has
    // a fitted delay of 10 s, one sample, which does not exceed two sample intervals (20 s), so the resolved-delay rule keeps
    // the first order. The old 2 % crossing seed read a delay of 20 to 30 s here.
    let samples: Vec<IdentificationSample> = (0..=600_u32)
        .map(|index| {
            let time_seconds = f64::from(index) * 10.0;
            let input = if time_seconds >= 100.0 { 1.0 } else { 0.0 };
            let output = if time_seconds >= 100.0 {
                1.0 - (-(time_seconds - 100.0) / 1500.0).exp()
            } else {
                0.0
            };
            sample(time_seconds, input, output)
        })
        .collect();
    let result = identify(&samples);
    assert_eq!(result.recommended.kind, LowOrderModelKind::FirstOrder);
    let first_order = result
        .models
        .iter()
        .find(|model| model.kind == LowOrderModelKind::FirstOrder)
        .expect("the first order is fitted");
    assert!(
        (first_order.gain - 1.0).abs() <= 0.02,
        "gain {}",
        first_order.gain
    );
    let time_constant = first_order.time_constant_seconds.expect("a time constant");
    assert!(
        (time_constant / 1500.0 - 1.0).abs() <= 0.02,
        "time constant {time_constant}"
    );
    let delayed = result
        .models
        .iter()
        .find(|model| model.kind == LowOrderModelKind::FirstOrderDeadTime)
        .expect("the first order with delay is fitted");
    let delay = delayed.dead_time_seconds.unwrap_or(0.0);
    assert!(
        delay <= 10.0 + 1e-9,
        "dead time {delay} is within one sample of zero"
    );
}

#[test]
fn a_partial_held_step_is_refused_with_its_fraction_and_the_hold_its_time_constant_suggests() {
    // Specification A10.3 and N2.3: a first-order lag with gain 1 and T = 600 s, stepped from 0 to 1 at 60 s, with a hold of
    // 300 s (T = 2H). The response reaches 38 % of its fitted final change, below 63 %. At the policy limit 0.02 the integrating
    // form is outside the limit, so the first order is the simplest form within it and no form is acceptable (N2.3); the
    // refusal names the fraction, the 5 min held so far and the hold 3·T̂ ≈ 1,819 s, about 30 min, which the fitted time
    // constant suggests (A10.3). Under the limit 0.15 the integrating form would be accepted and refused by the integrator rule.
    let samples: Vec<IdentificationSample> = (0..=36_u32)
        .map(|index| {
            let time_seconds = f64::from(index) * 10.0;
            let input = if time_seconds >= 60.0 { 1.0 } else { 0.0 };
            let output = if time_seconds >= 60.0 {
                1.0 - (-(time_seconds - 60.0) / 600.0).exp()
            } else {
                0.0
            };
            sample(time_seconds, input, output)
        })
        .collect();
    let result = identify_with(&samples, 0.02, &IdentifyPlantOptions::default());
    assert_eq!(result.recommended.kind, LowOrderModelKind::FirstOrder);
    let refusal = result.refusal.expect("the partial response is refused");
    assert!(
        refusal.contains("reached 38 % of its fitted final change"),
        "{refusal}"
    );
    assert!(refusal.contains("in 5 min"), "{refusal}");
    assert!(
        refusal.contains("hold the step for about 30 min"),
        "{refusal}"
    );
}

#[test]
fn a_ramped_hold_gives_its_fraction_and_the_fit_recovers_the_plant_driven_by_the_recorded_ramp() {
    // Specification N2.2 and N2.3: the input ramps 0.25 per sample to 1.0 over four samples after the first change at 1 s,
    // and is held to 30 s. The output is the first order with delay of gain 1.5, T = 4 s and L = 1 s on the recorded input
    // (N3.1). The fit recovers gain 1.499, T = 3.98 s and L = 1.0 s, and the fraction is defined because the input reaches its
    // final value. The step amplitude is the mean of the 291 held samples from the first change, 289.5 / 291 = 0.9948 (the
    // four ramp samples sum to 2.5, the 287 samples at 1.0 to 287), less the baseline mean of zero.
    use trust_control_math::identify::response::LowOrderShape;
    use trust_control_math::identify::simulate::unit_response;
    let times: Vec<f64> = (0..301_u32).map(|index| f64::from(index) * 0.1).collect();
    let inputs: Vec<f64> = (0..301_u32)
        .map(|index| match index {
            0..=9 => 0.0,
            10..=13 => 0.25 * f64::from(index - 9),
            _ => 1.0,
        })
        .collect();
    let shape = LowOrderShape {
        time_constant_seconds: Some(4.0),
        second_time_constant_seconds: None,
        dead_time_seconds: Some(1.0),
    };
    let response = unit_response(
        LowOrderModelKind::FirstOrderDeadTime,
        shape,
        &times,
        &inputs,
    );
    let samples: Vec<IdentificationSample> = times
        .iter()
        .zip(&inputs)
        .zip(&response)
        .map(|((&time_seconds, &input), &unit)| sample(time_seconds, input, 1.5 * unit))
        .collect();
    let result = identify(&samples);
    assert_eq!(
        result.recommended.kind,
        LowOrderModelKind::FirstOrderDeadTime
    );
    assert!(
        (result.recommended.gain - 1.5).abs() <= 0.02,
        "gain {}",
        result.recommended.gain
    );
    let delay = result.recommended.dead_time_seconds.expect("a dead time");
    assert!((delay - 1.0).abs() <= 0.1 + 1e-9, "dead time {delay}");
    let evidence = held_step_evidence(&result.recommended, &samples)
        .expect("a ramp to its final value is a held step");
    let fraction = evidence.fraction.expect("the fraction is defined");
    assert!((0.9..=1.1).contains(&fraction), "fraction {fraction}");
    let held = &inputs[10..];
    let amplitude =
        held.iter().sum::<f64>() / f64::from(u32::try_from(held.len()).expect("291 samples"));
    assert!(
        (evidence.response_size - result.recommended.gain.abs() * amplitude).abs() <= 1e-9,
        "response size {} against |K·Δu| with Δu {amplitude}",
        evidence.response_size
    );
}

#[test]
fn the_same_recording_gives_the_same_identification_every_time() {
    // Determinism (N2.2): two identifications of the same recording are equal, value for value.
    let samples = first_order_with_delay(1.5, 4.0, 10, 2.0);
    assert_eq!(identify(&samples), identify(&samples));
}

/// A first-order plant with gain 1.5 and a 4 s time constant, driven by a held input of `amplitude` for `width_samples`
/// samples from sample 20 (2 s), sampled every 0.1 s for 30 s. The output is the exact sampled first-order response, so a
/// width of 30 samples is a 3 s pulse that returns to its baseline, and a width of 281 samples holds the step to the end.
fn first_order_pulse(amplitude: f64, width_samples: u32) -> Vec<IdentificationSample> {
    let decay = (-0.1_f64 / 4.0).exp();
    let mut output = 0.0;
    let mut samples = Vec::with_capacity(301);
    for index in 0..=300_u32 {
        let input = if (20..20 + width_samples).contains(&index) {
            amplitude
        } else {
            0.0
        };
        samples.push(sample(f64::from(index) * 0.1, input, output));
        output = decay * output + (1.0 - decay) * 1.5 * input;
    }
    samples
}

/// The options of a single run that is a pulse (N2.2).
fn pulse_options() -> IdentifyPlantOptions {
    IdentifyPlantOptions {
        pulse: true,
        ..IdentifyPlantOptions::default()
    }
}

#[test]
fn a_pulse_that_returns_to_its_baseline_is_fitted_on_its_whole_record() {
    // N2.2, N2.3: a pulse is its own excitation. Its fit uses the whole record from the steady baseline, the return included,
    // and defines no fraction. A first-order plant with gain 1.5 and a 4 s time constant, pulsed by 2 for 3 s from 2 s, is
    // recovered as first order, with gain and time constant each within 2 %. The held-step measurement would read the final
    // tenth of this record, which has returned to the baseline, as the response.
    let result = identify_with(
        &first_order_pulse(2.0, 30),
        DEFAULT_MAXIMUM_NORMALIZED_ERROR,
        &pulse_options(),
    );
    assert_eq!(result.recommended.kind, LowOrderModelKind::FirstOrder);
    let gain = result.recommended.gain;
    assert!((gain - 1.5).abs() <= 0.03, "gain {gain}");
    let time_constant = result.recommended.time_constant_seconds.unwrap_or(0.0);
    assert!(
        (time_constant - 4.0).abs() <= 0.08,
        "time constant {time_constant}"
    );
    assert!(result.refusal.is_none(), "{:?}", result.refusal);
    assert!(result.integrator_refusal.is_none());
}

#[test]
fn a_short_pulse_gives_a_model_without_a_fraction_refusal() {
    // N2.3, N2.6, A10.3: a pulse of 0.5 s has a small response and returns to its baseline. It has no fraction, so no A10.3
    // refusal applies, and the noise check passes on this record, which has no noise.
    let result = identify_with(
        &first_order_pulse(2.0, 5),
        DEFAULT_MAXIMUM_NORMALIZED_ERROR,
        &pulse_options(),
    );
    assert_eq!(result.recommended.kind, LowOrderModelKind::FirstOrder);
    assert!(result.refusal.is_none(), "{:?}", result.refusal);
    assert!(result.integrator_refusal.is_none());
}

#[test]
fn a_held_step_of_the_same_plant_still_reaches_its_fraction() {
    // N2.3, A10.3: a held step is still judged by the fraction of its fitted final change, which the pulse rule leaves
    // alone. The same plant held from 2 s to the end of the record reaches 99.9 % of its change.
    let samples = first_order_pulse(2.0, 281);
    let result = identify(&samples);
    assert!(result.refusal.is_none(), "{:?}", result.refusal);
    let fraction = held_step_evidence(&result.recommended, &samples)
        .and_then(|evidence| evidence.fraction)
        .expect("the held step has a fraction");
    assert!((fraction - 0.9989).abs() <= 0.001, "fraction {fraction}");
}

#[test]
fn a_time_constant_beyond_the_grid_is_kept_as_a_search_boundary_finding_on_the_model() {
    // N2.2, N2.4: the time constants run to 10·H. A first-order response of 4500 s on a 200 s hold is best fitted at the
    // largest grid value, so its model keeps the search-boundary finding. A well-resolved response keeps none.
    let slow = identify(&slow_recording(300.0, 10.0));
    let fitted = slow
        .models
        .iter()
        .find(|model| model.kind == LowOrderModelKind::FirstOrder)
        .expect("the first order is fitted");
    assert!(fitted.on_search_boundary, "{fitted:?}");
    let resolved = identify(&fopdt_samples(2.0));
    let fitted = resolved
        .models
        .iter()
        .find(|model| model.kind == LowOrderModelKind::FirstOrder)
        .expect("the first order is fitted");
    assert!(!fitted.on_search_boundary, "{fitted:?}");
}
