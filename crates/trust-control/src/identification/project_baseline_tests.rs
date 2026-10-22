//! The baseline of a project-scripted experiment (specification 35, C12 Baseline, B3.4 steps 2 and 4, numerical annex N1.3). A run of it
//! waits for no steady baseline, so the baseline it carries, the window of its fit and the noise a return to Auto allows come from the
//! baseline the project declares, and its plan lasts the declared duration, whose last instant is the restoration sample. Lesson 61
//! declares a 2 s baseline in a 92.1 s test on a 100 ms task; Lesson 13 declares none in a 30.1 s test.

#![allow(clippy::float_cmp)] // Whole milliseconds and declared baselines are compared exactly on purpose.

use std::collections::BTreeMap;

use serde_json::json;

use super::{campaign_fit_window, declared_baseline, declared_baseline_noise};
use crate::experiment::{ControlExperimentKind, ControllerMode, ExperimentSample, PlanSignal};
use crate::identification::campaign::baseline::BaselineSummary;
use crate::identification::campaign::{campaign_maximum_ms, campaign_recording_ms};
use crate::identification::connected_run::{
    fit_window, restoration_allowance_ms, step_hold_ms, step_plan, FitWindow,
};
use crate::identification::policy::IdentificationMethod;
use crate::loop_definition::LoopDefinition;
use crate::messages::RecordedChartSample;
use crate::recording::channels::ChannelQuality;
use crate::recording::fixtures::loop_definition;
use crate::recording::EvidenceSource;
use crate::signals::Quality;

/// A project-scripted test on a task of `period_ms`: the project owns its scenario and enables itself when the activation is written
/// (C12). It declares `baseline_seconds` of baseline when it gives one, and records `duration_seconds`.
fn project_loop(
    period_ms: f64,
    baseline_seconds: Option<f64>,
    duration_seconds: f64,
) -> LoopDefinition {
    let mut value = serde_json::to_value(loop_definition()).expect("the fixture loop encodes");
    value["controller"]["taskPeriodMs"] = json!(period_ms);
    value["observedSampleTiming"] = json!({ "nominalPeriodMs": period_ms });
    let mut experiment = json!({
        "execution": "project-scripted", "kind": "observation", "amplitude": 0,
        "durationSeconds": duration_seconds, "seed": 1,
        "activation": { "symbol": "P1.Run", "address": "%MX64.0", "iecType": "BOOL", "direction": "write" }
    });
    if let Some(baseline) = baseline_seconds {
        experiment["baselineSeconds"] = json!(baseline);
    }
    value["experiment"] = experiment;
    serde_json::from_value(value).expect("the project test decodes")
}

/// One signal of a live sample of the fixture loop, good and in percent.
fn signal(symbol: &str, address: &str, value: f64) -> PlanSignal {
    PlanSignal {
        symbol: symbol.to_owned(),
        address: Some(address.to_owned()),
        value,
        unit: "%".to_owned(),
        quality: Quality::Good,
    }
}

/// A live sample of the fixture loop on a simulated target: the setpoint and the measurement at 45 %, the output at 30 %.
fn live() -> ExperimentSample {
    ExperimentSample {
        scan: 7,
        source: EvidenceSource::SimulatedPlant,
        controller_mode: ControllerMode::Auto,
        setpoint: Some(signal("LevelSP", "%ML0", 45.0)),
        process_variable: signal("LevelPV", "%IL0", 45.0),
        manipulated_variable: signal("ValveMV", "%QL0", 30.0),
        additional: Vec::new(),
    }
}

/// The steady baseline of a campaign: 45 %, a 30 s window at one sample per second.
fn steady() -> BaselineSummary {
    BaselineSummary {
        mean: 45.0,
        noise: 0.03,
        half_range: 0.05,
        spacing_seconds: 1.0,
        window_seconds: 30.0,
        input_mean: None,
        input_noise: None,
    }
}

