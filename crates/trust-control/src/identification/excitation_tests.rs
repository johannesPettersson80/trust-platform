#![allow(clippy::float_cmp)] // The operating point of the relay record is the baseline itself, which the TypeScript asserts exactly.

use super::*;

// The assertions of `control-studio-excitation-identification.test.ts`, `control-studio-jittered-sampling.test.ts` and
// `control-studio-long-process-fit.test.ts` that call `identifyPlantFromExcitation`, with the fixtures those tests build.
// The tests of the first file that fit a recording through `fitRecordedModel` belong to the recorded-model fit and are not
// here. The last three tests are hand-computed refusals of this port.

/// A good sample with no units and no clipped or saturated flag.
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

/// Asserts that `actual` lies within `tolerance` of `expected` (the `near` helper of the `TypeScript` tests).
fn near(actual: Option<f64>, expected: f64, tolerance: f64, label: &str) {
    assert!(
        actual.is_some_and(|value| (value - expected).abs() <= tolerance),
        "{label}: expected {expected} ± {tolerance}, got {actual:?}"
    );
}

/// The fixtures' generator (`random`): a 32-bit linear congruential sequence, mapped to the interval from −1 to 1.
#[allow(clippy::cast_precision_loss)] // A seed below 2³² is exact as a float.
fn next_random(seed: &mut u64) -> f64 {
    *seed = (*seed * 1_664_525 + 1_013_904_223) % 4_294_967_296;
    (*seed as f64 / 4_294_967_296.0) * 2.0 - 1.0
}

/// The plant and the record of the feedback-relay fixture (the options of `relayRecord`). Shared with the recorded-model tests.
pub(crate) struct RelayOptions {
    pub(crate) gain: f64,
    pub(crate) tau: f64,
    pub(crate) delay: f64,
    pub(crate) u0: f64,
    pub(crate) y0: f64,
    pub(crate) d: f64,
    pub(crate) band: f64,
    pub(crate) noise: f64,
}

/// A feedback-relay test on a delayed first-order plant: a steady baseline, then switching at `y0 ± band`, on 0.1 s scans
/// for 60 s (`relayRecord`).
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)] // Scan indices and the delay in scans are small counts.
pub(crate) fn relay_record(options: &RelayOptions) -> Vec<IdentificationSample> {
    let dt = 0.1;
    let steps = (options.delay / dt).round() as usize;
    let decay = (-dt / options.tau).exp();
    let mut seed = 20_260_927;
    let mut x = 0.0;
    let mut high = true;
    let mut applied: Vec<f64> = Vec::new();
    let mut samples = Vec::new();
    for k in 0..=600_usize {
        let output = options.y0 + x + options.noise * next_random(&mut seed);
        let mut input = options.u0;
        if (60..599).contains(&k) {
            if output >= options.y0 + options.band {
                high = false;
            } else if output <= options.y0 - options.band {
                high = true;
            }
            let switch = if high { options.d } else { -options.d };
            input = options.u0 + switch;
        }
        applied.push(input);
        samples.push(sample(js_to_fixed(k as f64 * dt, 10), input, output));
        let delayed = if k >= steps {
            applied[k - steps]
        } else {
            options.u0
        };
        x = decay * x + (1.0 - decay) * options.gain * (delayed - options.u0);
    }
    samples
}

/// The plant and the record of the closed-loop fixture (the options of `noisySetpointStepRecord`). Shared with the
/// recorded-model tests.
pub(crate) struct NoisySetpointOptions {
    pub(crate) gain: f64,
    pub(crate) tau: f64,
    pub(crate) delay: f64,
    pub(crate) kp: f64,
    pub(crate) ki: f64,
    pub(crate) noise: f64,
    pub(crate) step_at: usize,
}

/// A PI loop in Auto on a delayed first-order plant whose measurement is noisy: the controller moves the plant input at
/// every scan, also before the setpoint step, on 0.1 s scans for 40 s (`noisySetpointStepRecord`).
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)] // Scan indices and the delay in scans are small counts.
pub(crate) fn noisy_setpoint_record(options: &NoisySetpointOptions) -> Vec<IdentificationSample> {
    let dt = 0.1;
    let steps = (options.delay / dt).round() as usize;
    let decay = (-dt / options.tau).exp();
    let mut seed = 20_261_008;
    let mut x = 0.0;
    let mut integral = 0.0;
    let mut applied: Vec<f64> = Vec::new();
    let mut samples = Vec::new();
    for k in 0..=400_usize {
        let output = 20.0 + x + options.noise * next_random(&mut seed);
        let setpoint = if k >= options.step_at { 25.0 } else { 20.0 };
        let error = setpoint - output;
        integral += options.ki * error * dt;
        let input = (10.0 + options.kp * error + integral).clamp(0.0, 100.0);
        applied.push(input);
        samples.push(sample(js_to_fixed(k as f64 * dt, 10), input, output));
        let delayed = if k >= steps { applied[k - steps] } else { 10.0 };
        x = decay * x + (1.0 - decay) * options.gain * (delayed - 10.0);
    }
    samples
}

