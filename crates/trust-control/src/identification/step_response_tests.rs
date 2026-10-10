#![allow(clippy::float_cmp)] // The `TypeScript` compares baselines, thresholds and grid values exactly on purpose.

use trust_control_math::identify::step_response::FirstOrderFit;
use trust_control_math::identify::SampleQuality;

use crate::experiment::number_text::js_to_fixed;
use crate::identification::data::{IdentificationSample, MAXIMUM_FIT_SAMPLES};
use crate::identification::refusal::Refusal;

use super::analysis::on_search_boundary;
use super::measurement::onset_direction;
use super::*;

// The assertions of `control-studio-step-response-numerics.test.ts`, of the step-response tests of
// `control-studio-jittered-sampling.test.ts` and of the step-response tests of `control-studio-long-process-fit.test.ts`.

fn grid(minimum: f64, maximum: f64, step: f64) -> SearchGrid {
    SearchGrid {
        minimum,
        maximum,
        step,
    }
}

/// The settings of the numerics fixture (`settings`).
fn numerics_settings() -> StepResponseSettings {
    StepResponseSettings {
        baseline_duration_seconds: 1.0,
        final_duration_seconds: 1.0,
        maximum_window_range: 0.1,
        maximum_window_slope_per_second: 0.01,
        measurement_bound: 0.05,
        onset_threshold: 0.15,
        onset_samples: 3,
        delay: grid(0.0, 0.2, 0.1),
        time_constant: grid(0.1, 0.4, 0.1),
        baseline_step: 0.01,
        gain_step: 0.001,
        timestamp_shift_seconds: 0.0,
    }
}

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

/// The numerics fixture (`samples`): 41 samples 0.1 s apart, an input step from 75 to 85 at 1 s held until 4 s, and a
/// first-order response of 2 with a 0.2 s time constant after a 0.1 s delay. `overrides` replaces outputs by index.
fn samples(overrides: &[(u32, f64)]) -> Vec<IdentificationSample> {
    (0..=40_u32)
        .map(|index| {
            let time_seconds = f64::from(index) * 0.1;
            let input = if (10..40).contains(&index) {
                85.0
            } else {
                75.0
            };
            let elapsed = (time_seconds - 1.0 - 0.1).max(0.0);
            let response = if index < 10 {
                35.0
            } else {
                35.0 + 2.0 * (1.0 - (-elapsed / 0.2).exp())
            };
            let output = overrides
                .iter()
                .find(|(at, _)| *at == index)
                .map_or(response, |&(_, value)| value);
            sample(time_seconds, input, output)
        })
        .collect()
}

/// The analysis with the annex default limit, as the `TypeScript` tests call it without one.
fn analyze(
    record: &[IdentificationSample],
    settings: &StepResponseSettings,
) -> Result<StepResponseAnalysis, Refusal> {
    analyze_step_response(record, settings, DEFAULT_MAXIMUM_NORMALIZED_ERROR)
}

/// Whether `text` holds `first` and, after it, `then`, ignoring case (the tests' `/first.*then/i`).
fn mentions(text: &str, first: &str, then: &str) -> bool {
    let text = text.to_lowercase();
    text.find(first)
        .is_some_and(|at| text[at + first.len()..].contains(then))
}

/// The same record with every output replaced by `output(sample)`.
fn with_outputs(
    record: Vec<IdentificationSample>,
    output: impl Fn(&IdentificationSample) -> f64,
) -> Vec<IdentificationSample> {
    record
        .into_iter()
        .map(|sample| IdentificationSample {
            output: output(&sample),
            ..sample
        })
        .collect()
}

#[test]
fn qualifies_a_complete_held_step_and_rejects_drift() {
    let settings = numerics_settings();
    assert!(qualify_step_response(&samples(&[]), &settings).is_ok());
    let drifting = samples(&[
        (0, 35.0),
        (1, 35.02),
        (2, 35.04),
        (3, 35.06),
        (4, 35.08),
        (5, 35.1),
        (6, 35.12),
        (7, 35.14),
        (8, 35.16),
        (9, 35.18),
    ]);
    let refusal =
        qualify_step_response(&drifting, &settings).expect_err("a drifting baseline is refused");
    let reason = refusal.0.to_lowercase();
    assert!(
        ["baseline", "range", "slope"]
            .iter()
            .any(|word| reason.contains(word)),
        "{refusal}"
    );
}

