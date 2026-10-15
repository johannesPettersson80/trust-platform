// The declared values are exact fixture numbers and the specification's formulas, compared with a tolerance.
#![allow(clippy::float_cmp)]

use super::super::found_mode::{reenter_found_mode, AutoReentry, FoundMode};
use super::*;
use crate::commissioning::entry_allowance::{noise_aware_transfer_tolerance, AllowanceTerms};
use crate::commissioning::fixture_loops::{
    control_pid_loop, first_order_model, project_pi_loop_on_p1, requirements,
};
use crate::commissioning::library_pid::library_pid_initialization;
use crate::commissioning::profile::RequiredTracking;
use crate::commissioning::transfer::{TransferMember, TransferValueType};
use crate::commissioning::transfer_tracking::TransferTracking;
use crate::loop_definition::{
    Direction, GainSchedule, InitialSource, InitialValue, InitializedState, LoopSignal,
    PidInitialization, PostScan, Range, SchedulePoint, SchedulingVariable, StateType,
};
use crate::validation::run_validation::RunCheck;

const OWN: &str = "Main.TankController.StateValue.ProcessValue";

fn close(actual: f64, expected: f64, tolerance: f64) -> bool {
    (actual - expected).abs() <= tolerance
}

fn seven_states() -> Vec<InitializedState> {
    let state = |member: &str, unit: &str, initial: InitialValue| InitializedState {
        symbol: format!("Main.TankController.StateValue.{member}"),
        iec_type: StateType::Lreal,
        unit: unit.to_owned(),
        minimum: -10_000.0,
        maximum: 10_000.0,
        initial,
        post_scan: PostScan::Bounded,
    };
    let output = || InitialValue::From(InitialSource::ModelOutput { index: 0 });
    let input = || InitialValue::From(InitialSource::ModelInput { index: 0 });
    vec![
        state("Setpoint", "%", output()),
        state("SetpointRate", "%/s", InitialValue::Constant(0.0)),
        state("ProcessValue", "%", output()),
        state("ProcessValueRate", "%/s", InitialValue::Constant(0.0)),
        state("FeedForward", "%", InitialValue::Constant(0.0)),
        state("NominalOutput", "%", input()),
        state("SaturatedOutput", "%", input()),
    ]
}

/// The fixture loop as a `Control.PID` of the shipped library: its seven states, and its process value its own measurement.
fn library_loop() -> LoopDefinition {
    let mut loop_definition = control_pid_loop();
    loop_definition.signals.process_variable = LoopSignal {
        symbol: OWN.to_owned(),
        address: None,
        iec_type: "LREAL".to_owned(),
        direction: Direction::Read,
        unit: "%".to_owned(),
        range: Range {
            minimum: 0.0,
            maximum: 100.0,
        },
    };
    loop_definition.controller.pid_initialization = Some(PidInitialization::ControlPid {
        maximum_snapshot_age_ms: 100,
        states: seven_states(),
        application_states: None,
    });
    loop_definition
}

fn library_definition(loop_definition: &LoopDefinition) -> ControllerInitialization {
    library_pid_initialization(loop_definition).expect("the seven states are bound")
}

fn parameters() -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("Main.Parameters.Kp".to_owned(), 1.0),
        ("Main.Parameters.Ki".to_owned(), 0.0),
        ("Main.Parameters.Kd".to_owned(), 0.0),
    ])
}

fn gains(kp: f64, ki: f64, kd: f64) -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("Main.Parameters.Kp".to_owned(), kp),
        ("Main.Parameters.Ki".to_owned(), ki),
        ("Main.Parameters.Kd".to_owned(), kd),
    ])
}

fn declared_tracking(tracking: TransferTracking) -> DeclaredStateTracking {
    match tracking {
        TransferTracking::DeclaredState(declared) => declared,
        other => panic!("the tracking is declared-state-v1: {other:?}"),
    }
}

