// The written values are the fixture's own gains, which the rules keep unchanged, so they are compared as they are.
#![allow(clippy::float_cmp)]

use super::*;
use crate::commissioning::fixture_loops::{
    control_pid_loop, first_order_model, parallel_pid_loop, parked_project_loop, project_p_loop,
    project_p_loop_with_formula_application, project_p_loop_with_law_state,
    project_p_loop_with_startup, project_pid_loop, proportional_loop, requirements,
};
use crate::loop_definition::{AntiWindup, AntiWindupMethod, Parameter, ProjectSeedBlock};
use crate::tuning::project_seed::ParallelPidSeed;
use serde_json::json;

/// The gains of the connected P (C13): the proportional gain alone.
fn proportional(kp: f64) -> ControllerGains {
    ControllerGains {
        proportional_gain: Some(kp),
        ..ControllerGains::default()
    }
}

/// The gains of a parallel PID or `Control.PID` record: the three gains.
fn three_gains(kp: f64, ki: f64, kd: f64) -> ControllerGains {
    ControllerGains {
        proportional_gain: Some(kp),
        integral_gain: Some(ki),
        derivative_gain: Some(kd),
        ..ControllerGains::default()
    }
}

/// The observed values that an adapter keeps: one unrelated variable.
fn observed() -> BTreeMap<String, f64> {
    BTreeMap::from([("Main.Observed".to_owned(), 7.0)])
}

/// The three gain values of the parallel PID fixture, as a plan takes them.
fn pid_values(kp: f64, ki: f64, kd: f64) -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("Main.Parameters.Kp".to_owned(), kp),
        ("Main.Parameters.Ki".to_owned(), ki),
        ("Main.Parameters.Kd".to_owned(), kd),
    ])
}

/// The shipped `Control.PID` adapter of a loop, with an empty declared list: the materialize rules do not read the states.
fn control_pid_adapter(loop_definition: &LoopDefinition) -> ControllerAdapter<'_> {
    ControllerAdapter {
        loop_definition,
        kind: AdapterKind::ControlPid(
            ControllerInitialization::new(Vec::new()).expect("an empty state list is valid"),
        ),
    }
}

/// The refusal of a loop, mutated after the parallel PID fixture is decoded, when its adapter is chosen.
fn refusal_after(mutate: impl FnOnce(&mut LoopDefinition)) -> Failure {
    let mut loop_definition = parallel_pid_loop();
    mutate(&mut loop_definition);
    controller_adapter(&loop_definition).expect_err("the loop has no adapter here")
}

/// The parameter of the binding with `symbol`, for a test that changes its type or bounds.
fn parameter_mut<'a>(loop_definition: &'a mut LoopDefinition, symbol: &str) -> &'a mut Parameter {
    loop_definition
        .parameter_binding
        .as_mut()
        .expect("the fixture has a binding")
        .parameters
        .iter_mut()
        .find(|parameter| parameter.symbol == symbol)
        .expect("the fixture declares the parameter")
}

#[test]
fn a_parallel_pid_law_is_the_parallel_pid_adapter_with_the_pid_structures() {
    let loop_definition = parallel_pid_loop();
    let adapter =
        controller_adapter(&loop_definition).expect("the parallel PID fixture is a valid adapter");
    assert_eq!(adapter.kind(), &AdapterKind::ParallelPid);
    assert_eq!(
        adapter.families(),
        &[Structure::P, Structure::PI, Structure::PD, Structure::PID]
    );
    assert!(
        adapter.check_initialization().is_ok(),
        "the fixture declares its parallel PID contract"
    );
}

#[test]
fn a_control_pid_whose_states_do_not_fit_the_loop_is_refused_at_selection() {
    // libraryPidInitialization runs at the adapter's choice (C1): the loop has no seven states, so no adapter is offered.
    let loop_definition = control_pid_loop();
    assert!(controller_adapter(&loop_definition).is_err());
}

