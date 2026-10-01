//! Wall-clock pacing of the simulator's cycle loop.

use std::time::{Duration, Instant};

/// Schedules the start of each simulator cycle on a fixed grid, so that a late wake-up does not delay every
/// later cycle and simulated time keeps pace with wall time.
#[derive(Debug)]
pub(super) struct CyclePacer {
    interval: Duration,
    scheduled: Instant,
}

impl CyclePacer {
    pub(super) fn new(interval: Duration, now: Instant) -> Self {
        Self {
            interval,
            scheduled: now,
        }
    }

    /// A cycle starts at `now`. A start more than one period after its due time (a paused breakpoint or a
    /// stalled host) moves the grid to `now` instead of running the missed cycles in a burst.
    pub(super) fn begin(&mut self, now: Instant) {
        if now > self.scheduled + self.interval {
            self.scheduled = now;
        }
    }

    /// The instant the next cycle is due.
    pub(super) fn next(&mut self) -> Instant {
        self.scheduled += self.interval;
        self.scheduled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_wake_ups_do_not_shift_the_cycle_grid() {
        let interval = Duration::from_millis(20);
        let start = Instant::now();
        let mut pacer = CyclePacer::new(interval, start);
        let mut wake = start;
        let mut due = start;
        for _ in 0..100 {
            pacer.begin(wake);
            due = pacer.next();
            // Every sleep returns 0.5 ms after the instant it was asked to wake.
            wake = due + Duration::from_micros(500);
        }
        assert_eq!(
            due - start,
            Duration::from_millis(2000),
            "100 cycles of 20 ms stay on the 2 s grid"
        );
    }

    #[test]
    fn a_stall_longer_than_one_period_resynchronizes_without_a_burst() {
        let interval = Duration::from_millis(20);
        let start = Instant::now();
        let mut pacer = CyclePacer::new(interval, start);
        pacer.begin(start);
        assert_eq!(pacer.next(), start + interval);
        // A paused breakpoint holds the loop for 100 ms: the next cycle starts at once, then one period later.
        let resumed = start + Duration::from_millis(100);
        pacer.begin(resumed);
        assert_eq!(pacer.next(), resumed + interval);
    }
}
