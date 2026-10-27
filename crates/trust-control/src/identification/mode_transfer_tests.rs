//! Tests of the rules of Identify's mode transfer (`connectedIdentificationMode.ts`; specification 35, A6.2 to A6.4 and B3.4
//! step 1; annex E6.2, E6.4 and E6.9), on the commissioning fixture loops. The three tests of `control-studio-generated-loop.test.ts`
//! that reach `ConnectedIdentificationMode` are ported on the library PID fixture (the loop Create PLC code writes is a library
//! PID too), with its Manual command following the output where the test's loop has it follow.

#![allow(clippy::float_cmp)] // An allowance that is the exact check of 1e-6 is compared exactly.

use std::collections::BTreeMap;

use super::*;
use crate::commissioning::commissioning_result::TransferState;
use crate::commissioning::fixture_loops::{
    library_pid_loop, parallel_pid_loop_without_actuator, project_p_loop, project_pid_loop,
    proportional_loop,
};
use crate::commissioning::transfer::TransferMember;

/// The status the runtime reports for `binding` (annex E6.3): the mode signal at `mode`, every state, guard and output, and every
/// further output of a PID group, at `value(symbol)`, and no transaction.
pub(super) fn status_of(
    binding: &TransferBinding,
    mode: bool,
    value: &dyn Fn(&str) -> f64,
) -> TransferStatus {
    let mut values = BTreeMap::new();
    values.insert(binding.mode.symbol.clone(), TransferValue::Boolean(mode));
    for member in binding
        .states
        .iter()
        .chain(&binding.guards)
        .chain(std::iter::once(&binding.output))
        .chain(&binding.additional_outputs)
    {
        values.insert(
            member.symbol.clone(),
            TransferValue::Number(value(&member.symbol)),
        );
    }
    TransferStatus {
        program_revision: "program".to_owned(),
        binding_revision: "binding".to_owned(),
        values,
        runtime_time_ns: None,
        status: TransferState::Idle,
        reason: None,
        supported_tracking: None,
        transaction: None,
    }
}

/// The symbols of `members`, in order.
pub(super) fn symbols(members: &[TransferMember]) -> Vec<&str> {
    members
        .iter()
        .map(|member| member.symbol.as_str())
        .collect()
}

/// The six states of the library PID fixture that its transfer writes besides the setpoint and the Manual command: its own
/// measurement is read, never written.
const LIBRARY_STATES: [&str; 6] = [
    "Main.TankController.StateValue.Setpoint",
    "Main.TankController.StateValue.SetpointRate",
    "Main.TankController.StateValue.ProcessValueRate",
    "Main.TankController.StateValue.FeedForward",
    "Main.TankController.StateValue.NominalOutput",
    "Main.TankController.StateValue.SaturatedOutput",
];

/// A6.3, E6.2 and E12.2: the mode transfer writes the binding of the loop's controller adapter on a transfer loop of its own, so
/// that its record stays apart from Apply's: the parallel PID's setpoint, Manual command, integral and derivative states, with its
/// gains and process value as guards.
#[test]
fn the_mode_binding_is_the_adapters_binding_on_its_own_transfer_loop() {
    let binding =
        mode_binding(&parallel_pid_loop_without_actuator()).expect("a parallel PID transfers");
    assert_eq!(binding.loop_id, "loop:tank:identification-mode");
    assert_eq!(
        (
            binding.task_name.as_str(),
            binding.mode.symbol.as_str(),
            binding.mode.manual_value
        ),
        ("MainTask", "Main.Auto", false)
    );
    assert_eq!(
        symbols(&binding.states),
        [
            "LevelSP",
            "Main.ManualCommand",
            "Main.Integral",
            "Main.PreviousError"
        ]
    );
    assert_eq!(
        symbols(&binding.guards),
        [
            "Main.Parameters.Kp",
            "Main.Parameters.Ki",
            "Main.Parameters.Kd",
            "LevelPV"
        ]
    );
    assert_eq!(binding.output.symbol, "ValveMV");
}

/// C0 ("Missing adapters"): a loop whose controller no adapter realizes has no transfer, so Identify cannot change its mode and
/// says so with the adapter's refusal.
#[test]
fn a_loop_whose_controller_has_no_adapter_has_no_mode_transfer() {
    let mut loop_definition = parallel_pid_loop_without_actuator();
    loop_definition.controller.law = None;
    let refusal = mode_binding(&loop_definition).expect_err("no adapter, no transfer");
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::ControllerCapability, NextAction::OpenLoopSetup)
    );
}

