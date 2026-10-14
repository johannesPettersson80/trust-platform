//! Tests of the channel step of the step-by-step setup (`configureLoopChannels.ts`; channels parked for V7 except the
//! single channel's rate limits and applied-action feedback).
#![allow(clippy::float_cmp)]

use super::*;
use crate::loop_definition::{
    Action, Controller, Design, Execution, Experiment, ExperimentKind, Limits, LoopDefinition,
    LoopSignal, Range, SampleTiming, Signals, Topology, LOOP_SCHEMA_VERSION,
};
use crate::setup::catalog::CompiledSignal;
use crate::setup::prompts::{
    ActuatorChoice, ChannelMode, FeedbackChoice, RateAnswer, SetupStepAnswers,
};

fn loop_signal(
    symbol: &str,
    address: &str,
    iec_type: &str,
    direction: Direction,
    unit: &str,
    range: (f64, f64),
) -> LoopSignal {
    LoopSignal {
        symbol: symbol.into(),
        address: Some(address.into()),
        iec_type: iec_type.into(),
        direction,
        unit: unit.into(),
        range: Range {
            minimum: range.0,
            maximum: range.1,
        },
    }
}

/// A loop with a process variable (`Main.PV`, bar, 0 to 12) and a continuous output (`Main.Output`, %, 0 to 100); the
/// observation of the custom open loop of `control-studio-command.test.ts`.
fn base_loop() -> LoopDefinition {
    LoopDefinition {
        schema_version: LOOP_SCHEMA_VERSION,
        id: "loop:pump".into(),
        name: "Pump".into(),
        revision: "sha256:test".into(),
        project_id: "project:test".into(),
        runtime_id: "runtime:test".into(),
        controller: Controller {
            instance: "P".into(),
            type_name: "Law".into(),
            revision: "sha256:unverified".into(),
            action: Action::Direct,
            task_period_ms: 10.0,
            design: Some(Design::Project),
            law: None,
            mode_signal: None,
            mode_when_true: None,
            tracking_signal: None,
            manual_output_signal: None,
            manual_output_address: None,
            manual_output_type: None,
            manual_output_follows_output: None,
            pid_initialization: None,
            state_feedback: None,
            on_off: None,
            project: None,
            group: None,
        },
        signals: Signals {
            setpoint: None,
            process_variable: loop_signal(
                "Main.PV",
                "%IL0",
                "LREAL",
                Direction::Read,
                "bar",
                (0.0, 12.0),
            ),
            manipulated_variable: loop_signal(
                "Main.Output",
                "%QL0",
                "LREAL",
                Direction::Write,
                "%",
                (0.0, 100.0),
            ),
            actuator: None,
            additional: None,
        },
        limits: Limits {
            output_minimum: Some(0.0),
            output_maximum: Some(100.0),
            rate_up_per_s: None,
            rate_down_per_s: None,
            expected_settling_seconds: None,
        },
        observed_sample_timing: SampleTiming {
            nominal_period_ms: 10.0,
            maximum_jitter_ms: None,
        },
        parameter_binding: None,
        parameter_purpose: None,
        topology: Some(Topology::OpenLoop),
        experiment: Some(Experiment {
            execution: Execution::ProjectScripted,
            kind: ExperimentKind::Observation,
            amplitude: 0.0,
            relay_direction: None,
            duration_seconds: Some(30.0),
            baseline_seconds: None,
            hold_parameter: None,
            seed: 1,
            activation: None,
            abort: None,
        }),
        plant_id: None,
        operating_point: None,
        identification_policy: None,
        verification_scenarios: None,
        channels: None,
        analysis: None,
        analysis_parameter_symbols: None,
        cascade: None,
    }
}

fn applied_signal(symbol: &str, address: &str, iec_type: &str) -> CompiledSignal {
    CompiledSignal {
        symbol: symbol.into(),
        address: address.into(),
        iec_type: iec_type.into(),
    }
}

