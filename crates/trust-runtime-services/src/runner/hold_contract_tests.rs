//! The hold of a scan by the debugger on a device (specification 13, "Pause", "Continue", "Stepping
//! Semantics" and "Safe Points"; specification 11 §6.9; specification 34, STATE-07, STATE-08,
//! PORT-05 and TIME-04), with the board probe on the real engine behind the cycle port.
//!
//! The board probe's program `Gpio` (`portability/f401/gpio.st`) has these statements in its
//! `LINE_TABLE`, as 1-based `line:column` spans of the one file: `led := button;` 23:1-24:1, the
//! `CASE` statement 24:1-30:9 and, in its `ELSE` branch, `depth_result := INT#1;` 29:5-30:1. With
//! `probe_depth` at its initial 1, a scan that runs the program runs the first, the second and the
//! third. Every task counts its next activation from the start of the program, so the first scan,
//! due at power-on, runs no program: the program first runs at the scan 10 ms later, which the
//! tests hold.
//!
//! A scan holds inside one call of `run_pass`, so the tests use an advancing wait that lets the
//! time of its sleep pass on the clock, and schedule what reaches the loop during the hold: a
//! request arrives at a time on the clock, or the debugger acts.

use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use serde_json::{json, Value};
use smol_str::SmolStr;
use trust_runtime_core::debug::StatementSpan;
use trust_runtime_core::deployment::ControlMode;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::VariableStorage;
use trust_runtime_core::ports::{MonotonicClock, PortError};
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::{self, Value as PlcValue};
use trust_runtime_core::vm::ExecutionServices;
use trust_runtime_core::watchdog::{WatchdogAction, WatchdogPolicy};

use super::debugger::{LastStop, Position};
use super::program::Program;
use crate::debug::{Breakpoint, HitCondition, Span, StatementId, StepKind, StopReason};
use crate::status::{IoDriverHealth, IoDriverStatus};
use crate::tests::runner_ports::{ms, CycleCall, Fixture, IoStep, TestPlatform, TestRunner};

/// The time of the scan that the tests hold, in milliseconds: the program's first activation, one
/// cycle interval after the start of the program.
pub(super) const HELD_SCAN_AT: u64 = 10;

pub(super) fn request(id: u64, kind: &str) -> Value {
    json!({"id": id, "type": kind})
}

/// A breakpoint on the statement of the board probe whose source span is `start..end`.
fn breakpoint(start: Position, end: Position) -> Breakpoint<Position> {
    Breakpoint::new(StatementId {
        file_id: 0,
        span: Span::new(start, end),
        generation: 1,
    })
}

/// A breakpoint on `led := button;`, the first statement of a scan that runs the program.
pub(super) fn on_led_assignment() -> Breakpoint<Position> {
    breakpoint((23, 1), (24, 1))
}

/// A breakpoint on `depth_result := INT#1;`, the last statement such a scan runs.
fn on_else_branch() -> Breakpoint<Position> {
    breakpoint((29, 5), (30, 1))
}

/// `breakpoint`, stopping at its first hit only (specification 13: `Equal(1)` stops at the first
/// counted hit).
fn once(mut breakpoint: Breakpoint<Position>) -> Breakpoint<Position> {
    breakpoint.hit_condition = Some(HitCondition::Equal(1));
    breakpoint
}

/// The span the line table gives `led := button;`.
const LED_ASSIGNMENT: StatementSpan = StatementSpan {
    file_index: 0,
    start: (23, 1),
    end: (24, 1),
};

/// The span the line table gives the `CASE` statement.
const CASE_STATEMENT: StatementSpan = StatementSpan {
    file_index: 0,
    start: (24, 1),
    end: (30, 9),
};

/// A variable of the program instance `Plant`, read from `storage`.
fn plant_var(storage: &VariableStorage, name: &str) -> Option<PlcValue> {
    let Some(PlcValue::Instance(plant)) = storage.get_global("Plant") else {
        return None;
    };
    storage.get_instance_var(*plant, name).cloned()
}