/// B3.4 step 1 and A6.2: the mode a status reports is read against the binding's Manual value; a loop found in Auto returns to
/// Auto, and a loop found in Manual, or one whose mode the status does not report, stays in Manual (`found_mode_of`).
#[test]
fn the_mode_is_read_against_the_manual_value_and_only_a_loop_found_in_auto_returns_to_auto() {
    let binding = mode_binding(&parallel_pid_loop_without_actuator()).expect("a binding");
    // Main.Auto is TRUE in Auto: its Manual value is FALSE.
    let auto = status_of(&binding, true, &|_| 1.0);
    let manual = status_of(&binding, false, &|_| 1.0);
    assert!(in_mode(&binding, &auto, false));
    assert!(!in_mode(&binding, &auto, true));
    assert!(in_mode(&binding, &manual, true));
    assert!(found_in_auto(&binding, &auto));
    assert!(!found_in_auto(&binding, &manual));
    let mut unknown = auto;
    unknown.values.remove("Main.Auto");
    assert!(!found_in_auto(&binding, &unknown));
    assert!(!in_mode(&binding, &unknown, false));
}

/// E6.5: a mode transfer writes every state at the value the status reports, which is the exact state set the runtime requires;
/// a value outside its member's bounds is refused before anything is sent (E11.8).
#[test]
fn the_transfer_writes_every_state_at_its_reported_value() {
    let binding = mode_binding(&parallel_pid_loop_without_actuator()).expect("a binding");
    let status = status_of(&binding, true, &|symbol| match symbol {
        "LevelSP" => 45.0,
        "Main.ManualCommand" => 12.0,
        "Main.Integral" => -3.5,
        _ => 0.0,
    });
    assert_eq!(
        mode_states(&binding, &status).expect("the states"),
        BTreeMap::from([
            ("LevelSP".to_owned(), 45.0),
            ("Main.ManualCommand".to_owned(), 12.0),
            ("Main.Integral".to_owned(), -3.5),
            ("Main.PreviousError".to_owned(), 0.0),
        ])
    );
    let outside = status_of(&binding, true, &|symbol| {
        if symbol == "Main.Integral" {
            250.0
        } else {
            0.0
        }
    });
    assert!(
        mode_states(&binding, &outside).is_err(),
        "the integral state is bounded at 100"
    );
}

/// `control-studio-generated-loop.test.ts`, "switches to Manual for identification with the manual output the code sets as a
/// bounded state" (§17.1, A6.4, annex E6.4): the transfer carries the library PID's states as the program holds them, unaligned
/// and bounded, and a Manual command that the program copies from the output is bounded as well; without the copy the Manual
/// command is written exactly.
#[test]
fn the_tracking_carries_the_states_as_they_are_and_bounds_a_manual_command_that_follows_the_output()
{
    let mut loop_definition = library_pid_loop();
    loop_definition.controller.manual_output_follows_output = Some(true);
    let binding = mode_binding(&loop_definition).expect("the library PID transfers");
    let status = status_of(&binding, true, &|symbol| {
        if symbol == "Main.TankController.StateValue.NominalOutput" {
            30.0
        } else {
            2.0
        }
    });
    let TransferTracking::DeclaredState(tracking) =
        mode_tracking(&loop_definition, &binding, &status).expect("the tracking")
    else {
        panic!("a mode transfer tracks declared states");
    };
    assert_eq!(
        (
            tracking.reference_symbol.as_str(),
            tracking.manual_command_symbol.as_str(),
            tracking.process_variable.as_str()
        ),
        (
            "LevelSP",
            "Main.ManualCommand",
            "Main.TankController.StateValue.ProcessValue"
        )
    );
    assert!(!tracking.align_output, "the output is held, not aligned");
    assert_eq!(tracking.initialization_formulas, None);
    assert_eq!(
        tracking.initialization_values,
        LIBRARY_STATES
            .iter()
            .map(|symbol| {
                let value = if symbol.ends_with("NominalOutput") {
                    30.0
                } else {
                    2.0
                };
                ((*symbol).to_owned(), value)
            })
            .collect::<BTreeMap<String, f64>>()
    );
    let mut bounded: Vec<&str> = LIBRARY_STATES.to_vec();
    bounded.push("Main.ManualCommand");
    assert_eq!(tracking.bounded_state_symbols, bounded);
    assert_eq!(
        tracking.additional_measurements,
        ["Main.TankController.StateValue.ProcessValue"]
    );
    assert_eq!(
        (
            tracking.additional_references.len(),
            tracking.additional_commands.len()
        ),
        (0, 0)
    );
    loop_definition.controller.manual_output_follows_output = None;
    let TransferTracking::DeclaredState(plain) =
        mode_tracking(&loop_definition, &binding, &status).expect("the tracking")
    else {
        panic!("a mode transfer tracks declared states");
    };
    assert_eq!(plain.bounded_state_symbols, LIBRARY_STATES);
}

