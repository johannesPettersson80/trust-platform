//! Wall-clock pacing of the simulator's cycle loop, and the achieved speed it reports (specification 46, §6.1).

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::scheduler::AchievedSpeedWindow;

/// Identifies the simulated resource of a debug session in the achieved-speed report of its metrics.
const SIMULATOR_SOURCE: u64 = 1;

/// The wall interval between simulator cycles at `speed`: the cycle interval divided by the speed (specification 46,
/// §6.1 item 2).
pub(super) fn paced_interval(cycle_interval: Duration, speed: u32) -> Duration {
    let nanos = cycle_interval.as_nanos() / u128::from(speed.max(1));
    Duration::from_nanos(u64::try_from(nanos).unwrap_or(u64::MAX).max(1))
}

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
    /// stalled host) moves the grid to `now` instead of running the missed cycles in a burst. Returns whether the
    /// grid moved.
    pub(super) fn begin(&mut self, now: Instant) -> bool {
        if now > self.scheduled + self.interval {
            self.scheduled = now;
            return true;
        }
        false
    }

    /// Moves the grid to `interval` and starts its schedule at `now`, so that a speed change never starts a burst
    /// (specification 46, §6.1 item 5).
    pub(super) fn retarget(&mut self, interval: Duration, now: Instant) {
        self.interval = interval;
        self.scheduled = now;
    }

    /// The instant the next cycle is due.
    pub(super) fn next(&mut self) -> Instant {
        self.scheduled += self.interval;
        self.scheduled
    }
}

/// Paces the simulator's cycles at the speed in force and measures the achieved speed (specification 46, §6.1
/// items 2 to 5). A change of speed, a pause or a late resume restarts the schedule and the measurement.
pub(super) struct SimulatorClock {
    pacer: CyclePacer,
    cycle_interval: Duration,
    scheduled_speed: u32,
    origin: Instant,
    last_begin: Option<Instant>,
    window: AchievedSpeedWindow,
    metrics: Option<Arc<Mutex<RuntimeMetrics>>>,
}

/// A gap between cycle starts longer than this is a stall or a paused debugger, not a late cycle.
const STALL: Duration = Duration::from_secs(1);

impl SimulatorClock {
    /// A clock for cycles of `cycle_interval` at `speed`, starting at `now`. With `metrics`, the achieved speed is
    /// published into them.
    pub(super) fn new(
        cycle_interval: Duration,
        speed: u32,
        metrics: Option<Arc<Mutex<RuntimeMetrics>>>,
        now: Instant,
    ) -> Self {
        Self {
            pacer: CyclePacer::new(paced_interval(cycle_interval, speed), now),
            cycle_interval,
            scheduled_speed: speed,
            origin: now,
            last_begin: None,
            window: AchievedSpeedWindow::default(),
            metrics,
        }
    }

    /// Prepares the cycle that starts at `now`. A change of speed re-targets the schedule. A paused debugger or a stall
    /// longer than a second restarts the measurement, so a pause never counts as simulated time. A merely late cycle
    /// does not: a host that cannot keep the pace is exactly what the achieved speed reports (spec 46, §6.1 item 3).
    pub(super) fn begin(&mut self, now: Instant, speed: u32, paused: bool) {
        if speed != self.scheduled_speed {
            self.scheduled_speed = speed;
            self.pacer
                .retarget(paced_interval(self.cycle_interval, speed), now);
            self.restart_measurement();
        }
        let resynced = self.pacer.begin(now);
        let stalled = self
            .last_begin
            .is_some_and(|last| now.saturating_duration_since(last) > STALL);
        self.last_begin = Some(now);
        if paused || (resynced && stalled) {
            self.restart_measurement();
        }
    }

    /// Records a cycle that starts at `now` and sees `process_time_nanos` of simulation time.
    pub(super) fn record_cycle(&mut self, now: Instant, process_time_nanos: i64) {
        let wall_nanos = i64::try_from(now.saturating_duration_since(self.origin).as_nanos())
            .unwrap_or(i64::MAX);
        let achieved = self.window.record(wall_nanos, process_time_nanos);
        self.publish(achieved);
    }

    /// The instant the next cycle is due.
    pub(super) fn next(&mut self) -> Instant {
        self.pacer.next()
    }

