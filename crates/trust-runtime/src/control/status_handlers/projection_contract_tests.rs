use std::sync::{Arc, Mutex};
use std::time::Duration as StdDuration;

use serde_json::{json, Value};
use smol_str::SmolStr;

use super::*;
use crate::io::{IoDriverHealth, IoDriverStatus};

const SOURCE: &str = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;

fn state() -> ControlState {
    crate::control::tests::hmi_test_state(SOURCE)
}

fn result(response: ControlResponse) -> Value {
    assert!(response.ok, "request failed: {:?}", response.error);
    response.result.expect("response result")
}

fn assert_unavailable(response: ControlResponse) {
    assert!(!response.ok, "unavailable state must fail closed");
    assert!(
        response
            .error
            .as_deref()
            .is_some_and(|error| error.contains("unavailable")),
        "missing unavailable diagnostic: {:?}",
        response.error
    );
}

fn poison<T>(mutex: &Arc<Mutex<T>>) {
    let mutex = Arc::clone(mutex);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _guard = mutex.lock().expect("lock before poison");
        panic!("intentional contract-test poison");
    }));
    assert!(result.is_err(), "poison setup must panic");
}

fn driver(name: &str, health: IoDriverHealth) -> IoDriverStatus {
    IoDriverStatus {
        name: SmolStr::new(name),
        health,
    }
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
fn status_baseline_preserves_runtime_identity_and_modes() {
    let value = result(handle_status(1, &state()));

    assert_eq!(value.get("state"), Some(&json!("ready")));
    assert_eq!(value.get("resource"), Some(&json!("RESOURCE")));
    assert_eq!(value.get("plc_name"), Some(&json!("RESOURCE")));
    assert_eq!(value.get("control_mode"), Some(&json!("debug")));
    assert_eq!(value.get("execution_backend"), Some(&json!("vm")));
    assert_eq!(
        value.get("execution_backend_source"),
        Some(&json!("default"))
    );
    assert_eq!(value.get("hmi_read_only"), Some(&json!(true)));
    assert_eq!(
        value.get("openot_persistence"),
        Some(&json!({
            "state": "disabled",
            "backend": null,
            "schema_version": null,
            "documents_read": 0,
            "documents_committed": 0,
            "documents_duplicated": 0,
            "remote_pending": 0,
            "projection_rows_committed": 0,
            "unclassified_event_count": 0,
            "reconciled_part_count": 0,
            "pending_part_count": 0,
            "documents_retried": 0,
            "pending": 0,
            "rejected": 0,
            "unresolved": 0,
            "loss_range_count": 0,
            "lost_record_count": 0,
            "cursor_abs": 0,
            "head_abs": 0,
            "cursor_lag": 0,
            "last_success_time_ns": null,
            "last_error": null,
            "warnings": [],
        }))
    );
}

#[test]
fn openot_status_projects_every_applicable_operator_warning() {
    let mut state = state();
    state.openot_persistence_status = Some(std::sync::Arc::new(std::sync::Mutex::new(
        crate::openot_persistence::OpenOtPersistenceStatus {
            state: crate::openot_persistence::OpenOtPersistenceState::Faulted,
            backend: Some("influxdb3".to_string()),
            schema_version: Some(2),
            documents_read: 10,
            documents_committed: 4,
            documents_duplicated: 0,
            remote_pending: 3,
            projection_rows_committed: 17,
            unclassified_event_count: 2,
            reconciled_part_count: 11,
            pending_part_count: 7,
            documents_retried: 2,
            pending: 64,
            rejected: 0,
            unresolved: 1,
            loss_range_count: 1,
            lost_record_count: 5,
            cursor_abs: 32,
            head_abs: 96,
            last_success_time_ns: Some(1),
            last_error: Some("redacted migration or disk failure".to_string()),
        },
    )));

    let value = result(handle_status(1, &state));
    assert_eq!(value["openot_persistence"]["projection_rows_committed"], 17);
    assert_eq!(value["openot_persistence"]["unclassified_event_count"], 2);
    assert_eq!(value["openot_persistence"]["reconciled_part_count"], 11);
    assert_eq!(value["openot_persistence"]["pending_part_count"], 7);
    assert_eq!(
        value["openot_persistence"]["warnings"],
        json!([
            "lag",
            "retrying",
            "placeholder",
            "loss",
            "spool_pressure",
            "migration_or_storage_fault",
            "shutdown_pending"
        ])
    );
}

#[test]
fn status_preserves_io_driver_registration_order() {
    let state = state();
    state.io_health.lock().expect("io health").extend([
        driver("z-last", IoDriverHealth::Ok),
        driver(
            "a-first",
            IoDriverHealth::Degraded {
                error: SmolStr::new("slow"),
            },
        ),
    ]);

    let value = result(handle_status(2, &state));
    let names = value["io_drivers"]
        .as_array()
        .expect("io drivers")
        .iter()
        .map(|entry| entry["name"].as_str().expect("driver name"))
        .collect::<Vec<_>>();
    assert_eq!(names, ["z-last", "a-first"]);
}

#[test]
fn status_projects_recorded_cycle_fault_and_overrun_metrics() {
    let state = state();
    {
        let mut metrics = state.metrics.lock().expect("metrics");
        metrics.record_cycle(StdDuration::from_millis(12));
        metrics.record_overrun(&SmolStr::new("Main"), 3);
        metrics.record_fault();
    }

    let value = result(handle_status(3, &state));
    assert_eq!(value.pointer("/metrics/cycle_ms/last"), Some(&json!(12.0)));
    assert_eq!(
        value.pointer("/metrics/cycle_ms/window_samples"),
        Some(&json!(1))
    );
    assert_eq!(value.pointer("/metrics/overruns"), Some(&json!(3)));
    assert_eq!(value.pointer("/metrics/faults"), Some(&json!(1)));
}

#[test]
fn status_preserves_realtime_warnings_and_errors() {
    let state = state();
    {
        let mut realtime = state.realtime_status.lock().expect("realtime");
        realtime.warnings.push(SmolStr::new("kernel not realtime"));
        realtime.errors.push(SmolStr::new("scheduler rejected"));
    }

    let value = result(handle_status(4, &state));
    assert_eq!(
        value.pointer("/realtime/warnings/0"),
        Some(&json!("kernel not realtime"))
    );
    assert_eq!(
        value.pointer("/realtime/errors/0"),
        Some(&json!("scheduler rejected"))
    );
}

#[test]
fn health_is_true_for_ready_runtime_without_fault_evidence() {
    let value = result(handle_health(5, &state()));

    assert_eq!(value.get("ok"), Some(&json!(true)));
    assert_eq!(value.get("state"), Some(&json!("ready")));
    assert_eq!(value.get("fault"), Some(&Value::Null));
}

#[test]
fn degraded_io_is_visible_but_not_aggregate_failure() {
    let state = state();
    state.io_health.lock().expect("io health").push(driver(
        "mqtt",
        IoDriverHealth::Degraded {
            error: SmolStr::new("reconnecting"),
        },
    ));

    let value = result(handle_health(6, &state));
    assert_eq!(value.get("ok"), Some(&json!(true)));
    assert_eq!(
        value.pointer("/io_drivers/0"),
        Some(&json!({
            "name": "mqtt",
            "status": "degraded",
            "error": "reconnecting",
        }))
    );
}

#[test]
fn realtime_warning_is_visible_but_not_aggregate_failure() {
    let state = state();
    state
        .realtime_status
        .lock()
        .expect("realtime")
        .warnings
        .push(SmolStr::new("best effort"));

    let value = result(handle_health(7, &state));
    assert_eq!(value.get("ok"), Some(&json!(true)));
}

#[test]
fn faulted_io_makes_aggregate_health_false() {
    let state = state();
    state.io_health.lock().expect("io health").push(driver(
        "fieldbus",
        IoDriverHealth::Faulted {
            error: SmolStr::new("wire break"),
        },
    ));

    let value = result(handle_health(8, &state));
    assert_eq!(value.get("ok"), Some(&json!(false)));
    assert_eq!(
        value.pointer("/io_drivers/0/error"),
        Some(&json!("wire break"))
    );
}

#[test]
fn realtime_error_makes_aggregate_health_false() {
    let state = state();
    state
        .realtime_status
        .lock()
        .expect("realtime")
        .errors
        .push(SmolStr::new("affinity mismatch"));

    let value = result(handle_health(9, &state));
    assert_eq!(value.get("ok"), Some(&json!(false)));
}

#[test]
fn status_fails_closed_when_settings_are_unavailable() {
    let state = state();
    poison(&state.settings);
    assert_unavailable(handle_status(10, &state));
}

#[test]
fn status_fails_closed_when_control_mode_is_unavailable() {
    let state = state();
    poison(&state.control_mode);
    assert_unavailable(handle_status(11, &state));
}

#[test]
fn status_fails_closed_when_io_health_is_unavailable() {
    let state = state();
    poison(&state.io_health);
    assert_unavailable(handle_status(12, &state));
}

#[test]
fn status_fails_closed_when_metrics_are_unavailable() {
    let state = state();
    poison(&state.metrics);
    assert_unavailable(handle_status(13, &state));
}

#[test]
fn status_fails_closed_when_realtime_status_is_unavailable() {
    let state = state();
    poison(&state.realtime_status);
    assert_unavailable(handle_status(14, &state));
}

#[test]
fn health_fails_closed_when_io_health_is_unavailable() {
    let state = state();
    poison(&state.io_health);
    assert_unavailable(handle_health(15, &state));
}

#[test]
fn health_fails_closed_when_realtime_status_is_unavailable() {
    let state = state();
    poison(&state.realtime_status);
    assert_unavailable(handle_health(16, &state));
}

#[test]
fn task_stats_fail_closed_when_metrics_are_unavailable() {
    let state = state();
    poison(&state.metrics);
    assert_unavailable(handle_task_stats(17, &state));
}

#[test]
fn task_stats_empty_shape_is_stable() {
    assert_eq!(
        result(handle_task_stats(18, &state())),
        json!({
            "tasks": [],
            "profiling_enabled": true,
            "top_contributors": [],
        })
    );
}

#[test]
fn task_stats_are_sorted_lexically_by_name() {
    let state = state();
    {
        let mut metrics = state.metrics.lock().expect("metrics");
        for name in ["Zulu", "alpha", "Middle", "Alpha"] {
            metrics.record_task(&SmolStr::new(name), StdDuration::from_millis(1));
        }
    }

    let value = result(handle_task_stats(19, &state));
    let names = value["tasks"]
        .as_array()
        .expect("tasks")
        .iter()
        .map(|entry| entry["name"].as_str().expect("task name"))
        .collect::<Vec<_>>();
    assert_eq!(names, ["Alpha", "Middle", "Zulu", "alpha"]);
}

#[test]
fn task_stats_preserve_duration_and_overrun_values() {
    let state = state();
    {
        let mut metrics = state.metrics.lock().expect("metrics");
        metrics.record_task(&SmolStr::new("Main"), StdDuration::from_millis(2));
        metrics.record_task(&SmolStr::new("Main"), StdDuration::from_millis(6));
        metrics.record_overrun(&SmolStr::new("Main"), 4);
    }

    let value = result(handle_task_stats(20, &state));
    assert_eq!(
        value.pointer("/tasks/0"),
        Some(&json!({
            "name": "Main",
            "min_ms": 2.0,
            "avg_ms": 4.0,
            "max_ms": 6.0,
            "last_ms": 6.0,
            "overruns": 4,
        }))
    );
}

#[test]
fn task_stats_preserve_contribution_ranking() {
    let state = state();
    {
        let mut metrics = state.metrics.lock().expect("metrics");
        metrics.record_cycle(StdDuration::from_millis(20));
        metrics.record_call(
            "program",
            &SmolStr::new("Main"),
            StdDuration::from_millis(8),
        );
        metrics.record_call(
            "function_block",
            &SmolStr::new("Pump"),
            StdDuration::from_millis(3),
        );
    }

    let value = result(handle_task_stats(21, &state));
    assert_eq!(
        value.pointer("/top_contributors/0/key"),
        Some(&json!("program:Main"))
    );
    assert_eq!(
        value.pointer("/top_contributors/1/key"),
        Some(&json!("function_block:Pump"))
    );
}

#[test]
fn status_and_task_stats_share_profiling_enabled_state() {
    let state = state();
    state
        .metrics
        .lock()
        .expect("metrics")
        .set_profiling_enabled(false);

    let status = result(handle_status(22, &state));
    let tasks = result(handle_task_stats(23, &state));
    assert_eq!(
        status.pointer("/metrics/profiling/enabled"),
        Some(&json!(false))
    );
    assert_eq!(tasks.get("profiling_enabled"), Some(&json!(false)));
}
