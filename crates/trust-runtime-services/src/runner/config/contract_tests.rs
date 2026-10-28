//! `config.get` and `config.set` on a device without an operating system (specification 11, the
//! runtime configuration contract; specification 34 TARGET-07), with the board probe on the
//! compiled cycle. Every expected value is worked out by hand from the contract: a computer's keys
//! and spellings, the commit before the reply, the scheduler's copies at the program's boundary, and
//! the refusals.

use alloc::string::ToString;
use alloc::vec::Vec;

use serde_json::{json, Value};
use trust_runtime_core::deployment::{ControlMode, HangAction, LogLevel};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::Duration;
use trust_runtime_core::watchdog::{FaultPolicy, RetainMode, WatchdogAction, WatchdogPolicy};

use crate::resource::commands::SchedulerPolicies;
use crate::settings::RuntimeSettings;
use crate::tests::runner_ports::{ms, CycleCall, Fixture, TestPlatform, TestRunner};

/// The settings of the probe's record: `info`, a 10 ms cycle, no cycle watchdog, `halt`, no retained
/// values saved each second, and debug control.
fn record() -> RuntimeSettings {
    RuntimeSettings {
        log_level: LogLevel::Info,
        cycle_interval: Duration::from_millis(10),
        watchdog: WatchdogPolicy::default(),
        hang_action: HangAction::Fault,
        fault_policy: FaultPolicy::Halt,
        retain_mode: RetainMode::None,
        retain_save_interval: Some(Duration::from_millis(1_000)),
        control_mode: ControlMode::Debug,
        debug_enabled: true,
    }
}

fn platform() -> TestPlatform {
    TestPlatform {
        runtime: Some(record()),
        ..TestPlatform::default()
    }
}

/// The loop over `platform` after its first scan, at 0 ms, so that it serves requests at once until
/// its next boundary at 10 ms.
fn started(fixture: &Fixture, platform: TestPlatform) -> TestRunner<'_> {
    let mut runner = fixture.runner_with(platform);
    fixture.pass_at(&mut runner, 0);
    let _ = fixture.take_calls();
    runner
}

fn get(fixture: &Fixture, runner: &mut TestRunner<'_>, id: u64) -> Value {
    fixture.call(runner, &json!({"id": id, "type": "config.get"}))
}

fn set(fixture: &Fixture, runner: &mut TestRunner<'_>, id: u64, params: &Value) -> Value {
    fixture.call(
        runner,
        &json!({"id": id, "type": "config.set", "params": params}),
    )
}

/// The record's settings in a computer's keys and spellings, with the token's presence and no
/// key of a computer's own services (`web.*`, `mesh.*`, ...), which a chip does not run. Catches a
/// key a chip names otherwise than a computer and a key of a service the chip does not have.
#[test]
fn config_get_reports_the_records_settings_in_a_computers_keys() {
    let fixture = Fixture::board_probe();
    let mut runner = started(&fixture, platform());
    let reply = get(&fixture, &mut runner, 1);
    assert_eq!(
        reply["result"],
        json!({
            "log.level": "info",
            "watchdog.enabled": false,
            "watchdog.timeout_ms": 0,
            "watchdog.action": "safe_halt",
            "watchdog.hang_action": "fault",
            "resource.cycle_interval_ms": 10,
            "runtime.execution_backend": "native",
            "runtime.execution_backend_source": "default",
            "fault.policy": "halt",
            "retain.mode": "none",
            "retain.save_interval_ms": 1000,
            "control.auth_token_set": false,
            "control.auth_token_length": null,
            "control.debug_enabled": true,
            "control.mode": "debug",
        })
    );
}

