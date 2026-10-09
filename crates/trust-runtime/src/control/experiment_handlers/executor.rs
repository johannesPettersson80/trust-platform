//! Runs experiments from the runtime's own scan (Specification 35, A3.5 and A6.5). Each published scan reaches
//! `ControlExperimentService::observe` once, after the scan's tasks ran and before the next scan applies its writes, so a
//! write queued here takes effect at the task's next execution, whatever the time scale.

use trust_hir::TypeId;

use super::values::{numeric_value, snapshot_type, snapshot_value};
use super::{
    finish_record, settle_at_boundary, AppliedPoint, ControlExperimentService, ExperimentRecord,
    ExperimentStatus, BACKGROUND_TASK,
};
use crate::debug::{DebugControl, ScanObserver, ScanView};
use crate::value::Value;

/// Executions of the task that may pass after the restoring write before the restoration is failed (Specification 35, A7).
const RESTORATION_CONFIRMATION_EXECUTIONS: u32 = 3;

impl ControlExperimentService {
    /// The observer that drives this service from every published scan; `ControlState::connect_experiments` registers it.
    pub(crate) fn scan_observer(&self) -> ScanObserver {
        let service = self.clone();
        ScanObserver::new(move |debug, view| service.observe(debug, view))
    }

    /// Advances the active experiment by one scan. An experiment advances only on executions of its task (every scan for
    /// the background programs), on the runtime's process time; nothing is measured on the wall clock.
    pub(crate) fn observe(&self, debug: &DebugControl, view: &ScanView<'_>) {
        self.record_simulated_target(view.simulated_target);
        let Ok(mut active) = self.active.lock() else {
            return;
        };
        let Some(record) = active.as_mut() else {
            return;
        };
        // Restoration counts only executions after the restoring write, so a stop made in this scan does not count.
        let awaiting_restoration =
            record.status != ExperimentStatus::Running && record.restoration == "pending";
        settle_at_boundary(record, debug, view.io.scan);
        if !runs_in(record, view) {
            return;
        }
        let origin = *record.origin.get_or_insert(view.time);
        record.elapsed = view.time.saturating_sub(origin);
        if record.status == ExperimentStatus::Running {
            execute(record, debug, view);
        } else if awaiting_restoration {
            confirm_restoration(record, view);
        }
    }
}

/// Whether the scan executed the experiment's task; the background programs execute in every scan.
fn runs_in(record: &ExperimentRecord, view: &ScanView<'_>) -> bool {
    record.task.as_str() == BACKGROUND_TASK
        || view
            .tasks
            .iter()
            .any(|(name, _)| name.as_str() == record.task.as_str())
}

/// One execution of a running experiment: its checks, then the writes that this execution decides.
fn execute(record: &mut ExperimentRecord, debug: &DebugControl, view: &ScanView<'_>) {
    // A2.4: nothing is written to a physical target without the engineer's approval. The target's class is the runtime's
    // fact, so a target that turns physical after the start stops the experiment here.
    if record.write_enabled && !view.simulated_target && !record.physically_approved {
        finish_record(
            record,
            debug,
            view.io.scan,
            ExperimentStatus::Failed,
            "physical_approval_required",
        );
        return;
    }
    confirm_applied_point(record, view);
    match record
        .mode
        .as_ref()
        .map(|mode| mode.holds_required_mode(view.storage))
    {
        Some(Ok(false)) => {
            finish_record(
                record,
                debug,
                view.io.scan,
                ExperimentStatus::Failed,
                "mode_changed",
            );
            return;
        }
        Some(Err(reason)) => {
            finish_record(
                record,
                debug,
                view.io.scan,
                ExperimentStatus::Failed,
                reason,
            );
            return;
        }
        Some(Ok(true)) | None => {}
    }
    // The legacy plans end on their duration before the guards, as they always did; a reviewed abort policy checks its
    // faults first.
    let completing = record.elapsed.saturating_add(record.interval) >= record.duration;
    if completing && record.abort_binding.is_none() {
        finish_record(
            record,
            debug,
            view.io.scan,
            ExperimentStatus::Completed,
            "duration_complete",
        );
        return;
    }
    if record.target.forced(debug, view.io) {
        finish_record(
            record,
            debug,
            view.io.scan,
            ExperimentStatus::Failed,
            "signal_forced",
        );
        return;
    }
    let current = record
        .target
        .observed(view.io, view.storage)
        .as_ref()
        .and_then(numeric_value)
        .filter(|value| value.is_finite());
    let Some(current) = current else {
        finish_record(
            record,
            debug,
            view.io.scan,
            ExperimentStatus::Failed,
            "bad_runtime_data",
        );
        return;
    };
    if current < record.minimum || current > record.maximum {
        finish_record(
            record,
            debug,
            view.io.scan,
            ExperimentStatus::Failed,
            "signal_outside_limits",
        );
        return;
    }
    if let Some(reason) = abort_fault(record, view) {
        finish_record(
            record,
            debug,
            view.io.scan,
            ExperimentStatus::Failed,
            reason,
        );
        return;
    }
    if completing {
        finish_record(
            record,
            debug,
            view.io.scan,
            ExperimentStatus::Completed,
            "duration_complete",
        );
        return;
    }
    // The trajectory's first point is queued before the relay decides: a relay that switches on the first execution
    // must have the last word.
    if record.write_enabled {
        queue_due_point(record, debug);
    }
    if let Some(reason) = switch_relay(record, debug, view) {
        finish_record(
            record,
            debug,
            view.io.scan,
            ExperimentStatus::Failed,
            reason,
        );
    }
}