/// The program's `depth_result` in the loop's installed program, between scans.
fn depth_result(runner: &TestRunner<'_>) -> PlcValue {
    let Program::Installed(cycle) = &runner.cycle else {
        panic!("the board probe is installed");
    };
    plant_var(cycle.engine().state().storage(), "depth_result")
        .expect("the program instance has depth_result")
}

/// The address of the storage of the loop's installed program.
fn storage_address(runner: &TestRunner<'_>) -> usize {
    let Program::Installed(cycle) = &runner.cycle else {
        panic!("the board probe is installed");
    };
    core::ptr::from_ref(cycle.engine().state().storage()).addr()
}

/// Checks that the pass that ran the first scan waited for the second, as the tests assume.
pub(super) fn assert_waited_for_the_held_scan(fixture: &Fixture) {
    assert_eq!(
        fixture.clock.now().since_origin(),
        ms(HELD_SCAN_AT),
        "the advancing wait slept until the next scan was due"
    );
}

/// What a hold sees of the halted state at one pass.
#[derive(Debug, Clone, PartialEq)]
struct Halted {
    storage_address: usize,
    probe_depth: Option<PlcValue>,
    depth_result: Option<PlcValue>,
    pou_id: Option<u32>,
    logical_nanos: i64,
}

#[test]
fn a_held_scan_answers_status_and_variables_without_a_copy() {
    // Specification 11 §6.9: while the debugger holds the scan, `status` reports the hold rather than
    // `running`, and the requests that a device serves between scans are served all the same. The
    // value of a held scan is the halted state (§6.13.1), read in place and never copied per
    // statement; specification 34 STATE-07 gives a chip the same behaviour. The scan stops before
    // `depth_result := INT#1;`, so `depth_result` is still 0 and becomes 1 once the scan continues.
    // Wrong behaviour caught: a loop that serves nothing while it holds the scan, a `status` that
    // reports `running` or has lost the I/O health the cycle published before the program ran, a
    // hold that copies the storage instead of lending it (a copy has another address), and a hold
    // that stops after the statement.
    let fixture = Fixture::board_probe().with_debugger();
    fixture.io.state.borrow_mut().health = vec![IoDriverStatus {
        name: SmolStr::new("probe-bus"),
        health: IoDriverHealth::Degraded {
            error: SmolStr::new("line 3 is slow"),
        },
    }];
    let mut runner = fixture.runner();
    runner.shell().debug.add_breakpoint(on_else_branch());
    let seen = Rc::new(RefCell::new(Vec::new()));
    {
        let seen = Rc::clone(&seen);
        runner.shell().set_probe(move |view| {
            seen.borrow_mut().push(Halted {
                storage_address: core::ptr::from_ref(view.storage).addr(),
                probe_depth: view.storage.get_global("probe_depth").cloned(),
                depth_result: plant_var(view.storage, "depth_result"),
                pou_id: view.frame.pou_id(),
                logical_nanos: view.logical_time.as_nanos(),
            });
        });
    }
    fixture.wait.advance_time();
    fixture.arrive_at(ms(1_000), &request(1, "status"));
    fixture.arrive_at(ms(2_000), &request(2, "health"));
    fixture.arrive_at(ms(3_000), &request(3, "resume"));
    runner.run_pass().expect("the test wait does not fail");
    assert_waited_for_the_held_scan(&fixture);

    runner.run_pass().expect("the test wait does not fail");

    let replies = fixture.link.take_replies();
    assert_eq!(replies.len(), 3, "one reply to each request: {replies:?}");
    let drivers = json!([{"name": "probe-bus", "status": "degraded", "error": "line 3 is slow"}]);
    assert_eq!(replies[0]["result"]["state"], json!("paused"));
    assert_eq!(
        replies[0]["result"]["debug_hold"],
        json!({"reason": "breakpoint"})
    );
    assert_eq!(replies[0]["result"]["io_drivers"], drivers);
    assert_eq!(
        replies[1]["result"],
        json!({"ok": true, "state": "running", "fault": null, "io_drivers": drivers}),
        "a degraded driver is reported and stays healthy"
    );
    assert_eq!(replies[2]["result"], json!({"status": "running"}));
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(runner.last_error(), None);

    // The hold ran one pass at the stop, one at each arrival and one at the release.
    let halted = Halted {
        storage_address: storage_address(&runner),
        probe_depth: Some(PlcValue::Int(1)),
        depth_result: Some(PlcValue::Int(0)),
        pou_id: Some(0),
        logical_nanos: 10_000_000,
    };
    assert_eq!(*seen.borrow(), vec![halted; 4]);
    assert_eq!(
        depth_result(&runner),
        PlcValue::Int(1),
        "the statement ran once the hold was released"
    );
}