#[test]
fn a_proportional_law_is_the_connected_p_with_the_proportional_family_only() {
    let loop_definition = proportional_loop();
    let adapter =
        controller_adapter(&loop_definition).expect("the proportional fixture is a valid adapter");
    assert_eq!(adapter.kind(), &AdapterKind::ConnectedP);
    assert_eq!(adapter.families(), &[Structure::P]);
    assert!(adapter.check_initialization().is_ok());
}

#[test]
fn a_loop_without_a_law_is_named_as_not_a_recognised_adapter() {
    let mut loop_definition = control_pid_loop();
    loop_definition.controller.law = None;
    let refusal = controller_adapter(&loop_definition).expect_err("no law, no adapter");
    let text = full_text(&refusal);
    assert!(
        text.starts_with("Controller type Control.PID is not a recognised ST adapter."),
        "{text}"
    );
    assert_eq!(refusal.next_action, NextAction::OpenLoopSetup);
}

/// The project adapter of the own-law loop, with its block's seed replaced by the seed given, so that the block and the seed agree.
fn project_adapter(
    loop_definition: &LoopDefinition,
    seed: ParallelPidSeed,
) -> ControllerAdapter<'_> {
    let mut project = loop_definition
        .controller
        .project
        .clone()
        .expect("the fixture has a project block");
    project.seed = Some(ProjectSeedBlock::ParallelPid(seed.clone()));
    ControllerAdapter {
        loop_definition,
        kind: AdapterKind::ProjectPid {
            project: Box::new(project),
            seed,
        },
    }
}

/// The seed of a program's PD law: a proportional and a derivative variable, and no integral one.
fn pd_seed() -> ParallelPidSeed {
    ParallelPidSeed {
        family: Structure::PD,
        proportional_symbol: "Main.Program.Kp".to_owned(),
        integral_symbol: None,
        derivative_symbol: Some("Main.Program.Kd".to_owned()),
    }
}

#[test]
fn every_parked_family_is_refused_by_its_name_and_no_adapter_is_offered() {
    // The parked features that remain (checklist V7): a program's law without a served seed, the channels of a PID, and the two-loop
    // cascade. State feedback, ON/OFF, groups and the served seeds have their own adapters (C3, C4, C7), so they are not listed here.
    let project = controller_adapter(&parked_project_loop())
        .expect_err("a program's law with a parked seed has no adapter here");
    assert!(
        full_text(&project).contains("the program's own law"),
        "{}",
        full_text(&project)
    );
    let channels = refusal_after(|loop_definition| {
        loop_definition.channels = Some(crate::loop_definition::LoopChannels {
            inputs: Vec::new(),
            outputs: Vec::new(),
            initial_state: None,
        });
    });
    assert!(full_text(&channels).contains("channels"));
    let cascade = refusal_after(|loop_definition| {
        loop_definition.cascade = Some(json!({}));
    });
    assert!(
        full_text(&cascade).contains("a cascade"),
        "{}",
        full_text(&cascade)
    );
    assert_eq!(project.next_action, NextAction::None);
    assert_eq!(project.cause, Cause::ControllerCapability);
}

#[test]
fn the_connected_p_writes_its_gain_and_keeps_the_observed_values() {
    let loop_definition = proportional_loop();
    let adapter = controller_adapter(&loop_definition).expect("valid");
    let values = adapter
        .materialize(&proportional(2.5), &observed())
        .expect("in bounds");
    assert_eq!(values.get("Main.Parameters.Kp").copied(), Some(2.5));
    assert_eq!(values.get("Main.Observed").copied(), Some(7.0));
}

#[test]
fn a_real_connected_gain_is_rounded_to_single_precision_before_it_is_written() {
    // C0: a REAL value is rounded to single precision before it is submitted.
    let mut loop_definition = proportional_loop();
    parameter_mut(&mut loop_definition, "Main.Parameters.Kp").iec_type = RealType::Real;
    let adapter = controller_adapter(&loop_definition).expect("valid");
    let values = adapter
        .materialize(&proportional(0.1), &BTreeMap::new())
        .expect("in bounds");
    let written = values
        .get("Main.Parameters.Kp")
        .copied()
        .expect("the gain is written");
    assert!((written - f64::from(0.1_f32)).abs() <= 1e-15, "{written}");
}

