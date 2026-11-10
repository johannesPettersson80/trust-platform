#![cfg(feature = "full")]
//! A debugger hold and the simulation the resource runs (specification 13, "Pause"; specification
//! 47): the plants' status and trace are read, and a plant's trace is cleared, in the simulation
//! itself, which the scan thread locks only for a cycle's plant steps, so a hold of the scan between
//! them keeps no such request waiting.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use std::time::{Duration as WallDuration, Instant};

use serde_json::{json, Value as Json};
use trust_runtime::debug::DebugControl;
use trust_runtime::harness::SourceFile;
use trust_runtime::native::{build, Options, Target, Toolchain};
use trust_runtime::scheduler::{ResourceRunner, StdClock};
use trust_runtime::simulation::{PlantTraceClearError, SimulationConfig, SimulationController};
use trust_runtime::value::Duration;
use trust_runtime::{CompiledDebugger, CompiledRuntime, RestartMode};
use trust_runtime_services::live_values::io_requests::IoSnapshotRead;
use trust_runtime_services::protocol::ControlRequest;

/// The path the program's source has, which the breakpoint request names.
const PATH: &str = "src/main.st";
/// Zero-based line 4, `count := count + 1;`, is where the breakpoint holds the scan.
const SOURCE: &str = "PROGRAM Main
VAR
    count : DINT;
END_VAR
count := count + 1;
END_PROGRAM
";

/// The JSON of the debugger's answer to the debug operation `kind` with `params`.
fn ask(debugger: &CompiledDebugger, kind: &str, params: Option<Json>) -> Json {
    let request = ControlRequest {
        id: 1,
        r#type: kind.to_string(),
        params,
        auth: None,
        request_id: None,
    };
    serde_json::to_value(debugger.answer(&request, IoSnapshotRead::Missing))
        .expect("a response serializes")
}

/// Whether the debugger records a stop within five seconds.
fn stopped(debugger: &CompiledDebugger) -> bool {
    let deadline = Instant::now() + WallDuration::from_secs(5);
    while Instant::now() < deadline {
        let answer = ask(debugger, "debug.stops", None);
        if answer["result"]["stops"]
            .as_array()
            .is_some_and(|stops| !stops.is_empty())
        {
            return true;
        }
        std::thread::sleep(WallDuration::from_millis(5));
    }
    false
}

/// Specification 13 "Pause" and specification 47: while the debugger holds the scan at a
/// breakpoint, the resource's control answers with the snapshot of the simulation it runs, active,
/// and answers a plant's trace clear with the simulation's own refusal, the revision `r1` not being
/// the active configuration's (none). Catches both asked of the held scan thread, as before, which
/// answered neither, so `plant.status`, `plant.trace.read` and the start of a finite search gave up
/// after 250 ms (the CODESYS toolkit's B15 and S15).
#[test]
fn a_held_scan_keeps_no_simulation_request_waiting() {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let work = trust_test_support::unique_temp_path("trust-hold-simulation");
    let built = build(
        &[SourceFile::with_path(PATH, SOURCE)],
        &Options {
            target: Target::this_cpu().expect("a computer target"),
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the program compiles");
    let _ = std::fs::remove_dir_all(&work);
    let mut runtime = CompiledRuntime::load(built.image, "RESOURCE").expect("the image loads");
    let debug = DebugControl::new();
    runtime.set_debug_control(debug.clone());
    runtime.restart(RestartMode::Cold).expect("the cold start");
    let debugger = debug.compiled().expect("the compiled program's debugger");
    let set = ask(
        &debugger,
        "breakpoints.set",
        Some(json!({ "source": PATH, "lines": [4] })),
    );
    assert_eq!(set["ok"], json!(true), "{set}");
    let config = SimulationConfig {
        enabled: true,
        ..SimulationConfig::default()
    };
    let mut handle = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(10))
        .with_simulation(SimulationController::new(config))
        .spawn("held-simulation")
        .expect("spawn the resource");
    let held = stopped(&debugger);
    let control = handle.control();
    let snapshot = control.simulation_snapshot();
    let cleared = control.clear_plant_trace("Tank", "r1");
    // A stop ends the hold without a resume (specification 11 §6.9).
    handle.stop();
    handle.join().expect("the resource thread ends");

    assert!(held, "the breakpoint holds the scan");
    assert!(
        matches!(&snapshot, Ok(Some(snapshot)) if snapshot.status.active),
        "{snapshot:?}"
    );
    assert_eq!(cleared, Ok(Err(PlantTraceClearError::RevisionConflict)));
}