/// `control-studio-generated-loop.test.ts`, "returns to Auto after identification allowing the PID's own first step, not a fixed
/// 1e-6" (A6.4, annex E6.9): with the noise n = 0.05 that the baseline measured, Kp = 33.3, Ki = 0.0667 and Kd = 0 on a 100 ms
/// task, the first output may move by Kp·2n from the noise and Ki·Δt·n from the integral, 2·33.3·0.05 + 0.0667·0.1·0.05 =
/// 3.3303335, which the transfer sends, and by Ki·Δt·|e| from the error it enters with, which the runtime adds with the gain
/// Ki·Δt = 0.00667: with the measurement 2 degC below the setpoint at the entry, 3.3436735 in all. Catches a return whose
/// tolerance keeps the error of the reading before the transfer (3.3436735), or that sends no gain for the integral step.
#[test]
fn a_return_to_auto_allows_the_pids_own_first_step() {
    let mut loop_definition = parallel_pid_loop_without_actuator();
    loop_definition.controller.task_period_ms = 100.0;
    let binding = mode_binding(&loop_definition).expect("a binding");
    let observed = status_of(&binding, false, &|symbol| match symbol {
        "Main.Parameters.Kp" => 33.3,
        "Main.Parameters.Ki" => 0.0667,
        "LevelSP" => 45.0,
        "LevelPV" => 43.0,
        _ => 0.0,
    });
    let expected = 2.0 * 33.3 * 0.05 + 0.0667 * 0.1 * 0.05;
    let allowance = auto_entry_response(&loop_definition, &observed, Some(0.05));
    assert!(
        (allowance - expected).abs() < 1e-9,
        "{allowance} versus {expected}"
    );
    let tolerance = mode_output_change(&loop_definition, false, Some(&observed), Some(0.05));
    assert!(
        (tolerance - expected).abs() < 1e-9,
        "{tolerance} versus {expected}"
    );
    let gain = mode_entry_error_gain(&loop_definition, false, Some(&observed), Some(0.05))
        .expect("a PID's Auto entry has the integral step");
    assert!((gain - 0.0667 * 0.1).abs() < 1e-12, "{gain}");
    assert!(
        (tolerance + gain * 2.0 - 3.343_673_5).abs() < 1e-9,
        "with 2 degC at the entry"
    );
}

/// A6.4 and annex E6.9: the mode transfer carries the controller's states, so its first execution in Auto also responds to the
/// measurement's change since its last one, by Kp: the return sends |Kp| = 33.3 as `entry_measurement_gain`, which the runtime
/// multiplies by the change it reads between the scan before the transfer and the scan of the entry. A transfer to Manual, an
/// entry without a reviewed status, a status that does not report Kp, and a law without an integral step (a program's P law)
/// send none. Catches a return that sends no measurement gain, which the runtime then refuses whenever the plant still moves
/// at the entry (lesson 30's return after Apply, 27 October 2026).
#[test]
fn a_return_to_auto_allows_the_response_to_the_measurement_change_before_the_entry() {
    let mut loop_definition = parallel_pid_loop_without_actuator();
    loop_definition.controller.task_period_ms = 100.0;
    let binding = mode_binding(&loop_definition).expect("a binding");
    let observed = status_of(&binding, false, &|symbol| match symbol {
        "Main.Parameters.Kp" => 33.3,
        "Main.Parameters.Ki" => 0.0667,
        _ => 0.0,
    });
    let gain = mode_entry_measurement_gain(&loop_definition, false, Some(&observed), None);
    assert_eq!(gain, Some(33.3), "an Auto entry sends |Kp|");
    assert_eq!(
        mode_entry_measurement_gain(&loop_definition, true, Some(&observed), None),
        None,
        "a transfer to Manual"
    );
    assert_eq!(
        mode_entry_measurement_gain(&loop_definition, false, None, None),
        None,
        "an entry without a reviewed status"
    );
    let mut unreported = observed.clone();
    unreported.values.remove("Main.Parameters.Kp");
    assert_eq!(
        mode_entry_measurement_gain(&loop_definition, false, Some(&unreported), None),
        None,
        "a status that does not report Kp"
    );
    let p_law = project_p_loop();
    let p_binding = mode_binding(&p_law).expect("the connected P transfers");
    let p_observed = status_of(&p_binding, true, &|_| 1.0);
    assert_eq!(
        mode_entry_measurement_gain(&p_law, false, Some(&p_observed), None),
        None,
        "a law without an integral step"
    );
}