#[test]
fn detects_three_consecutive_samples_above_the_absolute_onset_threshold() {
    let record = samples(&[(11, 35.1), (12, 35.2), (13, 35.3)]);
    let result = analyze(&record, &numerics_settings()).expect("the held step is fitted");
    let onset = result
        .detectable_onset_seconds
        .expect("the onset is detected");
    assert!((onset - 0.2).abs() < 1e-9, "onset {onset}");
    assert_eq!(result.onset_threshold, 35.15);
}

#[test]
fn detects_sustained_onset_for_each_input_and_output_sign() {
    let settings = numerics_settings();
    for input_sign in [-1.0, 1.0] {
        for output_sign in [-1.0, 1.0] {
            let record: Vec<IdentificationSample> = samples(&[(11, 35.2), (12, 35.1)])
                .into_iter()
                .map(|sample| IdentificationSample {
                    input: 75.0 + input_sign * (sample.input - 75.0),
                    output: 35.0 + output_sign * (sample.output - 35.0),
                    ..sample
                })
                .collect();
            let measured =
                measure_step_response(&record, &settings).expect("the held step is measured");
            let analyzed = analyze(&record, &settings).expect("the held step is fitted");
            for (onset, threshold) in [
                (measured.detectable_onset_seconds, measured.onset_threshold),
                (analyzed.detectable_onset_seconds, analyzed.onset_threshold),
            ] {
                let onset = onset.expect("a sustained onset is detected");
                assert!(
                    (onset - 0.3).abs() < 1e-9,
                    "onset must start at the first sustained crossing after the isolated excursion \
                     (input sign {input_sign}, output sign {output_sign}): {onset}"
                );
                assert_eq!(threshold, 35.0 + output_sign * 0.15);
            }
        }
    }
}

#[test]
fn leaves_onset_unavailable_when_the_output_does_not_change() {
    let settings = numerics_settings();
    let record = with_outputs(samples(&[]), |_| 35.0);
    // N2.5: an unchanged output has no onset. The fit refuses it, and the measurement still reports the onset as
    // unavailable.
    let measured = measure_step_response(&record, &settings).expect("the held step is measured");
    assert_eq!(measured.detectable_onset_seconds, None);
    let refusal = analyze(&record, &settings).expect_err("an unchanged output is refused");
    assert!(
        refusal.0.contains("no measurable final change"),
        "{refusal}"
    );
}

/// Whether one combination of the brute-force oracle keeps the steady value within the bound of the final mean and
/// every fitted point within the bound of its closed-form first-order prediction.
fn oracle_consistent(
    result: &StepResponseAnalysis,
    bound: f64,
    gain: f64,
    baseline: f64,
    delay: f64,
    tau: f64,
) -> bool {
    let predicted_final = baseline + gain * result.input_change;
    let final_mean = result.final_window.mean;
    ((final_mean - bound - 1e-8)..=(final_mean + bound + 1e-8)).contains(&predicted_final)
        && result.points.iter().all(|point| {
            let elapsed = point.time_seconds - result.step_time_seconds - delay;
            let predicted = if elapsed < 0.0 {
                baseline
            } else {
                baseline + gain * result.input_change * (1.0 - (-elapsed / tau).exp())
            };
            (point.measured - predicted).abs() <= bound + 1e-8
        })
}

