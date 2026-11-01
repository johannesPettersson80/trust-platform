#![allow(clippy::float_cmp)] // The fixtures are exact decimal inputs, and the requirements are compared as the TypeScript tests state them.

use serde_json::json;

use super::test_support::{base_task, text};
use super::*;
use crate::model::decode_general_model;

/// A model channel with the fixtures' range of -20 to 20.
fn channel(id: &str, unit: &str) -> Value {
    json!({ "id": id, "unit": unit, "operatingRange": { "minimum": -20, "maximum": 20 } })
}

/// A gain of the parameter record: a `LREAL` from 0 to 100.
fn gain(symbol: &str) -> Value {
    json!({ "symbol": symbol, "iec_type": "LREAL", "unit": "V/m", "convention": "gain",
        "minimum": 0, "maximum": 100 })
}

/// The engineer fixture of `control-studio-engineer/control-loop.json`, without its analysis block: the gate of the plan
/// refuses a declared analysis (`rust_engine_support`). The fixture declares the proportional law of its name, which the gate
/// needs and the plan does not read.
fn engineer_loop_json() -> Value {
    json!({
        "schemaVersion": 2,
        "id": "control-studio-engineer",
        "name": "Engineer proportional first-order fixture",
        "revision": "sha256:control-studio-engineer-v3",
        "projectId": "project:control-studio-engineer",
        "runtimeId": "simulator",
        "controller": {
            "instance": "Main",
            "type": "EngineerProportional",
            "design": "project",
            "revision": "sha256:engineer-proportional-v1",
            "action": "direct",
            "taskPeriodMs": 20,
            "manualOutputSignal": "Main.ManualCommand",
            "manualOutputAddress": "%ML8",
            "manualOutputType": "LREAL",
            "modeSignal": "Main.Manual",
            "modeWhenTrue": "Manual",
            "law": { "kind": "proportional", "gainSymbol": "Main.Kp" }
        },
        "signals": {
            "setpoint": { "symbol": "Main.SP", "address": "%ML0", "iecType": "LREAL",
                "direction": "read", "unit": "m", "range": { "minimum": -20, "maximum": 20 } },
            "processVariable": { "symbol": "Main.PV", "address": "%IL0", "iecType": "LREAL",
                "direction": "read", "unit": "m", "range": { "minimum": -20, "maximum": 20 } },
            "manipulatedVariable": { "symbol": "Main.MV", "address": "%QL0", "iecType": "LREAL",
                "direction": "write", "unit": "V", "range": { "minimum": -20, "maximum": 20 } }
        },
        "limits": { "outputMinimum": -20, "outputMaximum": 20 },
        "observedSampleTiming": { "nominalPeriodMs": 20 },
        "operatingPoint": "Preconfigured non-course engineer fixture",
        "experiment": { "execution": "control-studio", "kind": "observation", "amplitude": 0,
            "durationSeconds": 2, "seed": 7 },
        "parameterBinding": {
            "loop_id": "control-studio-engineer",
            "task_name": "MainTask",
            "parameters": [gain("Main.Kp")],
            "mode": { "symbol": "Main.Manual", "manual_value": true }
        }
    })
}

/// The engineer fixture as it decodes, with its one gain and no PID law.
fn engineer_loop() -> LoopDefinition {
    decode_value(engineer_loop_json()).expect("the engineer loop fixture")
}

/// The loop of `control-studio-pid-fixtures.ts` (`pidLoop`): the engineer fixture as a `Control.PID` with three gains.
fn pid_loop() -> LoopDefinition {
    let mut value = engineer_loop_json();
    value["controller"]["type"] = json!("Control.PID");
    value["controller"]["design"] = json!("pid");
    value["controller"]["law"] = json!({
        "kind": "pid",
        "parameterRecordSymbol": "Main.Parameters",
        "parameterRevisionSymbol": "Main.ParameterRevision",
        "proportionalGainSymbol": "Main.Kp",
        "integralGainSymbol": "Main.Ki",
        "derivativeGainSymbol": "Main.Kd"
    });
    value["parameterBinding"]["parameters"] =
        json!([gain("Main.Kp"), gain("Main.Ki"), gain("Main.Kd")]);
    decode_value(value).expect("the PID loop fixture")
}

