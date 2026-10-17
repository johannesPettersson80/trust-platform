//! The host's status surface. The `status`, `health`, `tasks.stats`, `events.tail` and `faults`
//! operations are shared runtime code (spec 34 ARCH-11, `trust_runtime_services::status`); this
//! module projects the host-only blocks they report (the Linux realtime posture and the `OpenOT`
//! persistence state) and answers the historian operations.

use crate::linux_rt::LinuxRtRuntimeStatus;
use serde_json::{json, Value};
use trust_runtime_services::status::{invalid_params, validate_object_fields};

use super::types::{HistorianAlertsParams, HistorianQueryParams};
use super::{ControlResponse, ControlState};

#[cfg(test)]
#[path = "status_handlers/projection_contract_tests.rs"]
mod projection_contract_tests;

#[cfg(test)]
#[path = "status_handlers/event_contract_tests.rs"]
mod event_contract_tests;

#[cfg(test)]
#[path = "status_handlers/historian_contract_tests.rs"]
mod historian_contract_tests;

// The in-crate status contract tests call the shared handlers through these wrappers.
#[cfg(test)]
pub(super) fn handle_status(id: u64, state: &ControlState) -> ControlResponse {
    trust_runtime_services::status::status(id, state)
}

#[cfg(test)]
pub(super) fn handle_health(id: u64, state: &ControlState) -> ControlResponse {
    trust_runtime_services::status::health(id, state)
}

#[cfg(test)]
pub(super) fn handle_task_stats(id: u64, state: &ControlState) -> ControlResponse {
    trust_runtime_services::status::task_stats(id, state)
}

#[cfg(test)]
pub(super) fn handle_events_tail(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    trust_runtime_services::status::events_tail(id, params.as_ref(), state)
}

#[cfg(test)]
pub(super) fn handle_faults(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    trust_runtime_services::status::faults(id, params.as_ref(), state)
}

/// The `realtime` block of `status`: the requested and observed Linux realtime posture.
pub(super) fn realtime_status_json(realtime: &LinuxRtRuntimeStatus) -> Value {
    json!({
        "profile": realtime.requested.profile_name(),
        "enabled": realtime.requested.enabled,
        "strict": realtime.requested.strict,
        "require_preempt_rt_kernel": realtime.requested.require_preempt_rt_kernel,
        "requested": {
            "lock_memory": realtime.requested.lock_memory,
            "scheduler": realtime.requested.scheduler.as_str(),
            "priority": realtime.requested.priority,
            "cpu_affinity": realtime.requested.cpu_affinity,
        },
        "observed": {
            "kernel_realtime": realtime.kernel_realtime,
            "scheduler": realtime.active_scheduler.map(|value| value.as_str()),
            "priority": realtime.active_priority,
            "cpu_affinity": realtime.active_cpu_affinity,
            "memory_locked_kb": realtime.memory_locked_kb,
        },
        "runtime_applied": {
            "memory_lock": realtime.memory_lock_applied,
            "affinity": realtime.affinity_applied_by_runtime,
            "scheduler": realtime.scheduler_applied_by_runtime,
        },
        "active": realtime.active,
        "warnings": realtime
            .warnings
            .iter()
            .map(|value| value.as_str())
            .collect::<Vec<_>>(),
        "errors": realtime
            .errors
            .iter()
            .map(|value| value.as_str())
            .collect::<Vec<_>>(),
    })
}