#[test]
fn matches_a_small_explicit_brute_force_consistency_oracle() {
    let settings = numerics_settings();
    let result = analyze(&samples(&[]), &settings).expect("the held step is fitted");
    let baseline = result.baseline.mean;
    let final_mean = result.final_window.mean;
    let bound = settings.measurement_bound;
    let gain_min = (final_mean - baseline - 2.0 * bound) / result.input_change;
    let gain_max = (final_mean - baseline + 2.0 * bound) / result.input_change;
    let mut count = 0_usize;
    let mut gain = gain_min;
    while gain <= gain_max + 1e-9 {
        let mut candidate_baseline = baseline - bound;
        while candidate_baseline <= baseline + bound + 1e-9 {
            let mut delay = settings.delay.minimum;
            while delay <= settings.delay.maximum + 1e-9 {
                let mut tau = settings.time_constant.minimum;
                while tau <= settings.time_constant.maximum + 1e-9 {
                    if oracle_consistent(&result, bound, gain, candidate_baseline, delay, tau) {
                        count += 1;
                    }
                    tau += settings.time_constant.step;
                }
                delay += settings.delay.step;
            }
            candidate_baseline += settings.baseline_step;
        }
        gain += settings.gain_step;
    }
    assert!(
        count > 0,
        "the independent oracle must exercise a nonempty consistent set"
    );
    assert_eq!(result.consistency.count, count);
}

#[test]
fn returns_an_empty_consistent_set_without_inventing_intervals() {
    let impossible = StepResponseSettings {
        measurement_bound: 1e-6,
        ..numerics_settings()
    };
    let result = analyze(&samples(&[]), &impossible).expect("the held step is fitted");
    assert_eq!(result.consistency.count, 0);
    assert_eq!(result.consistency.gain, None);
    assert!(result
        .warnings
        .iter()
        .any(|warning| warning.to_lowercase().contains("parameter combination")));
}

#[test]
fn rejects_zero_steps_and_preserves_source_samples() {
    let settings = numerics_settings();
    let source = samples(&[]);
    let before = source.clone();
    let zero_step: Vec<IdentificationSample> = source
        .iter()
        .map(|sample| IdentificationSample {
            input: 75.0,
            ..sample.clone()
        })
        .collect();
    let qualified = qualify_step_response(&zero_step, &settings)
        .expect_err("a record without a step is refused");
    assert!(
        mentions(&qualified.0, "nonzero input step", ""),
        "{qualified}"
    );
    let measured = measure_step_response(&zero_step, &settings)
        .expect_err("a record without a step is refused");
    assert!(
        mentions(&measured.0, "nonzero input step", ""),
        "{measured}"
    );
    let analyzed = analyze(&zero_step, &settings).expect_err("a record without a step is refused");
    assert!(
        mentions(&analyzed.0, "nonzero input step", ""),
        "{analyzed}"
    );
    assert_eq!(source, before);
}

#[test]
fn validates_settings_and_rejects_malformed_sample_quality() {
    assert!(valid_step_response_settings(&numerics_settings()));
    let reversed = StepResponseSettings {
        delay: grid(1.0, 0.0, 0.1),
        ..numerics_settings()
    };
    assert!(!valid_step_response_settings(&reversed));
    let mut bad = samples(&[]);
    bad[4].quality = SampleQuality::Bad;
    assert!(qualify_step_response(&bad, &numerics_settings()).is_err());
}

