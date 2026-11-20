//! What the resource loop records of a debug image's Rust POU records after a scan (specification 48
//! RU-49; specification 11, "Rust POU budgets"): each changed record in the metrics for
//! `tasks.stats`, and the event `rust_budget_exceeded` for a call over the budget longer than every
//! call before it. A report comes only when a record changes, so this runs from flash on the C6,
//! outside the scan path's RAM code (`firmware/trust-esp32c6/ld/rwtext_hook.x`), which names the
//! observer's module and not this one.

use smol_str::SmolStr;
use trust_runtime_core::native::program::RustPouReport;
use trust_runtime_core::ports::MonotonicClock;
use trust_runtime_core::value::Duration;

use super::state::Records;
use crate::events::RuntimeEvent;
use crate::metrics::RuntimeMetrics;

/// A newly built program's Rust POUs replace those that a report gave before.
#[inline(never)]
pub(super) fn fresh<C: MonotonicClock>(records: &Records<C>) {
    records.record(RuntimeMetrics::clear_rust_pous);
}

/// The Rust POU of `report` after the scan at the logical time `now`: its record into the metrics,
/// and the event when its longest call grew past its budget.
#[inline(never)]
pub(super) fn record<C: MonotonicClock>(
    records: &Records<C>,
    report: &RustPouReport<'_>,
    now: Duration,
) {
    let record = report.record;
    records.record(|metrics| {
        metrics.record_rust_pou(
            report.pou,
            report.name,
            record.budget_ns,
            record.longest_ns,
            record.over,
        );
    });
    if report.exceeded {
        records.push_event(RuntimeEvent::RustBudgetExceeded {
            name: SmolStr::new(report.name),
            call_ns: record.longest_ns,
            budget_ns: record.budget_ns,
            time: now,
        });
    }
}

#[cfg(test)]
#[path = "rust_report_tests.rs"]
mod tests;
