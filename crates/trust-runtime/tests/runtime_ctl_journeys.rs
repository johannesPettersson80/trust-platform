//! The documented `ctl` workflows, run against the real `trust-runtime play` process: a real
//! control endpoint, real scans and real I/O. Each test owns one runtime.

#[path = "support/runtime_process.rs"]
mod runtime_process;

use std::time::Duration;

use runtime_process::{hold_for, stderr_text, ExampleRuntime, CLIMB, WAIT};

/// The stable refusal in control/program_handlers.rs (STEP_REQUIRES_HOLD_MESSAGE).
const STEP_REQUIRES_HOLD: &str =
    "stepping requires the scan to be held by the debugger (a breakpoint or a debug-mode pause)";

#[test]
fn io_write_of_an_integer_to_an_int_binding_keeps_the_plc_running() {
    // #135: `io-write %MW0 41` stored INT 41 into the INT binding, and the PLC faulted and halted.
    // Spec 11, "Runtime control I/O and variable mutation contract": an untyped integer takes the
    // declared type of its destination. prove_counter_write shows that the write landed, by
    // arithmetic on the snapshots, with no time window.
    let mut runtime = ExampleRuntime::new("memory_marker_counter").start();
    let write = runtime.prove_counter_write(41, 1);
    let after = &write.after;
    // %QW0 latched the cycle-start value 41 at the landing cycle, and then one per scan.
    assert_eq!(
        after.int("%QW0") + 1,
        after.int("%MW0"),
        "%QW0 must sit one below the write-back"
    );
    assert_eq!(
        after.int("%QW0"),
        41 + (after.scan() as i64 - write.landing_scan),
        "%QW0 must latch 41 at the landing cycle"
    );
    runtime.assert_running_without_fault();

    runtime.ctl_ok(&["io-force", "%MW0", "41"]);
    runtime.wait_for_snapshot(WAIT, "%MW0 forced at 41", |s| {
        s.int("%MW0") == 41 && s.forced("%MW0")
    });
    // Five more scans while forced: each one still reads 41, and the scan counter advances.
    let mut scan = runtime.scan();
    for _ in 0..5 {
        let next =
            runtime.wait_for_snapshot(WAIT, "the next scan while forced", |s| s.scan() > scan);
        assert_eq!(
            next.int("%MW0"),
            41,
            "forced %MW0 changed at scan {}",
            next.scan()
        );
        assert!(
            next.forced("%MW0"),
            "%MW0 lost its force at scan {}",
            next.scan()
        );
        scan = next.scan();
    }

    runtime.ctl_ok(&["io-unforce", "%MW0"]);
    runtime.wait_for_snapshot(WAIT, "%MW0 counting again after io-unforce", |s| {
        !s.forced("%MW0") && s.int("%MW0") > 41
    });
    runtime.assert_running_without_fault();
}

#[test]
fn out_of_range_io_write_is_rejected_without_a_fault() {
    // Spec 11 (I/O contract) and spec 34 section 4.3.1: 40000 does not fit INT, so only that
    // request is rejected, as a write_rejected event, and the PLC keeps running. This test asserts
    // the observable effects on the process image; rejected_io_write_is_listed_by_ctl_events
    // checks the event itself through `ctl events`.
    let mut runtime = ExampleRuntime::new("memory_marker_counter").start();
    let before = runtime
        .wait_for_snapshot(WAIT, "the first scan", |_| true)
        .scan();

    let queued = runtime.ctl_ok(&["io-write", "%MW0", "40000"]);
    assert!(
        queued.contains("queued"),
        "io-write must accept the request: {queued}"
    );
    // Three scans bring the queued request to a scan boundary, where it is applied or rejected.
    let after = runtime.wait_for_snapshot(WAIT, "three scans after the write", |s| {
        s.scan() >= before + 3
    });
    let later = runtime.wait_for_snapshot(WAIT, "two more scans", |s| s.scan() >= after.scan() + 2);

    assert_ne!(
        after.int("%MW0"),
        40_000,
        "a rejected write must not reach %MW0"
    );
    assert_ne!(
        later.int("%MW0"),
        40_000,
        "a rejected write must not reach %MW0"
    );
    // The counter keeps counting one per scan, so the rejected request changed nothing.
    let scans = (later.scan() - after.scan()) as i64;
    assert_eq!(
        later.int("%MW0") - after.int("%MW0"),
        scans,
        "%MW0 must keep counting one per scan"
    );
    runtime.assert_running_without_fault();
}

