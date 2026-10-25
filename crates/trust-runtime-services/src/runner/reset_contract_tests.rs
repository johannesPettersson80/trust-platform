//! A device's boot after a reset that ended its program (specification 34 TARGET-08; specification
//! 11, hosted hang supervision): the program a boot starts after the hardware watchdog's reset stays
//! faulted with `WatchdogTimeout` when `runtime.watchdog.hang_action` is `fault` and runs when it is
//! `abort`, and after a panic it stays faulted with `ResourcePanic` and the panic's location, as
//! trust-runtime's resource stays faulted after a scan that did not return or panicked. Every
//! expected value is worked out by hand from those rules.

use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

use serde_json::{json, Value};
use smol_str::SmolStr;
use trust_runtime_core::deployment::HangAction;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value;
use trust_runtime_core::watchdog::{FaultPolicy, WatchdogAction, WatchdogPolicy};

use super::RetainSource;
use crate::capabilities::{LastReset, ResetCauseName};
use crate::tests::runner_ports::{ms, CycleCall, Fixture, TestPlatform, TestRunner};

/// The location a chip's panic path keeps in its reset record.
const PANIC_AT: &str = "src/main.rs:42";

fn request(id: u64, kind: &str) -> Value {
    json!({"id": id, "type": kind})
}

fn latch(error: &RuntimeError) -> CycleCall {
    CycleCall::Latch(error.to_string())
}

fn panic_error() -> RuntimeError {
    RuntimeError::ResourcePanic(SmolStr::new_static(PANIC_AT))
}

/// A platform whose boot read a reset of `cause`, with `text`.
fn after(cause: ResetCauseName, text: &str) -> TestPlatform {
    TestPlatform {
        last_reset: Some(LastReset {
            cause,
            text: SmolStr::new(text),
            boot_count: 7,
        }),
        ..TestPlatform::default()
    }
}

/// The loop of a device that has just booted on `platform`, its program started, with the boot's
/// rule applied, and what it asked of the program since the start.
fn booted(fixture: &Fixture, platform: TestPlatform) -> (TestRunner<'_>, Vec<CycleCall>) {
    let mut runner = fixture.runner_with(platform);
    runner.fault_after_reset();
    let calls = fixture.take_calls();
    (runner, calls)
}

/// The hardware watchdog's reset, which follows a pass that did not return, with the default
/// `hang_action` `fault` and the default watchdog action `safe_halt`: the cycle latches `watchdog
/// timeout`, the safe outputs follow, the fault is logged at time zero, no scan runs, and a warm
/// `restart` at 11 ms is taken at the faulted wait's poll at 20 ms, which scans at once. Catches a
/// chip that runs its program again after a hang, as it did, one that faults without the safe
/// outputs or the fault log, and one that a `restart` cannot recover.
#[test]
fn a_boot_after_a_watchdog_reset_stays_faulted_until_a_restart() {
    let fixture = Fixture::board_probe();
    let (mut runner, calls) = booted(&fixture, after(ResetCauseName::Watchdog, ""));
    assert_eq!(
        calls,
        [
            latch(&RuntimeError::WatchdogTimeout),
            CycleCall::SafeOutputs
        ]
    );
    assert_eq!(runner.resource_state(), ResourceState::Faulted);
    assert_eq!(runner.last_error(), Some(RuntimeError::WatchdogTimeout));

    fixture.clock.set(ms(1));
    let status = fixture.call(&mut runner, &request(1, "status"));
    assert_eq!(status["result"]["state"], json!("faulted"));
    assert_eq!(status["result"]["fault"], json!("watchdog timeout"));
    let faults = fixture.call(&mut runner, &request(2, "faults"));
    assert_eq!(
        faults["result"]["faults"],
        json!([{"type": "fault", "error": "watchdog timeout", "time_ns": 0}])
    );
    fixture.pass_at(&mut runner, 10);
    assert_eq!(fixture.take_calls(), [] as [CycleCall; 0], "no scan runs");

    let reply = fixture.call_at(
        &mut runner,
        11,
        &json!({"id": 3, "type": "restart", "params": {"mode": "warm"}}),
    );
    assert_eq!(reply["result"], json!({"status": "restart queued"}));
    fixture.pass_at(&mut runner, 20);
    assert_eq!(
        fixture.take_calls(),
        [
            CycleCall::Restart {
                mode: RestartMode::Warm,
                source: RetainSource::Checkpoint,
            },
            CycleCall::TaskTiming(ms(20)),
            CycleCall::Scan {
                now: ms(20),
                deadline: None,
            },
        ]
    );
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(runner.last_error(), None);
}

