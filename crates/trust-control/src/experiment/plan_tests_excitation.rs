//! Plan review of the excitation experiments: the setpoint and output steps, the pulses, the relay, the PRBS and the
//! multisine, built from the loop's current evidence. Split from `plan_tests.rs` by concept.
#![allow(clippy::float_cmp)] // The plans carry exact values on purpose.

use serde_json::json;

use super::super::{
    build_control_experiment_plan, manual_command_binding, ControlExperimentKind,
    ControlExperimentPlan, ControlExperimentPoint, ControlExperimentRequestKind,
    ControlRelayFeedbackPlan, ControllerMode, ExperimentModeBinding, ExperimentSample,
    ExperimentValue, PlanAuxiliarySignal, PlanInput, PlanSource, Quality,
};
use super::{accepted, manual, point, refusal, request, sample, setpoint_stop_conditions, signal};
use crate::experiment::trajectory::{experiment_trajectory, prbs_points};
use crate::loop_definition::{AuxiliarySignal, ManualOutputType, Topology};
use crate::recording::fixtures::{bound_loop, loop_definition, loop_with_disturbance};

/// The sample with the writeable disturbance `feed-flow` at 32, good (`sampleWithDisturbance()`).
fn sample_with_disturbance() -> ExperimentSample {
    let mut sample = sample();
    sample.additional = vec![PlanAuxiliarySignal {
        id: "feed-flow".to_string(),
        signal: signal("FeedFlow", Some("%QL8"), 32.0, "%", Quality::Good),
    }];
    sample
}

fn relay_expected() -> ControlRelayFeedbackPlan {
    ControlRelayFeedbackPlan {
        process_variable: "LevelPV".to_string(),
        address: "%IL0".to_string(),
        value_type: "LREAL".to_string(),
        unit: "%".to_string(),
        center: 48.0,
        hysteresis: 0.5,
        lower_threshold: 47.5,
        upper_threshold: 48.5,
        low_output: 27.0,
        high_output: 37.0,
    }
}

#[test]
fn a_concise_bounded_setpoint_plan_is_built_from_the_current_loop_evidence() {
    let plan = accepted(request(
        &bound_loop(),
        &sample(),
        ControlExperimentRequestKind::SetpointStep,
        5.0,
        30.0,
    ));
    assert_eq!(
        plan,
        ControlExperimentPlan {
            id: "experiment:loop:tank:41:setpoint_step".to_string(),
            source: PlanSource::Simulation,
            kind: ControlExperimentKind::SetpointStep,
            affected_signal: "LevelSP".to_string(),
            address: Some("%ML0".to_string()),
            value_type: "LREAL".to_string(),
            from: ExperimentValue::Number(50.0),
            to: ExperimentValue::Number(55.0),
            unit: "%".to_string(),
            minimum: 0.0,
            maximum: 100.0,
            duration_ms: 30_000.0,
            expected_scan: 41,
            task: Some("MainTask".to_string()),
            mode_binding: Some(ExperimentModeBinding {
                symbol: "P1.Run".to_string(),
                manual_value: false,
            }),
            stop_conditions: setpoint_stop_conditions("30"),
            restoration_value: ExperimentValue::Number(50.0),
            requires_physical_approval: false,
            trajectory: vec![point(0.0, 55.0)],
            abort_binding: None,
            relay_feedback: None,
            interval_ms: None,
            seed: None,
            frequencies_hz: None,
        }
    );
}

#[test]
fn an_mv_pulse_is_a_bounded_rise_and_return_and_an_mv_step_holds() {
    let manual_sample = manual(sample());
    let pulse = accepted(request(
        &loop_definition(),
        &manual_sample,
        ControlExperimentRequestKind::MvPulse,
        5.0,
        30.0,
    ));
    let step = accepted(request(
        &loop_definition(),
        &manual_sample,
        ControlExperimentRequestKind::MvStep,
        5.0,
        30.0,
    ));
    assert_eq!(pulse.trajectory, [point(0.0, 37.0), point(15_000.0, 32.0)]);
    assert_eq!(step.trajectory, [point(0.0, 37.0)]);
}