#[test]
fn the_engine_takes_its_stack_path_only_while_a_debugged_scan_runs() {
    // Specification 11 §6.9: debug hooks are side-effect-free when debugging is disabled, and the
    // instruction-level hook needs the engine's canonical stack path, which
    // `ExecutionServices::debug_active` selects; the register tier calls it at block starts only.
    // The services ask for the stack path while a scan that a debugger can hold runs, and never
    // between scans, so a program keeps its register tier when no debugger is there. Wrong behaviour
    // caught: a flag that stays on after the scan, and one that is never on, so that a register-tier
    // program is never held at a statement.
    let fixture = Fixture::board_probe().with_debugger();
    let services = fixture.services();
    let mut runner = fixture.runner();
    runner.shell().debug.add_breakpoint(on_led_assignment());
    let during_hold = Rc::new(Cell::new(None));
    {
        let during_hold = Rc::clone(&during_hold);
        runner
            .shell()
            .set_probe(move |_| during_hold.set(Some(services.debug_active())));
    }
    assert!(!services.debug_active(), "before the first scan");
    fixture.wait.advance_time();
    fixture.arrive_at(ms(1_000), &request(1, "resume"));
    runner.run_pass().expect("the test wait does not fail");
    assert!(
        !services.debug_active(),
        "the first scan has ended: no scan is running"
    );

    runner.run_pass().expect("the test wait does not fail");

    assert_eq!(
        during_hold.get(),
        Some(true),
        "the hook was live while the scan was held"
    );
    assert!(!services.debug_active(), "the held scan has ended");
}

#[test]
fn a_hold_feeds_the_watchdog_once_per_feed_interval() {
    // Specification 34 TIME-04 and STATE-08: the loop feeds the hardware watchdog in every pass and
    // waits no longer than its feed interval, here 1,000 ms against a 4,000 ms watchdog;
    // specification 11 §6.9 puts the same on the hold of a chip. The first pass feeds at 0 ms. The
    // hold begins at 10 ms and passes at 10, 1,010, 2,010 and 3,010 ms; at 3,510 ms, where the
    // `resume` arrives, the pass that serves it feeds once more, and the loop's own pass feeds once
    // at the end. Wrong behaviour caught: a hold that waits for the `resume` without feeding (the
    // chip resets after 4 s), and one that never waits and feeds on every spin.
    let fixture = Fixture::board_probe().with_debugger();
    let platform = TestPlatform::default();
    let feed_times = Rc::clone(&platform.feed_times);
    let mut runner = fixture.runner_with(platform);
    runner.shell().debug.add_breakpoint(on_led_assignment());
    fixture.wait.advance_time();
    fixture.arrive_at(ms(3_510), &request(1, "resume"));
    runner.run_pass().expect("the test wait does not fail");
    assert_waited_for_the_held_scan(&fixture);

    runner.run_pass().expect("the test wait does not fail");

    assert_eq!(
        *feed_times.borrow(),
        [0, 10, 1_010, 2_010, 3_010, 3_510, 3_510].map(ms),
        "the first pass, five feeds in the hold, one every interval and one at the release, and the \
         loop's own at the end"
    );
}

