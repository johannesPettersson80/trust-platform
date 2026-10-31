//! Tests of the measured MIMO paths (`measuredPaths`; specification 35, C7; numerical annex N4.5). The lessons' recordings are the
//! coupled vessels that their `simulation.toml` describes, integrated here in 1 ms RK4 steps and sampled every 100 ms, over the
//! signals that the engine requests for the loop (`loop_inspection_bindings`). Lesson 69's windows must give its README matrix and
//! lesson 70's array; each refusal is a failure with its sentence.

use std::time::Duration;

use serde_json::json;

use super::*;
use crate::analysis::budget::ANALYSIS_BUDGET;
use crate::recording::fixtures::{run as fixture_run, sample};
use crate::signals::{loop_inspection_bindings, Quality};

/// Lesson 69's loop (the coupled vessels), as its lesson file declares it.
fn lesson_61() -> LoopDefinition {
    LoopDefinition::decode(include_str!(
        "../../../../examples/control/10-multivariable-control/61-mimo-paths/control-loop.json"
    ))
    .expect("lesson 61's loop file")
}

/// Lesson 70's loop, two copies of the coupled vessels; its first pair is lesson 69's.
fn lesson_62() -> LoopDefinition {
    LoopDefinition::decode(include_str!(
        "../../../../examples/control/10-multivariable-control/62-relative-gain-array/control-loop.json"
    ))
    .expect("lesson 62's loop file")
}

/// The coupled vessels of `simulation.toml`: `x = T − 20` with `ẋ = A·x + B·u`, `u` the heater commands in %.
const A: [[f64; 2]; 2] = [[-0.36, 0.16], [0.16, -0.36]];
const B: [[f64; 2]; 2] = [[0.04, 0.0], [0.0, 0.04]];

/// The heater commands of the project test (`Main.st`) at `time_ms`: heater 1 at 80 % from 2 s to 32 s, heater 2 at 80 % from 62 s.
fn project_commands(time_ms: u64) -> [f64; 2] {
    [
        if (2_000..32_000).contains(&time_ms) {
            80.0
        } else {
            75.0
        },
        if time_ms >= 62_000 { 80.0 } else { 75.0 },
    ]
}

/// The same test, except that heater 2 moves to 76 % from 27 s, inside heater 1's settled window.
fn heater_two_drifts(time_ms: u64) -> [f64; 2] {
    let mut commands = project_commands(time_ms);
    if time_ms >= 27_000 {
        commands[1] = 76.0;
    }
    commands
}

/// The same test, except that heater 2 never moves: its trial has no step.
fn heater_two_fixed(time_ms: u64) -> [f64; 2] {
    [project_commands(time_ms)[0], 75.0]
}

/// The rate of the state `x` under the commands `u`.
fn rate(x: [f64; 2], u: [f64; 2]) -> [f64; 2] {
    let mut rate = [0.0; 2];
    for (row, value) in rate.iter_mut().enumerate() {
        *value = A[row][0] * x[0] + A[row][1] * x[1] + B[row][0] * u[0] + B[row][1] * u[1];
    }
    rate
}

/// One RK4 step of `h` seconds under the commands `u`, held over the step.
fn rk4(x: [f64; 2], u: [f64; 2], h: f64) -> [f64; 2] {
    let shifted = |slope: [f64; 2], scale: f64| [x[0] + scale * slope[0], x[1] + scale * slope[1]];
    let k1 = rate(x, u);
    let k2 = rate(shifted(k1, h / 2.0), u);
    let k3 = rate(shifted(k2, h / 2.0), u);
    let k4 = rate(shifted(k3, h), u);
    [0, 1].map(|index| {
        x[index] + h / 6.0 * (k1[index] + 2.0 * k2[index] + 2.0 * k3[index] + k4[index])
    })
}

/// The temperatures and the commands at each sample: one sample every 100 ms, from rest at 35 °C to 92.1 s.
fn plant_samples(commands: fn(u64) -> [f64; 2]) -> Vec<([f64; 2], [f64; 2])> {
    let mut state = [15.0, 15.0];
    let mut samples = Vec::with_capacity(922);
    for time_ms in 0..=92_100_u64 {
        if time_ms % 100 == 0 {
            samples.push(([state[0] + 20.0, state[1] + 20.0], commands(time_ms)));
        }
        state = rk4(state, commands(time_ms), 0.001);
    }
    samples
}