#[test]
fn the_declared_manual_command_is_written_rather_than_an_observed_controller_output() {
    let mut loop_definition = loop_definition();
    loop_definition.controller.manual_output_signal =
        Some("TankController.ManualCommand".to_string());
    loop_definition.controller.manual_output_address = Some("%ML8".to_string());
    loop_definition.controller.manual_output_type = Some(ManualOutputType::Lreal);
    let pulse = accepted(request(
        &loop_definition,
        &manual(sample()),
        ControlExperimentRequestKind::MvPulse,
        5.0,
        30.0,
    ));
    assert_eq!(pulse.affected_signal, "TankController.ManualCommand");
    assert_eq!(pulse.address.as_deref(), Some("%ML8"));
    assert_eq!(pulse.value_type, "LREAL");
    assert_eq!(
        pulse.from,
        ExperimentValue::Number(32.0),
        "the measured actual MV is the Manual-command baseline"
    );
    assert_eq!(pulse.trajectory, [point(0.0, 37.0), point(15_000.0, 32.0)]);
}

#[test]
fn a_manual_command_without_an_address_is_written_by_its_name() {
    let mut loop_definition = loop_definition();
    loop_definition.controller.manual_output_signal = Some("Main.TIC101_ManualOutput".to_string());
    loop_definition.controller.manual_output_type = Some(ManualOutputType::Lreal);
    let mut manual_sample = manual(sample());
    manual_sample.manipulated_variable.value = 30.0;
    let plan = accepted(request(
        &loop_definition,
        &manual_sample,
        ControlExperimentRequestKind::MvStep,
        10.0,
        120.0,
    ));
    assert_eq!(plan.affected_signal, "Main.TIC101_ManualOutput");
    assert_eq!(
        plan.address, None,
        "without an address the variable is written by name"
    );
    assert_eq!(plan.to, ExperimentValue::Number(40.0));
}

#[test]
fn a_setpoint_without_an_address_is_written_by_its_name() {
    let mut loop_definition = loop_definition();
    loop_definition
        .signals
        .setpoint
        .as_mut()
        .expect("the setpoint is set")
        .address = None;
    let mut setpoint_sample = sample();
    setpoint_sample
        .setpoint
        .as_mut()
        .expect("the setpoint is set")
        .address = None;
    let plan = accepted(request(
        &loop_definition,
        &setpoint_sample,
        ControlExperimentRequestKind::SetpointStep,
        2.0,
        120.0,
    ));
    assert_eq!(plan.affected_signal, "LevelSP");
    assert_eq!(plan.address, None);
}

#[test]
fn every_declared_bounded_experiment_is_built_from_the_same_loop_and_envelope() {
    let loop_definition = loop_with_disturbance();
    let kinds = [
        ControlExperimentRequestKind::Observation,
        ControlExperimentRequestKind::SetpointStep,
        ControlExperimentRequestKind::LoadStep,
        ControlExperimentRequestKind::MvStep,
        ControlExperimentRequestKind::MvPulse,
        ControlExperimentRequestKind::Relay,
        ControlExperimentRequestKind::Prbs,
        ControlExperimentRequestKind::Multisine,
    ];
    let plans: Vec<ControlExperimentPlan> = kinds
        .iter()
        .map(|kind| {
            let sample = match kind {
                ControlExperimentRequestKind::MvStep
                | ControlExperimentRequestKind::MvPulse
                | ControlExperimentRequestKind::Relay
                | ControlExperimentRequestKind::Prbs
                | ControlExperimentRequestKind::Multisine => manual(sample_with_disturbance()),
                _ => sample_with_disturbance(),
            };
            let result = build_control_experiment_plan(&PlanInput {
                loop_definition: &loop_definition,
                sample: Some(&sample),
                kind: *kind,
                amplitude: 5.0,
                duration_seconds: 10.0,
                physical_topology_reviewed: false,
                seed: Some(7),
                process_center: None,
                process_noise: None,
                hold_seconds: None,
            });
            accepted(result)
        })
        .collect();
    assert_eq!(
        plans.iter().map(|plan| plan.kind).collect::<Vec<_>>(),
        [
            ControlExperimentKind::Observation,
            ControlExperimentKind::SetpointStep,
            ControlExperimentKind::LoadStep,
            ControlExperimentKind::MvStep,
            ControlExperimentKind::MvPulse,
            ControlExperimentKind::Relay,
            ControlExperimentKind::Prbs,
            ControlExperimentKind::Multisine,
        ]
    );
    assert_eq!(plans[0].trajectory, [] as [ControlExperimentPoint; 0]);
    assert_eq!(
        plans[2].address.as_deref(),
        Some("%QL8"),
        "the disturbance is written at its own address"
    );
    assert_eq!(plans[5].trajectory, [] as [ControlExperimentPoint; 0]);
    assert_eq!(plans[5].relay_feedback, Some(relay_expected()));
    assert_eq!(plans[6].seed, Some(7));
    assert_eq!(plans[6].interval_ms, Some(500.0));
    let frequencies = plans[7]
        .frequencies_hz
        .clone()
        .expect("a multisine has frequencies");
    for (actual, expected) in frequencies.iter().zip([0.1, 0.2, 0.3]) {
        assert!(
            (actual - expected).abs() < 1e-12,
            "{actual} is not {expected}"
        );
    }
    for plan in plans
        .iter()
        .enumerate()
        .filter(|(index, plan)| *index > 0 && plan.kind != ControlExperimentKind::Relay)
        .map(|(_, plan)| plan)
    {
        assert!(
            !plan.trajectory.is_empty(),
            "{:?} writes a trajectory",
            plan.kind
        );
        assert!(plan
            .trajectory
            .iter()
            .all(|point| point.value >= plan.minimum && point.value <= plan.maximum));
        assert_eq!(
            plan.restoration_value,
            ExperimentValue::Number(match plan.from {
                ExperimentValue::Number(from) => from,
                ExperimentValue::Boolean(_) => panic!("an excitation starts from a number"),
            })
        );
    }
}

