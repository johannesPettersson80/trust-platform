//! `trust-runtime ctl fault-trace` against a running computer runtime (specification 11 §6.8.1,
//! "Cycle traces"; specification 48 RU-59, "The flight recorder"): with `[runtime.trace]
//! flight_recorder` the runtime keeps its last cycles, and after a fault the command saves them as a
//! `.trusttrace` file, which the runtime has also written into its project folder's `traces`. The
//! test runs the real binary on a private copy of the example `memory_marker_counter`, whose
//! resource `MarkerRes` runs its task `Cycle` every 50 ms and halts on a fault; the copy divides by
//! zero when its count reaches 200.
//!
//! The test needs the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

#[path = "support/runtime_process.rs"]
mod runtime_process;

use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

use runtime_process::{describe, stderr_text, trust_runtime_command, ExampleRuntime, Runtime};
use serde_json::json;
use trust_runtime_core::trace::{decode, Trace, TraceEnd, TraceOrigin};

const EXAMPLE: &str = "memory_marker_counter";

/// The 200 cycles of 50 ms before the fault, with room for a slow computer.
const UNTIL_THE_FAULT: Duration = Duration::from_secs(60);

/// `trust-runtime ctl fault-trace` without `--out`, run in `folder`, where it writes its file.
fn fault_trace_in(runtime: &Runtime, folder: &Path) -> Output {
    trust_runtime_command()
        .current_dir(folder)
        .arg("ctl")
        .arg("--project")
        .arg(runtime.project())
        .arg("fault-trace")
        .env_remove("TRUST_CTL_TOKEN")
        .output()
        .expect("run trust-runtime ctl fault-trace")
}

/// The files in `folder`.
fn files_in(folder: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(folder)
        .unwrap_or_else(|error| panic!("read {}: {error}", folder.display()))
        .map(|entry| entry.expect("an entry").path())
        .collect()
}

/// The name of `path`, which must be `fault-<UTC date and time>.trusttrace`: `fault-`, the date and
/// time in the 16 characters of `20261124T013000Z`, and `.trusttrace`.
fn fault_file_name(path: &Path) -> String {
    let name = path
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    let stamp = name
        .strip_prefix("fault-")
        .and_then(|rest| rest.strip_suffix(".trusttrace"))
        .unwrap_or_else(|| panic!("{name} is no fault-<UTC>.trusttrace"));
    let shape: String = stamp
        .chars()
        .map(|c| if c.is_ascii_digit() { '9' } else { c })
        .collect();
    assert_eq!(shape, "99999999T999999Z", "{name}");
    name
}

fn read_trace(path: &Path) -> Trace {
    let bytes =
        std::fs::read(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let decoded = decode(&bytes).expect("the saved trace decodes");
    assert_eq!(decoded.damaged_cycle, None);
    decoded.trace
}

#[test]
fn ctl_fault_trace_saves_the_cycles_before_a_fault_and_the_runtime_keeps_a_copy() {
    // Specification 11 §6.8.1: `ctl fault-trace` saves the fault's trace as `ctl record` saves a
    // recording, by default as `fault-<UTC date and time>.trusttrace` in the current folder, and
    // prints `saved the fault's trace of <n> cycles (<reason>) to <file>, <L> bytes`; a runtime
    // with a folder of its files also writes the trace there as `traces/fault-<UTC date and
    // time>.trusttrace`; a runtime that keeps no fault's trace ends the command with 1 and the
    // reason. Worked by hand: the copy keeps a flight recorder of 4 cycles. Its task counts in
    // `Counter` and first latches the count into `CounterLatched` at `%QW0`; the run that brings
    // the count to 200 divides by zero. The recordings of 4 cycles take turns, so the fault's trace
    // joins the complete recording before the faulted one, 4 cycles, and the faulted one, 1 to 4:
    // 5 to 8 cycles, only the last with the fault. The task's last run before it latched 198, and
    // `CounterLatched` changes only when the task runs, by 1 each time, so the cycles before the
    // fault latch counts that rise by 0 or 1 to 198 (8 cycles latching 192 to 198 when every cycle
    // ran the task; a computer's cycle late by more than an interval runs without it,
    // specification 10 §4.3). Catches a trace saved before a fault, a fault's trace that the
    // command cannot read or names otherwise, a trace that is not the flight recorder's last
    // cycles or loses one where the two recordings join, and a runtime that keeps no copy.
    let runtime = ExampleRuntime::new(EXAMPLE)
        .edit_source(
            "src/Main.st",
            "Counter := Counter + 1;",
            "Counter := Counter + 1;\nIF Counter = 200 THEN\n    Counter := Counter / (Counter - 200);\nEND_IF;",
        )
        .edit_source(
            "runtime.toml",
            "[runtime.log]",
            "[runtime.trace]\nflight_recorder = 4\n\n[runtime.log]",
        )
        .start();
    let folder = trust_test_support::unique_temp_path("ctl-fault-trace");
    std::fs::create_dir_all(&folder).expect("create the output folder");

    // The fault comes 10 s after the start: there is no fault's trace yet.
    let early = fault_trace_in(&runtime, &folder);
    assert_eq!(early.status.code(), Some(1), "{}", describe(&early));
    assert!(
        stderr_text(&early).contains("keeps no fault's trace"),
        "{}",
        describe(&early)
    );
    assert!(files_in(&folder).is_empty());

    // The status request, not the summary of `ctl status`, which prints a fault's text as it is,
    // spaces and all (specification 11 §6.8.1).
    runtime.wait_until(UNTIL_THE_FAULT, "the fault", || {
        runtime.control_request(json!({"id": 2, "type": "status"}))["result"]["state"]
            == json!("faulted")
    });
    let output = fault_trace_in(&runtime, &folder);

    assert!(output.status.success(), "{}", describe(&output));
    let saved = files_in(&folder);
    assert_eq!(saved.len(), 1, "{saved:?}");
    let name = fault_file_name(&saved[0]);
    let trace = read_trace(&saved[0]);
    let cycles = trace.cycles.len();
    let length = std::fs::metadata(&saved[0]).expect("the saved trace").len();
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("saved the fault's trace of {cycles} cycles (faulted) to {name}, {length} bytes\n")
    );
    assert_eq!(trace.meta.origin, TraceOrigin::FlightRecorder);
    assert_eq!(trace.meta.end, TraceEnd::Faulted);
    assert_eq!(trace.meta.resource, "MarkerRes");
    assert_eq!(trace.meta.cycle_count, u64::try_from(cycles).unwrap());
    assert!((5..=8).contains(&cycles), "{cycles} cycles");
    let (faulted, before) = trace.cycles.split_last().expect("cycles");
    assert!(faulted.fault.is_some());
    assert!(before.iter().all(|cycle| cycle.fault.is_none()));
    let latched: Vec<i16> = before
        .iter()
        .map(|cycle| i16::from_le_bytes([cycle.outputs[0], cycle.outputs[1]]))
        .collect();
    assert_eq!(latched.last(), Some(&198), "{latched:?}");
    assert!(
        latched
            .windows(2)
            .all(|pair| pair[1] == pair[0] || pair[1] == pair[0] + 1),
        "{latched:?}"
    );

    let copies = files_in(&runtime.project().join("traces"));
    assert_eq!(copies.len(), 1, "{copies:?}");
    fault_file_name(&copies[0]);
    assert_eq!(
        std::fs::read(&copies[0]).expect("the runtime's copy"),
        std::fs::read(&saved[0]).expect("the saved trace")
    );
    let _ = std::fs::remove_dir_all(folder);
}
