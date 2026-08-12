use std::sync::mpsc::{channel, TryRecvError};

use crate::memory::{FrameId, VariableStorage};
use crate::program_model::Expr;
use crate::value::{DateTimeProfile, Duration, Value};
use trust_hir::types::TypeRegistry;

use super::*;

fn context<'a>(
    storage: &'a mut VariableStorage,
    registry: &'a TypeRegistry,
    now_ms: i64,
) -> DebugRuntimeContext<'a> {
    DebugRuntimeContext {
        storage,
        registry,
        stdlib: None,
        profile: DateTimeProfile::default(),
        current_instance: None,
        now: Duration::from_millis(now_ms),
    }
}

#[test]
fn location_trace_format_is_exact_and_handles_absence() {
    assert_eq!(
        format_location_ref(Some(&SourceLocation::new(7, 10, 20))),
        "7:10..20"
    );
    assert_eq!(format_location_ref(None), "<none>");
}

#[test]
fn emit_stop_updates_last_stop_and_ordered_local_buffer() {
    let control = DebugControl::new();
    let location = SourceLocation::new(1, 10, 20);
    {
        let (lock, _) = &*control.state;
        let mut state = lock.lock().expect("state");
        state.current_thread = Some(17);
        emit_stop(
            &mut state,
            DebugStopReason::Breakpoint,
            Some(location),
            Some(9),
        );
        emit_stop(&mut state, DebugStopReason::Step, None, None);
    }

    let last = control.last_stop().expect("last stop");
    assert_eq!(last.reason, DebugStopReason::Step);
    assert_eq!(last.location, None);
    assert_eq!(last.thread_id, Some(17));
    let stops = control.drain_stops();
    assert_eq!(stops.len(), 2);
    assert_eq!(stops[0].reason, DebugStopReason::Breakpoint);
    assert_eq!(stops[0].location, Some(location));
    assert_eq!(stops[0].breakpoint_generation, Some(9));
    assert_eq!(stops[1].reason, DebugStopReason::Step);
}

#[test]
fn live_stop_receiver_gets_one_owned_copy_and_buffer_is_retained() {
    let control = DebugControl::new();
    let (tx, rx) = channel();
    let location = SourceLocation::new(1, 10, 20);
    {
        let (lock, _) = &*control.state;
        let mut state = lock.lock().expect("state");
        state.stop_tx = Some(tx);
        emit_stop(
            &mut state,
            DebugStopReason::Pause,
            Some(location),
            None,
        );
    }

    let delivered = rx.try_recv().expect("delivered stop");
    assert_eq!(delivered.reason, DebugStopReason::Pause);
    assert_eq!(delivered.location, Some(location));
    assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    let buffered = control.drain_stops();
    assert_eq!(buffered.len(), 1);
    assert_eq!(buffered[0].reason, DebugStopReason::Pause);
}

#[test]
fn closed_stop_receiver_cannot_discard_triggering_stop() {
    let control = DebugControl::new();
    let (tx, rx) = channel();
    drop(rx);
    {
        let (lock, _) = &*control.state;
        let mut state = lock.lock().expect("state");
        state.stop_tx = Some(tx);
        emit_stop(&mut state, DebugStopReason::Entry, None, None);
    }
    let stops = control.drain_stops();
    assert_eq!(stops.len(), 1);
    assert_eq!(stops[0].reason, DebugStopReason::Entry);
    assert_eq!(
        control.last_stop().expect("last stop").reason,
        DebugStopReason::Entry
    );
}

#[test]
fn snapshot_captures_owned_storage_and_exact_runtime_time() {
    let control = DebugControl::new();
    let registry = TypeRegistry::new();
    let mut storage = VariableStorage::new();
    storage.set_global("value", Value::DInt(7));
    {
        let mut ctx = context(&mut storage, &registry, 42);
        let (lock, _) = &*control.state;
        let mut state = lock.lock().expect("state");
        update_snapshot(&mut state, &mut ctx);
    }
    storage.set_global("value", Value::DInt(9));

    let snapshot = control.snapshot().expect("snapshot");
    assert_eq!(snapshot.now, Duration::from_millis(42));
    assert_eq!(snapshot.storage.get_global("value"), Some(&Value::DInt(7)));
}

#[test]
fn first_watch_evaluation_sets_value_and_change_edge() {
    let control = DebugControl::new();
    let registry = TypeRegistry::new();
    let mut storage = VariableStorage::new();
    let mut ctx = context(&mut storage, &registry, 0);
    let (lock, _) = &*control.state;
    let mut state = lock.lock().expect("state");
    state.watches.push(WatchEntry {
        expr: Expr::Literal(Value::DInt(7)),
        last: None,
    });

    update_watch_snapshot(&mut state, &mut ctx);
    assert_eq!(state.watches[0].last, Some(Value::DInt(7)));
    assert!(state.watch_changed);
}