#[test]
fn names_the_field_and_what_to_enter_for_each_settings_problem() {
    let problem_of = |settings: &StepResponseSettings| {
        step_response_settings_problem(settings).map(|problem| (problem.field, problem.message))
    };
    let named = |field: &str, message: &str| Some((field.to_string(), message.to_string()));
    let cases = [
        (
            StepResponseSettings {
                measurement_bound: 0.0,
                ..numerics_settings()
            },
            named("measurementBound", "Enter a value greater than zero."),
        ),
        (
            StepResponseSettings {
                onset_threshold: -0.1,
                ..numerics_settings()
            },
            named("onsetThreshold", "Enter zero or a greater value."),
        ),
        (
            StepResponseSettings {
                gain_step: f64::NAN,
                ..numerics_settings()
            },
            named("gainStep", "Enter a finite number."),
        ),
        (
            StepResponseSettings {
                timestamp_shift_seconds: f64::INFINITY,
                ..numerics_settings()
            },
            named("timestampShiftSeconds", "Enter a finite time offset."),
        ),
        (
            StepResponseSettings {
                onset_samples: 101,
                ..numerics_settings()
            },
            named("onsetSamples", "Enter a whole number from 1 to 100."),
        ),
        (
            StepResponseSettings {
                time_constant: grid(0.0, 0.4, 0.1),
                ..numerics_settings()
            },
            named("timeConstant.minimum", "Enter a value greater than zero."),
        ),
        (
            StepResponseSettings {
                delay: grid(0.0, 0.25, 0.1),
                ..numerics_settings()
            },
            named(
                "delay.step",
                "Choose a grid step that divides the minimum-to-maximum span exactly.",
            ),
        ),
        // 1000 delay intervals times 101 time-constant intervals exceed the 100 000 combinations.
        (
            StepResponseSettings {
                delay: grid(0.0, 100.0, 0.1),
                time_constant: grid(0.1, 10.2, 0.1),
                ..numerics_settings()
            },
            named(
                "delay.step",
                "Use a coarser grid or narrower delay and time-constant bounds.",
            ),
        ),
        (numerics_settings(), None),
    ];
    for (settings, expected) in cases {
        assert_eq!(problem_of(&settings), expected);
    }
}

#[test]
fn requires_the_first_baseline_sample_even_when_all_remaining_timestamps_are_uniform() {
    let incomplete = samples(&[])[1..].to_vec();
    for timestamp_shift_seconds in [-0.1, 0.0, 0.1] {
        let settings = StepResponseSettings {
            timestamp_shift_seconds,
            ..numerics_settings()
        };
        let refusal = qualify_step_response(&incomplete, &settings)
            .expect_err("a missing boundary sample must not qualify");
        assert!(
            mentions(&refusal.0, "baseline window", "coverage"),
            "{refusal}"
        );
    }
    let between = StepResponseSettings {
        baseline_duration_seconds: 0.95,
        ..numerics_settings()
    };
    assert!(
        qualify_step_response(&samples(&[]), &between).is_ok(),
        "a requested boundary between sample instants does not imply a missing sample"
    );
}

#[test]
fn rejects_windows_too_short_to_contain_two_samples() {
    for settings in [
        StepResponseSettings {
            baseline_duration_seconds: 0.1,
            ..numerics_settings()
        },
        StepResponseSettings {
            final_duration_seconds: 0.1,
            ..numerics_settings()
        },
    ] {
        let refusal = qualify_step_response(&samples(&[]), &settings)
            .expect_err("a window with one sample is refused");
        assert!(mentions(&refusal.0, "window", "coverage"), "{refusal}");
    }
}

#[test]
fn requires_the_final_window_to_stay_inside_the_held_step() {
    let unchanged = with_outputs(samples(&[]), |_| 35.0);
    let settings = StepResponseSettings {
        final_duration_seconds: 3.1,
        ..numerics_settings()
    };
    let refusal = qualify_step_response(&unchanged, &settings)
        .expect_err("a steady output cannot authorize a pre-step final window");
    assert!(
        mentions(&refusal.0, "final window", "held step"),
        "{refusal}"
    );
}

#[test]
fn supports_signed_gain_lattices_and_keeps_a_zero_delay_without_a_search_boundary_finding() {
    let reversed = with_outputs(samples(&[]), |sample| 70.0 - sample.output);
    let fixed_delay = StepResponseSettings {
        delay: grid(0.1, 0.1, 0.1),
        ..numerics_settings()
    };
    let result = analyze(&reversed, &fixed_delay).expect("the falling step is fitted");
    assert!(result.consistency.gain.map_or(0.0, |gain| gain.minimum) < 0.0);

    let immediate = with_outputs(samples(&[]), |sample| {
        if (1.0..3.95).contains(&sample.time_seconds) {
            35.0 + 2.0 * (1.0 - (-(sample.time_seconds - 1.0) / 0.2).exp())
        } else {
            35.0
        }
    });
    let zero_delay_settings = StepResponseSettings {
        delay: grid(0.0, 0.0, 0.1),
        ..numerics_settings()
    };
    let zero_delay =
        analyze(&immediate, &zero_delay_settings).expect("the immediate step is fitted");
    assert_eq!(zero_delay.best.delay_seconds, 0.0);
    // N2.4: a best delay of zero is one-sided. It is kept with the result, but it is no search boundary, so the fit has no
    // boundary flag and no boundary warning. `control-studio-step-response-numerics.test.ts` line 174 expects
    // `boundaryLimited` false here too.
    assert!(
        !zero_delay.boundary_limited,
        "a zero delay is no search boundary"
    );
    assert!(
        zero_delay
            .warnings
            .iter()
            .all(|warning| !warning.contains("search boundary")),
        "{:?}",
        zero_delay.warnings
    );
}