#[test]
fn the_connected_p_refuses_an_integral_or_derivative_gain_and_accepts_explicit_zeros() {
    let loop_definition = proportional_loop();
    let adapter = controller_adapter(&loop_definition).expect("valid");
    let with_integral = ControllerGains {
        integral_gain: Some(0.2),
        ..proportional(2.0)
    };
    let refusal = adapter
        .materialize(&with_integral, &BTreeMap::new())
        .expect_err("the P law has no integral term");
    assert_eq!(
        refusal.message,
        "The bound proportional ST law cannot realize integral or derivative gains."
    );
    assert_eq!(refusal.next_action, NextAction::ChooseMethod);
    let explicit_zero = ControllerGains {
        integral_gain: Some(0.0),
        derivative_gain: Some(0.0),
        ..proportional(2.0)
    };
    assert!(adapter
        .materialize(&explicit_zero, &BTreeMap::new())
        .is_ok());
}

#[test]
fn a_gain_is_bounded_by_its_magnitude_and_a_gain_outside_it_is_refused() {
    // N6.4: the bounds of a gain apply to its magnitude, so a negative gain of the same magnitude is written.
    let loop_definition = proportional_loop();
    let adapter = controller_adapter(&loop_definition).expect("valid");
    let negative = adapter
        .materialize(&proportional(-50.0), &BTreeMap::new())
        .expect("magnitude 50 is in [-100, 100]");
    assert_eq!(negative.get("Main.Parameters.Kp").copied(), Some(-50.0));
    let too_large = adapter
        .materialize(&proportional(-150.0), &BTreeMap::new())
        .expect_err("magnitude 150 is above 100");
    assert!(
        too_large.message.starts_with("Realized proportional gain"),
        "{}",
        too_large.message
    );
    assert_eq!(too_large.next_action, NextAction::Tune);
}

#[test]
fn a_missing_proportional_gain_names_the_connected_p_adapter() {
    let loop_definition = proportional_loop();
    let adapter = controller_adapter(&loop_definition).expect("valid");
    let refusal = adapter
        .materialize(&ControllerGains::default(), &BTreeMap::new())
        .expect_err("a gain is required");
    assert_eq!(
        refusal.message,
        "The proportional ST law needs a proportional gain."
    );
}

#[test]
fn the_parallel_pid_writes_its_three_gains_and_keeps_the_observed_values() {
    let loop_definition = parallel_pid_loop();
    let adapter = controller_adapter(&loop_definition).expect("valid");
    let values = adapter
        .materialize(&three_gains(2.0, 0.5, 0.1), &observed())
        .expect("in bounds");
    assert_eq!(values.get("Main.Parameters.Kp").copied(), Some(2.0));
    assert_eq!(values.get("Main.Parameters.Ki").copied(), Some(0.5));
    assert_eq!(values.get("Main.Parameters.Kd").copied(), Some(0.1));
    assert_eq!(values.get("Main.Observed").copied(), Some(7.0));
}

#[test]
fn an_absent_integral_or_derivative_gain_is_written_as_an_explicit_zero() {
    let loop_definition = parallel_pid_loop();
    let adapter = controller_adapter(&loop_definition).expect("valid");
    let values = adapter
        .materialize(&proportional(2.0), &BTreeMap::new())
        .expect("in bounds");
    assert_eq!(values.get("Main.Parameters.Ki").copied(), Some(0.0));
    assert_eq!(values.get("Main.Parameters.Kd").copied(), Some(0.0));
}