/// The plant and the record of the closed-loop fixture without measurement noise (the options of `setpointStepRecord`).
/// Shared with the recorded-model tests.
pub(crate) struct SetpointStepOptions {
    pub(crate) gain: f64,
    pub(crate) tau: f64,
    pub(crate) delay: f64,
    pub(crate) kp: f64,
    pub(crate) ki: f64,
}

/// A PI loop in Auto on a delayed first-order plant with no noise: the setpoint steps from 20 to 25 at scan 20, the plant input
/// starts at zero, on 0.1 s scans for 40 s (`setpointStepRecord`).
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)] // Scan indices and the delay in scans are small counts.
pub(crate) fn setpoint_step_record(options: &SetpointStepOptions) -> Vec<IdentificationSample> {
    let dt = 0.1;
    let steps = (options.delay / dt).round() as usize;
    let decay = (-dt / options.tau).exp();
    let mut x = 0.0;
    let mut integral = 0.0;
    let mut applied: Vec<f64> = Vec::new();
    let mut samples = Vec::new();
    for k in 0..=400_usize {
        let output = 20.0 + x;
        let setpoint = if k >= 20 { 25.0 } else { 20.0 };
        let error = setpoint - output;
        integral += options.ki * error * dt;
        let input = (options.kp * error + integral).clamp(0.0, 100.0);
        applied.push(input);
        samples.push(sample(js_to_fixed(k as f64 * dt, 10), input, output));
        let delayed = if k >= steps { applied[k - steps] } else { 0.0 };
        x = decay * x + (1.0 - decay) * options.gain * delayed;
    }
    samples
}

/// The execution times of a 100 ms task in a 10 ms resource: the intervals alternate between 110 and 100 ms (`taskTimes`).
fn jittered_times(count: usize) -> Vec<f64> {
    let mut times = Vec::with_capacity(count);
    let mut time = 0.0;
    for index in 0..count {
        times.push(time);
        time += if (index + 1) % 2 == 1 { 0.11 } else { 0.1 };
    }
    times
}

/// The next bit of the 7-stage maximal-length shift register (`register` starts at `0x7f`), as ±0.5 (`prbsInput`).
fn prbs_bit(register: &mut u32) -> f64 {
    let bit = *register & 1;
    *register = (*register >> 1) | (((*register ^ (*register >> 1)) & 1) << 6);
    if bit == 1 {
        0.5
    } else {
        -0.5
    }
}

/// The first-order plant of the excitation fixtures: each input is held over the real interval to the next scan, so the
/// output follows the exact discretization of that interval, with the given gain and time constant.
fn simulate_first_order(
    times: &[f64],
    inputs: &[f64],
    gain: f64,
    time_constant: f64,
) -> Vec<IdentificationSample> {
    let mut output = 0.0;
    times
        .iter()
        .zip(inputs)
        .enumerate()
        .map(|(index, (&time_seconds, &input))| {
            let current = sample(time_seconds, input, output);
            let decay = match times.get(index + 1) {
                Some(next) => (-(next - time_seconds) / time_constant).exp(),
                None => 0.0,
            };
            output = decay * output + (1.0 - decay) * gain * input;
            current
        })
        .collect()
}

/// The jittered PRBS record of the first-order test: 400 scans at 100 and 110 ms, a zero baseline of ten scans, then one bit
/// held for twenty scans, which is about 2 s, so each bit is seen whole. The plant has gain 2 and time constant 1 s.
fn jittered_prbs_samples() -> Vec<IdentificationSample> {
    let times = jittered_times(400);
    let mut register = 0x7f;
    let bits: Vec<f64> = (0..20).map(|_| prbs_bit(&mut register)).collect();
    let inputs: Vec<f64> = (0..400_usize)
        .map(|index| {
            if index < 10 {
                0.0
            } else {
                bits[(index - 10) / 20]
            }
        })
        .collect();
    simulate_first_order(&times, &inputs, 2.0, 1.0)
}