/// The value of the loop signal `id` at `time_ms`, from the temperatures and the commands of the plant.
fn signal_value(id: &str, time_ms: u64, temperatures: [f64; 2], commands: [f64; 2]) -> f64 {
    match id {
        "processVariable" | "model-1" => temperatures[0],
        "vessel-2" | "model-2" => temperatures[1],
        "manipulatedVariable" => commands[0],
        "heater-2" => commands[1],
        "wall-flow" => 8.0 * (temperatures[0] - temperatures[1]),
        "experiment-authority" => {
            if time_ms >= 2_000 {
                1.0
            } else {
                0.0
            }
        }
        "vessel-3" | "vessel-4" => 35.0,
        "heater-3" | "heater-4" => 75.0,
        _ => 0.0,
    }
}

/// The run that the engine keeps for the project test of `loop_definition`: every signal of the loop, as the start requests them, in
/// one sample for each plant sample.
fn project_run(loop_definition: &LoopDefinition, samples: &[([f64; 2], [f64; 2])]) -> RecordedRun {
    let bindings = loop_inspection_bindings(loop_definition);
    let recorded = samples
        .iter()
        .enumerate()
        .map(|(index, (temperatures, commands))| {
            let scan = u64::try_from(index).expect("a sample index");
            let time_ms = scan * 100;
            let runtime_ns = i64::try_from(time_ms * 1_000_000).expect("a runtime time");
            let values: Vec<Option<f64>> = bindings
                .iter()
                .map(|binding| Some(signal_value(&binding.id, time_ms, *temperatures, *commands)))
                .collect();
            let quality = vec![Quality::Good; bindings.len()];
            sample(scan, Some(runtime_ns), &values, &quality)
        })
        .collect();
    fixture_run(bindings, recorded)
}

/// The index of the binding `id` in the run.
fn binding_index(recording: &RecordedRun, id: &str) -> usize {
    recording
        .bindings
        .iter()
        .position(|binding| binding.id == id)
        .expect("the binding is recorded")
}

/// Makes the binding `id` equal to the binding `source` in every sample.
fn copy_binding(recording: &mut RecordedRun, id: &str, source: &str) {
    let target = binding_index(recording, id);
    let from = binding_index(recording, source);
    for recorded in &mut recording.samples {
        recorded.values[target] = recorded.values[from];
    }
}

fn window((start_seconds, end_seconds): (f64, f64)) -> TimeWindow {
    TimeWindow {
        start_seconds: Some(start_seconds),
        end_seconds: Some(end_seconds),
    }
}

fn trial(input: &str, baseline: (f64, f64), settled: (f64, f64)) -> SettledTrial {
    SettledTrial {
        input_id: input.to_string(),
        baseline: window(baseline),
        settled: window(settled),
    }
}

/// Lesson 69's windows and drift tolerance (its README, step 7): heater 1's trial, then heater 2's.
fn lesson_inputs() -> AnalysisInputs {
    AnalysisInputs {
        settled_trials: Some(vec![
            trial("heater-1", (0.0, 1.9), (27.1, 31.9)),
            trial("heater-2", (57.1, 61.9), (87.1, 92.0)),
        ]),
        maximum_unexcited_input_drift: Some(0.001),
        ..AnalysisInputs::default()
    }
}

/// The measured paths of `recording`, with the lesson's budget that never runs out.
fn answer(
    recording: &RecordedRun,
    loop_definition: &LoopDefinition,
    inputs: &AnalysisInputs,
) -> Result<MeasuredPathsResult, Failure> {
    let elapsed = || Duration::ZERO;
    let stop = || false;
    let budget = Budget::new(ANALYSIS_BUDGET, &elapsed, &stop);
    measured_paths_answer(
        "rec-measured-paths",
        recording,
        loop_definition,
        inputs,
        &budget,
    )
}

/// The text of a refusal: its whole text, or its one sentence.
fn text_of(failure: &Failure) -> &str {
    failure.detail.as_deref().unwrap_or(&failure.message)
}

fn assert_within(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} is not within {tolerance} of {expected}"
    );
}

fn pair_names(result: &MeasuredPathsResult) -> Vec<(&str, &str)> {
    result
        .pairing
        .pairs
        .iter()
        .map(|pair| (pair.output_id.as_str(), pair.input_id.as_str()))
        .collect()
}

