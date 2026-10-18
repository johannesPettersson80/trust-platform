//! The `status` blocks of the services a runtime does not run (specification 11, §6.8.1): the
//! `realtime` block of a runtime without a realtime profile and the `openot_persistence` block of
//! one whose `OpenOT` persistence is disabled. Every runtime reports them alike: trust-runtime
//! when its project configures neither, and a chip, which has neither yet.

use serde_json::json;

/// The `realtime` block of `status` for a runtime without a realtime profile: the profile is
/// disabled, the requested posture is the default one, and nothing was observed or applied.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn absent_realtime_profile() -> serde_json::Value {
    json!({
        "profile": "disabled",
        "enabled": false,
        "strict": false,
        "require_preempt_rt_kernel": false,
        "requested": {
            "lock_memory": false,
            "scheduler": "other",
            "priority": 70,
            "cpu_affinity": [],
        },
        "observed": {
            "kernel_realtime": null,
            "scheduler": null,
            "priority": null,
            "cpu_affinity": [],
            "memory_locked_kb": null,
        },
        "runtime_applied": {
            "memory_lock": false,
            "affinity": false,
            "scheduler": false,
        },
        "active": false,
        "warnings": [],
        "errors": [],
    })
}

/// The `openot_persistence` block of `status` for a runtime whose `OpenOT` document persistence
/// is disabled: no backend, nothing read, committed or pending, and no warning.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn disabled_openot_persistence() -> serde_json::Value {
    json!({
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
    })
}

#[cfg(test)]
mod contract_tests;
