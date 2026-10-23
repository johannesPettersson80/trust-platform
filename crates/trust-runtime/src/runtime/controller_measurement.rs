//! A Control.PID's measurement given by a simulated plant (Specification 46 §13.3, the `pid:`
//! feedback target): while a value is published for an instance, the `Input` that instance's
//! `Update` method receives carries it as `ProcessValue`. The program's own variables are not
//! written, so the controller sees the plant model however the program computes its measurement.

use crate::memory::InstanceId;
use crate::value::Value;

use super::installed::Runtime;

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

    pub(super) fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Gives the `Input` of an `Update` call on a measured instance the published
    /// `ProcessValue`. Only that field of that call changes.
    pub(super) fn substitute(
        &self,
        instance: InstanceId,
        method: &str,
        inputs: &mut trust_runtime_core::vm::MethodInputs<'_>,
    ) {
        if !method.eq_ignore_ascii_case("Update") {
            return;
        }
        let Some(measurement) = self.get(instance) else {
            return;
        };
        let Some(Value::Struct(input)) = inputs.input_mut("Input") else {
            return;
        };
        let input = std::sync::Arc::make_mut(input);
        let Some(field) = input
            .fields()
            .keys()
            .find(|name| name.eq_ignore_ascii_case("ProcessValue"))
            .cloned()
        else {
            return;
        };
        input.set_existing_field(field, Value::LReal(measurement));
    }
}

impl Runtime {
    /// Publishes a plant's measurement for a Control.PID instance; it stays until replaced.
    pub(crate) fn set_controller_measurement(&mut self, instance: InstanceId, value: f64) {
        self.state
            .services_mut()
            .measurements_mut()
            .set(instance, value);
    }

    /// The measurement published for an instance, if any.
    #[must_use]
    pub(crate) fn controller_measurement(&self, instance: InstanceId) -> Option<f64> {
        let measurements = self.state.services().measurements();
        if measurements.is_empty() {
            return None;
        }
        measurements.get(instance)
    }

    /// Removes every published measurement: the plant that published them is gone.
    pub(crate) fn clear_controller_measurements(&mut self) {
        self.state.services_mut().measurements_mut().values.clear();
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