#[test]
fn a_request_held_back_for_a_due_scan_is_answered_first_by_the_hold() {
    // Specification 34 STATE-08: a request that arrives while the program's next boundary is due
    // waits for that boundary, and is answered after it, before any request that arrives later. When
    // the scan that the boundary takes is held, the hold answers the waiting request first: it comes
    // from the same loop, which hands it over. Wrong behaviour caught: a hold that never sees the
    // request held back (it would answer after the `resume`, which arrived later, and `status`
    // would report `running`), and one that answers the later request first.
    let fixture = Fixture::board_probe().with_debugger();
    let mut runner = fixture.runner();
    runner.shell().debug.add_breakpoint(on_led_assignment());
    fixture.wait.advance_time();
    runner.run_pass().expect("the test wait does not fail");
    assert_waited_for_the_held_scan(&fixture);
    // The second scan is due now, so the loop takes it before it answers the request.
    fixture.link.send(&request(1, "status"));
    fixture.arrive_at(ms(1_000), &request(2, "resume"));

    runner.run_pass().expect("the test wait does not fail");

    let replies = fixture.link.take_replies();
    assert_eq!(replies.len(), 2, "both requests were answered: {replies:?}");
    assert_eq!(replies[0]["id"], json!(1));
    assert_eq!(replies[0]["result"]["state"], json!("paused"));
    assert_eq!(
        replies[0]["result"]["debug_hold"],
        json!({"reason": "breakpoint"})
    );
    assert_eq!(replies[1]["id"], json!(2));
    assert_eq!(replies[1]["result"], json!({"status": "running"}));
}

#[test]
fn resume_from_the_link_ends_the_hold_and_commits_outputs_once() {
    // Specification 11 §6.9: a hold waits before the first statement it stops at: no output is
    // committed while it lasts, and releasing it continues that same scan, which is neither
    // restarted nor repeated. Specification 34 STATE-07: the outputs keep their last committed
    // values. Wrong behaviour caught: outputs written during the hold, a release that starts the scan
    // again (a second input read and a second commit), and a release that drops the scan's commit.
    let fixture = Fixture::board_probe().with_debugger();
    let mut runner = fixture.runner();
    runner.shell().debug.add_breakpoint(on_led_assignment());
    fixture.wait.advance_time();
    let during_hold = Rc::new(RefCell::new(Vec::new()));
    {
        let io = fixture.io.clone();
        let during_hold = Rc::clone(&during_hold);
        fixture.wait.at(ms(2_000), move || {
            during_hold.borrow_mut().extend(io.take_steps());
        });
    }
    fixture.arrive_at(ms(3_000), &request(1, "resume"));
    runner.run_pass().expect("the test wait does not fail");
    assert_waited_for_the_held_scan(&fixture);
    let first_scan = fixture.io.take_steps();
    assert!(
        matches!(
            first_scan.as_slice(),
            [IoStep::ReadInputs, IoStep::WriteOutputs(_)]
        ),
        "the first scan read the inputs and committed its outputs: {first_scan:?}"
    );

    runner.run_pass().expect("the test wait does not fail");

    assert_eq!(
        *during_hold.borrow(),
        [IoStep::ReadInputs],
        "the inputs were read before the program ran, and no output was driven"
    );
    let after = fixture.io.take_steps();
    assert!(
        matches!(after.as_slice(), [IoStep::WriteOutputs(_)]),
        "the released scan committed its outputs once: {after:?}"
    );
    assert_eq!(
        fixture.take_scan_times(),
        [0, 10],
        "the held scan was not repeated"
    );
    let replies = fixture.link.take_replies();
    assert_eq!(replies[0]["result"], json!({"status": "running"}));
    assert_eq!(runner.resource_state(), ResourceState::Running);
}

#[test]
fn shutdown_during_a_hold_ends_it_and_takes_the_stop_path() {
    // Specification 11 §6.9: a stop request ends the hold with no `resume`, the suspended scan runs
    // to its end with no further pause, step or breakpoint, and the resource then takes its ordinary
    // stop path: the stop outputs, the retain save and `Stopped`. Specification 34 STATE-08: the
    // chip stays reachable. Wrong behaviour caught: a hold that ignores the stop (the test's wait
    // would sleep for ever), a scan that holds again at the later breakpoint, and a stop path that is
    // skipped or reordered.
    let fixture = Fixture::board_probe().with_debugger();
    let mut runner = fixture.runner();
    runner.shell().debug.add_breakpoint(on_led_assignment());
    runner.shell().debug.add_breakpoint(on_else_branch());
    fixture.wait.advance_time();
    fixture.arrive_at(ms(2_000), &request(1, "shutdown"));
    runner.run_pass().expect("the test wait does not fail");
    assert_waited_for_the_held_scan(&fixture);

    runner.run_pass().expect("the test wait does not fail");

    let replies = fixture.link.take_replies();
    assert_eq!(replies[0]["result"]["status"], json!("stopping"));
    assert_eq!(
        fixture.take_calls(),
        [
            CycleCall::Scan {
                now: ms(0),
                deadline: None
            },
            CycleCall::Scan {
                now: ms(10),
                deadline: None
            },
        ],
        "the held scan ran to its end, and the stop waits for the program's next boundary"
    );
    assert_eq!(runner.resource_state(), ResourceState::Running);

    runner.run_pass().expect("the test wait does not fail");

    assert_eq!(
        fixture.take_calls(),
        [CycleCall::SafeOutputs, CycleCall::SaveRetain],
        "the stop outputs, then the retain save"
    );
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    let status = fixture.call(&mut runner, &request(2, "status"));
    assert_eq!(status["result"]["state"], json!("stopped"));
    assert_eq!(status["result"]["debug_hold"], Value::Null);
}