/// Annex E6.9 and A6.4: a transfer to Manual allows 1e-6, or the loop's faster rate limit over one execution of its task when that
/// is larger. An Auto entry adds the controller's own response: without a measured noise the exact check of 1e-6 over the 100 %
/// span, 1e-4, and the gain Ki·Δt = 0.5·0.01 = 0.005 for the runtime's integral step on the error at entry; a transfer to Manual,
/// an entry without a reviewed status, a loop without a PID law, and a status that does not report a gain add nothing.
#[test]
fn a_transfer_allows_the_rate_limit_over_one_execution_and_only_a_pid_entry_adds_its_response() {
    let mut loop_definition = parallel_pid_loop_without_actuator();
    let binding = mode_binding(&loop_definition).expect("a binding");
    let observed = status_of(&binding, true, &|symbol| match symbol {
        "Main.Parameters.Kp" => 2.0,
        "Main.Parameters.Ki" => 0.5,
        "LevelSP" => 45.0,
        "LevelPV" => 44.0,
        _ => 0.0,
    });
    assert_eq!(
        mode_output_change(&loop_definition, true, Some(&observed), None),
        1e-6,
        "a transfer to Manual allows no response"
    );
    assert_eq!(
        mode_entry_error_gain(&loop_definition, true, Some(&observed), None),
        None,
        "a transfer to Manual has no integral step"
    );
    assert!(
        (mode_output_change(&loop_definition, false, Some(&observed), None) - 1e-4).abs() < 1e-12
    );
    let gain = mode_entry_error_gain(&loop_definition, false, Some(&observed), None)
        .expect("an Auto entry has the integral step");
    assert!((gain - 0.005).abs() < 1e-12, "{gain}");
    assert_eq!(
        mode_output_change(&loop_definition, false, None, None),
        1e-6,
        "an Auto entry without a reviewed status allows no response"
    );
    assert_eq!(
        mode_entry_error_gain(&loop_definition, false, None, None),
        None
    );
    let mut silent = observed;
    silent.values.remove("Main.Parameters.Ki");
    assert_eq!(
        auto_entry_response(&loop_definition, &silent, Some(0.05)),
        0.0,
        "a gain that the status does not report allows no response"
    );
    assert_eq!(
        mode_entry_error_gain(&loop_definition, false, Some(&silent), Some(0.05)),
        None
    );
    loop_definition.limits.rate_up_per_s = Some(20.0);
    loop_definition.limits.rate_down_per_s = Some(50.0);
    assert!(
        (mode_output_change(&loop_definition, true, None, None) - 0.5).abs() < 1e-12,
        "50 %/s over a 10 ms task"
    );
    let proportional = proportional_loop();
    let p_binding = mode_binding(&proportional).expect("the connected P transfers");
    let p_observed = status_of(&p_binding, true, &|_| 1.0);
    assert_eq!(
        auto_entry_response(&proportional, &p_observed, Some(0.05)),
        0.0,
        "a proportional law has no PID response"
    );
    assert_eq!(
        mode_entry_error_gain(&proportional, false, Some(&p_observed), Some(0.05)),
        None
    );
}

