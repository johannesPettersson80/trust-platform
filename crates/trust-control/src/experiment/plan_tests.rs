//! Tests of the experiment plan review. The assertions follow `control-studio-experiment.test.ts` and
//! `control-studio-next-actions.test.ts` (the Test recording window); the others are worked out by hand.
#![allow(clippy::float_cmp)] // The plans carry exact values on purpose.

use serde_json::{json, Value};

use super::*;
use crate::experiment::experiment_recording_window_ms;
use crate::recording::fixtures::loop_definition;

#[cfg(test)]
#[path = "plan_tests_project.rs"]
mod project;

#[cfg(test)]
#[path = "plan_tests_excitation.rs"]
mod excitation;

fn signal(
    symbol: &str,
    address: Option<&str>,
    value: f64,
    unit: &str,
    quality: Quality,
) -> PlanSignal {
    PlanSignal {
        symbol: symbol.to_string(),
        address: address.map(str::to_string),
        value,
        unit: unit.to_string(),
        quality,
    }
}

/// The live sample of the `TypeScript` tests (`sample()`): Auto mode, a setpoint of 50, a measurement of 48 and a
/// manipulated variable of 32, all good, at scan 41.
fn sample() -> ExperimentSample {
    ExperimentSample {
        scan: 41,
        source: EvidenceSource::SimulatedPlant,
        controller_mode: ControllerMode::Auto,
        setpoint: Some(signal("LevelSP", Some("%ML0"), 50.0, "%", Quality::Good)),
        process_variable: signal("LevelPV", Some("%IL0"), 48.0, "%", Quality::Good),
        manipulated_variable: signal("ValveMV", Some("%QL0"), 32.0, "%", Quality::Good),
        additional: Vec::new(),
    }
}

fn manual(mut sample: ExperimentSample) -> ExperimentSample {
    sample.controller_mode = ControllerMode::Manual;
    sample
}

fn request(
    loop_definition: &LoopDefinition,
    sample: &ExperimentSample,
    kind: ControlExperimentRequestKind,
    amplitude: f64,
    duration_seconds: f64,
) -> ControlExperimentPlanResult {
    build_control_experiment_plan(&PlanInput {
        loop_definition,
        sample: Some(sample),
        kind,
        amplitude,
        duration_seconds,
        physical_topology_reviewed: false,
        seed: None,
        process_center: None,
        process_noise: None,
        hold_seconds: None,
    })
}

fn accepted(result: ControlExperimentPlanResult) -> ControlExperimentPlan {
    match result {
        ControlExperimentPlanResult::Accepted { plan } => *plan,
        ControlExperimentPlanResult::Rejected { reason } => {
            panic!("the request is refused: {reason}")
        }
    }
}

fn refusal(result: ControlExperimentPlanResult) -> String {
    match result {
        ControlExperimentPlanResult::Rejected { reason } => reason,
        ControlExperimentPlanResult::Accepted { .. } => panic!("the request is accepted"),
    }
}

fn point(at_ms: f64, value: f64) -> ControlExperimentPoint {
    ControlExperimentPoint { at_ms, value }
}

/// The loop with its experiment replaced by the loop file's `experiment` JSON.
fn with_experiment(mut loop_definition: LoopDefinition, experiment: Value) -> LoopDefinition {
    loop_definition.experiment =
        Some(serde_json::from_value(experiment).expect("the experiment decodes"));
    loop_definition
}

fn setpoint_stop_conditions(seconds: &str) -> Vec<String> {
    vec![
        "Abort".to_string(),
        format!("{seconds} s timeout"),
        "Loop mode changed".to_string(),
        "Bad runtime data".to_string(),
        "Signal force".to_string(),
        "Signal outside its limits".to_string(),
        "Runtime fault".to_string(),
    ]
}

