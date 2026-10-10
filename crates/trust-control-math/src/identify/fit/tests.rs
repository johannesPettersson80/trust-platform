#![allow(
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)] // The fixtures compare their step counts and times the way the TypeScript fixtures build them.

use std::collections::VecDeque;

use super::*;

/// The parameters of a fixture response; a parameter the family does not use is zero.
#[derive(Debug, Clone, Copy)]
struct Parameters {
    gain: f64,
    tau1: f64,
    tau2: f64,
    delay: f64,
}

fn parameters(gain: f64, tau1: f64, tau2: f64, delay: f64) -> Parameters {
    Parameters {
        gain,
        tau1,
        tau2,
        delay,
    }
}

/// The step fixture of `numerics.test.ts` (`lowOrderFixture`): 301 samples every 0.1 s, the input stepping to
/// `amplitude` at sample 20, and the output of the family simulated sample by sample with the input delayed by
/// the whole number of samples nearest to the dead time.
fn fixture(kind: LowOrderModelKind, truth: Parameters, amplitude: f64) -> Vec<TimeDomainSample> {
    let dt = 0.1;
    let delay_steps = (truth.delay / dt).round() as usize;
    let mut delayed: VecDeque<f64> = VecDeque::from(vec![0.0; delay_steps]);
    let mut first = 0.0;
    let mut output = 0.0;
    let mut samples = Vec::with_capacity(301);
    for index in 0..=300_usize {
        let input = if index >= 20 { amplitude } else { 0.0 };
        delayed.push_back(input);
        let applied = delayed.pop_front().unwrap_or(input);
        match kind {
            LowOrderModelKind::StaticGain => output = truth.gain * applied,
            LowOrderModelKind::Integrating => output += dt * truth.gain * applied,
            LowOrderModelKind::FirstOrder | LowOrderModelKind::FirstOrderDeadTime => {
                output += (1.0 - (-dt / truth.tau1).exp()) * (truth.gain * applied - output);
            }
            LowOrderModelKind::SecondOrder | LowOrderModelKind::SecondOrderDeadTime => {
                first += (1.0 - (-dt / truth.tau1).exp()) * (truth.gain * applied - first);
                output += (1.0 - (-dt / truth.tau2).exp()) * (first - output);
            }
        }
        samples.push(TimeDomainSample {
            time_seconds: index as f64 * dt,
            input,
            output,
            quality: SampleQuality::Good,
        });
    }
    samples
}

fn options(allowed_kinds: &[LowOrderModelKind]) -> LowOrderIdentificationOptions {
    LowOrderIdentificationOptions {
        allowed_kinds: allowed_kinds.to_vec(),
        minimum_samples: 40,
        minimum_input_span: 0.1,
    }
}

/// The endpoint record of `numerics.test.ts`: 101 samples every 0.02 s, the input stepping to `amplitude` at sample 10
/// (0.2 s), and from there a first-order response with a 1 s time constant (or, when `integrating`, a unit-rate ramp).
fn step_samples(amplitude: f64, integrating: bool) -> Vec<TimeDomainSample> {
    (0..=100_usize)
        .map(|index| {
            let time = index as f64 * 0.02;
            let elapsed = (time - 0.2).max(0.0);
            let shape = if integrating {
                elapsed
            } else {
                1.0 - (-elapsed).exp()
            };
            TimeDomainSample {
                time_seconds: time,
                input: if index < 10 { 0.0 } else { amplitude },
                output: amplitude * shape,
                quality: SampleQuality::Good,
            }
        })
        .collect()
}

/// The options of the endpoint test: the first order and the integrating form.
fn endpoint_options() -> LowOrderIdentificationOptions {
    options(&[
        LowOrderModelKind::FirstOrder,
        LowOrderModelKind::Integrating,
    ])
}

/// The fits of an identification, or a panic that names the refusal.
fn fitted(result: Result<Vec<LowOrderModelFit>, LowOrderRejection>) -> Vec<LowOrderModelFit> {
    result.expect("the identification fits the families")
}

