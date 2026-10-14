//! Tests of the loop the step-by-step setup builds (`configureLoop.ts`; `control-studio-command.test.ts`'s step-by-step cases).
#![allow(clippy::float_cmp)]

use super::*;
use crate::limits::{record_period_executions, RECORDING_SAMPLE_LIMIT};
use crate::loop_definition::{
    ControllerLaw, Design, Direction, Execution, ExperimentKind, ManualOutputType, ModeBinding,
    ModeWhenTrue, Parameter, ParameterBinding, RealType, Topology,
};
use crate::setup::catalog::{
    CatalogAnswer, CompiledCatalog, CompiledController, CompiledSymbol, CompiledTask,
};
use crate::setup::prompts::{
    ActuatorChoice, ChannelMode, ControllerChoice, FeedbackChoice, RateAnswer, SetpointChoice,
};

fn scope() -> LoopScope {
    LoopScope {
        project_id: "project:test".into(),
        runtime_id: "runtime:guided-proportional".into(),
    }
}

fn signal(symbol: &str, address: &str, iec_type: &str) -> CompiledSignal {
    CompiledSignal {
        symbol: symbol.into(),
        address: address.into(),
        iec_type: iec_type.into(),
    }
}

fn symbol(name: &str, iec_type: &str) -> CompiledSymbol {
    CompiledSymbol {
        symbol: name.into(),
        iec_type: iec_type.into(),
        address: None,
    }
}

fn controller(instance: &str, type_name: &str, interval_ms: f64) -> CompiledController {
    CompiledController {
        instance: instance.into(),
        type_name: type_name.into(),
        tasks: vec![CompiledTask {
            name: "MainTask".into(),
            interval_ms,
        }],
    }
}

fn fixture(
    controllers: Vec<CompiledController>,
    symbols: Vec<CompiledSymbol>,
    signals: Vec<CompiledSignal>,
) -> CatalogAnswer {
    CatalogAnswer {
        control: CompiledCatalog {
            symbols,
            controllers,
            links: Vec::new(),
            writes: Vec::new(),
        },
        signals,
    }
}

/// The custom open loop's program: a pump, its command and its feedback (`control-studio-command.test.ts`).
fn pump_catalog() -> CatalogAnswer {
    fixture(
        vec![CompiledController {
            instance: "PumpStation.Regulator".into(),
            type_name: "MyPumpLaw".into(),
            tasks: vec![CompiledTask {
                name: "PumpTask".into(),
                interval_ms: 20.0,
            }],
        }],
        Vec::new(),
        vec![
            signal("PumpStation.Power", "%QL0", "LREAL"),
            signal("PumpStation.Pressure", "%IL0", "LREAL"),
        ],
    )
}

/// The existing proportional controller's program, with its task at `interval_ms`
/// (`control-studio-command.test.ts`, "guided proportional setup").
fn proportional_catalog(interval_ms: f64) -> CatalogAnswer {
    fixture(
        vec![controller(
            "Main.Controller",
            "ExistingProportional",
            interval_ms,
        )],
        vec![
            symbol("Main.Kp", "LREAL"),
            symbol("Main.ManualCommand", "LREAL"),
            symbol("Main.Manual", "BOOL"),
        ],
        vec![
            signal("Main.PV", "%IL0", "LREAL"),
            signal("Main.Output", "%QL0", "LREAL"),
            signal("Main.SP", "%ML0", "LREAL"),
            signal("Main.ManualCommand", "%ML8", "LREAL"),
        ],
    )
}

fn named<'a>(catalog: &'a CatalogAnswer, symbol: &str) -> &'a CompiledSignal {
    catalog
        .signals
        .iter()
        .find(|signal| signal.symbol == symbol)
        .expect("a signal of the fixture")
}

/// The channel answers of a single channel with a continuous output, no limits and the output as its own feedback.
fn single_channel() -> SetupStepAnswers {
    SetupStepAnswers {
        channel_mode: Some(ChannelMode::Single),
        actuator: Some(ActuatorChoice::Continuous),
        rising_rate: Some(RateAnswer::Unlimited),
        falling_rate: Some(RateAnswer::Unlimited),
        feedback: Some(FeedbackChoice::UseOutput),
        ..SetupStepAnswers::default()
    }
}