#[test]
fn a_control_pid_loop_found_in_auto_returns_to_auto_through_its_declared_initialization_allowing_the_measured_noise(
) {
    // TypeScript: control-studio-found-mode.test.ts, line 323: "a Control.PID loop found in Auto returns to Auto through its
    // declared initialization, allowing Apply's measured noise" (A6.2, A6.4, B6.4). The restored gains are Kp 1, Ki 0 and Kd 0,
    // and the measured noise is 0.05, so the allowance is 2·Kp·n.
    let loop_definition = library_loop();
    let model = first_order_model();
    let definition = library_definition(&loop_definition);
    let profile = declared_commissioning_profile(
        &loop_definition,
        &model,
        &definition,
        "Control.PID",
        &parameters(),
    )
    .expect("the profile");
    let binding = profile.binding().expect("the binding");
    let tracking = declared_tracking(profile.tracking(&binding, true).expect("the Auto entry"));
    let noise = 0.05;
    let tolerance = noise_aware_transfer_tolerance(
        &loop_definition,
        None,
        Some(1.0),
        Some(noise),
        AllowanceTerms {
            integral_gain: Some(0.0),
            derivative_gain: Some(0.0),
            cycle_seconds: Some(0.01),
            entry_error: None,
        },
    )
    .expect("the allowance");
    // The loop reads Manual, as after an Apply that left it in Manual, so the return to Auto is due.
    let current = BTreeMap::from([(
        binding.mode.symbol.clone(),
        TransferValue::Boolean(binding.mode.manual_value),
    )]);
    let entry = reenter_found_mode(Some(FoundMode::Auto), &binding.mode, &current, |_| {
        Some(AutoReentry {
            values: tracking.initialization_values.clone(),
            tolerance,
            tracking: TransferTracking::DeclaredState(tracking.clone()),
        })
    })
    .expect("the loop returns to Auto");
    let TransferTracking::DeclaredState(entered) = &entry.tracking else {
        panic!("the entry goes through the declared initialization");
    };
    assert_eq!(
        entered.kind,
        DeclaredStateKind::DeclaredStateV1,
        "the tracking kind is declared-state-v1"
    );
    assert!(
        entered.align_output,
        "the entry starts from the current output"
    );
    assert!(
        close(entry.tolerance, 2.0 * 1.0 * noise, 1e-12),
        "A6.4 for Kp 1, Ki 0, Kd 0 allows 2·Kp·n = {}, got {}",
        2.0 * 1.0 * noise,
        entry.tolerance
    );
}

#[test]
fn the_declared_binding_writes_the_setpoint_the_manual_command_and_the_initialized_states_and_reads_the_process_value(
) {
    let loop_definition = library_loop();
    let definition = library_definition(&loop_definition);
    let binding = declared_transfer_binding(&loop_definition, &definition).expect("the binding");
    let states: Vec<&str> = binding
        .states
        .iter()
        .map(|member| member.symbol.as_str())
        .collect();
    assert_eq!(
        states.len(),
        8,
        "the setpoint, the Manual command, and the six initialized states"
    );
    assert_eq!(states[0], "LevelSP");
    assert_eq!(states[1], "Main.ManualCommand");
    assert!(
        !states.contains(&OWN),
        "the process value is read, never initialized"
    );
    assert!(
        binding.guards.iter().any(|guard| guard.symbol == OWN),
        "the process value is a guard"
    );
    assert_eq!(binding.output.symbol, "ValveMV");
    assert_eq!(
        binding.tracking_outputs.get("ValveMV").map(String::as_str),
        Some("ValvePosition"),
        "the measured actuator is the feedback"
    );
}

#[test]
fn a_library_profile_declares_the_formulas_of_its_states_and_requires_the_tracking_that_reads_them()
{
    let loop_definition = library_loop();
    let definition = library_definition(&loop_definition);
    let profile = declared_commissioning_profile(
        &loop_definition,
        &first_order_model(),
        &definition,
        "Control.PID",
        &parameters(),
    )
    .expect("the profile");
    assert_eq!(
        profile.required_tracking,
        vec![RequiredTracking::DeclaredInitializationFormulas]
    );
    let binding = profile.binding().expect("the binding");
    let tracking = declared_tracking(profile.tracking(&binding, true).expect("the tracking"));
    let formulas = tracking.initialization_formulas.expect("the formulas");
    let names: Vec<&str> = formulas.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        vec![
            "Main.TankController.StateValue.NominalOutput",
            "Main.TankController.StateValue.SaturatedOutput",
            "Main.TankController.StateValue.Setpoint",
        ],
        "the states that come from the output, the input and the setpoint have a formula; the measured process value has none"
    );
    assert_eq!(
        formulas["Main.TankController.StateValue.Setpoint"],
        InitializationFormula::ModelOutput {
            process_variable: OWN.to_owned()
        }
    );
    assert_eq!(
        formulas["Main.TankController.StateValue.NominalOutput"],
        InitializationFormula::ModelInput {
            output_symbol: "ValveMV".to_owned()
        }
    );
}

