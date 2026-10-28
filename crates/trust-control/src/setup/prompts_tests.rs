//! Tests of the step-by-step questions (`configureLoop.ts`, `configureLoopChannels.ts`, `setupPromptTypes.ts`).
#![allow(clippy::float_cmp)]

use super::*;
use crate::limits::{MAXIMUM_EXPERIMENT_DURATION_MS, MINIMUM_EXPERIMENT_DURATION_MS};
use crate::setup::catalog::{CompiledCatalog, CompiledController, CompiledSymbol, CompiledTask};

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

fn controller(instance: &str, type_name: &str, tasks: &[(&str, f64)]) -> CompiledController {
    CompiledController {
        instance: instance.into(),
        type_name: type_name.into(),
        tasks: tasks
            .iter()
            .map(|(name, interval_ms)| CompiledTask {
                name: (*name).into(),
                interval_ms: *interval_ms,
            })
            .collect(),
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

/// The program of the custom open loop of `control-studio-command.test.ts` ("configures a compiled custom open loop step
/// by step"): one controller with one 20 ms task, a command and a feedback.
fn pump_catalog() -> CatalogAnswer {
    fixture(
        vec![controller(
            "PumpStation.Regulator",
            "MyPumpLaw",
            &[("PumpTask", 20.0)],
        )],
        Vec::new(),
        vec![
            signal("PumpStation.Power", "%QL0", "LREAL"),
            signal("PumpStation.Pressure", "%IL0", "LREAL"),
        ],
    )
}

/// The program of the existing proportional controller of `control-studio-command.test.ts` ("guided proportional setup").
fn proportional_catalog() -> CatalogAnswer {
    fixture(
        vec![controller(
            "Main.Controller",
            "ExistingProportional",
            &[("MainTask", 20.0)],
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

/// The engineer's picks in one scripted run of the questions.
#[derive(Clone)]
struct Canned {
    name: &'static str,
    controller: ControllerChoice,
    process: &'static str,
    output: &'static str,
    setpoint: SetpointChoice,
    workflow: SetupWorkflow,
    duration: Option<f64>,
    gain: Option<CompiledSymbol>,
    manual_command: Option<CompiledSymbol>,
    manual_mode: Option<CompiledSymbol>,
    channels: ChannelMode,
}

fn named<'a>(catalog: &'a CatalogAnswer, symbol: &str) -> &'a CompiledSignal {
    catalog
        .signals
        .iter()
        .find(|signal| signal.symbol == symbol)
        .expect("a signal of the fixture")
}

fn answer_next(
    catalog: &CatalogAnswer,
    canned: &Canned,
    answers: &mut SetupStepAnswers,
    step: &SetupStep,
) {
    match step.step {
        StepId::LoopName => answers.name = Some(canned.name.into()),
        StepId::Controller => answers.controller = Some(canned.controller.clone()),
        StepId::TaskPeriod => answers.task_period_ms = Some(20.0),
        StepId::ProcessVariable => {
            answers.process_variable = Some(named(catalog, canned.process).clone());
        }
        StepId::ControllerOutput => {
            answers.controller_output = Some(named(catalog, canned.output).clone());
        }
        StepId::Setpoint => answers.setpoint = Some(canned.setpoint.clone()),
        StepId::Workflow => answers.workflow = Some(canned.workflow),
        StepId::ProcessUnit => answers.process_unit = Some("bar".into()),
        StepId::ProcessMinimum => answers.process_minimum = Some(0.0),
        StepId::ProcessMaximum => answers.process_maximum = Some(12.0),
        StepId::OutputUnit => answers.output_unit = Some("%".into()),
        StepId::OutputMinimum => answers.output_minimum = Some(0.0),
        StepId::OutputMaximum => answers.output_maximum = Some(100.0),
        StepId::ProportionalGain => answers.gain = canned.gain.clone(),
        StepId::GainMinimum => answers.gain_minimum = Some(0.0),
        StepId::GainMaximum => answers.gain_maximum = Some(25.0),
        StepId::ManualCommand => answers.manual_command = canned.manual_command.clone(),
        StepId::ManualMode => answers.manual_mode = canned.manual_mode.clone(),
        StepId::ManualModePolarity => answers.manual_when_true = Some(true),
        StepId::ControllerTask => answers.controller_task = Some("MainTask".into()),
        StepId::ObservationDuration => answers.observation_duration_seconds = canned.duration,
        StepId::AdapterFile => answers.adapter_mapping = Some(b"{}".to_vec()),
        StepId::ChannelMode => answers.channel_mode = Some(canned.channels),
        StepId::ActuatorCapability => answers.actuator = Some(ActuatorChoice::Continuous),
        StepId::RisingRate => answers.rising_rate = Some(RateAnswer::Unlimited),
        StepId::FallingRate => answers.falling_rate = Some(RateAnswer::Unlimited),
        StepId::AppliedFeedback => answers.feedback = Some(FeedbackChoice::UseOutput),
        StepId::ControllerInstance | StepId::ControllerType => {
            panic!("the fixture controller is compiled")
        }
    }
}

/// Answers every question in order until none is left, as the presenter does, and returns the questions asked.
fn walk(catalog: &CatalogAnswer, canned: &Canned) -> Vec<StepId> {
    let mut answers = SetupStepAnswers::default();
    let mut asked = Vec::new();
    while let Some(step) = next_step(catalog, &answers) {
        asked.push(step.step);
        answer_next(catalog, canned, &mut answers, &step);
        assert!(asked.len() < 64, "the questions must end");
    }
    asked
}

fn pump_canned() -> Canned {
    Canned {
        name: "Custom pump",
        controller: ControllerChoice::Compiled(pump_catalog().control.controllers[0].clone()),
        process: "PumpStation.Pressure",
        output: "PumpStation.Power",
        setpoint: SetpointChoice::OpenLoop,
        workflow: SetupWorkflow::Observation,
        duration: Some(30.0),
        gain: None,
        manual_command: None,
        manual_mode: None,
        channels: ChannelMode::Single,
    }
}

fn proportional_canned() -> Canned {
    let catalog = proportional_catalog();
    Canned {
        name: "Existing proportional",
        controller: ControllerChoice::Compiled(catalog.control.controllers[0].clone()),
        process: "Main.PV",
        output: "Main.Output",
        setpoint: SetpointChoice::Signal(named(&catalog, "Main.SP").clone()),
        workflow: SetupWorkflow::Proportional,
        duration: None,
        gain: Some(symbol("Main.Kp", "LREAL")),
        manual_command: Some(symbol("Main.ManualCommand", "LREAL")),
        manual_mode: Some(symbol("Main.Manual", "BOOL")),
        channels: ChannelMode::Single,
    }
}

#[test]
fn the_duration_rule_is_the_one_to_twenty_four_hour_limit_of_a7() {
    // limits.rs: 1 s to 24 h. The rule is written in seconds, so its bounds are checked against the limits here.
    assert_eq!(MINIMUM_EXPERIMENT_DURATION_MS, 1_000);
    assert_eq!(MAXIMUM_EXPERIMENT_DURATION_MS, 86_400_000);
    assert!(DURATION_RULE.accepts(1.0));
    assert!(!DURATION_RULE.accepts(0.999));
    assert!(
        DURATION_RULE.accepts(86_400.0),
        "the 24 h maximum is inside"
    );
    assert!(!DURATION_RULE.accepts(86_400.001));
    assert!(!DURATION_RULE.accepts(f64::NAN));
}

#[test]
fn an_observation_of_one_hour_and_of_the_maximum_are_accepted() {
    // control-studio-command.test.ts: "guided observation setup accepts the 24 h maximum at a 700 ms task" (the validator
    // accepts "86400") and "accepts a one-hour observation within the 24 h limit" ("3600"). A7.
    assert_eq!(accept_number("86400", DURATION_RULE), Ok(86_400.0));
    assert_eq!(accept_number("3600", DURATION_RULE), Ok(3_600.0));
    assert_eq!(
        accept_number("86400.5", DURATION_RULE),
        Err("Enter a valid finite number.".to_owned())
    );
}

#[test]
fn a_number_answer_is_read_as_javascript_reads_it() {
    // configureLoop.ts `number`: a blank check, then `Number(text)`.
    assert_eq!(js_number("  1e3 "), Some(1_000.0));
    assert_eq!(js_number("0x10"), Some(16.0));
    assert_eq!(js_number("0b101"), Some(5.0));
    assert_eq!(js_number("0o17"), Some(15.0));
    assert_eq!(js_number("1."), Some(1.0));
    assert_eq!(js_number(".5"), Some(0.5));
    assert_eq!(
        js_number("-0x10"),
        None,
        "a sign is not allowed with a radix"
    );
    assert_eq!(js_number("0x"), None);
    assert_eq!(js_number("Infinity"), None);
    assert_eq!(js_number("1e400"), None, "an overflow is not finite");
    assert_eq!(js_number(""), None);
    assert_eq!(js_number("12px"), None);
    assert_eq!(js_number("1_000"), None);
}

#[test]
fn a_text_answer_is_trimmed_and_a_blank_one_is_refused() {
    // configureLoop.ts `input`: `value.trim() ? undefined : "Enter a value."`; the trimmed value is returned.
    assert_eq!(accept_text("  Custom pump  "), Ok("Custom pump".to_owned()));
    assert_eq!(
        accept_text("\u{feff} \t\u{3000}"),
        Err("Enter a value.".to_owned())
    );
    assert_eq!(accept_text(""), Err("Enter a value.".to_owned()));
}

#[test]
fn the_phases_are_numbered_one_to_four_and_carry_their_names() {
    // setupPromptTypes.ts `SETUP_PHASE_NAMES`.
    assert_eq!(SetupPhase::LoopAndSignals.number(), 1);
    assert_eq!(SetupPhase::ChannelsAndActuators.number(), 4);
    assert_eq!(
        SetupPhase::ControllerAndExperiment.name(),
        "Controller and experiment"
    );
    assert_eq!(SetupPhase::UnitsAndBounds.name(), "Units and bounds");
}

#[test]
fn the_workflows_depend_on_whether_there_is_a_setpoint() {
    // configureLoop.ts `selectWorkflow`: observation always; the other three only with a setpoint, in this order.
    assert_eq!(workflows_offered(false), vec![SetupWorkflow::Observation]);
    assert_eq!(
        workflows_offered(true),
        vec![
            SetupWorkflow::Observation,
            SetupWorkflow::Adapter,
            SetupWorkflow::OnOff,
            SetupWorkflow::Proportional,
        ]
    );
}

#[test]
fn the_controller_picker_omits_a_type_that_equals_the_instance() {
    // control-studio-command.test.ts: "keeps compiled controller picker entries legible when instance and type share a
    // name": an equal instance and type omit the description; the PID's type stays visible.
    assert_eq!(controller_item_description("P1", "P1"), None);
    assert_eq!(
        controller_item_description("P1.Controller", "Control.PID"),
        Some("Control.PID".to_owned())
    );
    let catalog = fixture(
        vec![
            controller("P1", "P1", &[]),
            controller("P1.Controller", "Control.PID", &[("MainTask", 100.0)]),
        ],
        Vec::new(),
        vec![signal("P1.ManipulatedValue", "%QL0", "LREAL")],
    );
    let answers = SetupStepAnswers {
        name: Some("Controller labels".into()),
        ..SetupStepAnswers::default()
    };
    let step = next_step(&catalog, &answers).expect("the controller is asked");
    assert_eq!(step.step, StepId::Controller);
    let StepKind::Choice(items) = step.kind else {
        panic!("the controller is a choice")
    };
    assert_eq!(items[0].label, "P1");
    assert_eq!(items[0].description, None);
    assert_eq!(items[1].description.as_deref(), Some("Control.PID"));
    assert_eq!(items[2].label, "Enter a custom controller");
    assert_eq!(items[2].index, 2);
}

#[test]
fn a_custom_controller_asks_its_identity_and_then_its_period_when_the_catalog_has_none() {
    // configureLoop.ts: without compiled controllers the instance, the type and the task period are typed in, in order.
    let catalog = fixture(Vec::new(), Vec::new(), vec![signal("A", "%IL0", "LREAL")]);
    let mut answers = SetupStepAnswers {
        name: Some("Loop".into()),
        ..SetupStepAnswers::default()
    };
    assert_eq!(
        next_step(&catalog, &answers).map(|step| step.step),
        Some(StepId::ControllerInstance)
    );
    answers.custom_instance = Some("Pump".into());
    assert_eq!(
        next_step(&catalog, &answers).map(|step| step.step),
        Some(StepId::ControllerType)
    );
    answers.custom_type = Some("Law".into());
    assert_eq!(
        next_step(&catalog, &answers).map(|step| step.step),
        Some(StepId::TaskPeriod)
    );
}

#[test]
fn the_single_task_of_a_controller_must_have_a_positive_interval() {
    // configureLoop.ts `task`: exactly one task, with an interval above zero.
    let one = ControllerChoice::Compiled(controller("C", "T", &[("MainTask", 20.0)]));
    let zero = ControllerChoice::Compiled(controller("C", "T", &[("MainTask", 0.0)]));
    let two = ControllerChoice::Compiled(controller("C", "T", &[("A", 20.0), ("B", 20.0)]));
    let none = ControllerChoice::Compiled(controller("C", "T", &[]));
    assert_eq!(
        sole_task(Some(&one)).map(|task| task.name.as_str()),
        Some("MainTask")
    );
    assert!(sole_task(Some(&zero)).is_none());
    assert!(sole_task(Some(&two)).is_none());
    assert!(sole_task(Some(&none)).is_none());
    assert!(sole_task(Some(&ControllerChoice::Custom)).is_none());
    assert!(sole_task(None).is_none());
}

#[test]
fn the_custom_open_loop_asks_no_identity_and_no_period_when_the_controller_has_one_task() {
    // control-studio-command.test.ts: "configures a compiled custom open loop step by step". The controller identity and
    // its sole periodic task are chosen, not retyped, so "Controller instance", "Controller type" and "Task period (ms)"
    // are never asked. The observation asks its duration (C12) and the channel questions (phase 4).
    let asked = walk(&pump_catalog(), &pump_canned());
    assert_eq!(
        asked,
        vec![
            StepId::LoopName,
            StepId::Controller,
            StepId::ProcessVariable,
            StepId::ControllerOutput,
            StepId::Setpoint,
            StepId::Workflow,
            StepId::ProcessUnit,
            StepId::ProcessMinimum,
            StepId::ProcessMaximum,
            StepId::OutputUnit,
            StepId::OutputMinimum,
            StepId::OutputMaximum,
            StepId::ObservationDuration,
            StepId::ChannelMode,
            StepId::ActuatorCapability,
            StepId::RisingRate,
            StepId::FallingRate,
            StepId::AppliedFeedback,
        ]
    );
    assert!(!asked.contains(&StepId::ControllerInstance));
    assert!(!asked.contains(&StepId::ControllerType));
    assert!(!asked.contains(&StepId::TaskPeriod));
}

#[test]
fn the_proportional_route_asks_gain_manual_command_and_polarity_but_no_duration() {
    // control-studio-command.test.ts: "guided proportional setup saves a declared existing law" and "asks for no
    // identification duration" (B1.3): the proportional questions follow phase 2; the duration is never asked; the
    // controller's sole task is not asked again.
    let asked = walk(&proportional_catalog(), &proportional_canned());
    assert_eq!(
        asked,
        vec![
            StepId::LoopName,
            StepId::Controller,
            StepId::ProcessVariable,
            StepId::ControllerOutput,
            StepId::Setpoint,
            StepId::Workflow,
            StepId::ProcessUnit,
            StepId::ProcessMinimum,
            StepId::ProcessMaximum,
            StepId::OutputUnit,
            StepId::OutputMinimum,
            StepId::OutputMaximum,
            StepId::ProportionalGain,
            StepId::GainMinimum,
            StepId::GainMaximum,
            StepId::ManualCommand,
            StepId::ManualMode,
            StepId::ManualModePolarity,
            StepId::ChannelMode,
            StepId::ActuatorCapability,
            StepId::RisingRate,
            StepId::FallingRate,
            StepId::AppliedFeedback,
        ]
    );
    assert!(!asked.contains(&StepId::ObservationDuration));
    assert!(!asked.contains(&StepId::ControllerTask));
}

#[test]
fn the_gain_maximum_is_at_or_above_its_minimum_and_a_process_maximum_strictly_above_its_minimum() {
    // control-studio-command.test.ts: the gain maximum validator is `value >= minimum`; configureLoop.ts's process and
    // output maximums are `value > minimum`, and the period is above zero.
    assert!(NumberRule::AtLeast(0.0).accepts(0.0));
    assert!(!NumberRule::AtLeast(0.0).accepts(-1.0));
    assert!(!NumberRule::AboveBound(12.0).accepts(12.0));
    assert!(NumberRule::AboveBound(12.0).accepts(12.5));
    assert!(!NumberRule::Positive.accepts(0.0));
    assert!(NumberRule::Positive.accepts(0.5));
}

#[test]
fn an_adapter_route_asks_a_mapping_file_and_no_duration() {
    // configureLoop.ts asks the duration for the adapter route; the spec does not (B1.3, C1 and C2 Setup): Identify
    // chooses it within Limits. The adapter route asks for the mapping file and no duration.
    let mut canned = proportional_canned();
    canned.workflow = SetupWorkflow::Adapter;
    canned.gain = None;
    canned.manual_command = None;
    canned.manual_mode = None;
    let asked = walk(&proportional_catalog(), &canned);
    assert!(asked.contains(&StepId::AdapterFile));
    assert!(!asked.contains(&StepId::ObservationDuration));
    assert!(!asked.contains(&StepId::ProportionalGain));
    let file = asked.iter().position(|id| *id == StepId::AdapterFile);
    let channels = asked.iter().position(|id| *id == StepId::ChannelMode);
    assert!(file < channels, "the mapping is read before the channels");
}

#[test]
fn the_output_and_the_setpoint_exclude_the_signals_already_claimed_by_address() {
    // configureLoop.ts: the controller output is any signal at another address than the process variable; the setpoint
    // is any signal at neither address, after "No setpoint (open loop)".
    let catalog = proportional_catalog();
    let process = named(&catalog, "Main.PV").clone();
    let answers = SetupStepAnswers {
        name: Some("L".into()),
        controller: Some(ControllerChoice::Compiled(
            catalog.control.controllers[0].clone(),
        )),
        process_variable: Some(process),
        ..SetupStepAnswers::default()
    };
    let output = next_step(&catalog, &answers).expect("the output is asked");
    assert_eq!(output.step, StepId::ControllerOutput);
    let StepKind::Choice(output_items) = output.kind else {
        panic!("the output is a choice")
    };
    assert!(output_items.iter().all(|item| item.label != "Main.PV"));

    let answers = SetupStepAnswers {
        controller_output: Some(named(&catalog, "Main.Output").clone()),
        ..answers
    };
    let setpoint = next_step(&catalog, &answers).expect("the setpoint is asked");
    assert_eq!(setpoint.step, StepId::Setpoint);
    let StepKind::Choice(items) = setpoint.kind else {
        panic!("the setpoint is a choice")
    };
    assert_eq!(items[0].label, "No setpoint (open loop)");
    assert_eq!(items[0].description, None);
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(
        labels,
        vec!["No setpoint (open loop)", "Main.SP", "Main.ManualCommand"]
    );
    assert_eq!(items[2].index, 2, "the indices are the list positions");
}

#[test]
fn the_feedback_offers_only_input_addresses_of_a_numeric_or_boolean_type_not_claimed_by_the_loop() {
    // configureLoopChannels.ts: the applied-action candidates are `%I` addresses of type REAL, LREAL or BOOL that are not
    // the process variable, the output or the setpoint; the description names the output's unit.
    let catalog = fixture(
        vec![controller("P", "Law", &[("MainTask", 10.0)])],
        Vec::new(),
        vec![
            signal("Main.PV", "%IL0", "LREAL"),
            signal("Main.Output", "%QL0", "LREAL"),
            signal("Main.Applied", "%IL1", "LREAL"),
            signal("Main.Flag", "%IX2.0", "BOOL"),
            signal("Main.Text", "%IW3", "DINT"),
            signal("Main.Memory", "%MX0.0", "BOOL"),
        ],
    );
    let answers = SetupStepAnswers {
        name: Some("L".into()),
        controller: Some(ControllerChoice::Compiled(
            catalog.control.controllers[0].clone(),
        )),
        process_variable: Some(named(&catalog, "Main.PV").clone()),
        controller_output: Some(named(&catalog, "Main.Output").clone()),
        setpoint: Some(SetpointChoice::OpenLoop),
        workflow: Some(SetupWorkflow::Observation),
        process_unit: Some("bar".into()),
        process_minimum: Some(0.0),
        process_maximum: Some(12.0),
        output_unit: Some("%".into()),
        output_minimum: Some(0.0),
        output_maximum: Some(100.0),
        observation_duration_seconds: Some(30.0),
        channel_mode: Some(ChannelMode::Single),
        actuator: Some(ActuatorChoice::Continuous),
        rising_rate: Some(RateAnswer::Unlimited),
        falling_rate: Some(RateAnswer::Unlimited),
        ..SetupStepAnswers::default()
    };
    let step = next_step(&catalog, &answers).expect("the feedback is asked");
    assert_eq!(step.step, StepId::AppliedFeedback);
    assert_eq!(step.title, "Main.Output: applied actuator feedback");
    let StepKind::Choice(items) = step.kind else {
        panic!("the feedback is a choice")
    };
    let offered: Vec<(&str, Option<&str>)> = items
        .iter()
        .map(|item| (item.label.as_str(), item.description.as_deref()))
        .collect();
    assert_eq!(
        offered,
        vec![
            (
                "Use the controller output",
                Some("No separate applied-action feedback is declared")
            ),
            ("Main.Applied", Some("LREAL · applied action in %")),
            ("Main.Flag", Some("BOOL · applied action in %")),
        ]
    );
}

#[test]
fn a_boolean_output_is_an_on_off_actuator_and_asks_no_actuator_question() {
    // configureLoopChannels.ts: a BOOL manipulated variable forces the ON/OFF capability. It is parked for V7, so the
    // plan asks no actuator question and the build refuses the loop by name.
    let catalog = fixture(
        vec![controller("P", "Law", &[("MainTask", 10.0)])],
        Vec::new(),
        vec![
            signal("Main.PV", "%IL0", "LREAL"),
            signal("Main.Valve", "%QX0.0", "BOOL"),
        ],
    );
    let canned = Canned {
        name: "Valve",
        controller: ControllerChoice::Compiled(catalog.control.controllers[0].clone()),
        process: "Main.PV",
        output: "Main.Valve",
        setpoint: SetpointChoice::OpenLoop,
        ..pump_canned()
    };
    let asked = walk(&catalog, &canned);
    assert_eq!(asked.last(), Some(&StepId::ChannelMode));
    assert!(!asked.contains(&StepId::ActuatorCapability));
}

#[test]
fn the_on_off_workflow_asks_no_channel_question_and_multiple_channels_stop_after_the_mode() {
    // configureOnOffController.ts and the multiple channels of configureLoopChannels.ts are parked (V7): the plan asks
    // nothing more after the bounds (on-off) or after the mode (multiple), and the build refuses them by name.
    let mut on_off = proportional_canned();
    on_off.workflow = SetupWorkflow::OnOff;
    let asked = walk(&proportional_catalog(), &on_off);
    assert_eq!(asked.last(), Some(&StepId::OutputMaximum));

    let mut several = proportional_canned();
    several.channels = ChannelMode::Multiple;
    let asked = walk(&proportional_catalog(), &several);
    assert_eq!(asked.last(), Some(&StepId::ChannelMode));
}

#[test]
fn a_catalog_without_a_signal_asks_nothing() {
    // configureLoop.ts: "No compiled process-image signals are available." is refused before any question.
    let catalog = fixture(vec![controller("P", "Law", &[])], Vec::new(), Vec::new());
    assert_eq!(next_step(&catalog, &SetupStepAnswers::default()), None);
}

#[test]
fn the_observation_asks_its_duration_once_and_the_rule_is_one_second_to_a_day() {
    // B1.3 and C12: the observation asks how long to record it, in seconds, 1 s to 24 h.
    let asked = walk(&pump_catalog(), &pump_canned());
    assert_eq!(
        asked
            .iter()
            .filter(|id| **id == StepId::ObservationDuration)
            .count(),
        1
    );
    assert_eq!(
        DURATION_RULE,
        NumberRule::Within {
            minimum: 1.0,
            maximum: 86_400.0,
        }
    );
}

#[test]
fn back_returns_to_the_nearest_earlier_answer_the_engineer_may_still_change() {
    // setupNavigation.ts `canGoBack` and the Back index: the first question has no Back
    // (control-studio-setup-navigation.test.ts: "Back revisits the prior answer ..." and `canGoBack === false` at the start).
    assert_eq!(back_target(&[], 0), None);
    assert_eq!(back_target(&[true], 1), Some(0));
    assert_eq!(back_target(&[true, false, true], 3), Some(2));
    assert_eq!(
        back_target(&[true, true, false], 2),
        Some(1),
        "an answer that is not editable is skipped"
    );
    assert_eq!(back_target(&[false, false], 2), None);
}

#[test]
fn a_changed_answer_discards_the_answers_that_came_after_it() {
    // control-studio-setup-navigation.test.ts: "changing an earlier choice discards dependent answers". An unchanged answer
    // keeps them, and the first answer discards nothing.
    assert!(changed_answer_discards_later(Some(&"A"), &"B"));
    assert!(!changed_answer_discards_later(Some(&"A"), &"A"));
    assert!(!changed_answer_discards_later::<&str>(None, &"A"));
}

#[test]
fn a_result_is_an_accepted_answer_a_back_or_a_cancel() {
    // setupPromptTypes.ts `SetupPromptResult`: the three results of a question.
    assert_eq!(StepResult::Accepted(3), StepResult::Accepted(3));
    assert_ne!(StepResult::<u8>::Back, StepResult::Cancelled);
}

#[test]
fn a_choice_items_value_is_any_json_and_its_declaration_is_unknown_not_an_object() {
    // A choice's value is the answer the page sends back: a signal's address, a symbol's name, a workflow's key, a flag or null for no
    // setpoint. The page's declaration of it is `unknown`; the declaration generator reads `tsType`.
    let schema = schemars::SchemaGenerator::default().into_root_schema_for::<ChoiceItem>();
    let schema = serde_json::to_value(&schema).expect("the schema is JSON");
    assert_eq!(
        schema["properties"]["value"]["tsType"],
        serde_json::json!("unknown"),
        "the generated type of the value is unknown"
    );
}