#[test]
fn a_control_pid_and_a_parallel_pid_name_their_own_missing_gain() {
    let loop_definition = parallel_pid_loop();
    let parallel = controller_adapter(&loop_definition).expect("valid");
    let missing_parallel = parallel
        .materialize(&ControllerGains::default(), &BTreeMap::new())
        .expect_err("Kp is required");
    assert_eq!(
        missing_parallel.message,
        "A proportional gain is required by the declared PID record."
    );
    let shipped = control_pid_adapter(&loop_definition);
    let missing_shipped = shipped
        .materialize(&ControllerGains::default(), &BTreeMap::new())
        .expect_err("Kp is required");
    assert_eq!(
        missing_shipped.message,
        "A proportional gain is required by Control.PID."
    );
}

#[test]
fn a_back_calculation_gain_needs_a_declared_anti_windup_term() {
    let loop_definition = parallel_pid_loop();
    let adapter = controller_adapter(&loop_definition).expect("valid");
    let gains = ControllerGains {
        back_calculation_gain: Some(0.5),
        ..three_gains(2.0, 0.5, 0.1)
    };
    let refusal = adapter
        .materialize(&gains, &BTreeMap::new())
        .expect_err("the fixture law has no anti-windup");
    assert_eq!(
        refusal.message,
        "A typed PID anti-windup adapter is required before materializing a back-calculation gain."
    );
}

#[test]
fn a_declared_anti_windup_term_takes_the_back_calculation_gain() {
    let mut loop_definition = parallel_pid_loop();
    loop_definition
        .parameter_binding
        .as_mut()
        .expect("binding")
        .parameters
        .push(Parameter {
            symbol: "Main.Parameters.Tracking".to_owned(),
            iec_type: RealType::Lreal,
            unit: "1/s".to_owned(),
            convention: "tracking gain".to_owned(),
            minimum: 0.0,
            maximum: 10.0,
        });
    if let Some(ControllerLaw::Pid(law)) = loop_definition.controller.law.as_mut() {
        law.anti_windup = Some(AntiWindup {
            method: AntiWindupMethod::BackCalculation,
            tracking_gain_symbol: "Main.Parameters.Tracking".to_owned(),
        });
    }
    let adapter = controller_adapter(&loop_definition).expect("valid");
    let gains = ControllerGains {
        back_calculation_gain: Some(0.5),
        ..three_gains(2.0, 0.5, 0.1)
    };
    let values = adapter
        .materialize(&gains, &BTreeMap::new())
        .expect("in bounds");
    assert_eq!(values.get("Main.Parameters.Tracking").copied(), Some(0.5));
}

#[test]
fn a_non_neutral_setpoint_weight_needs_a_declared_weight_term_and_a_weight_of_one_does_not() {
    let loop_definition = parallel_pid_loop();
    let adapter = controller_adapter(&loop_definition).expect("valid");
    let weighted = ControllerGains {
        proportional_setpoint_weight: Some(0.5),
        ..three_gains(2.0, 0.5, 0.1)
    };
    let refusal = adapter
        .materialize(&weighted, &BTreeMap::new())
        .expect_err("the law declares no weight");
    assert_eq!(refusal.message, "A declared proportional setpoint weight is required before materializing a weighted controller.");
    let unweighted = ControllerGains {
        proportional_setpoint_weight: Some(1.0),
        ..three_gains(2.0, 0.5, 0.1)
    };
    assert!(adapter.materialize(&unweighted, &BTreeMap::new()).is_ok());
}

#[test]
fn a_filter_time_needs_a_declared_filter_term_and_zero_needs_none() {
    let loop_definition = parallel_pid_loop();
    let adapter = controller_adapter(&loop_definition).expect("valid");
    let filtered = ControllerGains {
        measurement_filter_seconds: Some(0.5),
        ..three_gains(2.0, 0.5, 0.1)
    };
    let refusal = adapter
        .materialize(&filtered, &BTreeMap::new())
        .expect_err("the law declares no filter");
    assert_eq!(refusal.message, "A declared measurement filter time is required before materializing a filtered controller.");
    let unfiltered = ControllerGains {
        measurement_filter_seconds: Some(0.0),
        ..three_gains(2.0, 0.5, 0.1)
    };
    assert!(adapter.materialize(&unfiltered, &BTreeMap::new()).is_ok());
}

