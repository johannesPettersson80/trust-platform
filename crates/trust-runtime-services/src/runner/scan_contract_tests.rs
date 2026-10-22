//! The resource loop's scans (specification 34, STATE-03 to STATE-08): what a scan records, what
//! follows a failed or overrunning scan by the shared supervisor decisions, how a stop fails, and
//! that the loop waits instead of spinning (PORT-03), with the board probe on the real engine.

use alloc::rc::Rc;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::time::Duration;

use serde_json::{json, Value};
use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::{MonotonicInstant, PortError};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value;
use trust_runtime_core::vm::{CycleHooks, PreparedModule, RetainApplyEvent};
use trust_runtime_core::watchdog::{FaultPolicy, WatchdogAction, WatchdogPolicy};

use super::{ResourceCycle, RetainSource, Runner};
use crate::events::EVENT_STORE_CAPACITY;
use crate::runner::engine::{device_limits, CycleSettings, DeviceServices, EngineCycle, ProcessIo};
use crate::status::IoDriverStatus;
use crate::tests::runner_ports::{
    ms, CycleCall, Fixture, TestClock, TestPlatform, TestRunner, WaitCall, CHIP_MEMORY, GPIO,
};

fn request(id: u64, kind: &str) -> Value {
    json!({"id": id, "type": kind})
}

fn warm_restart(id: u64) -> Value {
    json!({"id": id, "type": "restart", "params": {"mode": "warm"}})
}

fn scan(at: u64, deadline: Option<u64>) -> CycleCall {
    CycleCall::Scan {
        now: ms(at),
        deadline: deadline.map(ms),
    }
}

fn latch(error: &RuntimeError) -> CycleCall {
    CycleCall::Latch(error.to_string())
}

fn restart_from(source: RetainSource) -> CycleCall {
    CycleCall::Restart {
        mode: RestartMode::Warm,
        source,
    }
}

fn timing(at: u64) -> CycleCall {
    CycleCall::TaskTiming(ms(at))
}

fn watchdog(action: WatchdogAction) -> WatchdogPolicy {
    WatchdogPolicy {
        enabled: true,
        timeout: value::Duration::from_millis(5),
        action,
    }
}

fn events(fixture: &Fixture, runner: &mut TestRunner<'_>) -> Value {
    fixture.call(runner, &request(900, "events.tail"))["result"]["events"].clone()
}

#[test]
fn a_scan_records_the_events_and_metrics_that_trust_runtime_records() {
    let fixture = Fixture::board_probe();
    fixture.script.borrow_mut().scan_time = ms(1);
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    fixture.pass_at(&mut runner, 10);
    fixture.clock.set(ms(12));
    assert_eq!(
        events(&fixture, &mut runner),
        json!([]),
        "two clean scans record no event: the store keeps none of the events that every scan \
         makes (specification 11, the event store); their timing is in `tasks.stats` and the \
         metrics below"
    );
    let task_stats = fixture.call(&mut runner, &request(1, "tasks.stats"));
    assert_eq!(
        task_stats["result"],
        json!({
            "tasks": [{
                "name": "Scan",
                "min_ms": 0.0,
                "avg_ms": 0.0,
                "max_ms": 0.0,
                "last_ms": 0.0,
                "overruns": 0,
            }],
            "profiling_enabled": true,
            "top_contributors": [{
                "key": "program:Plant",
                "kind": "program",
                "name": "Plant",
                "avg_cycle_ms": 0.0,
                "cycle_pct": 0.0,
                "last_ms": 0.0,
                "last_cycle_pct": 0.0,
            }],
        })
    );
    let status = fixture.call(&mut runner, &request(2, "status"));
    let metrics = &status["result"]["metrics"];
    assert_eq!(
        metrics["cycle_ms"],
        json!({
            "min": 1.0,
            "avg": 1.0,
            "max": 1.0,
            "last": 1.0,
            "p50": 1.0,
            "p95": 1.0,
            "p99": 1.0,
            "window_samples": 2,
        }),
        "a scan's duration is its time on the monotonic clock"
    );
    assert_eq!(metrics["faults"], json!(0));
    assert_eq!(metrics["overruns"], json!(0));
}