#[test]
fn lesson_61_windows_give_the_readme_matrix_within_its_tolerance() {
    let loop_definition = lesson_61();
    let recording = project_run(&loop_definition, &plant_samples(project_commands));
    let result = answer(&recording, &loop_definition, &lesson_inputs())
        .expect("lesson 69's windows qualify");
    // The README's measured matrix [0.1380 0.0607; 0.0611 0.1376] agrees with the heat balance within 0.0009 °C per point.
    let readme = [[0.1380, 0.0607], [0.0611, 0.1376]];
    let closed_form = [[9.0 / 65.0, 4.0 / 65.0], [4.0 / 65.0, 9.0 / 65.0]];
    for (row, values) in readme.iter().enumerate() {
        for (column, expected) in values.iter().enumerate() {
            assert_within(result.gain_matrix[row][column], *expected, 0.0009);
            assert_within(
                result.gain_matrix[row][column],
                closed_form[row][column],
                0.0009,
            );
        }
    }
    // 670 samples of the two windows, from 2 independent input steps (the README's evidence).
    assert_eq!(result.sample_count, 670);
    assert_eq!(result.trials.len(), 2);
    assert_eq!(result.trials[0].input_id, "heater-1");
    assert_eq!(result.trials[1].input_id, "heater-2");
    assert_eq!(result.inputs[1].symbol, "P1.Heater2Pct");
    assert_eq!(result.outputs[1].symbol, "P1.Vessel2C");
    assert_eq!(result.outputs[1].unit, "degC");
}

#[test]
fn lesson_61_pairs_each_heater_with_its_own_vessel() {
    let loop_definition = lesson_61();
    let recording = project_run(&loop_definition, &plant_samples(project_commands));
    let result = answer(&recording, &loop_definition, &lesson_inputs())
        .expect("lesson 69's windows qualify");
    // The array is 1.2426 on the diagonal and −0.2426 off it (lesson 70's README), so the crossed pairing is refused.
    assert_within(result.relative_gain_array[0][0], 1.2426, 0.0001);
    assert_within(result.relative_gain_array[1][1], 1.2426, 0.0001);
    assert_within(result.relative_gain_array[0][1], -0.2426, 0.0001);
    assert_within(result.relative_gain_array[1][0], -0.2426, 0.0001);
    assert_eq!(result.pairing.status, PairingStatus::Named);
    assert_eq!(
        pair_names(&result),
        [("vessel-1", "heater-1"), ("vessel-2", "heater-2")]
    );
}

#[test]
fn lesson_62_first_pair_gives_the_readme_relative_gain_array() {
    let loop_definition = lesson_62();
    let recording = project_run(&loop_definition, &plant_samples(project_commands));
    let result = answer(&recording, &loop_definition, &lesson_inputs())
        .expect("lesson 69's windows on lesson 70's first pair qualify");
    assert_within(result.relative_gain_array[0][0], 1.2426, 0.0001);
    assert_within(result.relative_gain_array[1][1], 1.2426, 0.0001);
    assert_within(result.relative_gain_array[0][1], -0.2426, 0.0001);
    assert_within(result.relative_gain_array[1][0], -0.2426, 0.0001);
    assert_eq!(result.pairing.status, PairingStatus::Named);
    assert_eq!(
        pair_names(&result),
        [("vessel-1", "heater-1"), ("vessel-2", "heater-2")]
    );
}

#[test]
fn the_recording_keeps_every_channel_input_and_output_of_the_loop() {
    // The engine records every declared signal of the loop, so each channel's input and output is in the kept run (C7).
    for loop_definition in [lesson_61(), lesson_62()] {
        let recording = project_run(&loop_definition, &plant_samples(project_commands));
        let channels = loop_definition
            .channels
            .as_ref()
            .expect("the lessons declare channels");
        for channel in &channels.inputs {
            let signal = channel
                .applied_signal_id
                .as_deref()
                .unwrap_or(&channel.signal_id);
            assert!(recording
                .bindings
                .iter()
                .any(|binding| binding.id == signal));
        }
        for channel in &channels.outputs {
            assert!(recording
                .bindings
                .iter()
                .any(|binding| binding.id == channel.signal_id));
        }
    }
}

#[test]
fn a_loop_without_channels_is_refused_as_not_square() {
    let mut loop_definition = lesson_61();
    loop_definition.channels = None;
    let recording = project_run(&lesson_61(), &plant_samples(project_commands));
    let failure = answer(&recording, &loop_definition, &lesson_inputs())
        .expect_err("a loop without channels has no multichannel analysis");
    assert_eq!(text_of(&failure), SQUARE);
}

