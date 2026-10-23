//! Checked admission: a debugger-queued write cannot move a debug-frame reference into PLC state.

use trust_runtime::{
    debug::RuntimeEvent,
    error::RuntimeError,
    harness::CompileSession,
    memory::{FrameId, MemoryLocation},
    value::Value,
};
use trust_runtime_core::value::ValueRef;

/// Spec 34 §4.3.1: references to debug scratch storage shall not escape into PLC state.
#[test]
fn queued_debug_write_refuses_a_frame_scoped_reference_without_plc_change() {
    let mut runtime = CompileSession::from_source(
        "VAR_GLOBAL frame_target : REF_TO INT; END_VAR PROGRAM Main END_PROGRAM",
    )
    .build_runtime()
    .unwrap();
    let debug = runtime.enable_debug();
    let before = runtime.storage().get_global("frame_target").cloned();
    // Debugger frames are frame-local (MemoryLocation::Local). No paused frame is
    // set up here, so the frame-local reference is built directly. A global
    // destination has no frame to outlive, so the write is refused whether or
    // not the activation is still live.
    let frame_reference = ValueRef {
        location: MemoryLocation::Local(FrameId(u32::MAX)),
        offset: 0,
        path: Vec::new(),
    };
    debug.enqueue_global_write("frame_target", Value::Reference(Some(frame_reference)));

    runtime
        .execute_cycle()
        .expect("a refused debugger write must not fault the scan");
    assert!(!runtime.faulted());
    assert_eq!(
        runtime.storage().get_global("frame_target").cloned(),
        before
    );
    let events = debug.drain_runtime_events();
    assert!(
        events.iter().any(|event| matches!(event,
            RuntimeEvent::WriteRejected { target, error, .. }
                if target.as_str() == "frame_target"
                    && error == &RuntimeError::ReferenceLifetime.to_string())),
        "expected a ReferenceLifetime rejection for the frame-scoped reference, got {events:?}"
    );
}