#[test]
fn a_restarted_program_can_be_held_again() {
    // Specification 11 §6.9: a stop ends the holds and keeps the debugger from holding that resource
    // again, and "a resource thread that starts later on the same debugger can be held again";
    // specification 34 STATE-08: `restart` after `shutdown` starts the program as a new process
    // would. Wrong behaviour caught: a debugger that stays inert after the first stop.
    let fixture = Fixture::board_probe().with_debugger();
    let mut runner = fixture.runner();
    runner.shell().debug.add_breakpoint(on_led_assignment());
    fixture.wait.advance_time();
    fixture.arrive_at(ms(1_000), &request(1, "shutdown"));
    runner.run_pass().expect("the test wait does not fail");
    runner.run_pass().expect("the test wait does not fail");
    runner.run_pass().expect("the test wait does not fail");
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    let _ = fixture.link.take_replies();

    // The restart starts the program as a new process: its first scan runs no task again.
    fixture
        .link
        .send(&json!({"id": 2, "type": "restart", "params": {"mode": "cold"}}));
    runner.run_pass().expect("the test wait does not fail");
    assert_eq!(runner.resource_state(), ResourceState::Running);
    let begun = fixture.clock.now().since_origin();
    fixture.arrive_at(begun + ms(1_000), &request(3, "status"));
    fixture.arrive_at(begun + ms(2_000), &request(4, "resume"));
    runner.run_pass().expect("the test wait does not fail");

    let replies = fixture.link.take_replies();
    assert_eq!(replies[0]["result"], json!({"status": "restart queued"}));
    assert_eq!(replies[1]["result"]["state"], json!("paused"));
    assert_eq!(
        replies[1]["result"]["debug_hold"],
        json!({"reason": "breakpoint"}),
        "the restarted program was held at its breakpoint"
    );
    assert_eq!(replies[2]["result"], json!({"status": "running"}));
    assert_eq!(runner.resource_state(), ResourceState::Running);
}

#[test]
fn a_pause_after_a_stop_request_does_not_hold_the_restarted_program() {
    // Specification 11 §6.9: a stop ends the holds, and a program that starts later on the same
    // debugger can be held again, as a new run with nothing pending from the one that ended. The
    // `shutdown` is requested at 2 ms, a `pause` arrives at 3 ms while the resource still runs, and
    // the stop is taken at 10 ms. After a restart the program runs its statements unheld. Wrong
    // behaviour caught: a stale pause that holds the new run at its first statement (the loop would
    // spin until the test platform stops it), and a hold that `status` still reports for a program
    // that has ended.
    let fixture = Fixture::board_probe().with_debugger();
    let platform = TestPlatform {
        control_mode: ControlMode::Debug,
        ..TestPlatform::default()
    };
    let mut runner = fixture.runner_with(platform);
    fixture.pass_at(&mut runner, 0);
    let stopping = fixture.call_at(&mut runner, 2, &request(1, "shutdown"));
    assert_eq!(stopping["result"]["status"], json!("stopping"));
    let paused = fixture.call_at(&mut runner, 3, &request(2, "pause"));
    assert_eq!(paused["result"], json!({"status": "paused"}));
    fixture.pass_at(&mut runner, 10);
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    let stopped = fixture.call(&mut runner, &request(3, "status"));
    assert_eq!(stopped["result"]["state"], json!("stopped"));
    assert_eq!(
        stopped["result"]["debug_hold"],
        Value::Null,
        "a program that has ended holds nothing"
    );

    let restarted = fixture.call_at(
        &mut runner,
        20,
        &json!({"id": 4, "type": "restart", "params": {"mode": "cold"}}),
    );
    assert_eq!(restarted["result"], json!({"status": "restart queued"}));
    fixture.pass_at(&mut runner, 30);

    let running = fixture.call(&mut runner, &request(5, "status"));
    assert_eq!(running["result"]["state"], json!("running"));
    assert_eq!(
        depth_result(&runner),
        PlcValue::Int(1),
        "the restarted program ran its statements unheld"
    );
}

