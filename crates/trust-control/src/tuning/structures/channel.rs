//! A measurement's own requirements (`ChannelRequirements` of Control Studio's loop channels; Specification 35, B4): the four values
//! that a measurement declares for its own band, in place of the loop's values for it.

#![allow(clippy::module_name_repetitions)] // `ChannelRequirements` names the module's subject.

use crate::tuning::requirements::TuningRequirements;

/// A measurement's own requirements: the setpoint change of its own test, the largest error allowed at its settling time, the
/// band around its final value, and the time by which it settles.
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelRequirements {
    /// The setpoint change of the measurement's own test.
    pub reference_change: f64,
    /// The largest error allowed at the settling time.
    pub maximum_error: f64,
    /// The band around the final value that the response must stay in.
    pub settling_band: f64,
    /// The time by which the response must settle, in seconds.
    pub settling_time_seconds: f64,
}

impl ChannelRequirements {
    /// The loop's requirements with these four values in place of the loop's own (`channelTask` of `cascadePidDesign.ts`); the
    /// other values stay as the loop has them.
    #[must_use]
    pub fn apply(&self, task: &TuningRequirements) -> TuningRequirements {
        TuningRequirements {
            reference_change: self.reference_change,
            maximum_error: self.maximum_error,
            settling_band: self.settling_band,
            settling_time_seconds: self.settling_time_seconds,
            ..task.clone()
        }
    }
}
