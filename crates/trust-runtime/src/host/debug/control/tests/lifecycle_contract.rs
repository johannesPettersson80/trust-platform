use std::sync::atomic::Ordering;
use std::time::Duration;

use super::super::*;

fn state_snapshot(
    control: &DebugControl,
) -> (
    DebugMode,
    Option<DebugStopReason>,
    Option<u32>,
    Option<u32>,
    usize,
    bool,
) {
    let (lock, _) = &*control.state;
    let state = lock.lock().expect("debug state");
    (
        state.mode,
        state.pending_stop,
        state.current_thread,
        state.target_thread,
        state.steps.len(),
        state.snapshot.is_some(),
    )
}

#[test]
fn new_debug_control_has_a_clean_running_state() {
    let control = DebugControl::new();
    let (lock, _) = &*control.state;
    let state = lock.lock().expect("debug state");

    assert_eq!(state.mode, DebugMode::Running);
    assert_eq!(state.current_thread, Some(1));
    assert_eq!(state.target_thread, None);
    assert!(state.breakpoints.is_empty());
    assert!(state.breakpoint_generation.is_empty());
    assert!(state.breakpoint_stops_this_cycle.is_empty());
    assert!(state.frame_locations.is_empty());
    assert!(state.logs.is_empty());
    assert!(state.snapshot.is_none());
    assert!(state.watches.is_empty());
    assert!(!state.watch_changed);
    assert!(state.runtime_events.is_empty());
    assert!(state.pending_stop.is_none());
    assert!(state.stops.is_empty());
    assert!(state.last_stop.is_none());
    assert!(state.steps.is_empty());
    assert!(state.io_writes.is_empty());
    assert!(state.pending_var_writes.is_empty());
    assert!(state.pending_lvalue_writes.is_empty());
    assert!(state.forced_vars.is_empty());
    assert!(state.forced_io.is_empty());
    assert_eq!(control.watchdog_pause_elapsed(), Duration::ZERO);
}

#[test]
fn default_debug_control_has_the_same_initial_state_as_new() {
    let control = DebugControl::default();
    assert_eq!(
        state_snapshot(&control),
        (DebugMode::Running, None, Some(1), None, 0, false)
    );
}

#[test]
fn pause_records_one_global_pending_stop_and_invalidates_snapshot() {
    let control = DebugControl::new();
    {
        let (lock, _) = &*control.state;
        let mut state = lock.lock().expect("debug state");
        state.snapshot = Some(crate::debug::DebugSnapshot {
            storage: crate::memory::VariableStorage::new(),
            now: crate::value::Duration::from_millis(7),
        });
        state.steps.insert(
            1,
            StepState {
                kind: StepKind::Into,
                target_depth: 0,
                started: true,
            },
        );
    }

    assert_eq!(
        control.apply_action(ControlAction::Pause(None)),
        ControlOutcome::Applied
    );
    assert_eq!(
        state_snapshot(&control),
        (
            DebugMode::Paused,
            Some(DebugStopReason::Pause),
            Some(1),
            None,
            0,
            false
        )
    );
}

#[test]
fn thread_pause_records_the_exact_target() {
    let control = DebugControl::new();
    assert_eq!(
        control.apply_action(ControlAction::Pause(Some(42))),
        ControlOutcome::Applied
    );
    assert_eq!(control.target_thread(), Some(42));
    let (lock, _) = &*control.state;
    assert_eq!(
        lock.lock().expect("debug state").pending_stop,
        Some(DebugStopReason::Pause)
    );
}

#[test]
fn second_pause_is_ignored_without_retargeting_or_replacing_reason() {
    let control = DebugControl::new();
    assert_eq!(
        control.apply_action(ControlAction::Pause(Some(7))),
        ControlOutcome::Applied
    );
    {
        let (lock, _) = &*control.state;
        lock.lock().expect("debug state").pending_stop = Some(DebugStopReason::Entry);
    }

    assert_eq!(
        control.apply_action(ControlAction::Pause(Some(9))),
        ControlOutcome::Ignored
    );
    let (lock, _) = &*control.state;
    let state = lock.lock().expect("debug state");
    assert_eq!(state.target_thread, Some(7));
    assert_eq!(state.pending_stop, Some(DebugStopReason::Entry));
}

#[test]
fn pause_entry_records_distinct_reason_without_thread_target() {
    let control = DebugControl::new();
    control.set_current_thread(Some(11));
    control.pause_entry();

    let (lock, _) = &*control.state;
    let state = lock.lock().expect("debug state");
    assert_eq!(state.mode, DebugMode::Paused);
    assert_eq!(state.pending_stop, Some(DebugStopReason::Entry));
    assert_eq!(state.target_thread, None);
}

