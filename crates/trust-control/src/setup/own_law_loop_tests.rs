//! Tests of the own-law generator (`ownLawLoopGenerator.ts`): the setup assertions of `control-studio-loop-setup-own-law.test.ts` on
//! its oven program (the program computes its own PI, P or PID), the refusals of the TypeScript, and the startup-done rule of C1 Setup.
//! The Tune families, the seed design and the noise allowance of that file are V7 or V4 and are not ported here.

#![allow(clippy::float_cmp)]

use serde_json::{json, Value};

use super::{own_law_loop_definition, DerivativeOn, OwnLawFamily, OwnLawGain, OwnLawLoopDraft};
use crate::commissioning::library_pid::library_pid_initialization;
use crate::failure::Failure;
use crate::json::decode_value;
use crate::loop_definition::{
    Action, Design, InitialSource, InitialValue, InitializedState, LoopDefinition, LoopScope,
    ModeWhenTrue, PostScan, RealType, StateType,
};
use crate::setup::catalog::{decode_catalog_answer, CompiledCatalog};
use crate::setup::loop_bindings::MeasuredSignal;
use crate::setup::pid_bindings::startup_state;
use crate::setup::pid_loop::{ModeChoice, SignalChoice};

fn scope() -> LoopScope {
    LoopScope {
        project_id: "project:setup".to_string(),
        runtime_id: "runtime-1".to_string(),
    }
}

/// The sentence a refusal shows: its whole text, which is the detail when it says more than its first sentence.
fn text_of(failure: &Failure) -> String {
    failure
        .detail
        .clone()
        .unwrap_or_else(|| failure.message.clone())
}

fn signal(symbol: &str, unit: &str, minimum: f64, maximum: f64) -> MeasuredSignal {
    MeasuredSignal {
        symbol: symbol.to_string(),
        unit: unit.to_string(),
        minimum,
        maximum,
    }
}

fn gain(symbol: &str, minimum: f64, maximum: f64) -> OwnLawGain {
    OwnLawGain {
        symbol: symbol.to_string(),
        minimum,
        maximum,
    }
}

#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literals its callers write in place.
fn decode(control: Value) -> CompiledCatalog {
    decode_catalog_answer(&json!({ "ok": true, "processImage": {}, "controlSymbols": control }))
        .expect("the compiled answer decodes")
        .control
}

/// The oven of `control-studio-loop-setup-own-law.test.ts`: the program computes its own PI,
/// `HeaterPct := Kp·(SetpointC − OvenC) + Integral`.
fn oven_json() -> Value {
    json!({
        "symbols": [
            { "symbol": "P1.OvenC", "type": "LREAL", "address": "%IL0" },
            { "symbol": "P1.HeaterPct", "type": "LREAL", "address": "%QL0" },
            { "symbol": "P1.SetpointC", "type": "LREAL", "address": "%ML0" },
            { "symbol": "P1.ManualPct", "type": "LREAL", "address": "%ML8" },
            { "symbol": "P1.AutoMode", "type": "BOOL" },
            { "symbol": "P1.Started", "type": "BOOL" },
            { "symbol": "P1.Kp", "type": "LREAL" },
            { "symbol": "P1.Ki", "type": "REAL" },
            { "symbol": "P1.Integral", "type": "LREAL" },
            { "symbol": "P1.Kd", "type": "LREAL" },
            { "symbol": "P1.PreviousErrorC", "type": "LREAL" },
            { "symbol": "P1.PreviousOvenC", "type": "LREAL" }
        ],
        "controllers": [
            { "instance": "P1", "type": "Main", "tasks": [{ "name": "MainTask", "intervalMs": 100 }] }
        ],
        "links": [],
        "writes": ["P1.HeaterPct", "P1.Integral", "P1.Started"]
    })
}

fn oven() -> CompiledCatalog {
    decode(oven_json())
}