#[test]
fn a_failing_scan_halts_the_program_until_a_restart_recovers_it() {
    let fixture = Fixture::board_probe();
    fixture
        .script
        .borrow_mut()
        .failing_scans
        .push_back(RuntimeError::DivisionByZero);
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    assert_eq!(
        fixture.take_calls(),
        [scan(0, None), latch(&RuntimeError::DivisionByZero)],
        "fault policy halt: the engine latches the fault and the outputs stay as they are"
    );
    assert_eq!(runner.resource_state(), ResourceState::Faulted);
    assert_eq!(runner.last_error(), Some(RuntimeError::DivisionByZero));

    fixture.clock.set(ms(1));
    let status = fixture.call(&mut runner, &request(1, "status"));
    assert_eq!(status["result"]["state"], json!("faulted"));
    assert_eq!(status["result"]["fault"], json!("division by zero"));
    assert_eq!(status["result"]["metrics"]["faults"], json!(1));
    let faults = fixture.call(&mut runner, &request(2, "faults"));
    assert_eq!(
        faults["result"]["faults"],
        json!([{"type": "fault", "error": "division by zero", "time_ns": 0}])
    );
    let health = fixture.call(&mut runner, &request(3, "health"));
    assert_eq!(health["result"]["ok"], json!(false));
    fixture.pass_at(&mut runner, 30);
    assert_eq!(
        fixture.take_calls(),
        [] as [CycleCall; 0],
        "a faulted program runs no scan"
    );

    let reply = fixture.call_at(&mut runner, 31, &warm_restart(4));
    assert_eq!(reply["result"], json!({"status": "restart queued"}));
    assert_eq!(
        runner.resource_state(),
        ResourceState::Faulted,
        "the restart waits for the faulted wait's next poll"
    );
    fixture.pass_at(&mut runner, 40);
    assert_eq!(
        fixture.take_calls(),
        [
            restart_from(RetainSource::Checkpoint),
            timing(40),
            scan(40, None)
        ],
        "the poll's restart recovers the program, which scans at once"
    );
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(runner.last_error(), None);
    let status = fixture.call(&mut runner, &request(5, "status"));
    assert_eq!(status["result"]["fault"], Value::Null);
}

#[test]
fn a_failed_restart_faults_a_running_program_and_keeps_a_faulted_one_faulted() {
    let unreadable = RuntimeError::RetainStore(SmolStr::new("checkpoint unreadable"));

    let fixture = Fixture::board_probe();
    fixture
        .script
        .borrow_mut()
        .failing_restarts
        .push_back(unreadable.clone());
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let _ = fixture.take_calls();
    let _ = fixture.call_at(&mut runner, 3, &warm_restart(1));
    fixture.pass_at(&mut runner, 10);
    assert_eq!(runner.resource_state(), ResourceState::Faulted);
    assert_eq!(runner.last_error(), Some(unreadable.clone()));
    assert_eq!(
        fixture.take_calls(),
        [restart_from(RetainSource::Checkpoint)],
        "the failed restart halts without a fault action and without a scan, as on the host"
    );

    let fixture = Fixture::board_probe();
    {
        let mut script = fixture.script.borrow_mut();
        script.failing_scans.push_back(RuntimeError::DivisionByZero);
        script.failing_restarts.push_back(unreadable.clone());
    }
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let _ = fixture.call_at(&mut runner, 3, &warm_restart(1));
    fixture.pass_at(&mut runner, 10);
    assert_eq!(runner.resource_state(), ResourceState::Faulted);
    assert_eq!(
        runner.last_error(),
        Some(unreadable),
        "the new error replaces the root fault"
    );
    let _ = fixture.call(&mut runner, &warm_restart(2));
    fixture.pass_at(&mut runner, 20);
    assert_eq!(runner.resource_state(), ResourceState::Running);
}

#[test]
fn safe_halt_applies_the_safe_outputs_and_reports_their_failure_with_its_root() {
    let offline = RuntimeError::ControlError(SmolStr::new("output driver offline"));
    let fixture = Fixture::board_probe();
    {
        let mut script = fixture.script.borrow_mut();
        script.fault_policy = FaultPolicy::SafeHalt;
        script.failing_scans.push_back(RuntimeError::DivisionByZero);
        script.safe_outputs_error = Some(offline.clone());
    }
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    assert_eq!(
        fixture.take_calls(),
        [
            scan(0, None),
            latch(&RuntimeError::DivisionByZero),
            CycleCall::SafeOutputs,
        ]
    );
    assert_eq!(
        runner.last_error(),
        Some(RuntimeError::SafeStateFailed {
            root: SmolStr::new("division by zero"),
            error: SmolStr::new(offline.to_string()),
        })
    );
    fixture.clock.set(ms(1));
    assert_eq!(
        events(&fixture, &mut runner),
        json!([
            {"type": "fault", "error": "division by zero", "time_ns": 0},
            {
                "type": "safe_state_failed",
                "root": "division by zero",
                "error": offline.to_string(),
                "time_ns": 0,
            },
        ])
    );
}