/// One sample of a recording, `index` samples of `period_seconds` after its first scan.
fn recorded(
    index: u32,
    period_seconds: f64,
    pv: Option<f64>,
    quality: ChannelQuality,
) -> RecordedChartSample {
    RecordedChartSample {
        elapsed_seconds: f64::from(index) * period_seconds,
        scan: u64::from(index),
        quality,
        sp: None,
        pv,
        requested_mv: None,
        applied_mv: None,
        additional: Vec::new(),
        plotted: BTreeMap::new(),
    }
}

#[test]
fn a_project_test_carries_its_declared_baseline_in_place_of_a_steady_one() {
    let lesson_61 = project_loop(100.0, Some(2.0), 92.1);
    let baseline = declared_baseline(&lesson_61, Some(&live())).expect("a project test");
    assert_eq!(baseline.window_seconds, 2.0, "the declared 2 s");
    assert_eq!(baseline.spacing_seconds, 0.1, "one sample per 100 ms task");
    assert_eq!(baseline.mean, 45.0, "the reading the run starts from");
    assert_eq!(
        (baseline.noise, baseline.half_range),
        (0.0, 0.0),
        "no steady window is measured before the plan starts"
    );
    assert!(baseline.input_mean.is_none() && baseline.input_noise.is_none());

    // The plan holds the activation off for whole sample periods (C12), and so does the window.
    let rounded = declared_baseline(&project_loop(100.0, Some(2.04), 92.1), Some(&live()))
        .expect("a project test");
    assert_eq!(
        rounded.window_seconds, 2.1,
        "2.04 s is 21 periods of 100 ms"
    );

    // A project test that declares no baseline waits for the steady baseline a campaign waits for (B3.4 step 2): carrying an empty
    // declared baseline instead would leave its fit one sample before the first input change, and no model.
    let lesson_13 = project_loop(100.0, None, 30.1);
    assert!(
        declared_baseline(&lesson_13, Some(&live())).is_none(),
        "lesson 13 declares no baseline, so its run waits for a steady one"
    );

    assert!(
        declared_baseline(&loop_definition(), Some(&live())).is_none(),
        "an experiment Control Studio runs waits for its own steady baseline (B3.4 step 2)"
    );
}

#[test]
fn the_fit_window_of_a_project_test_is_its_declared_baseline_and_a_campaigns_is_its_steady_window()
{
    let lesson_61 = project_loop(100.0, Some(2.0), 92.1);
    let baseline = declared_baseline(&lesson_61, Some(&live())).expect("a project test");
    let window = campaign_fit_window(&lesson_61, 0.0, &baseline);
    assert_eq!(
        window.fit_from_seconds, 0.0,
        "the fit starts with the recording"
    );
    assert!(
        (window.baseline_end_seconds - 1.9).abs() < 1e-9,
        "its baseline ends with the last sample before the plan enables the project at 2 s, the first sample of the project's run: {}",
        window.baseline_end_seconds
    );
    // What the run journaled does not move the window: a resumed run reads the same one.
    assert_eq!(
        campaign_fit_window(&lesson_61, 12.5, &baseline),
        campaign_fit_window(&lesson_61, 0.0, &baseline)
    );

    // A project test that declares no baseline waited for a steady one, so its fit starts one window before that baseline's end, as a
    // campaign's does (B3.4): 30 s before the end at 45 s.
    let lesson_13 = project_loop(100.0, None, 30.1);
    assert_eq!(
        campaign_fit_window(&lesson_13, 45.0, &steady()),
        FitWindow {
            fit_from_seconds: 15.0,
            baseline_end_seconds: 45.0
        },
        "lesson 13 fits from the steady baseline it waited for, not from an empty declared one"
    );

    assert_eq!(
        campaign_fit_window(&loop_definition(), 92.0, &steady()),
        fit_window(92.0, 30.0),
        "a campaign fits from one window before the end of its steady baseline"
    );
}

