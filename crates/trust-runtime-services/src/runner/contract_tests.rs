//! The resource loop of a device without an operating system (specification 34, STATE-08): its
//! states and their transitions, and where the effects of the requests it serves are taken, with
//! the board probe program on the real engine.

use serde_json::{json, Value};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::scheduler::ResourceState;

use super::RetainSource;
use crate::tests::runner_ports::{ms, CycleCall, Fixture};

fn request(id: u64, kind: &str) -> Value {
    json!({"id": id, "type": kind})
}

fn restart(id: u64, mode: &str) -> Value {
    json!({"id": id, "type": "restart", "params": {"mode": mode}})
}

fn state_of(status: &Value) -> &str {
    status["result"]["state"]
        .as_str()
        .expect("status reports a state")
}

fn restart_call(mode: RestartMode) -> CycleCall {
    CycleCall::Restart {
        mode,
        source: RetainSource::Checkpoint,
    }
}

fn scan(at: u64) -> CycleCall {
    CycleCall::Scan {
        now: ms(at),
        deadline: None,
    }
}

#[test]
fn power_on_runs_the_program_and_scans_on_the_cycle_beat() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(runner.last_error(), None);
    for at in [0, 4, 10, 20] {
        fixture.pass_at(&mut runner, at);
    }
    assert_eq!(fixture.take_scan_times(), [0, 10, 20]);
    assert_eq!(
        fixture.wait.take_deadlines(),
        [ms(10), ms(10), ms(20), ms(30)],
        "each pass waits for the next scan, which is due one cycle interval after the last started"
    );
}

#[test]
fn requests_are_served_between_scans_without_running_a_scan() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let _ = fixture.take_calls();
    let _ = fixture.wait.take_calls();

    let status = fixture.call_at(&mut runner, 3, &request(1, "status"));
    assert_eq!(state_of(&status), "running");
    assert_eq!(status["id"], json!(1));
    assert_eq!(status["audit_id"], json!("audit-1"));
    assert_eq!(status["result"]["resource"], json!("Controller"));
    assert_eq!(status["result"]["control_mode"], json!("production"));
    assert_eq!(status["result"]["debug_hold"], Value::Null);
    let health = fixture.call(&mut runner, &request(2, "health"));
    assert_eq!(health["result"]["ok"], json!(true));
    assert_eq!(
        fixture.take_calls(),
        [] as [CycleCall; 0],
        "no scan is due before 10 ms, so serving requests runs none"
    );
    assert_eq!(fixture.wait.take_deadlines(), [ms(10), ms(10)]);
}

#[test]
fn shutdown_stops_the_program_at_the_next_boundary_and_the_device_keeps_serving() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let _ = fixture.take_calls();

    let reply = fixture.call_at(&mut runner, 5, &request(7, "shutdown"));
    assert_eq!(
        reply,
        json!({"id": 7, "ok": true, "result": {"status": "stopping"}, "audit_id": "audit-1"}),
        "the reply is trust-runtime's, and it is sent before the stop"
    );
    let status = fixture.call_at(&mut runner, 7, &request(8, "status"));
    assert_eq!(
        state_of(&status),
        "running",
        "the stop waits for the next boundary, as trust-runtime's scan thread takes it at its \
         next pass"
    );
    assert_eq!(fixture.take_calls(), [] as [CycleCall; 0]);

    fixture.pass_at(&mut runner, 10);
    assert_eq!(
        fixture.take_calls(),
        [CycleCall::SafeOutputs, CycleCall::SaveRetain],
        "at the boundary the stop comes before the scan: the outputs go to their stop policy, \
         then the retained values are saved"
    );
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    assert_eq!(runner.last_error(), None);

    // Past the next beat the device scans nothing and still answers.
    let _ = fixture.wait.take_calls();
    let status = fixture.call_at(&mut runner, 40, &request(9, "status"));
    assert_eq!(state_of(&status), "stopped");
    assert_eq!(status["result"]["fault"], Value::Null);
    let health = fixture.call(&mut runner, &request(10, "health"));
    assert_eq!(health["result"]["ok"], json!(false));
    assert_eq!(health["result"]["state"], json!("stopped"));
    assert_eq!(
        fixture.take_calls(),
        [] as [CycleCall; 0],
        "a stopped program runs no scan"
    );
    assert_eq!(
        fixture.wait.take_deadlines(),
        [ms(1_040), ms(1_040)],
        "a stopped program has no boundary: the device waits for the link, and wakes only to \
         feed the hardware watchdog"
    );

    // A second shutdown answers as trust-runtime does and stops nothing more.
    let again = fixture.call(&mut runner, &request(11, "shutdown"));
    assert_eq!(again["result"], json!({"status": "stopping"}));
    fixture.pass_at(&mut runner, 50);
    assert_eq!(fixture.take_calls(), [] as [CycleCall; 0]);
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
}