#[test]
fn a_positive_delay_grid_end_is_a_search_boundary_but_a_zero_delay_is_not() {
    // N2.4: the delay at a positive end of its grid is a search-boundary finding. A zero delay is one-sided, because a delay
    // is never negative, so it is no finding even on a grid that starts at zero. An interior delay is no finding either.
    let fit = |delay_seconds: f64| FirstOrderFit {
        gain: 2.0,
        delay_seconds,
        time_constant_seconds: 0.2,
        squared_error: 0.0,
        response: Vec::new(),
    };
    let with_delay_grid = |minimum: f64, maximum: f64| StepResponseSettings {
        delay: grid(minimum, maximum, 0.1),
        ..numerics_settings()
    };
    assert!(
        !on_search_boundary(&fit(0.0), &with_delay_grid(0.0, 0.2)),
        "a zero delay on a grid that starts at zero is one-sided"
    );
    assert!(
        !on_search_boundary(&fit(0.0), &with_delay_grid(0.0, 0.0)),
        "a one-value grid at zero is one-sided too"
    );
    assert!(
        on_search_boundary(&fit(0.5), &with_delay_grid(0.5, 2.0)),
        "the positive minimum keeps its finding"
    );
    assert!(
        on_search_boundary(&fit(2.0), &with_delay_grid(0.0, 2.0)),
        "the positive maximum keeps its finding"
    );
    assert!(
        !on_search_boundary(&fit(1.0), &with_delay_grid(0.0, 2.0)),
        "an interior delay is no finding"
    );
}

#[test]
fn refuses_stored_settings_that_carry_a_field_of_the_parked_comparison() {
    // D8: the sensor-lag and model-adequacy fields belong to the parked comparison (stage V7). Stored settings that carry
    // one are refused; they are not read with the field dropped in silence.
    let stored = serde_json::to_value(numerics_settings()).expect("the settings encode");
    let decoded: StepResponseSettings =
        serde_json::from_value(stored.clone()).expect("the stored settings decode");
    assert_eq!(decoded, numerics_settings());
    for (field, value) in [
        (
            "sensorTimeConstant",
            serde_json::json!({ "minimum": 0.1, "maximum": 1.0, "step": 0.1 }),
        ),
        (
            "modelAdequacy",
            serde_json::json!({ "maximumAbsoluteResidual": 0.05, "maximumFinalError": 0.1 }),
        ),
    ] {
        let mut carrying = stored.clone();
        carrying[field] = value;
        let refusal = serde_json::from_value::<StepResponseSettings>(carrying)
            .expect_err("a settings field of the parked comparison is refused");
        assert!(refusal.to_string().contains(field), "{refusal}");
    }
    let grid_carrying =
        serde_json::json!({ "minimum": 0.0, "maximum": 1.0, "step": 0.1, "modelAdequacy": {} });
    assert!(
        serde_json::from_value::<SearchGrid>(grid_carrying).is_err(),
        "a search grid with an unknown field is refused"
    );
}

#[test]
fn does_not_let_timestamp_shift_sensitivity_relax_a_real_timing_gap() {
    let mut gapped = samples(&[]);
    gapped[15].time_seconds = gapped[14].time_seconds + 0.2;
    assert!(qualify_step_response(&gapped, &numerics_settings()).is_err());
}