#[test]
fn the_declared_baselines_samples_give_the_noise_a_return_to_auto_allows() {
    let lesson_61 = project_loop(100.0, Some(2.0), 92.1);
    // 2 s at rest, alternating by 0.1 about 35 degC, then the project's run raises the measurement.
    let rest = |index: u32| 35.0 + if index.is_multiple_of(2) { 0.05 } else { -0.05 };
    let mut samples: Vec<RecordedChartSample> = (0..20)
        .map(|index| recorded(index, 0.1, Some(rest(index)), ChannelQuality::Good))
        .collect();
    samples.extend((20..40).map(|index| recorded(index, 0.1, Some(36.0), ChannelQuality::Good)));
    let noise = declared_baseline_noise(&lesson_61, &samples).expect("20 samples of baseline");
    assert!(
        (noise - 0.05).abs() < 1e-9,
        "half the range of the baseline, not of the run after it: {noise}"
    );

    // A sample whose quality is not good, or which holds no value, is not the baseline.
    samples.push(recorded(5, 0.1, Some(99.0), ChannelQuality::Bad));
    samples.push(recorded(6, 0.1, Some(99.0), ChannelQuality::Forced));
    samples.push(recorded(7, 0.1, None, ChannelQuality::Good));
    assert_eq!(
        declared_baseline_noise(&lesson_61, &samples),
        Some(noise),
        "only the good samples count"
    );
}

#[test]
fn a_project_test_without_a_usable_declared_baseline_gives_no_noise() {
    let on_task = |period_ms: f64, baseline: Option<f64>| project_loop(period_ms, baseline, 92.1);
    let samples: Vec<RecordedChartSample> = (0..30)
        .map(|index| recorded(index, 0.1, Some(35.0), ChannelQuality::Good))
        .collect();
    assert_eq!(
        declared_baseline_noise(&on_task(100.0, None), &samples),
        None,
        "a test that declares no baseline has none (C12)"
    );
    // A 2 s baseline on a 1 s task holds three samples at most, fewer than the twenty a baseline needs (B3.4 step 2).
    let slow: Vec<RecordedChartSample> = (0..30)
        .map(|index| recorded(index, 1.0, Some(35.0), ChannelQuality::Good))
        .collect();
    assert_eq!(
        declared_baseline_noise(&on_task(1_000.0, Some(2.0)), &slow),
        None
    );
    assert_eq!(
        declared_baseline_noise(&loop_definition(), &samples),
        None,
        "the noise of an experiment Control Studio runs is its steady baseline's"
    );
}