#[test]
fn more_than_eight_pairs_are_refused_before_the_windows_are_read() {
    let mut loop_definition = lesson_61();
    let channels = loop_definition
        .channels
        .as_mut()
        .expect("the lesson declares channels");
    let input = channels.inputs[0].clone();
    channels.inputs = (0..9)
        .map(|index| {
            let mut channel = input.clone();
            channel.model_channel_id = format!("heater-{index}");
            channel
        })
        .collect();
    let output = channels.outputs[0].clone();
    channels.outputs = (0..9)
        .map(|index| {
            let mut channel = output.clone();
            channel.model_channel_id = format!("vessel-{index}");
            channel
        })
        .collect();
    let recording = project_run(&lesson_61(), &plant_samples(project_commands));
    let failure = answer(&recording, &loop_definition, &AnalysisInputs::default())
        .expect_err("nine pairs are more than the analysis compares");
    assert_eq!(text_of(&failure), PAIR_LIMIT);
}

#[test]
fn each_input_needs_exactly_one_settled_trial() {
    let loop_definition = lesson_61();
    let recording = project_run(&loop_definition, &plant_samples(project_commands));
    let heater_one = trial("heater-1", (0.0, 1.9), (27.1, 31.9));
    let unknown = trial("heater-9", (57.1, 61.9), (87.1, 92.0));
    let requests = [
        vec![heater_one.clone()],
        vec![heater_one.clone(), heater_one.clone()],
        vec![heater_one, unknown],
    ];
    for trials in requests {
        let inputs = AnalysisInputs {
            settled_trials: Some(trials),
            ..AnalysisInputs::default()
        };
        let failure = answer(&recording, &loop_definition, &inputs)
            .expect_err("one settled trial per input is required");
        assert_eq!(text_of(&failure), ONE_TRIAL);
    }
}

#[test]
fn a_recording_without_samples_or_with_untimed_samples_is_refused() {
    let loop_definition = lesson_61();
    let empty = project_run(&loop_definition, &[]);
    let failure = answer(&empty, &loop_definition, &lesson_inputs())
        .expect_err("the recording has no samples");
    assert_eq!(text_of(&failure), NO_SAMPLES);
    let mut untimed = project_run(&loop_definition, &plant_samples(project_commands));
    for recorded in &mut untimed.samples {
        recorded.runtime_time_ns = None;
    }
    let failure = answer(&untimed, &loop_definition, &lesson_inputs())
        .expect_err("no sample has a time, so none is in a window");
    assert_eq!(text_of(&failure), NO_SAMPLES);
}

#[test]
fn a_window_outside_the_recording_or_with_a_bad_sample_is_refused() {
    let loop_definition = lesson_61();
    let recording = project_run(&loop_definition, &plant_samples(project_commands));
    let mut beyond = lesson_inputs();
    if let Some(trials) = beyond.settled_trials.as_mut() {
        trials[1].settled.end_seconds = Some(93.0);
    }
    let failure = answer(&recording, &loop_definition, &beyond)
        .expect_err("the settled window runs past the last sample");
    assert_eq!(text_of(&failure), QUALITY);
    let mut bad = project_run(&loop_definition, &plant_samples(project_commands));
    let process = binding_index(&bad, "processVariable");
    bad.samples[280].values[process] = None;
    let failure = answer(&bad, &loop_definition, &lesson_inputs())
        .expect_err("a sample of the settled window has no value");
    assert_eq!(text_of(&failure), QUALITY);
}

#[test]
fn a_baseline_across_the_input_step_is_refused() {
    let loop_definition = lesson_61();
    let recording = project_run(&loop_definition, &plant_samples(project_commands));
    let mut across = lesson_inputs();
    if let Some(trials) = across.settled_trials.as_mut() {
        trials[0].baseline = window((1.9, 2.5));
    }
    let failure = answer(&recording, &loop_definition, &across)
        .expect_err("the baseline holds the step at 2 s");
    assert_eq!(text_of(&failure), STRADDLE);
}

#[test]
fn an_input_that_was_not_stepped_and_moved_is_refused_with_the_drift_tolerance() {
    let loop_definition = lesson_61();
    let recording = project_run(&loop_definition, &plant_samples(heater_two_drifts));
    let failure = answer(&recording, &loop_definition, &lesson_inputs())
        .expect_err("heater 2 moved during heater 1's trial");
    assert_eq!(text_of(&failure), DRIFT);
}