/// The fit of one family among the fits of an identification.
fn model_of(models: &[LowOrderModelFit], kind: LowOrderModelKind) -> &LowOrderModelFit {
    models
        .iter()
        .find(|model| model.kind == kind)
        .unwrap_or_else(|| panic!("{kind:?} is fitted"))
}

#[test]
fn every_declared_family_is_identified_on_independent_validation_data() {
    let cases = [
        (
            LowOrderModelKind::StaticGain,
            parameters(1.8, 0.0, 0.0, 0.0),
        ),
        (
            LowOrderModelKind::Integrating,
            parameters(0.35, 0.0, 0.0, 0.0),
        ),
        (
            LowOrderModelKind::FirstOrder,
            parameters(1.5, 3.0, 0.0, 0.0),
        ),
        (
            LowOrderModelKind::FirstOrderDeadTime,
            parameters(1.5, 4.0, 0.0, 0.5),
        ),
        (
            LowOrderModelKind::SecondOrder,
            parameters(1.2, 1.2, 4.5, 0.0),
        ),
        (
            LowOrderModelKind::SecondOrderDeadTime,
            parameters(1.2, 1.5, 5.0, 0.4),
        ),
    ];
    for (kind, truth) in cases {
        let estimation = fixture(kind, truth, 1.0);
        let validation = fixture(kind, truth, 0.65);
        let models = fitted(identify_low_order_models(
            &estimation,
            &validation,
            &options(&[kind]),
        ));
        let model = model_of(&models, kind);
        assert_eq!(model.estimation_residuals.len(), estimation.len());
        assert_eq!(model.validation_residuals.len(), validation.len());
        assert!(model.normalized_validation_error <= 0.03, "{kind:?}");
        assert!(model.final_normalized_validation_error <= 0.03, "{kind:?}");
        assert!(model.uncertainty.gain.minimum <= truth.gain, "{kind:?}");
        assert!(model.uncertainty.gain.maximum >= truth.gain, "{kind:?}");
        assert_eq!(
            model.uncertainty.method,
            UncertaintyMethod::GridResolutionAndValidationResidual
        );
        assert!(
            (model.gain - truth.gain).abs() <= 0.12 * truth.gain,
            "{kind:?}: gain {}",
            model.gain
        );
        match kind {
            LowOrderModelKind::FirstOrder | LowOrderModelKind::FirstOrderDeadTime => {
                let fitted = model.time_constant_seconds.expect("a time constant");
                assert!(
                    (fitted - truth.tau1).abs() <= 0.35_f64.max(0.22 * truth.tau1),
                    "{kind:?}: time constant {fitted}"
                );
            }
            LowOrderModelKind::SecondOrder | LowOrderModelKind::SecondOrderDeadTime => {
                let mut fitted = [
                    model.time_constant_seconds.expect("a time constant"),
                    model
                        .second_time_constant_seconds
                        .expect("a second time constant"),
                ];
                fitted.sort_by(f64::total_cmp);
                let mut expected = [truth.tau1, truth.tau2];
                expected.sort_by(f64::total_cmp);
                assert!((fitted[0] - expected[0]).abs() <= 0.5_f64.max(0.3 * expected[0]));
                assert!((fitted[1] - expected[1]).abs() <= 0.7_f64.max(0.25 * expected[1]));
            }
            LowOrderModelKind::StaticGain | LowOrderModelKind::Integrating => {}
        }
        if truth.delay > 0.0 {
            let dead_time = model.dead_time_seconds.expect("a dead time");
            assert!(
                (dead_time - truth.delay).abs() <= 0.25,
                "{kind:?}: dead time {dead_time}"
            );
        }
    }
}