#[test]
fn pause_entry_is_idempotent_while_paused() {
    let control = DebugControl::new();
    control.pause_thread(7);
    control.pause_entry();

    let (lock, _) = &*control.state;
    let state = lock.lock().expect("debug state");
    assert_eq!(state.pending_stop, Some(DebugStopReason::Pause));
    assert_eq!(state.target_thread, Some(7));
}

#[test]
fn continue_clears_pending_control_state_and_snapshot() {
    let control = DebugControl::new();
    control.pause_thread(7);
    {
        let (lock, _) = &*control.state;
        let mut state = lock.lock().expect("debug state");
        state.snapshot = Some(crate::debug::DebugSnapshot {
            storage: crate::memory::VariableStorage::new(),
            now: crate::value::Duration::from_millis(1),
        });
        state.steps.insert(
            7,
            StepState {
                kind: StepKind::Over,
                target_depth: 4,
                started: true,
            },
        );
    }

    assert_eq!(
        control.apply_action(ControlAction::Continue),
        ControlOutcome::Applied
    );
    assert_eq!(
        state_snapshot(&control),
        (DebugMode::Running, None, Some(1), None, 0, false)
    );
}

#[test]
fn step_in_targets_explicit_thread_and_replaces_prior_step() {
    let control = DebugControl::new();
    {
        let (lock, _) = &*control.state;
        let mut state = lock.lock().expect("debug state");
        state.mode = DebugMode::Paused;
        state.last_call_depth = 5;
        state.steps.insert(
            3,
            StepState {
                kind: StepKind::Out,
                target_depth: 1,
                started: false,
            },
        );
    }

    assert_eq!(
        control.apply_action(ControlAction::StepIn(Some(8))),
        ControlOutcome::Applied
    );
    let (lock, _) = &*control.state;
    let state = lock.lock().expect("debug state");
    assert_eq!(state.mode, DebugMode::Running);
    assert_eq!(state.target_thread, Some(8));
    assert_eq!(state.steps.len(), 1);
    let step = state.steps.get(&8).expect("thread step");
    assert_eq!(step.kind, StepKind::Into);
    assert_eq!(step.target_depth, 5);
    assert!(step.started);
}

#[test]
fn untargeted_step_uses_current_thread() {
    let control = DebugControl::new();
    control.set_current_thread(Some(12));
    control.step();

    let (lock, _) = &*control.state;
    let state = lock.lock().expect("debug state");
    assert_eq!(state.target_thread, Some(12));
    assert!(state.steps.contains_key(&12));
}

#[test]
fn step_without_any_current_thread_uses_global_step_key() {
    let control = DebugControl::new();
    control.set_current_thread(None);
    control.step();

    let (lock, _) = &*control.state;
    let state = lock.lock().expect("debug state");
    assert_eq!(state.target_thread, None);
    assert!(state.steps.contains_key(&0));
}

#[test]
fn step_over_uses_last_depth_for_the_target_thread() {
    let control = DebugControl::new();
    {
        let (lock, _) = &*control.state;
        let mut state = lock.lock().expect("debug state");
        state.mode = DebugMode::Paused;
        state.last_call_depth = 2;
        state.last_call_depths.insert(9, 17);
    }
    control.step_over_thread(9);

    let (lock, _) = &*control.state;
    let state = lock.lock().expect("debug state");
    let step = state.steps.get(&9).expect("step over");
    assert_eq!(step.kind, StepKind::Over);
    assert_eq!(step.target_depth, 17);
    assert!(step.started);
}

#[test]
fn step_out_uses_saturating_predecessor_of_target_depth() {
    let control = DebugControl::new();
    {
        let (lock, _) = &*control.state;
        let mut state = lock.lock().expect("debug state");
        state.mode = DebugMode::Paused;
        state.last_call_depths.insert(7, 0);
    }
    control.step_out_thread(7);

    let (lock, _) = &*control.state;
    let state = lock.lock().expect("debug state");
    let step = state.steps.get(&7).expect("step out");
    assert_eq!(step.kind, StepKind::Out);
    assert_eq!(step.target_depth, 0);
    assert!(step.started);
}

#[test]
fn step_started_flag_is_false_when_requested_while_running() {
    let control = DebugControl::new();
    control.step_over();

    let (lock, _) = &*control.state;
    let state = lock.lock().expect("debug state");
    assert!(!state.steps.get(&1).expect("step over").started);
}