#[test]
fn production_pause_refuses_steps_and_resume_restarts_the_scan() {
    // #136: after a production `pause` the scheduler holds the scan, not the debugger, so there
    // is no hold to step from. Each step is refused before anything is armed (program_handlers.rs).
    let mut runtime = ExampleRuntime::new("memory_marker_counter")
        .debug_enabled(true)
        .start();
    runtime.wait_for_snapshot(WAIT, "a running scan", |s| s.scan() >= 2);

    runtime.ctl_ok(&["pause"]);
    runtime.wait_until(WAIT, "the production pause to report paused", || {
        runtime.status().state == "paused"
    });
    let paused_at = runtime.scan();
    // Several 50 ms scans: a running scheduler would move the counter within this window.
    hold_for(Duration::from_millis(300));
    assert_eq!(
        runtime.scan(),
        paused_at,
        "the scan counter must stop while paused"
    );

    for step in ["step-over", "step-in", "step-out"] {
        let output = runtime.ctl(&[step]);
        assert!(
            !output.status.success(),
            "`ctl {step}` must be refused after a production pause"
        );
        assert!(
            stderr_text(&output).contains(STEP_REQUIRES_HOLD),
            "`ctl {step}` must be refused with the stable message: {}",
            stderr_text(&output)
        );
    }
    assert_eq!(
        runtime.scan(),
        paused_at,
        "refused steps must not move the scan"
    );

    runtime.ctl_ok(&["resume"]);
    runtime.wait_for_snapshot(WAIT, "the scan to advance after resume", |s| {
        s.scan() > paused_at
    });
    runtime.assert_running_without_fault();
}

#[test]
fn debug_mode_pause_accepts_a_step_and_resume_continues() {
    // Debug mode: `pause` holds the scan at the debugger, which then accepts a step (spec 11,
    // section 6.9). debug_enabled is set explicitly because the example's runtime.toml sets it to
    // false, and an explicit value overrides the mode.
    let mut runtime = ExampleRuntime::new("memory_marker_counter")
        .mode("debug")
        .debug_enabled(true)
        .start();
    runtime.wait_for_snapshot(WAIT, "a running scan", |s| s.scan() >= 2);

    runtime.ctl_ok(&["pause"]);
    runtime.wait_until(WAIT, "the debugger hold to report paused", || {
        runtime.status().state == "paused"
    });
    runtime.ctl_ok(&["step-over"]);

    let held_at = runtime.scan();
    runtime.ctl_ok(&["resume"]);
    runtime.wait_for_snapshot(WAIT, "the scan to advance after resume", |s| {
        s.scan() > held_at
    });
    runtime.wait_until(WAIT, "status to report running after resume", || {
        runtime.status().state == "running"
    });
    runtime.assert_running_without_fault();
}

#[test]
fn status_health_tasks_and_config_render_the_documented_summaries() {
    // Spec 11 section 6.8.1: the status, health and task renderings, and config-get (the CLI
    // spelling of the config.get request) reporting the vm backend from its default source.
    let runtime = ExampleRuntime::new("memory_marker_counter").start();
    runtime.wait_until(WAIT, "status to report running", || {
        runtime.status().state == "running"
    });

    let status = runtime.status();
    assert_eq!(status.fault, "none");
    assert_eq!(status.rt_profile, "disabled");
    assert_eq!(status.rt_active, "false");
    assert_eq!(runtime.ctl_ok(&["health"]).trim(), "ok=true");

    runtime.wait_until(WAIT, "task statistics for the Cycle task", || {
        runtime
            .ctl_ok(&["stats"])
            .lines()
            .any(|line| line.starts_with("task=Cycle "))
    });
    let stats = runtime.ctl_ok(&["stats"]);
    let task = stats
        .lines()
        .find(|line| line.starts_with("task=Cycle "))
        .expect("a stats line for the Cycle task");
    let fields: Vec<&str> = task
        .split(' ')
        .map(|field| field.split('=').next().unwrap_or(""))
        .collect();
    assert_eq!(
        fields,
        ["task", "min_ms", "avg_ms", "max_ms", "last_ms", "overruns"],
        "{task}"
    );

    let config = runtime.ctl_result(&["config-get"]);
    assert_eq!(config["runtime.execution_backend"], "vm", "{config}");
    assert_eq!(
        config["runtime.execution_backend_source"], "default",
        "{config}"
    );
}