/// The first-order model of `planModel()` in the `TypeScript` tests: a 0.5 s delay and an operating point of zero.
fn plan_model() -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function",
        "numerator": [1.5],
        "denominator": [4, 1],
        "delaySeconds": 0.5,
        "revision": "model-4",
        "source": "provided",
        "validation": "validated",
        "evidence": ["declared-fixture"],
        "inputs": [channel("u", "V")],
        "outputs": [channel("y", "m")],
        "operatingPoint": { "inputs": [0], "outputs": [0] }
    }))
    .expect("the model fixture")
}

/// The gains of the `TypeScript` tests' `planValues()`.
fn gains(kp: f64, ki: f64, kd: f64) -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("Main.Kp".to_owned(), kp),
        ("Main.Ki".to_owned(), ki),
        ("Main.Kd".to_owned(), kd),
    ])
}

/// The plan of a loop on the first-order model, with no controller initial values.
fn plan_of(
    loop_definition: &LoopDefinition,
    task: &EngineerRequirements,
    values: &BTreeMap<String, f64>,
) -> Result<VerificationPlan, Failure> {
    engineer_verification_plan(
        loop_definition,
        &plan_model(),
        task,
        values,
        &BTreeMap::new(),
    )
}

/// A bounds requirement on the manipulated variable, for the condition tests.
fn bounds_requirement(id: &str, window: Window) -> RunRequirement {
    requirement(
        id,
        "Main.MV",
        window,
        RunCheck::Bounds {
            minimum: -20.0,
            maximum: 20.0,
        },
    )
}

/// The identities of a plan's requirements, in order.
fn ids(plan: &VerificationPlan) -> Vec<&str> {
    plan.requirements
        .iter()
        .map(|item| item.id.as_str())
        .collect()
}

#[test]
fn an_overshoot_requirement_bounds_a_setpoint_step_by_its_percent() {
    // control-studio-overshoot-requirement.test.ts: "adds one overshoot requirement to a setpoint step, scaled to its percent of the step".
    let task = EngineerRequirements {
        maximum_overshoot_percent: Some(5.0),
        ..base_task()
    };
    let plan = plan_of(&pid_loop(), &task, &gains(1.0, 0.2, 0.0)).expect("a plan");
    let overshoot: Vec<&RunRequirement> = plan
        .requirements
        .iter()
        .filter(|item| matches!(item.check, RunCheck::Overshoot { .. }))
        .collect();
    assert_eq!(
        overshoot.len(),
        1,
        "one overshoot requirement per setpoint step"
    );
    assert_eq!(overshoot[0].id, "overshoot");
    assert_eq!(
        overshoot[0].channel, "Main.PV",
        "the limit bounds the measured output"
    );
    match overshoot[0].check {
        RunCheck::Overshoot {
            initial,
            target,
            maximum,
        } => {
            assert_eq!(initial, 0.0, "the step starts at the operating output");
            assert_eq!(target, 1.0);
            assert!(
                (maximum - 0.05).abs() < 1e-12,
                "5 % of a 1-unit step is 0.05 output units"
            );
        }
        ref other => panic!("an overshoot check, not {other:?}"),
    }
}

#[test]
fn no_overshoot_requirement_without_a_declared_limit_or_without_a_setpoint_step() {
    // control-studio-overshoot-requirement.test.ts: "adds no overshoot requirement when no limit is declared or when a run applies no setpoint step".
    let loop_definition = pid_loop();
    let values = gains(1.0, 0.2, 0.0);
    let undeclared = plan_of(&loop_definition, &base_task(), &values).expect("a plan");
    assert!(
        !undeclared
            .requirements
            .iter()
            .any(|item| matches!(item.check, RunCheck::Overshoot { .. })),
        "an undeclared limit leaves no overshoot requirement"
    );
    let regulation = EngineerRequirements {
        reference_change: 0.0,
        maximum_overshoot_percent: Some(5.0),
        ..base_task()
    };
    let plan = plan_of(&loop_definition, &regulation, &values).expect("a regulation plan");
    assert!(
        !plan
            .requirements
            .iter()
            .any(|item| matches!(item.check, RunCheck::Overshoot { .. })),
        "a run with no setpoint change has no step to bound"
    );
}