    fn restart_measurement(&mut self) {
        self.window.clear();
        self.publish(None);
    }

    fn publish(&self, achieved: Option<f64>) {
        let Some(metrics) = self.metrics.as_ref() else {
            return;
        };
        if let Ok(mut guard) = metrics.lock() {
            guard.set_simulation_achieved_time_scale(SIMULATOR_SOURCE, achieved);
        }
    }
}

impl Drop for SimulatorClock {
    /// A stopped simulator reports no achieved speed.
    fn drop(&mut self) {
        self.publish(None);
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
        assert!(pacer.begin(resumed), "a stall moves the grid");
        assert_eq!(pacer.next(), resumed + interval);
    }

    #[test]
    fn simulation_at_speed_ten_paces_cycles_at_a_tenth_of_the_cycle_interval() {
        assert_eq!(
            paced_interval(Duration::from_millis(10), 10),
            Duration::from_millis(1)
        );
        assert_eq!(
            paced_interval(Duration::from_millis(10), 1),
            Duration::from_millis(10)
        );
    }

    #[test]
    fn a_speed_change_restarts_the_schedule_at_the_next_cycle() {
        let start = Instant::now();
        let mut clock = SimulatorClock::new(Duration::from_millis(10), 1, None, start);
        clock.begin(start, 1, false);
        assert_eq!(clock.next(), start + Duration::from_millis(10));
        // The speed becomes 10 at 10 ms: the cycle starts at once, and the next one is due one paced interval later.
        let change = start + Duration::from_millis(10);
        clock.begin(change, 10, false);
        assert_eq!(clock.next(), change + Duration::from_millis(1));
    }

    /// Runs one simulated second of cycles at speed 10 and returns the clock, with its achieved speed published.
    fn measured_clock(metrics: &Arc<Mutex<RuntimeMetrics>>, start: Instant) -> SimulatorClock {
        let mut clock = SimulatorClock::new(
            Duration::from_millis(10),
            10,
            Some(Arc::clone(metrics)),
            start,
        );
        // A cycle every 1 ms of wall time, each seeing 10 ms more simulation time: the achieved speed is 10.
        for cycle in 0..=1_100_u64 {
            let now = start + Duration::from_millis(cycle);
            clock.begin(now, 10, false);
            clock.record_cycle(now, i64::try_from(cycle * 10_000_000).unwrap());
            clock.next();
        }
        clock
    }

    #[test]
    fn the_achieved_speed_is_published_once_a_second_of_wall_time_is_measured() {
        let metrics = Arc::new(Mutex::new(RuntimeMetrics::default()));
        let _clock = measured_clock(&metrics, Instant::now());
        let achieved = metrics
            .lock()
            .unwrap()
            .simulation_achieved_time_scale()
            .expect("one second of cycles is measured");
        assert!(
            (achieved - 10.0).abs() < 1e-9,
            "achieved speed {achieved} must be the speed of the simulation"
        );
    }

    #[test]
    fn a_speed_change_clears_the_achieved_speed_until_it_is_measured_again() {
        let metrics = Arc::new(Mutex::new(RuntimeMetrics::default()));
        let start = Instant::now();
        let mut clock = measured_clock(&metrics, start);
        clock.begin(start + Duration::from_millis(1_101), 1, false);
        assert_eq!(
            metrics.lock().unwrap().simulation_achieved_time_scale(),
            None
        );
    }

    #[test]
    fn a_pause_clears_the_achieved_speed() {
        let metrics = Arc::new(Mutex::new(RuntimeMetrics::default()));
        let start = Instant::now();
        let mut clock = measured_clock(&metrics, start);
        clock.begin(start + Duration::from_millis(1_101), 10, true);
        assert_eq!(
            metrics.lock().unwrap().simulation_achieved_time_scale(),
            None
        );
    }

    #[test]
    fn a_stopped_simulator_clears_the_achieved_speed() {
        let metrics = Arc::new(Mutex::new(RuntimeMetrics::default()));
        let clock = measured_clock(&metrics, Instant::now());
        drop(clock);
        assert_eq!(
            metrics.lock().unwrap().simulation_achieved_time_scale(),
            None
        );
    }
}