#[test]
fn cold_restart_keeps_the_runtime_serving() {
    // The operator guide's command, `restart cold`. A cold restart resets the scan counter to zero
    // (installed_lifecycle.rs, finish_restart), so the restart shows as a drop below the old count,
    // and the counter must then climb again from the reset. The first wait takes the counter well
    // past zero so that the reset is easy to see.
    let mut runtime = ExampleRuntime::new("memory_marker_counter").start();
    let before = runtime
        .wait_for_snapshot(CLIMB, "scan 40 or later", |s| s.scan() >= 40)
        .scan();

    runtime.ctl_ok(&["restart", "cold"]);
    let reset = runtime.wait_for_snapshot(WAIT, "the scan counter to reset", |s| s.scan() < before);
    runtime.wait_for_snapshot(WAIT, "the scan counter to advance after the restart", |s| {
        s.scan() > reset.scan()
    });
    runtime.assert_running_without_fault();
}

#[test]
fn shutdown_stops_the_runtime_process() {
    let mut runtime = ExampleRuntime::new("memory_marker_counter").start();
    runtime.wait_for_snapshot(WAIT, "a running scan", |_| true);

    runtime.ctl_ok(&["shutdown"]);
    let exit = runtime.wait_for_exit(Duration::from_secs(10));
    assert!(
        exit.success(),
        "the runtime must exit successfully after shutdown: {exit}\n{}",
        runtime.diagnostics()
    );
}

#[test]
fn boolean_io_write_is_visible_in_io_read() {
    // The mitsubishi example binds %QX0.0 to an output that its program assigns on every scan
    // (RiseSeen := Bridge.Rising), so a write there is overwritten at once. Its %IX0.0 (Pulse) is
    // only read by the program, and the simulated driver leaves the input image alone
    // (io/driver.rs, SimulatedIoDriver::read_inputs), so a written TRUE stays visible. The brief
    // allows this input choice when the program drives the output.
    let mut runtime = ExampleRuntime::new("mitsubishi_gxworks3_v1").start();
    runtime.wait_for_snapshot(WAIT, "a running scan", |_| true);

    let queued = runtime.ctl_ok(&["io-write", "%IX0.0", "TRUE"]);
    assert!(
        queued.contains("queued"),
        "io-write must queue the write: {queued}"
    );
    runtime.wait_for_snapshot(WAIT, "%IX0.0 to read TRUE", |s| {
        s.text("%IX0.0") == "Bool(true)"
    });
    runtime.assert_running_without_fault();
}

#[test]
fn rejected_io_write_is_listed_by_ctl_events() {
    // Spec 11 section 6.8.1: `ctl events` prints the raw events.tail response, so the
    // write_rejected event of an out-of-range write is visible from the command line. The runtime
    // keeps its newest 1,000 events and each 50 ms scan adds several, so the poll asks for the
    // largest page.
    let mut runtime = ExampleRuntime::new("memory_marker_counter").start();
    runtime.wait_for_snapshot(WAIT, "the first scan", |_| true);

    let queued = runtime.ctl_ok(&["io-write", "%MW0", "40000"]);
    assert!(
        queued.contains("queued"),
        "io-write must accept the request: {queued}"
    );
    runtime.wait_until(WAIT, "ctl events to list write_rejected", || {
        runtime
            .ctl_ok(&["events", "--limit", "1000"])
            .contains("write_rejected")
    });

    // Without --limit the request carries no parameters, so the runtime's default page comes back.
    let events = runtime.ctl_result(&["events"]);
    assert!(
        events["events"].is_array(),
        "ctl events must return an events array: {events}"
    );
    runtime.assert_running_without_fault();
}

