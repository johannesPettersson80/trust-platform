//! When the resource loop acts (specification 34, STATE-08, PORT-03 and TIME-04): the program's
//! logical time starts with the program, the faulted wait takes requests at its polls, the loop
//! feeds the hardware watchdog in every state, and the command queue has trust-runtime's bound.
//! With the board probe on the real engine.

use alloc::rc::Rc;
use alloc::vec::Vec;

use serde_json::{json, Value};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value;

use super::RetainSource;
use crate::resource::commands::RESOURCE_COMMAND_CAPACITY;
use crate::tests::runner_ports::{ms, CycleCall, Fixture, TestPlatform, TestRunner};

fn request(id: u64, kind: &str) -> Value {
    json!({"id": id, "type": kind})
}

fn restart(id: u64, mode: &str) -> Value {
    json!({"id": id, "type": "restart", "params": {"mode": mode}})
}

fn scan(at: u64) -> CycleCall {
    CycleCall::Scan {
        now: ms(at),
        deadline: None,
    }
}

fn from_checkpoint(mode: RestartMode) -> CycleCall {
    CycleCall::Restart {
        mode,
        source: RetainSource::Checkpoint,
    }
}

/// Runs one pass at each of `times` milliseconds and returns how far ahead of its pass each wait's
/// deadline lies, in milliseconds.
fn waits_ahead(fixture: &Fixture, runner: &mut TestRunner<'_>, times: &[u64]) -> Vec<u64> {
    times
        .iter()
        .map(|&at| {
            let _ = fixture.wait.take_calls();
            fixture.pass_at(runner, at);
            let waits = fixture.wait.take_deadlines();
            assert_eq!(waits.len(), 1, "one wait in the pass at {at} ms");
            u64::try_from(waits[0].as_millis()).expect("test deadlines are small") - at
        })
        .collect()
}

#[test]
fn power_on_counts_logical_time_from_the_program_start_not_from_the_chip_reset() {
    let fixture = Fixture::board_probe();
    fixture.clock.set(ms(350));
    let mut runner = fixture.power_on(TestPlatform::default());
    assert_eq!(
        fixture.take_calls(),
        [CycleCall::TaskTiming(ms(0))],
        "the loop starts the task timing with the program's logical time"
    );
    fixture.pass_at(&mut runner, 350);
    let tasks = fixture.call(&mut runner, &request(3, "tasks.stats"));
    assert_eq!(
        tasks["result"]["tasks"],
        json!([]),
        "the scan at the start runs no task: none is due at logical time zero"
    );
    fixture.pass_at(&mut runner, 360);
    assert_eq!(
        fixture.take_calls(),
        [scan(0), scan(10)],
        "the first scan runs at logical time zero, 350 ms after the chip's reset"
    );
    let tasks = fixture.call(&mut runner, &request(4, "tasks.stats"));
    assert_eq!(
        tasks["result"]["tasks"][0]["name"],
        json!("Scan"),
        "the task first runs one interval after the start"
    );
    let events = fixture.call(&mut runner, &request(1, "events.tail"));
    assert_eq!(
        events["result"]["events"],
        json!([]),
        "the boot time is no missed activation: no overrun is recorded"
    );
    let status = fixture.call(&mut runner, &request(2, "status"));
    assert_eq!(status["result"]["metrics"]["overruns"], json!(0));
    assert_eq!(
        status["result"]["uptime_ms"],
        json!(10),
        "the uptime counts from the program's start"
    );
}