#[test]
fn the_response_deadline_is_verified_against_the_error_band_of_the_task() {
    // control-studio-simple-controller-design.test.ts: "verifies the entered response deadline against the required target-error band".
    let model = decode_general_model(&json!({
        "kind": "transfer-function",
        "numerator": [1],
        "denominator": [1, 1],
        "delaySeconds": 0,
        "revision": "model",
        "source": "provided",
        "validation": "validated",
        "evidence": ["independent-validation"],
        "inputs": [channel("u", "V")],
        "outputs": [channel("y", "m")]
    }))
    .expect("the model fixture");
    let task = EngineerRequirements {
        reference_change: 1.0,
        maximum_error: 0.1,
        settling_band: 0.02,
        settling_time_seconds: 2.0,
        duration_seconds: 4.0,
        ..base_task()
    };
    let values = BTreeMap::from([("Main.Kp".to_owned(), 11.5)]);
    let plan =
        engineer_verification_plan(&engineer_loop(), &model, &task, &values, &BTreeMap::new())
            .expect("a plan");
    let settling = plan
        .requirements
        .iter()
        .find(|item| matches!(item.check, RunCheck::Settling { .. }))
        .expect("a settling check");
    assert_eq!(
        *settling,
        RunRequirement {
            id: "response-deadline".to_owned(),
            channel: "Main.PV".to_owned(),
            window: Window {
                start: 0.0,
                end: 4.0
            },
            check: RunCheck::Settling {
                target: 1.0,
                band: 0.1,
                hold_seconds: 2.0,
                deadline_seconds: 2.0,
            },
        },
        "the deadline is verified within the target-error band"
    );
}

#[test]
fn a_long_validation_runs_every_scan_and_records_at_the_process_period() {
    // control-studio-scenario-duration.test.ts: "simulates a long validation at the task period and records it at a record period that follows the process".
    let mut loop_definition = pid_loop();
    loop_definition.controller.task_period_ms = 100.0;
    let model = decode_general_model(&json!({
        "kind": "transfer-function",
        "numerator": [1.5],
        "denominator": [4500, 1],
        "delaySeconds": 10,
        "revision": "slow-process",
        "source": "provided",
        "validation": "validated",
        "evidence": ["declared-fixture"],
        "inputs": [channel("u", "V")],
        "outputs": [channel("y", "m")],
        "operatingPoint": { "inputs": [0], "outputs": [0] }
    }))
    .expect("the slow model fixture");
    let task = EngineerRequirements {
        settling_time_seconds: 7_200.0,
        duration_seconds: 14_400.0,
        maximum_error: 0.2,
        settling_band: 0.1,
        ..base_task()
    };
    let plan = engineer_verification_plan(
        &loop_definition,
        &model,
        &task,
        &gains(1.0, 0.001, 0.0),
        &BTreeMap::new(),
    )
    .expect("a plan");
    assert_eq!((plan.step_ms, plan.duration_ms), (100, 14_400_000));
    assert_eq!(
        plan.max_total_cycles, 144_001,
        "every execution of the 4 h run is simulated"
    );
    // About 200 samples over the 2 h settling time: one sample per 36 s, 360 executions each.
    assert_eq!(
        plan.record_every, 360,
        "the record period follows the process, not the 100 ms task"
    );
    let observed = u64::try_from(plan.observed_symbols.len()).expect("a small count");
    assert!(
        isolated_record_every(plan.step_ms, plan.duration_ms, observed) <= 360,
        "the process's record period also keeps the run within the sample limits"
    );
}

#[test]
fn the_requirements_follow_the_typescript_order_and_the_fixed_parameters_come_last() {
    let task = EngineerRequirements {
        maximum_overshoot_percent: Some(5.0),
        ..base_task()
    };
    let plan = plan_of(&pid_loop(), &task, &gains(1.0, 0.2, 0.0)).expect("a plan");
    assert_eq!(
        ids(&plan),
        [
            "response-deadline",
            "target-error",
            "overshoot",
            "process-bounds",
            "output-bounds",
            "fixed-parameter:Main.Kp",
            "fixed-parameter:Main.Ki",
            "fixed-parameter:Main.Kd",
        ]
    );
}

#[test]
fn the_observed_symbols_are_the_loop_signals_then_the_parameters() {
    let plan = plan_of(&pid_loop(), &base_task(), &gains(1.0, 0.2, 0.0)).expect("a plan");
    assert_eq!(
        plan.observed_symbols,
        ["Main.MV", "Main.PV", "Main.SP", "Main.Kp", "Main.Ki", "Main.Kd"]
    );
}

