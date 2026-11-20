//! The records of a debug image's Rust POU calls (specification 44 §5.13, §8.2; specification 48
//! RU-49): the program makes them from the image's Rust POU table when it is built, the module keeps
//! each POU's longest call and its calls over its budget through every activation's context, and
//! after each scan the runner reports what the scan changed, for the event `rust_budget_exceeded`
//! and `tasks.stats` (specification 11, "Rust POU budgets").

use trust_plc_abi::RustTime;

use super::super::image::{Image, RustPouRecord};
use super::{ModuleCode, NativeProgram};
use crate::cycle::CycleHooks;
use crate::value::Duration;

/// A Rust POU's record after the scans since the last report changed it (specification 48 RU-49).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RustPouReport<'a> {
    /// The POU's number in the image's Rust POU table.
    pub pou: u32,
    /// The POU's name as Structured Text declares it.
    pub name: &'a str,
    /// The POU's record: its budget, its longest call and its calls over the budget.
    pub record: RustTime,
    /// Whether a call over the budget was longer than every call of the POU before it, the call of
    /// `record.longest_ns`: the event `rust_budget_exceeded` (specification 11).
    pub exceeded: bool,
}

impl<C: ModuleCode> NativeProgram<'_, C> {
    /// The record of each Rust POU of a debug image, in the order of its Rust POU table; none for
    /// an image without the table, as a release image is.
    #[must_use]
    pub fn rust_times(&self) -> &[RustTime] {
        &self.rust_times
    }

    /// The name of the Rust POU numbered `pou` in the image's Rust POU table, as Structured Text
    /// declares it.
    #[must_use]
    pub fn rust_pou_name(&self, pou: u32) -> Option<&str> {
        let image = self.image();
        let name = image.table::<RustPouRecord>().ok()?.get(pou)?.name;
        image.string(name)
    }

    /// Reports to `hooks` what the scans since the last report changed in the Rust POU records,
    /// after the scan at the logical time `now`: on the program's first report
    /// [`CycleHooks::on_rust_times_fresh`] and every Rust POU, after it each POU whose record
    /// changed (specification 48 RU-49). A POU is `exceeded` when its longest call grew and is over
    /// the budget, at most once per POU and report: the first call over the budget and each one
    /// longer than every call before it. A program without Rust POU records reports only that it
    /// is fresh.
    pub fn report_rust_times(&mut self, now: Duration, hooks: &mut dyn CycleHooks) {
        let fresh = self.rust_reported.is_none();
        if fresh {
            hooks.on_rust_times_fresh();
        }
        let reported = self.rust_reported.get_or_insert_with(|| {
            self.rust_times
                .iter()
                .map(|time| RustTime::new(time.budget_ns))
                .collect()
        });
        // An image without Rust POU records, a release image's and most programs', has no more to
        // report after its first scan.
        if self.rust_times.is_empty() {
            return;
        }
        let image = Image::over(self.bytes.as_slice(), &self.shape);
        let Ok(table) = image.table::<RustPouRecord>() else {
            return;
        };
        for ((pou, record), before) in (0u32..)
            .zip(self.rust_times.iter())
            .zip(reported.iter_mut())
        {
            if !fresh && record == before {
                continue;
            }
            // The longest call grew past every call before it; over the budget, it is a call over
            // the budget longer than every call before it, the first one among them.
            let exceeded =
                record.longest_ns > before.longest_ns && record.longest_ns > record.budget_ns;
            let name = table
                .get(pou)
                .and_then(|entry| image.string(entry.name))
                .unwrap_or("");
            hooks.on_rust_pou(
                &RustPouReport {
                    pou,
                    name,
                    record: *record,
                    exceeded,
                },
                now,
            );
            *before = *record;
        }
    }
}