#[test]
fn the_faulted_wait_takes_a_restart_at_its_next_poll_and_keeps_the_records() {
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
    assert_eq!(
        fixture.wait.take_deadlines(),
        [ms(10)],
        "the faulted wait polls every 10 ms, as trust-runtime's does"
    );

    let _ = fixture.call_at(&mut runner, 3, &restart(1, "warm"));
    let status = fixture.call_at(&mut runner, 7, &request(2, "status"));
    assert_eq!(
        status["result"]["state"],
        json!("faulted"),
        "the restart waits for the poll"
    );
    fixture.pass_at(&mut runner, 10);
    assert_eq!(
        fixture.take_calls(),
        [
            from_checkpoint(RestartMode::Warm),
            CycleCall::TaskTiming(ms(10)),
            scan(10)
        ],
        "the poll takes the restart, and the recovered program scans at once"
    );
    assert_eq!(runner.resource_state(), ResourceState::Running);
    let status = fixture.call(&mut runner, &request(3, "status"));
    assert_eq!(
        status["result"]["metrics"]["faults"],
        json!(1),
        "a recovery restarts the program in place: the records stay"
    );
}

#[test]
fn at_a_poll_of_the_faulted_wait_a_restart_comes_before_a_stop() {
    let fixture = Fixture::board_probe();
    fixture
        .script
        .borrow_mut()
        .failing_scans
        .push_back(RuntimeError::DivisionByZero);
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let _ = fixture.take_calls();

    fixture.clock.set(ms(3));
    fixture.link.send(&restart(1, "cold"));
    fixture.link.send(&request(2, "shutdown"));
    runner.run_pass().expect("the test wait does not fail");
    assert_eq!(fixture.link.take_replies().len(), 2);
    assert_eq!(fixture.take_calls(), [] as [CycleCall; 0]);
    fixture.pass_at(&mut runner, 10);
    assert_eq!(
        fixture.take_calls(),
        [
            from_checkpoint(RestartMode::Cold),
            CycleCall::TaskTiming(ms(10)),
            CycleCall::SafeOutputs,
            CycleCall::SaveRetain,
        ],
        "the restart recovers the program first; its boundary then takes the stop before a \
         scan, as trust-runtime's scan loop checks the stop first when its faulted wait returns"
    );
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
}

#[test]
fn the_loop_feeds_the_hardware_watchdog_in_every_state_and_never_waits_past_its_feed_interval() {
    let fixture = Fixture::board_probe();
    fixture.script.borrow_mut().cycle_interval = value::Duration::from_millis(100);
    let platform = TestPlatform {
        feed_interval: ms(25),
        ..TestPlatform::default()
    };
    let feeds = Rc::clone(&platform.feeds);
    let idle_passes = Rc::clone(&platform.idle_passes);
    let mut runner = fixture.runner_with(platform);

    // Running, with scans 100 ms apart.
    assert_eq!(
        waits_ahead(&fixture, &mut runner, &[0, 25, 50, 75, 100]),
        [25; 5]
    );
    assert_eq!(fixture.take_scan_times(), [0, 100]);

    // Paused from the boundary at 200 ms.
    let _ = fixture.call_at(&mut runner, 110, &request(1, "pause"));
    assert_eq!(
        waits_ahead(&fixture, &mut runner, &[200, 225, 250]),
        [25; 3]
    );
    assert_eq!(runner.resource_state(), ResourceState::Paused);

    // Stopped from the boundary at 300 ms: no boundary any more, only the watchdog's feeds.
    let _ = fixture.call_at(&mut runner, 260, &request(2, "shutdown"));
    assert_eq!(
        waits_ahead(&fixture, &mut runner, &[300, 325, 350]),
        [25; 3]
    );
    assert_eq!(runner.resource_state(), ResourceState::Stopped);

    // Faulted: the faulted wait polls every 10 ms, sooner than the feed interval.
    fixture
        .script
        .borrow_mut()
        .failing_scans
        .push_back(RuntimeError::DivisionByZero);
    let _ = fixture.call_at(&mut runner, 360, &restart(3, "cold"));
    assert_eq!(runner.resource_state(), ResourceState::Faulted);
    assert_eq!(waits_ahead(&fixture, &mut runner, &[370, 380]), [10, 10]);

    assert_eq!(
        feeds.get(),
        16,
        "every one of the 16 passes fed the watchdog once, after its work"
    );
    assert_eq!(
        idle_passes.get(),
        16,
        "and did the platform's idle work once"
    );
}