#[test]
fn trials_that_do_not_excite_every_input_have_no_static_gain() {
    let loop_definition = lesson_61();
    let recording = project_run(&loop_definition, &plant_samples(heater_two_fixed));
    let failure = answer(&recording, &loop_definition, &lesson_inputs())
        .expect_err("heater 2 never steps, so the inputs are not excited independently");
    assert_eq!(text_of(&failure), POOR_EXCITATION);
}

#[test]
fn outputs_that_move_together_have_no_relative_gain_array() {
    let loop_definition = lesson_61();
    let mut recording = project_run(&loop_definition, &plant_samples(project_commands));
    copy_binding(&mut recording, "vessel-2", "processVariable");
    let failure = answer(&recording, &loop_definition, &lesson_inputs())
        .expect_err("identical outputs give a singular gain matrix");
    assert_eq!(text_of(&failure), SINGULAR);
}

#[test]
fn a_pairing_is_refused_when_no_pairing_is_positive_on_its_diagonal() {
    let relative: DenseMatrix = vec![
        vec![1.2, -0.1, -0.1],
        vec![1.2, -0.1, -0.1],
        vec![-1.4, 1.2, 1.2],
    ];
    let channels = |prefix: &str| -> Vec<PathChannel> {
        (0..3)
            .map(|index| PathChannel {
                id: format!("{prefix}-{index}"),
                symbol: format!("P1.Signal{index}"),
                unit: "1".to_string(),
            })
            .collect()
    };
    let pairing = pairing_of(&relative, &channels("heater"), &channels("vessel"));
    assert_eq!(pairing.status, PairingStatus::Refused);
    assert_eq!(pairing.pairs, [] as [PathPair; 0]);
    assert_eq!(pairing.reason.as_deref(), Some(NOT_PAIRED));
}

#[test]
fn the_window_and_drift_checks_run_before_any_calculation() {
    let mut backward = lesson_inputs();
    if let Some(trials) = backward.settled_trials.as_mut() {
        trials[0].baseline = window((1.9, 0.0));
    }
    let failure = checked_request(&backward).expect_err("the baseline runs backward");
    assert_eq!(text_of(&failure), WINDOWS);
    let negative = AnalysisInputs {
        maximum_unexcited_input_drift: Some(-0.001),
        ..AnalysisInputs::default()
    };
    let failure = checked_request(&negative).expect_err("the drift tolerance is negative");
    assert_eq!(text_of(&failure), DRIFT_TOLERANCE);
    assert!(checked_request(&lesson_inputs()).is_ok());
}

#[test]
fn a_trial_that_leaves_a_window_end_out_is_refused_with_its_field() {
    let mut blank = lesson_inputs();
    if let Some(trials) = blank.settled_trials.as_mut() {
        trials[1].settled.end_seconds = None;
    }
    let failure = checked_request(&blank).expect_err("heater 2's settled end is blank");
    assert_eq!(
        text_of(&failure),
        "Enter the settled end of input heater-2."
    );
    assert_eq!(failure.field.as_deref(), Some("heater-2:settled-end"));
}

#[test]
fn the_refusal_names_each_blank_window_end_and_its_first_field() {
    let mut blank = lesson_inputs();
    if let Some(trials) = blank.settled_trials.as_mut() {
        trials[0].baseline.end_seconds = None;
        trials[0].settled.start_seconds = None;
    }
    let failure = checked_request(&blank).expect_err("two window ends of heater 1 are blank");
    assert_eq!(
        text_of(&failure),
        "Enter the baseline end and settled start of input heater-1."
    );
    assert_eq!(failure.field.as_deref(), Some("heater-1:baseline-end"));
}

#[test]
fn a_request_with_a_blank_window_end_decodes_it_as_absent() {
    let inputs: AnalysisInputs = crate::json::decode_value(json!({
        "settledTrials": [{
            "inputId": "heater-1",
            "baseline": { "startSeconds": 0.0 },
            "settled": { "startSeconds": 27.1, "endSeconds": 31.9 }
        }]
    }))
    .expect("a request with a blank window end decodes");
    let trials = inputs.settled_trials.as_deref().expect("one trial is sent");
    assert_eq!(trials[0].baseline.start_seconds, Some(0.0));
    assert_eq!(trials[0].baseline.end_seconds, None);
    assert_eq!(trials[0].settled.end_seconds, Some(31.9));
    let failure = checked_request(&inputs).expect_err("the baseline end is blank");
    assert_eq!(failure.field.as_deref(), Some("heater-1:baseline-end"));
}