/// A `config.set` changes the settings before its reply: `config.get` at once reports them. The
/// program takes the scheduler's copies at its next boundary, 10 ms, before its scan, which runs
/// with the new watchdog's deadline, 10 + 50 = 60 ms. Catches settings reported before they are
/// committed, copies taken in the middle of the pass that served the request, and copies never
/// given to the program.
#[test]
fn config_set_commits_at_once_and_the_program_takes_the_policies_at_its_boundary() {
    let fixture = Fixture::board_probe();
    let mut runner = started(&fixture, platform());
    fixture.clock.set(ms(2));
    let reply = set(
        &fixture,
        &mut runner,
        1,
        &json!({
            "fault.policy": "restart",
            "log.level": "debug",
            "watchdog.enabled": true,
            "watchdog.timeout_ms": 50,
        }),
    );
    assert_eq!(
        reply["result"],
        json!({
            "updated": ["fault.policy", "log.level", "watchdog.enabled", "watchdog.timeout_ms"],
            "restart_required": [],
        })
    );
    let now = get(&fixture, &mut runner, 2)["result"].clone();
    assert_eq!(now["log.level"], json!("debug"));
    assert_eq!(now["fault.policy"], json!("restart"));
    assert_eq!(now["watchdog.enabled"], json!(true));
    assert_eq!(now["watchdog.timeout_ms"], json!(50));
    assert_eq!(
        fixture.take_calls(),
        Vec::<CycleCall>::new(),
        "the program takes nothing while the loop serves requests"
    );
    fixture.pass_at(&mut runner, 10);
    assert_eq!(
        fixture.take_calls(),
        [
            CycleCall::Policies(SchedulerPolicies {
                watchdog: WatchdogPolicy {
                    enabled: true,
                    timeout: Duration::from_millis(50),
                    action: WatchdogAction::SafeHalt,
                },
                fault_policy: FaultPolicy::Restart,
                retain_save_interval: Some(Duration::from_millis(1_000)),
            }),
            CycleCall::Scan {
                now: ms(10),
                deadline: Some(ms(60)),
            },
        ]
    );
}

/// A refused request changes nothing: `log.level` staged before an unknown key is not kept, and a
/// key of a computer's own service is unknown to a chip. A request that changes no scheduler copy
/// gives the program none. Catches a partial commit and a policy sent for a log level.
#[test]
fn a_refused_config_set_changes_nothing() {
    let fixture = Fixture::board_probe();
    let mut runner = started(&fixture, platform());
    let refused = set(
        &fixture,
        &mut runner,
        1,
        &json!({"log.level": "debug", "web.enabled": true}),
    );
    assert_eq!(refused["error"], json!("unknown config key 'web.enabled'"));
    let refused = set(&fixture, &mut runner, 2, &json!({"watchdog.timeout_ms": 0}));
    assert_eq!(
        refused["error"],
        json!("invalid config value for 'watchdog.timeout_ms': must be >= 1")
    );
    let refused = fixture.call(&mut runner, &json!({"id": 3, "type": "config.set"}));
    assert_eq!(refused["error"], json!("missing params"));
    assert_eq!(
        get(&fixture, &mut runner, 4)["result"]["log.level"],
        json!("info")
    );
    let _ = set(&fixture, &mut runner, 5, &json!({"log.level": "debug"}));
    fixture.pass_at(&mut runner, 10);
    assert!(
        !fixture
            .take_calls()
            .iter()
            .any(|call| matches!(call, CycleCall::Policies(_))),
        "a log level gives the program no scheduler copies"
    );
}

/// The hardware watchdog's interval follows the cycle watchdog's timeout by the larger of 4 s and
/// the timeout plus 2 s (specification 34 TIME-04). A watchdog that takes at most 10,000 ms takes
/// 8,000 ms (10,000) and refuses 9,000 ms (11,000). Catches a timeout committed that would leave the
/// hardware watchdog shorter than a scan the cycle watchdog allows.
#[test]
fn a_timeout_the_hardware_watchdog_does_not_take_is_refused() {
    let fixture = Fixture::board_probe();
    let mut runner = started(
        &fixture,
        TestPlatform {
            max_watchdog_ms: 10_000,
            ..platform()
        },
    );
    let refused = set(
        &fixture,
        &mut runner,
        1,
        &json!({"watchdog.timeout_ms": 9_000}),
    );
    assert_eq!(
        refused["error"],
        json!(
            "invalid config value for 'watchdog.timeout_ms': the device's hardware watchdog does \
             not take the interval this timeout needs"
        )
    );
    let taken = set(
        &fixture,
        &mut runner,
        2,
        &json!({"watchdog.timeout_ms": 8_000}),
    );
    assert_eq!(taken["ok"], json!(true));
}