/// The answers of an observation of the pump's program, with a duration in seconds.
fn pump_answers(name: &str, duration_seconds: f64) -> SetupStepAnswers {
    let catalog = pump_catalog();
    SetupStepAnswers {
        name: Some(name.into()),
        controller: Some(ControllerChoice::Compiled(
            catalog.control.controllers[0].clone(),
        )),
        process_variable: Some(named(&catalog, "PumpStation.Pressure").clone()),
        controller_output: Some(named(&catalog, "PumpStation.Power").clone()),
        setpoint: Some(SetpointChoice::OpenLoop),
        workflow: Some(SetupWorkflow::Observation),
        process_unit: Some("bar".into()),
        process_minimum: Some(0.0),
        process_maximum: Some(12.0),
        output_unit: Some("%".into()),
        output_minimum: Some(0.0),
        output_maximum: Some(100.0),
        observation_duration_seconds: Some(duration_seconds),
        ..single_channel()
    }
}

/// The answers of the existing proportional controller's setup, with the setpoint `Main.SP`.
fn proportional_answers(catalog: &CatalogAnswer, name: &str) -> SetupStepAnswers {
    SetupStepAnswers {
        name: Some(name.into()),
        controller: Some(ControllerChoice::Compiled(
            catalog.control.controllers[0].clone(),
        )),
        process_variable: Some(named(catalog, "Main.PV").clone()),
        controller_output: Some(named(catalog, "Main.Output").clone()),
        setpoint: Some(SetpointChoice::Signal(named(catalog, "Main.SP").clone())),
        workflow: Some(SetupWorkflow::Proportional),
        process_unit: Some("bar".into()),
        process_minimum: Some(0.0),
        process_maximum: Some(12.0),
        output_unit: Some("%".into()),
        output_minimum: Some(0.0),
        output_maximum: Some(100.0),
        gain: Some(symbol("Main.Kp", "LREAL")),
        gain_minimum: Some(0.0),
        gain_maximum: Some(25.0),
        manual_command: Some(symbol("Main.ManualCommand", "LREAL")),
        manual_mode: Some(symbol("Main.Manual", "BOOL")),
        manual_when_true: Some(true),
        ..single_channel()
    }
}

#[test]
fn the_custom_open_loop_is_built_from_its_answers_and_keeps_its_controller_and_period() {
    // control-studio-command.test.ts: "configures a compiled custom open loop step by step": the compiled controller
    // identity, the sole 20 ms task, the process variable, the open topology, the read output and the observation of 30 s.
    let loop_definition = configure_loop(
        &pump_catalog(),
        &scope(),
        &pump_answers("Custom pump", 30.0),
    )
    .expect("the custom open loop is built");
    assert_eq!(loop_definition.schema_version, 2);
    assert_eq!(loop_definition.id, "loop:custom-pump");
    assert_eq!(loop_definition.controller.type_name, "MyPumpLaw");
    assert_eq!(loop_definition.controller.task_period_ms, 20.0);
    assert_eq!(loop_definition.controller.design, Some(Design::Project));
    assert_eq!(
        loop_definition.signals.process_variable.symbol,
        "PumpStation.Pressure"
    );
    assert_eq!(loop_definition.topology, Some(Topology::OpenLoop));
    assert_eq!(loop_definition.signals.setpoint, None);
    assert_eq!(
        loop_definition.signals.manipulated_variable.direction,
        Direction::Read
    );
    let experiment = loop_definition
        .experiment
        .expect("the observation has its experiment");
    assert_eq!(experiment.execution, Execution::ProjectScripted);
    assert_eq!(experiment.kind, ExperimentKind::Observation);
    assert_eq!(experiment.amplitude, 0.0);
    assert_eq!(experiment.duration_seconds, Some(30.0));
    assert_eq!(experiment.seed, 1);
    assert!(loop_definition.revision.starts_with("sha256:"));
    assert_ne!(loop_definition.revision, "sha256:pending");
}

#[test]
fn a_custom_controller_takes_its_typed_identity_and_period_and_a_loop_name_is_trimmed() {
    // configureLoop.ts: without a compiled controller the instance, the type and the period are typed; the name is trimmed
    // and its id is the slug of the name.
    let catalog = fixture(
        Vec::new(),
        Vec::new(),
        vec![
            signal("Pump.Pressure", "%IL0", "LREAL"),
            signal("Pump.Power", "%QL0", "LREAL"),
        ],
    );
    let mut answers = pump_answers("  Custom Pump!  ", 30.0);
    answers.controller = None;
    answers.custom_instance = Some(" Pump ".into());
    answers.custom_type = Some("Law".into());
    answers.task_period_ms = Some(10.0);
    answers.process_variable = Some(named(&catalog, "Pump.Pressure").clone());
    answers.controller_output = Some(named(&catalog, "Pump.Power").clone());
    let loop_definition =
        configure_loop(&catalog, &scope(), &answers).expect("the custom controller is built");
    assert_eq!(loop_definition.name, "Custom Pump!");
    assert_eq!(loop_definition.id, "loop:custom-pump");
    assert_eq!(loop_definition.controller.instance, "Pump");
    assert_eq!(loop_definition.controller.type_name, "Law");
    assert_eq!(loop_definition.controller.task_period_ms, 10.0);
}