#[test]
fn a_pid_gain_outside_its_binding_is_refused_and_a_gain_is_bounded_by_magnitude() {
    let loop_definition = parallel_pid_loop();
    let adapter = controller_adapter(&loop_definition).expect("valid");
    let negative = adapter
        .materialize(&three_gains(-50.0, 0.5, 0.1), &BTreeMap::new())
        .expect("magnitude 50 is in [-100, 100]");
    assert_eq!(negative.get("Main.Parameters.Kp").copied(), Some(-50.0));
    let refusal = adapter
        .materialize(&three_gains(150.0, 0.5, 0.1), &BTreeMap::new())
        .expect_err("150 is above the bound");
    assert!(
        refusal
            .message
            .starts_with("PID gain for Main.Parameters.Kp is outside its declared binding"),
        "{}",
        refusal.message
    );
    assert_eq!(refusal.next_action, NextAction::Tune);
}

#[test]
fn a_real_pid_value_is_single_precision_and_is_checked_again() {
    let mut loop_definition = parallel_pid_loop();
    parameter_mut(&mut loop_definition, "Main.Parameters.Kp").iec_type = RealType::Real;
    let adapter = controller_adapter(&loop_definition).expect("valid");
    let values = adapter
        .materialize(&three_gains(0.1, 0.0, 0.0), &BTreeMap::new())
        .expect("in bounds");
    let written = values.get("Main.Parameters.Kp").copied().expect("written");
    assert!((written - f64::from(0.1_f32)).abs() <= 1e-15, "{written}");
}

#[test]
fn a_parallel_pid_without_its_explicit_contract_cannot_start_auto() {
    let mut loop_definition = parallel_pid_loop();
    loop_definition.controller.pid_initialization = None;
    let adapter = controller_adapter(&loop_definition)
        .expect("the choice of the adapter does not read the contract");
    assert_eq!(adapter.kind(), &AdapterKind::ParallelPid);
    assert!(
        adapter.check_initialization().is_err(),
        "Auto needs the parallel-pid-v1 contract"
    );
}

#[test]
fn a_parallel_pid_start_is_planned_as_a_parallel_pid_start() {
    let loop_definition = parallel_pid_loop();
    let adapter = controller_adapter(&loop_definition).expect("valid");
    let plan = adapter
        .initialization_plan(
            &first_order_model(),
            &requirements(),
            &pid_values(1.0, 0.0, 0.0),
            None,
        )
        .expect("the fixture starts");
    assert!(matches!(plan, AdapterStartPlan::ParallelPid(_)), "{plan:?}");
}

#[test]
fn a_connected_p_start_is_planned_as_a_single_start() {
    let loop_definition = proportional_loop();
    let adapter = controller_adapter(&loop_definition).expect("valid");
    let values = BTreeMap::from([("Main.Parameters.Kp".to_owned(), 2.0)]);
    let plan = adapter
        .initialization_plan(&first_order_model(), &requirements(), &values, Some(0.05))
        .expect("the fixture starts");
    assert!(matches!(plan, AdapterStartPlan::Start(_)), "{plan:?}");
}

#[test]
fn a_parallel_pid_declares_no_commissioning_profile_and_has_its_recovery_tracking() {
    let loop_definition = parallel_pid_loop();
    let adapter = controller_adapter(&loop_definition).expect("valid");
    assert!(adapter
        .declared_profile(&first_order_model(), &BTreeMap::new())
        .expect("no profile to fail")
        .is_none());
    assert!(matches!(
        adapter.recovery_tracking(&first_order_model()),
        Ok(TransferTracking::ParallelPid(_))
    ));
}

#[test]
fn a_program_pid_law_is_the_project_adapter_with_all_four_families() {
    let loop_definition = project_pid_loop();
    let adapter = controller_adapter(&loop_definition).expect("the PID seed is served");
    assert!(
        matches!(adapter.kind(), AdapterKind::ProjectPid { project, .. } if project.family == "PID"),
        "{:?}",
        adapter.kind()
    );
    assert_eq!(
        adapter.families(),
        vec![Structure::P, Structure::PI, Structure::PD, Structure::PID]
    );
    assert!(
        adapter.check_initialization().is_ok(),
        "the declared states and the seed's variables are bound"
    );
}