#[test]
fn fault_policy_restart_restarts_in_place_with_backoff_until_the_budget_is_spent() {
    let fixture = Fixture::board_probe();
    {
        let mut script = fixture.script.borrow_mut();
        script.fault_policy = FaultPolicy::Restart;
        for _ in 0..4 {
            script.failing_scans.push_back(RuntimeError::DivisionByZero);
        }
    }
    let mut runner = fixture.runner();
    let divide = latch(&RuntimeError::DivisionByZero);
    let in_memory = restart_from(RetainSource::Memory);
    fixture.pass_at(&mut runner, 0);
    assert_eq!(
        fixture.take_calls(),
        [scan(0, None), divide.clone(), in_memory.clone(), timing(0)]
    );
    assert_eq!(runner.resource_state(), ResourceState::Running);
    // The backoff starts at the cycle interval and doubles with each restart in a row.
    for (not_due, due, backoff) in [(9, 10, 20), (29, 30, 40)] {
        fixture.pass_at(&mut runner, not_due);
        assert_eq!(
            fixture.take_calls(),
            [] as [CycleCall; 0],
            "the backoff runs until {due} ms"
        );
        fixture.pass_at(&mut runner, due);
        assert_eq!(
            fixture.take_calls(),
            [
                scan(due, None),
                divide.clone(),
                in_memory.clone(),
                timing(due)
            ]
        );
        let waits: Vec<WaitCall> = fixture.wait.take_calls();
        assert_eq!(
            waits.last().map(|call| call.deadline),
            Some(ms(due + backoff))
        );
    }
    fixture.pass_at(&mut runner, 70);
    assert_eq!(fixture.take_calls(), [scan(70, None), divide]);
    assert_eq!(runner.resource_state(), ResourceState::Faulted);
    assert_eq!(
        runner.last_error(),
        Some(RuntimeError::RestartLimitExceeded {
            attempts: 3,
            reason: SmolStr::new("division by zero"),
        })
    );
    let status = fixture.call(&mut runner, &request(1, "status"));
    assert_eq!(status["result"]["metrics"]["faults"], json!(4));
}

#[test]
fn a_clean_scan_renews_the_automatic_restart_budget() {
    let fixture = Fixture::board_probe();
    {
        let mut script = fixture.script.borrow_mut();
        script.fault_policy = FaultPolicy::Restart;
        script.failing_scans.push_back(RuntimeError::DivisionByZero);
    }
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    fixture.pass_at(&mut runner, 10);
    let _ = fixture.take_calls();
    let _ = fixture.wait.take_calls();
    fixture
        .script
        .borrow_mut()
        .failing_scans
        .push_back(RuntimeError::DivisionByZero);
    fixture.pass_at(&mut runner, 20);
    assert_eq!(
        fixture.take_calls(),
        [
            scan(20, None),
            latch(&RuntimeError::DivisionByZero),
            restart_from(RetainSource::Memory),
            timing(20),
        ]
    );
    assert_eq!(
        fixture.wait.take_calls().last().map(|call| call.deadline),
        Some(ms(30)),
        "after a clean scan the next automatic restart is the first again, with the shortest \
         backoff"
    );
}

#[test]
fn an_overrunning_scan_follows_the_watchdog_action() {
    let fixture = Fixture::board_probe();
    {
        let mut script = fixture.script.borrow_mut();
        script.watchdog = watchdog(WatchdogAction::SafeHalt);
        script.scan_time = ms(6);
    }
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    assert_eq!(
        fixture.take_calls(),
        [
            scan(0, Some(5)),
            latch(&RuntimeError::WatchdogTimeout),
            CycleCall::SafeOutputs,
        ],
        "the scan returned after its 5 ms budget: watchdog timeout with safe outputs"
    );
    assert_eq!(runner.resource_state(), ResourceState::Faulted);
    assert_eq!(runner.last_error(), Some(RuntimeError::WatchdogTimeout));
    assert_eq!(
        events(&fixture, &mut runner),
        json!([{"type": "fault", "error": "watchdog timeout", "time_ns": 0}]),
        "the watchdog's fault, at the scan's time, is the scan's only event"
    );

    let fixture = Fixture::board_probe();
    {
        let mut script = fixture.script.borrow_mut();
        script.watchdog = watchdog(WatchdogAction::Restart);
        script.scan_time = ms(6);
    }
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    assert_eq!(
        fixture.take_calls(),
        [
            scan(0, Some(5)),
            restart_from(RetainSource::Memory),
            timing(6)
        ],
        "watchdog action restart: an automatic warm restart without a fault action, whose task \
         timing starts when the restart is done"
    );
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(
        fixture.wait.take_calls().last().map(|call| call.deadline),
        Some(ms(16))
    );
}

