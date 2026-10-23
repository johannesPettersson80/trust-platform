//! The resource loop of a device without an operating system (specification 34, STATE-08): its
//! states and their transitions, and where the effects of the requests it serves are taken, with
//! the board probe program on the real engine.

use serde_json::{json, Value};
use trust_runtime_core::deployment::ControlMode;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::scheduler::ResourceState;

use super::RetainSource;
use crate::tests::runner_ports::{ms, CycleCall, Fixture, TestPlatform};

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
fn the_runner_keeps_the_capacity_it_is_given() {
    // Specification 11, the event store: the store keeps the newest `runtime.events.capacity`
    // events and `status` reports that capacity as `event_capacity` (specification 34 §5.1.1,
    // section 7 and Appendix H, where a device without an operating system takes it from its
    // chip). Catches a runner that ignores the capacity it is given (keeping the computer's
    // 1,000), that reports another number than it keeps, or that drops the newest event instead
    // of the oldest.
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner_with_capacity(3);
    // The first boundary is due at once; running it at 20 ms misses the task's 10 ms activation,
    // and each scan 20 ms after the last misses one more, so the four scans record four overruns
    // of one missed activation each (the store keeps no event that every scan makes).
    for at in [20, 40, 60, 80] {
        fixture.pass_at(&mut runner, at);
    }
    let status = fixture.call(&mut runner, &request(1, "status"));
    assert_eq!(status["result"]["event_capacity"], json!(3));
    let events = fixture.call(
        &mut runner,
        &json!({"id": 2, "type": "events.tail", "params": {"limit": 10}}),
    );
    assert_eq!(
        events["result"]["events"],
        json!([
            {"type": "task_overrun", "name": "Scan", "missed": 1, "time_ns": 80_000_000},
            {"type": "task_overrun", "name": "Scan", "missed": 1, "time_ns": 60_000_000},
            {"type": "task_overrun", "name": "Scan", "missed": 1, "time_ns": 40_000_000},
        ]),
        "the store keeps the newest three of the four overruns, so the first, at 20 ms, has been \
         dropped"
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
fn the_explicit_exit_stops_a_device_as_shutdown_does_and_it_stays_reachable() {
    // Specification 34 STATE-08: a chip's firmware never ends, so it answers `shutdown` with
    // `{"exit": true}` as plain `shutdown`: the same reply, the stop at the next boundary, and the
    // device reachable in Stopped. Wrong behaviour: the device replies `stopping` but ignores the
    // exit and keeps scanning, or it refuses the request.
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let _ = fixture.take_calls();

    let reply = fixture.call_at(
        &mut runner,
        5,
        &json!({"id": 7, "type": "shutdown", "params": {"exit": true}}),
    );
    assert_eq!(
        reply,
        json!({"id": 7, "ok": true, "result": {"status": "stopping"}, "audit_id": "audit-1"}),
    );
    fixture.pass_at(&mut runner, 10);
    assert_eq!(
        fixture.take_calls(),
        [CycleCall::SafeOutputs, CycleCall::SaveRetain],
        "the boundary takes the stop: the outputs go to their stop policy, then the retained \
         values are saved"
    );
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    let status = fixture.call_at(&mut runner, 40, &request(8, "status"));
    assert_eq!(
        state_of(&status),
        "stopped",
        "the device still answers after the exit"
    );
}

#[test]
fn restart_from_stopped_starts_the_program_at_once_as_a_new_process_would() {
    for (mode, restart_mode) in [("cold", RestartMode::Cold), ("warm", RestartMode::Warm)] {
        let fixture = Fixture::board_probe();
        let mut runner = fixture.runner();
        fixture.pass_at(&mut runner, 0);
        // A write that the scan at 10 ms refuses leaves an event in the program's records.
        let queued = fixture.call_at(
            &mut runner,
            5,
            &json!({
                "id": 9,
                "type": "set",
                "params": {"target": "global:probe_depth", "value": "TRUE"},
            }),
        );
        assert_eq!(queued["result"], json!({"status": "queued"}), "{mode}");
        fixture.pass_at(&mut runner, 10);
        let recorded = fixture.call(&mut runner, &request(8, "events.tail"));
        assert_eq!(
            recorded["result"]["events"],
            json!([{
                "type": "write_rejected",
                "target": "probe_depth",
                "error": "type mismatch",
                "time_ns": 10_000_000,
            }]),
            "{mode}"
        );
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
            json!([]),
            "{mode}: the events start empty, as a new process's: the write refused before the \
             restart is gone, and the first scan, with no activation due, records none"
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
fn shutdown_of_a_faulted_program_stops_it_keeps_the_fault_and_a_restart_starts_it_anew() {
    // Specification 34 STATE-08 (the owner, 23 October 2026): a `shutdown` of a faulted resource
    // stops it as it stops a running one. At the faulted wait's next poll, 10 ms after the fault,
    // the outputs take the safe state again and the retained values are saved, and the resource is
    // Stopped; the root fault stays in `last_error` and `status` until the next start, which clears
    // it. Catches the old rule, under which a faulted resource stayed Faulted and so took no install,
    // and a stop that forgets the fault.
    let fixture = Fixture::board_probe();
    fixture
        .script
        .borrow_mut()
        .failing_scans
        .push_back(RuntimeError::DivisionByZero);
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    assert_eq!(runner.resource_state(), ResourceState::Faulted);
    let _ = fixture.take_calls();

    let reply = fixture.call_at(&mut runner, 3, &request(1, "shutdown"));
    assert_eq!(reply["result"], json!({"status": "stopping"}));
    fixture.pass_at(&mut runner, 10);
    assert_eq!(
        fixture.take_calls(),
        [CycleCall::SafeOutputs, CycleCall::SaveRetain],
        "the stop applies the safe state again and saves the retained values"
    );
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    assert_eq!(runner.last_error(), Some(RuntimeError::DivisionByZero));
    let status = fixture.call(&mut runner, &request(2, "status"));
    assert_eq!(status["result"]["state"], json!("stopped"));
    assert_eq!(status["result"]["fault"], json!("division by zero"));

    let _ = fixture.call_at(&mut runner, 12, &restart(3, "cold"));
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
fn shutdown_after_a_failed_start_leaves_the_resource_stopped_with_the_reason() {
    // Specification 34 STATE-08: a start that failed left the resource Faulted with no program
    // running and the outputs as the stop before it left them, so `shutdown` makes it Stopped at
    // once, with no second safe-state write and no retain save, and keeps the reason until the
    // next start. Catches a `shutdown` that the ended program ignores, which left such a chip
    // Faulted with no way to install a program, and a stop that writes outputs or retained values
    // for a program that never ran.
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let _ = fixture.call_at(&mut runner, 2, &request(1, "shutdown"));
    fixture.pass_at(&mut runner, 10);
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    let unreadable = RuntimeError::RetainStore(smol_str::SmolStr::new("checkpoint unreadable"));
    fixture
        .script
        .borrow_mut()
        .failing_restarts
        .push_back(unreadable.clone());
    let _ = fixture.call_at(&mut runner, 12, &restart(2, "warm"));
    assert_eq!(runner.resource_state(), ResourceState::Faulted);
    let _ = fixture.take_calls();

    let reply = fixture.call_at(&mut runner, 14, &request(3, "shutdown"));
    assert_eq!(reply["result"], json!({"status": "stopping"}));
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    assert_eq!(fixture.take_calls(), [] as [CycleCall; 0]);
    assert_eq!(runner.last_error(), Some(unreadable));
    fixture.pass_at(&mut runner, 30);
    assert_eq!(runner.resource_state(), ResourceState::Stopped);

    let _ = fixture.call_at(&mut runner, 32, &restart(4, "cold"));
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(runner.last_error(), None);
}

#[test]
fn a_stop_of_a_faulted_program_whose_safe_state_fails_stays_faulted_with_that_error() {
    // Specification 34 STATE-08 and 11 §6.6: the stop of a faulted resource follows the rule of
    // every stop, so an unconfirmed safe state leaves it Faulted with that error instead of the root
    // fault. Catches a faulted stop that reports Stopped although its outputs are not confirmed safe.
    let fixture = Fixture::board_probe();
    fixture
        .script
        .borrow_mut()
        .failing_scans
        .push_back(RuntimeError::DivisionByZero);
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let unconfirmed = RuntimeError::ControlError(smol_str::SmolStr::new("safe state unconfirmed"));
    fixture.script.borrow_mut().safe_outputs_error = Some(unconfirmed.clone());
    let _ = fixture.take_calls();

    let _ = fixture.call_at(&mut runner, 3, &request(1, "shutdown"));
    fixture.pass_at(&mut runner, 10);
    assert_eq!(
        fixture.take_calls(),
        [CycleCall::SafeOutputs, CycleCall::SaveRetain]
    );
    assert_eq!(runner.resource_state(), ResourceState::Faulted);
    assert_eq!(runner.last_error(), Some(unconfirmed));
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

#[test]
fn the_device_runner_reports_the_records_control_mode() {
    // Specification 11 §6.9: `runtime.control.mode` defaults to `production` and can be set to
    // `debug`, and `status` reports its lower-case name as `control_mode`. Specification 34
    // §5.1.1, section 7: a device takes the mode from its deployment record. Catches the bench's
    // device runner, which reported `production` for a record that sets `debug`.
    for (mode, label) in [
        (ControlMode::Production, "production"),
        (ControlMode::Debug, "debug"),
    ] {
        let fixture = Fixture::board_probe();
        let platform = TestPlatform {
            control_mode: mode,
            ..TestPlatform::default()
        };
        let mut runner = fixture.runner_with(platform);
        // The first scan, due at power-on, runs before a request that arrives then (STATE-08).
        fixture.pass_at(&mut runner, 0);
        let status = fixture.call(&mut runner, &request(1, "status"));
        assert_eq!(status["result"]["control_mode"], json!(label), "{mode:?}");
    }
}

#[test]
fn a_debug_mode_pause_on_the_device_runner_acts_as_trust_runtimes() {
    // Specification 11 §6.9: in production mode `pause` and `resume` act on the resource
    // scheduler, so a pause holds the scan between cycles; in debug mode a pause is a debugger
    // hold and does not pause the scheduler. The device runner routes the request by the record's
    // mode (specification 34 §5.1.1, section 7). Catches a device runner that treats every record
    // as production and pauses the scheduler in debug mode, as the C6 bench's firmware did.
    for (mode, reached) in [
        (ControlMode::Debug, ResourceState::Running),
        (ControlMode::Production, ResourceState::Paused),
    ] {
        let fixture = Fixture::board_probe();
        let platform = TestPlatform {
            control_mode: mode,
            ..TestPlatform::default()
        };
        let mut runner = fixture.runner_with(platform);
        fixture.pass_at(&mut runner, 0);
        let _ = fixture.take_calls();
        let paused = fixture.call_at(&mut runner, 2, &request(1, "pause"));
        assert_eq!(paused["result"], json!({"status": "paused"}), "{mode:?}");
        fixture.pass_at(&mut runner, 10);
        assert_eq!(runner.resource_state(), reached, "{mode:?}");
    }
}