#[test]
fn a_noise_free_first_order_dead_time_step_is_recovered_within_the_grid_resolution() {
    // Specification N2.2. The time constant 0.1·(280/0.1)^(20/44) ≈ 1.295 s lies between points of the 45-point grid from
    // the sample interval 0.1 s to 10·H = 280 s, whose ratio is 2800^(1/44). The refinement places ten points between the
    // two neighbouring grid values, a ratio of 2800^(2/(44·9)) per step, so the best refined point is within half a step:
    // |ln(τ̂/τ)| ≤ ln(2800)/396 ≈ 0.0200. The fitted value is 1.275 s (|ln| 0.016). The delay grid has steps of 0.168 s, and
    // the refinement steps the delay by the sample interval 0.1 s. The fixture delays its input by five samples, which
    // N1.2 places 0.4 s (not 0.5 s) before the output it drives, so the fitted delay is 0.4 s.
    let refined_half_step = (2800.0_f64).powf(1.0 / 396.0).ln();
    let true_time_constant = 0.1 * (28.0_f64 / 0.1).powf(20.0 / 44.0);
    let truth = parameters(1.5, true_time_constant, 0.0, 0.5);
    let kind = LowOrderModelKind::FirstOrderDeadTime;
    let models = fitted(identify_low_order_models(
        &fixture(kind, truth, 1.0),
        &fixture(kind, truth, 0.65),
        &options(&[kind]),
    ));
    let model = model_of(&models, kind);
    let fitted_time_constant = model.time_constant_seconds.expect("a time constant");
    assert!(
        (fitted_time_constant / true_time_constant).ln().abs() <= refined_half_step + 1e-9,
        "time constant {fitted_time_constant} is more than half a refined step from {true_time_constant}"
    );
    let fitted_dead_time = model.dead_time_seconds.expect("a dead time");
    assert!(
        (fitted_dead_time - 0.5).abs() <= 0.1 + 1e-9,
        "dead time {fitted_dead_time}"
    );
    assert!(
        (model.gain / 1.5 - 1.0).abs() <= 0.02,
        "gain {}",
        model.gain
    );
}

#[test]
fn every_allowed_family_is_returned_once_in_the_order_of_the_options() {
    // The library fits every family and leaves the choice to the caller (N3.3): both families come back, in the order
    // the options name them, whichever of them is the simpler.
    let truth = parameters(1.5, 3.0, 0.0, 0.0);
    let estimation = fixture(LowOrderModelKind::FirstOrder, truth, 1.0);
    let validation = fixture(LowOrderModelKind::FirstOrder, truth, 0.65);
    let models = fitted(identify_low_order_models(
        &estimation,
        &validation,
        &options(&[
            LowOrderModelKind::SecondOrder,
            LowOrderModelKind::FirstOrder,
            LowOrderModelKind::SecondOrder,
        ]),
    ));
    let kinds: Vec<LowOrderModelKind> = models.iter().map(|model| model.kind).collect();
    assert_eq!(
        kinds,
        [
            LowOrderModelKind::SecondOrder,
            LowOrderModelKind::FirstOrder
        ]
    );
    // The fit reports the sample interval of its estimation record: the fixture samples every 0.1 s.
    let first_order = model_of(&models, LowOrderModelKind::FirstOrder);
    assert!(
        (first_order.sample_interval_seconds - 0.1).abs() <= 1e-9,
        "sample interval {}",
        first_order.sample_interval_seconds
    );
}

#[test]
fn the_forms_rank_in_the_order_of_n2_1() {
    // Specification N2.1: static gain, first order, first order with delay, integrating with delay, second order, second
    // order with delay.
    let order = [
        LowOrderModelKind::StaticGain,
        LowOrderModelKind::FirstOrder,
        LowOrderModelKind::FirstOrderDeadTime,
        LowOrderModelKind::Integrating,
        LowOrderModelKind::SecondOrder,
        LowOrderModelKind::SecondOrderDeadTime,
    ];
    let ranks: Vec<u8> = order.into_iter().map(LowOrderModelKind::order).collect();
    assert_eq!(ranks, [1, 2, 3, 4, 5, 6]);
}