#[test]
fn a_hold_longer_than_the_cycle_timeout_does_not_fault() {
    // Specification 11 §6.6 and §6.9 and specification 34 STATE-07: the cycle watchdog is suspended
    // with the hold, and a hold is no active time. A scan with a 5 ms timeout that is held for 10 s
    // is no overrun. Wrong behaviour caught: a hold that does not count as held time, so the scan
    // faults with an execution timeout when the engine checks its deadline after the release, and
    // the loop's overrun check fails the resource on the wall time of 10 s.
    let fixture = Fixture::board_probe().with_debugger();
    fixture.script.borrow_mut().watchdog = WatchdogPolicy {
        enabled: true,
        timeout: value::Duration::from_millis(5),
        action: WatchdogAction::SafeHalt,
    };
    let mut runner = fixture.runner();
    runner
        .shell()
        .debug
        .add_breakpoint(once(on_led_assignment()));
    fixture.wait.advance_time();
    fixture.arrive_at(ms(10_010), &request(1, "resume"));
    runner.run_pass().expect("the test wait does not fail");
    assert_waited_for_the_held_scan(&fixture);

    runner.run_pass().expect("the test wait does not fail");

    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(runner.last_error(), None);
    assert_eq!(
        fixture.take_calls(),
        [
            CycleCall::Scan {
                now: ms(0),
                deadline: Some(ms(5))
            },
            CycleCall::Scan {
                now: ms(10),
                deadline: Some(ms(15))
            },
        ],
        "both scans ran, and no fault, safe state or restart followed the held one"
    );
    // The next scan is long due, so it runs first and the request waits for the pass after it
    // (STATE-08); that scan is not held, because the breakpoint stops at its first hit only.
    fixture.link.send(&request(2, "status"));
    runner.run_pass().expect("the test wait does not fail");
    runner.run_pass().expect("the test wait does not fail");
    let replies = fixture.link.take_replies();
    let status = replies.last().expect("the status request was answered");
    assert_eq!(status["result"]["metrics"]["overruns"], json!(0));
    assert_eq!(status["result"]["metrics"]["faults"], json!(0));
}

#[test]
fn a_step_from_a_hold_stops_at_the_next_statement() {
    // Specification 13 "Stepping Semantics" and "Run Control": each step resumes from a paused state
    // and pauses again at exactly the next statement boundary. The scan is held before
    // `led := button;` (23:1); the step runs that statement and stops before the next one, the
    // `CASE` statement (24:1), with the reason `step`, and the second hold lasts until the `resume`.
    // Wrong behaviour caught: a step that stops at the held statement again, one that runs on to the
    // end of the scan, and one that stops a statement late at the branch.
    let fixture = Fixture::board_probe().with_debugger();
    let mut runner = fixture.runner();
    runner.shell().debug.add_breakpoint(on_led_assignment());
    fixture.wait.advance_time();
    {
        let shell = Rc::clone(runner.shell());
        fixture
            .wait
            .at(ms(1_000), move || shell.debug.step(StepKind::Into));
    }
    fixture.arrive_at(ms(1_500), &request(1, "status"));
    fixture.arrive_at(ms(2_000), &request(2, "resume"));
    runner.run_pass().expect("the test wait does not fail");
    assert_waited_for_the_held_scan(&fixture);

    runner.run_pass().expect("the test wait does not fail");

    let replies = fixture.link.take_replies();
    assert_eq!(replies[0]["result"]["state"], json!("paused"));
    assert_eq!(
        replies[0]["result"]["debug_hold"],
        json!({"reason": "step"}),
        "the second hold reports its own reason"
    );
    assert_eq!(replies[1]["result"], json!({"status": "running"}));
    assert_eq!(
        runner.shell().debug.last_stop(),
        Some(LastStop {
            reason: StopReason::Step,
            statement: CASE_STATEMENT,
            thread: Some(1),
            breakpoint_generation: None,
        })
    );
    assert_eq!(depth_result(&runner), PlcValue::Int(1));
    assert_eq!(fixture.take_scan_times(), [0, 10], "one scan, held twice");
}

