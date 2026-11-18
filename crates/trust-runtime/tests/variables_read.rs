//! `variables.read` on a computer (specification 11 §6.8.1, "Reading variables"; specification 48
//! RU-62): the real `trust-runtime` runs a private copy of the example `memory_marker_counter`, in
//! production with the debug surface off as the example sets it, and a client reads its variables
//! by their paths, as a Rust service reads them.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

#[path = "support/runtime_process.rs"]
mod runtime_process;

use serde_json::{json, Value};

use runtime_process::{hold_for, ExampleRuntime, Runtime, CLIMB, WAIT};

const EXAMPLE: &str = "memory_marker_counter";

fn read(runtime: &Runtime, id: u64, paths: &[&str]) -> Value {
    runtime.control_request(json!({
        "id": id,
        "type": "variables.read",
        "params": { "paths": paths }
    }))
}

/// The scan number, logical time, counter and latched counter of a successful read of both.
fn counters(reply: &Value) -> (i64, i64, i64, i64) {
    assert_eq!(reply["ok"], json!(true), "{reply}");
    let result = &reply["result"];
    let values = result["values"].as_array().expect("the values");
    (
        result["scan"].as_i64().expect("the scan's number"),
        result["time_ns"].as_i64().expect("the scan's logical time"),
        values[0]["value"]
            .as_i64()
            .expect("an INT is a JSON integer"),
        values[1]["value"]
            .as_i64()
            .expect("an INT is a JSON integer"),
    )
}

#[test]
fn a_production_runtime_reads_a_set_from_one_completed_scan() {
    // The example's P1 latches Counter into CounterLatched and then adds one to Counter, in every
    // scan of its 50 ms task, which runs in each 50 ms scan of the resource. So a set read from one
    // completed scan has CounterLatched = Counter - 1, and Counter less the scan's number is the
    // same in every read. Read in production with the debug surface off. Catches a read refused in
    // production or as a debug operation, a set torn across two scans, and a scan number that is
    // not the scan the values come from (another scan's number with these values).
    let runtime = ExampleRuntime::new(EXAMPLE).start();
    runtime.wait_for_snapshot(CLIMB, "five scans", |s| s.scan() >= 5);
    let mut last: Option<(i64, i64)> = None;
    let mut offset: Option<i64> = None;
    for id in 1..=5 {
        let reply = read(&runtime, id, &["p1.counter", "P1.CounterLatched"]);
        let result = &reply["result"];
        assert_eq!(result["state"], json!("running"), "{reply}");
        assert_eq!(result["quality"], json!("good"), "{reply}");
        assert_eq!(result["values"][0]["path"], json!("p1.counter"));
        assert_eq!(result["values"][0]["type"], json!("INT"));
        assert_eq!(result["values"][1]["path"], json!("P1.CounterLatched"));
        let (scan, time, counter, latched) = counters(&reply);
        assert_eq!(latched, counter - 1, "both values of one scan: {reply}");
        let this = counter - scan;
        assert_eq!(
            *offset.get_or_insert(this),
            this,
            "the values of scan {scan}: {reply}"
        );
        if let Some((scan_before, time_before)) = last {
            assert!(scan >= scan_before, "{reply}");
            if scan > scan_before {
                assert!(time > time_before, "a later scan has a later time: {reply}");
            }
        }
        last = Some((scan, time));
        hold_for(std::time::Duration::from_millis(120));
    }
}

#[test]
fn a_path_without_a_single_value_refuses_the_read() {
    // Specification 11: a path that names nothing, and one that names a program instance, which
    // holds no value of its own, refuse the whole request with the path. Catches a reply with a
    // gap or a null in place of the refusal.
    let runtime = ExampleRuntime::new(EXAMPLE).start();
    runtime.wait_for_snapshot(CLIMB, "a scan", |s| s.scan() >= 1);
    let unknown = read(&runtime, 1, &["P1.Counter", "P1.Pressure"]);
    assert_eq!(unknown["ok"], json!(false), "{unknown}");
    assert_eq!(unknown["error"], json!("unknown variable: P1.Pressure"));
    let instance = read(&runtime, 2, &["P1"]);
    assert_eq!(instance["ok"], json!(false), "{instance}");
    assert_eq!(instance["error"], json!("not an elementary variable: P1"));
}

#[test]
fn a_stopped_resource_answers_the_last_scan_as_stale_and_numbered_as_io_read() {
    // While the resource does not run, the values are those of the last completed scan, the
    // quality `stale` and the state the resource's; the scan is numbered as `io.read` numbers its
    // snapshot, whose `%MW0` is Counter (VAR_CONFIG). The production example stops with `shutdown`
    // (a pause needs the debug surface). Catches a stopped read reported `good`, one that waits for
    // a scan that does not come, and a scan numbered otherwise than `io.read` numbers it.
    let runtime = ExampleRuntime::new(EXAMPLE).start();
    runtime.wait_for_snapshot(CLIMB, "five scans", |s| s.scan() >= 5);
    runtime.ctl_ok(&["shutdown"]);
    runtime.wait_until(WAIT, "the stopped state", || {
        runtime.status().state == "stopped"
    });
    let first = read(&runtime, 1, &["P1.Counter", "P1.CounterLatched"]);
    hold_for(std::time::Duration::from_millis(200));
    let second = read(&runtime, 2, &["P1.Counter", "P1.CounterLatched"]);
    for reply in [&first, &second] {
        assert_eq!(reply["result"]["state"], json!("stopped"), "{reply}");
        assert_eq!(reply["result"]["quality"], json!("stale"), "{reply}");
    }
    assert_eq!(
        counters(&first),
        counters(&second),
        "no scan runs while stopped"
    );
    let (scan, _, counter, _) = counters(&first);
    let snapshot = runtime
        .snapshot()
        .expect("io.read's snapshot of the last scan");
    assert_eq!(
        snapshot.scan() as i64,
        scan,
        "numbered as io.read numbers it"
    );
    assert_eq!(snapshot.int("%MW0"), counter, "the same scan's Counter");
}

#[test]
fn ctl_read_prints_each_value_with_its_path_and_type() {
    // `trust-runtime ctl read <path>...` sends `variables.read` and prints the scan line, then one
    // line per path. Catches a ctl that sends another request or drops the type.
    let runtime = ExampleRuntime::new(EXAMPLE).start();
    runtime.wait_for_snapshot(CLIMB, "a scan", |s| s.scan() >= 1);
    let output = runtime.ctl_ok(&["read", "P1.Counter", "P1.CounterLatched"]);
    let lines: Vec<&str> = output.lines().collect();
    assert!(
        lines[0].starts_with("scan=") && lines[0].ends_with("state=running quality=good"),
        "{output}"
    );
    assert!(lines[1].starts_with("P1.Counter : INT = "), "{output}");
    assert!(
        lines[2].starts_with("P1.CounterLatched : INT = "),
        "{output}"
    );
}