/// The answers of a single channel with a continuous output, no rate limits and the output as its own feedback.
fn single_answers() -> SetupStepAnswers {
    SetupStepAnswers {
        channel_mode: Some(ChannelMode::Single),
        actuator: Some(ActuatorChoice::Continuous),
        rising_rate: Some(RateAnswer::Unlimited),
        falling_rate: Some(RateAnswer::Unlimited),
        feedback: Some(FeedbackChoice::UseOutput),
        process_variable: Some(applied_signal("Main.PV", "%IL0", "LREAL")),
        controller_output: Some(applied_signal("Main.Output", "%QL0", "LREAL")),
        ..SetupStepAnswers::default()
    }
}

#[test]
fn a_rate_answer_is_blank_for_no_limit_or_a_nonnegative_finite_number() {
    // configureLoopChannels.ts `actuatorSetup`: `!value.trim() || Number.isFinite(Number(value)) && Number(value) >= 0`,
    // and only a non-blank value is stored, as `Number(raw)`.
    assert_eq!(accept_rate(""), Ok(RateAnswer::Unlimited));
    assert_eq!(accept_rate("   "), Ok(RateAnswer::Unlimited));
    assert_eq!(accept_rate("2.5"), Ok(RateAnswer::PerSecond(2.5)));
    assert_eq!(
        accept_rate("0"),
        Ok(RateAnswer::PerSecond(0.0)),
        "zero is a nonnegative rate"
    );
    assert_eq!(
        accept_rate("0x10"),
        Ok(RateAnswer::PerSecond(16.0)),
        "read as Number() reads it"
    );
    assert_eq!(
        accept_rate("-1"),
        Err("Enter a nonnegative finite value.".to_owned())
    );
    assert_eq!(
        accept_rate("Infinity"),
        Err("Enter a nonnegative finite value.".to_owned())
    );
    assert_eq!(
        accept_rate("abc"),
        Err("Enter a nonnegative finite value.".to_owned())
    );
}

#[test]
fn a_single_channel_with_no_limits_adds_no_signal_and_writes_no_channel_model() {
    // control-studio-domain-neutral.test.ts ("never offers a built-in physical model structure while setting up channels"):
    // the single channel, with the defaults the test picks, completes. The channel model is parked, so `channels` stays
    // unset (V7).
    let mut loop_definition = base_loop();
    apply_channels(&mut loop_definition, &single_answers()).expect("the single channel completes");
    assert_eq!(
        loop_definition.limits.rate_up_per_s, None,
        "a blank rate declares no limit"
    );
    assert_eq!(loop_definition.limits.rate_down_per_s, None);
    assert_eq!(
        loop_definition.signals.actuator, None,
        "the output is its own feedback"
    );
    assert_eq!(loop_definition.signals.additional, Some(Vec::new()));
    assert_eq!(
        loop_definition.channels, None,
        "the channel model is parked for V7"
    );
}

#[test]
fn the_rate_limits_of_a_continuous_output_are_written_to_the_limits() {
    // control-studio-command.test.ts: the rising and falling rates are the limits of the loop when they are given.
    let mut answers = single_answers();
    answers.rising_rate = Some(RateAnswer::PerSecond(2.0));
    answers.falling_rate = Some(RateAnswer::PerSecond(3.0));
    let mut loop_definition = base_loop();
    apply_channels(&mut loop_definition, &answers).expect("the rates complete");
    assert_eq!(loop_definition.limits.rate_up_per_s, Some(2.0));
    assert_eq!(loop_definition.limits.rate_down_per_s, Some(3.0));
}

#[test]
fn an_applied_feedback_is_an_input_read_in_the_output_unit_over_the_output_range() {
    // configureLoopChannels.ts: the feedback binding is read, in the command's unit, over the actuator's range (the output
    // range).
    let mut answers = single_answers();
    answers.feedback = Some(FeedbackChoice::Signal(applied_signal(
        "Main.Applied",
        "%IL1",
        "LREAL",
    )));
    let mut loop_definition = base_loop();
    apply_channels(&mut loop_definition, &answers).expect("the feedback completes");
    assert_eq!(
        loop_definition.signals.actuator,
        Some(LoopSignal {
            symbol: "Main.Applied".into(),
            address: Some("%IL1".into()),
            iec_type: "LREAL".into(),
            direction: Direction::Read,
            unit: "%".into(),
            range: Range {
                minimum: 0.0,
                maximum: 100.0,
            },
        })
    );
}