#[test]
fn a_second_pause_while_held_emits_no_second_stop() {
    // Specification 13 "Pause": a pause request while the scan is already paused succeeds and emits
    // no other pause event. In debug control mode the `pause` request reaches the debugger. Wrong
    // behaviour caught: a second pause that replaces the reason of the hold or emits a stop of its
    // own, so `status` reports `pause` for a scan that stopped at a breakpoint.
    let fixture = Fixture::board_probe().with_debugger();
    let platform = TestPlatform {
        control_mode: ControlMode::Debug,
        ..TestPlatform::default()
    };
    let mut runner = fixture.runner_with(platform);
    runner.shell().debug.add_breakpoint(on_led_assignment());
    fixture.wait.advance_time();
    fixture.arrive_at(ms(1_000), &request(1, "pause"));
    fixture.arrive_at(ms(1_500), &request(2, "status"));
    fixture.arrive_at(ms(2_000), &request(3, "resume"));
    runner.run_pass().expect("the test wait does not fail");
    assert_waited_for_the_held_scan(&fixture);

    runner.run_pass().expect("the test wait does not fail");

    let replies = fixture.link.take_replies();
    assert_eq!(replies[0]["result"], json!({"status": "paused"}));
    assert_eq!(
        replies[1]["result"]["debug_hold"],
        json!({"reason": "breakpoint"}),
        "the second pause changed nothing"
    );
    assert_eq!(
        runner.shell().debug.last_stop().map(|stop| stop.reason),
        Some(StopReason::Breakpoint)
    );
}

#[test]
fn a_debug_mode_pause_holds_the_next_scan_at_its_first_statement() {
    // Specification 11 §6.9 and specification 13 "Pause": in debug control mode a `pause` takes
    // effect at the next statement boundary, and `status` reports the hold from the request on.
    // The first scan runs no program; the pause arrives between scans; the next scan stops before
    // its first statement, `led := button;`, with the reason `pause`. Wrong behaviour caught: a
    // pause that holds nothing (the reply says `paused` and the scan runs on), and a hold that
    // stops at a later statement.
    let fixture = Fixture::board_probe().with_debugger();
    let platform = TestPlatform {
        control_mode: ControlMode::Debug,
        ..TestPlatform::default()
    };
    let mut runner = fixture.runner_with(platform);
    fixture.pass_at(&mut runner, 0);
    let paused = fixture.call_at(&mut runner, 2, &request(1, "pause"));
    assert_eq!(paused["result"], json!({"status": "paused"}));
    let before_the_stop = fixture.call(&mut runner, &request(2, "status"));
    assert_eq!(before_the_stop["result"]["state"], json!("paused"));
    assert_eq!(
        before_the_stop["result"]["debug_hold"],
        json!({"reason": "pause"})
    );

    fixture.wait.advance_time();
    fixture.arrive_at(ms(1_000), &request(3, "status"));
    fixture.arrive_at(ms(2_000), &request(4, "resume"));
    fixture.pass_at(&mut runner, HELD_SCAN_AT);

    let replies = fixture.link.take_replies();
    assert_eq!(
        replies[0]["result"]["debug_hold"],
        json!({"reason": "pause"})
    );
    assert_eq!(replies[1]["result"], json!({"status": "running"}));
    assert_eq!(
        runner.shell().debug.last_stop(),
        Some(LastStop {
            reason: StopReason::Pause,
            statement: LED_ASSIGNMENT,
            thread: Some(1),
            breakpoint_generation: None,
        })
    );
    assert_eq!(fixture.take_scan_times(), [0, 10]);
}

