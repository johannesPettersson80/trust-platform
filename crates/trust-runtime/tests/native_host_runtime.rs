//! A compiled program on a computer (specification 44 §8.8, NX-14 and NX-15): trust-runtime runs it
//! with the chips' compiled cycle, keeps its retained values in the retain file the bytecode engine
//! writes, changes it online, steps it, and runs it on the resource's scan thread.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::time::{Duration as WallDuration, Instant};

use smol_str::SmolStr;
use trust_runtime::harness::SourceFile;
use trust_runtime::io::IoAddress;
use trust_runtime::native::{build, Options, Target, Toolchain};
use trust_runtime::retain::FileRetainStore;
use trust_runtime::scheduler::{ResourceCommand, ResourceRunner, StdClock, StepRequest};
use trust_runtime::value::{Duration, Value};
use trust_runtime::{CompiledRuntime, RestartMode};

/// The running program: every scan adds 1 to `count` and 2 to the retained `kept`, and the lamp at
/// `%QX0.0` is on when `count` is odd.
const RUNNING: &str = "
PROGRAM Main
VAR
    count : DINT;
    lamp AT %QX0.0 : BOOL;
END_VAR
VAR RETAIN
    kept : DINT;
END_VAR
count := count + 1;
kept := kept + 2;
lamp := (count MOD 2) = 1;
END_PROGRAM

CONFIGURATION Plant
RESOURCE Cpu ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
";

/// The changed program: `added` is new with 7, and `kept` grows by 3.
const CHANGED: &str = "
PROGRAM Main
VAR
    count : DINT;
    lamp AT %QX0.0 : BOOL;
    added : INT := 7;
END_VAR
VAR RETAIN
    kept : DINT;
END_VAR
count := count + 1;
kept := kept + 3;
lamp := (count MOD 2) = 1;
END_PROGRAM

CONFIGURATION Plant
RESOURCE Cpu ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
";

