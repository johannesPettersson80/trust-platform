//! The program's `Hmi.Display` calls (specification 39, §18 Phase 5 item 4): at the end of a scan in
//! which the program called the block, after the scan's outputs are published and before the next
//! scan, the runtime serves the HMI exchange from the program, so the screens follow the interval and
//! priority of the task the user chose for them; the method commands the screens asked for are called
//! then, between scans (§21.2 item 2). The blocks are found and their call counts read through the
//! program's tables (specification 44 §13.3).

use std::sync::Arc;

use smol_str::SmolStr;
use trust_runtime_core::io_image::PreparedIoBinding;

use super::hmi_stale::LinkWatch;
use super::io_subsystem::LinkIo;
use super::program_access::{Located, ProgramWrite};
use crate::hmi::alarms::AlarmEvent;
use crate::hmi::exchange::HmiExchange;
use crate::hmi::program_instances::{declares_block, instances_of};
use crate::value::{Duration, Value};

/// The block a program calls to serve its screens.
pub(crate) const DISPLAY_BLOCK: &str = "Hmi.Display";
/// Most blocks followed.
const MAX_DISPLAYS: usize = 16;
/// How often the program's blocks are found again.
const WALK_PERIOD_MS: i64 = 1_000;

/// What the runtime knows of the scan that ended: what a call of `Hmi.Display` copies
/// with the screens' values and marks stale.
pub(crate) struct HmiScan<'a> {
    /// The runtime's cycle count, which the copy carries.
    pub(crate) cycle: u64,
    /// The `io.toml` links as their drivers report them in this scan (specification 42, §5
    /// item 4).
    pub(crate) links: &'a [LinkIo<'a>],
    /// The process image's bindings and the names of the variables they bind.
    pub(crate) bindings: &'a [PreparedIoBinding],
    pub(crate) binding_names: &'a [Option<SmolStr>],
}

/// What the calls of `Hmi.Display` in a scan leave for the runtime between scans.
pub(crate) struct Served {
    /// The exchange whose method commands are due (specification 39, §21.2 item 2).
    pub(crate) exchange: Arc<HmiExchange>,
    /// The alarm records the calls made, for the OpenOT history (specification 40, §5.2).
    pub(crate) alarm_records: Vec<AlarmEvent>,
}

/// The exchange and the program's `Hmi.Display` blocks.
#[derive(Default)]
pub(crate) struct HmiSubsystem {
    exchange: Option<Arc<HmiExchange>>,
    /// Where each block's call count lies, and the count when last looked at.
    displays: Vec<(Located, Option<u64>)>,
    /// When the blocks were last found, on the program's logical time.
    walked_ms: Option<i64>,
    /// Whether the program declares the block, once looked at.
    declared: Option<bool>,
    /// When each link last exchanged data (specification 42, §5 item 4).
    links: LinkWatch,
    /// What the calls since the last scan's end left.
    served: Option<Served>,
}

impl std::fmt::Debug for HmiSubsystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HmiSubsystem")
            .field("exchange", &self.exchange.is_some())
            .field("displays", &self.displays.len())
            .finish_non_exhaustive()
    }
}

impl HmiSubsystem {
    /// Shares the exchange with the web server; the method commands queued for an earlier program
    /// are withdrawn.
    pub(crate) fn set_exchange(&mut self, exchange: Arc<HmiExchange>) {
        exchange.methods.invalidate();
        self.exchange = Some(exchange);
    }

    /// The exchange, once shared.
    pub(crate) fn exchange(&self) -> Option<Arc<HmiExchange>> {
        self.exchange.clone()
    }

    /// The program restarted or changed online: its blocks are found again in it, and the method
    /// commands queued for it are withdrawn, never called on the new one.
    pub(crate) fn program_changed(&mut self) {
        if let Some(exchange) = &self.exchange {
            exchange.methods.invalidate();
        }
        self.displays.clear();
        self.walked_ms = None;
        self.declared = None;
    }