#[test]
fn restart_from_stopped_starts_the_program_at_once_as_a_new_process_would() {
    for (mode, restart_mode) in [("cold", RestartMode::Cold), ("warm", RestartMode::Warm)] {
        let fixture = Fixture::board_probe();
        let mut runner = fixture.runner();
        fixture.pass_at(&mut runner, 0);
        fixture.pass_at(&mut runner, 10);
        let _ = fixture.call_at(&mut runner, 15, &request(1, "shutdown"));
        fixture.pass_at(&mut runner, 20);
        assert_eq!(runner.resource_state(), ResourceState::Stopped, "{mode}");
        let _ = fixture.take_calls();

        let reply = fixture.call_at(&mut runner, 27, &restart(2, mode));
        assert_eq!(
            reply["result"],
            json!({"status": "restart queued"}),
            "{mode}"
        );
        assert_eq!(
            fixture.take_calls(),
            [
                restart_call(restart_mode),
                CycleCall::TaskTiming(ms(0)),
                scan(0)
            ],
            "{mode}: the program starts from its retain checkpoint, its logical time starts \
             again at zero, and it scans at once"
        );
        assert_eq!(runner.resource_state(), ResourceState::Running, "{mode}");
        let status = fixture.call(&mut runner, &request(3, "status"));
        assert_eq!(state_of(&status), "running", "{mode}");

        let events = fixture.call(
            &mut runner,
            &json!({"id": 4, "type": "events.tail", "params": {"limit": 10}}),
        );
        assert_eq!(
            events["result"]["events"],
            json!([
                {"type": "cycle_end", "cycle": 0, "time_ns": 0},
                {"type": "cycle_start", "cycle": 0, "time_ns": 0},
            ]),
            "{mode}: the events start empty, as a new process's; the first scan has no \
             activation due"
        );
        fixture.pass_at(&mut runner, 37);
        assert_eq!(fixture.take_scan_times(), [10], "{mode}");
    }
}

#[test]
fn restart_from_stopped_starts_with_the_empty_records_of_a_new_process() {
    let fixture = Fixture::board_probe();
    {
        let mut script = fixture.script.borrow_mut();
        script.fault_policy = trust_runtime_core::watchdog::FaultPolicy::Restart;
        script.failing_scans.push_back(RuntimeError::DivisionByZero);
    }
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    fixture.pass_at(&mut runner, 10);
    let before = fixture.call_at(&mut runner, 15, &request(1, "status"));
    assert_eq!(before["result"]["metrics"]["faults"], json!(1));
    assert_eq!(before["result"]["uptime_ms"], json!(15));
    let tasks = fixture.call(&mut runner, &request(2, "tasks.stats"));
    assert_eq!(tasks["result"]["tasks"][0]["name"], json!("Scan"));
    let _ = fixture.call(&mut runner, &request(3, "shutdown"));
    fixture.pass_at(&mut runner, 20);
    assert_eq!(runner.resource_state(), ResourceState::Stopped);

    let _ = fixture.call_at(&mut runner, 25, &restart(4, "cold"));
    let after = fixture.call(&mut runner, &request(5, "status"));
    let metrics = &after["result"]["metrics"];
    assert_eq!(metrics["faults"], json!(0), "the fault count starts again");
    assert_eq!(metrics["overruns"], json!(0));
    assert_eq!(
        metrics["cycle_ms"]["window_samples"],
        json!(1),
        "only the new program's first scan is measured"
    );
    assert_eq!(
        after["result"]["uptime_ms"],
        json!(0),
        "the uptime counts from the new start"
    );
    let tasks = fixture.call(&mut runner, &request(6, "tasks.stats"));
    assert_eq!(
        tasks["result"],
        json!({"tasks": [], "profiling_enabled": true, "top_contributors": []}),
        "no task and no program has run since the start"
    );
    let faults = fixture.call(&mut runner, &request(7, "faults"));
    assert_eq!(
        faults["result"]["faults"],
        json!([]),
        "the earlier fault is gone with the old program's events"
    );
}