#[test]
fn unchanged_watch_value_does_not_create_new_change_edge() {
    let control = DebugControl::new();
    let registry = TypeRegistry::new();
    let mut storage = VariableStorage::new();
    let mut ctx = context(&mut storage, &registry, 0);
    let (lock, _) = &*control.state;
    let mut state = lock.lock().expect("state");
    state.watches.push(WatchEntry {
        expr: Expr::Literal(Value::DInt(7)),
        last: Some(Value::DInt(7)),
    });
    state.watch_changed = false;

    update_watch_snapshot(&mut state, &mut ctx);
    assert!(!state.watch_changed);
}

#[test]
fn watch_evaluation_failure_transitions_prior_value_to_none() {
    let control = DebugControl::new();
    let registry = TypeRegistry::new();
    let mut storage = VariableStorage::new();
    let mut ctx = context(&mut storage, &registry, 0);
    let (lock, _) = &*control.state;
    let mut state = lock.lock().expect("state");
    state.watches.push(WatchEntry {
        expr: Expr::Name("missing".into()),
        last: Some(Value::DInt(7)),
    });

    update_watch_snapshot(&mut state, &mut ctx);
    assert_eq!(state.watches[0].last, None);
    assert!(state.watch_changed);
}

#[test]
fn running_hook_records_location_and_call_depth_without_stop() {
    let control = DebugControl::new();
    let location = SourceLocation::new(1, 10, 20);
    let mut hook = control.clone();
    hook.on_statement(Some(&location), 7);

    assert_eq!(control.last_location(), Some(location));
    assert_eq!(control.last_call_depth(), 7);
    assert!(control.drain_stops().is_empty());
}

#[test]
fn running_hook_records_absent_location_and_maximum_depth() {
    let control = DebugControl::new();
    let mut hook = control.clone();
    hook.on_statement(None, u32::MAX);
    assert_eq!(control.last_location(), None);
    assert_eq!(control.last_call_depth(), u32::MAX);
}

#[test]
fn context_hook_binds_current_frame_to_statement() {
    let control = DebugControl::new();
    let registry = TypeRegistry::new();
    let mut storage = VariableStorage::new();
    let frame = storage.push_frame("main");
    let location = SourceLocation::new(1, 10, 20);
    let mut ctx = context(&mut storage, &registry, 0);
    let mut hook = control.clone();
    hook.on_statement_with_context(&mut ctx, Some(&location), 1);

    assert_eq!(control.frame_location(frame), Some(location));
}

#[test]
fn context_hook_prunes_locations_for_frames_no_longer_present() {
    let control = DebugControl::new();
    {
        let (lock, _) = &*control.state;
        let mut state = lock.lock().expect("state");
        state
            .frame_locations
            .insert(FrameId(99), SourceLocation::new(1, 1, 2));
    }
    let registry = TypeRegistry::new();
    let mut storage = VariableStorage::new();
    let mut ctx = context(&mut storage, &registry, 0);
    let mut hook = control.clone();
    hook.on_statement_with_context(&mut ctx, None, 0);
    assert!(control.frame_locations().is_empty());
}

#[test]
fn running_context_hook_does_not_invent_paused_snapshot() {
    let control = DebugControl::new();
    let registry = TypeRegistry::new();
    let mut storage = VariableStorage::new();
    let mut ctx = context(&mut storage, &registry, 0);
    let mut hook = control.clone();
    hook.on_statement_with_context(
        &mut ctx,
        Some(&SourceLocation::new(1, 0, 1)),
        0,
    );
    assert!(control.snapshot().is_none());
}

#[test]
fn paused_non_target_thread_records_boundary_without_blocking_or_consuming_stop() {
    let control = DebugControl::new();
    {
        let (lock, _) = &*control.state;
        let mut state = lock.lock().expect("state");
        state.mode = DebugMode::Paused;
        state.current_thread = Some(1);
        state.target_thread = Some(2);
        state.pending_stop = Some(DebugStopReason::Pause);
    }
    let location = SourceLocation::new(1, 10, 20);
    let mut hook = control.clone();
    hook.on_statement(Some(&location), 3);

    assert_eq!(control.last_location(), Some(location));
    assert_eq!(control.last_call_depth(), 3);
    assert!(control.drain_stops().is_empty());
    let (lock, _) = &*control.state;
    let state = lock.lock().expect("state");
    assert_eq!(state.pending_stop, Some(DebugStopReason::Pause));
}