/// A control token that `config.set` gives the chip is required from then on, as on a computer's
/// local socket with a token: a request without it is refused, one with it is served, and
/// `config.get` reports its presence and length (6) but never the token. Catches a token stored
/// but not enforced, and a token written out.
#[test]
fn a_token_that_config_set_gives_is_required_from_then_on() {
    let fixture = Fixture::board_probe();
    let mut runner = started(&fixture, platform());
    let reply = set(
        &fixture,
        &mut runner,
        1,
        &json!({"control.auth_token": " s3cret "}),
    );
    assert_eq!(
        reply["result"],
        json!({"updated": ["control.auth_token"], "restart_required": []})
    );
    let refused = fixture.call(&mut runner, &json!({"id": 2, "type": "status"}));
    assert_eq!(refused["error_code"], json!("missing_auth_token"));
    let served = fixture.call(
        &mut runner,
        &json!({"id": 3, "type": "config.get", "auth": "s3cret"}),
    );
    assert_eq!(served["result"]["control.auth_token_set"], json!(true));
    assert_eq!(served["result"]["control.auth_token_length"], json!(6));
    assert!(!served.to_string().contains("s3cret"));
}

/// A program that has ended takes no scheduler copies: a `config.set` of one is refused with the
/// state, as a computer's closed scheduler refuses it, and a log level is still taken. A faulted
/// program takes them at its faulted wait's poll and stays faulted. Catches copies queued for a
/// program that is gone, and copies a faulted program never takes.
#[test]
fn an_ended_program_refuses_policies_and_a_faulted_one_takes_them_at_its_poll() {
    let fixture = Fixture::board_probe();
    fixture
        .script
        .borrow_mut()
        .failing_scans
        .push_back(RuntimeError::DivisionByZero);
    let mut runner = fixture.runner_with(platform());
    fixture.pass_at(&mut runner, 0);
    assert_eq!(runner.resource_state(), ResourceState::Faulted);
    let _ = fixture.take_calls();
    fixture.clock.set(ms(2));
    let reply = set(
        &fixture,
        &mut runner,
        1,
        &json!({"fault.policy": "safe_halt"}),
    );
    assert_eq!(reply["ok"], json!(true));
    fixture.pass_at(&mut runner, 10);
    assert_eq!(
        fixture.take_calls(),
        [CycleCall::Policies(SchedulerPolicies {
            watchdog: WatchdogPolicy::default(),
            fault_policy: FaultPolicy::SafeHalt,
            retain_save_interval: Some(Duration::from_millis(1_000)),
        })]
    );
    assert_eq!(runner.resource_state(), ResourceState::Faulted);

    let _ = fixture.call_at(&mut runner, 11, &json!({"id": 2, "type": "shutdown"}));
    fixture.pass_at(&mut runner, 20);
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    let refused = set(&fixture, &mut runner, 3, &json!({"fault.policy": "halt"}));
    assert_eq!(refused["error"], json!("resource is stopped"));
    assert_eq!(refused["error_code"], json!("resource_not_running"));
    let taken = set(&fixture, &mut runner, 4, &json!({"log.level": "trace"}));
    assert_eq!(taken["ok"], json!(true));
}

/// A chip without a program has no record to report: both requests are refused with the reason, as
/// `restart` is. Catches settings invented for a chip that has none.
#[test]
fn a_chip_without_a_program_refuses_config_requests_with_the_reason() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.without_program("no container is installed");
    for (id, kind) in [(1, "config.get"), (2, "config.set")] {
        let reply = fixture.call(
            &mut runner,
            &json!({"id": id, "type": kind, "params": {"log.level": "debug"}}),
        );
        assert_eq!(
            reply["error"],
            json!("no installed program: no container is installed"),
            "{kind}"
        );
        assert_eq!(reply["error_code"], json!("resource_not_running"), "{kind}");
    }
}