/// The `openot_persistence` block of `status`, or the name of the status that cannot be read.
pub(super) fn openot_persistence_status_json(state: &ControlState) -> Result<Value, &'static str> {
    Ok(match state.openot_persistence_status.as_ref() {
        Some(status) => {
            let status = status.lock().map_err(|_| "OpenOT persistence status")?;
            let mut warnings = Vec::new();
            if status.head_abs > status.cursor_abs {
                warnings.push("lag");
            }
            if status.documents_retried > 0 {
                warnings.push("retrying");
            }
            if status.unresolved > 0 {
                warnings.push("placeholder");
            }
            if status.loss_range_count > 0 {
                warnings.push("loss");
            }
            if status.remote_pending > 0 {
                warnings.push("spool_pressure");
            }
            if status.state == crate::openot_persistence::OpenOtPersistenceState::Faulted
                && status.last_error.is_some()
            {
                warnings.push("schema_or_storage_fault");
            }
            if matches!(
                status.state,
                crate::openot_persistence::OpenOtPersistenceState::Stopped
                    | crate::openot_persistence::OpenOtPersistenceState::Faulted
            ) && (status.pending > 0 || status.remote_pending > 0)
            {
                warnings.push("shutdown_pending");
            }
            json!({
                "state": status.state.as_str(),
                "backend": status.backend,
                "schema_generation": status.schema_version,
                "documents_read": status.documents_read,
                "documents_committed": status.documents_committed,
                "documents_duplicated": status.documents_duplicated,
                "remote_pending": status.remote_pending,
                "projection_rows_committed": status.projection_rows_committed,
                "unclassified_event_count": status.unclassified_event_count,
                "reconciled_part_count": status.reconciled_part_count,
                "pending_part_count": status.pending_part_count,
                "documents_retried": status.documents_retried,
                "pending": status.pending,
                "rejected": status.rejected,
                "unresolved": status.unresolved,
                "loss_range_count": status.loss_range_count,
                "lost_record_count": status.lost_record_count,
                "cursor_abs": status.cursor_abs,
                "head_abs": status.head_abs,
                "cursor_lag": status.head_abs.saturating_sub(status.cursor_abs),
                "last_success_time_ns": status.last_success_time_ns,
                "last_error": status.last_error,
                "warnings": warnings,
            })
        }
        None => json!({
            "state": "disabled",
            "backend": null,
            "schema_generation": null,
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
        }),
    })
}

pub(super) fn handle_historian_query(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let Some(historian) = state.historian.as_ref() else {
        return ControlResponse::error(id, "historian disabled".into());
    };
    let params = match params {
        Some(value) => {
            if let Err(error) = validate_object_fields(
                &value,
                &["variable", "since_ms", "limit"],
                &["variable", "since_ms", "limit"],
            ) {
                return invalid_params(id, error);
            }
            match serde_json::from_value::<HistorianQueryParams>(value) {
                Ok(parsed) => parsed,
                Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
            }
        }
        None => HistorianQueryParams::default(),
    };
    let mut params = params;
    if let Some(variable) = params.variable.as_mut() {
        *variable = variable.trim().to_string();
        if variable.is_empty() {
            return invalid_params(id, "variable must not be blank");
        }
    }
    if params
        .limit
        .is_some_and(|limit| !(1..=5_000).contains(&limit))
    {
        return invalid_params(id, "limit must be in 1..=5000");
    }
    let items = historian.query(
        params.variable.as_deref(),
        params.since_ms,
        params.limit.unwrap_or(250),
    );
    ControlResponse::ok(id, json!({ "items": items }))
}

pub(super) fn handle_historian_alerts(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let Some(historian) = state.historian.as_ref() else {
        return ControlResponse::error(id, "historian disabled".into());
    };
    let params = match params {
        Some(value) => {
            if let Err(error) = validate_object_fields(&value, &["limit"], &["limit"]) {
                return invalid_params(id, error);
            }
            match serde_json::from_value::<HistorianAlertsParams>(value) {
                Ok(parsed) => parsed,
                Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
            }
        }
        None => HistorianAlertsParams::default(),
    };
    if params
        .limit
        .is_some_and(|limit| !(1..=1_000).contains(&limit))
    {
        return invalid_params(id, "limit must be in 1..=1000");
    }
    let items = historian.alerts(params.limit.unwrap_or(200));
    ControlResponse::ok(id, json!({ "items": items }))
}