/// The PI draft of the oven (`draft` in `control-studio-loop-setup-own-law.test.ts`).
fn pi_draft() -> OwnLawLoopDraft {
    OwnLawLoopDraft {
        name: "Oven".to_string(),
        action: Action::Direct,
        law: OwnLawFamily::Pi,
        instance: "P1".to_string(),
        process_variable: signal("P1.OvenC", "degC", 0.0, 300.0),
        manipulated_variable: signal("P1.HeaterPct", "%", 0.0, 100.0),
        setpoint: SignalChoice {
            symbol: "P1.SetpointC".to_string(),
            unit: None,
            minimum: None,
            maximum: None,
        },
        mode: ModeChoice {
            symbol: "P1.AutoMode".to_string(),
            when_true: ModeWhenTrue::Auto,
        },
        manual_output: "P1.ManualPct".to_string(),
        proportional_gain: gain("P1.Kp", 0.0, 20.0),
        integral_gain: Some(gain("P1.Ki", 0.0, 2.0)),
        integral_term: Some("P1.Integral".to_string()),
        derivative_gain: None,
        derivative_state: None,
        derivative_on: None,
        experiment_step: None,
        experiment_duration_seconds: None,
        startup_flag: Some("P1.Started".to_string()),
    }
}

/// The project block of a generated own-law loop: the `controller.project` value.
fn project_of(loop_definition: &LoopDefinition) -> &Value {
    loop_definition
        .controller
        .project
        .as_ref()
        .expect("an own-law loop declares its project block")
}

#[test]
#[allow(clippy::too_many_lines)] // One ported TypeScript test, kept whole so its assertions read in its order.
fn the_program_pi_is_a_project_controller_whose_seed_names_its_gains() {
    // control-studio-loop-setup-own-law.test.ts: "writes the program's own PI as a project controller whose seed names its gain variables".
    let loop_definition = own_law_loop_definition(&oven(), &scope(), &pi_draft())
        .expect("the program's PI is generated");
    loop_definition
        .validate(&scope())
        .expect("the loop is accepted");
    let text = serde_json::to_string(&loop_definition).expect("the loop serializes");
    LoopDefinition::decode(&text)
        .expect("the loop contract accepts a PI seed without a derivative gain");

    let controller = &loop_definition.controller;
    assert_eq!(controller.instance, "P1");
    assert_eq!(
        controller.type_name, "Main",
        "the controller is the program, not a library PID"
    );
    assert_eq!(controller.design, Some(Design::Project));
    assert!(controller.law.is_none(), "the law belongs to the program");

    let project = project_of(&loop_definition);
    assert_eq!(project["family"], "PI");
    assert_eq!(project["convention"], "project-owned-native-controller");
    assert_eq!(
        project["seed"],
        json!({ "kind": "parallel-pid", "family": "PI", "proportionalSymbol": "P1.Kp", "integralSymbol": "P1.Ki" })
    );
    let binding = loop_definition
        .parameter_binding
        .as_ref()
        .expect("the gains are bound");
    let parameters: Vec<(&str, RealType, &str, f64, f64)> = binding
        .parameters
        .iter()
        .map(|parameter| {
            (
                parameter.symbol.as_str(),
                parameter.iec_type,
                parameter.unit.as_str(),
                parameter.minimum,
                parameter.maximum,
            )
        })
        .collect();
    assert_eq!(
        parameters,
        vec![
            ("P1.Kp", RealType::Lreal, "%/degC", 0.0, 20.0),
            ("P1.Ki", RealType::Real, "%/(degC*s)", 0.0, 2.0),
        ],
        "each gain takes its own variable's REAL or LREAL"
    );
    let states: Vec<InitializedState> =
        decode_value(project["initialization"]["states"].clone()).expect("the states decode");
    assert_eq!(
        states,
        vec![
            InitializedState {
                symbol: "P1.Integral".to_string(),
                iec_type: StateType::Lreal,
                unit: "%".to_string(),
                minimum: -1000.0,
                maximum: 1100.0,
                initial: InitialValue::From(InitialSource::PidIntegral {
                    index: 0,
                    input_index: 0,
                    gain_symbol: "P1.Kp".to_string(),
                    weight_symbol: None,
                    weight: None,
                }),
                post_scan: PostScan::Bounded,
            },
            startup_state("P1.Started"),
        ],
        "the integral term starts at the output of the switch to Auto; the startup flag starts TRUE"
    );
    assert_eq!(
        (
            loop_definition
                .signals
                .setpoint
                .as_ref()
                .map(|setpoint| setpoint.symbol.as_str()),
            loop_definition.signals.process_variable.symbol.as_str(),
            loop_definition.signals.manipulated_variable.symbol.as_str(),
        ),
        (Some("P1.SetpointC"), "P1.OvenC", "P1.HeaterPct")
    );
    assert_eq!(
        (
            controller.mode_signal.as_deref(),
            controller.manual_output_signal.as_deref(),
            binding.task_name.as_str(),
        ),
        (Some("P1.AutoMode"), Some("P1.ManualPct"), "MainTask")
    );
    assert_eq!(
        project["objective"],
        json!({ "requirementId": "target-error", "direction": "minimize" }),
        "Tune ranks the candidates by the error requirement the campaign measures"
    );
    assert!(
        loop_definition.experiment.is_none(),
        "setup writes no experiment the engineer did not set"
    );
}