/// The one-scan PRBS record of the long-process test: 400 scans at 100 and 110 ms, a zero baseline of ten scans, then one
/// bit per scan, driving a plant of gain 1.5 and time constant 2 s.
fn one_scan_prbs_samples() -> Vec<IdentificationSample> {
    let times = jittered_times(400);
    let mut register = 0x7f;
    let inputs: Vec<f64> = (0..400_usize)
        .map(|index| {
            if index < 10 {
                0.0
            } else {
                prbs_bit(&mut register)
            }
        })
        .collect();
    simulate_first_order(&times, &inputs, 1.5, 2.0)
}

#[test]
fn fits_a_delayed_first_order_model_to_a_relay_recording_from_its_baseline() {
    // control-studio-excitation-identification.test.ts: "fits a delayed first-order model to a relay recording from its baseline".
    let samples = relay_record(&RelayOptions {
        gain: 0.2,
        tau: 5.0,
        delay: 1.0,
        u0: 75.0,
        y0: 35.0,
        d: 5.0,
        band: 0.04,
        noise: 0.02,
    });
    let fit = identify_plant_from_excitation(&samples, 0.1, None, &IdentifyPlantOptions::default())
        .expect("the relay is fitted");
    assert_eq!(
        fit.recommended.kind,
        LowOrderModelKind::FirstOrderDeadTime,
        "The heater's one-second delay needs the delayed first-order model"
    );
    near(Some(fit.recommended.gain), 0.2, 0.01, "process gain K");
    near(
        fit.recommended.time_constant_seconds,
        5.0,
        0.5,
        "time constant τ",
    );
    near(fit.recommended.dead_time_seconds, 1.0, 0.15, "dead time L");
    assert!(
        fit.recommended.decisive_error() <= 0.1,
        "normalized error {}",
        fit.recommended.decisive_error()
    );
    assert_eq!(
        fit.input0, 75.0,
        "The operating point is the baseline before the first switch, not before the largest one"
    );
    near(Some(fit.output0), 35.0, 0.02, "baseline temperature");
    assert_eq!(
        fit.points.len(),
        samples.len(),
        "Every sample has a model prediction for the chart"
    );
}

#[test]
fn takes_the_operating_point_over_the_steady_baseline_window_a_loop_in_auto_records_before_its_setpoint_step(
) {
    // control-studio-excitation-identification.test.ts: "takes the operating point over the steady baseline window a loop
    // in Auto records before its setpoint step (N1.3, N2.2)".
    let samples = noisy_setpoint_record(&NoisySetpointOptions {
        gain: 1.5,
        tau: 4.0,
        delay: 0.5,
        kp: 1.0,
        ki: 0.3,
        noise: 0.02,
        step_at: 100,
    });
    // The controller moves the plant input with the noise from the first scan, so no sample is a first input change.
    let refusal =
        identify_plant_from_excitation(&samples, 0.1, None, &IdentifyPlantOptions::default())
            .expect_err("a moving input has no steady baseline");
    assert!(
        refusal
            .0
            .contains("steady baseline of at least three samples"),
        "{refusal}"
    );
    let fit =
        identify_plant_from_excitation(&samples, 0.1, Some(9.95), &IdentifyPlantOptions::default())
            .expect("the named baseline is fitted");
    let window: Vec<&IdentificationSample> = samples
        .iter()
        .filter(|sample| sample.time_seconds <= 9.95)
        .collect();
    let inputs: Vec<f64> = window.iter().map(|sample| sample.input).collect();
    let outputs: Vec<f64> = window.iter().map(|sample| sample.output).collect();
    near(
        Some(fit.input0),
        mean(&inputs),
        1e-9,
        "u0, the mean plant input over the window",
    );
    near(
        Some(fit.output0),
        mean(&outputs),
        1e-9,
        "y0, the mean measurement over the window",
    );
    near(Some(fit.recommended.gain), 1.5, 0.1, "process gain K");
    let lag_and_delay = fit.recommended.time_constant_seconds.unwrap_or(0.0)
        + fit.recommended.dead_time_seconds.unwrap_or(0.0);
    near(Some(lag_and_delay), 4.5, 0.4, "lag plus delay τ + L");
}

#[test]
fn fits_a_first_order_plant_from_jittered_prbs_excitation() {
    // control-studio-jittered-sampling.test.ts: "fits a first-order plant from jittered PRBS excitation".
    let samples = jittered_prbs_samples();
    let fit = identify_plant_from_excitation(&samples, 0.5, None, &IdentifyPlantOptions::default())
        .expect("the jittered PRBS record is fitted");
    let first_order = fit
        .models
        .iter()
        .find(|model| model.kind == LowOrderModelKind::FirstOrder)
        .expect("the first-order candidate is fitted");
    assert!(
        (first_order.gain / 2.0 - 1.0).abs() <= 0.1,
        "gain {}",
        first_order.gain
    );
}

