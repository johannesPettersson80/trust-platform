//! Pulse ends (specification 39, §12.6 and §21.5 item 1): the runtime ends every pulse. The end
//! is scheduled by the call that applies the pulse, written at the first call after the pulse's
//! duration and, when that write fails, tried again at every later call until it is written. It
//! belongs to the exchange, so a browser that disconnects, a session that ends or a request that
//! returns early cannot leave the point set.
//!
//! Whoever started the pulse is told what happened to its end. The telling runs on the runtime
//! thread between scans, so a report only records and queues; a report that fails does not stop
//! the call.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::{Duration, Instant};

use crate::hmi::commands::write::Written;
use crate::hmi::member_paths::MemberPath;

/// The latest end a pulse can have, for a duration beyond what the clock can add.
const FURTHEST: Duration = Duration::from_secs(100 * 365 * 24 * 60 * 60);

/// What happened to a pulse's end.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PulseEnd {
    /// The end was written at its `attempts`th try.
    Written {
        /// The point before and after the end.
        written: Written,
        /// How many calls tried it, this one included.
        attempts: u32,
    },
    /// The first try failed. The end stays due and is tried again at every call until it is
    /// written; this is told once.
    Retrying {
        /// Why the write failed.
        reason: String,
    },
    /// A later command to the same point replaced the end before it was written.
    Replaced {
        /// How many calls had tried it.
        attempts: u32,
    },
}

/// Told what happens to a pulse's end, on the runtime thread.
pub(crate) type PulseEndReport = Box<dyn FnMut(PulseEnd) + Send>;

/// An end not yet written.
pub(super) struct Scheduled {
    due: Instant,
    path: MemberPath,
    attempts: u32,
    report: Option<PulseEndReport>,
}

impl Scheduled {
    /// Tries the end with `write` and tells the outcome; the end again when it must be tried at
    /// the next call.
    pub(super) fn end(
        mut self,
        write: impl FnOnce(&MemberPath) -> Result<Written, String>,
    ) -> Option<Self> {
        self.attempts = self.attempts.saturating_add(1);
        match write(&self.path) {
            Ok(written) => {
                let attempts = self.attempts;
                self.tell(PulseEnd::Written { written, attempts });
                None
            }
            Err(reason) => {
                if self.attempts == 1 {
                    self.tell(PulseEnd::Retrying { reason });
                }
                Some(self)
            }
        }
    }

    /// A later command to the point replaced the end.
    pub(super) fn replaced(mut self) {
        let attempts = self.attempts;
        self.tell(PulseEnd::Replaced { attempts });
    }

    fn tell(&mut self, end: PulseEnd) {
        if let Some(report) = self.report.as_mut() {
            // A failing report must not stop the call that serves the screens.
            let _ = catch_unwind(AssertUnwindSafe(|| report(end)));
        }
    }
}

/// The ends not yet written, at most one per point.
#[derive(Default)]
pub(super) struct PulseEnds {
    scheduled: Vec<Scheduled>,
}

impl PulseEnds {
    /// Schedules a pulse's end `after` its start at `now`. The caller first takes out an end the
    /// point already has, with [`Self::replace`].
    pub(super) fn schedule(
        &mut self,
        path: MemberPath,
        now: Instant,
        after: Duration,
        report: Option<PulseEndReport>,
    ) {
        let due = now
            .checked_add(after)
            .or_else(|| now.checked_add(FURTHEST))
            .unwrap_or(now);
        self.scheduled.push(Scheduled {
            due,
            path,
            attempts: 0,
            report,
        });
    }

    /// Takes out the end of a point that a later command replaces.
    pub(super) fn replace(&mut self, path: &MemberPath) -> Vec<Scheduled> {
        let (replaced, kept) = std::mem::take(&mut self.scheduled)
            .into_iter()
            .partition(|end| &end.path == path);
        self.scheduled = kept;
        replaced
    }

    /// Takes out the ends due at `now`, to be written.
    pub(super) fn take_due(&mut self, now: Instant) -> Vec<Scheduled> {
        let (due, later) = std::mem::take(&mut self.scheduled)
            .into_iter()
            .partition(|end| end.due <= now);
        self.scheduled = later;
        due
    }