#[test]
fn a_scan_the_engine_stops_at_its_deadline_faults_twice_as_on_the_host() {
    let fixture = Fixture::board_probe();
    {
        let mut script = fixture.script.borrow_mut();
        script.watchdog = watchdog(WatchdogAction::SafeHalt);
        script.stall = ms(6);
    }
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    assert_eq!(
        fixture.take_calls(),
        [
            scan(0, Some(5)),
            latch(&RuntimeError::ExecutionTimeout),
            latch(&RuntimeError::WatchdogTimeout),
            CycleCall::SafeOutputs,
        ],
        "the engine's timeout follows the fault policy, then the watchdog action"
    );
    assert_eq!(runner.last_error(), Some(RuntimeError::WatchdogTimeout));
    let faults = fixture.call(&mut runner, &request(1, "faults"));
    assert_eq!(
        faults["result"]["faults"],
        json!([
            {"type": "fault", "error": "watchdog timeout", "time_ns": 0},
            {"type": "fault", "error": "execution timed out", "time_ns": 0},
        ])
    );
}

#[test]
fn a_stop_attempts_both_steps_and_faults_with_the_first_failure() {
    let offline = RuntimeError::ControlError(SmolStr::new("output driver offline"));
    let busy = RuntimeError::RetainStore(SmolStr::new("flash busy"));
    for (safe_outputs, retain, expected) in [
        (Some(offline.clone()), Some(busy.clone()), offline.clone()),
        (None, Some(busy.clone()), busy.clone()),
    ] {
        let fixture = Fixture::board_probe();
        {
            let mut script = fixture.script.borrow_mut();
            script.safe_outputs_error = safe_outputs;
            script.retain_error = retain;
        }
        let mut runner = fixture.runner();
        fixture.pass_at(&mut runner, 0);
        let _ = fixture.take_calls();
        let reply = fixture.call(&mut runner, &request(1, "shutdown"));
        assert_eq!(reply["result"], json!({"status": "stopping"}));
        fixture.pass_at(&mut runner, 10);
        assert_eq!(
            fixture.take_calls(),
            [CycleCall::SafeOutputs, CycleCall::SaveRetain]
        );
        assert_eq!(runner.resource_state(), ResourceState::Faulted);
        assert_eq!(runner.last_error(), Some(expected));
    }
}

#[test]
fn the_retained_values_a_restart_dropped_are_recorded_at_the_programs_time() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    fixture.pass_at(&mut runner, 10);
    fixture.script.borrow_mut().restart_events = alloc::vec![RetainApplyEvent::Orphan {
        name: SmolStr::new("old_total"),
    }];
    let _ = fixture.call_at(&mut runner, 13, &warm_restart(1));
    fixture.pass_at(&mut runner, 20);
    let newest = fixture.call(
        &mut runner,
        &json!({"id": 2, "type": "events.tail", "params": {"limit": 3}}),
    );
    assert_eq!(
        newest["result"]["events"],
        json!([
            {"type": "retain_orphan_dropped", "name": "old_total", "time_ns": 10_000_000},
        ]),
        "the program's time is its last scan's until the restart starts its task timing again"
    );
}

#[test]
fn the_loop_waits_for_its_next_scan_or_a_wake_and_never_spins() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    for at in [0, 3, 7] {
        fixture.pass_at(&mut runner, at);
    }
    assert_eq!(
        fixture.wait.take_calls(),
        [WaitCall {
            deadline: ms(10),
            sleeps: true,
        }; 3],
        "every pass waits once, for the next scan, and sleeps when nothing woke it"
    );

    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner_with(TestPlatform {
        wake_during_idle: Some(fixture.wait.clone()),
        ..TestPlatform::default()
    });
    fixture.pass_at(&mut runner, 0);
    assert_eq!(
        fixture.wait.take_calls(),
        [WaitCall {
            deadline: ms(10),
            sleeps: false,
        }],
        "a wake after the token was read makes the wait return at once (PORT-03)"
    );

    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let _ = fixture.call(&mut runner, &request(1, "shutdown"));
    fixture.pass_at(&mut runner, 10);
    let _ = fixture.wait.take_calls();
    for at in [100, 1_000] {
        fixture.pass_at(&mut runner, at);
    }
    assert_eq!(
        fixture.wait.take_calls(),
        [
            WaitCall {
                deadline: ms(1_100),
                sleeps: true,
            },
            WaitCall {
                deadline: ms(2_000),
                sleeps: true,
            },
        ],
        "a stopped device sleeps until the link wakes it or the watchdog needs feeding"
    );
}