#[test]
fn a_proportional_setup_declares_the_existing_law_its_gain_its_mode_and_its_manual_command() {
    // control-studio-command.test.ts: "guided proportional setup saves a declared existing law without inventing a plant".
    let catalog = proportional_catalog(20.0);
    let answers = SetupStepAnswers {
        channel_mode: Some(ChannelMode::Single),
        ..proportional_answers(&catalog, "Existing proportional")
    };
    let loop_definition =
        configure_loop(&catalog, &scope(), &answers).expect("the proportional loop is built");
    assert_eq!(loop_definition.id, "loop:existing-proportional");
    assert_eq!(
        loop_definition.controller.law,
        Some(ControllerLaw::Proportional {
            gain_symbol: "Main.Kp".into(),
        })
    );
    assert_eq!(
        loop_definition.controller.manual_output_signal.as_deref(),
        Some("Main.ManualCommand")
    );
    assert_eq!(
        loop_definition.controller.manual_output_address.as_deref(),
        Some("%ML8"),
        "the compiled Manual-command address is kept for the bounded test"
    );
    assert_eq!(
        loop_definition.controller.manual_output_type,
        Some(ManualOutputType::Lreal)
    );
    assert_eq!(
        loop_definition.controller.mode_signal.as_deref(),
        Some("Main.Manual")
    );
    assert_eq!(
        loop_definition.controller.mode_when_true,
        Some(ModeWhenTrue::Manual)
    );
    assert_eq!(
        loop_definition.parameter_binding,
        Some(ParameterBinding {
            loop_id: "loop:existing-proportional".into(),
            task_name: "MainTask".into(),
            parameters: vec![Parameter {
                symbol: "Main.Kp".into(),
                iec_type: RealType::Lreal,
                unit: "%/bar".into(),
                convention: "proportional gain".into(),
                minimum: 0.0,
                maximum: 25.0,
            }],
            mode: ModeBinding {
                symbol: "Main.Manual".into(),
                manual_value: true,
            },
        })
    );
    assert_eq!(
        loop_definition.signals.manipulated_variable.direction,
        Direction::Write
    );
    assert_eq!(
        loop_definition.experiment, None,
        "setup records no observation duration the engineer did not set"
    );
    assert_eq!(loop_definition.topology, Some(Topology::ClosedLoop));
}

#[test]
fn a_proportional_setup_writes_no_duration_and_the_record_period_bounds_a_long_identification() {
    // control-studio-command.test.ts: "guided proportional setup asks for no identification duration, so the record period
    // bounds a long identification (B1.3, A4.2)": a 50 ms task, no experiment, and a 600 s record at 50 ms keeps one
    // execution per sample (12 000 scans fit the 100 000 samples).
    let catalog = proportional_catalog(50.0);
    let answers = SetupStepAnswers {
        channel_mode: Some(ChannelMode::Single),
        ..proportional_answers(&catalog, "Sample budget proportional")
    };
    let loop_definition =
        configure_loop(&catalog, &scope(), &answers).expect("the proportional loop is built");
    assert_eq!(
        loop_definition.experiment, None,
        "Identify chooses the duration within Limits (B1.3)"
    );
    assert_eq!(
        record_period_executions(600_000, 50, RECORDING_SAMPLE_LIMIT, None),
        1
    );
}

#[test]
fn an_observation_accepts_the_24_hour_maximum_at_a_700_ms_task() {
    // control-studio-command.test.ts: "guided observation setup accepts the 24 h maximum at a 700 ms task" (A7, A4.2).
    let catalog = proportional_catalog(700.0);
    let mut answers = proportional_answers(&catalog, "Scan rounded observation");
    answers.workflow = Some(SetupWorkflow::Observation);
    answers.gain = None;
    answers.gain_minimum = None;
    answers.gain_maximum = None;
    answers.manual_command = None;
    answers.manual_mode = None;
    answers.manual_when_true = None;
    answers.observation_duration_seconds = Some(86_400.0);
    let loop_definition =
        configure_loop(&catalog, &scope(), &answers).expect("the observation is built");
    assert_eq!(loop_definition.controller.task_period_ms, 700.0);
    assert_eq!(
        loop_definition
            .experiment
            .expect("an experiment")
            .duration_seconds,
        Some(86_400.0)
    );
    assert_eq!(
        loop_definition.topology,
        Some(Topology::ClosedLoop),
        "a setpoint closes the loop"
    );
}