#[test]
fn a_project_test_plans_its_declared_duration_and_enables_the_project_when_its_baseline_ends() {
    let project_sequence = IdentificationMethod {
        kind: ControlExperimentKind::ProjectSequence,
        manual: false,
        automatic: true,
    };
    // Lesson 61: 2 s of baseline in 92.1 s. Lesson 89: 2 s of baseline in 26 s on a 20 ms task. (Lesson 13 declares no baseline and
    // waits for a steady one: see the next test.)
    for (period_ms, baseline_seconds, duration_seconds, enabled_after_ms) in [
        (100.0, Some(2.0), 92.1, Some(2_000.0)),
        (20.0, Some(2.0), 26.0, Some(2_000.0)),
    ] {
        let project = project_loop(period_ms, baseline_seconds, duration_seconds);
        let baseline = declared_baseline(&project, Some(&live())).expect("a project test");
        let maximum_ms = campaign_maximum_ms(&project).expect("the declared maximum");
        let recording_ms = campaign_recording_ms(&project, None).expect("the planned recording");
        let declared_ms = (duration_seconds * 1_000.0).round();
        // No wait and no allowance: the hold is the declared duration, counted from the start of the recording.
        let hold_ms = step_hold_ms(
            maximum_ms,
            recording_ms,
            restoration_allowance_ms(&project, &baseline),
            0.0,
            0.0,
            1,
        );
        assert_eq!(
            hold_ms, declared_ms,
            "{duration_seconds} s: the whole declared duration"
        );
        let plan = step_plan(
            &project,
            Some(&live()),
            project_sequence,
            &baseline,
            hold_ms,
            false,
            None,
        )
        .expect("the project's test is planned");
        assert_eq!(plan.kind, ControlExperimentKind::ProjectSequence);
        assert_eq!(
            plan.duration_ms, declared_ms,
            "the runtime counts a duration to the execution at which the restoration takes effect (E5.4), so the plan is the declared \
             duration and the activation is in force for every scan before it"
        );
        assert_eq!(
            plan.trajectory.last().map(|point| point.at_ms),
            enabled_after_ms,
            "the plan enables the project when the declared baseline ends"
        );
        // The project reads the activation and reacts in the same execution, so the sample at the instant the plan enables it already holds
        // the first change of the plant input: the baseline of the fit ends one sample before it.
        let end_ms = campaign_fit_window(&project, 0.0, &baseline).baseline_end_seconds * 1_000.0;
        let expected_end_ms = (enabled_after_ms.unwrap_or(0.0) - period_ms).max(0.0);
        assert!(
            (end_ms - expected_end_ms).abs() < 1e-6,
            "the fit's baseline ends with the last sample before the project is enabled: {end_ms} ms, not {expected_end_ms} ms"
        );
    }
}

#[test]
fn a_project_test_without_a_baseline_waits_for_a_steady_one_and_then_plans_its_declared_duration() {
    let project_sequence = IdentificationMethod {
        kind: ControlExperimentKind::ProjectSequence,
        manual: false,
        automatic: true,
    };
    // Lesson 13: no baseline in 30.1 s on a 100 ms task. Its run waits for the steady baseline a campaign waits for (B3.4 step 2), so
    // its recording plans that wait, five minutes, before the 30.1 s. A run that planned no wait would leave the fit one sample before
    // the first input change.
    let project = project_loop(100.0, None, 30.1);
    assert!(declared_baseline(&project, Some(&live())).is_none());
    let maximum_ms = campaign_maximum_ms(&project).expect("the declared maximum");
    let recording_ms = campaign_recording_ms(&project, None).expect("the planned recording");
    assert_eq!(
        recording_ms, 330_100,
        "300 s of wait and the declared 30.1 s"
    );
    // After the wait the hold is still the whole declared duration: the maximum counts from the end of the steady baseline (B3.4 step 4),
    // and the declared duration already ends with its restoration sample, so there is no allowance (C12).
    let hold_ms = step_hold_ms(
        maximum_ms,
        recording_ms,
        restoration_allowance_ms(&project, &steady()),
        45.0,
        45.0,
        1,
    );
    assert_eq!(
        hold_ms, 30_100.0,
        "the whole declared duration after the wait"
    );
    let plan = step_plan(
        &project,
        Some(&live()),
        project_sequence,
        &steady(),
        hold_ms,
        false,
        None,
    )
    .expect("the project's test is planned");
    assert_eq!(plan.duration_ms, 30_100.0);
    assert_eq!(
        plan.trajectory.last().map(|point| point.at_ms),
        None,
        "no declared baseline: the plan enables the project at its first execution"
    );
}

#[test]
fn a_campaign_keeps_its_allowance_of_one_baseline_window() {
    let campaign = loop_definition();
    assert_eq!(
        restoration_allowance_ms(&campaign, &steady()),
        30_000.0,
        "a campaign leaves one baseline window to restore (B3.4 step 4)"
    );
    let project = project_loop(100.0, Some(2.0), 92.1);
    assert_eq!(
        restoration_allowance_ms(&project, &steady()),
        0.0,
        "a project-scripted experiment's declared duration already ends with its restoration sample (C12)"
    );
}