#[test]
fn an_auto_entry_tracking_carries_the_operating_point_values_of_the_initialized_states_and_aligns_the_output(
) {
    let loop_definition = library_loop();
    let definition = library_definition(&loop_definition);
    let profile = declared_commissioning_profile(
        &loop_definition,
        &first_order_model(),
        &definition,
        "Control.PID",
        &parameters(),
    )
    .expect("the profile");
    let binding = profile.binding().expect("the binding");
    let tracking = declared_tracking(profile.tracking(&binding, true).expect("the tracking"));
    assert_eq!(tracking.kind, DeclaredStateKind::DeclaredStateV1);
    assert_eq!(tracking.reference_symbol, "LevelSP");
    assert_eq!(tracking.manual_command_symbol, "Main.ManualCommand");
    assert_eq!(tracking.process_variable, OWN);
    assert!(tracking.align_output);
    assert!(
        close(
            tracking.initialization_values["Main.TankController.StateValue.Setpoint"],
            2.0,
            1e-12
        ),
        "the output at the operating point"
    );
    assert!(
        close(
            tracking.initialization_values["Main.TankController.StateValue.NominalOutput"],
            0.0,
            1e-12
        ),
        "the input at the operating point"
    );
    assert!(
        !tracking.initialization_values.contains_key(OWN),
        "the process value is not initialized"
    );
    assert_eq!(
        tracking.bounded_state_symbols.len(),
        6,
        "every initialized state is bounded after the entry"
    );
    assert_eq!(
        tracking.additional_measurements,
        vec![OWN.to_owned()],
        "a loop without channels lists its process value as its measurement"
    );
    assert!(
        tracking.additional_references.is_empty() && tracking.additional_commands.is_empty(),
        "a loop without channels has no further channels"
    );
}

#[test]
fn a_later_target_change_does_not_align_the_output_and_carries_the_midpoints_of_the_state_ranges() {
    let loop_definition = library_loop();
    let definition = library_definition(&loop_definition);
    let profile = declared_commissioning_profile(
        &loop_definition,
        &first_order_model(),
        &definition,
        "Control.PID",
        &parameters(),
    )
    .expect("the profile");
    let binding = profile.binding().expect("the binding");
    let tracking = declared_tracking(profile.tracking(&binding, false).expect("the tracking"));
    assert!(
        !tracking.align_output,
        "the target change keeps the retained state"
    );
    for value in tracking.initialization_values.values() {
        assert!(
            close(*value, 0.0, 1e-12),
            "the midpoint of −10000..10000 is 0"
        );
    }
}

#[test]
fn the_start_plan_holds_the_setpoint_and_the_command_and_checks_the_transfer_after_its_third_period(
) {
    let loop_definition = library_loop();
    let definition = library_definition(&loop_definition);
    let plan = declared_initialization_plan(
        &loop_definition,
        &first_order_model(),
        &requirements(),
        &parameters(),
        &definition,
        None,
    )
    .expect("the plan");
    assert!(
        close(plan.reference, 2.0, 1e-12),
        "the reference is the operating output"
    );
    assert!(
        close(plan.tolerance, 1e-4, 1e-12),
        "the exact check without noise"
    );
    let run = &plan.run;
    assert_eq!(run.step_ms, 10);
    assert_eq!(run.duration_ms, 60);
    assert_eq!(run.max_total_cycles, 7);
    assert_eq!(
        run.events.len(),
        9,
        "the setpoint, the command, six states, and the mode"
    );
    assert!(
        run.events.iter().all(|event| event.at_ms == 30),
        "the transfer is after three periods"
    );
    let value_at = |symbol: &str| {
        run.events
            .iter()
            .find(|event| event.symbol == symbol)
            .map(|event| event.value)
    };
    assert_eq!(value_at("LevelSP"), Some(2.0));
    assert_eq!(value_at("Main.ManualCommand"), Some(0.0));
    assert_eq!(
        value_at("Main.Auto"),
        Some(1.0),
        "the mode enters Auto at the transfer"
    );
    assert_eq!(
        run.initial_values.get("Main.Auto"),
        Some(&0.0),
        "the run starts in Manual"
    );
    let continuity = plan
        .requirements
        .iter()
        .find(|requirement| requirement.id == "transfer-continuity")
        .expect("the continuity requirement");
    match continuity.check {
        RunCheck::Bounds { minimum, maximum } => {
            assert!(
                close(minimum, -1e-4, 1e-12) && close(maximum, 1e-4, 1e-12),
                "within the allowance of the operating command 0"
            );
        }
        ref other => panic!("the continuity is a bounds check: {other:?}"),
    }
    let auto = plan
        .requirements
        .iter()
        .find(|requirement| requirement.id == "auto-mode")
        .expect("the Auto requirement");
    assert!(
        close(auto.window.start, 0.03, 1e-12),
        "the Auto mode is checked from the transfer"
    );
    let state = plan
        .requirements
        .iter()
        .find(|requirement| {
            requirement.id == "initialized-state:Main.TankController.StateValue.Setpoint"
        })
        .expect("the initialized state requirement");
    assert!(
        matches!(state.check, RunCheck::StateBounds { minimum, maximum } if close(minimum, -10_000.0, 1e-9) && close(maximum, 10_000.0, 1e-9))
    );
}