/// `control-studio-generated-loop.test.ts`, "returns a loop to Auto after identification without writing back the manual output
/// the code sets" (§17.1, A6.2): a loop found in Auto whose program copies the output into the Manual command gets its setpoint
/// back only, since writing the copy back would move the output. Without the copy, and for a loop found in Manual, the Manual
/// command returns too. The guards are the record's parameters and the feedback guard of the tracked output (annex E6.2).
#[test]
fn the_command_return_writes_back_the_setpoint_and_a_manual_command_the_program_does_not_set() {
    let mut loop_definition = library_pid_loop();
    loop_definition.controller.manual_output_follows_output = Some(true);
    let binding = mode_binding(&loop_definition).expect("a binding");
    let original = |mode: bool| {
        status_of(&binding, mode, &|symbol| match symbol {
            "LevelSP" => 45.0,
            "Main.ManualCommand" => 12.0,
            _ => 1.0,
        })
    };
    let back = command_return(&loop_definition, &binding, &original(true)).expect("found in Auto");
    assert_eq!(
        back.binding.loop_id,
        "loop:tank:identification-mode:commands"
    );
    assert_eq!(symbols(&back.binding.states), ["LevelSP"]);
    assert_eq!(back.values, BTreeMap::from([("LevelSP".to_owned(), 45.0)]));
    assert_eq!(
        symbols(&back.binding.guards),
        [
            "Main.Parameters.Kp",
            "Main.Parameters.Ki",
            "Main.Parameters.Kd",
            "ValvePosition"
        ]
    );
    assert_eq!(
        back.binding.tracking_outputs,
        BTreeMap::from([("ValveMV".to_owned(), "ValvePosition".to_owned())])
    );
    let found_in_manual =
        command_return(&loop_definition, &binding, &original(false)).expect("found in Manual");
    assert_eq!(
        found_in_manual.values,
        BTreeMap::from([
            ("LevelSP".to_owned(), 45.0),
            ("Main.ManualCommand".to_owned(), 12.0),
        ])
    );
    loop_definition.controller.manual_output_follows_output = None;
    let without_copy =
        command_return(&loop_definition, &binding, &original(true)).expect("found in Auto");
    assert_eq!(
        symbols(&without_copy.binding.states),
        ["LevelSP", "Main.ManualCommand"]
    );
}

/// A6.2, A6.7 and E11.5: the mode returns only to the loop and the program it was changed on; a changed loop or a reloaded program
/// is refused, and the refusal names both revisions (A11.1).
#[test]
fn a_return_is_refused_on_a_changed_loop_or_program_naming_both_revisions() {
    let loop_definition = parallel_pid_loop_without_actuator();
    let binding = mode_binding(&loop_definition).expect("a binding");
    let original = status_of(&binding, true, &|_| 1.0);
    let changed = IdentificationModeCheckpoint {
        loop_revision: "sha256:loop-0".to_owned(),
        original: original.clone(),
        transfer: None,
    };
    let refusal = check_mode_loop(&loop_definition, &changed).expect_err("the loop changed");
    assert_eq!(
        refusal.message,
        "Restore the original loop bindings before recovering identification mode: Identify changed the loop's mode at loop revision sha256:loop-0, and the loop is now at revision sha256:loop-1."
    );
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::Settings, NextAction::OpenLoopSetup)
    );
    let same = IdentificationModeCheckpoint {
        loop_revision: loop_definition.revision.clone(),
        ..changed
    };
    assert_eq!(check_mode_loop(&loop_definition, &same), Ok(()));
    let mut reloaded = original.clone();
    reloaded.program_revision = "program-2".to_owned();
    let refusal = check_mode_program(&same, &reloaded).expect_err("the program changed");
    assert_eq!(
        refusal.message,
        "The program changed from revision program to program-2; the saved identification mode cannot be restored to this program."
    );
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::Runtime, NextAction::None)
    );
    assert_eq!(check_mode_program(&same, &original), Ok(()));
}

#[test]
fn restoring_the_found_manual_command_allows_the_change_it_makes() {
    // A6.2, E6.9: the output stands at 83 % after a setpoint step; writing the found command of 75 % back moves it by 8 points.
    let binding = mode_binding(&parallel_pid_loop_without_actuator()).expect("a binding");
    let standing = status_of(&binding, false, &|symbol| {
        if symbol == "ValveMV" {
            83.0
        } else {
            0.0
        }
    });
    let commands = CommandReturn {
        binding: binding.clone(),
        values: BTreeMap::from([
            ("Main.SP".to_owned(), 35.0),
            ("Main.ManualMV".to_owned(), 75.0),
        ]),
        commands: vec![WrittenCommand {
            command: "Main.ManualMV".to_owned(),
            output: "ValveMV".to_owned(),
        }],
    };
    assert!((commands.allowance(1e-6, &standing) - 8.0).abs() <= 1e-12);
    assert!(
        (commands.allowance(10.0, &standing) - 10.0).abs() <= 1e-12,
        "a larger mode allowance stays"
    );
    let mut unread = standing.clone();
    unread.values.remove("ValveMV");
    assert!(
        (commands.allowance(1e-6, &unread) - 1e-6).abs() <= 1e-18,
        "an unread output allows the base"
    );
    let references_only = CommandReturn {
        commands: Vec::new(),
        ..commands
    };
    assert!(
        (references_only.allowance(1e-6, &standing) - 1e-6).abs() <= 1e-18,
        "only references are written back"
    );
}

