//! Cycle timing of a resource loop (specification 46, §6.1): the wall-clock cycle grid and, in simulation,
//! exact simulation-time steps.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::metrics::RuntimeMetrics;
use crate::value::Duration;

use super::simulation_clock::{AchievedSpeedWindow, SimulationSpeed};

/// Identifies each simulated resource in the achieved-speed report of its metrics.
static NEXT_SOURCE_ID: AtomicU64 = AtomicU64::new(1);

/// Where one cycle sits in time: the process time it sees and the wall-clock slot it is paced to.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CycleSlot {
    pub(crate) process_time: Duration,
    pace_nanos: i64,
    start_nanos: i64,
}

/// Simulation time of one resource: one step per executed cycle, the shared speed, and the achieved speed
/// reported to control.status (specification 46, §6.1).
#[derive(Debug)]
struct SimulationClock {
    source: u64,
    speed: SimulationSpeed,
    /// Speed the schedule runs at. A change restarts the schedule.
    scheduled_speed: u32,
    /// Simulation time the next executed cycle sees; `None` until the first cycle starts.
    next_time: Option<Duration>,
    window: AchievedSpeedWindow,
    metrics: Option<Arc<Mutex<RuntimeMetrics>>>,
}

impl SimulationClock {
    fn new(speed: SimulationSpeed, metrics: Option<Arc<Mutex<RuntimeMetrics>>>) -> Self {
        let scheduled_speed = speed.get();
        Self {
            source: NEXT_SOURCE_ID.fetch_add(1, Ordering::Relaxed),
            speed,
            scheduled_speed,
            next_time: None,
            window: AchievedSpeedWindow::default(),
            metrics,
        }
    }

    fn scale(&self) -> u32 {
        self.speed.get()
    }

    /// Notes the speed in force. Returns `true` when it differs from the speed the schedule runs at.
    fn follow(&mut self, speed: u32) -> bool {
        let changed = speed != self.scheduled_speed;
        self.scheduled_speed = speed;
        changed
    }

    /// The simulation time the next cycle sees; the cycle after it sees one interval later. The first cycle
    /// starts from the scaled wall clock, as the cycle would without simulation time.
    fn step(&mut self, wall: Duration, interval: Duration, speed: u32) -> Duration {
        let time = self
            .next_time
            .unwrap_or_else(|| super::scaled_time(wall, speed));
        self.next_time = Some(Duration::from_nanos(
            time.as_nanos().saturating_add(interval.as_nanos()),
        ));
        time
    }

    /// Records a started cycle in the achieved-speed window and publishes the result.
    fn measure(&mut self, wall: Duration, time: Duration) {
        let achieved = self.window.record(wall.as_nanos(), time.as_nanos());
        self.publish(achieved);
    }

    /// Starts a new measurement, as after a pause, a restart or a speed change.
    fn restart_measurement(&mut self) {
        self.window.clear();
        self.publish(None);
    }

    fn publish(&self, achieved: Option<f64>) {
        let Some(metrics) = self.metrics.as_ref() else {
            return;
        };
        if let Ok(mut guard) = metrics.lock() {
            guard.set_simulation_achieved_time_scale(self.source, achieved);
        }
    }
}

impl Drop for SimulationClock {
    fn drop(&mut self) {
        self.publish(None);
    }
}

/// Timing of a resource loop. On the wall clock, cycles follow the grid of the cycle interval. In simulation,
/// each executed cycle also advances simulation time by one interval (specification 46, §6.1).
#[derive(Debug, Default)]
pub(crate) struct CycleTiming {
    /// Wall-clock start of the last cycle's slot. The next cycle is due one paced interval later.
    scheduled_start: Option<i64>,
    simulation: Option<SimulationClock>,
}

impl CycleTiming {
    /// Timing of a resource whose process time follows the wall clock.
    pub(crate) fn wall_clock() -> Self {
        Self::default()
    }

    /// Timing of a resource on simulation time at the shared `speed`. With `metrics`, the achieved speed is
    /// reported to control.status.
    pub(crate) fn simulated(
        speed: SimulationSpeed,
        metrics: Option<Arc<Mutex<RuntimeMetrics>>>,
    ) -> Self {
        Self {
            scheduled_start: None,
            simulation: Some(SimulationClock::new(speed, metrics)),
        }
    }

    /// The speed the loop paces with: the simulation speed, or 1 on the wall clock.
    pub(crate) fn scale(&self) -> u32 {
        self.simulation.as_ref().map_or(1, SimulationClock::scale)
    }

    /// The shared simulation speed, when the resource runs on simulation time.
    pub(crate) fn simulation_speed(&self) -> Option<SimulationSpeed> {
        self.simulation
            .as_ref()
            .map(|simulation| simulation.speed.clone())
    }

    /// The process time a restarted runtime continues from (specification 46, §6.1 item 4).
    pub(crate) fn restart_time(&self, wall: Duration) -> Duration {
        match self.simulation.as_ref() {
            Some(simulation) => simulation
                .next_time
                .unwrap_or_else(|| super::scaled_time(wall, simulation.scale())),
            None => wall,
        }
    }

    /// Restarts the schedule at the next cycle and measures the achieved speed afresh (specification 46, §6.1
    /// items 3 and 4). Has no effect on the wall clock.
    pub(crate) fn restart_schedule(&mut self) {
        if let Some(simulation) = self.simulation.as_mut() {
            self.scheduled_start = None;
            simulation.restart_measurement();
        }
    }

    /// Starts a cycle at wall time `wall`, with the cycle interval `interval`.
    pub(crate) fn begin_cycle(&mut self, wall: Duration, interval: Duration) -> CycleSlot {
        let speed = self.scale();
        let changed = self
            .simulation
            .as_mut()
            .is_some_and(|simulation| simulation.follow(speed));
        if changed {
            self.restart_schedule();
        }
        let pace_nanos = super::scaled_sleep_interval(interval, speed).as_nanos();
        let start_nanos =
            super::grid_cycle_start(self.scheduled_start, wall.as_nanos(), pace_nanos);
        let process_time = self.process_time(wall, interval, speed);
        CycleSlot {
            process_time,
            pace_nanos,
            start_nanos,
        }
    }

    /// Records the slot of the cycle that ran and returns the instant the next cycle is due.
    pub(crate) fn finish_cycle(&mut self, slot: CycleSlot) -> Duration {
        let deadline = slot.start_nanos.saturating_add(slot.pace_nanos);
        self.scheduled_start = Some(deadline);
        Duration::from_nanos(deadline)
    }

    /// Simulation time steps once per executed cycle that has a cycle interval. Otherwise process time is the
    /// scaled wall clock.
    fn process_time(&mut self, wall: Duration, interval: Duration, speed: u32) -> Duration {
        let stepping = interval.as_nanos() > 0;
        match self.simulation.as_mut() {
            Some(simulation) if stepping => {
                let time = simulation.step(wall, interval, speed);
                simulation.measure(wall, time);
                time
            }
            _ => super::scaled_time(wall, speed),
        }
    }
}