#[test]
fn an_integrating_form_with_a_delay_is_fitted_with_its_dead_time() {
    // Specification N2.1 item 4 (K′·exp(−L·s)/s) and N2.2. The record is the exact response of an integrator with rate
    // gain 0.35 per s and delay 0.5 s to a unit step at 2 s: the output is 0.35·(t − 2.5) from 2.5 s on. Each sample
    // integrates the delayed input over its interval (N1.2), so the rate and the delay are recovered exactly: the 0.5 s
    // delay is a grid point of the refinement (0.4, 0.5, 0.6 s between the neighbouring grid delays 0.336 and 0.672 s).
    let times: Vec<f64> = (0..301_u32).map(|index| f64::from(index) * 0.1).collect();
    let inputs: Vec<f64> = times
        .iter()
        .map(|&time| if time >= 2.0 - 1e-9 { 1.0 } else { 0.0 })
        .collect();
    let outputs: Vec<f64> = times
        .iter()
        .map(|&time| {
            if time >= 2.5 - 1e-9 {
                0.35 * (time - 2.5)
            } else {
                0.0
            }
        })
        .collect();
    let record =
        prepare_record(&times, &inputs, &outputs, 20).expect("the integrator record is prepared");
    let fit = fit_form(&record, LowOrderModelKind::Integrating).expect("the integrator is fitted");
    assert!(fit.shape.time_constant_seconds.is_none());
    let delay = fit.shape.dead_time_seconds.expect("a dead time");
    assert!((delay - 0.5).abs() <= 1e-6, "dead time {delay}");
    assert!((fit.gain - 0.35).abs() <= 1e-6, "rate gain {}", fit.gain);
}

#[test]
fn an_integrating_fit_keeps_its_full_response_error_and_reports_its_final_response_error() {
    // The integrator's mean error over the whole response passes, but its final response does not. The fit reports both
    // errors; rejecting the integrator on its final response is the choice's rule (N3.2, N3.3), not the fit's.
    let models = fitted(identify_low_order_models(
        &step_samples(1.0, false),
        &step_samples(0.7, false),
        &endpoint_options(),
    ));
    let integrating = model_of(&models, LowOrderModelKind::Integrating);
    assert!(
        integrating.normalized_validation_error < 0.15,
        "the full-response error passes: {}",
        integrating.normalized_validation_error
    );
    assert!(
        integrating.final_normalized_validation_error > 0.15,
        "the final-response error misses: {}",
        integrating.final_normalized_validation_error
    );
    assert!(model_of(&models, LowOrderModelKind::FirstOrder)
        .normalized_validation_error
        .is_finite());
}

#[test]
fn the_identification_refuses_bad_options_records_and_outputs_with_their_reason() {
    let kind = LowOrderModelKind::FirstOrder;
    let truth = parameters(1.5, 3.0, 0.0, 0.0);
    let estimation = fixture(kind, truth, 1.0);
    let validation = fixture(kind, truth, 0.65);

    let mut no_span = options(&[kind]);
    no_span.minimum_input_span = 0.0;
    assert_eq!(
        identify_low_order_models(&estimation, &validation, &no_span)
            .expect_err("a zero minimum span is refused"),
        LowOrderRejection::NonfiniteInput
    );
    let mut too_few_for_a_fit = options(&[kind]);
    too_few_for_a_fit.minimum_samples = 3;
    assert_eq!(
        identify_low_order_models(&estimation, &validation, &too_few_for_a_fit)
            .expect_err("three samples are refused"),
        LowOrderRejection::NonfiniteInput
    );

    assert_eq!(
        identify_low_order_models(&estimation[..30], &validation, &options(&[kind]))
            .expect_err("too few samples are refused"),
        LowOrderRejection::InsufficientData
    );

    let mut uncertain = estimation.clone();
    uncertain[50].quality = SampleQuality::Uncertain;
    assert_eq!(
        identify_low_order_models(&uncertain, &validation, &options(&[kind]))
            .expect_err("an uncertain sample is refused"),
        LowOrderRejection::NonfiniteInput,
        "only good samples are fitted"
    );

    let unexcited = fixture(kind, truth, 0.0);
    assert_eq!(
        identify_low_order_models(&unexcited, &validation, &options(&[kind]))
            .expect_err("an input that never changes is refused"),
        LowOrderRejection::PoorExcitation,
        "an input that never changes does not excite the plant"
    );

    let late_step: Vec<TimeDomainSample> = (0..40_usize)
        .map(|index| TimeDomainSample {
            time_seconds: index as f64 * 0.1,
            input: if index < 35 { 0.0 } else { 1.0 },
            output: 0.0,
            quality: SampleQuality::Good,
        })
        .collect();
    assert_eq!(
        identify_low_order_models(&late_step, &late_step, &options(&[kind]))
            .expect_err("a late step is refused"),
        LowOrderRejection::MissingStep,
        "a step within ten samples of the end leaves no response to fit"
    );

    // The input steps, but the output of the same record never changes: no error can be normalised by its span.
    let flat_output: Vec<TimeDomainSample> = estimation
        .iter()
        .map(|sample| TimeDomainSample {
            output: 0.0,
            ..*sample
        })
        .collect();
    assert_eq!(
        identify_low_order_models(&flat_output, &validation, &options(&[kind]))
            .expect_err("a flat output is refused"),
        LowOrderRejection::FlatOutput
    );
}