/// C1 and A6.2: a program that sets its reference from the measurement in Manual overwrites the setpoint written back there, so
/// the loop enters Auto at the measurement and the setpoint returns afterwards, in Auto: every state as the loop holds it, the
/// setpoint at its found value, the mode transfer's tracking, and the allowance of a target change across the outputs. A loop that
/// holds its found setpoint needs no step.
#[test]
fn a_setpoint_the_program_overwrote_in_manual_returns_after_the_auto_entry() {
    let loop_definition = library_pid_loop();
    let binding = mode_binding(&loop_definition).expect("a binding");
    let original = status_of(&binding, true, &|symbol| match symbol {
        "LevelSP" => 45.0,
        _ => 1.0,
    });
    let entered = status_of(&binding, true, &|symbol| match symbol {
        "LevelSP" => 44.25,
        _ => 2.0,
    });
    let step = reference_return(&loop_definition, &binding, &original, &entered)
        .expect("the step")
        .expect("the setpoint returns");
    let mut values = mode_states(&binding, &entered).expect("the states");
    assert_eq!(values.insert("LevelSP".to_owned(), 45.0), Some(44.25));
    assert_eq!(step.values, values);
    assert_eq!(
        step.tracking,
        mode_tracking(&loop_definition, &binding, &entered).expect("the tracking")
    );
    assert_eq!(
        Some(step.tolerance),
        transfer_target_allowance(&binding),
        "a target change across the outputs"
    );
    let holding = status_of(&binding, true, &|symbol| match symbol {
        "LevelSP" => 45.0,
        _ => 2.0,
    });
    assert_eq!(
        reference_return(&loop_definition, &binding, &original, &holding).expect("no step"),
        None
    );
}

/// C2 and A6.4: a program's own PI law answers its first step with the allowance of its seed's variables, with the same formula as a
/// PID record: 2·Kp·n from P and Ki·Δt·n from I on the noise, 2 · 2 · 0.05 + 0.5 · 0.01 · 0.05 = 0.20025 on the 10 ms fixture task, and
/// the gain Ki·Δt = 0.5 · 0.01 = 0.005 with which the runtime adds Ki·Δt·|r − y| for the error the step leaves at the entry.
#[test]
fn a_program_pi_law_allows_its_own_first_step_from_its_seed_variables() {
    let loop_definition = project_pid_loop();
    let binding = mode_binding(&loop_definition).expect("a binding");
    let observed = status_of(&binding, false, &|symbol| match symbol {
        "Main.Program.Kp" => 2.0,
        "Main.Program.Ki" => 0.5,
        "LevelSP" => 45.0,
        "LevelPV" => 43.0,
        _ => 0.0,
    });
    let expected = 2.0 * 2.0 * 0.05 + 0.5 * 0.01 * 0.05;
    let allowance = auto_entry_response(&loop_definition, &observed, Some(0.05));
    assert!(
        (allowance - expected).abs() < 1e-9,
        "{allowance} versus {expected}"
    );
    let gain = mode_entry_error_gain(&loop_definition, false, Some(&observed), Some(0.05));
    assert!(
        gain.is_some_and(|gain| (gain - 0.005).abs() < 1e-12),
        "{gain:?}"
    );
}

/// C13: a program's own P law has no integral or derivative term, so its Auto entry adds no PID response, as a proportional law has none.
#[test]
fn a_program_p_law_adds_no_pid_response_on_its_connected_p_entry() {
    let loop_definition = project_p_loop();
    let binding = mode_binding(&loop_definition).expect("the connected P transfers");
    let observed = status_of(&binding, true, &|_| 1.0);
    assert_eq!(
        auto_entry_response(&loop_definition, &observed, Some(0.05)),
        0.0,
        "a proportional law has no PID response"
    );
}