#[test]
fn a_p_law_has_only_its_proportional_gain() {
    // control-studio-loop-setup-own-law.test.ts: "offers Tune only the families whose gains the program has", the setup part.
    let draft = OwnLawLoopDraft {
        name: "Oven P".to_string(),
        law: OwnLawFamily::P,
        integral_gain: None,
        integral_term: None,
        ..pi_draft()
    };
    let loop_definition =
        own_law_loop_definition(&oven(), &scope(), &draft).expect("the program's P is generated");
    loop_definition
        .validate(&scope())
        .expect("the P loop is accepted");
    let project = project_of(&loop_definition);
    assert_eq!(project["family"], "P");
    assert_eq!(
        project["seed"],
        json!({ "kind": "parallel-pid", "family": "P", "proportionalSymbol": "P1.Kp" })
    );
    let binding = loop_definition
        .parameter_binding
        .as_ref()
        .expect("the gain is bound");
    assert_eq!(
        binding
            .parameters
            .iter()
            .map(|parameter| parameter.symbol.as_str())
            .collect::<Vec<_>>(),
        vec!["P1.Kp"]
    );
    let states: Vec<InitializedState> =
        decode_value(project["initialization"]["states"].clone()).expect("the states decode");
    assert_eq!(
        states
            .iter()
            .map(|state| state.symbol.as_str())
            .collect::<Vec<_>>(),
        vec!["P1.Started"],
        "a P law has no integral term; only the startup flag is started"
    );
}