#[test]
fn a_paused_program_stops_like_a_running_one_and_a_failed_start_faults() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let _ = fixture.call_at(&mut runner, 2, &request(1, "pause"));
    fixture.pass_at(&mut runner, 10);
    assert_eq!(runner.resource_state(), ResourceState::Paused);
    let _ = fixture.take_calls();
    let _ = fixture.call_at(&mut runner, 12, &request(2, "shutdown"));
    fixture.pass_at(&mut runner, 20);
    assert_eq!(
        fixture.take_calls(),
        [CycleCall::SafeOutputs, CycleCall::SaveRetain],
        "the paused program's boundary at 20 ms takes the stop"
    );
    assert_eq!(runner.resource_state(), ResourceState::Stopped);

    let unreadable = RuntimeError::RetainStore(smol_str::SmolStr::new("checkpoint unreadable"));
    fixture
        .script
        .borrow_mut()
        .failing_restarts
        .push_back(unreadable.clone());
    let reply = fixture.call(&mut runner, &restart(3, "warm"));
    assert_eq!(reply["result"], json!({"status": "restart queued"}));
    assert_eq!(
        runner.resource_state(),
        ResourceState::Faulted,
        "a program that cannot start is faulted, with the reason"
    );
    assert_eq!(runner.last_error(), Some(unreadable));
    let status = fixture.call(&mut runner, &request(4, "status"));
    assert_eq!(
        status["result"]["fault"],
        json!("retain store error 'checkpoint unreadable'")
    );

    let _ = fixture.call(&mut runner, &restart(5, "cold"));
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(runner.last_error(), None);
}

#[test]
fn restart_while_running_restarts_in_place_at_the_next_boundary_and_keeps_a_pause() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    fixture.pass_at(&mut runner, 10);
    let _ = fixture.take_calls();

    let reply = fixture.call_at(&mut runner, 13, &restart(1, "warm"));
    assert_eq!(reply["result"], json!({"status": "restart queued"}));
    assert_eq!(
        fixture.take_calls(),
        [] as [CycleCall; 0],
        "the restart waits for the next boundary"
    );
    fixture.pass_at(&mut runner, 20);
    assert_eq!(
        fixture.take_calls(),
        [
            CycleCall::Restart {
                mode: RestartMode::Warm,
                source: RetainSource::Checkpoint,
            },
            CycleCall::TaskTiming(ms(20)),
            scan(20),
        ],
        "taken at the boundary, as trust-runtime's scan thread takes it before its scan; the \
         task timing starts again there"
    );
    assert_eq!(runner.resource_state(), ResourceState::Running);

    let _ = fixture.call_at(&mut runner, 23, &request(2, "pause"));
    let _ = fixture.call(&mut runner, &restart(3, "cold"));
    assert_eq!(fixture.take_calls(), [] as [CycleCall; 0]);
    fixture.pass_at(&mut runner, 30);
    assert_eq!(
        fixture.take_calls(),
        [
            restart_call(RestartMode::Cold),
            CycleCall::TaskTiming(ms(30))
        ],
        "the pause is taken first, then the restart, and the paused program runs no scan"
    );
    assert_eq!(runner.resource_state(), ResourceState::Paused);
    let status = fixture.call(&mut runner, &request(4, "status"));
    assert_eq!(
        state_of(&status),
        "paused",
        "a restart keeps the pause, as on the host"
    );
    fixture.pass_at(&mut runner, 40);
    assert_eq!(fixture.take_calls(), [] as [CycleCall; 0]);
}