/// The uneven-sampling record of N2.2 and A4.3: the sample intervals alternate 0.1, 0.11, 0.09 and 0.1 s, a 100 ms task in a
/// 10 ms resource cycle, starting at zero.
fn uneven_times(count: usize) -> Vec<f64> {
    let pattern = [0.1, 0.11, 0.09, 0.1];
    let mut times = vec![0.0];
    for index in 0..count.saturating_sub(1) {
        let last = times[index];
        times.push(last + pattern[index % pattern.len()]);
    }
    times
}

/// A first-order-with-delay response (gain 1.5, time constant 4 s, delay 1 s) to a step of 2 at sample 20 on the given
/// times, simulated under the recorded input (N1.2): the record every sampling test fits.
fn model_record(times: &[f64]) -> PreparedRecord {
    let inputs: Vec<f64> = (0..times.len())
        .map(|index| if index >= 20 { 2.0 } else { 0.0 })
        .collect();
    let shape = LowOrderShape {
        time_constant_seconds: Some(4.0),
        second_time_constant_seconds: None,
        dead_time_seconds: Some(1.0),
    };
    let response = unit_response(LowOrderModelKind::FirstOrderDeadTime, shape, times, &inputs);
    let outputs: Vec<f64> = response.iter().map(|unit| 1.5 * unit).collect();
    prepare_record(times, &inputs, &outputs, 20).expect("the model record is prepared")
}

#[test]
fn a_slow_process_keeps_a_long_delay_without_a_fixed_cap() {
    // Specification N2.2: the delays run to 0.3·H with no fixed cap, so a slow process keeps its delay. The record is
    // K = 2, T = 600 s, L = 40 s, sampled every 1 s, stepped at 60 s and held to 1,860 s (H = 1,800 s): the delay grid has
    // steps of 10.8 s, the time constants run from 1 s to 18,000 s over 45 points (ratio 1.249), and the refinement steps the
    // delay by one sample. The true values are not refined points: 600 s lies between the refined points 598.98 s and
    // 600.45 s, and the delay goes from 42 s in the first pass to 41 s, where the later passes stop. So the fit gives
    // L = 41.0 s, T = 597.5 s and K = 1.9980 (the Python port of N2.2 gives the same values). The delay is not capped
    // at 5 s: it is one sample above 40 s, within the 2.5 s the assertion allows.
    let times: Vec<f64> = (0..=1860_u32).map(f64::from).collect();
    let inputs: Vec<f64> = times
        .iter()
        .map(|&time| if time >= 60.0 { 1.0 } else { 0.0 })
        .collect();
    let outputs: Vec<f64> = times
        .iter()
        .map(|&time| {
            if time >= 100.0 {
                2.0 * (1.0 - (-(time - 100.0) / 600.0).exp())
            } else {
                0.0
            }
        })
        .collect();
    let record =
        prepare_record(&times, &inputs, &outputs, 60).expect("the slow record is prepared");
    let fit = fit_form(&record, LowOrderModelKind::FirstOrderDeadTime).expect("the form is fitted");
    let delay = fit.shape.dead_time_seconds.expect("a dead time");
    let time_constant = fit.shape.time_constant_seconds.expect("a time constant");
    assert!((delay - 40.0).abs() <= 2.5, "dead time {delay}");
    assert!(
        (time_constant / 600.0 - 1.0).abs() <= 0.02,
        "time constant {time_constant}"
    );
    assert!((fit.gain / 2.0 - 1.0).abs() <= 0.01, "gain {}", fit.gain);
}

