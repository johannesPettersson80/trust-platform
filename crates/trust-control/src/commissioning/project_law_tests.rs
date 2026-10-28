//! The program's own law as the adapters read it (C2, C13): the route of a seed, and the gain variables of a loop's law. The
//! fixtures are the own-law loops the setup writes. The families a seed offers are tested with the block (`project_tests.rs`).

use super::*;
use crate::commissioning::fixture_loops::{
    parked_axes_project_loop, parked_project_loop, project_p_loop, project_pid_loop,
    proportional_loop,
};
use crate::loop_definition::ProjectSeedBlock;
use crate::tuning::structure::Structure;

/// A seed that names the proportional variable `Main.Program.Kp`, and the integral and derivative variables given.
fn seed(integral: Option<&str>, derivative: Option<&str>) -> ParallelPidSeed {
    ParallelPidSeed {
        family: Structure::P,
        proportional_symbol: "Main.Program.Kp".to_owned(),
        integral_symbol: integral.map(str::to_owned),
        derivative_symbol: derivative.map(str::to_owned),
    }
}

/// The own-law PID loop, with its seed's integral and derivative variables kept or dropped as the flags say.
fn pid_loop_with_terms(integral: bool, derivative: bool) -> LoopDefinition {
    let mut loop_definition = project_pid_loop();
    if let Some(project) = loop_definition.controller.project.as_mut()
        && let Some(ProjectSeedBlock::ParallelPid(seed)) = project.seed.as_mut()
    {
        if !integral {
            seed.integral_symbol = None;
        }
        if !derivative {
            seed.derivative_symbol = None;
        }
    }
    loop_definition
}

#[test]
fn a_seed_with_an_integral_or_a_derivative_variable_declares_states() {
    assert!(has_pid_terms(&seed(Some("Main.Program.Ki"), None)));
    assert!(has_pid_terms(&seed(None, Some("Main.Program.Kd"))));
    assert!(!has_pid_terms(&seed(None, None)));
}

#[test]
fn the_seed_of_the_block_routes_the_law_by_its_variables() {
    let with_both = pid_loop_with_terms(true, true);
    let with_derivative = pid_loop_with_terms(false, true);
    let proportional_only = pid_loop_with_terms(false, false);
    assert!(served_law(&with_both).is_some_and(|law| has_pid_terms(law.seed)));
    assert!(served_law(&with_derivative).is_some_and(|law| has_pid_terms(law.seed)));
    assert!(served_law(&proportional_only).is_some_and(|law| !has_pid_terms(law.seed)));
    assert_eq!(
        connected_p_gain(&proportional_only),
        Some("Main.Program.Kp")
    );
    assert_eq!(connected_p_gain(&with_derivative), None);
}

#[test]
fn a_program_pid_law_names_its_seed_variables_and_is_no_connected_p() {
    let loop_definition = project_pid_loop();
    assert_eq!(
        gain_symbols(&loop_definition),
        Some(GainSymbols {
            proportional: "Main.Program.Kp",
            integral: Some("Main.Program.Ki"),
            derivative: Some("Main.Program.Kd"),
        })
    );
    assert_eq!(connected_p_gain(&loop_definition), None);
}

#[test]
fn a_program_p_law_is_a_connected_p_on_its_proportional_variable() {
    let loop_definition = project_p_loop();
    assert_eq!(
        gain_symbols(&loop_definition).map(|symbols| symbols.integral),
        Some(None)
    );
    assert_eq!(connected_p_gain(&loop_definition), Some("Main.Program.Kp"));
}

#[test]
fn a_proportional_law_is_the_connected_p_on_its_gain_and_a_parked_seed_is_not_served() {
    assert_eq!(
        connected_p_gain(&proportional_loop()),
        Some("Main.Parameters.Kp")
    );
    assert!(served_law(&project_pid_loop()).is_some());
    assert!(served_law(&proportional_loop()).is_none());
    assert!(served_law(&parked_project_loop()).is_none());
}

#[test]
fn a_parallel_pid_block_with_a_tuning_axis_is_parked_and_no_caller_reads_it_as_served() {
    // The seed of this block is a served shape, but the block is a search (C5): the gate parks it, so `served_law` answers no for every
    // caller, including those that do not pass through the adapter's gate (`keepable_record`).
    let loop_definition = parked_axes_project_loop();
    assert!(loop_definition
        .controller
        .project
        .as_ref()
        .is_some_and(|project| project.parked_feature().is_some()));
    assert!(served_law(&loop_definition).is_none());
    assert!(gain_symbols(&loop_definition).is_none());
    assert!(connected_p_gain(&loop_definition).is_none());
}