/// The image of `source` for this computer.
fn image(source: &str, name: &str) -> Vec<u8> {
    let toolchain = Toolchain::from_env()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let work = std::env::temp_dir().join(format!("trust-host-{name}-{}", std::process::id()));
    let built = build(
        &[SourceFile::new(source)],
        &Options {
            target: Target::this_cpu().expect("a computer target"),
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the program compiles");
    let _ = std::fs::remove_dir_all(&work);
    built.image
}

/// The program's value of `name`, read from the state its last scan left.
fn value(runtime: &CompiledRuntime, name: &str) -> Option<Value> {
    runtime
        .snapshot()
        .expect("the program keeps its image")
        .value(name)
}

fn millis(ms: i64) -> Duration {
    Duration::from_millis(ms)
}

/// Specification 44 §8.8 with specification 11 §6.7 and §12, worked by hand from the two sources
/// and the periodic rule of specification 10 §4.3 (a task is due once `INTERVAL` has passed since
/// its last run, the start counting as one). Scans at 0, 10, 20 and 30 ms run the 10 ms task at 10,
/// 20 and 30 ms, which leaves `count` 3, `kept` 6 and the lamp on (3 is odd); the retain file, saved
/// at every changed scan, then holds `kept` 6. A second runtime of the same image, restarted warm
/// from that file at 0 ms, starts with `kept` 6 and `count` 0. Changed online, it keeps both and
/// `added` takes 7; a scan at 10 ms, when the task's schedule makes it due, then makes `count` 1 and
/// `kept` 9. A cold restart gives 0 and 0. Catches a cycle that does not run the task, an output the scan does not set, a retain file
/// the computer does not write or does not read at a warm restart, a change that loses the running
/// values or does not run the new code, and a cold restart that keeps them.
#[test]
fn a_computer_runs_a_compiled_program_with_its_retain_file_and_changes_it_online() {
    let running = image(RUNNING, "running");
    let changed = image(CHANGED, "changed");
    let retain = std::env::temp_dir().join(format!("trust-host-retain-{}.bin", std::process::id()));
    let _ = std::fs::remove_file(&retain);

    let mut first = CompiledRuntime::load(running.clone(), "Cpu").expect("the image loads");
    first
        .set_retain_store(
            Some(Box::new(FileRetainStore::new(&retain))),
            Some(Duration::ZERO),
        )
        .expect("the retain file is set");
    first.restart(RestartMode::Cold).expect("the cold start");
    first.reset_task_timing(Duration::ZERO);
    for at in [0, 10, 20, 30] {
        first.set_current_time(millis(at));
        first.execute_cycle().expect("the scan runs");
    }
    assert_eq!(value(&first, "P.count"), Some(Value::DInt(3)));
    assert_eq!(value(&first, "P.kept"), Some(Value::DInt(6)));
    assert_eq!(
        first.read_io(&IoAddress::parse("%QX0.0").expect("an address")),
        Ok(Value::Bool(true))
    );
    drop(first);

    let mut second = CompiledRuntime::load(running, "Cpu").expect("the image loads");
    second
        .set_retain_store(
            Some(Box::new(FileRetainStore::new(&retain))),
            Some(Duration::ZERO),
        )
        .expect("the retain file is set");
    second
        .restart_with_retain_store(RestartMode::Warm)
        .expect("the warm start from the retain file");
    second.reset_task_timing(Duration::ZERO);
    assert_eq!(value(&second, "P.kept"), Some(Value::DInt(6)));
    assert_eq!(value(&second, "P.count"), Some(Value::DInt(0)));

    second
        .apply_online_change_bytes(&changed)
        .expect("the change applies");
    assert_eq!(value(&second, "P.kept"), Some(Value::DInt(6)));
    assert_eq!(value(&second, "P.added"), Some(Value::Int(7)));
    second.set_current_time(millis(10));
    second.execute_cycle().expect("the changed program runs");
    assert_eq!(value(&second, "P.count"), Some(Value::DInt(1)));
    assert_eq!(value(&second, "P.kept"), Some(Value::DInt(9)));

    second.restart(RestartMode::Cold).expect("the cold restart");
    assert_eq!(value(&second, "P.count"), Some(Value::DInt(0)));
    assert_eq!(value(&second, "P.kept"), Some(Value::DInt(0)));
    let _ = std::fs::remove_file(&retain);
}

/// Specification 11 §6.8.1 on a compiled program: a step at 10 ms with a cold restart, which
/// restarts the program at 0 ms, writes 41 into `count` and runs one scan, in which the 10 ms task is
/// due (specification 10 §4.3), so `count` becomes 42 and `kept` 2. Catches a step that does not
/// write its inputs before the scan, does not restart, or reads the watched names before the scan.
#[test]
fn a_computer_steps_a_compiled_program() {
    let mut runtime =
        CompiledRuntime::load(image(RUNNING, "step"), "Cpu").expect("the image loads");
    let request = StepRequest {
        time: millis(10),
        restart: Some(RestartMode::Cold),
        inputs: vec![(SmolStr::new("P.count"), Value::DInt(41))],
        direct_inputs: Vec::new(),
        watch: vec![SmolStr::new("P.count"), SmolStr::new("P.kept")],
        watch_direct: Vec::new(),
        run: None,
    };
    let outcome = runtime
        .step(&request, Duration::ZERO)
        .expect("the step runs");
    assert_eq!(outcome.error, None);
    assert_eq!(
        outcome.values,
        vec![
            (SmolStr::new("P.count"), Value::DInt(42)),
            (SmolStr::new("P.kept"), Value::DInt(2)),
        ]
    );
}

/// Specification 44 §8.8 (NX-14): the resource's scan thread runs a compiled program as it runs an
/// interpreted one, and a resource command reads its variables between scans. Over at least 100 ms
/// at a 10 ms cycle the program runs at least 5 scans. Catches a scan thread that does not run a
/// compiled program, or a command the compiled program does not answer.
#[test]
fn the_scan_thread_runs_a_compiled_program() {
    let mut runtime =
        CompiledRuntime::load(image(RUNNING, "thread"), "Cpu").expect("the image loads");
    runtime.restart(RestartMode::Cold).expect("the cold start");
    let runner = ResourceRunner::new(runtime, StdClock::new(), millis(10));
    let mut handle = runner.spawn("compiled").expect("the scan thread starts");
    let started = Instant::now();
    std::thread::sleep(WallDuration::from_millis(100));
    let (reply, replies) = std::sync::mpsc::channel();
    handle
        .control()
        .send_command(ResourceCommand::MeshSnapshot {
            names: vec![SmolStr::new("P.count")],
            respond_to: reply,
        })
        .expect("the command is queued");
    let values = replies
        .recv_timeout(WallDuration::from_secs(5))
        .expect("the scan thread answers");
    handle.stop();
    handle.join().expect("the scan thread ends");
    let count = match values.get("P.count") {
        Some(Value::DInt(count)) => *count,
        other => panic!("count: {other:?}"),
    };
    let elapsed = started.elapsed().as_millis();
    assert!(
        count >= 5,
        "at a 10 ms cycle, {elapsed} ms ran {count} scans, fewer than 5"
    );
}

/// The JSON of `response`.
fn json_of(response: &trust_runtime_services::protocol::ControlResponse) -> serde_json::Value {
    serde_json::to_value(response).expect("a response serializes")
}

/// The control request `kind` with `params`.
fn request(
    id: u64,
    kind: &str,
    params: Option<serde_json::Value>,
) -> trust_runtime_services::protocol::ControlRequest {
    trust_runtime_services::protocol::ControlRequest {
        id,
        r#type: kind.to_string(),
        params,
        auth: None,
        request_id: None,
    }
}

/// Specification 44 §13.2 on a computer (§8.8): a breakpoint on line 11 of `src/main.st`, `kept :=
/// kept + 2;` (zero-based line 10), holds the scan thread before that statement in the scan at 10
/// ms, when the 10 ms task is first due (specification 10 §4.3); while held, `debug.state` is paused
/// and `debug.stack`'s innermost frame stands at line 10; after `resume` the scan ends with `count` 1
/// and `kept` 2. Catches a computer whose compiled scans take no breakpoint,
/// a hold that does not wait, a stack read that does not see the held scan, and a resume that does
/// not release it.
#[test]
fn the_computer_debugger_holds_a_compiled_scan_at_a_breakpoint() {
    let toolchain = Toolchain::from_env()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let work = std::env::temp_dir().join(format!("trust-host-debug-{}", std::process::id()));
    let image = build(
        &[SourceFile::with_path("src/main.st", RUNNING)],
        &Options {
            target: Target::this_cpu().expect("a computer target"),
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the program compiles")
    .image;
    let _ = std::fs::remove_dir_all(&work);
    let mut runtime = CompiledRuntime::load(image, "Cpu").expect("the image loads");
    let debug = trust_runtime::debug::DebugControl::new();
    runtime.set_debug_control(debug.clone());
    runtime.restart(RestartMode::Cold).expect("the cold start");
    runtime.reset_task_timing(Duration::ZERO);
    let debugger = debug.compiled().expect("the compiled program's debugger");
    let io = || trust_runtime_services::live_values::io_requests::IoSnapshotRead::Missing;

    let set = json_of(&debugger.answer(
        &request(
            1,
            "breakpoints.set",
            Some(serde_json::json!({ "source": "src/main.st", "lines": [10] })),
        ),
        io(),
    ));
    assert_eq!(set["ok"], serde_json::json!(true), "{set}");

    let scan = std::thread::spawn(move || {
        runtime.set_current_time(millis(10));
        let result = runtime.execute_cycle();
        (runtime, result)
    });
    let deadline = Instant::now() + WallDuration::from_secs(10);
    loop {
        let state = json_of(&debugger.answer(&request(2, "debug.state", None), io()));
        if state["result"]["paused"] == serde_json::json!(true) {
            break;
        }
        assert!(Instant::now() < deadline, "no hold within 10 s: {state}");
        std::thread::sleep(WallDuration::from_millis(10));
    }
    let stack = json_of(&debugger.answer(&request(3, "debug.stack", None), io()));
    assert_eq!(
        stack["result"]["stack_frames"][0]["line"],
        serde_json::json!(10),
        "{stack}"
    );
    debugger.resume();
    let (runtime, result) = scan.join().expect("the scan thread ends");
    result.expect("the held scan completes");
    assert_eq!(value(&runtime, "P.count"), Some(Value::DInt(1)));
    assert_eq!(value(&runtime, "P.kept"), Some(Value::DInt(2)));
}
