//! Status contracts over the fake platform. The I/O health projections moved from trust-runtime
//! (`control/status_handlers/projection_contract_tests.rs`) with the driver health types.

use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

use serde_json::{json, Value};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::value::Duration as EventTime;

use super::*;
use crate::events::RuntimeEvent;
use crate::metrics::{BudgetContributorSnapshot, TaskStatsSnapshot};
use crate::tests::fake_host::FakeHost;
use crate::tests::runner_ports::Fixture;

fn driver(name: &str, health: IoDriverHealth) -> IoDriverStatus {
    IoDriverStatus {
        name: SmolStr::new(name),
        health,
    }
}

fn result(response: ControlResponse) -> Value {
    assert!(response.ok, "request failed: {:?}", response.error);
    response.result.expect("response result")
}

fn error(response: &ControlResponse) -> &str {
    response.error.as_deref().expect("error text")
}

#[test]
fn io_health_ok_has_exact_public_shape() {
    assert_eq!(
        io_health_to_json(&driver("simulated", IoDriverHealth::Ok)),
        json!({"name": "simulated", "status": "ok"})
    );
}

#[test]
fn io_health_degraded_retains_error_detail() {
    assert_eq!(
        io_health_to_json(&driver(
            "mqtt",
            IoDriverHealth::Degraded {
                error: SmolStr::new("broker reconnecting"),
            },
        )),
        json!({
            "name": "mqtt",
            "status": "degraded",
            "error": "broker reconnecting",
        })
    );
}

#[test]
fn io_health_faulted_retains_error_detail() {
    assert_eq!(
        io_health_to_json(&driver(
            "ethercat",
            IoDriverHealth::Faulted {
                error: SmolStr::new("bus lost"),
            },
        )),
        json!({
            "name": "ethercat",
            "status": "faulted",
            "error": "bus lost",
        })
    );
}

#[test]
fn status_reports_identity_modes_settings_and_platform_blocks() {
    let mut host = FakeHost::running();
    host.io_drivers = vec![
        driver("z-last", IoDriverHealth::Ok),
        driver(
            "a-first",
            IoDriverHealth::Degraded {
                error: SmolStr::new("slow"),
            },
        ),
    ];
    let value = result(status(1, &host));
    assert_eq!(value["state"], json!("running"));
    assert_eq!(value["fault"], Value::Null);
    assert_eq!(value["resource"], json!("RESOURCE"));
    assert_eq!(value["plc_name"], json!("RESOURCE"));
    assert_eq!(value["uptime_ms"], json!(0));
    assert_eq!(value["debug_enabled"], json!(true));
    assert_eq!(value["control_mode"], json!("production"));
    assert_eq!(value["debug_hold"], Value::Null);
    assert_eq!(value["execution_backend"], json!("vm"));
    assert_eq!(value["execution_backend_source"], json!("default"));
    assert_eq!(value["simulation_mode"], json!("production"));
    assert_eq!(value["simulation_enabled"], json!(false));
    assert_eq!(value["simulation_time_scale"], json!(1));
    assert_eq!(value["simulation_warning"], json!(""));
    assert_eq!(value["hmi_read_only"], json!(true));
    assert_eq!(value["openot_persistence"], json!({"state": "disabled"}));
    assert_eq!(
        value["realtime"],
        json!({"profile": "disabled", "errors": []})
    );
    assert_eq!(
        value["io_drivers"],
        json!([
            {"name": "z-last", "status": "ok"},
            {"name": "a-first", "status": "degraded", "error": "slow"},
        ]),
        "drivers keep their registration order"
    );
    assert_eq!(
        value["metrics"],
        json!({
            "cycle_ms": {
                "min": 0.0, "avg": 0.0, "max": 0.0, "last": 0.0,
                "p50": 0.0, "p95": 0.0, "p99": 0.0, "window_samples": 0,
            },
            "overruns": 0,
            "faults": 0,
            "profiling": {"enabled": false, "top": []},
            "execution_backend": "vm",
        })
    );
}

#[test]
fn status_reports_the_event_capacity() {
    // Specification 11, the event store: `status` reports the store's capacity as
    // `event_capacity`, 1,000 by default on a computer. Wrong behaviour caught: the capacity is
    // not projected, or another number is reported in its place.
    let mut host = FakeHost::running();
    host.event_capacity = 200;
    assert_eq!(result(status(1, &host))["event_capacity"], json!(200));

    host.event_capacity = 1_000;
    assert_eq!(result(status(2, &host))["event_capacity"], json!(1_000));
}