#[test]
fn a_relay_is_measured_feedback_instead_of_a_timed_square_wave() {
    let result = accepted(request(
        &loop_definition(),
        &manual(sample()),
        ControlExperimentRequestKind::Relay,
        5.0,
        30.0,
    ));
    assert_eq!(result.trajectory, [] as [ControlExperimentPoint; 0]);
    assert_eq!(result.to, ExperimentValue::Number(37.0));
    assert_eq!(result.relay_feedback, Some(relay_expected()));
}

#[test]
fn the_relay_centres_on_the_measured_baseline_and_switches_just_above_its_noise() {
    let relay = |center: Option<f64>, noise: Option<f64>| {
        build_control_experiment_plan(&PlanInput {
            loop_definition: &loop_definition(),
            sample: Some(&manual(sample())),
            kind: ControlExperimentRequestKind::Relay,
            amplitude: 5.0,
            duration_seconds: 30.0,
            physical_topology_reviewed: false,
            seed: None,
            process_center: center,
            process_noise: noise,
            hold_seconds: None,
        })
    };
    let measured = accepted(relay(Some(48.01), Some(0.02)))
        .relay_feedback
        .expect("a relay plan has its feedback");
    assert!((measured.center - 48.01).abs() < 1e-12);
    assert!(
        (measured.hysteresis - 0.04).abs() < 1e-12,
        "a ±0.02 % noise gives a band of 0.04 %"
    );
    assert!((measured.lower_threshold - 47.97).abs() < 1e-9);
    assert!((measured.upper_threshold - 48.05).abs() < 1e-9);
    let noise_free = accepted(relay(Some(48.0), Some(0.0)))
        .relay_feedback
        .expect("a relay plan has its feedback");
    assert!(
        (noise_free.hysteresis - 0.01).abs() < 1e-12,
        "a noise-free baseline keeps 0.01 % of the range"
    );
    let undisturbed = accepted(relay(None, None))
        .relay_feedback
        .expect("a relay plan has its feedback");
    assert_eq!(
        undisturbed.hysteresis, 0.5,
        "without a measured baseline the band is 0.5 % of the range"
    );
}

#[test]
fn an_open_loop_output_step_is_reviewed_without_inventing_a_controller_mode() {
    let mut loop_definition = loop_definition();
    loop_definition.topology = Some(Topology::OpenLoop);
    loop_definition.signals.setpoint = None;
    let mut open_sample = manual(sample());
    open_sample.setpoint = None;
    open_sample.controller_mode = ControllerMode::Unavailable;
    let result = accepted(request(
        &loop_definition,
        &open_sample,
        ControlExperimentRequestKind::MvStep,
        5.0,
        30.0,
    ));
    assert_eq!(result.address.as_deref(), Some("%QL0"));
}