#[test]
fn restarts_requested_together_coalesce_and_cold_keeps_precedence() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let _ = fixture.take_calls();

    fixture.clock.set(ms(4));
    fixture.link.send(&restart(1, "cold"));
    fixture.link.send(&restart(2, "warm"));
    runner.run_pass().expect("the test wait does not fail");
    assert_eq!(fixture.link.take_replies().len(), 2);
    fixture.pass_at(&mut runner, 10);
    assert_eq!(
        fixture.take_calls(),
        [
            restart_call(RestartMode::Cold),
            CycleCall::TaskTiming(ms(10)),
            scan(10)
        ]
    );
}

#[test]
fn a_pause_holds_the_scans_from_the_next_boundary_and_the_resume_reports_the_missed_activations() {
    let fixture = Fixture::board_probe();
    fixture.script.borrow_mut().watchdog = trust_runtime_core::watchdog::WatchdogPolicy {
        enabled: true,
        timeout: trust_runtime_core::value::Duration::from_millis(5),
        action: trust_runtime_core::watchdog::WatchdogAction::SafeHalt,
    };
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    fixture.pass_at(&mut runner, 10);
    let _ = fixture.take_calls();
    let _ = fixture.wait.take_calls();

    let paused = fixture.call_at(&mut runner, 12, &request(1, "pause"));
    assert_eq!(paused["result"], json!({"status": "paused"}));
    assert_eq!(
        runner.resource_state(),
        ResourceState::Running,
        "the pause waits for the next boundary"
    );
    for at in [20, 30, 40] {
        fixture.pass_at(&mut runner, at);
    }
    assert_eq!(runner.resource_state(), ResourceState::Paused);
    assert_eq!(
        fixture.take_calls(),
        [] as [CycleCall; 0],
        "a paused program runs no scan"
    );
    assert_eq!(
        fixture.wait.take_deadlines(),
        [ms(20), ms(30), ms(40), ms(50)],
        "while paused the boundary comes one cycle interval after the last"
    );

    let resumed = fixture.call_at(&mut runner, 45, &request(2, "resume"));
    assert_eq!(resumed["result"], json!({"status": "running"}));
    assert_eq!(runner.resource_state(), ResourceState::Paused);
    fixture.pass_at(&mut runner, 50);
    assert_eq!(
        fixture.take_calls(),
        [CycleCall::Scan {
            now: ms(50),
            deadline: Some(ms(55)),
        }],
        "the resume is taken at the boundary, which scans at once; the scan's watchdog counts \
         from its own start, never from the pause"
    );
    let events = fixture.call(
        &mut runner,
        &json!({"id": 3, "type": "events.tail", "params": {"limit": 4}}),
    );
    assert_eq!(
        events["result"]["events"],
        json!([
            {"type": "cycle_end", "cycle": 2, "time_ns": 50_000_000},
            {"type": "task_end", "name": "Scan", "priority": 0, "time_ns": 50_000_000},
            {"type": "task_start", "name": "Scan", "priority": 0, "time_ns": 50_000_000},
            {"type": "task_overrun", "name": "Scan", "missed": 3, "time_ns": 50_000_000},
        ]),
        "the task reports the activations it missed while paused, as on the host"
    );
    let status = fixture.call(&mut runner, &request(4, "status"));
    assert_eq!(status["result"]["metrics"]["overruns"], json!(3));
    assert_eq!(
        status["result"]["metrics"]["cycle_ms"]["max"],
        json!(0.0),
        "the paused time is in no scan's duration"
    );
    fixture.pass_at(&mut runner, 55);
    fixture.pass_at(&mut runner, 60);
    assert_eq!(fixture.take_scan_times(), [60]);
}