/// Queues `value` for the next scan boundary and records it as the last value queued.
fn queue(record: &mut ExperimentRecord, debug: &DebugControl, value: Value) {
    record.target.write(debug, value.clone(), record.epoch);
    record.queued = Some(value);
    record.queued_point = None;
}

/// Queues the trajectory value due at the next execution: the latest point whose time is at or before the next execution
/// (A3.5). A value equal to the last one queued is not written again.
fn queue_due_point(record: &mut ExperimentRecord, debug: &DebugControl) {
    let horizon = record.elapsed.saturating_add(record.interval);
    let mut due = None;
    while record.next_point < record.trajectory.len()
        && record.trajectory[record.next_point].0 <= horizon
    {
        due = Some(record.next_point);
        record.next_point += 1;
    }
    let Some(index) = due else {
        return;
    };
    let value = record.trajectory[index].1.clone();
    if record.queued.as_ref() != Some(&value) {
        queue(record, debug, value);
    }
    record.queued_point = Some(index);
}

/// Reads back the trajectory point queued at the previous execution; it is applied once this execution has run.
fn confirm_applied_point(record: &mut ExperimentRecord, view: &ScanView<'_>) {
    let Some(index) = record.queued_point else {
        return;
    };
    let (at, value) = record.trajectory[index].clone();
    if record.target.observed(view.io, view.storage).as_ref() == Some(&value) {
        record.last_applied = Some(AppliedPoint {
            index,
            at,
            value,
            scan: view.io.scan,
            process_time: view.time,
        });
        record.queued_point = None;
    }
}

/// The fault the reviewed project abort binding reports for this scan, if any.
fn abort_fault(record: &ExperimentRecord, view: &ScanView<'_>) -> Option<&'static str> {
    let abort = record.abort_binding.as_ref()?;
    // A project abort binding exists only with an addressed target.
    let Some(address) = record.target.address() else {
        return Some("bad_runtime_data");
    };
    if snapshot_type(view.io, address) != Some(TypeId::BOOL) {
        return Some("bad_runtime_data");
    }
    match abort.asserted(view.io) {
        Ok(false) => None,
        Ok(true) => Some("project_abort_asserted"),
        Err(reason) => Some(reason),
    }
}

/// Decides the relay's next level from this execution's measurement, and queues the level when it changes (A3.5).
/// Returns the reason the relay cannot decide, when it cannot.
fn switch_relay(
    record: &mut ExperimentRecord,
    debug: &DebugControl,
    view: &ScanView<'_>,
) -> Option<&'static str> {
    let relay = record.relay_feedback.as_ref()?;
    if view.io.forced.iter().any(|forced| forced == &relay.address) {
        return Some("relay_feedback_forced");
    }
    let measured = (snapshot_type(view.io, &relay.address) == Some(relay.value_type))
        .then(|| snapshot_value(view.io, &relay.address))
        .flatten()
        .and_then(numeric_value)
        .filter(|value| value.is_finite());
    let Some(measured) = measured else {
        return Some("bad_relay_feedback");
    };
    // The relay changes state only after the measured PV crosses a hysteresis threshold.
    let high = super::super::relay_state::next_relay_state(
        measured,
        relay.center,
        relay.hysteresis,
        true,
        relay.high_active,
    );
    if high == relay.high_active {
        return None;
    }
    let level = if high {
        relay.high_value.clone()
    } else {
        relay.low_value.clone()
    };
    if let Some(relay) = record.relay_feedback.as_mut() {
        relay.high_active = high;
    }
    queue(record, debug, level);
    None
}

/// Counts an execution after the restoring write and confirms the restoration once the target reads back its restoration
/// value. Three executions without confirmation fail it (Specification 35, A7).
fn confirm_restoration(record: &mut ExperimentRecord, view: &ScanView<'_>) {
    if record.restoration != "pending" {
        return;
    }
    record.restoration_checks = record.restoration_checks.saturating_add(1);
    if restoration_confirmed(record, view) {
        record.restoration = "restored";
    } else if record.restoration_checks >= RESTORATION_CONFIRMATION_EXECUTIONS {
        record.status = ExperimentStatus::FailedToRestore;
        if let Some(abort) = record.abort_binding.as_mut() {
            abort.fail_confirmation();
        }
        record.restoration = "failed";
        record.restoration_reason = Some("confirmation_timeout");
    }
}

/// Whether the target reads back its restoration value and, with an abort binding, the abort input is observed inactive
/// in a scan after the restoring write.
fn restoration_confirmed(record: &mut ExperimentRecord, view: &ScanView<'_>) -> bool {
    if record.target.observed(view.io, view.storage).as_ref()
        != Some(&record.restoration_observed_value)
    {
        return false;
    }
    let Some(abort) = record.abort_binding.as_mut() else {
        return true;
    };
    let Some(address) = record.target.address() else {
        return false;
    };
    if view
        .io
        .scan
        .zip(record.restoration_after_scan)
        .is_none_or(|(scan, after)| scan <= after)
        || view.io.forced.contains(address)
        || snapshot_type(view.io, address) != Some(TypeId::BOOL)
    {
        return false;
    }
    abort.confirm(view.io)
}