#[test]
fn an_observation_accepts_one_hour_at_a_50_ms_task() {
    // control-studio-command.test.ts: "guided observation setup accepts a one-hour observation within the 24 h limit" (A7).
    let catalog = proportional_catalog(50.0);
    let mut answers = proportional_answers(&catalog, "One hour observation");
    answers.workflow = Some(SetupWorkflow::Observation);
    answers.gain = None;
    answers.gain_minimum = None;
    answers.gain_maximum = None;
    answers.manual_command = None;
    answers.manual_mode = None;
    answers.manual_when_true = None;
    answers.observation_duration_seconds = Some(3_600.0);
    let loop_definition =
        configure_loop(&catalog, &scope(), &answers).expect("the observation is built");
    assert_eq!(
        loop_definition
            .experiment
            .expect("an experiment")
            .duration_seconds,
        Some(3_600.0)
    );
}

#[test]
fn an_unfinished_setup_writes_no_loop_and_names_its_next_question() {
    // control-studio-command.test.ts: "cancelling the proportional workflow writes no loop file": a setup without its
    // workflow answer builds nothing; the refusal names the question.
    let catalog = proportional_catalog(20.0);
    let answers = SetupStepAnswers {
        workflow: None,
        ..proportional_answers(&catalog, "Cancelled proportional")
    };
    let failure =
        configure_loop(&catalog, &scope(), &answers).expect_err("no loop without the workflow");
    assert_eq!(
        failure.message,
        texts::configure::unanswered("Control workflow")
    );
    assert_eq!(failure.cause, Cause::Settings);
    assert_eq!(failure.next_action, NextAction::OpenLoopSetup);
}

#[test]
fn an_output_or_a_setpoint_at_the_process_variable_s_address_is_refused_by_name() {
    // configureLoop.ts: the output list excludes the process variable's address; the setpoint list excludes both.
    let catalog = proportional_catalog(20.0);
    let mut answers = pump_answers("Clash", 30.0);
    answers.process_variable = Some(named(&catalog, "Main.PV").clone());
    answers.controller_output = Some(named(&catalog, "Main.PV").clone());
    let failure = configure_loop(&pump_catalog(), &scope(), &answers)
        .expect_err("the output repeats the process variable");
    assert_eq!(
        failure.message,
        texts::configure::output_repeats_process("Main.PV")
    );

    let mut answers = pump_answers("Clash", 30.0);
    answers.setpoint = Some(SetpointChoice::Signal(
        named(&pump_catalog(), "PumpStation.Power").clone(),
    ));
    let failure = configure_loop(&pump_catalog(), &scope(), &answers)
        .expect_err("the setpoint repeats the output");
    assert_eq!(
        failure.message,
        texts::configure::setpoint_repeats("PumpStation.Power")
    );
}

#[test]
fn a_proportional_workflow_needs_a_setpoint() {
    // configureLoop.ts `selectWorkflow`: the proportional workflow is offered only with a setpoint.
    let catalog = proportional_catalog(20.0);
    let answers = SetupStepAnswers {
        setpoint: Some(SetpointChoice::OpenLoop),
        ..proportional_answers(&catalog, "No setpoint")
    };
    let failure =
        configure_loop(&catalog, &scope(), &answers).expect_err("the workflow needs a setpoint");
    assert_eq!(
        failure.message,
        texts::configure::workflow_needs_setpoint("Tune existing proportional controller")
    );
}

#[test]
fn a_process_maximum_must_be_above_its_minimum_and_a_duration_must_be_one_second_to_a_day() {
    // configureLoop.ts: the process maximum is `value > minimum`; the observation duration is 1 s to 24 h (A7).
    let mut answers = pump_answers("Bounds", 30.0);
    answers.process_maximum = Some(0.0);
    let failure = configure_loop(&pump_catalog(), &scope(), &answers)
        .expect_err("the maximum equals the minimum");
    assert_eq!(failure.message, "Enter a valid finite number.");

    for seconds in [0.5, 86_400.5] {
        let answers = pump_answers("Duration", seconds);
        let failure = configure_loop(&pump_catalog(), &scope(), &answers)
            .expect_err("out of the 1 s to 24 h range");
        assert!(
            failure.message.contains("needs an identification duration"),
            "{}",
            failure.message
        );
    }
}