#[test]
fn recovers_the_gain_of_a_prbs_excitation_with_one_scan_bits_at_alternating_100_and_110_ms_within_one_percent(
) {
    // control-studio-long-process-fit.test.ts: "recovers the gain of a PRBS excitation with one-scan bits at alternating
    // 100 and 110 ms within 1 %".
    let samples = one_scan_prbs_samples();
    let fit = identify_plant_from_excitation(&samples, 0.5, None, &IdentifyPlantOptions::default())
        .expect("the PRBS record is fitted");
    let first_order = fit
        .models
        .iter()
        .find(|model| model.kind == LowOrderModelKind::FirstOrder)
        .expect("the first-order candidate is fitted");
    assert!(
        (first_order.gain / 1.5 - 1.0).abs() <= 0.01,
        "gain {} against 1.5",
        first_order.gain
    );
}

#[test]
fn refuses_a_recording_of_fewer_than_twenty_samples() {
    // Hand-computed: nineteen samples are one fewer than the fit accepts. The input is flat as well, but the missing nonzero
    // step (N2.2, N2.6) is checked after the sample count, so the sample sentence stands.
    let samples: Vec<IdentificationSample> = (0..19_u32)
        .map(|index| sample(f64::from(index) * 0.1, 0.0, 0.0))
        .collect();
    let refusal =
        identify_plant_from_excitation(&samples, 0.1, None, &IdentifyPlantOptions::default())
            .expect_err("nineteen samples are refused");
    assert_eq!(
        refusal.0,
        "Identification requires at least 20 good, finite, time-ordered samples."
    );
}

#[test]
fn refuses_a_steady_baseline_of_fewer_than_three_samples_before_the_first_input_change() {
    // Hand-computed: the input changes at the third scan, so only two samples come before the first change.
    let samples: Vec<IdentificationSample> = (0..40_u32)
        .map(|index| {
            let input = if index < 2 { 0.0 } else { 1.0 };
            sample(f64::from(index) * 0.1, input, 0.0)
        })
        .collect();
    let refusal =
        identify_plant_from_excitation(&samples, 0.1, None, &IdentifyPlantOptions::default())
            .expect_err("two baseline samples are refused");
    assert_eq!(
        refusal.0,
        "A steady baseline of at least three samples before the first input change is required."
    );
}

#[test]
fn refuses_a_gap_and_names_its_start_and_length() {
    // Hand-computed: scans every 0.1 s, except that from the scan due at 1.2 s on they come 0.32 s late, so the interval
    // from 1.1 s to 1.52 s is 0.42 s long. The input is flat as well, but the gap is checked before the missing nonzero step
    // (N2.2, N2.6), so the gap sentence stands.
    let samples: Vec<IdentificationSample> = (0..30_u32)
        .map(|index| {
            let late = if index >= 12 { 0.32 } else { 0.0 };
            sample(f64::from(index) * 0.1 + late, 0.0, 0.0)
        })
        .collect();
    let refusal =
        identify_plant_from_excitation(&samples, 0.1, None, &IdentifyPlantOptions::default())
            .expect_err("the gap is refused");
    assert_eq!(refusal.0, "The recording has a gap of 0.42 s at 1.1 s.");
}

#[test]
fn a_record_whose_plant_input_never_changes_names_the_missing_nonzero_step() {
    // Specification N2.2 and N2.6: a fit with no input change (Δu = 0) is refused, and the refusal names the missing nonzero
    // step. Only the input is flat here: the output ramps on its own, and the baseline end makes no difference to the refusal.
    let samples: Vec<IdentificationSample> = (0..40_u32)
        .map(|index| {
            let time = f64::from(index) * 0.1;
            sample(time, 1.0, time)
        })
        .collect();
    for baseline_end in [None, Some(1.0)] {
        let refusal = identify_plant_from_excitation(
            &samples,
            0.1,
            baseline_end,
            &IdentifyPlantOptions::default(),
        )
        .expect_err("a flat input has no nonzero step");
        assert_eq!(refusal.0, MISSING_NONZERO_STEP);
    }
}

#[test]
fn the_excitation_forms_rank_in_the_order_of_n2_1() {
    // Specification N2.1: static gain, first order, first order with delay, integrating with delay. The rank is the one
    // rule of the library, shared with the plant fits.
    let ranks = [
        LowOrderModelKind::StaticGain.order(),
        LowOrderModelKind::FirstOrder.order(),
        LowOrderModelKind::FirstOrderDeadTime.order(),
        LowOrderModelKind::Integrating.order(),
    ];
    assert_eq!(ranks, [1, 2, 3, 4]);
}