#[test]
fn replacing_one_files_breakpoints_preserves_other_files_and_order() {
    let control = DebugControl::new();
    let other = DebugBreakpoint::new(SourceLocation::new(2, 20, 25));
    control.set_breakpoints_for_file(2, vec![other]);

    let mut first = DebugBreakpoint::new(SourceLocation::new(1, 1, 2));
    first.hits = 99;
    first.generation = 88;
    let second = DebugBreakpoint::new(SourceLocation::new(1, 3, 4));
    control.set_breakpoints_for_file(1, vec![first, second]);

    let breakpoints = control.breakpoints();
    assert_eq!(breakpoints.len(), 3);
    assert_eq!(breakpoints[0].location.file_id, 2);
    assert_eq!(breakpoints[1].location, SourceLocation::new(1, 1, 2));
    assert_eq!(breakpoints[2].location, SourceLocation::new(1, 3, 4));
    assert_eq!(breakpoints[1].generation, 1);
    assert_eq!(breakpoints[2].generation, 1);
    assert_eq!(
        breakpoints[1].hits, 0,
        "a replacement set starts a fresh hit-count lifetime"
    );
}

#[test]
fn repeated_breakpoint_replacement_advances_only_owning_file_generation() {
    let control = DebugControl::new();
    control.set_breakpoints_for_file(1, vec![DebugBreakpoint::new(SourceLocation::new(1, 1, 2))]);
    control.set_breakpoints_for_file(2, vec![DebugBreakpoint::new(SourceLocation::new(2, 1, 2))]);
    control.set_breakpoints_for_file(1, vec![DebugBreakpoint::new(SourceLocation::new(1, 3, 4))]);

    assert_eq!(control.breakpoint_generation(1), Some(2));
    assert_eq!(control.breakpoint_generation(2), Some(1));
    assert_eq!(
        control
            .breakpoints()
            .into_iter()
            .find(|breakpoint| breakpoint.location.file_id == 1)
            .expect("file one breakpoint")
            .generation,
        2
    );
}

#[test]
fn empty_replacement_removes_one_file_but_advances_its_generation() {
    let control = DebugControl::new();
    control.set_breakpoints_for_file(1, vec![DebugBreakpoint::new(SourceLocation::new(1, 1, 2))]);
    control.set_breakpoints_for_file(2, vec![DebugBreakpoint::new(SourceLocation::new(2, 1, 2))]);
    control.set_breakpoints_for_file(1, Vec::new());

    assert_eq!(control.breakpoint_generation(1), Some(2));
    assert_eq!(control.breakpoint_count(), 1);
    assert_eq!(control.breakpoints()[0].location.file_id, 2);
}

#[test]
fn breakpoint_generation_saturates_instead_of_wrapping() {
    let control = DebugControl::new();
    {
        let (lock, _) = &*control.state;
        lock.lock()
            .expect("debug state")
            .breakpoint_generation
            .insert(1, u64::MAX);
    }
    control.set_breakpoints_for_file(1, vec![DebugBreakpoint::new(SourceLocation::new(1, 1, 2))]);

    assert_eq!(control.breakpoint_generation(1), Some(u64::MAX));
    assert_eq!(control.breakpoints()[0].generation, u64::MAX);
}

#[test]
fn clear_breakpoints_clears_generation_and_cycle_guard() {
    let control = DebugControl::new();
    control.set_breakpoints_for_file(1, vec![DebugBreakpoint::new(SourceLocation::new(1, 1, 2))]);
    {
        let (lock, _) = &*control.state;
        let mut state = lock.lock().expect("debug state");
        state.breakpoint_stops_this_cycle.insert((1, 1, 2, 1));
    }

    control.clear_breakpoints();
    let (lock, _) = &*control.state;
    let state = lock.lock().expect("debug state");
    assert!(state.breakpoints.is_empty());
    assert!(state.breakpoint_generation.is_empty());
    assert!(state.breakpoint_stops_this_cycle.is_empty());
}

#[test]
fn begin_cycle_clears_only_the_duplicate_stop_guard() {
    let control = DebugControl::new();
    control.set_breakpoints_for_file(1, vec![DebugBreakpoint::new(SourceLocation::new(1, 1, 2))]);
    {
        let (lock, _) = &*control.state;
        lock.lock()
            .expect("debug state")
            .breakpoint_stops_this_cycle
            .insert((1, 1, 2, 1));
    }

    control.begin_cycle();
    assert_eq!(control.breakpoint_count(), 1);
    assert_eq!(control.breakpoint_generation(1), Some(1));
    let (lock, _) = &*control.state;
    assert!(lock
        .lock()
        .expect("debug state")
        .breakpoint_stops_this_cycle
        .is_empty());
}

#[test]
fn watchdog_pause_time_accumulates_and_saturates() {
    let control = DebugControl::new();
    control.record_watchdog_pause(Duration::from_nanos(2));
    control.record_watchdog_pause(Duration::from_nanos(3));
    assert_eq!(control.watchdog_pause_elapsed(), Duration::from_nanos(5));

    control
        .watchdog_pause_nanos
        .store(u64::MAX - 1, Ordering::Relaxed);
    control.record_watchdog_pause(Duration::from_nanos(9));
    assert_eq!(
        control.watchdog_pause_elapsed(),
        Duration::from_nanos(u64::MAX)
    );
}