#[test]
fn status_reports_version_and_platform() {
    let host = FakeHost::running();
    let value = result(status(1, &host));
    assert_eq!(value["version"], json!(env!("CARGO_PKG_VERSION")));
    // The fake host names its platform `test-platform` (tests/fake_host.rs).
    assert_eq!(value["platform"], json!("test-platform"));
}

#[test]
fn status_reports_the_runtime_id() {
    // Specification 11 `status`: the runtime's stable runtime ID. The fake host reports the
    // literal `test-runtime-id` (tests/fake_host.rs).
    let host = FakeHost::running();
    let value = result(status(1, &host));
    assert_eq!(value["runtime_id"], json!("test-runtime-id"));
}

#[test]
fn runner_status_reports_that_it_keeps_control() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let reply = fixture.call_at(&mut runner, 3, &json!({"id": 1, "type": "status"}));
    assert_eq!(reply["result"]["program_control"], json!("keeps_control"));
}

#[test]
fn debugger_hold_reports_paused_unless_faulted_or_stopped() {
    let mut host = FakeHost::running();
    host.debug_hold = Some("breakpoint");
    let value = result(status(1, &host));
    assert_eq!(value["state"], json!("paused"));
    assert_eq!(value["debug_hold"], json!({"reason": "breakpoint"}));

    for (state, label) in [
        (ResourceState::Faulted, "faulted"),
        (ResourceState::Stopped, "stopped"),
    ] {
        host.state.set(state);
        let value = result(status(2, &host));
        assert_eq!(value["state"], json!(label));
        assert_eq!(value["debug_hold"], json!({"reason": "breakpoint"}));
    }
}

#[test]
fn status_fails_closed_naming_the_first_unreadable_value() {
    for (unreadable, message) in [
        ("settings", "runtime settings unavailable"),
        ("io", "I/O health unavailable"),
        ("metrics", "runtime metrics unavailable"),
        ("realtime", "realtime status unavailable"),
        ("control_mode", "control mode unavailable"),
        ("openot", "OpenOT persistence status unavailable"),
        ("event_capacity", "event capacity unavailable"),
    ] {
        let mut host = FakeHost::running();
        host.unreadable = vec![unreadable, "openot"];
        let response = status(3, &host);
        assert!(!response.ok);
        assert_eq!(error(&response), message, "unreadable={unreadable}");
        assert_eq!(response.error_code, None);
    }
}

#[test]
fn health_is_ok_only_for_live_states_without_fault_evidence() {
    for (state, ok) in [
        (ResourceState::Boot, false),
        (ResourceState::Ready, true),
        (ResourceState::Running, true),
        (ResourceState::Paused, true),
        (ResourceState::Faulted, false),
        (ResourceState::Stopped, false),
    ] {
        let host = FakeHost::running();
        host.state.set(state);
        let value = result(health(1, &host));
        assert_eq!(value["ok"], json!(ok), "state={state:?}");
    }

    let mut faulted = FakeHost::running();
    faulted.last_error = Some(RuntimeError::DivisionByZero);
    let value = result(health(2, &faulted));
    assert_eq!(value["ok"], json!(false));
    assert_eq!(
        value["fault"],
        json!(RuntimeError::DivisionByZero.to_string())
    );

    let mut degraded = FakeHost::running();
    degraded.io_drivers = vec![driver(
        "mqtt",
        IoDriverHealth::Degraded {
            error: SmolStr::new("reconnecting"),
        },
    )];
    assert_eq!(result(health(3, &degraded))["ok"], json!(true));
    degraded.io_drivers = vec![driver(
        "fieldbus",
        IoDriverHealth::Faulted {
            error: SmolStr::new("wire break"),
        },
    )];
    let value = result(health(4, &degraded));
    assert_eq!(value["ok"], json!(false));
    assert_eq!(value["io_drivers"][0]["error"], json!("wire break"));

    let mut realtime = FakeHost::running();
    realtime.realtime_errors = true;
    assert_eq!(result(health(5, &realtime))["ok"], json!(false));
}

#[test]
fn health_fails_closed_naming_the_first_unreadable_value() {
    for (unreadable, message) in [
        (vec!["io", "realtime"], "I/O health unavailable"),
        (vec!["realtime"], "realtime status unavailable"),
    ] {
        let mut host = FakeHost::running();
        host.unreadable = unreadable;
        assert_eq!(error(&health(1, &host)), message);
    }
}

