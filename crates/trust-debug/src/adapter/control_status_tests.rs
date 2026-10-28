//! The tasks and the cycle interval that the debug simulator's control status reports (specification 35, A2.3; annex E1.3).
//! `status` describes what the simulator runs: the tasks of the loaded program, each with its own interval, and the resource
//! cycle it runs with, the project's as the deployed runtime takes it: `[resource] cycle_interval_ms` of `runtime.toml`, or
//! 10 ms without one (specification 13). The control server is exercised over its TCP endpoint, as the other control tests
//! are; the tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::fs;
use std::net::SocketAddr;
use std::time::{Duration as WaitTime, Instant};

use serde_json::{json, Value};
use trust_runtime::value::Duration;

use super::simulation_speed_tests::{
    assert_cycles_step, control_status, disconnect, launched, launched_state, next_cycle_ends,
    project_with, reload,
};
use super::tests_part_02::reserve_loopback_addr;
use crate::DebugAdapter;

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

/// The control server's `status` once it names the launched program. The launch answers when the
/// simulator starts, and until the simulator has loaded the program `status` lists no tasks and no
/// program revision; on a busy computer the first request can come before that (CI, 28 October 2026).
fn program_status(addr: SocketAddr, auth: &str) -> Value {
    let deadline = Instant::now() + WaitTime::from_secs(10);
    loop {
        let status = control_status(addr, auth);
        if status["program_revision"].is_string() || Instant::now() >= deadline {
            return status;
        }
        std::thread::sleep(WaitTime::from_millis(20));
    }
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

/// Asserts that the cycle in `status` is the step the simulator takes: each cycle it completes advances process time by
/// exactly the reported cycle (specification 46, §6.1 item 1).
fn assert_the_runner_takes_the_reported_cycle(adapter: &DebugAdapter, status: &Value) {
    let reported = status["cycle_interval_ms"]
        .as_i64()
        .expect("status reports a cycle interval");
    let cycles = next_cycle_ends(&launched_state(adapter), 4);
    assert_cycles_step(&cycles, Duration::from_millis(reported));
}

#[test]
fn status_lists_the_task_and_reports_the_cycle_the_runner_takes() {
    let root = project_with("status-one-task", ONE_TASK, None, None);
    let addr = reserve_loopback_addr();
    let auth = "status-one-task-token";
    let mut adapter = launched(&root, addr, auth);

    let status = program_status(addr, auth);
    assert_eq!(
        listed_tasks(&status),
        vec![("maintask".to_string(), 100)],
        "{status}"
    );
    // Without a runtime.toml the simulator runs the 10 ms resource cycle (specification 13); the task keeps its 100 ms.
    assert_eq!(status["cycle_interval_ms"], json!(10), "{status}");
    assert_the_runner_takes_the_reported_cycle(&adapter, &status);
    disconnect(&mut adapter);
    fs::remove_dir_all(root).ok();
}

#[test]
fn status_lists_every_task_and_reports_the_cycle_of_the_project_runtime_toml() {
    let root = project_with("status-two-tasks", TWO_TASKS, None, None);
    fs::write(
        root.join("runtime.toml"),
        trust_runtime::bundle_template::render_runtime_toml(&smol_str::SmolStr::new("Res"), 20),
    )
    .unwrap();
    let addr = reserve_loopback_addr();
    let auth = "status-two-tasks-token";
    let mut adapter = launched(&root, addr, auth);

    let status = program_status(addr, auth);
    assert_eq!(
        listed_tasks(&status),
        vec![("fasttask".to_string(), 40), ("slowtask".to_string(), 250)],
        "{status}"
    );
    // The project's runtime.toml gives the resource cycle, 20 ms, not the shortest task interval (specification 13).
    assert_eq!(status["cycle_interval_ms"], json!(20), "{status}");
    assert_the_runner_takes_the_reported_cycle(&adapter, &status);
    disconnect(&mut adapter);
    fs::remove_dir_all(root).ok();
}

#[test]
fn a_reload_updates_the_tasks_and_keeps_the_cycle_that_status_reports() {
    let root = project_with("status-reload", ONE_TASK, None, None);
    let main = root.join("main.st");
    let addr = reserve_loopback_addr();
    let auth = "status-reload-token";
    let mut adapter = launched(&root, addr, auth);
    let before = program_status(addr, auth);
    assert_eq!(
        listed_tasks(&before),
        vec![("maintask".to_string(), 100)],
        "{before}"
    );
    assert_eq!(before["cycle_interval_ms"], json!(10), "{before}");

    // The program now has other tasks; the reload changes the program online and keeps the project's cycle.
    fs::write(&main, TWO_TASKS).unwrap();
    reload(&mut adapter, &main);

    let after = control_status(addr, auth);
    assert_eq!(
        listed_tasks(&after),
        vec![("fasttask".to_string(), 40), ("slowtask".to_string(), 250)],
        "a reload replaces the tasks that status lists: {after}"
    );
    assert_eq!(after["cycle_interval_ms"], json!(10), "{after}");
    assert_the_runner_takes_the_reported_cycle(&adapter, &after);
    disconnect(&mut adapter);
    fs::remove_dir_all(root).ok();
}