#[test]
fn the_run_starts_in_auto_at_the_operating_input_and_steps_the_setpoint_to_its_target() {
    let plan = plan_of(&pid_loop(), &base_task(), &gains(1.0, 0.2, 0.0)).expect("a plan");
    assert_eq!(
        plan.initial_values,
        BTreeMap::from([
            ("Main.Manual".to_owned(), 0.0),
            ("Main.MV".to_owned(), 0.0),
            ("Main.SP".to_owned(), 1.0),
        ])
    );
    // A controller's initial value is kept; the mode, the input and the setpoint then write their own symbols.
    let initial = BTreeMap::from([
        ("Main.Integral".to_owned(), 2.5),
        ("Main.Manual".to_owned(), 1.0),
        ("Main.SP".to_owned(), 9.0),
    ]);
    let plan = engineer_verification_plan(
        &pid_loop(),
        &plan_model(),
        &base_task(),
        &gains(1.0, 0.2, 0.0),
        &initial,
    )
    .expect("a plan with controller initial values");
    assert_eq!(
        plan.initial_values,
        BTreeMap::from([
            ("Main.Integral".to_owned(), 2.5),
            ("Main.Manual".to_owned(), 0.0),
            ("Main.MV".to_owned(), 0.0),
            ("Main.SP".to_owned(), 1.0),
        ])
    );
}

#[test]
fn a_fast_process_keeps_every_scan() {
    let values = BTreeMap::from([("Main.Kp".to_owned(), 1.0)]);
    let task = EngineerRequirements {
        duration_seconds: 4.0,
        settling_time_seconds: 2.0,
        maximum_error: 0.1,
        settling_band: 0.02,
        ..base_task()
    };
    let plan = plan_of(&engineer_loop(), &task, &values).expect("a plan");
    assert_eq!((plan.step_ms, plan.duration_ms), (20, 4_000));
    assert_eq!(plan.max_total_cycles, 201);
    assert_eq!(
        plan.record_every, 1,
        "a settling time of 2 s keeps every scan"
    );
}

#[test]
fn the_run_must_fit_the_longest_experiment_and_pass_its_response_deadline() {
    const DURATION: &str =
        "Choose a validation of at most 24 h that runs past the response deadline.";
    let too_long = EngineerRequirements {
        duration_seconds: 86_401.0,
        ..base_task()
    };
    let failure =
        plan_of(&pid_loop(), &too_long, &gains(1.0, 0.2, 0.0)).expect_err("a run longer than 24 h");
    assert_eq!(text(&failure), DURATION);
    // A run of 15 s with a response deadline of 15 s does not run past the deadline.
    let short = EngineerRequirements {
        duration_seconds: 15.0,
        ..base_task()
    };
    let failure = plan_of(&pid_loop(), &short, &gains(1.0, 0.2, 0.0))
        .expect_err("a run that ends at its deadline");
    assert_eq!(text(&failure), DURATION);
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::OpenRequirements)
    );
}

#[test]
fn a_run_that_needs_more_scans_than_the_runtime_executes_is_refused() {
    let mut loop_definition = pid_loop();
    loop_definition.controller.task_period_ms = 1.0;
    let task = EngineerRequirements {
        duration_seconds: 86_400.0,
        ..base_task()
    };
    let failure =
        plan_of(&loop_definition, &task, &gains(1.0, 0.2, 0.0)).expect_err("86,400,001 scans");
    assert_eq!(
        text(&failure),
        "Validation would execute 86400001 scans; the runtime executes at most 10000000 per run. Shorten the test duration."
    );
    assert_eq!(failure.next_action, NextAction::OpenRequirements);
}

#[test]
fn the_task_period_must_be_a_whole_millisecond_from_1_to_1000() {
    for period in [0.0, 1.5, 1_001.0, f64::NAN] {
        let mut loop_definition = pid_loop();
        loop_definition.controller.task_period_ms = period;
        let failure = plan_of(&loop_definition, &base_task(), &gains(1.0, 0.2, 0.0))
            .expect_err("a period outside 1 to 1000 whole milliseconds");
        assert_eq!(
            text(&failure),
            "This runtime adapter requires a whole-millisecond task period from 1 to 1000."
        );
        assert_eq!(failure.next_action, NextAction::OpenLoopSetup);
    }
    let mut loop_definition = pid_loop();
    loop_definition.controller.task_period_ms = 1_000.0;
    assert!(
        plan_of(&loop_definition, &base_task(), &gains(1.0, 0.2, 0.0)).is_ok(),
        "1000 ms is the longest period"
    );
}