#[test]
fn rejects_clipped_saturated_and_inconsistent_unit_samples() {
    let settings = numerics_settings();
    let mut clipped = samples(&[]);
    clipped[15].clipped = Some(true);
    assert!(qualify_step_response(&clipped, &settings).is_err());
    let mut saturated = samples(&[]);
    saturated[15].saturated = Some(true);
    assert!(qualify_step_response(&saturated, &settings).is_err());
    let units: Vec<IdentificationSample> = samples(&[])
        .into_iter()
        .enumerate()
        .map(|(index, sample)| IdentificationSample {
            input_unit: Some(if index < 20 { "%" } else { "percent" }.to_string()),
            ..sample
        })
        .collect();
    assert!(qualify_step_response(&units, &settings).is_err());
}

// `control-studio-jittered-sampling.test.ts`: a first-order plant of gain 0.2 and time constant 1 s.

const JITTER_BASELINE: f64 = 35.0;
const INPUT_LOW: f64 = 75.0;
const INPUT_HIGH: f64 = 85.0;
const JITTER_GAIN: f64 = 0.2;
const JITTER_TIME_CONSTANT: f64 = 1.0;

/// Execution times of a 100 ms task in a 10 ms resource: the intervals alternate between 110 and 100 ms
/// (`taskTimes`).
fn task_times(count: usize) -> Vec<f64> {
    let mut time = 0.0;
    let mut times = vec![time];
    for index in 1..count {
        time += if index % 2 == 1 { 0.11 } else { 0.1 };
        times.push(time);
    }
    times
}

/// A first-order response to a 10-unit input step, held from `step_index` up to `release_index`, sampled at `times`
/// (`stepRecord`).
fn step_record(
    times: &[f64],
    step_index: usize,
    release_index: usize,
) -> Vec<IdentificationSample> {
    times
        .iter()
        .enumerate()
        .map(|(index, &time_seconds)| {
            let rise =
                1.0 - (-(time_seconds - times[step_index]).max(0.0) / JITTER_TIME_CONSTANT).exp();
            let fall = 1.0
                - (-(time_seconds - times[release_index]).max(0.0) / JITTER_TIME_CONSTANT).exp();
            let input = if (step_index..release_index).contains(&index) {
                INPUT_HIGH
            } else {
                INPUT_LOW
            };
            let output = JITTER_BASELINE + JITTER_GAIN * (INPUT_HIGH - INPUT_LOW) * (rise - fall);
            sample(time_seconds, input, output)
        })
        .collect()
}

/// The settings of the jittered-sampling tests: the numerics settings with no delay and a fine time-constant grid.
fn jittered_settings() -> StepResponseSettings {
    StepResponseSettings {
        delay: grid(0.0, 0.0, 0.1),
        time_constant: grid(0.5, 1.5, 0.001),
        ..numerics_settings()
    }
}

#[test]
fn identifies_a_step_at_alternating_100_and_110_ms_as_the_same_model_as_an_even_105_ms_grid() {
    // Both recordings place the step at 1.05 s and its release at 21.0 s, so only the sample spacing differs.
    let settings = jittered_settings();
    let jittered = analyze(&step_record(&task_times(260), 10, 200), &settings)
        .expect("the jittered step is fitted");
    let even_times: Vec<f64> = (0..260_u32).map(|index| f64::from(index) * 0.105).collect();
    let even = analyze(&step_record(&even_times, 10, 200), &settings)
        .expect("the evenly sampled step is fitted");
    let (jittered_fit, even_fit) = (&jittered.best, &even.best);
    assert!(
        (jittered_fit.gain / even_fit.gain - 1.0).abs() <= 0.02,
        "gain {} against {}",
        jittered_fit.gain,
        even_fit.gain
    );
    assert!(
        (jittered_fit.time_constant_seconds / even_fit.time_constant_seconds - 1.0).abs() <= 0.02,
        "time constant {} against {}",
        jittered_fit.time_constant_seconds,
        even_fit.time_constant_seconds
    );
    assert!(
        (even_fit.time_constant_seconds - JITTER_TIME_CONSTANT).abs()
            <= 0.02 * JITTER_TIME_CONSTANT,
        "the even grid recovers the time constant: {}",
        even_fit.time_constant_seconds
    );
}