#[test]
#[allow(clippy::too_many_lines)] // One ported TypeScript test, kept whole so its assertions read in its order.
fn a_pid_law_takes_its_derivative_from_the_error_or_the_measurement() {
    // control-studio-loop-setup-own-law.test.ts: "writes the program's own PID with the previous value its derivative is taken from".
    let pid = OwnLawLoopDraft {
        name: "Oven PID".to_string(),
        law: OwnLawFamily::Pid,
        derivative_gain: Some(gain("P1.Kd", 0.0, 50.0)),
        derivative_state: Some("P1.PreviousErrorC".to_string()),
        derivative_on: Some(DerivativeOn::Error),
        ..pi_draft()
    };
    let loop_definition =
        own_law_loop_definition(&oven(), &scope(), &pid).expect("the program's PID is generated");
    loop_definition
        .validate(&scope())
        .expect("the PID loop is accepted");
    assert_eq!(
        project_of(&loop_definition)["seed"],
        json!({ "kind": "parallel-pid", "family": "PID", "proportionalSymbol": "P1.Kp",
                "integralSymbol": "P1.Ki", "derivativeSymbol": "P1.Kd" })
    );
    let binding = loop_definition
        .parameter_binding
        .as_ref()
        .expect("the gains are bound");
    assert_eq!(
        binding
            .parameters
            .iter()
            .map(|parameter| (parameter.symbol.as_str(), parameter.unit.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("P1.Kp", "%/degC"),
            ("P1.Ki", "%/(degC*s)"),
            ("P1.Kd", "%*s/degC"),
        ]
    );
    let states: Vec<InitializedState> =
        decode_value(project_of(&loop_definition)["initialization"]["states"].clone())
            .expect("the states decode");
    let previous = states
        .iter()
        .find(|state| state.symbol == "P1.PreviousErrorC")
        .expect("the previous error is a state");
    assert_eq!(previous.unit, "degC");
    assert_eq!(
        (previous.minimum, previous.maximum),
        (-300.0, 300.0),
        "the error is within the span of the measurement"
    );
    assert_eq!(
        previous.initial,
        InitialValue::From(InitialSource::WeightedError {
            index: 0,
            weight_symbol: None,
            weight: Some(1.0),
        }),
        "the previous error starts at the error of that moment, zero with the setpoint at the measurement"
    );

    let measured = own_law_loop_definition(
        &oven(),
        &scope(),
        &OwnLawLoopDraft {
            name: "Oven PID measured".to_string(),
            derivative_state: Some("P1.PreviousOvenC".to_string()),
            derivative_on: Some(DerivativeOn::Measurement),
            ..pid.clone()
        },
    )
    .expect("the measurement variant is generated");
    let states: Vec<InitializedState> =
        decode_value(project_of(&measured)["initialization"]["states"].clone())
            .expect("the states decode");
    let previous = states
        .iter()
        .find(|state| state.symbol == "P1.PreviousOvenC")
        .expect("the previous measurement is a state");
    assert_eq!(
        (previous.initial.clone(), previous.minimum, previous.maximum),
        (
            InitialValue::From(InitialSource::ModelOutput { index: 0 }),
            0.0,
            300.0
        ),
        "the previous measurement starts at the measurement of the switch"
    );

    let refused = |draft: OwnLawLoopDraft| {
        text_of(&own_law_loop_definition(&oven(), &scope(), &draft).expect_err("refused"))
    };
    assert_eq!(
        refused(OwnLawLoopDraft {
            derivative_gain: None,
            ..pid.clone()
        }),
        "A PID law needs the variable holding its derivative gain."
    );
    assert_eq!(
        refused(OwnLawLoopDraft {
            derivative_state: None,
            ..pid.clone()
        }),
        "A PID law needs the variable holding the previous error or measurement its derivative is taken from."
    );
    assert_eq!(
        refused(OwnLawLoopDraft {
            derivative_state: Some("P1.Integral".to_string()),
            ..pid
        }),
        "Choose a different variable for each gain and for the integral term."
    );
}

#[test]
fn the_refusals_of_a_pi_law_are_the_typescript_refusals() {
    // control-studio-loop-setup-own-law.test.ts: "refuses a PI without its integral term and gains that are not distinct REAL variables".
    let refused = |draft: OwnLawLoopDraft| {
        text_of(&own_law_loop_definition(&oven(), &scope(), &draft).expect_err("refused"))
    };
    assert_eq!(
        refused(OwnLawLoopDraft {
            integral_term: None,
            ..pi_draft()
        }),
        "A PI law needs the variable holding its integral term."
    );
    assert_eq!(
        refused(OwnLawLoopDraft {
            integral_gain: None,
            ..pi_draft()
        }),
        "A PI law needs the variable holding its integral gain."
    );
    assert_eq!(
        refused(OwnLawLoopDraft {
            integral_gain: Some(gain("P1.Kp", 0.0, 2.0)),
            ..pi_draft()
        }),
        "Choose a different variable for each gain and for the integral term."
    );
    assert_eq!(
        refused(OwnLawLoopDraft {
            proportional_gain: gain("P1.AutoMode", 0.0, 20.0),
            ..pi_draft()
        }),
        "P1.AutoMode must be REAL or LREAL."
    );
    assert_eq!(
        refused(OwnLawLoopDraft {
            proportional_gain: gain("P1.Kp", 5.0, 1.0),
            ..pi_draft()
        }),
        "The range of P1.Kp needs a minimum below its maximum."
    );
}

#[test]
fn an_instance_that_is_not_a_compiled_program_is_refused() {
    let failure = own_law_loop_definition(
        &oven(),
        &scope(),
        &OwnLawLoopDraft {
            instance: "P9".to_string(),
            ..pi_draft()
        },
    )
    .expect_err("P9 is not compiled");
    assert_eq!(
        text_of(&failure),
        "P9 is not a compiled program or function block instance."
    );
}

#[test]
fn a_program_that_runs_in_two_tasks_has_no_own_law_loop() {
    let mut value = oven_json();
    value["controllers"][0]["tasks"] = json!([
        { "name": "MainTask", "intervalMs": 100 },
        { "name": "Fast", "intervalMs": 10 }
    ]);
    let failure = own_law_loop_definition(&decode(value), &scope(), &pi_draft())
        .expect_err("two tasks are refused");
    assert_eq!(text_of(&failure), "P1 must run in exactly one cyclic task.");
}

#[test]
fn a_law_whose_gain_the_program_writes_needs_the_startup_flag() {
    // C1 Setup: a program that writes a gain itself would replace the candidate being validated, so setup requires the startup flag.
    let mut value = oven_json();
    value["writes"] = json!(["P1.HeaterPct", "P1.Integral", "P1.Kp"]);
    let catalog = decode(value);
    let failure = own_law_loop_definition(
        &catalog,
        &scope(),
        &OwnLawLoopDraft {
            startup_flag: None,
            ..pi_draft()
        },
    )
    .expect_err("a written gain needs the startup flag");
    assert_eq!(
        text_of(&failure),
        "The program writes P1.Kp in its own code. Choose the flag that marks its startup as done, so validation keeps the values it starts from."
    );
    own_law_loop_definition(&catalog, &scope(), &pi_draft())
        .expect("with the startup flag the loop is generated");
}

#[test]
fn the_integral_term_the_program_updates_every_scan_needs_no_startup_flag() {
    // control-studio-loop-setup-own-law.test.ts: "the integral term the program updates every scan needs no startup flag": the
    // program writes P1.Integral, which is the law's own running state, not a seeded value.
    let loop_definition = own_law_loop_definition(
        &oven(),
        &scope(),
        &OwnLawLoopDraft {
            startup_flag: None,
            ..pi_draft()
        },
    )
    .expect("the integral term needs no flag");
    loop_definition
        .validate(&scope())
        .expect("the loop is accepted");
}

#[test]
fn a_programs_own_law_is_not_a_library_pid() {
    // A program's law has no `control-pid-v1` initialization; the library check is for Control.PID loops only.
    let loop_definition =
        own_law_loop_definition(&oven(), &scope(), &pi_draft()).expect("the PI is generated");
    assert!(
        library_pid_initialization(&loop_definition).is_err(),
        "a program's own law is not a Control.PID"
    );
}

#[test]
fn the_mode_and_the_manual_output_are_bound_as_the_engineer_chose() {
    let loop_definition =
        own_law_loop_definition(&oven(), &scope(), &pi_draft()).expect("the PI is generated");
    let binding = loop_definition
        .parameter_binding
        .as_ref()
        .expect("the gains are bound");
    assert_eq!(binding.mode.symbol, "P1.AutoMode");
    assert!(
        !binding.mode.manual_value,
        "Auto is the FALSE value of the mode"
    );
    assert_eq!(
        loop_definition.controller.manual_output_address.as_deref(),
        Some("%ML8"),
        "the manual output's address is the compiled one"
    );
    assert_eq!(
        loop_definition.controller.manual_output_type,
        Some(crate::loop_definition::ManualOutputType::Lreal)
    );
    assert_eq!(
        loop_definition.signals.manipulated_variable.direction,
        crate::loop_definition::Direction::Write
    );
}