#[test]
fn a_parameter_outside_its_bounds_or_not_representable_is_refused() {
    let task = base_task();
    let failure =
        plan_of(&pid_loop(), &task, &gains(150.0, 0.2, 0.0)).expect_err("a gain above its bound");
    assert_eq!(
        text(&failure),
        "Parameter Main.Kp is missing, out of bounds or not representable."
    );
    let mut missing = gains(1.0, 0.2, 0.0);
    missing.remove("Main.Kp");
    assert!(
        plan_of(&pid_loop(), &task, &missing).is_err(),
        "a missing gain"
    );
    // A REAL parameter holds only single-precision values (C0): 0.1 is not one, 0.5 is.
    let mut real_loop = pid_loop();
    real_loop
        .parameter_binding
        .as_mut()
        .expect("the binding")
        .parameters[0]
        .iec_type = RealType::Real;
    let failure =
        plan_of(&real_loop, &task, &gains(0.1, 0.2, 0.0)).expect_err("0.1 is not a REAL value");
    assert_eq!(
        text(&failure),
        "Parameter Main.Kp is missing, out of bounds or not representable."
    );
    assert!(
        plan_of(&real_loop, &task, &gains(0.5, 0.2, 0.0)).is_ok(),
        "0.5 is a REAL value"
    );
}

/// The PID loop with `extra` further gains, each a parameter of the record with the value 0.
fn loop_with_extra_gains(extra: usize) -> (LoopDefinition, BTreeMap<String, f64>) {
    let mut loop_definition = pid_loop();
    let mut values = gains(1.0, 0.2, 0.0);
    let binding = loop_definition
        .parameter_binding
        .as_mut()
        .expect("the binding");
    for index in 0..extra {
        let symbol = format!("Main.Extra{index}");
        binding.parameters.push(Parameter {
            symbol: symbol.clone(),
            iec_type: RealType::Lreal,
            unit: "1".to_owned(),
            convention: "extra".to_owned(),
            minimum: 0.0,
            maximum: 100.0,
        });
        values.insert(symbol, 0.0);
    }
    (loop_definition, values)
}

#[test]
fn the_observed_channels_are_limited_to_64_including_the_parameters() {
    let (accepted, values) = loop_with_extra_gains(58);
    assert!(
        plan_of(&accepted, &base_task(), &values).is_ok(),
        "64 channels with the signals and the parameters"
    );
    let (refused, values) = loop_with_extra_gains(59);
    let failure = plan_of(&refused, &base_task(), &values).expect_err("65 channels");
    assert_eq!(
        text(&failure),
        "Verification supports at most 64 observed channels, including parameters."
    );
}

#[test]
fn an_actuator_and_the_rates_add_their_checks_and_their_observation() {
    let mut loop_definition = pid_loop();
    loop_definition.signals.actuator = Some(
        decode_value(json!({
            "symbol": "Main.Act",
            "address": "%IL1",
            "iecType": "LREAL",
            "direction": "read",
            "unit": "V",
            "range": { "minimum": -20, "maximum": 20 }
        }))
        .expect("the actuator signal"),
    );
    loop_definition.limits.rate_up_per_s = Some(5.0);
    loop_definition.limits.rate_down_per_s = Some(4.0);
    let plan = plan_of(&loop_definition, &base_task(), &gains(1.0, 0.2, 0.0)).expect("a plan");
    assert_eq!(
        ids(&plan),
        [
            "response-deadline",
            "target-error",
            "process-bounds",
            "output-bounds",
            "requested-output-bounds",
            "output-rate-up",
            "output-rate-down",
            "fixed-parameter:Main.Kp",
            "fixed-parameter:Main.Ki",
            "fixed-parameter:Main.Kd",
        ]
    );
    let channel_of = |id: &str| {
        plan.requirements
            .iter()
            .find(|item| item.id == id)
            .map(|item| item.channel.clone())
    };
    assert_eq!(channel_of("output-bounds").as_deref(), Some("Main.Act"));
    assert_eq!(
        channel_of("requested-output-bounds").as_deref(),
        Some("Main.MV")
    );
    assert_eq!(channel_of("output-rate-up").as_deref(), Some("Main.Act"));
    let rate_up = plan
        .requirements
        .iter()
        .find(|item| item.id == "output-rate-up")
        .expect("a rise rate");
    assert_eq!(
        rate_up.check,
        RunCheck::Rate {
            maximum: 5.0,
            direction: Some(Movement::Up),
        }
    );
    assert_eq!(
        plan.observed_symbols,
        ["Main.MV", "Main.Act", "Main.PV", "Main.SP", "Main.Kp", "Main.Ki", "Main.Kd"]
    );
}