#[test]
fn refuses_a_0_42_s_gap_and_names_its_start_time() {
    // The 0.32 s shift after index 123 makes the interval from 12.3 s to 12.72 s 0.42 s long.
    let times: Vec<f64> = (0..140_u32)
        .map(|index| {
            let shift = if index >= 124 { 0.32 } else { 0.0 };
            js_to_fixed(f64::from(index) * 0.1 + shift, 10)
        })
        .collect();
    let gapped = step_record(&times, 10, 130);
    let settings = jittered_settings();
    let expected = "The recording has a gap of 0.42 s at 12.3 s.";
    let qualified = qualify_step_response(&gapped, &settings).expect_err("the gap is refused");
    assert!(qualified.0.contains(expected), "{qualified}");
    let measured = measure_step_response(&gapped, &settings).expect_err("the gap is refused");
    assert!(measured.0.contains(expected), "{measured}");
}

#[test]
fn accepts_a_20000_sample_recording_and_fits_it_on_at_most_5000_block_averaged_points() {
    let times: Vec<f64> = (0..20_000_u32)
        .map(|index| f64::from(index) * 0.01)
        .collect();
    let settings = StepResponseSettings {
        time_constant: grid(0.5, 1.5, 0.01),
        ..jittered_settings()
    };
    let result = analyze(&step_record(&times, 100, 15_000), &settings).expect("the fit completes");
    assert!(result.best.squared_error.is_finite(), "the fit completes");
    assert!(
        result.points.len() <= MAXIMUM_FIT_SAMPLES,
        "the fit sees {} points",
        result.points.len()
    );
    assert!(
        (result.best.gain / JITTER_GAIN - 1.0).abs() <= 0.01,
        "gain {}",
        result.best.gain
    );
    assert!(
        (result.best.time_constant_seconds - JITTER_TIME_CONSTANT).abs()
            <= 0.02 * JITTER_TIME_CONSTANT,
        "time constant {}",
        result.best.time_constant_seconds
    );
}

// `control-studio-long-process-fit.test.ts`: a first-order plant with a 60 s time constant, sampled every half second.

const LONG_TIME_CONSTANT: f64 = 60.0;

/// The unit-step response of a first-order lag, zero before its start (`rise`).
fn rise(elapsed: f64) -> f64 {
    if elapsed > 0.0 {
        1.0 - (-elapsed / LONG_TIME_CONSTANT).exp()
    } else {
        0.0
    }
}

/// A held step of `change` output units from 60 s for `hold_seconds`, sampled every half second. An alternating
/// deviation of `noise` gives the baseline a known first-difference RMS, the noise times the square root of two
/// (`heldStepRecord`).
fn held_step_record(change: f64, hold_seconds: f64, noise: f64) -> Vec<IdentificationSample> {
    let step_time = 60.0;
    let release_time = step_time + hold_seconds;
    let mut record = Vec::new();
    let mut index = 0_u32;
    while f64::from(index) * 0.5 <= release_time + 0.5 {
        let time_seconds = f64::from(index) * 0.5;
        let held = (step_time..release_time).contains(&time_seconds);
        let deviation = if index.is_multiple_of(2) {
            noise
        } else {
            -noise
        };
        record.push(IdentificationSample {
            time_seconds,
            input: if held { 85.0 } else { 75.0 },
            output: 35.0
                + change * (rise(time_seconds - step_time) - rise(time_seconds - release_time))
                + deviation,
            quality: SampleQuality::Good,
            input_unit: Some("%".to_string()),
            output_unit: Some("°C".to_string()),
            clipped: None,
            saturated: None,
        });
        index += 1;
    }
    record
}

