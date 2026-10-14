//! Installation and isolated restart construction publish no debugger stop or snapshot.
use super::*;
use crate::{
    debug::{DebugBreakpoint, DebugStopReason, SourceLocation},
    harness::CompileSession,
    value::{Duration, Value},
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

// Variable initializers must be constant expressions (E202), so construction runs the
// function-block default `kp := DINT#7` as constructor bytecode. Constructor bytecode
// has no statement source entries. The program body is the only statement that calls
// Probe, so it is the only place a breakpoint can stop.
const SOURCE: &str = "FUNCTION Probe : DINT\nVAR_INPUT seed : DINT; END_VAR\nProbe := seed + DINT#1;\nEND_FUNCTION\nFUNCTION_BLOCK Tuning\nVAR kp : DINT := DINT#7; END_VAR\nEND_FUNCTION_BLOCK\nVAR_GLOBAL gains : Tuning; END_VAR\nPROGRAM Main\nVAR result : DINT; END_VAR\nresult := Probe(DINT#4);\nEND_PROGRAM\n";

fn probe_location(runtime: &Runtime) -> SourceLocation {
    let prepared = runtime.prepared();
    let metadata = runtime.metadata_snapshot();
    let (_, source) = prepared
        .debug_map()
        .source_by_pc
        .iter()
        .filter(|((pou, _), _)| prepared.pou_name(*pou) == Some("Probe"))
        .min_by_key(|((_, pc), _)| *pc)
        .expect("Probe statement source");
    metadata
        .resolve_vm_debug_location(&source.file, source.line, source.column)
        .expect("Probe statement in immutable metadata")
}

fn tuning_kp(runtime: &Runtime) -> Option<Value> {
    let Some(Value::Instance(tuning)) = runtime.storage().get_global("gains") else {
        panic!("admitted function-block instance")
    };
    runtime.storage().get_instance_var(*tuning, "kp").cloned()
}

/// Spec 34 HOST-09 (lines 480-481): an installation or isolated restart candidate
/// publishes no snapshot or stop. Construction materializes the constructor defaults,
/// and the body's own call stops exactly once, in the cycle that runs before the
/// installation. Installing bytecode builds metadata from the prepared module, which
/// has no source text (metadata/prepared.rs:288), so the stop is checked first.
#[test]
fn hosted_observation_installation_and_restart_publish_no_stop_or_snapshot() {
    let mut installed = CompileSession::from_source(SOURCE).build_runtime().unwrap();
    let debug = installed.enable_debug();
    let location = probe_location(&installed);
    debug.set_breakpoints_for_file(location.file_id, vec![DebugBreakpoint::new(location)]);
    let (sender, receiver) = std::sync::mpsc::channel();
    debug.set_stop_sender(sender);
    let finished = Arc::new(AtomicBool::new(false));
    // A stop during candidate construction would pause the installing thread, so a
    // watcher releases each stop and keeps the list for the assertions below.
    let watcher = {
        let debug = debug.clone();
        let finished = Arc::clone(&finished);
        std::thread::spawn(move || {
            let mut stops = Vec::new();
            loop {
                match receiver.recv_timeout(std::time::Duration::from_millis(20)) {
                    Ok(stop) => {
                        stops.push(stop);
                        debug.continue_run();
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                        if !finished.load(Ordering::SeqCst) => {}
                    Err(_) => break,
                }
            }
            stops
        })
    };

    // Isolated restart: the candidate constructs the default and publishes nothing.
    installed.restart(RestartMode::Cold).unwrap();
    assert_eq!(
        tuning_kp(&installed),
        Some(Value::DInt(7)),
        "the isolated restart ran the constructor defaults"
    );
    assert!(
        debug.snapshot().is_none(),
        "the isolated restart publishes no snapshot"
    );

    // The body's own call is the one observation this installation may publish.
    installed.set_current_time(Duration::from_millis(10));
    installed.execute_cycle().unwrap();

    // Installation: the candidate constructs the default and publishes nothing.
    let replacement = CompileSession::from_source(SOURCE)
        .build_bytecode_module()
        .unwrap();
    installed.apply_bytecode_module(&replacement, None).unwrap();
    assert_eq!(
        tuning_kp(&installed),
        Some(Value::DInt(7)),
        "the installed candidate ran the constructor defaults"
    );
    assert!(
        debug.snapshot().is_none(),
        "installation publishes no snapshot"
    );

    finished.store(true, Ordering::SeqCst);
    let stops = watcher.join().unwrap();
    assert_eq!(
        stops.len(),
        1,
        "only the body's call stops; the restart and the installation stop nothing"
    );
    assert_eq!(stops[0].reason, DebugStopReason::Breakpoint);
}