#[test]
fn a_pure_first_order_lag_has_no_delay_beyond_one_sample() {
    // Specification N2.2 and N3.3: a first-order lag T = 1,500 s sampled every 10 s, stepped at 100 s and held to 6,000 s.
    // The fitted delay of the first order with delay is zero or one sample (10 s), so the resolved-delay rule, which
    // needs more than two sample intervals (20 s), keeps the first order (numerical annex N3.3). The 2 % crossing of the
    // old single-run seed gave 20 to 30 s on this record.
    let times: Vec<f64> = (0..=600_u32).map(|index| f64::from(index) * 10.0).collect();
    let inputs: Vec<f64> = (0..times.len())
        .map(|index| if index >= 10 { 1.0 } else { 0.0 })
        .collect();
    let outputs: Vec<f64> = times
        .iter()
        .map(|&time| {
            if time >= 100.0 {
                1.0 - (-(time - 100.0) / 1500.0).exp()
            } else {
                0.0
            }
        })
        .collect();
    let record = prepare_record(&times, &inputs, &outputs, 10).expect("the lag record is prepared");
    let fit = fit_form(&record, LowOrderModelKind::FirstOrderDeadTime).expect("the form is fitted");
    let delay = fit.shape.dead_time_seconds.expect("a dead time");
    assert!(
        delay <= 10.0 + 1e-9,
        "dead time {delay} is within one sample of zero"
    );
    let time_constant = fit.shape.time_constant_seconds.expect("a time constant");
    assert!(
        (time_constant / 1500.0 - 1.0).abs() <= 0.02,
        "time constant {time_constant}"
    );
}

#[test]
fn uneven_sample_times_fit_the_same_form_within_one_refinement_step() {
    // Specification N1.2 and A4.3: the fit simulates on the recorded instants, so a 100 ms task whose intervals alternate
    // 0.1, 0.11, 0.09 and 0.1 s is fitted like the even record of the same model. On the even record the fit gives
    // K = 1.49999, T = 3.99974 s, L = 1.0 s; on the uneven record K = 1.4973, T = 3.8885 s, L = 1.1 s. The time constants
    // differ by less than one refined step: their ratio is 1.0286, and one refined step is 2800^(2/396) = 1.0409. The delays
    // differ by one sample (0.1 s).
    let even: Vec<f64> = (0..301_u32).map(|index| f64::from(index) * 0.1).collect();
    let even_fit = fit_form(&model_record(&even), LowOrderModelKind::FirstOrderDeadTime)
        .expect("the even record is fitted");
    let uneven = uneven_times(301);
    let uneven_fit = fit_form(
        &model_record(&uneven),
        LowOrderModelKind::FirstOrderDeadTime,
    )
    .expect("the uneven record is fitted");
    assert_eq!(even_fit.kind, uneven_fit.kind);
    let even_time_constant = even_fit
        .shape
        .time_constant_seconds
        .expect("a time constant");
    let uneven_time_constant = uneven_fit
        .shape
        .time_constant_seconds
        .expect("a time constant");
    let refined_step = (2800.0_f64).powf(2.0 / 396.0).ln();
    assert!(
        (uneven_time_constant / even_time_constant).ln().abs() <= refined_step + 1e-3,
        "time constants {even_time_constant} and {uneven_time_constant} differ by more than one refined step"
    );
    let even_delay = even_fit.shape.dead_time_seconds.expect("a dead time");
    let uneven_delay = uneven_fit.shape.dead_time_seconds.expect("a dead time");
    assert!(
        (uneven_delay - even_delay).abs() <= 0.1 + 1e-6,
        "delays {even_delay} and {uneven_delay} differ by more than one sample"
    );
    assert!((uneven_fit.gain / even_fit.gain - 1.0).abs() <= 0.01);
}