#[test]
fn a_program_pid_law_declares_a_profile_labelled_with_its_family_and_recovers_through_its_states() {
    let loop_definition = project_pid_loop();
    let adapter = controller_adapter(&loop_definition).expect("valid");
    let profile = adapter
        .declared_profile(&first_order_model(), &BTreeMap::new())
        .expect("the profile holds at the operating point")
        .expect("a program's PID declares a profile");
    assert_eq!(profile.label, "PID");
    assert!(matches!(
        adapter.recovery_tracking(&first_order_model()),
        Ok(TransferTracking::DeclaredState(_))
    ));
}

#[test]
fn a_program_pid_law_transfers_through_its_declared_integral_state() {
    let loop_definition = project_pid_loop();
    let adapter = controller_adapter(&loop_definition).expect("valid");
    let binding = adapter
        .transfer_binding()
        .expect("the declared states bind the loop");
    assert!(
        binding
            .states
            .iter()
            .any(|member| member.symbol == "Main.Program.Integral"),
        "{:?}",
        binding.states
    );
}

#[test]
fn a_program_pid_law_writes_its_variables_and_zeros_the_terms_its_structure_omits() {
    let loop_definition = project_pid_loop();
    let adapter = controller_adapter(&loop_definition).expect("valid");
    let values = adapter
        .materialize(&proportional(2.0), &observed())
        .expect("a P structure is written with explicit zeros");
    assert_eq!(values.get("Main.Program.Kp").copied(), Some(2.0));
    assert_eq!(values.get("Main.Program.Ki").copied(), Some(0.0));
    assert_eq!(values.get("Main.Program.Kd").copied(), Some(0.0));
    assert_eq!(values.get("Main.Observed").copied(), Some(7.0));
}

#[test]
fn a_program_pd_law_offers_p_and_pd_and_refuses_an_integral_gain_it_has_no_variable_for() {
    let loop_definition = project_pid_loop();
    let adapter = project_adapter(&loop_definition, pd_seed());
    assert_eq!(adapter.families(), vec![Structure::P, Structure::PD]);
    let refusal = adapter
        .materialize(&three_gains(2.0, 0.2, 0.1), &BTreeMap::new())
        .expect_err("the PD law names no integral variable");
    assert_eq!(
        refusal.message,
        "The program's own law names no integral variable, so it cannot write a nonzero integral gain."
    );
    assert_eq!(refusal.next_action, NextAction::ChooseMethod);
    let values = adapter
        .materialize(&three_gains(2.0, 0.0, 0.1), &BTreeMap::new())
        .expect("a zero integral gain has no variable to write");
    assert_eq!(values.get("Main.Program.Kd").copied(), Some(0.1));
    assert!(!values.contains_key("Main.Program.Ki"));
}

#[test]
fn a_program_law_refuses_the_setpoint_weight_and_filter_it_cannot_hold() {
    let loop_definition = project_pid_loop();
    let adapter = controller_adapter(&loop_definition).expect("valid");
    let weighted = ControllerGains {
        proportional_setpoint_weight: Some(0.5),
        ..three_gains(2.0, 0.5, 0.1)
    };
    assert_eq!(
        adapter
            .materialize(&weighted, &BTreeMap::new())
            .expect_err("the law declares no weight")
            .message,
        "A declared proportional setpoint weight is required before materializing a weighted controller."
    );
    let filtered = ControllerGains {
        measurement_filter_seconds: Some(0.5),
        ..three_gains(2.0, 0.5, 0.1)
    };
    assert_eq!(
        adapter
            .materialize(&filtered, &BTreeMap::new())
            .expect_err("the law declares no filter")
            .message,
        "A declared measurement filter time is required before materializing a filtered controller."
    );
}