    /// Puts back ends that were not written, to be tried at the next call.
    pub(super) fn retry(&mut self, ends: Vec<Scheduled>) {
        self.scheduled.extend(ends);
    }

    /// How many ends are not yet written.
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.scheduled.len()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::value::Value;

    fn path() -> MemberPath {
        MemberPath::parse("Main.Pulse").expect("a path")
    }

    fn written() -> Written {
        Written {
            old: Value::Bool(true),
            readback: Value::Bool(false),
        }
    }

    /// A report that keeps what it is told.
    fn recorder() -> (PulseEndReport, Arc<Mutex<Vec<PulseEnd>>>) {
        let told = Arc::new(Mutex::new(Vec::new()));
        let keep = told.clone();
        (
            Box::new(move |end| keep.lock().expect("told").push(end)),
            told,
        )
    }

    #[test]
    fn an_end_is_due_only_after_the_pulse_duration() {
        let mut ends = PulseEnds::default();
        let start = Instant::now();
        ends.schedule(path(), start, Duration::from_millis(300), None);
        assert!(ends.take_due(start + Duration::from_millis(299)).is_empty());
        assert_eq!(ends.take_due(start + Duration::from_millis(300)).len(), 1);
        assert_eq!(ends.len(), 0);
    }

    #[test]
    fn a_failing_end_is_retried_at_every_call_until_it_is_written_and_told_once() {
        let (report, told) = recorder();
        let mut ends = PulseEnds::default();
        let start = Instant::now();
        ends.schedule(path(), start, Duration::ZERO, Some(report));
        let mut tries = 0;
        for _ in 0..3 {
            let unwritten: Vec<_> = ends
                .take_due(Instant::now())
                .into_iter()
                .filter_map(|end| {
                    end.end(|_| {
                        tries += 1;
                        Err("the program is not reachable".to_string())
                    })
                })
                .collect();
            ends.retry(unwritten);
        }
        assert_eq!(tries, 3, "tried at each call");
        assert_eq!(ends.len(), 1, "still due");
        assert_eq!(
            *told.lock().expect("told"),
            vec![PulseEnd::Retrying {
                reason: "the program is not reachable".into()
            }],
            "the failure is told once"
        );
        let unwritten: Vec<_> = ends
            .take_due(Instant::now())
            .into_iter()
            .filter_map(|end| end.end(|_| Ok(written())))
            .collect();
        assert!(unwritten.is_empty());
        assert_eq!(ends.len(), 0);
        assert_eq!(
            told.lock().expect("told").last(),
            Some(&PulseEnd::Written {
                written: written(),
                attempts: 4
            })
        );
    }

    #[test]
    fn a_later_command_to_the_point_takes_out_its_end_and_it_is_told() {
        let (report, told) = recorder();
        let mut ends = PulseEnds::default();
        let other = MemberPath::parse("Main.Other").expect("a path");
        ends.schedule(path(), Instant::now(), Duration::from_secs(5), Some(report));
        ends.schedule(other.clone(), Instant::now(), Duration::from_secs(5), None);
        for end in ends.replace(&path()) {
            end.replaced();
        }
        assert_eq!(ends.len(), 1, "the other point's end stays");
        assert_eq!(
            *told.lock().expect("told"),
            vec![PulseEnd::Replaced { attempts: 0 }]
        );
        assert!(ends.replace(&path()).is_empty());
        assert_eq!(ends.replace(&other).len(), 1);
    }

    #[test]
    fn a_report_that_fails_does_not_stop_the_call() {
        let mut ends = PulseEnds::default();
        ends.schedule(
            path(),
            Instant::now(),
            Duration::ZERO,
            Some(Box::new(|_: PulseEnd| panic!("a broken report"))),
        );
        let unwritten: Vec<_> = ends
            .take_due(Instant::now())
            .into_iter()
            .filter_map(|end| end.end(|_| Ok(written())))
            .collect();
        assert!(unwritten.is_empty(), "the end was written all the same");
    }

    #[test]
    fn a_duration_beyond_the_clock_still_schedules_an_end() {
        let mut ends = PulseEnds::default();
        let start = Instant::now();
        ends.schedule(path(), start, Duration::MAX, None);
        assert!(ends.take_due(start + Duration::from_secs(3600)).is_empty());
        assert_eq!(ends.len(), 1);
    }
}