#[test]
fn the_same_record_gives_bit_identical_fits() {
    // Determinism: a fit of the same record is equal to another fit of it, value for value, for every form.
    let record = model_record(
        &(0..301_u32)
            .map(|index| f64::from(index) * 0.1)
            .collect::<Vec<f64>>(),
    );
    for kind in [
        LowOrderModelKind::StaticGain,
        LowOrderModelKind::Integrating,
        LowOrderModelKind::FirstOrder,
        LowOrderModelKind::FirstOrderDeadTime,
        LowOrderModelKind::SecondOrder,
        LowOrderModelKind::SecondOrderDeadTime,
    ] {
        let first = fit_form(&record, kind).expect("the form is fitted");
        let second = fit_form(&record, kind).expect("the form is fitted");
        assert_eq!(first, second, "{kind:?}");
    }
}

#[test]
fn a_time_constant_beyond_the_grid_is_a_search_boundary_finding() {
    // N2.2, N2.4: the time constants run to 10·H. A first-order response of 4500 s on a 200 s hold is slower than 10·H = 2000 s,
    // so the best grid point is the largest time constant, which lies on the edge of its grid, and the fit keeps that finding.
    let times: Vec<f64> = (0..=30_u32).map(|index| f64::from(index) * 10.0).collect();
    let inputs: Vec<f64> = (0..times.len())
        .map(|index| if index >= 10 { 10.0 } else { 0.0 })
        .collect();
    let outputs: Vec<f64> = times
        .iter()
        .map(|&time| {
            if time >= 100.0 {
                20.0 * (1.0 - (-(time - 100.0) / 4500.0).exp())
            } else {
                0.0
            }
        })
        .collect();
    let record =
        prepare_record(&times, &inputs, &outputs, 10).expect("the slow record is prepared");
    let fit = fit_form(&record, LowOrderModelKind::FirstOrder).expect("the form is fitted");
    assert!(
        fit.on_search_boundary,
        "the best time constant is the largest grid value"
    );
    assert!(
        fit.shape
            .time_constant_seconds
            .is_some_and(|tau| tau > 1900.0),
        "the fitted time constant sits at the grid's top: {:?}",
        fit.shape
    );
}

#[test]
fn a_delay_beyond_the_grid_is_a_search_boundary_finding() {
    // N2.2, N2.4: the delays run to 0.3·H. This response starts 9 s after its step, beyond 0.3·H = 5.4 s on an 18 s hold, so
    // the best grid delay is the longest one, which lies on the edge of its grid.
    let times: Vec<f64> = (0..=200_u32).map(|index| f64::from(index) * 0.1).collect();
    let inputs: Vec<f64> = (0..times.len())
        .map(|index| if index >= 20 { 2.0 } else { 0.0 })
        .collect();
    let outputs: Vec<f64> = times
        .iter()
        .map(|&time| {
            if time > 11.0 {
                3.0 * (1.0 - (-(time - 11.0) / 3.6).exp())
            } else {
                0.0
            }
        })
        .collect();
    let record =
        prepare_record(&times, &inputs, &outputs, 20).expect("the late record is prepared");
    let fit = fit_form(&record, LowOrderModelKind::FirstOrderDeadTime).expect("the form is fitted");
    assert!(
        fit.on_search_boundary,
        "the best delay is the longest grid delay"
    );
}