#[test]
fn task_stats_sort_tasks_by_name_and_share_the_top_contributors() {
    let mut host = FakeHost::running();
    host.metrics.profiling.enabled = true;
    host.metrics.tasks = ["Zulu", "alpha", "Middle", "Alpha"]
        .into_iter()
        .map(|name| TaskStatsSnapshot {
            name: SmolStr::new(name),
            min_ms: 1.0,
            max_ms: 3.0,
            avg_ms: 2.0,
            last_ms: 3.0,
            overruns: 4,
        })
        .collect();
    host.metrics.profiling.top_contributors = vec![BudgetContributorSnapshot {
        key: SmolStr::new("program:Main"),
        kind: SmolStr::new("program"),
        name: SmolStr::new("Main"),
        avg_cycle_ms: 8.0,
        cycle_pct: 40.0,
        last_ms: 8.0,
        last_cycle_pct: 40.0,
    }];
    let value = result(task_stats(1, &host));
    let names = value["tasks"]
        .as_array()
        .expect("tasks")
        .iter()
        .map(|entry| entry["name"].as_str().expect("task name"))
        .collect::<Vec<_>>();
    assert_eq!(names, ["Alpha", "Middle", "Zulu", "alpha"]);
    assert_eq!(
        value["tasks"][0],
        json!({"name": "Alpha", "min_ms": 1.0, "avg_ms": 2.0, "max_ms": 3.0, "last_ms": 3.0, "overruns": 4})
    );
    assert_eq!(value["profiling_enabled"], json!(true));
    let contributor = json!({
        "key": "program:Main",
        "kind": "program",
        "name": "Main",
        "avg_cycle_ms": 8.0,
        "cycle_pct": 40.0,
        "last_ms": 8.0,
        "last_cycle_pct": 40.0,
    });
    assert_eq!(value["top_contributors"], json!([contributor.clone()]));
    let status = result(status(2, &host));
    assert_eq!(status["metrics"]["profiling"]["top"], json!([contributor]));

    host.unreadable = vec!["metrics"];
    assert_eq!(error(&task_stats(3, &host)), "runtime metrics unavailable");
}

#[test]
fn events_and_faults_read_the_newest_entries_within_their_limit() {
    let host = FakeHost::running();
    host.events
        .borrow_mut()
        .extend((1..=55).map(|cycle| RuntimeEvent::CycleStart {
            cycle,
            time: EventTime::ZERO,
        }));
    host.events.borrow_mut().push_back(RuntimeEvent::fault(
        &RuntimeError::SimulationFault("newest fault".into()),
        EventTime::from_millis(2),
    ));

    let events = result(events_tail(1, None, &host));
    let events = events["events"].as_array().expect("events");
    assert_eq!(events.len(), 50, "the default limit is 50");
    assert_eq!(events[0]["type"], json!("fault"));
    assert_eq!(events[1]["cycle"], json!(55));

    let faults = result(faults(2, Some(&json!({"limit": 1_000})), &host));
    assert_eq!(
        faults,
        json!({"faults": [{"type": "fault", "error": "simulation fault 'newest fault'", "time_ns": 2_000_000}]})
    );
}

#[test]
fn event_reads_refuse_bad_limits_and_an_unreadable_store() {
    let host = FakeHost::running();
    for (params, problem) in [
        (Value::Null, "parameters must be an object"),
        (json!([]), "parameters must be an object"),
        (json!({"count": 2}), "unknown parameter field"),
        (json!({"limit": null}), "parameter fields must not be null"),
        (json!({"limit": true}), "limit must be an integer"),
        (json!({"limit": 2.5}), "limit must be an integer"),
        (json!({"limit": -1}), "limit must be an integer"),
        (json!({"limit": 0}), "limit is outside the supported range"),
        (
            json!({"limit": 1_001}),
            "limit is outside the supported range",
        ),
    ] {
        let expected = alloc::format!("invalid params: {problem}");
        assert_eq!(error(&events_tail(1, Some(&params), &host)), expected);
        assert_eq!(error(&faults(2, Some(&params), &host)), expected);
    }

    let mut unreadable = FakeHost::running();
    unreadable.unreadable = vec!["events"];
    assert_eq!(
        error(&events_tail(3, None, &unreadable)),
        "event store unavailable"
    );
    assert_eq!(
        error(&faults(4, None, &unreadable)),
        "event store unavailable"
    );
}
