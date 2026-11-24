//! `trust-runtime ctl record` against a running computer runtime (specification 11 §6.8.1, "Cycle
//! traces"; specification 48 RU-59): the command records the resource's next cycles and saves the
//! trace as a `.trusttrace` file that the core's reader reads back, and Ctrl+C ends a recording
//! early and saves what it holds. Every test runs the real binary on a private copy of the example
//! `memory_marker_counter`: the resource `MarkerRes` of 50 ms, whose task `Cycle`, declared first,
//! runs every 50 ms, in production mode with the debug surface off, which a recording does not need.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

#[path = "support/runtime_process.rs"]
mod runtime_process;

use std::path::Path;

use runtime_process::{describe, stderr_text, ExampleRuntime, CLIMB};
use trust_runtime_core::trace::{decode, Trace, TraceEnd, TraceOrigin};

const EXAMPLE: &str = "memory_marker_counter";

fn read_trace(path: &Path) -> Trace {
    let bytes =
        std::fs::read(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let decoded = decode(&bytes).expect("the saved trace decodes");
    assert_eq!(decoded.damaged_cycle, None);
    decoded.trace
}

/// The cycles follow each other in logical time, and each task activation has its cycle's time:
/// every task runs at its scan's time (specification 10 §4.3). On a computer that time is the
/// clock's when the cycle began, so it lies near the 50 ms grid rather than on it, and task `Cycle`
/// (index 0), whose interval is the resource's, ran in the recording.
fn assert_cycles_in_order(trace: &Trace) {
    for pair in trace.cycles.windows(2) {
        assert!(
            pair[1].time_ns > pair[0].time_ns,
            "a cycle at {} ns after one at {} ns",
            pair[1].time_ns,
            pair[0].time_ns
        );
    }
    for cycle in &trace.cycles {
        for run in &cycle.activations {
            assert_eq!(
                run.now_ns, cycle.time_ns,
                "task {} ran at another time than its cycle's",
                run.task
            );
        }
    }
    assert!(
        trace
            .cycles
            .iter()
            .any(|cycle| cycle.activations.iter().any(|run| run.task == 0)),
        "task Cycle never ran: {:?}",
        trace.cycles
    );
}

#[test]
fn ctl_record_saves_the_next_cycles_as_a_trace_that_reads_back() {
    // Specification 11: `ctl record --cycles 5 --out <file>` records the resource's next 5 cycles,
    // says on standard error which recording runs, and prints `recorded 5 cycles (complete) to
    // <file>, <L> bytes`, L being the file's length. Worked by hand from the example: the trace
    // names the resource `MarkerRes`, its cycle interval of 50 000 000 ns and the runtime's
    // version, ended complete with 5 cycles in order. Catches a recording that is not
    // started, saved or checked, a file other than the runtime's trace, and a goal not kept.
    let runtime = ExampleRuntime::new(EXAMPLE).start();
    runtime.wait_for_snapshot(CLIMB, "the program running", |snapshot| {
        snapshot.scan() >= 2
    });
    let folder = trust_test_support::unique_temp_path("ctl-record");
    std::fs::create_dir_all(&folder).expect("create the output folder");
    let out = folder.join("cycles.trusttrace");
    let path = out.to_str().expect("the temporary path is text");

    let output = runtime.ctl(&["record", "--cycles", "5", "--out", path]);

    assert!(output.status.success(), "{}", describe(&output));
    let length = std::fs::metadata(&out).expect("the saved trace").len();
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("recorded 5 cycles (complete) to {path}, {length} bytes\n")
    );
    let stderr = stderr_text(&output);
    assert!(stderr.contains("recording trace 1, "), "{stderr}");
    let trace = read_trace(&out);
    assert_eq!(trace.meta.end, TraceEnd::Complete);
    assert_eq!(trace.meta.resource, "MarkerRes");
    assert_eq!(trace.meta.cycle_interval_ns, 50_000_000);
    assert_eq!(trace.meta.runtime_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(trace.meta.origin, TraceOrigin::OnDemand);
    assert_eq!((trace.meta.cycle_count, trace.cycles.len()), (5, 5));
    assert_cycles_in_order(&trace);
    let _ = std::fs::remove_dir_all(folder);
}

#[cfg(unix)]
#[test]
fn ctl_record_saves_what_the_recording_holds_on_ctrl_c() {
    // Specification 11: while a recording runs, another `ctl record` is refused (`trace_busy`) and
    // ends with 1; Ctrl+C sends `trace.stop` and saves what the trace holds, ending with 0; without
    // `--out`, the file is `trace-<UTC date and time>.trusttrace` in the current folder, as
    // `trace-20261124T013000Z.trusttrace`. Worked by hand: the recording of a million cycles
    // holds at least the 3 the test waits for when it is stopped, and ends `stopped` with the
    // cycles that `recorded` prints. Catches Ctrl+C killing the command without the file, a
    // stopped recording not saved, a refusal that ends with 0, and a default name in another form.
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;

    use runtime_process::{trust_runtime_command, WAIT};

    let runtime = ExampleRuntime::new(EXAMPLE).start();
    runtime.wait_for_snapshot(CLIMB, "the program running", |snapshot| {
        snapshot.scan() >= 2
    });
    let folder = trust_test_support::unique_temp_path("ctl-record-interrupted");
    std::fs::create_dir_all(&folder).expect("create the current folder");
    let mut child = trust_runtime_command()
        .current_dir(&folder)
        .arg("ctl")
        .arg("--project")
        .arg(runtime.project())
        .args(["record", "--cycles", "1000000"])
        .env_remove("TRUST_CTL_TOKEN")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start ctl record");
    let mut stderr = BufReader::new(child.stderr.take().expect("ctl record's standard error"));
    let mut line = String::new();
    stderr
        .read_line(&mut line)
        .expect("read ctl record's first line");
    assert!(line.starts_with("recording trace 1, "), "{line:?}");

    let busy = runtime.ctl(&["record", "--cycles", "1"]);
    assert_eq!(busy.status.code(), Some(1), "{}", describe(&busy));
    assert!(
        stderr_text(&busy).contains("trace_busy"),
        "{}",
        describe(&busy)
    );

    runtime.wait_until(WAIT, "3 recorded cycles", || {
        let state = runtime.control_request(serde_json::json!({
            "id": 1, "type": "trace.read", "params": {"trace": 1}
        }));
        state["result"]["cycles"]
            .as_u64()
            .is_some_and(|cycles| cycles >= 3)
    });
    let interrupted = std::process::Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .expect("send Ctrl+C's SIGINT");
    assert!(interrupted.success());
    let output = child.wait_with_output().expect("ctl record ends");

    assert!(output.status.success(), "{}", describe(&output));
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let (cycles, rest) = stdout
        .strip_prefix("recorded ")
        .and_then(|rest| rest.split_once(" cycles (stopped) to "))
        .unwrap_or_else(|| panic!("a stopped recording's line: {stdout:?}"));
    let (file, _) = rest.split_once(", ").expect("the file and its length");
    let stamp = file
        .strip_prefix("trace-")
        .and_then(|stamp| stamp.strip_suffix(".trusttrace"))
        .unwrap_or_else(|| panic!("a default file name: {file:?}"));
    let shape: String = stamp
        .chars()
        .map(|ch| if ch.is_ascii_digit() { '9' } else { ch })
        .collect();
    assert_eq!(shape, "99999999T999999Z", "{file}");
    let trace = read_trace(&folder.join(file));
    assert_eq!(trace.meta.end, TraceEnd::Stopped);
    let cycles: usize = cycles.parse().expect("the cycles recorded");
    assert!(cycles >= 3, "{stdout}");
    assert_eq!(trace.cycles.len(), cycles);
    assert_cycles_in_order(&trace);
    let _ = std::fs::remove_dir_all(folder);
}
