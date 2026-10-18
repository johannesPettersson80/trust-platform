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
        storage_abi: Default::default(),
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
        emit_stop(&mut state, DebugStopReason::Pause, Some(location), None);
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

/// The last value that the registered watch evaluated to, if it has been evaluated.
fn watch_value(control: &DebugControl) -> Option<Value> {
    let (lock, _) = &*control.state;
    let state = lock.lock().expect("state");
    state.watches[0].last.clone()
}

/// Waits until the held statement reports a stop, and drains the stops it has reported.
fn wait_for_stop(control: &DebugControl) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while control.drain_stops().is_empty() {
        assert!(
            std::time::Instant::now() < deadline,
            "the statement did not stop"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// Spec 13 line 279 ("Watch-change state is recomputed for each stop from the current storage") and
/// spec 13 line 485 ("never a copy the runtime makes per statement"). The earlier version of this test
/// expected every statement to move its storage into the published state and to evaluate the watch.
/// Those lines replace that: a copy and a watch evaluation belong to a stop. The test pins the new rule.
/// A statement that does not stop keeps the live storage with its caller, publishes nothing and evaluates
/// no watch. Only a stop publishes a detached copy of the storage, and the stop evaluates the watch once.
/// Wrong behaviour caught: a statement that takes the live buffer or evaluates a watch without a stop, and
/// a stop that publishes the live buffer instead of a copy of it.
#[test]
fn only_a_stop_detaches_a_copy_and_evaluates_the_watches() {
    let registry = TypeRegistry::new();
    let mut control = DebugControl::new();
    control.register_watch_expression(Expr::Literal(Value::DInt(7)));
    let location = SourceLocation::new(1, 0, 10);
    let buffer = "A".repeat(80);
    let live = buffer.as_ptr();
    let mut storage = VariableStorage::new();
    storage.set_global("text", Value::WString(buffer));

    // A statement that does not stop: nothing is published, and no watch is evaluated.
    {
        let mut ctx = context(&mut storage, &registry, 42);
        control.on_statement_with_owned_context(&mut ctx, Some(&location), 0);
    }
    let Some(Value::WString(text)) = storage.get_global("text") else {
        panic!("the live storage keeps its text")
    };
    assert_eq!(
        text.as_ptr(),
        live,
        "a statement that does not stop leaves the live buffer in place"
    );
    assert!(
        control.snapshot().is_none(),
        "a statement that does not stop publishes nothing"
    );
    assert!(
        !control.take_watch_changed(),
        "a statement that does not stop evaluates no watch"
    );
    assert_eq!(watch_value(&control), None);

    // A stop: a pause holds the statement, so it runs on another thread while this test observes it.
    control.pause();
    let mut hook = control.clone();
    let worker = std::thread::spawn(move || {
        let registry = TypeRegistry::new();
        {
            let mut ctx = context(&mut storage, &registry, 42);
            hook.on_statement_with_owned_context(&mut ctx, Some(&location), 0);
        }
        storage
    });
    wait_for_stop(&control);
    let published = control
        .snapshot()
        .expect("the stop publishes its halted state");
    assert_eq!(published.now, Duration::from_millis(42));
    let Some(Value::WString(copy)) = published.storage.get_global("text") else {
        panic!("the stop publishes the text")
    };
    assert_ne!(
        copy.as_ptr(),
        live,
        "the stop publishes a detached copy, not the live buffer"
    );
    let expected = "A".repeat(80);
    assert_eq!(copy.as_str(), expected.as_str());
    assert!(
        control.take_watch_changed(),
        "the stop evaluates the watch and reports its change"
    );
    assert!(!control.take_watch_changed(), "the change is reported once");
    assert_eq!(watch_value(&control), Some(Value::DInt(7)));

    control.continue_run();
    let storage = worker
        .join()
        .expect("the held statement ends after the continue");
    let Some(Value::WString(live_again)) = storage.get_global("text") else {
        panic!("the live storage keeps its text")
    };
    assert_eq!(
        live_again.as_ptr(),
        live,
        "the scan's storage keeps its live buffer after the stop"
    );
}

#[test]
fn schema_observation_retirement_preserves_pause_and_clears_old_frame_handles() {
    let control = DebugControl::new();
    let registry = TypeRegistry::new();
    let mut storage = VariableStorage::new();
    let frame = storage.push_frame("retired");
    let location = SourceLocation::new(1, 4, 9);
    let mut ctx = context(&mut storage, &registry, 42);
    {
        let (lock, _) = &*control.state;
        let mut state = lock.lock().unwrap();
        state.mode = DebugMode::Paused;
        state.frame_locations.insert(frame, location);
        state.last_location = Some(location);
        update_snapshot(&mut state, &mut ctx);
        emit_stop(
            &mut state,
            DebugStopReason::Breakpoint,
            Some(location),
            None,
        );
    }
    control.register_watch_expression(Expr::Literal(Value::DInt(7)));
    control.invalidate_schema_observations();
    assert_eq!(control.mode(), DebugMode::Paused);
    assert!(control.snapshot().is_none());
    assert!(control.last_location().is_none());
    assert!(control.last_stop().is_none());
    assert!(control.frame_locations().is_empty());
    assert!(control.drain_stops().is_empty());
    let (lock, _) = &*control.state;
    let state = lock.lock().unwrap();
    assert!(state.watches.is_empty());
    assert_eq!(state.pending_stop, Some(DebugStopReason::Pause));
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
        registry: Some(std::sync::Arc::new(registry.clone())),
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
        registry: Some(std::sync::Arc::new(registry.clone())),
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
        registry: Some(std::sync::Arc::new(registry.clone())),
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
    hook.on_statement_with_context(&mut ctx, Some(&SourceLocation::new(1, 0, 1)), 0);
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
