//! The tasks and the cycle interval that the debug simulator's control status reports (specification 35, A2.3; annex E1.3).
//! `status` describes what the simulator runs: the tasks of the loaded program, and the cycle its managed runner takes
//! (specification 13). The control server is exercised over its TCP endpoint, as the other control tests are.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration as StdDuration, Instant};

use serde_json::{json, Value};

use super::simulation_speed_tests::{control_status, source_options, start_control_server};
use super::tests_part_02::{reserve_loopback_addr, unique_project_root};
use crate::protocol::{MessageType, Request, Response};
use crate::{DebugAdapter, DebugSession};

/// One task of 100 ms.
const ONE_TASK: &str = r#"PROGRAM Main
VAR
    Count : DINT := 0;
END_VAR
    Count := Count + 1;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Main WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
"#;

/// Two tasks, of 40 ms and of 250 ms.
const TWO_TASKS: &str = r#"PROGRAM FastProgram
VAR
    Ticks : DINT := 0;
END_VAR
    Ticks := Ticks + 1;
END_PROGRAM

PROGRAM SlowProgram
VAR
    Ticks : DINT := 0;
END_VAR
    Ticks := Ticks + 1;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK FastTask(INTERVAL := T#40ms, PRIORITY := 1);
    TASK SlowTask(INTERVAL := T#250ms, PRIORITY := 2);
    PROGRAM FastInstance WITH FastTask : FastProgram;
    PROGRAM SlowInstance WITH SlowTask : SlowProgram;
END_RESOURCE
END_CONFIGURATION
"#;

/// Writes `program` as the project's only source file and loads it.
fn loaded_program(label: &str, program: &str) -> (DebugSession, PathBuf) {
    let root = unique_project_root(label);
    let main = root.join("main.st");
    fs::write(&main, program).unwrap();
    let mut session = DebugSession::new(
        trust_runtime::RuntimeBuilder::new()
            .build()
            .expect("admit an empty runtime"),
    );
    session.update_source_options(source_options(&root));
    session
        .reload_program(Some(main.to_string_lossy().as_ref()))
        .unwrap();
    (session, root)
}

/// The `(name, interval_ms)` pairs of the task list in `status`, ordered by name. The names are in lower case, because a task
/// name matches without regard to case (annex E4.2).
fn listed_tasks(status: &Value) -> Vec<(String, i64)> {
    let mut tasks: Vec<(String, i64)> = status["tasks"]
        .as_array()
        .expect("status lists its tasks")
        .iter()
        .map(|task| {
            (
                task["name"]
                    .as_str()
                    .expect("a task has a name")
                    .to_ascii_lowercase(),
                task["interval_ms"]
                    .as_i64()
                    .expect("a task has an interval"),
            )
        })
        .collect();
    tasks.sort();
    tasks
}

/// The `stReload` request for the program at `program`.
fn reload_request(program: &Path) -> Request<Value> {
    Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "stReload".to_string(),
        arguments: Some(json!({ "program": program.to_string_lossy() })),
    }
}

/// Runs the managed runner until it has completed a cycle, stops it, and returns the cycles it completed and the process time
/// it reached, in ms.
fn run_the_runner_for_a_cycle(adapter: &mut DebugAdapter) -> (i64, i64) {
    let runtime_handle = adapter.session().runtime_handle();
    adapter.start_runner();
    let deadline = Instant::now() + StdDuration::from_secs(2);
    while runtime_handle.lock().unwrap().cycle_counter() == 0 {
        assert!(
            Instant::now() < deadline,
            "the runner did not complete a cycle"
        );
        thread::sleep(StdDuration::from_millis(1));
    }
    adapter.stop_runner();
    let runtime = runtime_handle.lock().unwrap();
    (
        i64::try_from(runtime.cycle_counter()).expect("a cycle count fits an i64"),
        runtime.current_time().as_millis(),
    )
}

/// Asserts that the cycle in `status` is the step the managed runner takes: each cycle it completes advances process time by
/// exactly the reported cycle (specification 46, §6.1 item 1).
fn assert_the_runner_takes_the_reported_cycle(adapter: &mut DebugAdapter, status: &Value) {
    let reported = status["cycle_interval_ms"]
        .as_i64()
        .expect("status reports a cycle interval");
    let (cycles, process_ms) = run_the_runner_for_a_cycle(adapter);
    assert_eq!(
        process_ms,
        cycles * reported,
        "{cycles} completed cycles advance process time by the reported {reported} ms each: {status}"
    );
}

#[test]
fn status_lists_the_task_and_reports_the_cycle_the_runner_takes() {
    let (session, root) = loaded_program("status-one-task", ONE_TASK);
    let mut adapter = DebugAdapter::new(session);
    let addr = reserve_loopback_addr();
    let auth = "status-one-task-token";
    start_control_server(&mut adapter, &root, addr, auth);

    let status = control_status(addr, auth);
    assert_eq!(
        listed_tasks(&status),
        vec![("maintask".to_string(), 100)],
        "{status}"
    );
    // The runner takes the interval of its only task as its cycle (specification 13), not a constant.
    assert_eq!(status["cycle_interval_ms"], json!(100), "{status}");
    assert_the_runner_takes_the_reported_cycle(&mut adapter, &status);
    fs::remove_dir_all(root).ok();
}

#[test]
fn status_lists_every_task_and_reports_the_shortest_interval_as_the_cycle() {
    let (session, root) = loaded_program("status-two-tasks", TWO_TASKS);
    let mut adapter = DebugAdapter::new(session);
    let addr = reserve_loopback_addr();
    let auth = "status-two-tasks-token";
    start_control_server(&mut adapter, &root, addr, auth);

    let status = control_status(addr, auth);
    assert_eq!(
        listed_tasks(&status),
        vec![("fasttask".to_string(), 40), ("slowtask".to_string(), 250)],
        "{status}"
    );
    assert_eq!(status["cycle_interval_ms"], json!(40), "{status}");
    assert_the_runner_takes_the_reported_cycle(&mut adapter, &status);
    fs::remove_dir_all(root).ok();
}

#[test]
fn a_reload_updates_the_tasks_and_the_cycle_that_status_reports() {
    let (session, root) = loaded_program("status-reload", ONE_TASK);
    let main = root.join("main.st");
    let mut adapter = DebugAdapter::new(session);
    let addr = reserve_loopback_addr();
    let auth = "status-reload-token";
    start_control_server(&mut adapter, &root, addr, auth);
    let before = control_status(addr, auth);
    assert_eq!(
        listed_tasks(&before),
        vec![("maintask".to_string(), 100)],
        "{before}"
    );
    assert_eq!(before["cycle_interval_ms"], json!(100), "{before}");

    // The program now has other tasks, and the runner starts again with the cycle they give.
    fs::write(&main, TWO_TASKS).unwrap();
    let outcome = adapter.dispatch_request(reload_request(&main));
    assert_eq!(outcome.responses.len(), 1);
    let response: Response<Value> = serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(response.success, "stReload failed: {:?}", response.message);

    let after = control_status(addr, auth);
    assert_eq!(
        listed_tasks(&after),
        vec![("fasttask".to_string(), 40), ("slowtask".to_string(), 250)],
        "a reload replaces the tasks that status lists: {after}"
    );
    assert_eq!(after["cycle_interval_ms"], json!(40), "{after}");
    assert_the_runner_takes_the_reported_cycle(&mut adapter, &after);
    fs::remove_dir_all(root).ok();
}