#[test]
fn a_real_program_gain_is_rounded_to_single_precision_before_it_is_written() {
    let mut loop_definition = project_pid_loop();
    parameter_mut(&mut loop_definition, "Main.Program.Kp").iec_type = RealType::Real;
    let adapter = controller_adapter(&loop_definition).expect("valid");
    let values = adapter
        .materialize(&three_gains(0.1, 0.0, 0.0), &BTreeMap::new())
        .expect("in bounds");
    let written = values
        .get("Main.Program.Kp")
        .copied()
        .expect("the gain is written");
    assert!((written - f64::from(0.1_f32)).abs() <= 1e-15, "{written}");
}

#[test]
fn a_program_gain_outside_its_binding_is_refused_and_a_gain_is_bounded_by_magnitude() {
    let loop_definition = project_pid_loop();
    let adapter = controller_adapter(&loop_definition).expect("valid");
    let negative = adapter
        .materialize(&three_gains(-50.0, 0.5, 0.1), &BTreeMap::new())
        .expect("magnitude 50 is in [-100, 100]");
    assert_eq!(negative.get("Main.Program.Kp").copied(), Some(-50.0));
    let refusal = adapter
        .materialize(&three_gains(150.0, 0.5, 0.1), &BTreeMap::new())
        .expect_err("150 is above the bound");
    assert!(
        refusal
            .message
            .starts_with("PID gain for Main.Program.Kp is outside its declared binding"),
        "{}",
        refusal.message
    );
    assert_eq!(refusal.next_action, NextAction::Tune);
}

#[test]
fn a_program_law_refuses_to_write_a_seed_variable_the_loop_does_not_bind() {
    // The loop contract checks the seed's variables at decode; this is the write's own refusal when a binding no longer holds one.
    let mut loop_definition = project_pid_loop();
    loop_definition
        .parameter_binding
        .as_mut()
        .expect("the fixture has a binding")
        .parameters
        .retain(|parameter| parameter.symbol != "Main.Program.Ki");
    let adapter = controller_adapter(&loop_definition)
        .expect("the choice of the adapter does not read the parameter binding");
    let refusal = adapter
        .materialize(&three_gains(2.0, 0.5, 0.1), &BTreeMap::new())
        .expect_err("the seed names an unbound variable");
    assert_eq!(
        refusal.message,
        texts::parameter_not_declared("Main.Program.Ki")
    );
    assert_eq!(refusal.next_action, NextAction::Tune);
}

#[test]
fn a_program_p_law_is_the_connected_p_on_its_variable_with_the_proportional_tracking() {
    let loop_definition = project_p_loop();
    let adapter = controller_adapter(&loop_definition).expect("the P seed is served");
    assert_eq!(adapter.kind(), &AdapterKind::ConnectedP);
    assert_eq!(adapter.families(), vec![Structure::P]);
    assert!(adapter.check_initialization().is_ok());
    assert!(adapter
        .declared_profile(&first_order_model(), &BTreeMap::new())
        .expect("no profile to fail")
        .is_none());
    assert!(matches!(
        adapter.recovery_tracking(&first_order_model()),
        Ok(TransferTracking::Proportional(tracking)) if tracking.gain_symbol == "Main.Program.Kp"
    ));
    let values = adapter
        .materialize(&proportional(2.5), &observed())
        .expect("in bounds");
    assert_eq!(values.get("Main.Program.Kp").copied(), Some(2.5));
    assert_eq!(values.get("Main.Observed").copied(), Some(7.0));
}

#[test]
fn a_program_p_law_that_declares_a_law_state_is_refused_by_its_name() {
    // C13: the connected P entry cannot start an integral or a derivative term, so the law state is refused, naming it.
    let loop_definition = project_p_loop_with_law_state();
    let refusal = controller_adapter(&loop_definition)
        .expect_err("the connected P entry cannot start a law state");
    // `Failure::new` keeps the first sentence as the message and the whole text as the detail.
    assert_eq!(
        refusal
            .detail
            .as_deref()
            .unwrap_or(refusal.message.as_str()),
        texts::project_p_law_state("Main.Program.Integral")
    );
    assert_eq!(refusal.next_action, NextAction::OpenLoopSetup);
}

