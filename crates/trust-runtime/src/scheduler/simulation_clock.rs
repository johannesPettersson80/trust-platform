//! Simulation speed and achieved speed of a resource (specification 33, §6.1 items 2, 3 and 5).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// Lowest permitted simulation speed (specification 33, §6.1 item 2).
pub const SIMULATION_TIME_SCALE_MIN: u32 = 1;
/// Highest permitted simulation speed (specification 33, §6.1 item 2).
pub const SIMULATION_TIME_SCALE_MAX: u32 = 100;

/// Wall time over which the achieved speed is measured (specification 33, §6.1 item 3).
const ACHIEVED_SPEED_WINDOW_NANOS: i64 = 5_000_000_000;
/// Wall time that must be measured before an achieved speed is reported (specification 33, §6.1 item 3).
const ACHIEVED_SPEED_MIN_SPAN_NANOS: i64 = 1_000_000_000;

/// Checks a simulation speed against the permitted range. The message names the range.
pub fn validate_time_scale(value: i64) -> Result<u32, String> {
    match u32::try_from(value) {
        Ok(scale) if (SIMULATION_TIME_SCALE_MIN..=SIMULATION_TIME_SCALE_MAX).contains(&scale) => {
            Ok(scale)
        }
        _ => Err(format!(
            "must be >= {SIMULATION_TIME_SCALE_MIN} and <= {SIMULATION_TIME_SCALE_MAX}, got {value}"
        )),
    }
}

/// The message for a speed that is not a whole number, such as `2.5`. The message names the range.
#[must_use]
pub fn time_scale_not_whole_message(got: impl std::fmt::Display) -> String {
    format!(
        "must be a whole number >= {SIMULATION_TIME_SCALE_MIN} and <= {SIMULATION_TIME_SCALE_MAX}, got {got}"
    )
}

/// The warning shown while simulation mode runs at `time_scale`.
#[must_use]
pub fn simulation_warning(time_scale: u32) -> String {
    format!(
        "Simulation mode active (time scale x{}). Not for live hardware.",
        time_scale.max(1)
    )
}

/// The speed of a running simulation. The resource loop reads it before every cycle, and `config.set` changes it.
#[derive(Debug, Clone)]
pub struct SimulationSpeed {
    scale: Arc<AtomicU32>,
}

impl SimulationSpeed {
    /// Creates a speed. Zero is raised to 1; callers check the permitted range with [`validate_time_scale`].
    #[must_use]
    pub fn new(scale: u32) -> Self {
        Self {
            scale: Arc::new(AtomicU32::new(scale.max(1))),
        }
    }

    /// The speed in force now.
    #[must_use]
    pub fn get(&self) -> u32 {
        self.scale.load(Ordering::SeqCst).max(1)
    }

    /// Changes the speed. It applies from the next cycle and never moves simulation time.
    pub fn set(&self, scale: u32) {
        self.scale.store(scale.max(1), Ordering::SeqCst);
    }
}

/// Simulation time advanced per unit of wall time over the recent cycles (specification 33, §6.1 item 3).
///
/// Each executed cycle records its wall-clock start and its simulation time. The window keeps the last five
/// seconds of wall time, or everything since the last [`clear`](Self::clear) when that is shorter.
#[derive(Debug, Default)]
pub struct AchievedSpeedWindow {
    samples: VecDeque<(i64, i64)>,
}

impl AchievedSpeedWindow {
    /// Forgets the samples, so the next measurement starts with the next cycle.
    pub fn clear(&mut self) {
        self.samples.clear();
    }

    /// Records one cycle start and returns the achieved speed, or `None` until one second of wall time is measured.
    pub fn record(&mut self, wall_nanos: i64, simulation_nanos: i64) -> Option<f64> {
        self.samples.push_back((wall_nanos, simulation_nanos));
        let oldest_kept = wall_nanos.saturating_sub(ACHIEVED_SPEED_WINDOW_NANOS);
        while self.samples.len() > 1
            && self
                .samples
                .front()
                .is_some_and(|(wall, _)| *wall < oldest_kept)
        {
            self.samples.pop_front();
        }
        let (first_wall, first_simulation) = *self.samples.front()?;
        let wall_span = wall_nanos.saturating_sub(first_wall);
        if wall_span < ACHIEVED_SPEED_MIN_SPAN_NANOS {
            return None;
        }
        let simulation_span = simulation_nanos.saturating_sub(first_simulation);
        Some(simulation_span as f64 / wall_span as f64)
    }
}