#[test]
fn an_integrating_plant_is_identified_as_integrating_with_delay_on_the_excitation_path() {
    // Specification N2.1 item 4 and N2.2: the integrating form is fitted over the delay grid from zero. The input steps
    // from 0 to 1 at 1.0 s; the output is a rate gain of 0.5 per unit of input, delayed by 0.5 s. That delay lies on the
    // 0.1 s grid, so the integrating candidate recovers the delay and the rate gain.
    let samples: Vec<IdentificationSample> = (0..40_u32)
        .map(|index| {
            let input = if index < 10 { 0.0 } else { 1.0 };
            let output = if index < 15 {
                0.0
            } else {
                0.5 * f64::from(index - 15) * 0.1
            };
            sample(f64::from(index) * 0.1, input, output)
        })
        .collect();
    // A pure integrator cannot be told from a slow first order on a short hold (N2.3a), so the policy declares the
    // integrating process; the integrating form is then the simplest acceptable one and is recommended.
    let fit = identify_plant_from_excitation(&samples, 0.15, None, &declared_integrating())
        .expect("the integrator is fitted");
    assert_eq!(fit.recommended.kind, LowOrderModelKind::Integrating);
    let integrating = fit
        .models
        .iter()
        .find(|model| model.kind == LowOrderModelKind::Integrating)
        .expect("the integrating form is fitted");
    let dead_time = integrating.dead_time_seconds.expect("a dead time");
    assert!((dead_time - 0.5).abs() <= 0.05, "dead time {dead_time}");
    assert!(
        (integrating.gain - 0.5).abs() <= 0.01,
        "rate gain {}",
        integrating.gain
    );
}

/// The options of a loop whose identification policy declares an integrating process (N2.3a).
fn declared_integrating() -> IdentifyPlantOptions {
    IdentifyPlantOptions {
        declared_process: Some("integrating".to_string()),
        closed_loop: false,
        pulse: false,
    }
}

/// A first-order plant with gain 1.5, a 4 s time constant and a delay of `delay_samples` samples, stepped by 2 at sample 20
/// and sampled every 0.1 s for 30 s: a single held plant-input step (the example of the numerical annex).
fn held_first_order_record(delay_samples: usize) -> Vec<IdentificationSample> {
    let dt = 0.1;
    let mut pv = 0.0;
    let mut samples = Vec::with_capacity(301);
    for index in 0..=300_usize {
        let input = if index >= 20 { 2.0 } else { 0.0 };
        let applied = if index >= delay_samples && index - delay_samples >= 20 {
            2.0
        } else {
            0.0
        };
        pv += dt * (1.5 * applied - pv) / 4.0;
        let step = f64::from(u32::try_from(index).expect("a few samples"));
        samples.push(sample(step * dt, input, pv));
    }
    samples
}

#[test]
fn a_first_order_plant_with_a_delay_is_identified_as_first_order_with_delay_on_the_excitation_path()
{
    // The example of the numerical annex (K = 1.5, T = 4 s, L = 1 s, sampled every 0.1 s), stepped at 2 s. The single held
    // step is fitted by the excitation path, which recommends the first order with delay (N3.3): its error is far below the
    // first order's, and its delay is longer than two sample intervals. Nothing is refused (A10.3, N2.6).
    let samples = held_first_order_record(10);
    let fit =
        identify_plant_from_excitation(&samples, 0.15, None, &IdentifyPlantOptions::default())
            .expect("the held step is fitted");
    assert_eq!(
        fit.recommended.kind,
        LowOrderModelKind::FirstOrderDeadTime,
        "the first order with delay is recommended"
    );
    near(Some(fit.recommended.gain), 1.5, 0.05, "process gain K");
    near(
        fit.recommended.time_constant_seconds,
        4.0,
        0.5,
        "time constant τ",
    );
    near(fit.recommended.dead_time_seconds, 1.0, 0.2, "dead time L");
    assert_eq!(fit.refusal, None);
    // Specification N2.1 items 1 to 6: the excitation path offers every form, so the choice can take any of them.
    for kind in [
        LowOrderModelKind::StaticGain,
        LowOrderModelKind::Integrating,
        LowOrderModelKind::FirstOrder,
        LowOrderModelKind::FirstOrderDeadTime,
        LowOrderModelKind::SecondOrder,
        LowOrderModelKind::SecondOrderDeadTime,
    ] {
        assert!(
            fit.models.iter().any(|model| model.kind == kind),
            "{kind:?} is fitted"
        );
    }
}
