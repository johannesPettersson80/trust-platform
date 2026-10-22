//! The simulation facts a status answer reports about the target and its speed (specification 35, A2.4 and B9.2; specification 33,
//! §6.1). The runtime computes them here once. The status answer carries the speed words, the runtime's HMI display shows the same
//! speed from the same facts (it translates only the phrase for a speed that is behind), and the engine names the target from them.

#[cfg(test)]
#[path = "speed_tests.rs"]
mod tests;

/// The multiplication sign of the speed words (U+00D7).
const TIMES: char = '\u{D7}';

/// The achieved speed is behind when it is below this share of the configured speed (B9.2).
const BEHIND_SHARE: f64 = 0.95;

/// The facts of a status answer about the simulated target and its speed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeedFacts {
    /// Simulation is enabled and every attached I/O is simulated (A2.4). Anything else is physical.
    pub simulated: bool,
    /// The configured speed, when it is above 1x (B9.2); `None` at 1x and without simulation.
    pub configured: Option<u32>,
    /// The achieved speed, rounded down to one decimal, when it is behind the configured speed (B9.2); `None` otherwise.
    pub achieved_behind: Option<String>,
}

impl SpeedFacts {
    /// Whether the achieved speed is behind the configured one (B9.2).
    #[must_use]
    pub fn speed_behind(&self) -> bool {
        self.achieved_behind.is_some()
    }

    /// The speed words after the simulation's name (B9.2): "" at 1x, "×N" above 1x, and "×A of ×N" when the achieved speed A is behind.
    #[must_use]
    pub fn speed_text(&self) -> String {
        match (self.configured, &self.achieved_behind) {
            (None, _) => String::new(),
            (Some(configured), None) => format!("{TIMES}{configured}"),
            (Some(configured), Some(achieved)) => {
                format!("{TIMES}{achieved} of {TIMES}{configured}")
            }
        }
    }
}

/// The facts of a status answer, from the runtime's own state: whether simulation is enabled, whether every attached I/O is
/// simulated, the configured speed and the achieved speed (`None` until it is measured). Without simulation, and for a configured
/// speed of 0, the speed is 1 (specification 33, §6.1, item 5).
#[must_use]
pub fn speed_facts(
    enabled: bool,
    only_io: bool,
    configured: u32,
    achieved: Option<f64>,
) -> SpeedFacts {
    let configured = if enabled && configured > 0 {
        configured
    } else {
        1
    };
    let achieved = achieved.filter(|speed| enabled && speed.is_finite() && *speed > 0.0);
    let behind = configured > 1
        && achieved.is_some_and(|speed| speed < f64::from(configured) * BEHIND_SHARE);
    SpeedFacts {
        simulated: enabled && only_io,
        configured: (configured > 1).then_some(configured),
        achieved_behind: achieved.filter(|_| behind).map(format_speed),
    }
}

/// One decimal place, rounded down, so a shown achieved speed is never at or above the threshold it is behind (B9.2). A whole
/// speed has no decimal place.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // A speed behind a configured one of at most 100 is a small positive number.
fn format_speed(speed: f64) -> String {
    let tenths = (speed * 10.0 + 1e-9).floor() as u64;
    if tenths.is_multiple_of(10) {
        format!("{}", tenths / 10)
    } else {
        format!("{}.{}", tenths / 10, tenths % 10)
    }
}