#[test]
fn every_supported_experiment_is_offered_without_the_legacy_alias() {
    let choices = control_experiment_choices();
    assert_eq!(
        choices
            .iter()
            .map(|choice| choice.value)
            .collect::<Vec<_>>(),
        [
            ControlExperimentRequestKind::Automatic,
            ControlExperimentRequestKind::Observation,
            ControlExperimentRequestKind::SetpointStep,
            ControlExperimentRequestKind::LoadStep,
            ControlExperimentRequestKind::MvStep,
            ControlExperimentRequestKind::MvPulse,
            ControlExperimentRequestKind::Relay,
            ControlExperimentRequestKind::Prbs,
            ControlExperimentRequestKind::Multisine,
        ]
    );
    assert_eq!(
        choices
            .iter()
            .map(|choice| choice.label)
            .collect::<Vec<_>>(),
        [
            "Automatic bounded excitation",
            "Observe without excitation",
            "Setpoint step",
            "Load disturbance step",
            "Manual output step",
            "Manual output pulse",
            "Relay feedback",
            "PRBS",
            "Multisine",
        ]
    );
}

#[test]
fn a_test_form_opens_with_the_exact_defaults_the_lesson_loop_owns() {
    let configured = with_experiment(
        loop_definition(),
        json!({
            "execution": "project-scripted", "kind": "observation", "amplitude": 0.0,
            "durationSeconds": 120.0, "seed": 20_260_824_u64
        }),
    );
    assert_eq!(
        control_experiment_defaults(&configured),
        ControlExperimentDefaults {
            execution: Execution::ProjectScripted,
            kind: ControlExperimentRequestKind::Observation,
            amplitude: 0.0,
            duration_seconds: 120.0,
            seed: 20_260_824,
        }
    );
    assert_eq!(
        control_experiment_defaults(&loop_definition()),
        ControlExperimentDefaults {
            execution: Execution::ControlStudio,
            kind: ControlExperimentRequestKind::Automatic,
            amplitude: 10.0,
            duration_seconds: 30.0,
            seed: 1,
        }
    );
}

#[test]
fn a_configured_experiment_without_a_declared_duration_opens_its_form_at_thirty_seconds() {
    let observation = with_experiment(
        loop_definition(),
        json!({
            "execution": "control-studio", "kind": "observation", "amplitude": 0.0, "seed": 7
        }),
    );
    assert_eq!(
        control_experiment_defaults(&observation).duration_seconds,
        30.0,
        "an automatic window with no declared duration starts at 30 s"
    );
    let step = with_experiment(
        loop_definition(),
        json!({
            "execution": "control-studio", "kind": "mv_step", "amplitude": 5.0, "seed": 7
        }),
    );
    assert_eq!(
        control_experiment_defaults(&step).duration_seconds,
        30.0,
        "a configured step with no declared duration starts at 30 s"
    );
}

#[test]
fn a_bounded_excitation_is_selected_automatically_while_manual_observation_remains() {
    let configured = with_experiment(
        loop_definition(),
        json!({
            "execution": "control-studio", "kind": "observation", "amplitude": 0.0, "durationSeconds": 30.0, "seed": 7
        }),
    );
    let defaults = control_experiment_defaults(&configured);
    assert_eq!(
        defaults.kind,
        ControlExperimentRequestKind::Automatic,
        "a zero-amplitude observation is an automatic request"
    );
    assert_eq!(
        defaults.amplitude, 10.0,
        "automatic amplitude starts at ten percent of the declared signal span"
    );
    let short = with_experiment(
        loop_definition(),
        json!({
            "execution": "control-studio", "kind": "observation", "amplitude": 0.0, "durationSeconds": 2.0, "seed": 7
        }),
    );
    assert_eq!(
        control_experiment_defaults(&short).duration_seconds,
        10.0,
        "a ten-second window unless the configured duration is longer"
    );

    let manual_sample = manual(sample());
    let automatic = accepted(request(
        &configured,
        &manual_sample,
        ControlExperimentRequestKind::Automatic,
        defaults.amplitude,
        defaults.duration_seconds,
    ));
    assert_eq!(
        automatic.kind,
        ControlExperimentKind::MvStep,
        "a Manual loop receives a bounded output step"
    );
    assert_eq!(
        automatic.trajectory,
        [point(0.0, 42.0)],
        "B3.4: the step is held from the start"
    );
    assert_eq!(automatic.restoration_value, ExperimentValue::Number(32.0));

    let automatic_auto = accepted(request(
        &configured,
        &sample(),
        ControlExperimentRequestKind::Automatic,
        defaults.amplitude,
        defaults.duration_seconds,
    ));
    assert_eq!(
        automatic_auto.kind,
        ControlExperimentKind::SetpointStep,
        "an Auto loop uses the declared setpoint"
    );
    assert_eq!(automatic_auto.to, ExperimentValue::Number(60.0));
    assert_eq!(automatic_auto.trajectory, [point(0.0, 60.0)]);
    assert_eq!(
        automatic_auto.restoration_value,
        ExperimentValue::Number(50.0)
    );

    let observation = accepted(request(
        &configured,
        &sample(),
        ControlExperimentRequestKind::Observation,
        0.0,
        defaults.duration_seconds,
    ));
    assert_eq!(
        observation.kind,
        ControlExperimentKind::Observation,
        "manual observation remains non-exciting"
    );
    assert_eq!(observation.trajectory, [] as [ControlExperimentPoint; 0]);

    let mut slow = configured.clone();
    slow.observed_sample_timing.nominal_period_ms = 500.0;
    assert_eq!(
        refusal(request(
            &slow,
            &manual_sample,
            ControlExperimentRequestKind::Automatic,
            defaults.amplitude,
            1.0
        )),
        "Automatic Test duration is too short to capture baseline, held step and restoration."
    );
}