#[test]
fn a_catalog_without_a_process_image_signal_builds_nothing() {
    // configureLoop.ts: "No compiled process-image signals are available. Symbolic-only setup is not available yet."
    let empty = fixture(vec![controller("P", "Law", 20.0)], Vec::new(), Vec::new());
    let failure =
        configure_loop(&empty, &scope(), &pump_answers("Empty", 30.0)).expect_err("no signals");
    // A11: the message is the first sentence; the detail keeps the whole text.
    assert_eq!(
        failure
            .detail
            .as_deref()
            .unwrap_or(failure.message.as_str()),
        texts::configure::no_process_signals()
    );
    assert_eq!(failure.cause, Cause::Implementation);
}

#[test]
fn the_on_off_workflow_and_multiple_channels_are_refused_by_name() {
    // configureOnOffController.ts is parked for V7 (refused here by name); multiple channels need the channel model (V7).
    let catalog = proportional_catalog(20.0);
    let mut answers = proportional_answers(&catalog, "Parked");
    answers.workflow = Some(SetupWorkflow::OnOff);
    let failure =
        configure_loop(&catalog, &scope(), &answers).expect_err("the ON/OFF workflow is parked");
    // A11: the message is the first sentence; the detail keeps the whole text.
    assert_eq!(
        failure
            .detail
            .as_deref()
            .unwrap_or(failure.message.as_str()),
        texts::configure::on_off_controller_not_available()
    );
    assert_eq!(failure.cause, Cause::ControllerCapability);
    assert_eq!(failure.next_action, NextAction::None);

    let mut answers = pump_answers("Several", 30.0);
    answers.channel_mode = Some(ChannelMode::Multiple);
    let failure = configure_loop(&pump_catalog(), &scope(), &answers)
        .expect_err("multiple channels are parked");
    // A11: the message is the first sentence; the detail keeps the whole text.
    assert_eq!(
        failure
            .detail
            .as_deref()
            .unwrap_or(failure.message.as_str()),
        texts::configure::multiple_channels_not_available()
    );
}

#[test]
fn a_control_pid_with_a_setpoint_is_the_pid_design_and_any_other_controller_is_the_project_s() {
    // configureLoop.ts: `design` is `pid` for a `Control.PID` with a setpoint, and `project` otherwise.
    let pid = fixture(
        vec![controller("P1.Controller", "Control.PID", 100.0)],
        Vec::new(),
        vec![
            signal("P1.ProcessValue", "%IL0", "LREAL"),
            signal("P1.ManipulatedValue", "%QL0", "LREAL"),
            signal("P1.Setpoint", "%ML0", "LREAL"),
        ],
    );
    let mut answers = pump_answers("Heater", 30.0);
    answers.controller = Some(ControllerChoice::Compiled(
        pid.control.controllers[0].clone(),
    ));
    answers.process_variable = Some(named(&pid, "P1.ProcessValue").clone());
    answers.controller_output = Some(named(&pid, "P1.ManipulatedValue").clone());
    answers.setpoint = Some(SetpointChoice::Signal(named(&pid, "P1.Setpoint").clone()));
    let loop_definition =
        configure_loop(&pid, &scope(), &answers).expect("the PID observation is built");
    assert_eq!(loop_definition.controller.design, Some(Design::Pid));
    assert_eq!(loop_definition.topology, Some(Topology::ClosedLoop));
}

#[test]
fn the_save_sentence_names_the_loop_its_signals_and_its_period_and_what_save_writes() {
    // configureControlLoop's confirmation (the TypeScript sentence, word for word).
    let catalog = proportional_catalog(20.0);
    let answers = SetupStepAnswers {
        channel_mode: Some(ChannelMode::Single),
        ..proportional_answers(&catalog, "Existing proportional")
    };
    let loop_definition =
        configure_loop(&catalog, &scope(), &answers).expect("the proportional loop is built");
    assert_eq!(
        texts::configure::save_summary(&loop_definition, SetupWorkflow::Proportional),
        "Save Existing proportional for runtime:guided-proportional? PV: Main.PV (bar); output: Main.Output (%); 20 ms. \
         This declares the existing proportional controller; it does not write the runtime or edit ST."
    );
}