#[test]
fn the_start_allowance_is_the_noise_allowance_of_a6_4_for_the_restored_gains() {
    let plan = declared_initialization_plan(
        &library_loop(),
        &first_order_model(),
        &requirements(),
        &gains(1.0, 0.0, 0.0),
        &library_definition(&library_loop()),
        Some(0.05),
    )
    .expect("the plan");
    assert!(close(plan.tolerance, 0.1, 1e-9), "2·Kp·n = 2 · 1 · 0.05");
}

#[test]
fn a_declared_state_that_repeats_the_manual_command_is_refused_as_an_overlap() {
    let loop_definition = library_loop();
    let definition = ControllerInitialization::new(vec![InitializedState {
        symbol: "Main.ManualCommand".to_owned(),
        iec_type: StateType::Lreal,
        unit: "%".to_owned(),
        minimum: 0.0,
        maximum: 100.0,
        initial: InitialValue::Constant(0.0),
        post_scan: PostScan::Bounded,
    }])
    .expect("the declaration is valid");
    let refusal =
        declared_transfer_binding(&loop_definition, &definition).expect_err("the roles overlap");
    assert!(refusal.message.contains("overlap"));
}

#[test]
fn the_reported_states_are_read_back_and_a_missing_one_is_refused() {
    let loop_definition = library_loop();
    let definition = library_definition(&loop_definition);
    let binding = declared_transfer_binding(&loop_definition, &definition).expect("the binding");
    let mut observed: BTreeMap<String, TransferValue> = binding
        .states
        .iter()
        .map(|member| (member.symbol.clone(), TransferValue::Number(1.0)))
        .collect();
    let read = declared_states(&binding, &observed).expect("every state is reported");
    assert!(close(read["LevelSP"], 1.0, 1e-12));
    observed.remove("Main.ManualCommand");
    assert!(
        declared_states(&binding, &observed).is_err(),
        "a state the runtime does not report is refused"
    );
    let member = TransferMember {
        symbol: "Main.Flag".to_owned(),
        iec_type: TransferValueType::Bool,
        unit: "1".to_owned(),
        minimum: 0.0,
        maximum: 1.0,
    };
    assert!(close(
        rounded_transfer_value(&member, 1.0).expect("a level"),
        1.0,
        1e-12
    ));
}

/// The schedule of the fixture's loop: it tunes `Low`, whose gains the fixture names, beside a second point `High` with its own gains.
fn schedule_of_the_fixture() -> GainSchedule {
    let point = |name: &str, value: f64, proportional: &str, integral: &str| SchedulePoint {
        name: name.to_owned(),
        loop_id: format!("loop:{}", name.to_lowercase()),
        value,
        tolerance: 0.01,
        proportional_symbol: proportional.to_owned(),
        integral_symbol: integral.to_owned(),
    };
    GainSchedule {
        parameter_record: "Main.Record".to_owned(),
        scheduling: SchedulingVariable {
            symbol: "P1.Level".to_owned(),
            unit: "m".to_owned(),
            iec_type: crate::loop_definition::RealType::Lreal,
        },
        point: "Low".to_owned(),
        points: vec![
            point("Low", 0.09, "P1.Kp", "P1.Ki"),
            point("High", 0.49, "P1.KpHigh", "P1.KiHigh"),
        ],
    }
}

#[test]
fn a_schedule_states_its_table_and_takes_its_own_gains_out_of_the_guards() {
    let mut loop_definition = project_pi_loop_on_p1();
    loop_definition
        .controller
        .project
        .as_mut()
        .expect("a project law")
        .schedule = Some(schedule_of_the_fixture());
    let project = loop_definition
        .controller
        .project
        .as_ref()
        .expect("a project law");
    let binding = declared_transfer_binding(&loop_definition, &project.initialization)
        .expect("the schedule's binding");
    let states: Vec<&str> = binding
        .states
        .iter()
        .map(|member| member.symbol.as_str())
        .collect();
    assert!(states.ends_with(&["P1.Kp", "P1.Ki", "P1.KpHigh", "P1.KiHigh"]));
    assert!(binding
        .guards
        .iter()
        .all(|guard| guard.symbol != "P1.Kp" && guard.symbol != "P1.Ki"));
}