#[test]
fn a_slow_process_experiment_of_twenty_minutes_is_accepted_and_one_over_twenty_four_hours_is_refused(
) {
    let plan = accepted(request(
        &loop_definition(),
        &sample(),
        ControlExperimentRequestKind::SetpointStep,
        5.0,
        1_200.0,
    ));
    assert_eq!(plan.duration_ms, 1_200_000.0);
    assert_eq!(
        refusal(request(
            &loop_definition(),
            &sample(),
            ControlExperimentRequestKind::SetpointStep,
            5.0,
            86_401.0
        )),
        "Duration must be between 1 s and 24 h."
    );
}

#[test]
fn unsafe_stale_forced_out_of_limit_and_unapproved_plans_are_refused() {
    let loop_definition = loop_definition();
    let base = sample();
    let refuse =
        |sample: Option<&ExperimentSample>, amplitude: f64, kind: ControlExperimentRequestKind| {
            build_control_experiment_plan(&PlanInput {
                loop_definition: &loop_definition,
                sample,
                kind,
                amplitude,
                duration_seconds: 30.0,
                physical_topology_reviewed: false,
                seed: None,
                process_center: None,
                process_noise: None,
                hold_seconds: None,
            })
        };
    let setpoint = ControlExperimentRequestKind::SetpointStep;
    assert_eq!(
        refusal(refuse(None, 5.0, setpoint)),
        "Fresh synchronized SP, PV, and MV are required."
    );
    let mut forced_setpoint = base.clone();
    forced_setpoint
        .setpoint
        .as_mut()
        .expect("the setpoint is set")
        .quality = Quality::Forced;
    assert_eq!(
        refusal(refuse(Some(&forced_setpoint), 5.0, setpoint)),
        "The affected signal is forced."
    );
    let mut bad_pv = base.clone();
    bad_pv.process_variable.quality = Quality::Bad;
    assert_eq!(
        refusal(refuse(Some(&bad_pv), 5.0, setpoint)),
        "Fresh good-quality SP, PV, and MV are required."
    );
    assert_eq!(
        refusal(refuse(Some(&base), 60.0, setpoint)),
        "The requested value 110 % is outside 0…100 %."
    );
    assert_eq!(
        refusal(refuse(
            Some(&base),
            5.0,
            ControlExperimentRequestKind::OutputStep
        )),
        "Output steps require confirmed Manual controller mode."
    );
    let mut physical = base.clone();
    physical.source = EvidenceSource::PhysicalPlant;
    assert_eq!(
        refusal(refuse(Some(&physical), 5.0, setpoint)),
        "A reviewed physical topology is required before approval can be requested."
    );
}

#[test]
fn a_test_recording_window_adds_at_least_ten_seconds_or_ten_percent_capped_at_the_limit() {
    assert_eq!(
        experiment_recording_window_ms(60_000),
        70_000,
        "the margin is at least 10 s"
    );
    assert_eq!(
        experiment_recording_window_ms(3_600_000),
        3_960_000,
        "the margin is 10 % of a longer hold"
    );
    assert_eq!(
        experiment_recording_window_ms(86_400_000),
        86_400_000,
        "the window is capped at the recording limit"
    );
    assert_eq!(
        experiment_recording_window_ms(123_456),
        135_802,
        "the margin of 12 345.6 ms rounds up to whole milliseconds (E4.2)"
    );
}
