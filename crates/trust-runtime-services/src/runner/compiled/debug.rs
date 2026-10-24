//! The debugger of a compiled program's cycle (specification 44 §13.2): the loop's hold, which the
//! cycle attaches, the statement table's sources, the statements armed for the next stretch of the
//! scan, and the holds' time, which is no active time (specification 11 §6.6).
//!
//! The decisions are the shared ones of the loop's debugger ([`ChipDebug`](super::super::debugger)),
//! the same a held interpreter takes: the kernel calls the scan's hooks at a statement whose bit is
//! set, the cycle asks the debugger whether the scan holds there, and while it holds the loop serves
//! the link with the held scan's memory.

use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use core::marker::PhantomData;

use trust_runtime_core::debug::StatementSpan;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::FrameId;
use trust_runtime_core::native::image::{Image, StmtRecord};
use trust_runtime_core::native::memory::ProgramMemory;
use trust_runtime_core::native::stop::Stop;
use trust_runtime_core::ports::MonotonicClock;

use super::super::debugger::{Boundary, StatementStart, StopPoints};
use super::super::engine::held_time::HeldTime;
use super::super::hold::{CompiledHeld, CompiledHold};
use crate::debug::LineSources;
use crate::status::IoDriverStatus;

/// The statement ids of `image` that `points` stops at, one flag per id: a breakpoint stops at every
/// statement of its file that starts where it starts.
pub(super) fn wanted(image: &Image<'_>, points: &StopPoints) -> Vec<bool> {
    let Ok(statements) = image.table::<StmtRecord>() else {
        return Vec::new();
    };
    statements
        .iter()
        .map(|statement| {
            let start = (statement.start_line, u32::from(statement.start_column));
            points
                .starts
                .iter()
                .any(|(file, at)| *file == u32::from(statement.file) && *at == start)
        })
        .collect()
}

/// The debugger state of a compiled program's cycle, whose scans the platform's hold `H` holds.
pub(super) struct CompiledDebug<H> {
    /// The platform's hold, once the platform attached it.
    pub(super) hold: Option<H>,
    /// The statement table and file labels of a debug image; none for a release image, which is
    /// not debuggable (specification 44 §13.2).
    pub(super) sources: Option<LineSources>,
    /// Whether the image is a release image.
    pub(super) release: bool,
    /// The statements armed now, as the debugger last gave them.
    armed: RefCell<StopPoints>,
    /// The holds on the platform's clock.
    pub(super) holds: HeldTime,
    /// The health of the process I/O, published before the program runs while debugging is live,
    /// for a hold to report.
    pub(super) drivers: Cell<Vec<IoDriverStatus>>,
}

impl<H: CompiledHold> CompiledDebug<H> {
    /// The debugger of the program of `image`, with no hold attached yet.
    pub(super) fn new(image: &Image<'_>) -> Self {
        let release = image.header().is_release();
        Self {
            hold: None,
            sources: (!release).then(|| LineSources::from_image(image)),
            release,
            armed: RefCell::new(StopPoints::default()),
            holds: HeldTime::new(),
            drivers: Cell::new(Vec::new()),
        }
    }

    /// What the program's statements stop for from now on: what the debugger gives while the
    /// platform enables debugging and the image has hooks, otherwise nothing.
    pub(super) fn stop_points(&self, enabled: bool) -> StopPoints {
        match &self.hold {
            Some(hold) if enabled && !self.release => hold.stop_points(),
            _ => StopPoints::default(),
        }
    }

    /// Whether `points` differ from the statements armed now, and if so takes them as armed.
    pub(super) fn rearms(&self, points: &StopPoints) -> bool {
        let mut armed = self.armed.borrow_mut();
        if *armed == *points {
            return false;
        }
        armed.clone_from(points);
        true
    }

    /// The stop at the held statement of `stop`: the debugger decides, and while it holds the scan
    /// the loop serves the link with the held scan's memory. Returns whether the scan was held.
    ///
    /// # Errors
    ///
    /// The error that fails the scan when the hold cannot go on.
    pub(super) fn at_stop(
        &self,
        stop: &mut Stop<'_>,
        clock: &dyn MonotonicClock,
    ) -> Result<bool, RuntimeError> {
        let Some(hold) = self.hold.as_ref() else {
            return Ok(false);
        };
        let Some(record) = stop.record() else {
            return Ok(false);
        };
        let depth = stop.depth();
        let at = StatementStart {
            depth,
            span: StatementSpan {
                file_index: u32::from(record.file),
                start: (record.start_line, u32::from(record.start_column)),
                end: (record.end_line, u32::from(record.end_column)),
            },
            frame: Some(FrameId(depth)),
            call: PhantomData,
        };
        if hold.at_statement(&at)? == Boundary::Run {
            return Ok(false);
        }
        let view = CompiledHeld {
            memory: &*stop,
            sources: self.sources.as_ref(),
            drivers: &self.drivers,
        };
        self.holds.start(clock.now());
        let outcome = hold.hold_compiled(&at, &view);
        self.holds.end(clock.now());
        outcome.map(|()| true)
    }

    /// Arms the statements of the held scan of `stop` for the rest of the scan, as the debugger
    /// gives them after the stop.
    pub(super) fn arm_stop(&self, stop: &mut Stop<'_>) {
        let points = self.stop_points(true);
        if !self.rearms(&points) {
            return;
        }
        let wanted = wanted(&stop.image(), &points);
        stop.arm(points.every, &|id| {
            usize::try_from(id)
                .ok()
                .and_then(|id| wanted.get(id))
                .copied()
                .unwrap_or(false)
        });
    }
}
