//! A Control.PID's measurement given by a simulated plant (Specification 33 §13.3, the `pid:`
//! feedback target): while a value is published for an instance, the `Input` that instance's
//! `Update` method receives carries it as `ProcessValue`. The program's own variables are not
//! written, so the controller sees the plant model however the program computes its measurement.

use crate::memory::InstanceId;

/// The published measurements, one per instance; empty unless a plant has a `pid:` target, so the
/// method-call path pays one empty check.
#[derive(Debug, Clone, Default)]
pub(crate) struct ControllerMeasurements {
    values: Vec<(InstanceId, f64)>,
}

impl ControllerMeasurements {
    fn set(&mut self, instance: InstanceId, value: f64) {
        match self.values.iter_mut().find(|(id, _)| *id == instance) {
            Some(entry) => entry.1 = value,
            None => self.values.push((instance, value)),
        }
    }

    fn get(&self, instance: InstanceId) -> Option<f64> {
        self.values
            .iter()
            .find(|(id, _)| *id == instance)
            .map(|(_, value)| *value)
    }
}

impl super::core::Runtime {
    /// Publishes a plant's measurement for a Control.PID instance; it stays until replaced.
    pub(crate) fn set_controller_measurement(&mut self, instance: InstanceId, value: f64) {
        self.controller_measurements.set(instance, value);
    }

    /// The measurement published for an instance, if any.
    #[must_use]
    pub(crate) fn controller_measurement(&self, instance: InstanceId) -> Option<f64> {
        if self.controller_measurements.values.is_empty() {
            return None;
        }
        self.controller_measurements.get(instance)
    }

    /// Removes every published measurement: the plant that published them is gone.
    pub(crate) fn clear_controller_measurements(&mut self) {
        self.controller_measurements.values.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::ControllerMeasurements;
    use crate::memory::InstanceId;

    #[test]
    fn a_measurement_is_replaced_per_instance_and_absent_for_others() {
        let mut measurements = ControllerMeasurements::default();
        measurements.set(InstanceId(4), 1.5);
        measurements.set(InstanceId(4), 2.5);
        measurements.set(InstanceId(9), 7.0);
        assert_eq!(measurements.get(InstanceId(4)), Some(2.5));
        assert_eq!(measurements.get(InstanceId(9)), Some(7.0));
        assert_eq!(measurements.get(InstanceId(5)), None);
    }
}
