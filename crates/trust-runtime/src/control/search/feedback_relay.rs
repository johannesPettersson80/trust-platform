//! Bounded feedback excitation inside the isolated actual-ST worker.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FeedbackRelay {
    pub process_variable: String,
    pub command: String,
    pub center: f64,
    pub hysteresis: f64,
    pub low: f64,
    pub high: f64,
    pub restore: f64,
    pub process_minimum: f64,
    pub process_maximum: f64,
    pub positive_gain: bool,
    pub start_ms: u64,
    pub restore_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_high_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_low_ms: Option<u64>,
}
impl FeedbackRelay {
    pub fn validate(&self, step: u64, duration: u64) -> Result<(), String> {
        if self.process_variable.is_empty()
            || self.command.is_empty()
            || self.process_variable.len() > 256
            || self.command.len() > 256
            || self.command.eq_ignore_ascii_case(&self.process_variable)
            || ![
                self.center,
                self.hysteresis,
                self.low,
                self.high,
                self.restore,
                self.process_minimum,
                self.process_maximum,
            ]
            .iter()
            .all(|v| v.is_finite())
            || self.hysteresis <= 0.0
            || self.low >= self.high
            || self.restore < self.low
            || self.restore > self.high
            || self.center - self.hysteresis <= self.process_minimum
            || self.center + self.hysteresis >= self.process_maximum
            || step == 0
            || self.start_ms % step != 0
            || self.restore_ms % step != 0
            || self.start_ms >= self.restore_ms
            || self.restore_ms > duration
            || self.minimum_high_ms.unwrap_or(0) % step != 0
            || self.minimum_low_ms.unwrap_or(0) % step != 0
            || self
                .minimum_high_ms
                .unwrap_or(0)
                .max(self.minimum_low_ms.unwrap_or(0))
                >= self.restore_ms - self.start_ms
        {
            return Err("invalid bounded feedback-relay declaration".into());
        }
        Ok(())
    }
    pub fn command_at(
        &self,
        time_ms: u64,
        measured: f64,
        high: &mut bool,
        last_switch_ms: &mut u64,
    ) -> Result<f64, String> {
        if !measured.is_finite()
            || measured < self.process_minimum
            || measured > self.process_maximum
        {
            return Err(
                "feedback relay stopped: process measurement outside declared bounds".into(),
            );
        }
        if time_ms < self.start_ms || time_ms >= self.restore_ms {
            return Ok(self.restore);
        }
        let next = super::super::relay_state::next_relay_state(
            measured,
            self.center,
            self.hysteresis,
            self.positive_gain,
            *high,
        );
        let dwell = if *high {
            self.minimum_high_ms.unwrap_or(0)
        } else {
            self.minimum_low_ms.unwrap_or(0)
        };
        let next_dwell = if next {
            self.minimum_high_ms.unwrap_or(0)
        } else {
            self.minimum_low_ms.unwrap_or(0)
        };
        if next != *high
            && time_ms.saturating_sub(*last_switch_ms) >= dwell
            && self.restore_ms.saturating_sub(time_ms) >= next_dwell
        {
            *high = next;
            *last_switch_ms = time_ms;
        }
        Ok(if *high { self.high } else { self.low })
    }
}