#[test]
fn a_loop_without_a_setpoint_or_a_parameter_binding_is_refused() {
    let mut without_setpoint = pid_loop();
    without_setpoint.signals.setpoint = None;
    let mut without_binding = pid_loop();
    without_binding.parameter_binding = None;
    for loop_definition in [&without_setpoint, &without_binding] {
        let failure =
            plan_of(loop_definition, &base_task(), &gains(1.0, 0.2, 0.0)).expect_err("no binding");
        assert_eq!(
            text(&failure),
            "Declare the parameter and reference bindings first."
        );
        assert_eq!(failure.next_action, NextAction::OpenLoopSetup);
    }
}

#[test]
fn a_model_without_evidence_is_refused_with_the_compatibility_sentence() {
    let model = decode_general_model(&json!({
        "kind": "transfer-function",
        "numerator": [1.5],
        "denominator": [4, 1],
        "delaySeconds": 0.5,
        "revision": "model-4",
        "source": "provided",
        "validation": "unvalidated",
        "evidence": [],
        "inputs": [channel("u", "V")],
        "outputs": [channel("y", "m")]
    }))
    .expect("an unvalidated model");
    let failure = engineer_verification_plan(
        &pid_loop(),
        &model,
        &base_task(),
        &gains(1.0, 0.2, 0.0),
        &BTreeMap::new(),
    )
    .expect_err("a model without evidence");
    assert_eq!(
        text(&failure),
        "The model needs measurement evidence or independent validation before tuning."
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::ModelOrData, NextAction::Identify)
    );
}

#[test]
fn a_target_outside_the_declared_scope_is_refused() {
    let task = EngineerRequirements {
        reference_change: 30.0,
        ..base_task()
    };
    let failure = plan_of(&pid_loop(), &task, &gains(1.0, 0.2, 0.0))
        .expect_err("a target of 30 is outside -20 to 20");
    assert_eq!(
        text(&failure),
        "Target or deadline is outside the declared scope for Main.PV."
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::TaskInfeasible, NextAction::OpenRequirements)
    );
}

#[test]
fn a_repeated_condition_keeps_only_its_first_requirement() {
    let whole = Window {
        start: 0.0,
        end: 30.0,
    };
    let kept = unique_conditions(vec![
        bounds_requirement("first", whole),
        bounds_requirement("repeated", whole),
        bounds_requirement(
            "other-window",
            Window {
                start: 0.0,
                end: 29.0,
            },
        ),
    ]);
    let kept_ids: Vec<&str> = kept.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(kept_ids, ["first", "other-window"]);
}

#[test]
fn a_loop_with_a_parked_feature_is_refused_with_the_gate_failure() {
    // The plan's first step is the Rust engine's gate, so its failure is the one the loop definition returns (A11.2).
    let mut loop_definition = pid_loop();
    loop_definition.cascade = Some(json!({ "outer": "Main.Outer" }));
    let gate = loop_definition
        .rust_engine_support()
        .expect_err("a cascade is parked");
    let failure = plan_of(&loop_definition, &base_task(), &gains(1.0, 0.2, 0.0))
        .expect_err("the plan refuses the parked cascade");
    assert_eq!(failure, gate);
    assert!(
        text(&failure).contains("a cascade"),
        "the cascade is the reason"
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::ControllerCapability, NextAction::None)
    );
}