#[test]
fn an_open_loop_disturbance_step_is_reviewed_without_inventing_a_controller_mode() {
    let disturbance: AuxiliarySignal = serde_json::from_value(json!({
        "id": "gust", "channel": "airframe", "role": "disturbance", "symbol": "GustLoad",
        "address": "%ML32", "iecType": "LREAL", "direction": "write", "unit": "g",
        "range": { "minimum": -1.0, "maximum": 1.0 }
    }))
    .expect("the disturbance decodes");
    let mut loop_definition = loop_definition();
    loop_definition.topology = Some(Topology::OpenLoop);
    loop_definition.signals.setpoint = None;
    loop_definition.signals.additional = Some(vec![disturbance]);
    let mut open_sample = sample();
    open_sample.setpoint = None;
    open_sample.controller_mode = ControllerMode::Unavailable;
    open_sample.additional = vec![PlanAuxiliarySignal {
        id: "gust".to_string(),
        signal: signal("GustLoad", Some("%ML32"), 0.0, "g", Quality::Good),
    }];
    let result = accepted(request(
        &loop_definition,
        &open_sample,
        ControlExperimentRequestKind::LoadStep,
        0.1,
        30.0,
    ));
    assert_eq!(result.address.as_deref(), Some("%ML32"));
}

#[test]
fn a_prbs_sequence_steps_by_its_seed_and_xorshift_bits() {
    // The xorshift32 state from seed 7 is 0xE007, then 0x1CE0E7, then 0x1C099207: bits 1, 1 and 1.
    let points = prbs_points(32.0, 5.0, 10_000.0, 500.0, 7);
    assert_eq!(points.len(), 20);
    assert_eq!(points[0].value, 37.0);
    assert_eq!(points[1].value, 37.0);
    assert_eq!(points[1].at_ms, 500.0);
}

#[test]
fn a_mv_pulse_returns_at_the_floor_of_half_its_duration() {
    let points =
        experiment_trajectory(ControlExperimentKind::MvPulse, 32.0, 5.0, 10_001.0, 10.0, 1);
    assert_eq!(points, [point(0.0, 37.0), point(5_000.0, 32.0)]);
}

#[test]
fn a_manual_command_needs_a_real_or_lreal_type() {
    let mut loop_definition = loop_definition();
    loop_definition.controller.manual_output_signal = Some("Command".to_string());
    loop_definition.controller.manual_output_type = Some(ManualOutputType::Bool);
    assert!(manual_command_binding(&loop_definition).is_none());
    loop_definition.controller.manual_output_type = Some(ManualOutputType::Real);
    assert_eq!(
        manual_command_binding(&loop_definition).map(|command| command.iec_type),
        Some("REAL")
    );
}

#[test]
fn a_relay_restores_the_output_it_started_from() {
    // The relay's restoration in `control-studio-experiment.test.ts` (the every-experiment test, line 324): a relay writes
    // no trajectory, and it restores the manipulated variable it started from, which is the sample's 32 %.
    let loop_definition = loop_with_disturbance();
    let relay = accepted(build_control_experiment_plan(&PlanInput {
        loop_definition: &loop_definition,
        sample: Some(&manual(sample_with_disturbance())),
        kind: ControlExperimentRequestKind::Relay,
        amplitude: 5.0,
        duration_seconds: 10.0,
        physical_topology_reviewed: false,
        seed: Some(7),
        process_center: None,
        process_noise: None,
        hold_seconds: None,
    }));
    assert_eq!(relay.from, ExperimentValue::Number(32.0));
    assert_eq!(
        relay.restoration_value, relay.from,
        "a relay restores the output it started from"
    );
}

#[test]
fn an_output_excitation_without_a_plant_input_limit_is_refused_with_the_limit_named() {
    let mut loop_definition = loop_definition();
    loop_definition.limits.output_minimum = None;
    assert_eq!(
        refusal(request(
            &loop_definition,
            &manual(sample()),
            ControlExperimentRequestKind::MvStep,
            5.0,
            30.0
        )),
        "The plant input limits are missing: limits.outputMinimum must be set, with the minimum below the maximum."
    );
    let setpoint = accepted(request(
        &loop_definition,
        &sample(),
        ControlExperimentRequestKind::SetpointStep,
        5.0,
        30.0,
    ));
    assert_eq!(
        (setpoint.minimum, setpoint.maximum),
        (0.0, 100.0),
        "a setpoint step is bounded by its signal's range, not by the output limits"
    );
}