#[test]
fn ctl_faults_accepts_a_limit_and_lists_no_fault_on_a_healthy_runtime() {
    // Spec 11 section 6.8.1: `faults --limit 1` sends the limit with the request. A runtime
    // that has not faulted lists no fault.
    let mut runtime = ExampleRuntime::new("memory_marker_counter").start();
    runtime.wait_for_snapshot(WAIT, "a running scan", |_| true);

    let faults = runtime.ctl_result(&["faults", "--limit", "1"]);
    let listed = faults["faults"]
        .as_array()
        .expect("ctl faults must return a faults array");
    assert!(
        listed.is_empty(),
        "a healthy runtime lists no faults: {faults}"
    );
    runtime.assert_running_without_fault();
}

/// The memory-marker counter, extended so that writing 1000 to its counter input faults the PLC.
/// The retained flag makes the fault happen once, so a warm restart does not trip it again. The
/// counter climbs by one per scan, so it cannot reach 1000 by itself within a test.
fn counter_faults_on_a_written_value() -> ExampleRuntime {
    ExampleRuntime::new("memory_marker_counter")
        .edit_source(
            "src/Main.st",
            "END_VAR\n\nCounterLatched := Counter;\n",
            "END_VAR\nVAR RETAIN\n    Tripped : BOOL;\nEND_VAR\n\nCounterLatched := Counter;\n",
        )
        .edit_source(
            "src/Main.st",
            "Counter := Counter + 1;\nEND_PROGRAM",
            "Counter := Counter + 1;\nIF NOT Tripped AND CounterLatched = 1000 THEN\n    Tripped := TRUE;\n    CounterLatched := 100 / (CounterLatched - 1000);\nEND_IF;\nEND_PROGRAM",
        )
}

#[test]
fn halting_fault_keeps_the_process_serving_until_a_warm_restart_recovers_it() {
    // Spec 11 section 6.6, "Faulted resource lifecycle (hosted runtime)": the process keeps running
    // and `status` reports the fault. `restart warm` runs the warm initialization and resumes.
    let mut runtime = counter_faults_on_a_written_value().start();
    runtime.wait_for_snapshot(WAIT, "a running scan", |s| s.scan() >= 3);

    runtime.ctl_ok(&["io-write", "%MW0", "1000"]);
    runtime.wait_until(WAIT, "the division fault to be reported", || {
        runtime.ctl_ok(&["status"]).contains("state=faulted")
    });
    let line = runtime.ctl_ok(&["status"]);
    assert!(
        line.contains("state=faulted") && line.contains("fault=division by zero"),
        "status must report the root fault: {line}"
    );
    assert!(
        runtime.is_running(),
        "a halting fault must not end the process\n{}",
        runtime.diagnostics()
    );

    runtime.ctl_ok(&["restart", "warm"]);
    runtime.wait_until(WAIT, "the warm restart to resume the scan", || {
        runtime.ctl_ok(&["status"]).contains("state=running")
    });
    runtime.wait_for_snapshot(WAIT, "a scan after the warm restart", |s| s.scan() >= 1);
    runtime.assert_running_without_fault();
}

#[test]
fn stopped_faulted_runtime_exits_with_status_one() {
    // Spec 11 section 6.6: the process exits with status 0 when the resource ends Stopped and with
    // status 1 when it ends Faulted, so a service manager can tell a clean stop from a failure.
    let mut runtime = counter_faults_on_a_written_value().start();
    runtime.wait_for_snapshot(WAIT, "a running scan", |s| s.scan() >= 3);

    runtime.ctl_ok(&["io-write", "%MW0", "1000"]);
    runtime.wait_until(WAIT, "the division fault to be reported", || {
        runtime.ctl_ok(&["status"]).contains("state=faulted")
    });

    runtime.ctl_ok(&["shutdown"]);
    let exit = runtime.wait_for_exit(Duration::from_secs(10));
    assert_eq!(
        exit.code(),
        Some(1),
        "a runtime that stops while faulted must exit with status 1\n{}",
        runtime.diagnostics()
    );
}