#[test]
fn a_program_p_law_whose_application_state_takes_a_formula_is_refused_by_its_name() {
    // The Auto entry writes the application states at their declared constants, without a model, so a formula is refused here.
    let loop_definition = project_p_loop_with_formula_application();
    let refusal = controller_adapter(&loop_definition)
        .expect_err("the Auto entry writes a constant application state only");
    assert_eq!(
        refusal.message,
        texts::project_p_application_formula("Main.Program.Integral")
    );
    assert_eq!(refusal.next_action, NextAction::OpenLoopSetup);
}

#[test]
fn an_own_law_p_loop_with_its_startup_flag_plans_one_entry_that_writes_the_reference_the_mode_the_command_and_the_flag(
) {
    // C13 Apply step 4 and C1: the startup-done flag is an application state. The one entry writes it at the boundary with the reference
    // and the mode; the runtime's tracking writes the Manual command at that boundary too. Validate's run starts the flag TRUE.
    let loop_definition = project_p_loop_with_startup();
    let adapter = controller_adapter(&loop_definition)
        .expect("the P law with its application state is served");
    assert_eq!(adapter.kind(), &AdapterKind::ConnectedP);
    let binding = adapter
        .transfer_binding()
        .expect("the binding states the reference, the Manual command and the flag");
    let states: Vec<&str> = binding
        .states
        .iter()
        .map(|member| member.symbol.as_str())
        .collect();
    assert_eq!(
        states,
        vec!["LevelSP", "Main.ManualCommand", "Main.Program.Started"]
    );
    let values = BTreeMap::from([("Main.Program.Kp".to_owned(), 2.0)]);
    let plan = match adapter
        .initialization_plan(&first_order_model(), &requirements(), &values, Some(0.05))
        .expect("the fixture starts")
    {
        AdapterStartPlan::Start(plan) => plan,
        other @ AdapterStartPlan::ParallelPid(_) => {
            panic!("a P law starts with one plan, got {other:?}")
        }
    };
    let flag = plan
        .run
        .events
        .iter()
        .find(|event| event.symbol == "Main.Program.Started")
        .expect("the entry writes the flag");
    assert_eq!(flag.value, 1.0, "the flag is written TRUE");
    let boundary = flag.at_ms;
    assert!(
        plan.run.events.iter().all(|event| event.at_ms == boundary),
        "one boundary: {:?}",
        plan.run.events
    );
    let written: Vec<&str> = plan
        .run
        .events
        .iter()
        .map(|event| event.symbol.as_str())
        .collect();
    assert!(written.contains(&"LevelSP"), "the reference: {written:?}");
    assert!(written.contains(&"Main.Auto"), "the mode: {written:?}");
    assert_eq!(
        plan.run.initial_values.get("Main.Program.Started").copied(),
        Some(1.0),
        "Validate's run starts the flag TRUE"
    );
    assert_eq!(
        plan.run.initial_values.get("Main.ManualCommand").copied(),
        Some(0.0),
        "the Manual command starts at the applied output"
    );
    assert!(plan
        .requirements
        .iter()
        .any(|requirement| requirement.id == "initialized-state:Main.Program.Started"));
}

#[test]
fn a_program_pid_start_is_planned_as_one_start_from_its_declared_states() {
    let loop_definition = project_pid_loop();
    let adapter = controller_adapter(&loop_definition).expect("valid");
    let values = BTreeMap::from([
        ("Main.Program.Kp".to_owned(), 1.0),
        ("Main.Program.Ki".to_owned(), 0.0),
        ("Main.Program.Kd".to_owned(), 0.0),
    ]);
    let plan = adapter
        .initialization_plan(&first_order_model(), &requirements(), &values, Some(0.05))
        .expect("the fixture starts");
    assert!(matches!(plan, AdapterStartPlan::Start(_)), "{plan:?}");
}