#[test]
fn stopped_and_faulted_resources_refuse_pause_and_resume_as_trust_runtime_does() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let _ = fixture.call(&mut runner, &request(1, "shutdown"));
    fixture.pass_at(&mut runner, 10);
    for kind in ["pause", "resume"] {
        let reply = fixture.call(&mut runner, &request(2, kind));
        assert_eq!(reply["error"], json!("resource is stopped"), "{kind}");
        assert_eq!(reply["error_code"], json!("resource_not_running"), "{kind}");
    }
    assert_eq!(runner.resource_state(), ResourceState::Stopped);

    let fixture = Fixture::board_probe();
    fixture
        .script
        .borrow_mut()
        .failing_scans
        .push_back(RuntimeError::DivisionByZero);
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    assert_eq!(runner.resource_state(), ResourceState::Faulted);
    for kind in ["pause", "resume"] {
        let reply = fixture.call(&mut runner, &request(3, kind));
        assert_eq!(
            reply["error"],
            json!("resource is faulted; restart required"),
            "{kind}"
        );
        assert_eq!(reply["error_code"], json!("resource_not_running"), "{kind}");
    }
    assert_eq!(runner.resource_state(), ResourceState::Faulted);
}

#[test]
fn shutdown_while_faulted_keeps_the_fault_and_a_restart_starts_the_program_anew() {
    let fixture = Fixture::board_probe();
    fixture
        .script
        .borrow_mut()
        .failing_scans
        .push_back(RuntimeError::DivisionByZero);
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let _ = fixture.take_calls();

    let reply = fixture.call_at(&mut runner, 3, &request(1, "shutdown"));
    assert_eq!(reply["result"], json!({"status": "stopping"}));
    fixture.pass_at(&mut runner, 10);
    assert_eq!(
        runner.resource_state(),
        ResourceState::Faulted,
        "trust-runtime's faulted wait ends on a stop at its next poll and keeps the fault"
    );
    assert_eq!(
        fixture.take_calls(),
        [] as [CycleCall; 0],
        "no second safe-state write and no retain save"
    );

    let _ = fixture.call_at(&mut runner, 12, &restart(2, "cold"));
    assert_eq!(
        fixture.take_calls(),
        [
            restart_call(RestartMode::Cold),
            CycleCall::TaskTiming(ms(0)),
            scan(0)
        ],
        "the program has ended, so the restart starts it as a new process would"
    );
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(runner.last_error(), None);
}

#[test]
fn a_stop_drops_the_requests_before_it_and_a_restart_after_it_starts_the_program() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let _ = fixture.take_calls();
    fixture.clock.set(ms(4));
    fixture.link.send(&restart(1, "warm"));
    fixture.link.send(&request(2, "pause"));
    fixture.link.send(&request(3, "shutdown"));
    runner.run_pass().expect("the test wait does not fail");
    assert_eq!(fixture.link.take_replies().len(), 3);
    fixture.pass_at(&mut runner, 10);
    assert_eq!(
        fixture.take_calls(),
        [CycleCall::SafeOutputs, CycleCall::SaveRetain],
        "the stop comes first at the boundary, and the restart and pause queued before it are \
         dropped, as when trust-runtime's scan thread ends"
    );
    assert_eq!(runner.resource_state(), ResourceState::Stopped);

    fixture.clock.set(ms(12));
    fixture.link.send(&request(4, "shutdown"));
    fixture.link.send(&restart(5, "warm"));
    runner.run_pass().expect("the test wait does not fail");
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(
        fixture.take_calls(),
        [
            restart_call(RestartMode::Warm),
            CycleCall::TaskTiming(ms(0)),
            scan(0)
        ]
    );
}