#[test]
fn a_zero_best_delay_of_a_dead_time_form_is_kept_and_is_no_search_boundary_finding() {
    // N2.2, N2.4: a best delay of exactly zero is one-sided, because a delay is never negative. It is kept with the result
    // and is not a search boundary, on every path that fits a form, so it refuses nothing. The record is a first-order
    // response with time constant 4 s and no delay (gain 1.5) to a step of 2 at sample 20, sampled every 0.1 s to 30 s:
    // H = 28 s, so the delays run from zero to 8.4 s in steps of 0.168 s. The grid's best point is delay 0.168 s and time
    // constant 3.6888 s, off both edges. The refinement (N2.2) then finds the delay zero, with a squared error of 2.2e-7
    // against 7.1e-2 at 0.1 s in its last pass, and the time constant 3.9997 s.
    let times: Vec<f64> = (0..301_u32).map(|index| f64::from(index) * 0.1).collect();
    let inputs: Vec<f64> = (0..times.len())
        .map(|index| if index >= 20 { 2.0 } else { 0.0 })
        .collect();
    let shape = LowOrderShape {
        time_constant_seconds: Some(4.0),
        second_time_constant_seconds: None,
        dead_time_seconds: Some(0.0),
    };
    let response = unit_response(
        LowOrderModelKind::FirstOrderDeadTime,
        shape,
        &times,
        &inputs,
    );
    let outputs: Vec<f64> = response.iter().map(|unit| 1.5 * unit).collect();
    let record =
        prepare_record(&times, &inputs, &outputs, 20).expect("the zero-delay record is prepared");
    let fit = fit_form(&record, LowOrderModelKind::FirstOrderDeadTime).expect("the form is fitted");
    assert_eq!(fit.shape.dead_time_seconds, Some(0.0), "{:?}", fit.shape);
    assert!(!fit.on_search_boundary, "{:?}", fit.shape);

    // The validated fit of the same record reaches the same conclusion (identify_low_order_models).
    let samples: Vec<TimeDomainSample> = (0..times.len())
        .map(|index| TimeDomainSample {
            time_seconds: times[index],
            input: inputs[index],
            output: outputs[index],
            quality: SampleQuality::Good,
        })
        .collect();
    let models = fitted(identify_low_order_models(
        &samples,
        &samples,
        &options(&[LowOrderModelKind::FirstOrderDeadTime]),
    ));
    assert!(
        !model_of(&models, LowOrderModelKind::FirstOrderDeadTime).on_search_boundary,
        "the validated fit has a zero delay, which is no search boundary"
    );

    // A form without dead time has no delay grid, so the zero delay is no finding for it.
    let first_order = fit_form(&record, LowOrderModelKind::FirstOrder).expect("the form is fitted");
    assert!(!first_order.on_search_boundary, "{:?}", first_order.shape);
}

#[test]
fn a_best_point_inside_its_grids_is_not_a_search_boundary_finding() {
    // N2.4 and V1 final fixes, item 9: a first-order-with-delay response whose delay and time constant lie inside their
    // grids has no boundary finding. The fit's delay is positive and below 0.3·H, the top of the delay grid, and its time
    // constant lies between the sample interval and 10·H, so neither edge rule applies.
    let times: Vec<f64> = (0..301_u32).map(|index| f64::from(index) * 0.1).collect();
    let record = model_record(&times);
    let fit = fit_form(&record, LowOrderModelKind::FirstOrderDeadTime).expect("the form is fitted");
    let delay = fit.shape.dead_time_seconds.expect("a dead time");
    let time_constant = fit.shape.time_constant_seconds.expect("a time constant");
    assert!(
        delay > 0.0 && delay < 0.3 * record.horizon_seconds,
        "dead time {delay}"
    );
    assert!(
        time_constant > record.sample_interval_seconds,
        "time constant {time_constant}"
    );
    assert!(
        time_constant < 10.0 * record.horizon_seconds,
        "time constant {time_constant}"
    );
    assert!(!fit.on_search_boundary, "{:?}", fit.shape);
}