#[test]
fn a_full_command_queue_refuses_a_command_with_resource_busy_as_trust_runtime_does() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let _ = fixture.take_calls();

    fixture.clock.set(ms(1));
    for index in 0..RESOURCE_COMMAND_CAPACITY {
        let kind = if index % 2 == 0 { "pause" } else { "resume" };
        let id = u64::try_from(index).expect("small index");
        fixture.link.send(&request(id, kind));
    }
    let mut replies = Vec::new();
    while replies.len() < RESOURCE_COMMAND_CAPACITY {
        runner.run_pass().expect("the test wait does not fail");
        replies.extend(fixture.link.take_replies());
    }
    assert!(
        replies.iter().all(|reply| reply["ok"] == json!(true)),
        "the queue takes {RESOURCE_COMMAND_CAPACITY} commands before the boundary"
    );
    let busy = fixture.call(&mut runner, &request(5_000, "pause"));
    assert_eq!(busy["error_code"], json!("resource_busy"));
    assert_eq!(
        busy["error"],
        json!("resource command queue is full; the request was not queued")
    );

    fixture.pass_at(&mut runner, 10);
    assert_eq!(
        fixture.take_scan_times(),
        [10],
        "the boundary takes the queued commands in order, and the last one was a resume"
    );
    let accepted = fixture.call(&mut runner, &request(5_001, "pause"));
    assert_eq!(accepted["result"], json!({"status": "paused"}));
}

#[test]
fn a_scan_that_overruns_its_interval_still_lets_the_link_answer_one_request_per_scan() {
    // Specification 34, STATE-08: a boundary that is due again as soon as it is taken (here a
    // 15 ms scan on a 10 ms interval) must not hold every request back, or the link is starved.
    let fixture = Fixture::board_probe();
    fixture.script.borrow_mut().scan_time = ms(15);
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let _ = fixture.take_calls();
    for id in 0..3 {
        fixture.link.send(&request(id, "health"));
    }

    // The boundary at 10 ms is overdue at 15 ms: request 0 is answered, then the next boundary
    // runs the scan (15 ms to 30 ms) before request 1.
    fixture.pass_at(&mut runner, 15);
    assert_eq!(fixture.take_scan_times(), [15]);
    let replies = fixture.link.take_replies();
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0]["id"], json!(0));

    // The grid's next boundary, 20 ms, had passed when the scan ended at 30 ms, so the next scan is
    // due at once: request 1 is answered, then the scan runs.
    fixture.pass_at(&mut runner, 30);
    assert_eq!(fixture.take_scan_times(), [30]);
    let replies = fixture.link.take_replies();
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0]["id"], json!(1));
}

#[test]
fn a_late_wake_does_not_move_the_cycle_grid() {
    // Specification 10 §4.3 and specification 34 CYCLE-03: the resource samples its tasks on a fixed
    // grid of the cycle interval. Each pass here comes 1 ms after the deadline of the wait before
    // it, as a wake that comes late: the scans run 1 ms after each 10 ms boundary, never later, and
    // the 10 ms task misses no activation in 100 cycles. Catches pacing one interval after the
    // scan's start, which drifts 1 ms per cycle and loses an activation every 10 cycles.
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    for _ in 0..100 {
        let deadline = fixture
            .wait
            .take_deadlines()
            .last()
            .map(|deadline| u64::try_from(deadline.as_millis()).expect("test deadlines are small"))
            .expect("the pass waits");
        fixture.pass_at(&mut runner, deadline + 1);
    }
    let expected: Vec<u128> = core::iter::once(0)
        .chain((1..=100).map(|cycle| cycle * 10 + 1))
        .collect();
    assert_eq!(fixture.take_scan_times(), expected);
    let status = fixture.call(&mut runner, &request(1, "status"));
    assert_eq!(status["result"]["metrics"]["overruns"], json!(0));
}