#[test]
fn a_model_that_is_not_one_input_and_one_output_is_refused_without_channel_bindings() {
    // A two-input state-space model: a loop without channel bindings maps one plant input and one measured output.
    let model = decode_general_model(&json!({
        "revision": "two-inputs",
        "source": "provided",
        "validation": "validated",
        "evidence": ["declared-fixture"],
        "delaySeconds": 0.0,
        "inputs": [channel("heater", "V"), channel("fan", "V")],
        "outputs": [channel("temperature", "m")],
        "states": [channel("thermal", "m")],
        "kind": "continuous-state-space",
        "a": [[-2.0]],
        "b": [[1.0, 3.0]],
        "c": [[2.0]],
        "d": [[0.0, 1.0]]
    }))
    .expect("a two-input model");
    let failure = engineer_verification_plan(
        &pid_loop(),
        &model,
        &base_task(),
        &gains(1.0, 0.2, 0.0),
        &BTreeMap::new(),
    )
    .expect_err("two inputs without channel bindings");
    assert_eq!(
        text(&failure),
        "Map every model input and output in loop channel bindings."
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Implementation, NextAction::OpenChannels)
    );
}

/// Lesson 84's loop as its file declares it, and its supplied two-input heater (`provided-model.json`), with the six values its controller
/// runs with.
fn lesson_84() -> (LoopDefinition, GeneralModel, BTreeMap<String, f64>) {
    let loop_definition = LoopDefinition::decode(include_str!(
        "../../../../examples/control/14-integrated-capstones/84-independent/control-loop.json"
    ))
    .expect("lesson 84 decodes");
    let model = decode_general_model(
        &serde_json::from_str(include_str!(
        "../../../../examples/control/14-integrated-capstones/84-independent/provided-model.json"
    ))
        .expect("the supplied model is JSON"),
    )
    .expect("the supplied model decodes");
    let values = BTreeMap::from([
        ("P1.Parameters.ProportionalGain".to_owned(), 2.0),
        ("P1.Parameters.IntegralGain".to_owned(), 0.5),
        ("P1.Parameters.DerivativeGain".to_owned(), 0.0),
        ("P1.FeedforwardGain".to_owned(), 0.5),
        ("P1.LeadS".to_owned(), 8.0),
        ("P1.LagS".to_owned(), 3.0),
    ]);
    (loop_definition, model, values)
}

#[test]
fn a_two_input_plan_bounds_the_heater_and_starts_the_draw_off_at_its_operating_value() {
    // C8: the heater is the one actuated input, so its bounds and its rates are checked and its identities are plain; the measured
    // draw-off has no check of its own and starts at its operating value (zero), so it holds that value unless a case writes it. The
    // controlled water temperature keeps its response checks, and its setpoint starts at the target (35 °C plus 1 °C).
    let (loop_definition, model, values) = lesson_84();
    let task = engineer_requirements(&json!({
        "referenceChange": 1.0,
        "maximumError": 0.3,
        "settlingBand": 0.1,
        "settlingTimeSeconds": 40.0,
        "durationSeconds": 80.0,
    }))
    .expect("the task decodes");
    let plan =
        engineer_verification_plan(&loop_definition, &model, &task, &values, &BTreeMap::new())
            .expect("the plan of lesson 84");
    assert_eq!(plan.initial_values["P1.HeaterPct"], 75.0);
    assert_eq!(plan.initial_values["P1.LoadKw"], 0.0);
    assert_eq!(plan.initial_values["P1.SetpointC"], 36.0);
    let identities = ids(&plan);
    for id in [
        "response-deadline",
        "target-error",
        "process-bounds",
        "output-bounds",
    ] {
        assert!(identities.contains(&id), "{id} is planned: {identities:?}");
    }
    // With one output and one actuated input, the channel prefix is not used (VA-1); the fixed parameters keep their own prefix
    // whatever the channel count, as `engineerVerificationPlan.ts` names them.
    assert!(
        identities
            .iter()
            .filter(|id| !id.starts_with(FIXED_PARAMETER_PREFIX))
            .all(|id| !id.contains(':')),
        "one output and one actuated input keep their plain identities: {identities:?}"
    );
    assert!(
        identities
            .iter()
            .any(|id| id.starts_with(FIXED_PARAMETER_PREFIX)),
        "the fixed parameters are planned under their prefix: {identities:?}"
    );
    assert!(
        plan.requirements
            .iter()
            .all(|requirement| requirement.channel != "P1.LoadKw"),
        "nothing bounds the measured disturbance (C8)"
    );
    assert!(plan
        .observed_symbols
        .iter()
        .any(|symbol| symbol == "P1.LoadKw"));
}