#[test]
fn a_feedback_the_loop_uses_or_that_is_not_an_input_is_refused_by_name() {
    // configureLoopChannels.ts: the feedback list excludes the PV, the MV and the setpoint, and offers only `%I` addresses
    // of type REAL, LREAL or BOOL.
    let refused = |signal: CompiledSignal| {
        let mut answers = single_answers();
        answers.feedback = Some(FeedbackChoice::Signal(signal.clone()));
        let mut loop_definition = base_loop();
        let failure =
            apply_channels(&mut loop_definition, &answers).expect_err("the feedback is refused");
        assert_eq!(
            failure.message,
            texts::configure::not_applied_feedback(&signal.symbol)
        );
        assert_eq!(loop_definition.signals.actuator, None, "nothing is written");
    };
    refused(applied_signal("Main.PV", "%IL0", "LREAL"));
    refused(applied_signal("Main.Output", "%QL0", "LREAL"));
    refused(applied_signal("Main.Text", "%IW3", "DINT"));
    refused(applied_signal("Main.Memory", "%MX0.0", "BOOL"));
}

#[test]
fn multiple_channels_are_refused_by_name_with_the_parked_cause() {
    // configureLoopChannels.ts's multiple channels need the channel model, parked for V7.
    let mut answers = single_answers();
    answers.channel_mode = Some(ChannelMode::Multiple);
    let mut loop_definition = base_loop();
    let failure =
        apply_channels(&mut loop_definition, &answers).expect_err("multiple channels are parked");
    // A11: the message is the first sentence; the detail keeps the whole text.
    assert_eq!(
        failure
            .detail
            .as_deref()
            .unwrap_or(failure.message.as_str()),
        texts::configure::multiple_channels_not_available()
    );
    assert_eq!(failure.cause, Cause::ControllerCapability);
    assert_eq!(failure.next_action, NextAction::None);
}

#[test]
fn an_on_off_actuator_and_a_boolean_output_are_refused_by_name() {
    // configureLoopChannels.ts `actuatorSetup`: the ON/OFF capability has dwell times in the channel model (parked for V7),
    // and a BOOL manipulated variable forces it.
    let mut answers = single_answers();
    answers.actuator = Some(ActuatorChoice::OnOff);
    let mut loop_definition = base_loop();
    let failure =
        apply_channels(&mut loop_definition, &answers).expect_err("an ON/OFF actuator is parked");
    // A11: the message is the first sentence; the detail keeps the whole text.
    assert_eq!(
        failure
            .detail
            .as_deref()
            .unwrap_or(failure.message.as_str()),
        texts::configure::on_off_actuator_not_available()
    );

    let mut loop_definition = base_loop();
    loop_definition.signals.manipulated_variable.iec_type = "BOOL".into();
    let failure = apply_channels(&mut loop_definition, &single_answers())
        .expect_err("a BOOL output is ON/OFF");
    // A11: the message is the first sentence; the detail keeps the whole text.
    assert_eq!(
        failure
            .detail
            .as_deref()
            .unwrap_or(failure.message.as_str()),
        texts::configure::on_off_actuator_not_available()
    );
}

#[test]
fn a_missing_channel_answer_is_named_and_never_defaulted() {
    // configureLoop.ts asks every question of the channel step; a missing answer names its question (A11.1).
    let mut loop_definition = base_loop();
    let mut answers = single_answers();
    answers.channel_mode = None;
    let failure = apply_channels(&mut loop_definition, &answers).expect_err("the mode is missing");
    assert_eq!(
        failure.message,
        texts::configure::unanswered("Control channels")
    );

    let mut answers = single_answers();
    answers.feedback = None;
    let failure =
        apply_channels(&mut loop_definition, &answers).expect_err("the feedback is missing");
    assert_eq!(
        failure.message,
        texts::configure::unanswered("Main.Output: applied actuator feedback")
    );
}