#[test]
fn a_disabled_debugger_never_holds_the_scan() {
    // Specification 11 §6.9: debug hooks are side-effect-free when debugging is disabled. With the
    // platform's debug surface off, a breakpoint holds nothing: each pass runs its scan and feeds
    // the watchdog once. Wrong behaviour caught: a hook that ignores the switch and holds the scan
    // (the test's wait would sleep for ever, and the feeds would be many).
    let fixture = Fixture::board_probe().with_debugger();
    let platform = TestPlatform {
        debug_enabled: false,
        ..TestPlatform::default()
    };
    let feeds = Rc::clone(&platform.feeds);
    let mut runner = fixture.runner_with(platform);
    runner.shell().debug.add_breakpoint(on_led_assignment());
    fixture.wait.advance_time();
    runner.run_pass().expect("the test wait does not fail");
    assert_waited_for_the_held_scan(&fixture);

    runner.run_pass().expect("the test wait does not fail");

    assert_eq!(fixture.take_scan_times(), [0, 10]);
    assert_eq!(
        feeds.get(),
        2,
        "the two passes fed once each, and no hold fed"
    );
    assert_eq!(depth_result(&runner), PlcValue::Int(1));
}

#[test]
fn a_write_queued_during_a_hold_waits_for_the_next_scan() {
    // Specification 11 §6.9: checked writes are accepted while the debugger holds the scan, and "a
    // write queued while a scan waits in the hold is not applied by that scan; it waits for the
    // boundary of the next scan". The write of `probe_depth := 3` reaches the held scan, which still
    // takes the `ELSE` branch (`depth_result` 1); the next scan applies it at its start and takes the
    // branch for 3 (`ProbeMiddle() + 1`, `depth_result` 3). Wrong behaviour caught: a write applied
    // to the suspended scan, one refused during the hold, and one lost.
    let fixture = Fixture::board_probe().with_debugger();
    let mut runner = fixture.runner();
    runner
        .shell()
        .debug
        .add_breakpoint(once(on_led_assignment()));
    fixture.wait.advance_time();
    fixture.arrive_at(
        ms(1_000),
        &json!({"id": 1, "type": "set", "params": {"target": "global:probe_depth", "value": "3"}}),
    );
    fixture.arrive_at(ms(2_000), &request(2, "resume"));
    runner.run_pass().expect("the test wait does not fail");
    assert_waited_for_the_held_scan(&fixture);

    runner.run_pass().expect("the test wait does not fail");

    let replies = fixture.link.take_replies();
    assert_eq!(replies[0]["result"], json!({"status": "queued"}));
    assert_eq!(depth_result(&runner), PlcValue::Int(1));

    runner.run_pass().expect("the test wait does not fail");

    assert_eq!(depth_result(&runner), PlcValue::Int(3));
}

#[test]
fn a_port_failure_in_a_hold_ends_the_run_with_that_failure() {
    // Specification 34 PORT-03: the loop returns the failure of a wait that fails. A hold cannot go
    // on without its wait, so the scan fails closed with the resource faulted, and the run ends with
    // the port's failure at its next pass. Wrong behaviour caught: a hold that drops the failure and
    // spins on, and a run that never learns the wait failed.
    let fixture = Fixture::board_probe().with_debugger();
    let mut runner = fixture.runner();
    runner.shell().debug.add_breakpoint(on_led_assignment());
    fixture.wait.advance_time();
    runner.run_pass().expect("the test wait does not fail");
    assert_waited_for_the_held_scan(&fixture);
    let failure = PortError { code: 9, device: 2 };
    fixture.wait.fail_next(failure);

    runner
        .run_pass()
        .expect("the pass that ran the scan returns");

    assert_eq!(runner.resource_state(), ResourceState::Faulted);
    assert!(
        matches!(runner.last_error(), Some(RuntimeError::ControlError(_))),
        "the scan failed: {:?}",
        runner.last_error()
    );
    assert_eq!(runner.run(), failure);
}