/// The safe outputs after a hang follow the watchdog's action, as for an overrun: `halt` drives
/// them, `restart` does not; and the watchdog's `restart` does not restart a program after a hang,
/// which only the hang action decides, as trust-runtime's supervisor faults the resource whatever
/// the watchdog's action. Catches a hang treated as an overrun that restarts.
#[test]
fn after_a_watchdog_reset_the_safe_outputs_follow_the_watchdog_action() {
    for (action, expected) in [
        (
            WatchdogAction::Halt,
            vec![
                latch(&RuntimeError::WatchdogTimeout),
                CycleCall::SafeOutputs,
            ],
        ),
        (
            WatchdogAction::Restart,
            vec![latch(&RuntimeError::WatchdogTimeout)],
        ),
    ] {
        let fixture = Fixture::board_probe();
        fixture.script.borrow_mut().watchdog = WatchdogPolicy {
            enabled: true,
            timeout: value::Duration::from_millis(5),
            action,
        };
        let (mut runner, calls) = booted(&fixture, after(ResetCauseName::Watchdog, ""));
        assert_eq!(calls, expected, "{action:?}");
        for at in [10, 20, 30] {
            fixture.pass_at(&mut runner, at);
        }
        assert_eq!(fixture.take_scan_times(), [] as [u128; 0], "{action:?}");
        assert_eq!(
            runner.resource_state(),
            ResourceState::Faulted,
            "{action:?}"
        );
    }
}

/// `hang_action` `abort` asks for the program to run again, as trust-runtime's process exits for
/// its service manager to start it: the boot leaves the program running, with no fault, and it
/// scans at once. Catches `abort` ignored.
#[test]
fn with_hang_action_abort_a_watchdog_reset_runs_the_program() {
    let fixture = Fixture::board_probe();
    fixture.script.borrow_mut().hang_action = HangAction::Abort;
    let (mut runner, calls) = booted(&fixture, after(ResetCauseName::Watchdog, ""));
    assert_eq!(calls, [] as [CycleCall; 0]);
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(runner.last_error(), None);
    fixture.pass_at(&mut runner, 0);
    assert_eq!(fixture.take_scan_times(), [0]);
}

/// A panic ends the boot with its location in the reset record: the program stays faulted with
/// `resource panic 'src/main.rs:42'`, and the safe outputs follow the fault policy as for a scan's
/// panic on trust-runtime: `safe_halt` drives them, `halt` and `restart` do not. A panic is never
/// restarted automatically, so `restart` too stays faulted. Catches a chip that runs again after a
/// panic, one that drops the location, and a panic restarted by the fault policy.
#[test]
fn a_boot_after_a_panic_stays_faulted_with_the_panic_location() {
    for (policy, expected) in [
        (FaultPolicy::Halt, vec![latch(&panic_error())]),
        (
            FaultPolicy::SafeHalt,
            vec![latch(&panic_error()), CycleCall::SafeOutputs],
        ),
        (FaultPolicy::Restart, vec![latch(&panic_error())]),
    ] {
        let fixture = Fixture::board_probe();
        fixture.script.borrow_mut().fault_policy = policy;
        let (mut runner, calls) = booted(&fixture, after(ResetCauseName::Panic, PANIC_AT));
        assert_eq!(calls, expected, "{policy:?}");
        assert_eq!(runner.last_error(), Some(panic_error()), "{policy:?}");
        fixture.clock.set(ms(1));
        let status = fixture.call(&mut runner, &request(1, "status"));
        assert_eq!(
            status["result"]["fault"],
            json!("resource panic 'src/main.rs:42'"),
            "{policy:?}"
        );
        for at in [10, 20, 30] {
            fixture.pass_at(&mut runner, at);
        }
        assert_eq!(fixture.take_scan_times(), [] as [u128; 0], "{policy:?}");
        assert_eq!(
            runner.resource_state(),
            ResourceState::Faulted,
            "{policy:?}"
        );
    }
}

/// A power-on, a host reset or a requested restart ended no program, and a computer reports no
/// last reset: the program runs and scans at once. Catches a fault made of a reset that is no
/// fault.
#[test]
fn a_boot_after_a_power_on_or_a_requested_reset_runs_the_program() {
    for platform in [
        after(ResetCauseName::PowerOn, ""),
        after(ResetCauseName::Requested, ""),
        TestPlatform::default(),
    ] {
        let cause = platform.last_reset.as_ref().map(|reset| reset.cause);
        let fixture = Fixture::board_probe();
        let (mut runner, calls) = booted(&fixture, platform);
        assert_eq!(calls, [] as [CycleCall; 0], "{cause:?}");
        assert_eq!(runner.resource_state(), ResourceState::Running, "{cause:?}");
        fixture.pass_at(&mut runner, 0);
        assert_eq!(fixture.take_scan_times(), [0], "{cause:?}");
    }
}