#[test]
fn run_serves_until_a_port_fails_and_returns_the_failure() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    let failure = PortError { code: 7, device: 1 };
    fixture.wait.fail_next(failure);
    assert_eq!(runner.run(), failure);
    assert_eq!(
        fixture.take_scan_times(),
        [0],
        "the pass ran its scan before its wait failed"
    );
}

/// One step of a scan's input phase in the holding tests: time that passes, or a debugger hold that
/// lasts its time.
#[derive(Clone, Copy)]
enum Step {
    Work(Duration),
    Hold(Duration),
}

/// The process I/O of the holding tests. Each scan runs the test's steps at its input phase, so the
/// time they take is inside the scan, where the engine's deadline and the loop's overrun check both
/// see it. It reads no inputs and drives no outputs.
struct HoldingIo<'a> {
    services: &'a DeviceServices<TestClock>,
    clock: TestClock,
    steps: Rc<RefCell<Vec<Step>>>,
}

impl ProcessIo for HoldingIo<'_> {
    fn read_inputs(&mut self, _image: &mut [u8]) -> Result<(), RuntimeError> {
        for step in self.steps.borrow().iter() {
            match *step {
                Step::Work(time) => self.clock.advance(time),
                Step::Hold(time) => {
                    self.services.hold_started();
                    self.clock.advance(time);
                    self.services.hold_ended();
                }
            }
        }
        Ok(())
    }

    fn write_outputs(&mut self, _image: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_safe_outputs(&mut self, _image: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn driver_health(&self) -> Vec<IoDriverStatus> {
        Vec::new()
    }
}

/// An observer that records nothing, so the engine's own rules are what the tests see.
struct NoHooks;

impl CycleHooks for NoHooks {}

/// The engine cycle of the board probe, its input phase running `steps` in every scan, started as a
/// chip starts it under `watchdog`.
fn held_cycle<'a>(
    prepared: &'a PreparedModule,
    services: &'a DeviceServices<TestClock>,
    clock: TestClock,
    steps: Rc<RefCell<Vec<Step>>>,
    watchdog: WatchdogPolicy,
) -> EngineCycle<'a, TestClock, HoldingIo<'a>> {
    let settings = CycleSettings {
        // Long enough that no second scan is due while the test reads the status.
        cycle_interval: value::Duration::from_millis(1_000),
        watchdog,
        fault_policy: FaultPolicy::Halt,
        safe_outputs: Vec::new(),
    };
    let io = HoldingIo {
        services,
        clock,
        steps,
    };
    let mut cycle =
        EngineCycle::start(prepared, services, io, settings).expect("the board probe starts");
    cycle.reset_task_timing(value::Duration::ZERO);
    cycle
}

/// Runs one scan of the board probe at logical time zero, its input phase running `steps`, with its
/// deadline 5 ms after its start, and returns the engine's result.
fn run_held_scan(steps: Vec<Step>) -> Result<(), RuntimeError> {
    let clock = TestClock::default();
    let prepared = PreparedModule::from_bytes(GPIO, device_limits(CHIP_MEMORY))
        .expect("the board probe is admitted");
    let services = DeviceServices::new(clock.clone());
    let mut cycle = held_cycle(
        &prepared,
        &services,
        clock,
        Rc::new(RefCell::new(steps)),
        watchdog(WatchdogAction::SafeHalt),
    );
    cycle.run_scan(
        value::Duration::ZERO,
        Some(MonotonicInstant::from_origin(ms(5))),
        &mut NoHooks,
    )
}

#[test]
fn two_holds_in_one_scan_add_up() {
    // Spec 11 §6.6: the deadline moves by each completed hold. The 5 ms timeout and holds of 3 ms
    // and 3 ms make a deadline of 11 ms, and the scan ends at 10 ms with 4 ms of its own work.
    // Wrong behaviour caught: a total that keeps only one hold moves the deadline to 8 ms, so the
    // scan faults at 10 ms.
    let result = run_held_scan(vec![
        Step::Hold(ms(3)),
        Step::Work(ms(4)),
        Step::Hold(ms(3)),
    ]);
    assert_eq!(result, Ok(()), "the two holds add up to 6 ms of held time");
}