#[test]
fn a_window_end_typed_as_text_reads_as_its_number_and_a_blank_one_is_absent() {
    let typed: TimeWindow = crate::json::decode_value(json!({
        "startSeconds": " 1.5 ",
        "endSeconds": ""
    }))
    .expect("a window whose ends are typed decodes");
    assert_eq!(typed.start_seconds, Some(1.5));
    assert_eq!(typed.end_seconds, None);
    let exponent: TimeWindow = crate::json::decode_value(json!({ "endSeconds": "2.5e1" }))
        .expect("an exponent the page accepted before still reads");
    assert_eq!(exponent.end_seconds, Some(25.0));
}

#[test]
fn a_window_end_that_no_number_reads_is_refused_under_its_field() {
    let typed: TimeWindow = crate::json::decode_value(json!({
        "startSeconds": "0,5",
        "endSeconds": "inf"
    }))
    .expect("a window whose ends are text decodes; the checks refuse them");
    assert!(typed.start_seconds.is_some_and(f64::is_nan));
    assert!(typed.end_seconds.is_some_and(f64::is_nan));
    let mut unreadable = lesson_inputs();
    if let Some(trials) = unreadable.settled_trials.as_mut() {
        trials[0].settled.end_seconds = Some(f64::NAN);
    }
    let failure =
        checked_request(&unreadable).expect_err("the settled end is text no number reads");
    assert_eq!(
        text_of(&failure),
        "The settled end of input heater-1 is not a number of seconds."
    );
    assert_eq!(failure.field.as_deref(), Some("heater-1:settled-end"));
}

#[test]
fn a_drift_tolerance_typed_as_text_that_no_number_reads_is_refused() {
    let inputs: AnalysisInputs = crate::json::decode_value(json!({
        "maximumUnexcitedInputDrift": "small"
    }))
    .expect("a drift tolerance sent as text decodes; the checks refuse it");
    let failure = checked_request(&inputs).expect_err("the tolerance is no number");
    assert_eq!(text_of(&failure), DRIFT_TOLERANCE);
    let blank: AnalysisInputs = crate::json::decode_value(json!({
        "maximumUnexcitedInputDrift": "  "
    }))
    .expect("a blank tolerance decodes");
    assert_eq!(blank.maximum_unexcited_input_drift, None);
}

#[test]
fn a_complete_trial_is_accepted_and_reported_with_its_four_ends() {
    let inputs = lesson_inputs();
    assert!(checked_request(&inputs).is_ok());
    let loop_definition = lesson_61();
    let recording = project_run(&loop_definition, &plant_samples(project_commands));
    let result =
        answer(&recording, &loop_definition, &inputs).expect("lesson 69's windows qualify");
    assert_eq!(result.trials, inputs.settled_trials.unwrap_or_default());
    let wire = serde_json::to_value(&result.trials[0]).expect("a trial encodes");
    assert_eq!(wire["baseline"]["endSeconds"], 1.9);
    assert_eq!(wire["settled"]["startSeconds"], 27.1);
}

#[test]
fn a_request_for_another_recording_or_revision_is_refused() {
    let loop_definition = lesson_61();
    let recording = project_run(&loop_definition, &plant_samples(project_commands));
    let other = AnalysisInputs {
        recording_id: Some("rec-other".to_string()),
        ..lesson_inputs()
    };
    let failure = answer(&recording, &loop_definition, &other)
        .expect_err("the request names another recording");
    assert_eq!(text_of(&failure), NO_RECORDING);
    let revised = AnalysisInputs {
        recording_revision: Some("sha256:other".to_string()),
        ..lesson_inputs()
    };
    let failure = answer(&recording, &loop_definition, &revised)
        .expect_err("the request names another revision");
    assert_eq!(text_of(&failure), NO_REVISION);
}

#[test]
fn without_a_recording_the_study_keeps_its_last_one() {
    let kept = |id: &str| -> Result<RecordedRun, Failure> {
        let mut run = fixture_run(Vec::new(), Vec::new());
        run.recording_id = id.to_string();
        Ok(run)
    };
    let recordings = vec!["rec-first".to_string(), "rec-last".to_string()];
    let (id, run) = last_recording(&recordings, kept).expect("the study keeps recordings");
    assert_eq!(id, "rec-last");
    assert_eq!(run.recording_id, "rec-last");
    let failure = last_recording(&[], |_| unreachable!("no recording is read"))
        .expect_err("the study keeps no recording");
    assert_eq!(text_of(&failure), NO_RECORDING_KEPT);
}