/// The settings of the long-process tests (`stepSettings`).
fn long_settings() -> StepResponseSettings {
    StepResponseSettings {
        baseline_duration_seconds: 60.0,
        final_duration_seconds: 1.0,
        maximum_window_range: 0.5,
        maximum_window_slope_per_second: 0.01,
        measurement_bound: 0.05,
        onset_threshold: 0.15,
        onset_samples: 3,
        delay: grid(0.0, 0.0, 1.0),
        time_constant: grid(30.0, 90.0, 1.0),
        baseline_step: 0.01,
        gain_step: 0.001,
        timestamp_shift_seconds: 0.0,
    }
}

#[test]
fn accepts_a_held_step_that_has_reached_70_percent_of_its_final_change_and_reports_the_fraction() {
    // A 75 s hold of a 60 s time constant ends at about 71 % of the final change; the 5 % tail is averaged.
    let result = analyze(&held_step_record(20.0, 75.0, 0.0), &long_settings())
        .expect("the partial response is accepted");
    let fraction = result
        .final_change_fraction
        .expect("the held step reports its fraction");
    assert!((0.65..=0.75).contains(&fraction), "fraction {fraction}");
    assert!(
        (result.best.gain / 2.0 - 1.0).abs() <= 0.01,
        "gain {} against 2",
        result.best.gain
    );
}

#[test]
fn refuses_a_held_step_that_has_reached_only_40_percent_naming_the_fraction_and_a_suggested_hold() {
    // A 32 s hold ends at about 41 % of the final change. The suggested hold is the delay plus three time constants:
    // 3 min.
    let refusal = analyze(&held_step_record(20.0, 32.0, 0.0), &long_settings())
        .expect_err("the partial response is refused");
    let message = refusal.0;
    assert!(
        ["39", "40", "41"]
            .iter()
            .any(|percent| message
                .contains(&format!("reached {percent} % of its fitted final change"))),
        "{message}"
    );
    assert!(
        message.contains("hold the step for about 3 min"),
        "{message}"
    );
}

#[test]
fn refuses_a_response_buried_in_the_measured_noise_naming_both_values() {
    // Baseline noise of 0.05 gives a first-difference RMS of 0.0707; twenty times that is 1.414, above the 1.0
    // response.
    let refusal = analyze(&held_step_record(1.0, 75.0, 0.05), &long_settings())
        .expect_err("the response is buried in the noise");
    assert!(
        refusal
            .0
            .contains("is smaller than 20 times the measured noise")
            && refusal.0.contains("Permit a larger step in Limits"),
        "{refusal}"
    );
}

#[test]
fn accepts_a_response_just_above_the_noise_threshold() {
    // The same noise with a 1.6 response, above the 1.414 threshold, is fitted and accepted.
    let result = analyze(&held_step_record(1.6, 75.0, 0.05), &long_settings())
        .expect("the response is accepted");
    assert!(
        (result.best.gain / 0.16 - 1.0).abs() <= 0.05,
        "gain {} against 0.16",
        result.best.gain
    );
}

#[test]
fn identifies_a_negative_gain_from_a_falling_held_step() {
    // A 20 unit output fall for the 10 unit input rise: a plant gain of -2 with a 60 s time constant and no delay.
    let identified = analyze(&held_step_record(-20.0, 75.0, 0.0), &long_settings())
        .expect("the falling step is fitted");
    assert!(
        identified.best.gain < 0.0,
        "identified gain {}",
        identified.best.gain
    );
}

#[test]
fn the_onset_direction_follows_the_mean_of_the_last_tenth_of_the_hold() {
    // Specification N2.5: the direction runs from the baseline mean y0 to the mean yf of the last 10 % of the hold samples,
    // at least three. The whole hold averages above the baseline here, but its last three samples fall below it, so the
    // direction is falling. The TypeScript took it from the final window, or from the sign of the fitted change.
    let mut hold = vec![2.0; 17];
    hold.extend([0.5, 0.5, 0.5]);
    assert!(onset_direction(&hold, 1.0) < 0.0, "a falling hold");
    assert!(onset_direction(&[2.0; 20], 1.0) > 0.0, "a rising hold");
    assert!(
        onset_direction(&[1.0; 20], 1.0).abs() <= 1e-12,
        "a hold at the baseline has no direction"
    );
}