#[test]
fn a_scan_faults_when_its_active_time_exceeds_the_timeout() {
    // Spec 11 §6.6 and §6.9: a hold excuses its own time and no more. The scan does 3 ms of work,
    // holds for 10 ms and does 3 ms more: 6 ms of active time against a 5 ms timeout. Its deadline
    // is 5 ms plus the hold, 15 ms, and the scan reaches 16 ms. Wrong behaviour caught: a deadline
    // reset at the end of the hold, due at 18 ms, lets the scan finish.
    let result = run_held_scan(vec![
        Step::Work(ms(3)),
        Step::Hold(ms(10)),
        Step::Work(ms(3)),
    ]);
    assert_eq!(
        result,
        Err(RuntimeError::ExecutionTimeout),
        "6 ms of active time exceed the 5 ms timeout"
    );
}

#[test]
fn a_scan_held_longer_than_its_timeout_does_not_fault() {
    // Spec 11 §6.6 and spec 34 STATE-07: a hold is no active time. A scan that does 2 ms of work
    // and then holds for 20 ms, against a 5 ms timeout, is no overrun. Wrong behaviour caught: a
    // loop that measures wall time (22 ms) faults the resource when the scan ends.
    let ports = Fixture::board_probe();
    let prepared = PreparedModule::from_bytes(GPIO, device_limits(CHIP_MEMORY))
        .expect("the board probe is admitted");
    let services = DeviceServices::new(ports.clock.clone());
    let cycle = held_cycle(
        &prepared,
        &services,
        ports.clock.clone(),
        Rc::new(RefCell::new(vec![Step::Work(ms(2)), Step::Hold(ms(20))])),
        watchdog(WatchdogAction::SafeHalt),
    );
    let mut runner = Runner::new(
        ports.link.clone(),
        ports.wait.clone(),
        cycle,
        TestPlatform::default(),
        ports.clock.clone(),
        EVENT_STORE_CAPACITY,
    );
    ports.clock.set(ms(0));
    runner.run_pass().expect("the test wait does not fail");
    ports.link.send(&request(1, "status"));
    runner.run_pass().expect("the test wait does not fail");
    let mut replies = ports.link.take_replies();
    assert_eq!(replies.len(), 1, "one reply to the status request");
    let status = replies.remove(0);
    assert_eq!(
        runner.resource_state(),
        ResourceState::Running,
        "the held scan is no overrun, so the program keeps running"
    );
    assert_eq!(status["result"]["metrics"]["overruns"], json!(0));
    assert_eq!(status["result"]["metrics"]["faults"], json!(0));
}

#[test]
fn the_cycle_metric_excludes_held_time() {
    // Spec 34 STATE-07 and spec 11 §6.6: a scan's cycle time is its active time: 2 ms of work and
    // 3 ms after a 50 ms hold, so 5 ms. Wrong behaviour caught: a metric of wall time records 55 ms.
    let ports = Fixture::board_probe();
    let prepared = PreparedModule::from_bytes(GPIO, device_limits(CHIP_MEMORY))
        .expect("the board probe is admitted");
    let services = DeviceServices::new(ports.clock.clone());
    let cycle = held_cycle(
        &prepared,
        &services,
        ports.clock.clone(),
        Rc::new(RefCell::new(vec![
            Step::Work(ms(2)),
            Step::Hold(ms(50)),
            Step::Work(ms(3)),
        ])),
        WatchdogPolicy::default(),
    );
    let mut runner = Runner::new(
        ports.link.clone(),
        ports.wait.clone(),
        cycle,
        TestPlatform::default(),
        ports.clock.clone(),
        EVENT_STORE_CAPACITY,
    );
    ports.clock.set(ms(0));
    runner.run_pass().expect("the test wait does not fail");
    ports.link.send(&request(1, "status"));
    runner.run_pass().expect("the test wait does not fail");
    let mut replies = ports.link.take_replies();
    assert_eq!(replies.len(), 1, "one reply to the status request");
    let status = replies.remove(0);
    let cycle_ms = &status["result"]["metrics"]["cycle_ms"];
    assert_eq!(
        cycle_ms["last"],
        json!(5.0),
        "the scan's active time is 5 ms"
    );
    assert_eq!(
        cycle_ms["max"],
        json!(5.0),
        "the one scan is the largest cycle"
    );
}