    /// At the end of a scan, at the program's logical time `now`: serves the exchange when a block's
    /// call count grew since it was last looked at, so the program called `Hmi.Display` in that
    /// scan, in whichever task. The queued pulse ends, commands and settings are applied, the values
    /// the open screens and the trends show are copied, those at the inputs of a link without data
    /// marked stale, and the alarms read. A program without the block is not searched; a scan without
    /// a call does no HMI work.
    pub(crate) fn serve_after_scan(
        &mut self,
        program: &mut dyn ProgramWrite,
        now: Duration,
        scan: &HmiScan<'_>,
    ) {
        let Some(exchange) = self.exchange.clone() else {
            return;
        };
        let declared = *self
            .declared
            .get_or_insert_with(|| declares_block(&*program, DISPLAY_BLOCK));
        if !declared {
            return;
        }
        let now_ms = now.as_millis();
        self.find_displays(&*program, now_ms);
        let mut called = false;
        for (calls, seen) in &mut self.displays {
            let Ok(Value::UDInt(count)) = program.read_at(calls) else {
                continue;
            };
            let count = u64::from(count);
            if seen.map_or(count > 0, |last| last != count) {
                called = true;
            }
            *seen = Some(count);
        }
        if !called {
            return;
        }
        let silent =
            self.links
                .silent_inputs(now_ms, scan.links, scan.bindings, scan.binding_names);
        let records = exchange.serve(program, scan.cycle, &silent);
        self.served
            .get_or_insert_with(|| Served {
                exchange: Arc::clone(&exchange),
                alarm_records: Vec::new(),
            })
            .alarm_records
            .extend(records);
    }

    /// After a scan: what its calls of `Hmi.Display` left, `None` when it made none.
    pub(crate) fn take_served(&mut self) -> Option<Served> {
        self.served.take()
    }

    /// Finds the blocks once a second, at most [`MAX_DISPLAYS`], for an online change that adds
    /// one; a block found before keeps the count last seen.
    fn find_displays(&mut self, program: &dyn super::program_access::ProgramRead, now_ms: i64) {
        let due = self
            .walked_ms
            .is_none_or(|walked| now_ms < walked || now_ms - walked >= WALK_PERIOD_MS);
        if !due {
            return;
        }
        let found: Vec<Located> = instances_of(program, DISPLAY_BLOCK, MAX_DISPLAYS)
            .into_iter()
            .filter_map(|display| program.member(&display.at, "Calls"))
            .collect();
        self.displays = found
            .into_iter()
            .map(|calls| {
                let seen = self
                    .displays
                    .iter()
                    .find(|(known, _)| *known == calls)
                    .and_then(|(_, seen)| *seen);
                (calls, seen)
            })
            .collect();
        self.walked_ms = Some(now_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::TestHarness;

    const HMI: &str = include_str!("../../../../libraries/hmi/src/hmi.st");
    const PROGRAM: &str =
        "PROGRAM Screens\nVAR\n    Display : Hmi.Display;\nEND_VAR\n    Display();\nEND_PROGRAM\n";

    /// The end of a scan of `harness`'s program at its time, served by `hmi`.
    fn scan_end(harness: &mut TestHarness, hmi: &mut HmiSubsystem) {
        let now = harness.current_time();
        let scan = HmiScan {
            cycle: harness.cycle_count(),
            links: &[],
            bindings: &[],
            binding_names: &[],
        };
        harness
            .runtime_mut()
            .with_program(|program| hmi.serve_after_scan(program, now, &scan));
    }

    /// Specification 39, §18 Phase 5 item 4: the end of a scan serves the exchange only when a
    /// block's call count grew since it was last looked at, once per growth; a scan without a call
    /// serves nothing, and the serving is handed over once after the scan. Catches serving at every
    /// scan end, a count that is not looked at again, and a hand-over that repeats.
    #[test]
    fn a_scan_end_serves_the_exchange_only_after_the_program_called_the_block() {
        let mut harness = TestHarness::from_sources(&[HMI, PROGRAM]).expect("compiles");
        let exchange = Arc::new(HmiExchange::new());
        let mut hmi = HmiSubsystem::default();
        hmi.set_exchange(Arc::clone(&exchange));
        scan_end(&mut harness, &mut hmi);
        assert_eq!(exchange.calls(), 0, "the block has not been called");
        assert!(hmi.take_served().is_none());
        harness.cycle();
        scan_end(&mut harness, &mut hmi);
        assert_eq!(exchange.calls(), 1, "called once: served once");
        scan_end(&mut harness, &mut hmi);
        assert_eq!(exchange.calls(), 1, "no call since: nothing served");
        assert!(hmi.take_served().is_some(), "handed over after the scan");
        assert!(hmi.take_served().is_none(), "and only once");
        harness.cycle();
        scan_end(&mut harness, &mut hmi);
        assert_eq!(exchange.calls(), 2, "called again: served again");
    }

    /// Specification 39, §21.2: a restart or an online change withdraws the method commands queued
    /// for the program that ran, so none is called on the new one. Catches a generation that a
    /// changed program keeps.
    #[test]
    fn a_changed_program_withdraws_the_queued_method_commands() {
        let exchange = Arc::new(HmiExchange::new());
        let mut hmi = HmiSubsystem::default();
        hmi.set_exchange(Arc::clone(&exchange));
        let before = exchange.methods.generation();
        hmi.program_changed();
        assert_ne!(exchange.methods.generation(), before);
    }
}
