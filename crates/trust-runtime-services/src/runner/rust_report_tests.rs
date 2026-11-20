//! The chip's loop hears a debug image's Rust POU reports through the hooks of its Live Values,
//! which wrap its observer (specification 48 RU-49; specification 11, "Rust POU budgets"): the
//! records reach the metrics that `tasks.stats` reads, and a call over the budget longer than every
//! call before it the event store.

use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_plc_abi::RustTime;
use trust_runtime_core::cycle::CycleHooks;
use trust_runtime_core::native::program::RustPouReport;
use trust_runtime_core::ports::{MonotonicClock, MonotonicInstant};
use trust_runtime_core::value::Duration;

use super::super::debugger::ChipDebug;
use super::super::live::{DeviceLiveValues, LiveValuesHooks};
use super::super::observer::ScanObserver;
use super::super::state::Records;
use crate::events::RuntimeEvent;
use crate::metrics::RustPouStats;

/// A clock that stands still.
struct Still;

impl MonotonicClock for Still {
    fn now(&self) -> MonotonicInstant {
        MonotonicInstant::from_origin(core::time::Duration::ZERO)
    }
}

#[test]
fn the_loops_live_values_hooks_pass_the_rust_pou_reports_to_its_observer() {
    // The loop scans with its observer wrapped in the Live Values' hooks (supervision.rs), so a
    // report that the wrapper does not pass on never reaches the metrics or the events, which the
    // C6 bench of 18 November 2026 found: `ctl events` and `ctl stats` showed nothing for a
    // program over its budget. A fresh report, then SPIN's first call over its 1,000 ns, 52,000
    // ns, exceeded, and CLOCKED within its budget: tasks.stats' list holds both in table order,
    // and the store holds one event, SPIN's, with the call's time and the budget.
    let records = Records::new(Still, Still.now(), 200);
    let debug = ChipDebug::new(0);
    let live = DeviceLiveValues::empty();
    let mut observer = ScanObserver::new(&[], &[], &records, &Still, &debug);
    let mut hooks = LiveValuesHooks::new(&live, &records, 1, &mut observer);
    let now = Duration::from_millis(20);
    hooks.on_rust_times_fresh();
    hooks.on_rust_pou(
        &RustPouReport {
            pou: 1,
            name: "SPIN",
            record: RustTime {
                budget_ns: 1_000,
                longest_ns: 52_000,
                over: 1,
            },
            exceeded: true,
        },
        now,
    );
    hooks.on_rust_pou(
        &RustPouReport {
            pou: 0,
            name: "CLOCKED",
            record: RustTime {
                budget_ns: 1_000_000,
                longest_ns: 4_000,
                over: 0,
            },
            exceeded: false,
        },
        now,
    );
    let listed: Vec<RustPouStats> = records.metrics_snapshot().expect("the metrics").rust_pous;
    assert_eq!(
        listed,
        [
            RustPouStats {
                name: SmolStr::new("CLOCKED"),
                budget_ns: 1_000_000,
                longest_ns: 4_000,
                over_budget: 0,
            },
            RustPouStats {
                name: SmolStr::new("SPIN"),
                budget_ns: 1_000,
                longest_ns: 52_000,
                over_budget: 1,
            },
        ]
    );
    let events: Vec<RuntimeEvent> = records
        .read_events(|events| events.iter().cloned().collect())
        .expect("the event store");
    assert_eq!(
        events,
        [RuntimeEvent::RustBudgetExceeded {
            name: SmolStr::new("SPIN"),
            call_ns: 52_000,
            budget_ns: 1_000,
            time: now,
        }]
    );
}
